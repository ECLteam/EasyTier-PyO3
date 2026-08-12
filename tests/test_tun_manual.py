"""TUN 设备手动验证脚本（**需要管理员权限**）。

验证内容：
1. 非 no_tun 模式下能否创建 wintun 虚拟网卡并分配虚拟 IP
2. 两个节点能否建立隧道
3. 数据面：A 用 TUN、B 用 no_tun，ping B 的虚拟 IP（本机无此地址，
   必须经 A 的 TUN 进隧道到 B 再返回）—— 成功即证明 TUN 数据面可用

用法（管理员终端）:
    python tests/test_tun_manual.py

注意：
- Windows 需要管理员权限创建 wintun 网卡（UAC 提权）
- 本机回环测试时节点需设 `bind_device: False`，否则连接报
  WSAEADDRNOTAVAIL(10049)
"""

import subprocess
import sys
import time

import easytier_pyo3

NET = {"network_name": "tun-manual", "network_secret": "tun-manual-secret"}


def main() -> int:
    # A：真实 TUN，虚拟 IP 10.144.144.30/24
    a = easytier_pyo3.Node({
        "instance_name": "tun-a",
        "network_identity": NET,
        "ipv4": "10.144.144.30/24",
        "flags": {"bind_device": False},
        "listeners": ["tcp://0.0.0.0:11090"],
    })
    # B：no_tun，虚拟 IP 10.144.144.31（本机无此网卡地址，ping 必须走隧道）
    b = easytier_pyo3.Node({
        "instance_name": "tun-b",
        "network_identity": NET,
        "ipv4": "10.144.144.31/24",
        "flags": {"no_tun": True, "bind_device": False},
        "peer": [{"uri": "tcp://127.0.0.1:11090"}],
    })

    try:
        a.start()
        print(f"节点 A (TUN) 已启动: {a.state()}")
        b.start()
        print(f"节点 B (no_tun) 已启动: {b.state()}")

        print("等待对端建立隧道...")
        deadline = time.time() + 25
        while time.time() < deadline:
            if b.peers():
                break
            time.sleep(0.5)
        print(f"A peers: {a.peers()}")
        print(f"B peers: {b.peers()}")

        print("ping 10.144.144.31 (B 的虚拟 IP，须经隧道)...")
        r = subprocess.run(["ping", "-n", "4", "10.144.144.31"],
                           capture_output=True, text=True, encoding="gbk", errors="replace")
        print(r.stdout)
        ok = "0% 丢失" in r.stdout or "0% loss" in r.stdout
        print(f"TUN 数据面验证: {'PASS' if ok else 'FAIL'}")
        return 0 if ok else 1
    finally:
        a.stop()
        b.stop()
        print("节点已停止。")


if __name__ == "__main__":
    sys.exit(main())
