# 构建指南（EasyTier-PyO3）

> 本页面向用户/贡献者的源码构建。想直接安装使用，见
> [README](../README.md)；API 参考见 [python_api.md](python_api.md)。

---

## 目录

- [环境要求](#环境要求)
  - [按平台](#按平台)
  - [支持的平台 / 架构](#支持的平台--架构)
- [安装 Rust 工具链](#安装-rust-工具链)
- [克隆项目](#克隆项目)
- [创建虚拟环境并安装 maturin](#创建虚拟环境并安装-maturin)
- [构建](#构建)
- [验证安装](#验证安装)
- [CI 自动构建](#ci-自动构建)
- [构建常见问题](#构建常见问题)

---

## 环境要求

| 依赖 | 版本 | 说明 |
| --- | --- | --- |
| Rust | 稳定版 (1.85+) | 通过 [rustup](https://rustup.rs) 安装 |
| Python | 3.11+ | 64 位，建议使用虚拟环境（3.9/3.10 无 Windows ARM64 官方构建，全平台统一从 3.11 起） |
| maturin | >=1.5 | Rust/Python 桥接构建工具 |
| C/C++ 工具链 | — | Windows 需 Visual Studio Build Tools (C++ 工作负载)；Linux/macOS 需 gcc/clang |
| protoc | — | 见下方「按平台」 |

> 首次构建会联网下载 EasyTier 的数百个依赖，根据机器性能可能需要
> **10 ~ 40 分钟**，属正常现象。

### 按平台

| 平台 | 额外要求 |
| --- | --- |
| Windows | **Visual Studio Build Tools**（C++ 工作负载，编译 wintun/windivert 等 C 代码）；**LLVM**（提供 `libclang.dll`，bindgen 需要，`winget install LLVM.LLVM`）；protoc 可让 easytier-proto 自动下载，或参照 `.cargo/config.toml.example` 配置本机路径 |
| Linux | C 编译器；protoc：`sudo apt install protobuf-compiler`（Debian/Ubuntu）或 `sudo dnf install protobuf-compiler`（Fedora） |
| macOS | Xcode Command Line Tools（`xcode-select --install`）；protoc：`brew install protobuf` |

> 机器相关工具路径（如 Windows 上的 `PROTOC` / `LIBCLANG_PATH`）放在
> `.cargo/config.toml`（已被 gitignore，参考 `.cargo/config.toml.example`），
> 不影响其他平台/协作者。

### 支持的平台 / 架构

CI 会在以下平台构建并测试（见 `.github/workflows/ci.yml`）：

| OS | 架构 | Python | 说明 |
| --- | --- | --- | --- |
| Linux | x86_64 | 3.11 / 3.12 / 3.13 | manylinux 2_28 (glibc 2.28+) |
| Windows | x86_64 | 3.11 / 3.12 / 3.13 | 完整功能（含 fake-tcp） |
| Windows | ARM64 | 3.11 / 3.12 / 3.13 | **实验性**（GitHub-hosted `windows-11-arm`） |
| macOS | arm64 | 3.11 / 3.12 / 3.13 | `macos-latest` |

> **关于 Windows ARM64**：easytier 官方在 aarch64 上**不支持 WinDivert**（其发行包内
> `WinDivert64.sys` 为占位文件，文本提示 "WinDivert doesn't support aarch64"），因此
> ARM64 构建以**基础 TCP/UDP 隧道**为主。ARM64 wheel 仍处于实验阶段，如有构建问题
> 以 CI 实际情况为准。

## 安装 Rust 工具链

如果你还没有 Rust：

```bash
# Windows (PowerShell) 或 Linux/macOS (sh)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

> Windows 上请确保安装了 **Visual Studio Build Tools 2022**，并勾选
> “使用 C++ 的桌面开发” 工作负载，否则无法编译 EasyTier 依赖的 C 代码
> (wintun / windivert 等)。

## 克隆项目

```bash
git clone <你的仓库地址> EasyTier-PyO3
cd EasyTier-PyO3
```

## 创建 Python 虚拟环境并安装 maturin

```bash
# Windows
python -m venv .venv
.venv\Scripts\activate

# Linux / macOS
python3 -m venv .venv
source .venv/bin/activate

pip install -U pip maturin
```

## 构建

方式一：开发模式安装（推荐，直接安装到当前虚拟环境）：

```bash
maturin develop
```

方式二：构建 wheel 后手动安装：

```bash
maturin build --release
pip install target/wheels/easytier_pyo3-*.whl
```

方式三：直接用 pip（通过项目内的 `pyproject.toml`）：

```bash
pip install .
```

## 验证安装

```bash
python -c "from easytier_pyo3 import version; print(version())"
# 输出类似: 2.6.4
```

> 构建产物：`maturin develop` 会直接把 `easytier_pyo3.pyd` 放入虚拟环境，
> `import easytier_pyo3` 即可使用。

## CI 自动构建

CI（`.github/workflows/ci.yml` / `publish.yml`）会为每个平台构建 **3.11 / 3.12 / 3.13**
的 wheel 并上传为构建产物（`ci` 工作流同时自动安装并运行
`tests/test_easietier.py` 自测）。

- `ci` 工作流：push / PR 时构建 + 测试，产物以 `wheels-<os>` 形式归档
  （可在 Actions 页面下载）
- `publish` 工作流：打 `v*` tag 时构建并发布到 PyPI（Trusted Publishing）
- Windows：运行时 DLL（`wintun.dll` / `Packet.dll` / `WinDivert64.sys`）由
  `build.rs` 在构建时按 target 架构从 `third_party/<arch>/` 复制到
  `python/easytier_pyo3/`，随 wheel 自动分发，无需手工 staging（arm64 构建不含
  WinDivert64.sys，easytier 在 aarch64 上不支持 winfilter）

## 构建常见问题

**Q1：`maturin develop` 报错缺少 C 编译器？**
Windows 安装 Visual Studio Build Tools 并勾选 “使用 C++ 的桌面开发”。

**Q2：构建很慢 / 卡在下载？**
首次构建需下载 EasyTier 全部依赖及 `protoc`，请耐心等待；后续构建为增量，速度很快。

**Q3：`pip install .` 与 `maturin develop` 有什么区别？**
`maturin develop` 直接安装到当前虚拟环境（调试方便）；`pip install .` 走完整 wheel
构建流程（产物更正式，可用 `--release` 优化）。

**Q4：构建报错 `Could not find protoc`？**
EasyTier 的部分 proto 构建依赖需要 `protoc` 命令。本项目的 `prost-wkt-types`
依赖已通过 `vendored-protox` 特性改用纯 Rust 编译，无需 protoc；
但 `easytier-proto` 在 Linux/macOS 上仍需要系统 protoc。若遇到该错误：

- Windows：`easytier-proto` 会自动下载 protoc；也可用 `winget install protobuf` 安装。
- Linux：`sudo apt install protobuf-compiler`；macOS：`brew install protobuf`。
- 通用：安装后设置环境变量 `PROTOC` 指向 `protoc` 可执行文件路径再重新构建。

**Q5：构建报错 `Unable to find libclang`（bindgen）？**
Windows 下编译 `windivert` 等 C-FFI 依赖需要 `libclang.dll`。安装 LLVM
（`winget install LLVM.LLVM`），然后把其 `bin` 目录设置到 `LIBCLANG_PATH`
环境变量后重新构建。

> 机器相关的 `PROTOC` / `LIBCLANG_PATH` 可写在 `.cargo/config.toml`
> （该文件已被 gitignore，参考 `.cargo/config.toml.example`），
> cargo 会把它注入所有构建脚本，从 PyCharm/pip 启动也能生效。

**Q6：链接报错 `LNK1181: 无法打开输入文件"Packet.lib"`？**
easytier 的构建脚本给 `Packet.lib` 输出了相对链接路径，作为依赖库编译时该路径
解析不到（相对路径以最终链接目录为基准）。本项目已在 `third_party/x86_64/` 内置
`Packet.lib`，并由 `build.rs` 输出绝对链接路径解决，无需手动处理。

**Q7：ARM64 wheel 安装后 `import easytier_pyo3` 报 `not a valid Win32 application`？**
wheel 里的运行时 DLL 与 Python 架构不匹配。`build.rs` 会按 target 架构从
`third_party/<arch>/`（x86_64/arm64 各有官方 DLL）自动复制进包，官方产物不会
出现此问题；自行交叉构建时请确认 DLL 来源与 target 一致。