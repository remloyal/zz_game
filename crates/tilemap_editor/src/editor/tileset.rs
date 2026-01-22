//! Tileset（精灵图）导入与加载。
//!
//! 职责：
//! - 通过文件对话框选择 tileset 图片，并复制到 workspace `assets/tilesets/`
//! - 触发 Bevy AssetServer 加载图片
//! - 图片加载完成后计算 columns/rows，并写入 `TilesetRuntime`
//!
//! 该模块采用“门面（facade）+ 子模块”结构，具体实现位于 [crates/tilemap_editor/src/editor/tileset/](crates/tilemap_editor/src/editor/tileset/) 下。

mod library;
mod loading;
mod map_setup;
mod open;
mod rect;
mod spawn;

pub use library::{load_tileset_library_startup, merge_tilesets_from_map, save_tileset_library};
pub use loading::progress_spritesheet_loading;
pub use map_setup::setup_map;
pub use open::{open_spritesheet_shortcut, open_tileset_impl};
pub use rect::rect_for_tile_index;
pub use spawn::spawn_map_entities_with_layers;
