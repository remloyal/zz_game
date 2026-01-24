use bevy::prelude::*;

use crate::editor::types::{
    CellChange, EditCommand, EditorConfig, MapSizeFocus, MapSizeInput, ShiftMapMode,
    ShiftMapSettings, TileMapData, UndoStack,
};

use super::{apply_tile_change, TilemapRenderParams};

/// Shift Map：Ctrl + 方向键整体平移一格（空出来的格子填 None），并可撤销。
pub fn shift_map_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    settings: Res<ShiftMapSettings>,
    config: Res<EditorConfig>,
    mut undo: ResMut<UndoStack>,
    map: Option<ResMut<TileMapData>>,
    mut render: TilemapRenderParams,
) {
    // 输入框聚焦时不抢快捷键
    if input.focus != MapSizeFocus::None {
        return;
    }

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }

    let mut dx: i32 = 0;
    let mut dy: i32 = 0;
    if keys.just_pressed(KeyCode::ArrowLeft) {
        dx = -1;
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        dx = 1;
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        dy = 1;
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        dy = -1;
    } else {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let w = map.width;
    let h = map.height;
    if w == 0 || h == 0 {
        return;
    }

    let layers = map.layers.max(1);
    let layer_len = map.layer_len();
    let mut new_tiles = vec![None; layer_len * layers as usize];
    for layer in 0..layers {
        for y in 0..h {
            for x in 0..w {
                let (dst_x, dst_y) = match settings.mode {
                    ShiftMapMode::Blank => {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        (nx as u32, ny as u32)
                    }
                    ShiftMapMode::Wrap => {
                        let nx = (x as i32 + dx).rem_euclid(w as i32) as u32;
                        let ny = (y as i32 + dy).rem_euclid(h as i32) as u32;
                        (nx, ny)
                    }
                };

                let src = map.idx_layer(layer, x, y);
                let dst = map.idx_layer(layer, dst_x, dst_y);
                new_tiles[dst] = map.tiles[src].clone();
            }
        }
    }

    let mut cmd = EditCommand::default();
    for i in 0..map.tiles.len() {
        let before = map.tiles[i].clone();
        let after = new_tiles[i].clone();
        if before != after {
            cmd.changes.push(CellChange { idx: i, before, after });
        }
    }

    if cmd.changes.is_empty() {
        return;
    }

    map.tiles = new_tiles;
    let layer_len = map.layer_len();
    for ch in &cmd.changes {
        let layer = (ch.idx / layer_len) as u32;
        let local = ch.idx % layer_len;
        let x = (local % map.width as usize) as u32;
        let y = (local / map.width as usize) as u32;
        apply_tile_change(&mut render, &config, layer, x, y, &ch.before, &ch.after);
    }
    undo.push(cmd);
}
