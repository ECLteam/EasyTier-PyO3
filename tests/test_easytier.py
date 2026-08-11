"""EasyTier-PyO3 安装自检 / 功能测试。

在已安装 easytier_py 的虚拟环境中运行:

    python tests/test_easytier.py

覆盖内容:
    1. 模块导入与 version()
    2. Node 创建 / 启动 / 状态查询
    3. 手动连接管理 (add/remove/clear/connectors)
    4. 单机双节点对端发现 (no_tun, 无需管理员权限)
    5. 事件订阅 (events / next_event)
    6. 快照与统计 (peers / node_info / dump_route / metrics / acl_whitelist)

无需 pytest，直接运行即可；任一项失败时退出码为 1。
"""

import sys
import time
from typing import Optional

import easytier_py

# 两个节点共用同一网络标识，才能互相发现。
NET = {"network_name": "pytest-net", "network_secret": "pytest-secret"}

_passed = 0
_failed = 0


def check(name: str, condition: bool, detail: str = "") -> None:
    """记录一条测试结果并打印。"""
    global _passed, _failed
    if condition:
        _passed += 1
        print(f"  [PASS] {name}")
    else:
        _failed += 1
        print(f"  [FAIL] {name}  {detail}")


def make_node(name: str, listener: str, peer: Optional[str] = None) -> "easytier_py.Node":
    """按统一配置创建测试节点。"""
    config = {
        "instance_name": name,
        "network_identity": NET,
        # no_tun：不创建 TUN 设备（测试无需管理员权限）。
        # bind_device: False：no_tun 下禁用把客户端 socket 绑定到虚拟 IP，
        # 否则 Windows 上连接会报 WSAEADDRNOTAVAIL(10049)。
        "flags": {"no_tun": True, "bind_device": False},
    }
    if listener:
        config["listeners"] = [listener]
    if peer:
        config["peer"] = [{"uri": peer}]
    return easytier_py.Node(config)


def test_import_and_version() -> None:
    print("[1] 模块导入与 version()")
    v = easytier_py.version()
    check("version() 返回非空字符串", isinstance(v, str) and bool(v), f"got {v!r}")
    check("模块暴露 Node", hasattr(easytier_py, "Node"))
    check("模块暴露 version", hasattr(easytier_py, "version"))


def test_create_and_start() -> "easytier_py.Node":
    print("[2] Node 创建 / 启动 / 状态")
    node = make_node("test-a", "tcp://127.0.0.1:11010")
    check("node 是 Node 实例", isinstance(node, easytier_py.Node))
    check("初始状态为 Created", node.state() == "Created", node.state())

    node.start()
    check("启动后 is_ready() 为 True", node.is_ready(), node.state())
    check("state() 为 Running", node.state() == "Running", node.state())
    check("instance_id() 非空", bool(node.instance_id()), node.instance_id())
    check("instance_name() 正确", node.instance_name() == "test-a", node.instance_name())
    check("peer_id() 为 int", isinstance(node.peer_id(), int), str(node.peer_id()))
    check(
        "监听地址包含 11010",
        any("11010" in url for url in node.running_listeners()),
        str(node.running_listeners()),
    )
    return node


def test_connectors(node: "easytier_py.Node") -> None:
    print("[3] 手动连接管理")
    node.add_connector("tcp://127.0.0.1:11020")
    conns = node.connectors()
    check("connectors() 包含新地址", any("11020" in c["url"] for c in conns), str(conns))
    check("connectors() 含 status 字段", all("status" in c for c in conns), str(conns))

    check("remove_connector() 返回 True", node.remove_connector("tcp://127.0.0.1:11020") is True)
    check("再次 remove 返回 False", node.remove_connector("tcp://127.0.0.1:11020") is False)

    node.add_connector("tcp://127.0.0.1:11020")
    node.clear_connectors()
    check("clear_connectors() 后为空", node.connectors() == [], str(node.connectors()))


def test_two_nodes() -> None:
    print("[4] 单机双节点对端发现")
    a = make_node("peer-a", "tcp://127.0.0.1:11030")
    b = make_node("peer-b", "", peer="tcp://127.0.0.1:11030")
    a.start()
    b.start()

    deadline = time.time() + 15
    discovered = False
    while time.time() < deadline:
        if b.peers():
            discovered = True
            break
        time.sleep(0.5)

    check("B 发现对端 A", discovered, f"peers={b.peers()}")
    if discovered:
        first = b.peers()[0]
        check("peers 条目含 peer_id", "peer_id" in first, str(first))
    check("dump_route() 非空", bool(b.dump_route()))
    check("node_info() 返回 dict", isinstance(b.node_info(), dict))
    check("routes() 返回列表", isinstance(b.routes(), list))

    a.stop()
    b.stop()
    check("stop() 幂等", a.stop() is None)


def test_events(node: "easytier_py.Node") -> None:
    print("[5] 事件订阅")
    evs = node.events()
    check("events() 返回列表", isinstance(evs, list))
    check("events() 条目为 dict", all(isinstance(e, dict) for e in evs))

    got = node.next_event(timeout=0.5)
    check("next_event() 返回 dict 或 None", got is None or isinstance(got, dict), repr(got))


def test_stats(node: "easytier_py.Node") -> None:
    print("[6] 快照与统计")
    check("metrics() 返回列表", isinstance(node.metrics(), list))
    check("prometheus_metrics() 返回 str", isinstance(node.prometheus_metrics(), str))
    check("acl_whitelist() 返回 dict", isinstance(node.acl_whitelist(), dict))
    check("is_ready() 为 bool", isinstance(node.is_ready(), bool))


def main() -> None:
    node = None
    try:
        test_import_and_version()
        node = test_create_and_start()
        test_connectors(node)
        test_two_nodes()
        test_events(node)
        test_stats(node)
    except Exception as exc:  # noqa: BLE001 - 测试脚本要兜底打印
        global _failed
        _failed += 1
        print(f"  [FAIL] 未捕获异常: {exc!r}")
    finally:
        if node is not None:
            try:
                node.stop()
            except Exception:  # noqa: BLE001
                pass

    print("-" * 44)
    print(f"通过: {_passed}, 失败: {_failed}")
    sys.exit(1 if _failed else 0)


if __name__ == "__main__":
    main()
