//! EasyTier 的 Python 绑定库。
//!
//! 通过 PyO3 将 EasyTier 的核心 API 暴露给 Python：
//! - `Node`：一个 EasyTier 节点（创建/启动/停止、连接管理、状态查询、
//!   路由/对端/指标快照、凭证管理、事件订阅等）
//! - `version()`：EasyTier 内核版本号
//!
//! 所有可能耗时的操作都会先释放 GIL（`py.detach`），避免阻塞其他 Python 线程。

use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use pyo3::IntoPyObjectExt;

use easytier::common::config::TomlConfig;
use easytier::common::global_ctx::GlobalCtxEvent;
use easytier::instance::factory::{
    NativeCoreInstance, create_native_instance, subscribe_native_instance_event,
};
use easytier_core::peers::credential_manager::{
    CredentialCreateOptions, CredentialUpsertOptions,
};

// ====================== 通用辅助函数 ======================

/// 把任意实现了 `Display` 的错误转换成 `PyRuntimeError`，方便直接 `?` 抛出。
fn to_py_err<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// 在释放 GIL 的情况下，用节点自己的 tokio runtime 阻塞执行一个异步闭包。
///
/// `fut` 接收节点的实例句柄并返回一个异步任务，其返回值必须是 `Send`。
fn run_blocking<R, F, Fut>(
    py: Python<'_>,
    instance: &Arc<NativeCoreInstance>,
    handle: &tokio::runtime::Handle,
    fut: F,
) -> PyResult<R>
where
    R: Send,
    F: FnOnce(Arc<NativeCoreInstance>) -> Fut + Send,
    Fut: std::future::Future<Output = R> + Send,
{
    let instance = instance.clone();
    let handle = handle.clone();
    Ok(py.detach(move || handle.block_on(fut(instance))))
}

/// 递归地把 Python 对象转换为 `serde_json::Value`。
///
/// 支持 dict / list / tuple / str / int / float / bool / None，其它类型报错。
fn py_value_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        return Ok(serde_json::Value::Null);
    }
    // 注意：bool 必须放在 int 前面判断，因为 Python 的 bool 是 int 的子类。
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(serde_json::Value::Bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(serde_json::Value::from(i));
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(serde_json::Value::from(f));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(serde_json::Value::String(s));
    }
    if let Ok(d) = obj.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in d.iter() {
            let key = k.extract::<String>().map_err(|_| {
                PyTypeError::new_err("config dict 的键必须是字符串")
            })?;
            map.insert(key, py_value_to_json(&v)?);
        }
        return Ok(serde_json::Value::Object(map));
    }
    if let Ok(list) = obj.cast::<PyList>() {
        return list
            .iter()
            .map(|item| py_value_to_json(&item))
            .collect::<PyResult<Vec<_>>>()
            .map(serde_json::Value::Array);
    }
    if let Ok(tuple) = obj.cast::<PyTuple>() {
        return tuple
            .iter()
            .map(|item| py_value_to_json(&item))
            .collect::<PyResult<Vec<_>>>()
            .map(serde_json::Value::Array);
    }
    Err(PyTypeError::new_err(format!(
        "不支持的配置类型: {}",
        obj.get_type().name()?
    )))
}

/// 把 `serde_json::Value` 转换为 `toml::Value`，自动跳过 `None` 字段。
///
/// TOML 没有 null，所以 dict 里的 `None` 值会被忽略（等价于不配置该字段）。
fn json_to_toml_value(value: &serde_json::Value) -> Option<toml::Value> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(b) => Some(toml::Value::Boolean(*b)),
        serde_json::Value::Number(n) => Some(match n.as_i64() {
            Some(i) => toml::Value::Integer(i),
            None => toml::Value::Float(n.as_f64().unwrap_or(0.0)),
        }),
        serde_json::Value::String(s) => Some(toml::Value::String(s.clone())),
        serde_json::Value::Array(items) => Some(toml::Value::Array(
            items.iter().filter_map(json_to_toml_value).collect(),
        )),
        serde_json::Value::Object(map) => Some(toml::Value::Table(
            map.iter()
                .filter_map(|(k, v)| json_to_toml_value(v).map(|v| (k.clone(), v)))
                .collect(),
        )),
    }
}

/// 把 Python 配置对象转成 TOML 文本。
///
/// - 传入 `str`：直接当作 TOML 配置文本
/// - 传入其它可序列化对象（如 `dict`）：先转 JSON，再转 TOML
fn config_to_toml_text(config: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(s) = config.extract::<&str>() {
        return Ok(s.to_owned());
    }
    let json = py_value_to_json(config)?;
    let value = json_to_toml_value(&json)
        .ok_or_else(|| PyValueError::new_err("config 不能为空"))?;
    toml::to_string(&value).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// 把 `serde_json::Value` 转回 Python 对象（dict / list / str / int / float / bool / None）。
fn json_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<Py<PyAny>> {
    Ok(match value {
        serde_json::Value::Null => py.None(),
        serde_json::Value::Bool(b) => (*b).into_py_any(py)?,
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py_any(py)?
            } else if let Some(u) = n.as_u64() {
                u.into_py_any(py)?
            } else {
                n.as_f64().unwrap_or(0.0).into_py_any(py)?
            }
        }
        serde_json::Value::String(s) => s.as_str().into_py_any(py)?,
        serde_json::Value::Array(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(json_to_py(py, item)?)?;
            }
            list.into_any().unbind()
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k.as_str(), json_to_py(py, v)?)?;
            }
            dict.into_any().unbind()
        }
    })
}

/// 把任意 serde 可序列化的值转成 Python 对象。
fn serde_to_py<T: serde::Serialize>(py: Python<'_>, value: T) -> PyResult<Py<PyAny>> {
    let json = serde_json::to_value(value).map_err(to_py_err)?;
    json_to_py(py, &json)
}

/// 把字节数组格式化为十六进制字符串。
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ====================== 节点类 ======================

/// 一个 EasyTier 节点。
///
/// 用 TOML 字符串或 dict 创建（字段与 `easytier-core` 的配置文件一致），
/// 每个节点拥有独立的 tokio runtime。用完请显式调用 `stop()` 优雅停止；
/// 对象被回收时仅在后台关闭 runtime，不保证清理 TUN 等系统资源。
#[pyclass(module = "easytier_py")]
struct Node {
    /// 底层 EasyTier 核心实例。
    instance: Arc<NativeCoreInstance>,
    /// 节点自己的 tokio runtime，承载所有后台任务。
    /// 用 `Option` 包裹，便于在 `Drop` 中取出所有权做后台关闭。
    runtime: Option<tokio::runtime::Runtime>,
    /// 节点事件总线订阅者，用于 `events()` / `next_event()`。
    /// 用 `Mutex` 包裹：保证这两个方法能用 `&self` 安全地取出/放回订阅者，
    /// 从而允许从多个 Python 线程并发调用节点方法（共享借用互不冲突）。
    event_rx: std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<GlobalCtxEvent>>>,
}

impl Node {
    /// 返回节点的 tokio runtime handle。
    fn handle(&self) -> tokio::runtime::Handle {
        self.runtime
            .as_ref()
            .expect("Node runtime 不应为空")
            .handle()
            .clone()
    }
}

#[pymethods]
impl Node {
    /// 创建节点（尚未启动）。
    ///
    /// 参数可以是 TOML 字符串或 dict，例如:
    /// ```python
    /// Node({"network_identity": {"network_name": "net", "network_secret": "key"},
    ///       "ipv4": "10.144.144.1/24"})
    /// ```
    #[new]
    fn new(config: &Bound<'_, PyAny>) -> PyResult<Self> {
        // 1) 把 Python 配置对象转成 TOML 文本。
        let toml_text = config_to_toml_text(config)?;
        // 2) 解析成 EasyTier 配置。
        let config = TomlConfig::new_from_str(&toml_text)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        // 3) 创建 tokio runtime，并在其中构建核心实例。
        let runtime = tokio::runtime::Runtime::new().map_err(to_py_err)?;
        let instance = runtime
            .block_on(async { create_native_instance(config) })
            .map_err(to_py_err)?;
        // 4) 订阅事件总线，供 events() / next_event() 使用。
        let event_rx = subscribe_native_instance_event(&instance);
        Ok(Self {
            instance,
            runtime: Some(runtime),
            event_rx: std::sync::Mutex::new(event_rx),
        })
    }

    // ---------- 生命周期 ----------

    /// 启动节点，阻塞直到启动完成；失败时抛出 `RuntimeError`。
    fn start(&self, py: Python<'_>) -> PyResult<()> {
        run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.start().await
        })?
        .map_err(to_py_err)
    }

    /// 停止节点（幂等）。
    fn stop(&self, py: Python<'_>) -> PyResult<()> {
        run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.stop().await;
        })
    }

    /// 阻塞直到节点停止。
    fn wait(&self, py: Python<'_>) -> PyResult<()> {
        run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.wait().await;
        })
    }

    // ---------- 基础信息 ----------

    /// 节点状态：Created / Starting / Running / Stopping / Stopped。
    fn state(&self) -> String {
        format!("{:?}", self.instance.state())
    }

    /// 是否已成功启动（状态为 Running）。
    fn is_ready(&self) -> bool {
        self.instance.is_ready()
    }

    /// 节点唯一 ID。
    fn instance_id(&self) -> String {
        self.instance.instance_id().to_string()
    }

    /// 节点名称。
    fn instance_name(&self) -> String {
        self.instance.instance_name().to_string()
    }

    /// 本节点在虚拟网络中的 peer id。
    fn peer_id(&self) -> u32 {
        self.instance.peer_id()
    }

    /// 最近一次启动/运行错误（没有则为 None）。
    fn latest_error(&self) -> Option<String> {
        self.instance.latest_error()
    }

    /// 最近产生的管理事件列表（字符串形式）。
    fn management_events(&self) -> Vec<String> {
        self.instance.management_events()
    }

    /// 当前正在监听的地址列表。
    fn running_listeners(&self) -> Vec<String> {
        self.instance
            .running_listeners()
            .into_iter()
            .map(|url| url.to_string())
            .collect()
    }

    /// 绑定一个已存在的 TUN 文件描述符（仅用于嵌入式场景）。
    fn attach_tun_fd(&self, fd: i32) -> PyResult<()> {
        self.instance.attach_tun_fd(fd).map_err(to_py_err)
    }

    // ---------- 手动连接管理 ----------

    /// 手动添加对端连接地址（运行时生效，无需重启）。
    fn add_connector(&self, url: &str) -> PyResult<()> {
        let url = url
            .parse::<url::Url>()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        self.instance.add_connector(url).map_err(to_py_err)
    }

    /// 移除一个手动连接地址，成功返回 True。
    fn remove_connector(&self, url: &str) -> bool {
        match url.parse::<url::Url>() {
            Ok(url) => self.instance.remove_connector(&url),
            Err(_) => false,
        }
    }

    /// 清空所有手动连接地址。
    fn clear_connectors(&self) {
        self.instance.clear_connectors();
    }

    /// 当前手动连接列表，每一项为 `{"url": str, "status": str}`。
    fn connectors(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let conns = self.instance.list_connectors();
        let arr = conns
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "url": c.url.to_string(),
                    "status": format!("{:?}", c.status),
                })
            })
            .collect();
        serde_to_py(py, serde_json::Value::Array(arr))
    }

    // ---------- 路由与对端快照 ----------

    /// 所有对端连接快照，每一项包含 peer_id 与连接信息。
    fn peers(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let snaps = run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.peer_snapshots().await
        })?;
        let mut arr = Vec::new();
        for s in snaps {
            let conns = serde_json::to_value(&s.conns).map_err(to_py_err)?;
            arr.push(serde_json::json!({
                "peer_id": s.peer_id,
                "default_conn_id": s.default_conn_id.map(|id| id.to_string()),
                "directly_connected_conns": s.directly_connected_conns
                    .iter().map(|id| id.to_string()).collect::<Vec<String>>(),
                "conns": conns,
            }));
        }
        serde_to_py(py, serde_json::Value::Array(arr))
    }

    /// 本节点的信息快照（IP、主机名、监听地址、版本、STUN 等）。
    fn node_info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let snap = run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.node_snapshot().await
        })?;
        let mut node = serde_json::Map::new();
        node.insert("peer_id".into(), serde_json::json!(snap.peer_id));
        node.insert(
            "ipv4_addr".into(),
            serde_json::json!(snap.ipv4_addr.as_ref().map(|v| v.to_string())),
        );
        node.insert(
            "proxy_networks".into(),
            serde_json::to_value(&snap.proxy_networks).map_err(to_py_err)?,
        );
        node.insert("hostname".into(), serde_json::json!(snap.hostname));
        node.insert(
            "stun_info".into(),
            serde_json::to_value(&snap.stun_info).map_err(to_py_err)?,
        );
        node.insert(
            "instance_id".into(),
            serde_json::json!(snap.instance_id.to_string()),
        );
        node.insert(
            "listeners".into(),
            serde_json::json!(
                snap.listeners.iter().map(|u| u.to_string()).collect::<Vec<String>>()
            ),
        );
        node.insert("version".into(), serde_json::json!(snap.version));
        node.insert(
            "feature_flags".into(),
            serde_json::to_value(&snap.feature_flags).map_err(to_py_err)?,
        );
        node.insert(
            "ip_list".into(),
            serde_json::to_value(&snap.ip_list).map_err(to_py_err)?,
        );
        node.insert(
            "public_ipv6_addr".into(),
            serde_json::json!(snap.public_ipv6_addr.as_ref().map(|v| v.to_string())),
        );
        node.insert(
            "ipv6_public_addr_prefix".into(),
            serde_json::json!(snap.ipv6_public_addr_prefix.as_ref().map(|v| v.to_string())),
        );
        serde_to_py(py, serde_json::Value::Object(node))
    }

    /// 当前路由快照列表。
    fn routes(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let routes = run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.route_snapshots().await
        })?;
        serde_to_py(py, serde_json::to_value(&routes).map_err(to_py_err)?)
    }

    /// 当前路由表文本（便于调试）。
    fn dump_route(&self, py: Python<'_>) -> PyResult<String> {
        run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.dump_route().await
        })
    }

    /// 全局对端图快照。
    fn global_peer_map(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let snap = run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.global_peer_map_snapshot()
        })?;
        serde_to_py(py, serde_json::to_value(&snap).map_err(to_py_err)?)
    }

    /// 本节点公网 IPv6 信息。
    fn local_public_ipv6(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let info = run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.local_public_ipv6_info().await
        })?;
        serde_to_py(py, serde_json::to_value(&info).map_err(to_py_err)?)
    }

    /// 外部网络（foreign network）的路由信息。
    fn foreign_network_route_infos(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let info = run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.foreign_network_route_infos().await
        })?;
        serde_to_py(py, serde_json::to_value(&info).map_err(to_py_err)?)
    }

    /// 所有外部网络的快照，键为网络名。
    fn foreign_networks(
        &self,
        py: Python<'_>,
        include_trusted_keys: bool,
    ) -> PyResult<Py<PyAny>> {
        let map = run_blocking(py, &self.instance, &self.handle(), move |i| async move {
            i.foreign_network_snapshots(include_trusted_keys).await
        })?;
        let mut obj = serde_json::Map::new();
        for (name, entry) in map {
            let peers = entry
                .peers
                .iter()
                .map(|p| {
                    let conns = serde_json::to_value(&p.conns)
                        .unwrap_or(serde_json::Value::Null);
                    serde_json::json!({ "peer_id": p.peer_id, "conns": conns })
                })
                .collect::<Vec<_>>();
            let trusted = entry
                .trusted_keys
                .iter()
                .map(|k| {
                    serde_json::json!({
                        "pubkey_hex": to_hex(&k.pubkey),
                        "source": format!("{:?}", k.source),
                        "expiry_unix": k.expiry_unix,
                    })
                })
                .collect::<Vec<_>>();
            obj.insert(
                name,
                serde_json::json!({
                    "network_secret_digest_hex": to_hex(&entry.network_secret_digest),
                    "my_peer_id": entry.my_peer_id_for_this_network,
                    "peers": peers,
                    "trusted_keys": trusted,
                }),
            );
        }
        serde_to_py(py, serde_json::Value::Object(obj))
    }

    /// 外部网络路由汇总。
    fn foreign_network_route_summary(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let summary = run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.foreign_network_route_summary().await
        })?;
        serde_to_py(py, serde_json::to_value(&summary).map_err(to_py_err)?)
    }

    // ---------- ACL 与统计 ----------

    /// ACL 统计信息。
    fn acl_stats(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let stats = self.instance.acl_stats();
        serde_to_py(py, serde_json::to_value(&stats).map_err(to_py_err)?)
    }

    /// 当前 ACL 白名单（TCP/UDP 端口列表）。
    fn acl_whitelist(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let snap = self.instance.acl_whitelist_snapshot();
        serde_to_py(
            py,
            serde_json::json!({
                "tcp_ports": snap.tcp_ports,
                "udp_ports": snap.udp_ports,
            }),
        )
    }

    /// 节点所有指标快照。
    fn metrics(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let snapshots = self.instance.metric_snapshots();
        serde_to_py(py, serde_json::to_value(&snapshots).map_err(to_py_err)?)
    }

    /// 以 Prometheus 文本格式导出的指标。
    fn prometheus_metrics(&self) -> String {
        self.instance.prometheus_metrics()
    }

    // ---------- 凭证管理（需要 admin 节点） ----------

    /// 生成一个接入凭证，返回 `{"credential_id", "secret", "expiry_unix", "changed"}`。
    #[pyo3(signature = (
        groups,
        allowed_proxy_cidrs,
        allow_relay = false,
        ttl_seconds = 3600.0,
        credential_id = None,
        reusable = true,
    ))]
    fn generate_credential(
        &self,
        py: Python<'_>,
        groups: Vec<String>,
        allowed_proxy_cidrs: Vec<String>,
        allow_relay: bool,
        ttl_seconds: f64,
        credential_id: Option<String>,
        reusable: bool,
    ) -> PyResult<Py<PyAny>> {
        let options = CredentialCreateOptions {
            groups,
            allow_relay,
            allowed_proxy_cidrs,
            ttl: Duration::from_secs_f64(ttl_seconds),
            credential_id,
            reusable,
        };
        let generated = self.instance.generate_credential(options).map_err(to_py_err)?;
        serde_to_py(
            py,
            serde_json::json!({
                "credential_id": generated.credential_id,
                "secret": generated.secret,
                "expiry_unix": generated.expiry_unix,
                "changed": generated.changed,
            }),
        )
    }

    /// 吊销一个凭证，成功返回 True。
    fn revoke_credential(&self, credential_id: &str) -> PyResult<bool> {
        self.instance
            .revoke_credential(credential_id)
            .map_err(to_py_err)
    }

    /// 导入一个已存在的凭证，发生变更返回 True。
    fn upsert_credential(
        &self,
        credential_id: String,
        credential_secret: String,
        groups: Vec<String>,
        allow_relay: bool,
        allowed_proxy_cidrs: Vec<String>,
        expiry_unix: i64,
        reusable: bool,
    ) -> PyResult<bool> {
        let options = CredentialUpsertOptions {
            credential_id,
            credential_secret,
            groups,
            allow_relay,
            allowed_proxy_cidrs,
            expiry_unix,
            reusable,
        };
        self.instance.upsert_credential(options).map_err(to_py_err)
    }

    /// 当前所有凭证列表。
    fn credentials(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let infos = self.instance.credential_snapshots();
        let arr = infos
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "credential_id": c.credential_id,
                    "groups": c.groups,
                    "allow_relay": c.allow_relay,
                    "expiry_unix": c.expiry_unix,
                    "allowed_proxy_cidrs": c.allowed_proxy_cidrs,
                    "reusable": c.reusable,
                    "public_key_fingerprint": c.public_key_fingerprint,
                })
            })
            .collect();
        serde_to_py(py, serde_json::Value::Array(arr))
    }

    // ---------- 其它运行时操作 ----------

    /// 关闭与指定对端的一条连接（conn_id 为 UUID 字符串）。
    fn close_peer_conn(&self, py: Python<'_>, peer_id: u32, conn_id: &str) -> PyResult<()> {
        let conn_id = uuid::Uuid::parse_str(conn_id)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let result = run_blocking(
            py,
            &self.instance,
            &self.handle(),
            move |i| async move { i.close_peer_conn(peer_id, &conn_id).await.map_err(to_py_err) },
        )?;
        result?;
        Ok(())
    }

    /// 运行时更新出口节点列表（IP 字符串列表）。
    fn update_exit_nodes(&self, py: Python<'_>, ips: Vec<String>) -> PyResult<()> {
        let ips = ips
            .into_iter()
            .map(|s| {
                s.parse::<IpAddr>()
                    .map_err(|e| PyValueError::new_err(e.to_string()))
            })
            .collect::<PyResult<Vec<_>>>()?;
        run_blocking(py, &self.instance, &self.handle(), move |i| async move {
            i.update_exit_nodes(ips).await;
        })
    }

    /// 刷新 ACL 组（读取路由信息后重新计算）。
    fn refresh_acl_groups(&self, py: Python<'_>) -> PyResult<()> {
        run_blocking(py, &self.instance, &self.handle(), |i| async move {
            i.refresh_acl_groups().await;
        })
    }

    // ---------- 事件订阅 ----------

    /// 取出当前所有待处理事件（不阻塞），返回事件 dict 列表。
    fn events(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut guard = self.event_rx.lock().unwrap();
        let Some(mut rx) = guard.take() else {
            return Err(PyRuntimeError::new_err("事件订阅不可用"));
        };
        // 先只取原始事件，放回 receiver 后再做序列化，
        // 这样即使某个事件序列化失败，订阅也不会丢失。
        let mut raw = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(ev) => raw.push(ev),
                Err(
                    tokio::sync::broadcast::error::TryRecvError::Empty
                    | tokio::sync::broadcast::error::TryRecvError::Lagged(_)
                    | tokio::sync::broadcast::error::TryRecvError::Closed,
                ) => break,
            }
        }
        *guard = Some(rx);
        drop(guard);
        raw.into_iter().map(|ev| serde_to_py(py, ev)).collect()
    }

    /// 阻塞等待下一个事件；给定 `timeout`（秒）超时后仍无事件则返回 None。
    fn next_event(&self, py: Python<'_>, timeout: Option<f64>) -> PyResult<Option<Py<PyAny>>> {
        let mut rx = {
            let mut guard = self.event_rx.lock().unwrap();
            match guard.take() {
                Some(rx) => rx,
                None => return Err(PyRuntimeError::new_err("事件订阅不可用")),
            }
        };
        let handle = self.handle();
        // 阻塞等待事件时释放 GIL；同时把 receiver 一并返回，避免丢失订阅。
        // 注意：tokio::time::timeout 必须在 runtime 上下文内构造，
        // 因此把它放进 block_on 的 async 块里，不能作为 block_on 的实参。
        let (event, rx) = py.detach(move || {
            let result = match timeout {
                Some(secs) => handle.block_on(async {
                    match tokio::time::timeout(Duration::from_secs_f64(secs), rx.recv()).await {
                        Ok(Ok(ev)) => Ok(Some(ev)),
                        Ok(Err(_)) => Err(PyRuntimeError::new_err("事件通道已关闭")),
                        Err(_) => Ok(None),
                    }
                }),
                None => match handle.block_on(rx.recv()) {
                    Ok(ev) => Ok(Some(ev)),
                    Err(_) => Err(PyRuntimeError::new_err("事件通道已关闭")),
                },
            };
            (result, rx)
        });
        *self.event_rx.lock().unwrap() = Some(rx);
        match event? {
            Some(ev) => Ok(Some(serde_to_py(py, ev)?)),
            None => Ok(None),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Node(name={:?}, id={}, state={})",
            self.instance.instance_name(),
            self.instance.instance_id(),
            self.state()
        )
    }
}

impl Drop for Node {
    /// 节点被回收时的清理。
    ///
    /// 注意：这里**不能**阻塞等待异步 `stop()`——pyo3 的 pyclass 析构
    /// 发生在持有 GIL 的 GC / 解释器退出阶段，此时 `block_on(stop())`
    /// 会与 tokio 后台任务互锁导致进程卡死。
    /// 因此只取出 runtime 并后台关闭（立即返回，不等待任务）；
    /// 需要优雅停止请显式调用 `stop()`。
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

// ====================== 模块级函数 ======================

/// EasyTier 内核版本号。
#[pyfunction]
fn version() -> String {
    easytier::VERSION.to_string()
}

/// 注册为 Python 模块 `easytier_py`。
#[pymodule]
fn easytier_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_class::<Node>()?;
    Ok(())
}
