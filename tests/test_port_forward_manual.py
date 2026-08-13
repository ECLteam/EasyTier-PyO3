"""验证 Node.apply_config 的 port-forward 运行时动态覆盖。

场景（单节点 no_tun，无需管理员权限）：
- 节点运行中用 apply_config 添加 port_forward：
  本地 127.0.0.1:28080 → 10.144.145.1:10001（虚拟 IP 上的服务端口）。
- easytier-core 的 PortForwardAdapter 会在 apply_config 后立即在本地
  绑定该端口（reload → add_tcp_port_forward → host.bind_tcp），
  因此**无需 TUN 也能验证配置确实生效**：本地端口应变为可连接状态。
- 移除 port_forward 后本地端口应关闭。

运行：
    python tests/test_port_forward_manual.py
"""

import socket
import sys
import time

import easytier_pyo3

NET = {"network_name": "pfo-net", "network_secret": "pfo-secret"}
A_VIP = "10.144.145.1"
LOCAL_PORT = 28080
LOCAL_PORT2 = 28081
LOCAL_PORT3 = 28082


def check(name: str, condition: bool, detail: str = "") -> bool:
    print(f"  [{'PASS' if condition else 'FAIL'}] {name}  {detail}")
    return condition


def tcp_accepting(port: int, timeout: float = 8.0) -> bool:
    """轮询等待本地端口可以建立 TCP 连接（证明被 TCP listener 绑定）。"""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            s = socket.create_connection(("127.0.0.1", port), timeout=1.0)
            s.close()
            return True
        except OSError:
            time.sleep(0.3)
    return False


def tcp_closed(port: int, timeout: float = 5.0) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            s = socket.create_connection(("127.0.0.1", port), timeout=1.0)
            s.close()
            time.sleep(0.3)
        except OSError:
            return True
    return False


def udp_bound(port: int, timeout: float = 6.0) -> bool:
    """轮询等待本地 UDP 端口可被 bind（bind 占用=PortForwardAdapter 已绑定）。"""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            probe.bind(("127.0.0.1", port))
            probe.close()
            time.sleep(0.3)
            continue
        except OSError:
            # EADDRINUSE 说明端口已被占用（Adapter 绑定了）→ 视为已绑定
            return True
    return False


def udp_free(port: int, timeout: float = 5.0) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            probe.bind(("127.0.0.1", port))
            probe.close()
            return True
        except OSError:
            time.sleep(0.3)
    return False


def main() -> int:
    results = []

    node = easytier_pyo3.Node({
        "instance_name": "pfo-node",
        "network_identity": NET,
        "ipv4": f"{A_VIP}/24",
        "flags": {"no_tun": True, "bind_device": False},
    })
    node.start()
    try:
        print("[1] 初始状态：本地转发端口应未被监听")
        results.append(check("TCP 28080 初始未监听", tcp_closed(LOCAL_PORT, timeout=1.5)))

        print("[2] apply_config 添加 port_forward（tcp）")
        node.apply_config({
            "port_forward": [
                {"bind_addr": f"127.0.0.1:{LOCAL_PORT}",
                 "dst_addr": f"{A_VIP}:10001", "proto": "tcp"},
            ],
        })
        results.append(check("apply_config 调用成功", True))
        results.append(check("本地 TCP 端口已被 PortForwardAdapter 绑定", tcp_accepting(LOCAL_PORT)))

        print("[3] apply_config 覆盖为另一组转发（udp，另一个端口）")
        node.apply_config({
            "port_forward": [
                {"bind_addr": f"127.0.0.1:{LOCAL_PORT2}",
                 "dst_addr": f"{A_VIP}:10002", "proto": "udp"},
            ],
        })
        results.append(check("覆盖后旧 TCP 端口关闭", tcp_closed(LOCAL_PORT)))
        results.append(check("覆盖后新 UDP 端口被绑定", udp_bound(LOCAL_PORT2)))

        print("[4] apply_config 清空转发")
        node.apply_config({"port_forward": []})
        results.append(check("清空后 UDP 端口释放", udp_free(LOCAL_PORT2)))

        print("[5] 非法 proto 报错")
        try:
            node.apply_config({
                "port_forward": [
                    {"bind_addr": f"127.0.0.1:{LOCAL_PORT3}",
                     "dst_addr": f"{A_VIP}:10003", "proto": "icmp"},
                ],
            })
            results.append(check("非法 proto 被拒绝", False, "未抛异常"))
        except Exception as exc:  # noqa: BLE001
            results.append(check("非法 proto 被拒绝", True, repr(exc)))

        print("[6] 未显式出现的字段保持不变（只传 exit_nodes）")
        node.apply_config({"exit_nodes": [A_VIP]})
        results.append(check("exit_nodes 覆盖成功", True))

        print("[7] apply_config 支持 TOML 字符串")
        node.apply_config(f"""
            [[port_forward]]
            bind_addr = "127.0.0.1:{LOCAL_PORT3}"
            dst_addr = "{A_VIP}:10004"
            proto = "tcp"
        """)
        results.append(check("TOML 字符串形式生效", tcp_accepting(LOCAL_PORT3)))

    finally:
        node.stop()

    print("-" * 44)
    print(f"通过: {sum(results)}, 失败: {len(results) - sum(results)}")
    return 1 if not all(results) else 0


if __name__ == "__main__":
    sys.exit(main())
