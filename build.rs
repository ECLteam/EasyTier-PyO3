//! 构建脚本：修复 easytier 作为依赖库时的链接问题。
//!
//! 上游 easytier 的构建脚本（build/main.rs）为 Windows 下的 `Packet.lib`
//! 输出的是**相对**链接搜索路径 `easytier/third_party/<arch>/`，它按链接时的
//! CWD（即本项目根目录）解析，因此当我们把 easytier 作为依赖编译最终的
//! `easytier_py.dll` 时找不到该库（LNK1181: Packet.lib）。
//!
//! 解决办法：把各架构的 `Packet.lib` 放到本项目 `third_party/<arch>/` 下，
//! 这里输出绝对路径的链接搜索目录。上游那个无效的相对路径会被链接器忽略。
//!
//! 说明：easytier 在 x86/x86_64/aarch64 上均使用 pnet/winpcap
//! （`#[link(name="Packet")]`，需对应架构的 Packet.lib），因此按 target_arch
//! 选择链接目录；其它平台不链接 Packet。

fn main() {
    // Windows 的 x86/x86_64/aarch64 需要 pnet 的 Packet.lib；其它平台无此问题。
    #[cfg(all(target_os = "windows", any(target_arch = "x86_64", target_arch = "x86")))]
    let arch_dir = "x86_64";
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    let arch_dir = "arm64";

    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let link_dir = manifest_dir.join("third_party").join(arch_dir);
        println!("cargo:rustc-link-search=native={}", link_dir.display());
        println!("cargo:rerun-if-changed=third_party/{arch_dir}/Packet.lib");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
