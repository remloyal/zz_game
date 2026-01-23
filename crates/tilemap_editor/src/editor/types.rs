//! 资源（Resource）与组件（Component）的集中定义。
//!
//! 说明：为了便于工程化维护，这里把跨模块共享的数据类型统一放在一起。
pub const DEFAULT_SPRITESHEET: &str = "tiles.png";
pub const DEFAULT_SAVE_PATH: &str = "maps/map.ron";
pub const DEFAULT_UI_FONT_PATH: &str = "chinese.ttf";

mod tilemap;
mod camera;
mod clipboard;
mod config;
mod context_menu;
mod editor_state;
mod input;
mod selection;
mod tileset;
mod tools;
mod undo;
mod ui;

pub use tilemap::{LayerState, TileEntities};

pub use tilemap_core::{TileMapData, TileRef, TilesetId, DEFAULT_LAYER_COUNT};

pub use camera::WorldCamera;
pub use clipboard::{Clipboard, PasteState};
pub use config::EditorConfig;
pub use context_menu::{
    ContextMenuAction, ContextMenuBackdrop, ContextMenuCommand, ContextMenuDisabled, ContextMenuItem,
    ContextMenuRoot, ContextMenuState, PastePreview, PastePreviewTile,
};
pub use editor_state::EditorState;
pub use input::PanState;
pub use selection::{SelectionMovePreviewTile, SelectionRect, SelectionState};
pub use tileset::{
    PendingTileset, TilesetEntry, TilesetLibrary, TilesetLoading, TilesetRuntime,
    TilesetRuntimeEntry,
};
pub use tools::{
    BrushSettings, ShiftMapMode, ShiftMapSettings, ShiftModeButton, ShiftModeLabel, ToolButton, ToolKind,
    ToolState,
};
pub use undo::{CellChange, EditCommand, UndoStack};
pub use ui::{
    ActionButton, ActionKind, CanvasRoot, HudText,
    LayerPrevButton, LayerNextButton, LayerActiveLabel, LayerActiveVisLabel, LayerActiveVisToggleButton,
    LayerActiveLockLabel, LayerActiveLockToggleButton,
    MapSizeApplyButton, MapSizeFocus, MapSizeHeightField,
    MapSizeHeightText, MapSizeInput, MapSizeWidthField, MapSizeWidthText, PaletteRoot, PaletteScroll,
    PaletteTileButton, TilesetActiveLabel,
    PaletteSearchClearButton, PaletteSearchField, PaletteSearchInput, PaletteSearchText,
    PaletteZoomButton, PaletteZoomLevel,
	MenuBackdrop, MenuButton, MenuDropdown, MenuId, MenuItem, MenuState,
    TilesetBar, TilesetCategoryCycleButton, TilesetCategoryLabel,
    TilesetMenuRoot, TilesetSelectItem, TilesetToggleButton, UiFont, UiRoot, UiState,
    BrushSizeButton,
};
