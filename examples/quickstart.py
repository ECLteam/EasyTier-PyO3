"""EasyTier-PyO3 快速开始示例。

运行前请先完成构建：
    pip install maturin
    maturin develop

示例在单机上创建两个节点，验证对端发现与事件订阅。
"""

import threading
import time

from easytier_py import Node, version

print(f"EasyTier 内核版本: {version()}")

NET = {"network_name": "demo", "network_secret": "demo-secret"}
# no_tun：不创建 TUN 设备（示例无需管理员权限）。
# bind_device: False：no_tun 下禁用把客户端 socket 绑定到虚拟 IP，
# 否则 Windows 上连接会报 WSAEADDRNOTAVAIL(10049)。
FLAGS = {"no_tun": True, "bind_device": False}


def main() -> None:
    # 节点 A：监听 tcp://127.0.0.1:11010
    node_a = Node({
        "instance_name": "a",
        "network_identity": NET,
        "flags": FLAGS,
        "listeners": ["tcp://127.0.0.1:11010"],
    })
    node_a.start()
    print(f"节点 A 已启动: {node_a.state()}, peer_id={node_a.peer_id()}")

    # 节点 B：手动连接节点 A
    node_b = Node({
        "instance_name": "b",
        "network_identity": NET,
        "flags": FLAGS,
        "peer": [{"uri": "tcp://127.0.0.1:11010"}],
    })
    node_b.start()
    print(f"节点 B 已启动: {node_b.state()}")

    # 监听节点 A 的事件（后台线程阻塞等待，不会卡住主线程）
    def printer() -> None:
        while True:
            event = node_a.next_event(timeout=2.0)
            if event is not None:
                print(f"  [事件] {event}")

    threading.Thread(target=printer, daemon=True).start()

    # 等待对端互相发现
    deadline = time.time() + 10
    while time.time() < deadline:
        if node_a.peers() and node_b.peers():
            break
        time.sleep(0.5)

    print(f"节点 A 发现的对端: {node_a.peers()}")
    print(f"节点 B 发现的对端: {node_b.peers()}")
    print(f"节点 A 路由:\n{node_a.dump_route()}")

    node_a.stop()
    node_b.stop()
    print("节点已停止。")


if __name__ == "__main__":
    main()
