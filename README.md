# EasyTier-PyO3

[![License: LGPL-3.0](https://img.shields.io/badge/License-LGPL--3.0-blue.svg)](LICENSE)

使用 [PyO3](https://pyo3.rs) 编写的 [EasyTier](https://github.com/EasyTier/EasyTier)
(开源 mesh P2P VPN) Python 绑定库。

可以在 Python 中直接创建、启动、管理 EasyTier 节点，查询对端/路由/指标快照，
管理接入凭证，以及订阅节点事件。

> 本项目以 **LGPL-3.0** 发布（与 easytier 一致），详见 [LICENSE](LICENSE) 与
> [NOTICE](NOTICE)。

```python
from easytier_pyo3 import Node

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

直接安装：

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
from easytier_pyo3 import Node

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
from easytier_pyo3 import Node

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

配置字段与 `easytier-core` 的配置文件一致（缺省值 = 不配置，行为见"默认值"列）：

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `instance_name` | str | `"default"` | 节点名称 |
| `instance_id` | str(UUID) | 自动生成随机 UUID v4 | 指定节点 ID（一般省略） |
| `hostname` | str | 不设置（空） | 节点主机名（等价 CLI `--hostname`；自动过滤控制字符、截断前 32 字符） |
| `netns` | str | 未配置 | 网络命名空间（仅 Linux） |
| `ipv4` | str | 未分配 | 虚拟 IPv4 地址，如 `10.144.144.1/24`（写成 `/32` 会自动按 `/24` 处理） |
| `ipv6` | str | 未分配 | 虚拟 IPv6 地址，如 `fd00::1/64` |
| `ipv6_public_addr_provider` | bool | false | 作为 IPv6 公网地址提供者 |
| `ipv6_public_addr_auto` | bool | false | 自动获取公网 IPv6 前缀 |
| `ipv6_public_addr_prefix` | str | 未配置 | IPv6 公网前缀，如 `2001:db8:100::/64` |
| `dhcp` | bool | false | 是否启用 DHCP 获取 IPv4 |
| `network_identity` | dict | `{"network_name": "default", "network_secret": ""}` | 网络身份 |
| `listeners` | list[str] | 空（不监听） | 监听地址，如 `tcp://0.0.0.0:11010` |
| `mapped_listeners` | list[str] | 空 | 端口映射后的公网地址 |
| `exit_nodes` | list[str] | 空 | 出口节点 IP 列表 |
| `peer` | list[dict] | 空 | 手动对端，`{"uri": str, "peer_public_key": str?}` |
| `proxy_network` | list[dict] | 空 | 代理网段 `{"cidr": str, "allow": list[str]?}` |
| `routes` | list[str] | 空 | 路由网段列表 |
| `socks5_proxy` | str | 不启用 | SOCKS5 代理地址 |
| `port_forward` | list[dict] | 空 | 端口转发配置 |
| `vpn_portal_config` | dict | 不启用 | VPN Portal 配置（WireGuard 客户端接入） |
| `secure_mode` | dict | 不启用 | 安全模式（私钥/公钥）配置 |
| `acl` | dict | 未配置（放行所有） | ACL 规则 |
| `tcp_whitelist` / `udp_whitelist` | list[str] | 空 | ACL 端口白名单 |
| `stun_servers` / `tcp_stun_servers` / `stun_servers_v6` | list[str] | 内置默认服务器（见下） | 自定义 STUN 服务器 |
| `credential_file` | str | 未配置 | 凭证文件路径 |
| `flags` | dict | 全部取默认值（见下） | 运行时标志 |

### `flags` 字段与默认值

`flags` 里未配置的字段一律取下表默认值（源码 `gen_default_flags()`，
见 `easytier-core/src/config/toml.rs`）：

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `default_protocol` | `"tcp"` | 默认传输协议 |
| `dev_name` | `""` | TUN 设备名 |
| `enable_encryption` | true | 启用加密 |
| `enable_ipv6` | true | 启用 IPv6 |
| `mtu` | 1380 | MTU |
| `latency_first` | false | 优先选择低延迟链路 |
| `enable_exit_node` | false | 作为出口节点 |
| `proxy_forward_by_system` | false | 系统级代理转发 |
| `no_tun` | false | 不创建 TUN 设备 |
| `use_smoltcp` | false | 使用用户态 smoltcp 协议栈 |
| `relay_network_whitelist` | `"*"` | 允许的中继网络白名单 |
| `disable_p2p` | false | 禁用 P2P 直连 |
| `p2p_only` | false | 仅使用 P2P |
| `lazy_p2p` | false | 懒 P2P（先走中继，延迟高再打洞） |
| `relay_all_peer_rpc` | false | 中继所有对端 RPC |
| `disable_tcp_hole_punching` | false | 禁用 TCP 打洞 |
| `disable_udp_hole_punching` | false | 禁用 UDP 打洞 |
| `disable_sym_hole_punching` | false | 禁用对称型 NAT 打洞 |
| `disable_upnp` | false | 禁用 UPnP 端口映射 |
| `multi_thread` | true | 多线程模式 |
| `multi_thread_count` | 2 | 多线程线程数 |
| `data_compress_algo` | 无压缩 | 数据压缩算法（如 `zstd`） |
| `bind_device` | true | 把隧道 socket 绑定到虚拟 IP；同机回环测试（或连接报 WSAEADDRNOTAVAIL/10049 时）请设为 false |
| `enable_kcp_proxy` | false | 启用 KCP 代理 |
| `disable_kcp_input` | false | 禁用 KCP 入站 |
| `disable_relay_kcp` | false | 禁用 KCP 中继 |
| `enable_relay_foreign_network_kcp` | false | 对外部网络启用 KCP 中继 |
| `accept_dns` | false | 接受 DNS 服务 |
| `private_mode` | false | 私密模式 |
| `enable_quic_proxy` | false | 启用 QUIC 代理 |
| `disable_quic_input` | false | 禁用 QUIC 入站 |
| `disable_relay_quic` | false | 禁用 QUIC 中继 |
| `enable_relay_foreign_network_quic` | false | 对外部网络启用 QUIC 中继 |
| `quic_listen_port` | 自动分配 | QUIC 监听端口 |
| `need_p2p` | false | 强制要求 P2P 连接 |
| `foreign_relay_bps_limit` | 不限速（`u64::MAX`） | 对外中继限速（B/s） |
| `instance_recv_bps_limit` | 不限速（`u64::MAX`） | 节点接收限速（B/s） |
| `disable_relay_data` | false | 禁用数据中继 |
| `enable_udp_broadcast_relay` | false | 启用 UDP 广播中继 |
| `socket_mark` | 不设置 | Linux socket SO_MARK 标记 |
| `encryption_algorithm` | `"aes-gcm"` | 加密算法（`aes-gcm` / `aes-256-gcm` / `chacha20` / `xor`） |
| `tld_dns_zone` | `"et.net."` | 虚拟 DNS 域名后缀 |

### 其它内置默认值

- **默认 STUN 服务器**（未配置 `stun_servers` 系列字段时使用，源码
  `easytier-core/src/config/mod.rs`）：
  - UDP v4：`stun.easytier.cn`、`stun.miwifi.com`、`stun.chat.bilibili.com`、`stun.hitv.com`
  - TCP：`stun.hot-chilli.net`、`stun.fitauto.ru`、`fwa.lifesizecloud.com`、
    `global.turn.twilio.com`、`turn.cloudflare.com`、`stun.voip.blackberry.com`、`stun.radiojar.com`
  - UDP v6：`stun-v6.easytier.cn`
- **默认监听端口 11010 是 easytier CLI 的默认值，本库 `Node` 不经过 CLI**：
  不配置 `listeners` 时节点**不会监听任何端口**，请显式指定。
- 数据来源：easytier-core 源码 `easytier-core/src/config/toml.rs`（`Config` 结构体与
  `gen_default_flags()`）与 `easytier-core/src/config/mod.rs`（`NetworkIdentity`、
  `PeerPolicyConfig`、STUN 服务器常量）。

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
wheel 内随包分发 `wintun.dll`、`Packet.dll`、`WinDivert64.sys`（构建时由
`build.rs` 从 `third_party/<arch>/` 自动复制进包，与 Python 架构一致）。
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
