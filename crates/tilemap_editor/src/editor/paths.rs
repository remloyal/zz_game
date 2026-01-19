//! 与 workspace/assets 路径相关的工具函数。

use std::path::PathBuf;

/// workspace 的 `assets/` 目录绝对路径。
///
/// `CARGO_MANIFEST_DIR` 指向 `crates/tilemap_editor`，因此向上两级即可到 workspace 根。
pub fn workspace_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
}

/// 生成 Bevy 资产路径（建议使用 `/` 分隔符）。
pub fn path_join_asset(dir: &str, file: &str) -> String {
    let dir = dir.trim_matches(['/', '\\']);
    let file = file.trim_matches(['/', '\\']);
    if dir.is_empty() {
        file.to_string()
    } else {
        format!("{dir}/{file}")
    }
}
