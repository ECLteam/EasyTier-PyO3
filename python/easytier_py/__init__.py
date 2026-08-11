"""EasyTier-PyO3 Python 包。

真正实现在编译产物 `easytier_py.pyd` 中；本文件用于：
- 让 maturin 把编译模块打包为 `easytier_py` 包
- 顺带携带运行时 DLL（wintun / Packet / WinDivert64）到安装目录
"""

from . import easytier_py as _impl
from .easytier_py import *  # noqa: F401,F403
from .easytier_py import __doc__  # noqa: F401

if hasattr(_impl, "__all__"):
    __all__ = _impl.__all__
