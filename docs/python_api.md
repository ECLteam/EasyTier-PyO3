# EasyTier-PyO3 Python API 参考

> 模块名：`easytier_py`
> 版本：0.1.0

本文档覆盖该绑定库暴露给 Python 的全部接口。所有“耗时”操作（`start`、`stop`、
各类快照查询等）都会在内部释放 GIL，不会阻塞其他 Python 线程。

---

## 目录

- [模块级函数](#模块级函数)
- [class Node](#class-node)
  - [构造与生命周期](#构造与生命周期)
  - [基础信息](#基础信息)
  - [手动连接管理](#手动连接管理)
  - [路由与对端快照](#路由与对端快照)
  - [ACL 与统计](#acl-与统计)
  - [凭证管理](#凭证管理)
  - [其它运行时操作](#其它运行时操作)
  - [事件订阅](#事件订阅)
- [返回数据格式汇总](#返回数据格式汇总)
- [完整示例](#完整示例)

---

## 模块级函数

### `version() -> str`

返回 EasyTier 内核版本号。

```python
>>> from easytier_py import version
>>> version()
'2.6.4'
```

---

## class Node

```python
Node(config: Union[str, dict])
```

表示一个 EasyTier 节点。每个节点拥有**独立的 tokio runtime** 与事件总线。

- `config` 为 **TOML 字符串** 时直接作为配置；
- `config` 为 **dict** 时自动转换为 TOML（dict 中的 `None` 值被忽略）；
- 字段与 `easytier-core` 配置文件一致，详见 [README 配置说明](../README.md#配置说明)。

创建节点**不会**立即启动，需要调用 `start()`。`stop()` 是幂等的，请显式调用以
优雅停止并释放资源；节点被垃圾回收时只会在后台关闭其运行时（不阻塞、不保证清理
TUN 等资源），因此**用完务必调用 `stop()`**。

> **线程安全**：`Node` 的所有方法都可在多个 Python 线程中并发调用
> （内部通过锁保护可变状态，不会抛 `Already mutably borrowed`），
> 适合"后台线程等事件 + 主线程查询"的用法。

```python
from easytier_py import Node

# 用 dict 创建
node = Node({
    "instance_name": "node-a",
    "network_identity": {"network_name": "my-net", "network_secret": "secret"},
    "ipv4": "10.144.144.1/24",
    "listeners": ["tcp://0.0.0.0:11010"],
})

# 或用 TOML 字符串创建
node2 = Node("""
instance_name = "node-b"
network_identity = { network_name = "my-net", network_secret = "secret" }
ipv4 = "10.144.144.2/24"
listeners = ["tcp://0.0.0.0:11011"]
""")
```

---

### 构造与生命周期

#### `start() -> None`

启动节点，**阻塞直到启动完成**。失败时抛出 `RuntimeError`。

```python
node.start()          # 阻塞，直到成功启动或报错
assert node.state() == "Running"
```

#### `stop() -> None`

停止节点（幂等，可重复调用）。停止后节点状态为 `Stopped`。

```python
node.stop()
assert node.state() == "Stopped"
```

#### `wait() -> None`

阻塞直到节点停止（例如被其他线程调用 `stop()` 或内部出错）。

#### `state() -> str`

返回节点状态字符串，取值：`Created` / `Starting` / `Running` / `Stopping` / `Stopped`。

```python
>>> node.state()
'Created'
>>> node.start(); node.state()
'Running'
```

#### `is_ready() -> bool`

是否已成功启动（`state() == "Running"`）。

#### `latest_error() -> Optional[str]`

最近一次启动/运行错误。没有错误时为 `None`。

```python
err = node.latest_error()
if err:
    print(f"节点错误: {err}")
```

---

### 基础信息

#### `instance_id() -> str`

节点唯一 ID（UUID 字符串）。

```python
>>> node.instance_id()
'0b1f2e3d-...'
```

#### `instance_name() -> str`

节点名称（来自配置的 `instance_name`）。

#### `peer_id() -> int`

本节点在虚拟网络中的 peer id。

```python
>>> node.peer_id()
12345
```

#### `running_listeners() -> List[str]`

当前正在监听的地址列表。

```python
>>> node.running_listeners()
['tcp://0.0.0.0:11010']
```

#### `management_events() -> List[str]`

最近产生的管理事件（字符串形式，调试用）。

#### `attach_tun_fd(fd: int) -> None`

绑定一个已存在的 TUN 文件描述符（嵌入式场景使用）。

---

### 手动连接管理

#### `add_connector(url: str) -> None`

运行时手动添加一个对端连接地址，**无需重启节点**。URL 非法时抛出 `ValueError`。

```python
node.add_connector("tcp://192.168.1.100:11010")
```

#### `remove_connector(url: str) -> bool`

移除一个手动连接地址；该地址存在且被移除返回 `True`，否则返回 `False`。

#### `clear_connectors() -> None`

清空所有手动连接地址。

#### `connectors() -> List[dict]`

当前手动连接列表。每一项：

```python
[
    {
        "url": "tcp://192.168.1.100:11010",
        "status": "Connecting",   # Connected / Disconnected / Connecting
    },
]
```

---

### 路由与对端快照

> 以下查询方法内部会阻塞等待 EasyTier 返回，返回格式均为 Python dict / list。

#### `peers() -> List[dict]`

所有对端的连接快照。每一项：

```python
[
    {
        "peer_id": 12346,
        "default_conn_id": "uuid-string",           # 主连接 id（可为 None）
        "directly_connected_conns": ["uuid-string"],
        "conns": [ { ... }, ... ],                  # PeerConnInfo 详情
    },
]
```

#### `node_info() -> dict`

本节点信息快照：

```python
{
    "peer_id": 12345,
    "ipv4_addr": "10.144.144.1/24",      # 可能为 None
    "proxy_networks": [ ... ],
    "hostname": "my-host",
    "stun_info": { ... },
    "instance_id": "uuid-string",
    "listeners": ["tcp://0.0.0.0:11010"],
    "version": "2.6.4",
    "feature_flags": { ... },
    "ip_list": { ... },
    "public_ipv6_addr": "2001:db8::1/64",   # 可能为 None
    "ipv6_public_addr_prefix": None,
}
```

#### `routes() -> List[dict]`

当前路由快照列表（结构来自 EasyTier 的 proto `Route`，pbjson 序列化）。

#### `dump_route() -> str`

当前路由表的文本形式，便于调试。

#### `global_peer_map() -> dict`

全局对端图快照（proto `GetGlobalPeerMapResponse` 序列化结果）。

#### `local_public_ipv6() -> dict`

本节点公网 IPv6 信息。

#### `foreign_network_route_infos() -> dict`

外部网络（foreign network）路由信息。

#### `foreign_networks(include_trusted_keys: bool) -> dict`

所有外部网络的快照，键为网络名：

```python
{
    "net-1": {
        "network_secret_digest_hex": "a1b2c3...",
        "my_peer_id": 12345,
        "peers": [
            {"peer_id": 67890, "conns": [ ... ]},
        ],
        "trusted_keys": [
            {"pubkey_hex": "abcd...", "source": "SomeSource", "expiry_unix": 1700000000},
        ],
    },
}
```

#### `foreign_network_route_summary() -> dict`

外部网络路由汇总。

---

### ACL 与统计

#### `acl_stats() -> dict`

ACL 统计信息（proto `AclStats` 序列化结果）。

#### `acl_whitelist() -> dict`

当前 ACL 白名单：

```python
{
    "tcp_ports": ["80", "443"],
    "udp_ports": ["53"],
}
```

#### `metrics() -> List[dict]`

节点所有指标快照：

```python
[
    {
        "name": "some.metric",
        "labels": {...},
        "value": 123,
    },
]
```

#### `prometheus_metrics() -> str`

以 Prometheus 文本格式导出的指标。

```python
print(node.prometheus_metrics())
# easytier_xxx_total 123
# ...
```

---

### 凭证管理

> 仅 **admin 节点**（配置了 `network_secret`）可以生成/吊销/导入凭证，否则抛出 `RuntimeError`。

#### `generate_credential(...) -> dict`

生成一个接入凭证。

签名（`groups`、`allowed_proxy_cidrs` 必填，其余有默认值）：

```python
generate_credential(
    groups: List[str],
    allowed_proxy_cidrs: List[str],
    allow_relay: bool = False,
    ttl_seconds: float = 3600.0,        # 有效期，秒
    credential_id: Optional[str] = None,
    reusable: bool = True,              # 是否可重复使用
) -> dict
```

返回值：

```python
{
    "credential_id": "abc",
    "secret": "def",          # 分发给对端作为 network_secret
    "expiry_unix": 1700000000,
    "changed": True,
}
```

```python
cred = node.generate_credential(groups=["admin"], allowed_proxy_cidrs=[], ttl_seconds=86400)
print(cred["credential_id"], cred["secret"])
```

#### `revoke_credential(credential_id: str) -> bool`

吊销一个凭证；成功吊销返回 `True`，凭证不存在返回 `False`。

#### `upsert_credential(...) -> bool`

导入/更新一个已存在的凭证。签名：

```python
upsert_credential(
    credential_id: str,
    credential_secret: str,
    groups: List[str],
    allow_relay: bool,
    allowed_proxy_cidrs: List[str],
    expiry_unix: int,          # 过期时间戳（Unix 秒）
    reusable: bool,
) -> bool                     # 是否发生变更
```

#### `credentials() -> List[dict]`

当前所有凭证列表：

```python
[
    {
        "credential_id": "abc",
        "groups": ["admin"],
        "allow_relay": False,
        "expiry_unix": 1700000000,
        "allowed_proxy_cidrs": [],
        "reusable": True,
        "public_key_fingerprint": "...",
    },
]
```

---

### 其它运行时操作

#### `close_peer_conn(peer_id: int, conn_id: str) -> None`

关闭与指定对端的一条连接。`conn_id` 为连接 UUID 字符串（可从 `peers()` 中获取）。

#### `update_exit_nodes(ips: List[str]) -> None`

运行时更新出口节点列表。

```python
node.update_exit_nodes(["1.2.3.4", "5.6.7.8"])
```

#### `refresh_acl_groups() -> None`

刷新 ACL 组（读取路由信息后重新计算）。

---

### 事件订阅

节点运行时会持续产生事件。事件以 `{"事件名": 载荷}` 的形式返回。

常用事件：

| 事件 | 载荷 | 说明 |
| --- | --- | --- |
| `TunDeviceReady` | str | TUN 设备就绪 |
| `TunDeviceError` | str | TUN 设备错误 |
| `PeerAdded` / `PeerRemoved` | int (peer_id) | 对端加入/离开 |
| `PeerConnAdded` / `PeerConnRemoved` | dict | 对端连接建立/断开 |
| `ConnectionAccepted` | [local, remote] | 隧道建立 |
| `ConnectionError` | [local, remote, msg] | 隧道错误 |
| `ListenerAdded` | str | 监听启动成功 |
| `ListenerAddFailed` | [url, msg] | 监听启动失败 |
| `CredentialChanged` | None | 凭证变更 |
| `VpnPortalStarted` | str | VPN Portal 启动 |

#### `events() -> List[dict]`

非阻塞地取出当前所有待处理事件并清空缓冲。

```python
for event in node.events():
    print(event)
```

#### `next_event(timeout: Optional[float] = None) -> Optional[dict]`

阻塞等待下一个事件；`timeout` 秒内无事件返回 `None`。不传 `timeout` 表示一直等待。

```python
event = node.next_event(timeout=5.0)
if event is not None:
    print(event)
```

---

## 返回数据格式汇总

| 方法 | 返回类型 |
| --- | --- |
| `version` | `str` |
| `start` / `stop` / `wait` | `None` |
| `state` | `str` |
| `is_ready` | `bool` |
| `latest_error` | `Optional[str]` |
| `instance_id` / `instance_name` | `str` |
| `peer_id` | `int` |
| `running_listeners` / `management_events` | `List[str]` |
| `add_connector` / `attach_tun_fd` | `None` |
| `remove_connector` / `revoke_credential` / `upsert_credential` | `bool` |
| `connectors` / `peers` / `routes` / `credentials` / `metrics` / `events` | `List[dict]` |
| `node_info` / `global_peer_map` / `local_public_ipv6` / `foreign_networks` / `foreign_network_route_infos` / `foreign_network_route_summary` / `acl_stats` / `acl_whitelist` / `generate_credential` | `dict` |
| `dump_route` / `prometheus_metrics` | `str` |
| `next_event` | `Optional[dict]` |
| `close_peer_conn` / `update_exit_nodes` / `refresh_acl_groups` / `clear_connectors` | `None` |

---

## 完整示例

### 单机双节点连通性测试（不创建 TUN）

```python
from easytier_py import Node
import time

net = {"network_name": "demo", "network_secret": "demo-secret"}
# no_tun 模式下必须 bind_device=False，否则 Windows 连接报 10049。
flags = {"no_tun": True, "bind_device": False}

# 节点 A 监听 11010
a = Node({
    "instance_name": "a",
    "network_identity": net,
    "flags": flags,
    "listeners": ["tcp://127.0.0.1:11010"],
})
a.start()

# 节点 B 连节点 A
b = Node({
    "instance_name": "b",
    "network_identity": net,
    "flags": flags,
    "peer": [{"uri": "tcp://127.0.0.1:11010"}],
})
b.start()

# 等待对端建立连接
deadline = time.time() + 10
while time.time() < deadline:
    if b.peers():
        break
    time.sleep(0.5)

print("A peers:", a.peers())
print("B peers:", b.peers())
print("B 路由:", b.dump_route())

a.stop()
b.stop()
```

### 事件订阅 + 生命周期管理

```python
from easytier_py import Node
import threading, time

node = Node({
    "network_identity": {"network_name": "evt", "network_secret": "evt"},
    "flags": {"no_tun": True, "bind_device": False},
    "listeners": ["tcp://127.0.0.1:11012"],
})

def printer():
    while True:
        ev = node.next_event(timeout=2.0)
        if ev is not None:
            print("事件:", ev)

t = threading.Thread(target=printer, daemon=True)
t.start()

node.start()
time.sleep(3)
node.stop()
```

> 提示：`next_event` 内部释放 GIL，因此可以在后台线程中长期阻塞等待，
> 不会阻塞主线程。
