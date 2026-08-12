//! 构建脚本：修复 easytier 作为依赖库时的链接问题。
//!
//! 上游 easytier 的构建脚本（build/main.rs）为 Windows 下的 `Packet.lib`
//! 输出的是**相对**链接搜索路径 `easytier/third_party/x86_64/`，它按链接时的
//! CWD（即本项目根目录）解析，因此当我们把 easytier 作为依赖编译最终的
//! `easytier_py.dll` 时找不到该库（LNK1181: Packet.lib）。
//!
//! 解决办法：把 `Packet.lib` 拷贝到本项目的 `third_party/x86_64/` 下，
//! 这里输出绝对路径的链接搜索目录。上游那个无效的相对路径会被链接器忽略。

fn main() {
    // 仅 Windows 需要链接 pnet 的 Packet.lib；其它平台无此问题。
    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let link_dir = manifest_dir.join("third_party").join("x86_64");
        println!("cargo:rustc-link-search=native={}", link_dir.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=third_party/x86_64/Packet.lib");
}
