//! Tilemap Editor（类似 RPG Maker 的基础版）
//!
//! 目标：
//! - 左侧 tileset 调色板（palette）选择 tile
//! - 右侧画布绘制/擦除
//! - 保存/读取地图（RON）
//!
//! 说明：
//! - 本 crate 使用 Bevy 0.18。
//! - UI 与世界渲染共存时，必须明确指定“世界相机”，否则鼠标拾取可能取到 UI 相机导致绘制失效。

mod paths;
mod persistence;
mod tileset;
mod types;
mod ui;
mod world;

use bevy::prelude::*;

use paths::workspace_assets_dir;
use tileset::{
    load_tileset_library_startup, open_spritesheet_shortcut, progress_spritesheet_loading, setup_map,
};
use types::{
    EditorConfig, EditorState, MapSizeInput, PanState, TilesetLibrary, TilesetLoading, TilesetRuntime,
    Clipboard, SelectionState, ShiftMapSettings, ToolState, UiState, UndoStack,
};
use ui::{
    action_button_click, apply_custom_map_size, build_palette_when_ready, map_size_text_input,
    map_size_widget_interactions, palette_scroll_wheel, palette_tile_click, setup_ui,
    apply_ui_font_to_all_text, load_ui_font, rebuild_tileset_menu_when_needed,
    sync_map_size_input_from_config, tileset_menu_item_click,
    tileset_category_cycle_click, tileset_menu_visibility, tileset_toggle_button_click,
    shift_mode_button_click, sync_tool_button_styles, tool_button_click, update_shift_mode_label,
    update_hud_text, update_map_size_field_text, update_tileset_active_label,
    update_tileset_category_label,
};
use world::{
    camera_pan, camera_zoom, draw_canvas_helpers, keyboard_shortcuts, paint_with_mouse,
    recenter_camera_on_map_change, refresh_map_on_tileset_runtime_change, save_load_shortcuts,
    copy_paste_shortcuts, fill_with_mouse, paste_with_mouse, rect_with_mouse, select_with_mouse,
    eyedropper_hold_shortcut, eyedropper_with_mouse, move_selection_shortcuts, setup_world,
	selection_cut_delete_shortcuts, shift_map_shortcuts, tool_shortcuts, undo_redo_shortcuts,
    selection_selectall_cancel_shortcuts,
};

/// UI 相关常量
pub const LEFT_PANEL_WIDTH_PX: f32 = 320.0;
pub const TILE_BUTTON_PX: f32 = 40.0;

/// 右侧画布顶部工具条高度（用于避免 UI 区域误绘制/缩放/拖拽）。
pub const RIGHT_TOPBAR_HEIGHT_PX: f32 = 56.0;

pub const UI_BG: Color = Color::srgb(0.12, 0.12, 0.12);
pub const UI_PANEL: Color = Color::srgb(0.16, 0.16, 0.16);
pub const UI_HIGHLIGHT: Color = Color::srgb(0.25, 0.45, 0.95);
pub const UI_BUTTON: Color = Color::srgb(0.22, 0.22, 0.22);
pub const UI_BUTTON_HOVER: Color = Color::srgb(0.28, 0.28, 0.28);
pub const UI_BUTTON_PRESS: Color = Color::srgb(0.35, 0.35, 0.35);

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
        .init_resource::<EditorConfig>()
        .init_resource::<EditorState>()
        .init_resource::<TilesetLibrary>()
        .init_resource::<TilesetRuntime>()
        .init_resource::<TilesetLoading>()
        .init_resource::<PanState>()
        .init_resource::<MapSizeInput>()
        .init_resource::<UiState>()
        .init_resource::<ToolState>()
        .init_resource::<Clipboard>()
		.init_resource::<SelectionState>()
		.init_resource::<ShiftMapSettings>()
        .init_resource::<UndoStack>()
        .add_systems(
            Startup,
            (
                load_ui_font,
                setup_world,
                setup_ui,
                setup_map,
                load_tileset_library_startup,
            ),
        )
        .add_systems(
            Update,
            (
                apply_ui_font_to_all_text,
                progress_spritesheet_loading,
                open_spritesheet_shortcut,
                update_tileset_active_label,
                update_tileset_category_label,
                tileset_category_cycle_click,
                tileset_toggle_button_click,
                tileset_menu_visibility,
                rebuild_tileset_menu_when_needed,
                tileset_menu_item_click,
                build_palette_when_ready,
                palette_tile_click,
                palette_scroll_wheel,
                tool_button_click,
                sync_tool_button_styles,
				shift_mode_button_click,
				update_shift_mode_label,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                map_size_widget_interactions,
                map_size_text_input,
                apply_custom_map_size,
                sync_map_size_input_from_config,
                update_map_size_field_text,
                action_button_click,
            ),
        )
        .add_systems(
            Update,
            (
                keyboard_shortcuts,
                tool_shortcuts,
                eyedropper_hold_shortcut,
                copy_paste_shortcuts,
                shift_map_shortcuts,
                move_selection_shortcuts,
                selection_cut_delete_shortcuts,
                selection_selectall_cancel_shortcuts,
                undo_redo_shortcuts,
                save_load_shortcuts,
            ),
        )
        .add_systems(
            Update,
            (
                refresh_map_on_tileset_runtime_change,
                recenter_camera_on_map_change,
                camera_zoom,
                camera_pan,
                draw_canvas_helpers,
                eyedropper_with_mouse,
                paint_with_mouse,
				rect_with_mouse,
				fill_with_mouse,
				select_with_mouse,
				paste_with_mouse,
                update_hud_text,
            ),
        )
        .run();
}
