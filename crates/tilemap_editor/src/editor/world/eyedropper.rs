use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{
    ContextMenuState, EditorConfig, EditorState, MapSizeFocus, MapSizeInput, TileEntities,
    TileMapData, TilesetLibrary, ToolKind, ToolState, WorldCamera,
};

pub fn eyedropper_hold_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    mut tools: ResMut<ToolState>,
    mut prev: Local<Option<ToolKind>>,
) {
    if input.focus != MapSizeFocus::None {
        return;
    }

    if keys.just_pressed(KeyCode::KeyI) {
        if *prev == None {
            *prev = Some(tools.tool);
        }
        tools.tool = ToolKind::Eyedropper;
    }

    if keys.just_released(KeyCode::KeyI) {
        if let Some(back) = prev.take() {
            // 若用户手动点了吸管按钮，就不要强制切回
            if tools.tool == ToolKind::Eyedropper {
                tools.tool = back;
            }
        }
    }
}

/// 吸管工具：点击格子后把该格子的 tile 设为当前选择。
pub fn eyedropper_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tools: Res<ToolState>,
    menu: Res<ContextMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    map: Option<Res<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut state: ResMut<EditorState>,
    mut lib: ResMut<TilesetLibrary>,
) {
    if tools.tool != ToolKind::Eyedropper {
        return;
    }
    if menu.open || menu.consume_left_click {
        return;
    }
    // Alt + 拖拽用于“从任意工具框选”，避免与吸管冲突。
    if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
        return;
    }
    if keys.pressed(KeyCode::Space) {
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };
    let Some(pos) = super::cursor_tile_pos(
        window,
        camera,
        camera_transform,
        &config,
        tile_entities.width,
        tile_entities.height,
    ) else {
        return;
    };

    let Some(idx) = map.topmost_idx_at(pos.x, pos.y) else {
        return;
    };
    let Some(tile) = map.tiles.get(idx).cloned().flatten() else {
        return;
    };

    state.selected_tile = tile.index;
    lib.active_id = Some(tile.tileset_id.clone());
    if let Some(entry) = lib.entries.iter().find(|e| e.id == tile.tileset_id) {
        let cat = entry.category.trim();
        if !cat.is_empty() {
            lib.active_category = cat.to_string();
        }
    }
}
