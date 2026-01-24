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
    Undo,
    Redo,
    /// 切换地图尺寸（会重建格子实体）。
    SetMapSize { width: u32, height: u32 },
    /// 从文件导入地图（文件选择器）。
    ImportMap,
    /// 导出地图到文件（文件选择器）。
    ExportMap,
    /// 切换网格显示。
    ToggleGrid,
    /// 切换 hover 高亮显示。
    ToggleHover,
    /// 切换 HUD 坐标显示。
    ToggleCursor,
	/// Shift Map 模式 Blank <-> Wrap。
	ToggleShiftMode,
}

#[derive(Component)]
pub struct ActionButton(pub ActionKind);

// --- 顶部菜单栏（分类 + 悬浮下拉） ---

#[derive(Resource, Default)]
pub struct MenuState {
    pub open: Option<MenuId>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    File,
    Edit,
    View,
	Map,
    Layer,
    Help,
}

#[derive(Component, Clone, Copy)]
pub struct MenuButton(pub MenuId);

#[derive(Component, Clone, Copy)]
pub struct MenuDropdown;

#[derive(Component)]
pub struct MenuBackdrop;

#[derive(Component)]
pub struct MenuItem;

#[derive(Resource)]
pub struct UiState {
    pub built_for_tileset_path: String,
    pub palette_tile_px: f32,
    pub built_palette_tile_px: f32,
    pub built_palette_filter: String,
	pub built_palette_filtered_count: u32,
    pub tileset_menu_open: bool,
    pub built_tileset_menu_count: usize,
    pub built_tileset_menu_active_id: String,
    pub built_tileset_menu_category: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            built_for_tileset_path: String::new(),
            palette_tile_px: 40.0,
            built_palette_tile_px: -1.0,
            built_palette_filter: String::new(),
			built_palette_filtered_count: 0,
            tileset_menu_open: false,
            built_tileset_menu_count: 0,
            built_tileset_menu_active_id: String::new(),
            built_tileset_menu_category: String::new(),
        }
    }
}

impl UiState {
	pub fn palette_tile_px(&self) -> f32 {
		self.palette_tile_px.clamp(24.0, 96.0)
	}
}

// --- Palette 缩略图缩放 ---

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaletteZoomLevel {
    Small,
    Medium,
    Large,
}

#[derive(Component, Clone, Copy)]
pub struct PaletteZoomButton(pub PaletteZoomLevel);

// --- Palette 搜索 ---

#[derive(Component)]
pub struct PaletteSearchField;

#[derive(Component)]
pub struct PaletteSearchText;

#[derive(Component)]
pub struct PaletteSearchClearButton;

#[derive(Resource)]
pub struct PaletteSearchInput {
    pub buf: String,
    pub focused: bool,
}

impl Default for PaletteSearchInput {
    fn default() -> Self {
        Self {
            buf: String::new(),
            focused: false,
        }
    }
}

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

// --- 图层重命名输入 ---

#[derive(Component)]
pub struct LayerNameField;

#[derive(Component)]
pub struct LayerNameText;

#[derive(Component)]
pub struct LayerNameApplyButton;

#[derive(Resource)]
pub struct LayerNameInput {
    pub buf: String,
    pub focused: bool,
    pub apply_requested: bool,
}

impl Default for LayerNameInput {
    fn default() -> Self {
        Self {
            buf: String::new(),
            focused: false,
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
