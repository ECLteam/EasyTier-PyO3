# EasyTier-PyO3

使用 [PyO3](https://pyo3.rs) 编写的 [EasyTier](https://github.com/EasyTier/EasyTier)
(易梯, 开源 mesh P2P VPN) Python 绑定库。

可以在 Python 中直接创建、启动、管理 EasyTier 节点，查询对端/路由/指标快照，
管理接入凭证，以及订阅节点事件。

```python
from easytier_py import Node

node = Node({
    "instance_name": "my-node",
    "network_identity": {"network_name": "net1", "network_secret": "secret"},
    "ipv4": "10.144.144.1/24",
    "listeners": ["tcp://0.0.0.0:11010"],
})
node.start()
print(node.state())        # Running
print(node.peer_id())
node.stop()
```

---

## 目录

- [从零开始构建](#从零开始构建)
- [快速开始](#快速开始)
- [配置说明](#配置说明)
- [API 参考](#api-参考)
- [事件订阅](#事件订阅)
- [常见问题](#常见问题)

---

## 从零开始构建

### 1. 环境要求

| 依赖 | 版本 | 说明 |
| --- | --- | --- |
| Rust | 稳定版 (1.85+) | 通过 [rustup](https://rustup.rs) 安装 |
| Python | 3.8+ | 64 位，建议使用虚拟环境 |
| maturin | >=1.5 | Rust/Python 桥接构建工具 |
| C/C++ 工具链 | — | Windows 需 Visual Studio Build Tools (C++ 工作负载)；Linux/macOS 需 gcc/clang |

> 首次构建会联网下载 EasyTier 的数百个依赖，并自动下载 `protoc`，
> 根据机器性能可能需要 **10 ~ 40 分钟**，属正常现象。

### 2. 安装 Rust 工具链

如果你还没有 Rust：

```bash
# Windows (PowerShell) 或 Linux/macOS (sh)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

> Windows 上请确保安装了 **Visual Studio Build Tools 2022**，并勾选
> “使用 C++ 的桌面开发” 工作负载，否则无法编译 EasyTier 依赖的 C 代码
> (wintun / windivert 等)。

### 3. 克隆项目

```bash
git clone <你的仓库地址> EasyTier-PyO3
cd EasyTier-PyO3
```

### 4. 创建 Python 虚拟环境并安装 maturin

```bash
# Windows
python -m venv .venv
.venv\Scripts\activate

# Linux / macOS
python3 -m venv .venv
source .venv/bin/activate

pip install -U pip maturin
```

### 5. 构建

方式一：开发模式安装（推荐，直接安装到当前虚拟环境）：

```bash
maturin develop
```

方式二：构建 wheel 后手动安装：

```bash
maturin build --release
pip install target/wheels/easytier_pyo3-*.whl
```

方式三：直接用 pip（通过项目内的 `pyproject.toml`）：

```bash
pip install .
```

### 6. 验证安装

```bash
python -c "from easytier_py import version; print(version())"
# 输出类似: 2.6.4
```

> 构建产物：`maturin develop` 会直接把 `easytier_py.pyd` 放入虚拟环境，
> `import easytier_py` 即可使用。

---

## 快速开始

### 最小示例（两台机器组网）

**节点 A：**

```python
from easytier_py import Node

node_a = Node({
    "instance_name": "node-a",
    "network_identity": {"network_name": "my-net", "network_secret": "topsecret"},
    "ipv4": "10.144.144.1/24",
    "listeners": ["tcp://0.0.0.0:11010"],   # 监听端口，供对端连接
})
node_a.start()
```

**节点 B：**

```python
from easytier_py import Node

node_b = Node({
    "instance_name": "node-b",
    "network_identity": {"network_name": "my-net", "network_secret": "topsecret"},
    "ipv4": "10.144.144.2/24",
    "peer": [{"uri": "tcp://<node-a-ip>:11010"}],  # 对端地址
})
node_b.start()
```

> 同一台机器上测试时，可让节点 B 显式连节点 A 的 `127.0.0.1:11010`。
> 两个节点 `network_name` / `network_secret` 必须一致。

### 不创建 TUN 设备（无管理员权限测试）

Windows 上创建 TUN 设备需要管理员权限；仅做连通性/对端发现测试时可关闭：

```python
node = Node({
    "network_identity": {"network_name": "test", "network_secret": "test"},
    "flags": {"no_tun": True},
})
```

### 运行时手动连接对端

```python
node.add_connector("tcp://10.0.0.5:11010")
node.remove_connector("tcp://10.0.0.5:11010")
```

---

## 配置说明

`Node()` 接受 **TOML 字符串** 或 **Python dict**（dict 中的 `None` 值会被忽略，
等价于不配置该字段）。

配置字段与 `easytier-core` 的配置文件一致：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `instance_name` | str | 节点名称 |
| `instance_id` | str(UUID) | 指定节点 ID（一般省略，自动生成） |
| `ipv4` | str | 虚拟 IPv4 地址，如 `10.144.144.1/24` |
| `ipv6` | str | 虚拟 IPv6 地址，如 `fd00::1/64` |
| `dhcp` | bool | 是否启用 DHCP 获取 IPv4 |
| `network_identity` | dict | `{"network_name": str, "network_secret": str}` |
| `listeners` | list[str] | 监听地址，如 `tcp://0.0.0.0:11010` |
| `mapped_listeners` | list[str] | 端口映射后的公网地址 |
| `exit_nodes` | list[str] | 出口节点 IP 列表 |
| `peer` | list[dict] | 手动对端，`{"uri": str, "peer_public_key": str?}` |
| `proxy_network` | list[dict] | 代理网段 `{"cidr": str, "allow": list[str]?}` |
| `routes` | list[str] | 路由网段列表 |
| `socks5_proxy` | str | SOCKS5 代理地址 |
| `port_forward` | list[dict] | 端口转发配置 |
| `secure_mode` | dict | 安全模式配置 |
| `acl` | dict | ACL 规则 |
| `tcp_whitelist` / `udp_whitelist` | list[str] | ACL 端口白名单 |
| `stun_servers` / `stun_servers_v6` | list[str] | 自定义 STUN 服务器 |
| `credential_file` | str | 凭证文件路径 |
| `flags` | dict | 运行时标志，见下表 |

### `flags` 常用项

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `no_tun` | false | 不创建 TUN 设备 |
| `dev_name` | "" | TUN 设备名 |
| `enable_ipv6` | true | 启用 IPv6 |
| `mtu` | 1380 | MTU |
| `default_protocol` | "tcp" | 默认传输协议 |
| `disable_p2p` | false | 禁用 P2P 直连 |
| `p2p_only` | false | 仅使用 P2P |
| `relay_network_whitelist` | "*" | 允许的中继网络白名单 |
| `enable_encryption` | true | 启用加密 |
| `encryption_algorithm` | "" | 加密算法 |
| `multi_thread` | true | 多线程模式 |
| `accept_dns` | false | 接受 DNS 服务 |
| `enable_exit_node` | false | 作为出口节点 |
| `proxy_forward_by_system` | false | 系统级代理转发 |

完整字段见 `easytier-core` 源码 `crates/easytier-core/src/config/toml.rs`。

---

## API 参考

完整 API 文档见 [docs/python_api.md](docs/python_api.md)。

模块级函数：

- `version() -> str`：EasyTier 内核版本号

类 `Node` 主要方法：

- 生命周期：`start()` / `stop()` / `wait()` / `state()` / `is_ready()` / `latest_error()`
- 信息：`instance_id()` / `instance_name()` / `peer_id()` / `running_listeners()` / `management_events()`
- 连接：`add_connector(url)` / `remove_connector(url)` / `clear_connectors()` / `connectors()`
- 快照：`peers()` / `node_info()` / `routes()` / `dump_route()` / `global_peer_map()` / `local_public_ipv6()` / `foreign_networks()`
- 统计：`metrics()` / `prometheus_metrics()` / `acl_stats()` / `acl_whitelist()`
- 凭证：`generate_credential()` / `revoke_credential()` / `upsert_credential()` / `credentials()`
- 事件：`events()` / `next_event(timeout)`

---

## 事件订阅

节点运行时会持续产生事件（对端加入/离开、连接建立/断开、TUN 就绪等），
可以通过 `events()` 或 `next_event()` 获取：

```python
import time

node.start()

# 非阻塞：取出当前所有待处理事件
for event in node.events():
    print(event)

# 阻塞：最多等 5 秒，取下一个事件
event = node.next_event(timeout=5.0)
print(event)
```

事件返回格式为 `{"事件名": 载荷}` 的 dict，例如：

```python
{"TunDeviceReady": "easytier0"}
{"PeerAdded": 123}
{"PeerConnAdded": {...}}
{"ConnectionAccepted": ["tcp://0.0.0.0:11010", "tcp://1.2.3.4:4567"]}
```

---

## 常见问题

**Q1：`maturin develop` 报错缺少 C 编译器？**
Windows 安装 Visual Studio Build Tools 并勾选 “使用 C++ 的桌面开发”。

**Q2：构建很慢 / 卡在下载？**
首次构建需下载 EasyTier 全部依赖及 `protoc`，请耐心等待；后续构建为增量，速度很快。

**Q3：Windows 上创建 TUN 失败？**
需要以管理员权限运行，或在 `flags` 中设置 `no_tun = true`。

**Q4：`pip install .` 与 `maturin develop` 有什么区别？**
`maturin develop` 直接安装到当前虚拟环境（调试方便）；`pip install .` 走完整 wheel
构建流程（产物更正式，可用 `--release` 优化）。

**Q5：如何卸载？**
`pip uninstall easytier-pyo3`。

**Q6：构建报错 `Could not find protoc`？**
EasyTier 的部分 proto 构建依赖需要 `protoc` 命令。本项目的 `prost-wkt-types`
依赖已通过 `vendored-protox` 特性改用纯 Rust 编译，无需 protoc；
但 `easytier-proto` 在 Linux/macOS 上仍需要系统 protoc。若遇到该错误：

- Windows：`easytier-proto` 会自动下载 protoc；也可用 `winget install protobuf` 安装。
- Linux：`sudo apt install protobuf-compiler`；macOS：`brew install protobuf`。
- 通用：安装后设置环境变量 `PROTOC` 指向 `protoc` 可执行文件路径再重新构建。

**Q7：构建报错 `Unable to find libclang`（bindgen）？**
Windows 下编译 `windivert` 等 C-FFI 依赖需要 `libclang.dll`。安装 LLVM
（`winget install LLVM.LLVM`），然后把其 `bin` 目录设置到 `LIBCLANG_PATH`
环境变量后重新构建。

> 本项目根目录的 `.cargo/config.toml` 已内置本机的 `PROTOC` 与 `LIBCLANG_PATH`
> 路径（cargo 会把它们注入所有构建脚本，从 PyCharm/pip 启动也能生效）。
> 更换机器后请按注释修改这两个路径。

**Q8：链接报错 `LNK1181: 无法打开输入文件"Packet.lib"`？**
easytier 的构建脚本给 `Packet.lib` 输出了相对链接路径，作为依赖库编译时该路径
解析不到（相对路径以最终链接目录为基准）。本项目已在 `third_party/x86_64/` 内置
`Packet.lib`，并由 `build.rs` 输出绝对链接路径解决，无需手动处理。

**Q9：安装后创建 TUN 设备失败 / 提示找不到 wintun.dll？**
`third_party/x86_64/` 内置了 `wintun.dll`、`Packet.dll`、`WinDivert64.sys`。
需要把它们放到 `easytier_py.pyd` 同目录（即 site-packages 下）才能使用
TUN / WinDivert 功能。仅做 `no_tun` 的连通性测试则无需这些 DLL。
