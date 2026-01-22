use bevy::prelude::*;

use crate::editor::paths::workspace_assets_dir;

use super::DEFAULT_SAVE_PATH;

/// 编辑器配置。
///
/// - `save_path`：保存地图的绝对路径（默认 workspace/assets/maps/map.ron）
#[derive(Resource)]
pub struct EditorConfig {
    pub tile_size: UVec2,
    pub map_size: UVec2,
    pub save_path: String,
    pub tileset_import_dir: String,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tile_size: UVec2::new(32, 32),
            map_size: UVec2::new(40, 25),
            save_path: workspace_assets_dir()
                .join(DEFAULT_SAVE_PATH)
                .to_string_lossy()
                .to_string(),
            tileset_import_dir: "tilesets".to_string(),
        }
    }
}
