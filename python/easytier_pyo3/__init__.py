"""EasyTier-PyO3 Python 包。

真正实现在编译产物 `easytier_pyo3.pyd` 中；本文件用于：
- 让 maturin 把编译模块打包为 `easytier_pyo3` 包
- 顺带携带运行时 DLL（wintun / Packet / WinDivert64）到安装目录
"""

from . import easytier_pyo3 as _impl
from .easytier_pyo3 import *  # noqa: F401,F403
from .easytier_pyo3 import __doc__  # noqa: F401

if hasattr(_impl, "__all__"):
    __all__ = _impl.__all__
