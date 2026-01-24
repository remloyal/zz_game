//! 编辑器应用装配（Bevy App 构建与系统注册）。

use bevy::prelude::*;
use bevy::ui::UiSystems;

use super::{
	paths::workspace_assets_dir,
	tileset,
	types::{
		BrushSettings, Clipboard, ContextMenuCommand, ContextMenuState, EditorConfig, EditorState,
		LayerState, MapSizeInput, MenuState, PanState, PastePreview, PasteState, SelectionState, ShiftMapSettings,
		TilesetLibrary, TilesetLoading, TilesetRuntime, ToolState, UiState, UndoStack, PaletteSearchInput,
		LayerNameInput,
	},
	ui,
	world,
	UI_BG,
};

/// 运行编辑器。
pub fn run() {
	let assets_dir = workspace_assets_dir();

	App::new()
		// 用 ClearColor 控制背景色，而不是用全屏 UI 背景盖住世界渲染。
		.insert_resource(ClearColor(UI_BG))
		.add_plugins(
			DefaultPlugins
				.set(AssetPlugin {
					// 用绝对路径避免 cwd 差异导致找不到资源
					file_path: assets_dir.to_string_lossy().to_string(),
					..default()
				})
				.set(WindowPlugin {
					primary_window: Some(Window {
						title: "Tilemap Editor".to_string(),
						..default()
					}),
					..default()
				}),
		)
		// --- Resources ---
		.init_resource::<EditorConfig>()
		.init_resource::<EditorState>()
		.init_resource::<TilesetLibrary>()
		.init_resource::<TilesetRuntime>()
		.init_resource::<TilesetLoading>()
		.init_resource::<PanState>()
		.init_resource::<MapSizeInput>()
		.init_resource::<UiState>()
		.init_resource::<PaletteSearchInput>()
		.init_resource::<LayerNameInput>()
		.init_resource::<MenuState>()
		.init_resource::<ToolState>()
		.init_resource::<BrushSettings>()
		.init_resource::<Clipboard>()
		.init_resource::<ContextMenuState>()
		.init_resource::<ContextMenuCommand>()
		.init_resource::<PasteState>()
		.init_resource::<PastePreview>()
		.init_resource::<LayerState>()
		.init_resource::<SelectionState>()
		.init_resource::<ShiftMapSettings>()
		.init_resource::<UndoStack>()
		.add_systems(
			Startup,
			(
				// --- Startup ---
				ui::load_ui_font,
				world::setup_world,
				ui::setup_ui,
				tileset::setup_map,
				tileset::load_tileset_library_startup,
			),
		)
		.add_systems(
			Update,
			(
				(
					// --- UI: tileset/palette/tools ---
					(
						ui::apply_ui_font_to_all_text,
						tileset::progress_spritesheet_loading,
						tileset::open_spritesheet_shortcut,
						ui::layer_topbar_buttons,
						ui::update_layer_topbar_label,
						ui::update_tileset_active_label,
						ui::update_tileset_category_label,
						ui::tileset_category_cycle_click,
						ui::tileset_toggle_button_click,
						ui::tileset_menu_visibility,
					)
						.chain(),
					(
						(
							ui::tileset_menu_item_click,
							ui::palette_zoom_button_click,
							ui::palette_search_widget_interactions,
							ui::palette_search_text_input,
							ui::update_palette_search_text,
							ui::sync_palette_zoom_button_styles,
							ui::palette_tile_click,
							ui::palette_scroll_wheel,
						)
							.chain(),
						(
							ui::tool_button_click,
							ui::sync_tool_button_styles,
						ui::brush_size_button_click,
						ui::sync_brush_size_button_styles,
							ui::shift_mode_button_click,
							ui::update_shift_mode_label,
						)
							.chain(),
					)
						.chain(),
				)
					.chain(),
				(
					// --- UI: context menu ---
					ui::context_menu_sync,
					ui::context_menu_item_styles,
					ui::context_menu_backdrop_click,
					ui::context_menu_item_click,
				)
					.chain(),
			)
				.chain(),
		)
		.add_systems(
			Update,
			(
				// --- UI: map size + actions ---
				ui::map_size_widget_interactions,
				ui::map_size_text_input,
				ui::apply_custom_map_size,
				ui::sync_map_size_input_from_config,
				ui::update_map_size_field_text,
				ui::layer_name_widget_interactions,
				ui::layer_name_text_input,
				ui::apply_layer_name_change,
				ui::sync_layer_name_input_from_map,
				ui::update_layer_name_field_text,
				ui::menubar_button_interactions,
				ui::menubar_sync_button_styles,
				ui::menubar_backdrop_click_to_close,
				ui::menubar_close_when_menu_item_pressed,
				ui::action_button_click,
			),
		)
		.add_systems(
			PostUpdate,
			(
				// --- UI: rebuild / spawn & despawn (run late to avoid entity-despawn command errors) ---
				ui::menubar_rebuild_dropdown_when_needed,
				ui::context_menu_rebuild,
				ui::rebuild_tileset_menu_when_needed,
				ui::build_palette_when_ready,
			)
				.chain()
				.before(UiSystems::Layout),
		)
		.add_systems(
			PostUpdate,
			(
				// --- UI: palette scroll (needs valid ComputedNode size) ---
				ui::palette_clamp_scroll_position,
				ui::palette_apply_scroll_position_to_root,
			)
				.chain()
				.after(UiSystems::Layout),
		)
		.add_systems(
			Update,
			(
				// --- World: keyboard shortcuts ---
				world::keyboard_shortcuts,
				world::layer_shortcuts,
				world::tool_shortcuts,
				world::eyedropper_hold_shortcut,
				world::copy_paste_shortcuts,
				world::paste_transform_shortcuts,
				world::context_menu_open_close,
				world::context_menu_clear_consumption,
				world::apply_context_menu_command,
				world::shift_map_shortcuts,
				world::move_selection_shortcuts,
				world::selection_cut_delete_shortcuts,
				world::selection_selectall_cancel_shortcuts,
				world::undo_redo_shortcuts,
				world::save_load_shortcuts,
			),
		)
		.add_systems(
			Update,
			(
				// --- World: camera ---
				world::refresh_map_on_tileset_runtime_change,
				world::sync_layer_visibility_on_layer_data_change,
				world::recenter_camera_on_map_change,
				world::camera_zoom,
				world::camera_pan,
			),
		)
		// --- World: mouse tools + HUD ---
		.add_systems(Update, world::draw_canvas_helpers)
		.add_systems(Update, world::update_paste_preview)
		.add_systems(Update, world::selection_move_with_mouse)
		.add_systems(Update, world::eyedropper_with_mouse)
		.add_systems(Update, world::paint_with_mouse)
		.add_systems(Update, world::rect_with_mouse)
		.add_systems(Update, world::fill_with_mouse)
		.add_systems(Update, world::select_with_mouse)
		.add_systems(Update, (world::paste_with_mouse, ui::update_hud_text))
		.run();
}
