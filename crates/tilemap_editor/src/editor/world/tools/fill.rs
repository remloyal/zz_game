use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use std::collections::VecDeque;

use crate::editor::types::{
    CellChange, ContextMenuState, EditCommand, EditorConfig, EditorState, LayerState, TileEntities,
    TileMapData, TileRef, TilesetLibrary, TilesetRuntime, ToolKind, ToolState, UndoStack,
    WorldCamera,
};

use super::super::{apply_tile_visual, cursor_tile_pos};

/// 油漆桶（Flood Fill）：点击格子后，按 4 邻接填充“同类 tile”的连通区域。
///
/// - 左键：填充为当前选择的 tile
/// - 右键：保留给右键菜单
pub fn fill_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tools: Res<ToolState>,
    layer_state: Res<LayerState>,
    menu: Res<ContextMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    state: Res<EditorState>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    mut undo: ResMut<UndoStack>,
) {
    if tools.tool != ToolKind::Fill {
        return;
    }
    if menu.open || menu.consume_left_click {
        return;
    }

    // Alt + 拖拽用于“从任意工具框选”，避免与 Flood Fill 冲突。
    if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
        return;
    }

    // Space 用于平移（Space + 左键拖拽），避免与点击填充冲突。
    if keys.pressed(KeyCode::Space) {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };

    let layer = layer_state.active.min(map.layers.saturating_sub(1));
    let layer_locked = map
        .layer_data
        .get(layer as usize)
        .map(|d| d.locked)
        .unwrap_or(false);
    let layer_visible = map
        .layer_data
        .get(layer as usize)
        .map(|d| d.visible)
        .unwrap_or(true);
    if layer_locked {
        return;
    }

    let left_start = buttons.just_pressed(MouseButton::Left);
    if !left_start {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };

    let Some(pos) = cursor_tile_pos(
        window,
        camera,
        camera_transform,
        &config,
        tile_entities.width,
        tile_entities.height,
    ) else {
        return;
    };

    let erase = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let desired: Option<TileRef> = if erase {
        None
    } else {
        let Some(active_id) = lib.active_id.clone() else {
            return;
        };
        Some(TileRef {
            tileset_id: active_id,
            index: state.selected_tile,
            rot: 0,
            flip_x: false,
            flip_y: false,
        })
    };

    let start_idx = map.idx_layer(layer, pos.x, pos.y);
    let target = map.tiles[start_idx].clone();
    if target == desired {
        return;
    }

    let w = tile_entities.width;
    let h = tile_entities.height;
    let mut visited = vec![false; (w * h) as usize];
    let mut q = VecDeque::new();
    let start_local = (pos.y * w + pos.x) as usize;
    visited[start_local] = true;
    q.push_back((pos.x, pos.y));

    let mut cmd = EditCommand::default();

    while let Some((x, y)) = q.pop_front() {
        let idx = map.idx_layer(layer, x, y);
        if map.tiles[idx] != target {
            continue;
        }

        let before = map.tiles[idx].clone();
        map.tiles[idx] = desired.clone();
        cmd.changes.push(CellChange {
            idx,
            before,
            after: desired.clone(),
        });

        let push =
            |nx: i32, ny: i32, visited: &mut [bool], q: &mut VecDeque<(u32, u32)>, w: u32, h: u32| {
                if nx < 0 || ny < 0 {
                    return;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                if nx >= w || ny >= h {
                    return;
                }
                let nidx = (ny * w + nx) as usize;
                if visited[nidx] {
                    return;
                }
                visited[nidx] = true;
                q.push_back((nx, ny));
            };

        push(x as i32 - 1, y as i32, &mut visited, &mut q, w, h);
        push(x as i32 + 1, y as i32, &mut visited, &mut q, w, h);
        push(x as i32, y as i32 - 1, &mut visited, &mut q, w, h);
        push(x as i32, y as i32 + 1, &mut visited, &mut q, w, h);
    }

    // 局部刷新渲染（只刷改动格子）
    let layer_len = map.layer_len();
    let layer_offset = (layer as usize) * layer_len;
    for ch in &cmd.changes {
        let local = ch.idx.saturating_sub(layer_offset);
        let x = (local % map.width as usize) as u32;
        let y = (local / map.width as usize) as u32;
        let entity_idx = tile_entities.idx_layer(layer, x, y);
        if entity_idx >= tile_entities.entities.len() {
            continue;
        }
        let entity = tile_entities.entities[entity_idx];
        if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
            apply_tile_visual(&runtime, &ch.after, &mut sprite, &mut tf, &mut vis, &config);
            if !layer_visible {
                *vis = Visibility::Hidden;
            }
        }
    }

    undo.push(cmd);
}
