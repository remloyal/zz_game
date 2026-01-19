//! 资源（Resource）与组件（Component）的集中定义。
//!
//! 说明：为了便于工程化维护，这里把跨模块共享的数据类型统一放在一起。

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::paths::workspace_assets_dir;

pub const DEFAULT_SPRITESHEET: &str = "tiles.png";
pub const DEFAULT_SAVE_PATH: &str = "maps/map.ron";
pub const DEFAULT_UI_FONT_PATH: &str = "chinese.ttf";

/// 标记“世界相机”（用于世界坐标拾取/绘制）。
///
/// 注意：UI 可能会创建/使用自己的相机。若鼠标拾取系统用 `Query<(&Camera, &GlobalTransform)>`
/// 并 `single()`，当场景存在多相机时会直接失败，从而导致“右侧无法绘制”。
#[derive(Component)]
pub struct WorldCamera;

#[derive(Resource, Clone)]
pub struct UiFont(pub Handle<Font>);

/// 稳定 tileset id。
///
/// 约定：使用导入图片内容的 hash（或至少是文件名+hash）生成，保证跨机器/拷贝时一致。
pub type TilesetId = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileRef {
    pub tileset_id: TilesetId,
    pub index: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TilesetEntry {
    pub id: TilesetId,
    pub name: String,
    pub category: String,
    /// 相对 `AssetPlugin.file_path` 的路径，例如：`tilesets/foo.png`
    pub asset_path: String,
}

#[derive(Resource, Serialize, Deserialize, Clone, Default)]
pub struct TilesetLibrary {
    pub entries: Vec<TilesetEntry>,
    pub active_id: Option<TilesetId>,
    pub active_category: String,
}

#[derive(Clone)]
pub struct TilesetRuntimeEntry {
    pub texture: Handle<Image>,
    pub columns: u32,
    pub rows: u32,
}

#[derive(Resource, Default)]
pub struct TilesetRuntime {
    pub by_id: HashMap<TilesetId, TilesetRuntimeEntry>,
}

#[derive(Clone)]
pub struct PendingTileset {
    pub id: TilesetId,
    pub texture: Handle<Image>,
}

#[derive(Resource, Default)]
pub struct TilesetLoading {
    pub pending: Vec<PendingTileset>,
}

#[derive(Resource)]
pub struct TileEntities {
    pub width: u32,
    pub height: u32,
    pub entities: Vec<Entity>,
}

#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct TileMapData {
    pub width: u32,
    pub height: u32,
	pub tiles: Vec<Option<TileRef>>,
}

impl TileMapData {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
			tiles: vec![None; (width * height) as usize],
        }
    }

    pub fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }
}

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

#[derive(Resource)]
pub struct EditorState {
	pub selected_tile: u32,
}

impl Default for EditorState {
    fn default() -> Self {
		Self { selected_tile: 0 }
    }
}

#[derive(Component)]
pub struct HudText;

#[derive(Component)]
pub struct UiRoot;

#[derive(Component)]
pub struct PaletteRoot;

/// 标记左侧可滚动区域（用于鼠标滚轮滚动）。
#[derive(Component)]
pub struct PaletteScroll;

#[derive(Component)]
pub struct PaletteTileButton {
    pub index: u32,
}

#[derive(Component)]
pub struct CanvasRoot;

#[derive(Component)]
pub struct TilesetBar;

#[derive(Component)]
pub struct TilesetActiveLabel;

#[derive(Component)]
pub struct TilesetCategoryLabel;

#[derive(Component)]
pub struct TilesetToggleButton;

#[derive(Component)]
pub struct TilesetCategoryCycleButton;

#[derive(Component)]
pub struct TilesetMenuRoot;

#[derive(Component, Clone)]
pub struct TilesetSelectItem {
    pub id: TilesetId,
}

#[derive(Component, Clone, Copy)]
pub enum ActionKind {
    OpenTileset,
    SaveMap,
    LoadMap,
    NewMap,
    /// 切换地图尺寸（会重建格子实体）。
    SetMapSize { width: u32, height: u32 },
    /// 从文件导入地图（文件选择器）。
    ImportMap,
    /// 导出地图到文件（文件选择器）。
    ExportMap,
}

#[derive(Component)]
pub struct ActionButton(pub ActionKind);

#[derive(Resource, Default)]
pub struct UiState {
    pub built_for_tileset_path: String,
    pub tileset_menu_open: bool,
    pub built_tileset_menu_count: usize,
    pub built_tileset_menu_active_id: String,
    pub built_tileset_menu_category: String,
}

#[derive(Component)]
pub struct MapSizeWidthField;

#[derive(Component)]
pub struct MapSizeHeightField;

#[derive(Component)]
pub struct MapSizeApplyButton;

#[derive(Component)]
pub struct MapSizeWidthText;

#[derive(Component)]
pub struct MapSizeHeightText;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum MapSizeFocus {
    #[default]
    None,
    Width,
    Height,
}

#[derive(Resource)]
pub struct MapSizeInput {
    pub width_buf: String,
    pub height_buf: String,
    pub focus: MapSizeFocus,
    pub apply_requested: bool,
}

impl Default for MapSizeInput {
    fn default() -> Self {
        Self {
            width_buf: "40".to_string(),
            height_buf: "25".to_string(),
            focus: MapSizeFocus::None,
            apply_requested: false,
        }
    }
}

/// 画布平移（拖拽）状态。
#[derive(Resource, Default)]
pub struct PanState {
    pub active: bool,
    pub last_world: Option<Vec2>,
}
