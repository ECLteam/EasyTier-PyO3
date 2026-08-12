# EasyTier-PyO3

[![License: LGPL-3.0](https://img.shields.io/badge/License-LGPL--3.0-blue.svg)](LICENSE)

使用 [PyO3](https://pyo3.rs) 编写的 [EasyTier](https://github.com/EasyTier/EasyTier)
(开源 mesh P2P VPN) Python 绑定库。

可以在 Python 中直接创建、启动、管理 EasyTier 节点，查询对端/路由/指标快照，
管理接入凭证，以及订阅节点事件。

> 本项目以 **LGPL-3.0** 发布（与 easytier 一致），详见 [LICENSE](LICENSE) 与
> [NOTICE](NOTICE)。

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

- [安装与构建](#安装与构建)
- [快速开始](#快速开始)
- [配置说明](#配置说明)
- [API 参考](#api-参考)
- [事件订阅](#事件订阅)
- [常见问题](#常见问题)

---

## 安装与构建

直接安装（PyPI 发布后可用）：

```bash
pip install easytier-pyo3
```

源码构建 / 本地开发（环境要求、支持的平台架构、三种构建方式、构建 FAQ 等）
见 [**构建指南**](docs/BUILDING.md)。CI 会为每个平台自动构建
Python 3.11 / 3.12 / 3.13 的 wheel 并运行自测。

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

Windows 上创建 TUN 设备需要管理员权限；仅做连通性/对端发现测试时可关闭。
注意 `no_tun` 模式下必须同时设置 `bind_device = false`，否则客户端 socket
尝试绑定到虚拟 IP 会报 WSAEADDRNOTAVAIL(10049)：

```python
node = Node({
    "network_identity": {"network_name": "test", "network_secret": "test"},
    "flags": {"no_tun": True, "bind_device": False},
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
| `bind_device` | true | 把隧道 socket 绑定到虚拟 IP；同机回环测试（或连接报 WSAEADDRNOTAVAIL/10049 时）请设为 false |
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

**Q1：Windows 上创建 TUN 失败？**
需要以管理员权限运行，或在 `flags` 中设置 `no_tun = true`。

**Q2：如何卸载？**
`pip uninstall easytier-pyo3`。

**Q3：安装后创建 TUN 设备失败 / 提示找不到 wintun.dll？**
`python/easytier_py/` 里随包分发了 `wintun.dll`、`Packet.dll`、`WinDivert64.sys`。
模块导入时会把 pyd 所在目录加入进程 DLL 搜索路径（`AddDllDirectory`），
因此随包安装的 `wintun.dll` 能被 easytier 自动找到，无需手动放置。
仅做 `no_tun` 的连通性测试则不需要这些 DLL。

> TUN 已在 Windows 上实测通过（需管理员权限）：双节点建链后，
> 用 `tests/test_tun_manual.py` 可验证 TUN 数据面（ping 经隧道可达）。
> 同机回环测试时节点需设 `bind_device: False`，否则连接报 WSAEADDRNOTAVAIL。

**Q4：easytier 会不会影响我机器上的 Radmin VPN / 其它 VPN 网卡或劫持出站？**
不会。easytier 创建的是**自己独立的 wintun 网卡**（`et_*`），只添加
`虚拟网段/24` 的 on-link 路由，**不设默认路由**，不触碰其它网卡（实测
Radmin VPN 网卡的 IP/状态/路由在 easytier 运行前后完全一致）。停止节点后
网卡与路由自动清理。
