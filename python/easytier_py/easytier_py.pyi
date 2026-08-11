"""EasyTier-PyO3 类型存根（供 IDE / 类型检查器使用）。

PyCharm 等 IDE 无法静态分析编译型 .pyd 模块的成员，
此存根让编辑器能正确补全与提示所有 API。
"""

from typing import Any, Dict, List, Optional, Union

# Node 的配置可以是 TOML 文本，也可以是可序列化的 dict（dict 中的 None 会被忽略）。
Config = Union[str, Dict[str, Any]]


def version() -> str:
    """返回 EasyTier 内核版本号。"""
    ...


class Node:
    """一个 EasyTier 节点。

    用 TOML 字符串或 dict 创建（字段与 easytier-core 配置文件一致）。
    用完请显式调用 stop() 以优雅停止；对象被回收时仅在后台关闭其运行时，
    不保证清理 TUN 等系统资源。

    线程安全：所有方法均可在多个 Python 线程中并发调用（内部线程安全，
    不会出现 "Already mutably borrowed"）。
    """

    def __init__(self, config: Config) -> None:
        ...

    # ---------- 生命周期 ----------

    def start(self) -> None:
        """启动节点，阻塞直到启动完成；失败抛 RuntimeError。"""
        ...

    def stop(self) -> None:
        """停止节点（幂等）。"""
        ...

    def wait(self) -> None:
        """阻塞直到节点停止。"""
        ...

    # ---------- 基础信息 ----------

    def state(self) -> str:
        """节点状态: Created / Starting / Running / Stopping / Stopped。"""
        ...

    def is_ready(self) -> bool:
        """是否已成功启动（状态为 Running）。"""
        ...

    def latest_error(self) -> Optional[str]:
        """最近一次启动/运行错误，没有则为 None。"""
        ...

    def instance_id(self) -> str:
        """节点唯一 ID（UUID）。"""
        ...

    def instance_name(self) -> str:
        """节点名称。"""
        ...

    def peer_id(self) -> int:
        """本节点在虚拟网络中的 peer id。"""
        ...

    def running_listeners(self) -> List[str]:
        """当前正在监听的地址列表。"""
        ...

    def management_events(self) -> List[str]:
        """最近产生的管理事件（字符串列表）。"""
        ...

    def attach_tun_fd(self, fd: int) -> None:
        """绑定一个已存在的 TUN 文件描述符。"""
        ...

    # ---------- 手动连接管理 ----------

    def add_connector(self, url: str) -> None:
        """运行时手动添加对端连接地址。"""
        ...

    def remove_connector(self, url: str) -> bool:
        """移除手动连接地址，成功返回 True。"""
        ...

    def clear_connectors(self) -> None:
        """清空所有手动连接地址。"""
        ...

    def connectors(self) -> List[Dict[str, Any]]:
        """当前手动连接列表，每一项为 {"url": str, "status": str}。"""
        ...

    # ---------- 路由与对端快照 ----------

    def peers(self) -> List[Dict[str, Any]]:
        """所有对端连接快照。"""
        ...

    def node_info(self) -> Dict[str, Any]:
        """本节点信息快照。"""
        ...

    def routes(self) -> List[Dict[str, Any]]:
        """当前路由快照列表。"""
        ...

    def dump_route(self) -> str:
        """当前路由表文本。"""
        ...

    def global_peer_map(self) -> Dict[str, Any]:
        """全局对端图快照。"""
        ...

    def local_public_ipv6(self) -> Dict[str, Any]:
        """本节点公网 IPv6 信息。"""
        ...

    def foreign_network_route_infos(self) -> Dict[str, Any]:
        """外部网络的路由信息。"""
        ...

    def foreign_networks(self, include_trusted_keys: bool) -> Dict[str, Any]:
        """所有外部网络快照，键为网络名。"""
        ...

    def foreign_network_route_summary(self) -> Dict[str, Any]:
        """外部网络路由汇总。"""
        ...

    # ---------- ACL 与统计 ----------

    def acl_stats(self) -> Dict[str, Any]:
        """ACL 统计信息。"""
        ...

    def acl_whitelist(self) -> Dict[str, Any]:
        """当前 ACL 白名单 {"tcp_ports": [...], "udp_ports": [...]}。"""
        ...

    def metrics(self) -> List[Dict[str, Any]]:
        """节点所有指标快照。"""
        ...

    def prometheus_metrics(self) -> str:
        """以 Prometheus 文本格式导出的指标。"""
        ...

    # ---------- 凭证管理（需要 admin 节点） ----------

    def generate_credential(
        self,
        groups: List[str],
        allowed_proxy_cidrs: List[str],
        allow_relay: bool = False,
        ttl_seconds: float = 3600.0,
        credential_id: Optional[str] = None,
        reusable: bool = True,
    ) -> Dict[str, Any]:
        """生成接入凭证，返回 {"credential_id", "secret", "expiry_unix", "changed"}。"""
        ...

    def revoke_credential(self, credential_id: str) -> bool:
        """吊销凭证，成功返回 True。"""
        ...

    def upsert_credential(
        self,
        credential_id: str,
        credential_secret: str,
        groups: List[str],
        allow_relay: bool,
        allowed_proxy_cidrs: List[str],
        expiry_unix: int,
        reusable: bool,
    ) -> bool:
        """导入已存在凭证，发生变更返回 True。"""
        ...

    def credentials(self) -> List[Dict[str, Any]]:
        """当前所有凭证列表。"""
        ...

    # ---------- 其它运行时操作 ----------

    def close_peer_conn(self, peer_id: int, conn_id: str) -> None:
        """关闭与指定对端的一条连接。"""
        ...

    def update_exit_nodes(self, ips: List[str]) -> None:
        """运行时更新出口节点列表。"""
        ...

    def refresh_acl_groups(self) -> None:
        """刷新 ACL 组。"""
        ...

    # ---------- 事件订阅 ----------

    def events(self) -> List[Dict[str, Any]]:
        """非阻塞取出当前所有待处理事件并清空缓冲。"""
        ...

    def next_event(self, timeout: Optional[float] = None) -> Optional[Dict[str, Any]]:
        """阻塞等待下一个事件；timeout 秒内无事件返回 None。"""
        ...
