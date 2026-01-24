use bevy::prelude::*;

use crate::editor::types::{
    CellChange, EditCommand, EditorConfig, EditorState, MapSizeFocus, MapSizeInput, TileEntities,
    BrushSettings, TileMapData, TilesetLibrary, TilesetRuntime, ToolKind, ToolState, UndoStack,
    PaletteSearchInput, LayerNameInput,
};

use super::apply_map_to_entities;

/// 键盘快捷键：选择 tile（[ / ]）+ 清空地图（R）。
pub fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    mut state: ResMut<EditorState>,
    mut undo: ResMut<UndoStack>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    map: Option<ResMut<TileMapData>>,
    config: Res<EditorConfig>,
) {
    let tile_count = lib
        .active_id
        .as_ref()
        .and_then(|id| runtime.by_id.get(id))
        .map(|a| a.columns.saturating_mul(a.rows))
        .unwrap_or(1)
        .max(1);

    if keys.just_pressed(KeyCode::BracketLeft) {
        state.selected_tile = state.selected_tile.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        state.selected_tile = (state.selected_tile + 1).min(tile_count - 1);
    }

    // 清空地图（做成可 Undo 的命令）。
    if keys.just_pressed(KeyCode::KeyR) {
        if let (Some(tile_entities), Some(mut map)) = (tile_entities.as_deref(), map) {
            let mut changes: Vec<CellChange> = Vec::new();
            for (idx, cell) in map.tiles.iter_mut().enumerate() {
                let before = cell.clone();
                if before.is_some() {
                    *cell = None;
                    changes.push(CellChange {
                        idx,
                        before,
                        after: None,
                    });
                }
            }
            undo.push(EditCommand { changes });
            apply_map_to_entities(&runtime, &map, tile_entities, &mut tiles_q, &config);
        }
    }
}

/// 工具快捷键：1/2/3/4/5/6 切换（笔刷/矩形/填充/选择/粘贴/橡皮）。
pub fn tool_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    palette_input: Res<PaletteSearchInput>,
    layer_name_input: Res<LayerNameInput>,
    mut tools: ResMut<ToolState>,
    mut brush: ResMut<BrushSettings>,
) {
    // 正在输入地图尺寸时，数字键留给输入框。
    if input.focus != MapSizeFocus::None || palette_input.focused || layer_name_input.focused {
        return;
    }

	let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
	if shift {
		if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
			brush.size = 1;
		} else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
			brush.size = 2;
		} else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
			brush.size = 3;
		}
		return;
	}

    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        tools.tool = ToolKind::Pencil;
    } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
        tools.tool = ToolKind::Rect;
    } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
        tools.tool = ToolKind::Fill;
    } else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
        tools.tool = ToolKind::Select;
    } else if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5) {
        tools.tool = ToolKind::Paste;
    } else if keys.just_pressed(KeyCode::Digit6) || keys.just_pressed(KeyCode::Numpad6) {
        tools.tool = ToolKind::Eraser;
    }
}
