use bevy::prelude::*;

use super::TilesetId;

#[derive(Resource, Clone)]
pub struct UiFont(pub Handle<Font>);

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
    /// 切换网格显示。
    ToggleGrid,
}

#[derive(Component)]
pub struct ActionButton(pub ActionKind);

#[derive(Resource)]
pub struct UiState {
    pub built_for_tileset_path: String,
    pub palette_page: u32,
    pub palette_page_size: u32,
    pub built_palette_page: u32,
    pub tileset_menu_open: bool,
    pub built_tileset_menu_count: usize,
    pub built_tileset_menu_active_id: String,
    pub built_tileset_menu_category: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            built_for_tileset_path: String::new(),
            palette_page: 0,
            palette_page_size: 256,
            built_palette_page: u32::MAX,
            tileset_menu_open: false,
            built_tileset_menu_count: 0,
            built_tileset_menu_active_id: String::new(),
            built_tileset_menu_category: String::new(),
        }
    }
}

impl UiState {
	pub fn palette_page_size(&self) -> u32 {
		self.palette_page_size.max(1)
	}
}

// --- Palette 分页控件 ---

#[derive(Component)]
pub struct PalettePrevPageButton;

#[derive(Component)]
pub struct PaletteNextPageButton;

#[derive(Component)]
pub struct PalettePageLabel;

// --- 笔刷尺寸控件 ---

#[derive(Component, Clone, Copy)]
pub struct BrushSizeButton(pub u32);

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

// --- 右上角：图层切换控件 ---

#[derive(Component)]
pub struct LayerPrevButton;

#[derive(Component)]
pub struct LayerNextButton;

#[derive(Component)]
pub struct LayerActiveLabel;

#[derive(Component)]
pub struct LayerActiveVisToggleButton;

#[derive(Component)]
pub struct LayerActiveVisLabel;

#[derive(Component)]
pub struct LayerActiveLockToggleButton;

#[derive(Component)]
pub struct LayerActiveLockLabel;
