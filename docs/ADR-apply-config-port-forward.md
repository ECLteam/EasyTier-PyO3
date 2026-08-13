# ADR-014：Node.apply_config 运行时配置覆盖（port-forward 动态转发）

- 状态：已实施
- 日期：2026-08-14
- 参与：代码移植（easytier CLI port-forward → PyO3 绑定）/ 运行时配置

## 背景

用户要求"像 CLI 一样 port-forward，将虚拟 IP 的端口绑定到本地端口转发"。
easytier CLI 支持两种 port-forward 方式：

1. **启动配置**：`easytier-core -p tcp://0.0.0.0:8080/10.1.1.1:80`（URL 形式），
   写入 `TomlConfig.port_forward`，启动时由 `PortForwardAdapter` 绑定本地端口并
   经数据面转发到虚拟 IP。
2. **运行时动态**：`easytier-cli port-forward add/remove/list`，走
   `apply_config_patch` → `InstanceConfigPatch.port_forwards` → `patch_port_forwards`
   → `update_runtime_config` → `port_forward_adapter.reload()`（立即生效，无需重启）。

本项目 Node 只有启动期配置（`port_forward` 字段已透传），缺运行时动态能力。

## 可行性证据

- `easytier-core` 的 `proxy-smoltcp-stack` feature 已通过 easytier 默认 features
  传递启用：`gateway/port_forward.rs`（PortForwardAdapter）已编译进产物。
- `web-client` feature 已启用：`management/full/config_patch.rs`
  （`apply_config_patch`）已编译进产物。
- `CoreInstance::from_toml` 保留共享 `TomlConfig`（`toml_config()` 可用），
  满足 `apply_config_patch` 的候选快照契约。
- proto 类型（`InstanceConfigPatch` / `PortForwardPatch` / `PortForwardConfigPb` 等）
  经 `easytier::proto::api::config` 与 `easytier::proto::common` 直接可用。

## 决策

采用**新增 `Node.apply_config(config)`**，复用 easytier-core 的
`apply_config_patch`，不自己实现 socket 转发：

1. `apply_config` 接收与 `__init__` 相同的 TOML 字符串 / dict，解析为 `TomlConfig`。
2. `build_config_patch` 只 patch **显式出现**的顶层 key；集合类字段
   （`port_forward` / `routes` / `exit_nodes` / `proxy_network` /
   `mapped_listeners` / ACL 白名单）用 CLEAR + 逐条 ADD 实现**全量覆盖**。
3. 调用 `easytier_core::management::apply_config_patch(&instance, patch)`，
   节点必须 Running（与 CLI 一致）。

未拆 `add_port_forward` / `remove_port_forward` 单独方法：用户明确"不需要那么多
包装，像创建节点一样写配置覆盖它"，保持与 `__init__` 对称的单一入口。

## 关键映射决策

| 事项 | 决策 |
|---|---|
| CLI `port-forward add` | `apply_config({"port_forward": [{bind_addr, dst_addr, proto}]})` |
| CLI `port-forward remove` | `apply_config({"port_forward": [...]})` 全量覆盖（CLEAR+ADD） |
| CLI `port-forward list` | 未提供查询方法（后续需要再加 `port_forwards()`） |
| 协议校验 | 非 tcp/udp 抛 `ValueError`（对齐 CLI `apply_port_forward_modify`） |
| 覆盖语义 | 只覆盖显式出现的 key；集合字段全量覆盖（CLEAR+ADD） |
| 运行时配置入口 | 单一 `apply_config(config)`，与 `__init__` 格式对称 |
| 依赖 | `cidr = "0.3"` 新增（routes/proxy_network 转 proto Ipv4Inet） |

## 遗留技术债

- `port_forward` 的 `allow` 字段（proxy_network）无 proto 对应，传入会被丢弃
  （与 CLI 的 `ProxyNetworkPatch` 行为一致）。
- 数据面连通验证需 TUN 或 smoltcp 数据面；no_tun 模式测试仅验证配置层
  端口绑定（本地端口被 PortForwardAdapter 监听）。
- 未提供运行时查询当前转发的 `port_forwards()`；如需可与 CLI `port-forward list`
  对齐后补充。
- `connectors`（`--peer` 运行时增删）未纳入 `apply_config` 字段提取（走既有
  `add_connector`/`remove_connector` 方法）。

## 验证

- `cargo build --lib` / `python -m maturin build --release` 通过。
- `tests/test_port_forward_manual.py`：9/9 PASS（端口绑定/覆盖/清空/协议校验/TOML 形式）。
- `tests/test_easytier.py`：30/30 PASS（回归无破坏）。
- QA 快照比对（临时脚本）全部通过：非法协议拒绝、端口绑定、覆盖语义、
  清空、多字段共存。
