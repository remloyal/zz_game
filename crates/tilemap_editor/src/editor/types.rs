//! 资源（Resource）与组件（Component）的集中定义。
//!
//! 说明：为了便于工程化维护，这里把跨模块共享的数据类型统一放在一起。

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::paths::workspace_assets_dir;

mod tilemap;
pub use tilemap::{LayerState, TileEntities};

pub const DEFAULT_SPRITESHEET: &str = "tiles.png";
pub const DEFAULT_SAVE_PATH: &str = "maps/map.ron";
pub const DEFAULT_UI_FONT_PATH: &str = "chinese.ttf";

pub use tilemap_core::{TileMapData, TileRef, TilesetId, DEFAULT_LAYER_COUNT};

/// 标记“世界相机”（用于世界坐标拾取/绘制）。
///
/// 注意：UI 可能会创建/使用自己的相机。若鼠标拾取系统用 `Query<(&Camera, &GlobalTransform)>`
/// 并 `single()`，当场景存在多相机时会直接失败，从而导致“右侧无法绘制”。
#[derive(Component)]
pub struct WorldCamera;

#[derive(Resource, Clone)]
pub struct UiFont(pub Handle<Font>);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Pencil,
    Eraser,
    Rect,
    Fill,
    Select,
    Paste,
	Eyedropper,
}

impl Default for ToolKind {
    fn default() -> Self {
        Self::Pencil
    }
}

#[derive(Resource)]
pub struct ToolState {
    pub tool: ToolKind,
    /// 通过 Ctrl+V / 右键菜单进入粘贴时，记住进入前的工具，便于粘贴落地后自动恢复。
    pub return_after_paste: Option<ToolKind>,
}

impl Default for ToolState {
    fn default() -> Self {
        Self {
            tool: ToolKind::default(),
            return_after_paste: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftMapMode {
    Blank,
    Wrap,
}

impl Default for ShiftMapMode {
    fn default() -> Self {
        Self::Blank
    }
}

#[derive(Resource, Default)]
pub struct ShiftMapSettings {
    pub mode: ShiftMapMode,
}

#[derive(Resource, Default, Clone)]
pub struct Clipboard {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Option<TileRef>>,
}

#[derive(Resource, Default, Clone, Copy)]
pub struct PasteState {
    /// 0,1,2,3 => 0/90/180/270 度顺时针。
    pub rot: u8,
    pub flip_x: bool,
    pub flip_y: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextMenuAction {
    Undo,
    Redo,
    EnterPaste,
    SelectionCopy,
    SelectionCut,
    SelectionDelete,
    SelectionSelectAll,
    SelectionDeselect,

    PasteRotateCcw,
    PasteRotateCw,
    PasteFlipX,
    PasteFlipY,
    PasteReset,
    ExitPaste,
}

#[derive(Resource, Default)]
pub struct ContextMenuState {
    pub open: bool,
    /// UI 屏幕坐标（原点左上）。
    pub screen_pos: Vec2,
	/// 打开菜单时鼠标所在的地图格子坐标（若不在地图上则为 None）。
	pub map_pos: Option<UVec2>,
    /// 用于“点击菜单项/点击空白关闭”时，避免同一帧触发画布左键操作。
    pub consume_left_click: bool,
    /// 用于 UI 动态重建菜单：状态签名（工具/选区/剪贴板等）变化时重建。
    pub signature: u64,
}

#[derive(Component)]
pub struct ContextMenuRoot;

#[derive(Component)]
pub struct ContextMenuBackdrop;

#[derive(Component, Clone, Copy)]
pub struct ContextMenuItem(pub ContextMenuAction);

#[derive(Component)]
pub struct ContextMenuDisabled;

#[derive(Resource, Default)]
pub struct ContextMenuCommand {
    pub action: Option<ContextMenuAction>,
}

#[derive(Resource, Default)]
pub struct PastePreview {
    pub entities: Vec<Entity>,
    pub dims: (u32, u32),
}

#[derive(Component)]
pub struct PastePreviewTile;

#[derive(Component)]
pub struct SelectionMovePreviewTile;

#[derive(Clone, Copy, Debug, Default)]
pub struct SelectionRect {
    pub min: UVec2,
    pub max: UVec2,
}

impl SelectionRect {
    pub fn width(&self) -> u32 {
        self.max.x.saturating_sub(self.min.x) + 1
    }

    pub fn height(&self) -> u32 {
        self.max.y.saturating_sub(self.min.y) + 1
    }
}

#[derive(Resource, Default)]
pub struct SelectionState {
    pub dragging: bool,
    pub start: UVec2,
    pub current: UVec2,
    pub rect: Option<SelectionRect>,
	/// 是否正在“拖拽移动选区内容”（与 dragging=框选不同）。
	pub moving: bool,
}

#[derive(Component, Clone, Copy)]
pub struct ToolButton(pub ToolKind);

#[derive(Component)]
pub struct ShiftModeButton;

#[derive(Component)]
pub struct ShiftModeLabel;

#[derive(Clone, Debug)]
pub struct CellChange {
    pub idx: usize,
    pub before: Option<TileRef>,
    pub after: Option<TileRef>,
}

#[derive(Clone, Debug, Default)]
pub struct EditCommand {
    pub changes: Vec<CellChange>,
}

#[derive(Resource, Default)]
pub struct UndoStack {
    pub undo: Vec<EditCommand>,
    pub redo: Vec<EditCommand>,
    pub max_len: usize,
}

impl UndoStack {
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn push(&mut self, cmd: EditCommand) {
        if cmd.changes.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push(cmd);
        let max_len = if self.max_len == 0 { 200 } else { self.max_len };
        if self.undo.len() > max_len {
            let drain = self.undo.len() - max_len;
            self.undo.drain(0..drain);
        }
    }
}
