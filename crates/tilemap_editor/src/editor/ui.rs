//! 编辑器 UI：左侧工具栏 + tile palette + HUD。
//!
//! 该模块采用“门面（facade）+ 子模块”结构：
//! - [crates/tilemap_editor/src/editor/ui.rs](crates/tilemap_editor/src/editor/ui.rs) 仅负责 `mod` + `pub use`
//! - 具体实现放在 [crates/tilemap_editor/src/editor/ui/](crates/tilemap_editor/src/editor/ui/) 下

mod actions;
mod context_menu;
mod font;
mod hud;
mod layers;
mod map_size;
mod menubar;
mod palette;
mod root;
mod tileset_menu;
mod tools;
mod util;

pub use actions::action_button_click;
pub use context_menu::{
	context_menu_backdrop_click, context_menu_item_click, context_menu_item_styles, context_menu_rebuild,
	context_menu_sync,
};
pub use font::{apply_ui_font_to_all_text, load_ui_font};
pub use hud::update_hud_text;
pub use layers::{layer_topbar_buttons, update_layer_topbar_label};
pub use map_size::{
	apply_custom_map_size, map_size_text_input, map_size_widget_interactions,
	sync_map_size_input_from_config, update_map_size_field_text,
};

pub use menubar::{
	menubar_backdrop_click_to_close, menubar_button_interactions, menubar_close_when_menu_item_pressed,
	menubar_rebuild_dropdown_when_needed, menubar_sync_button_styles,
};

pub use palette::{
	build_palette_when_ready, palette_page_buttons, palette_scroll_wheel, palette_tile_click,
	palette_search_text_input, palette_search_widget_interactions, palette_zoom_button_click,
	sync_palette_zoom_button_styles, update_palette_page_label, update_palette_search_text,
};
pub use root::setup_ui;
pub use tileset_menu::{
	rebuild_tileset_menu_when_needed, tileset_category_cycle_click, tileset_menu_item_click,
	tileset_menu_visibility, tileset_toggle_button_click, update_tileset_active_label,
	update_tileset_category_label,
};
pub use tools::{
	brush_size_button_click, shift_mode_button_click, sync_brush_size_button_styles,
	sync_tool_button_styles, tool_button_click, update_shift_mode_label,
};
