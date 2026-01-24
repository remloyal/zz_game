use bevy::prelude::*;

use crate::editor::types::{EditorConfig, TileMapData, UndoStack};

use super::{apply_tile_change, TilemapRenderParams};

pub fn undo_redo_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut undo: ResMut<UndoStack>,
    config: Res<EditorConfig>,
    map: Option<ResMut<TileMapData>>,
    mut render: TilemapRenderParams,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let want_undo = keys.just_pressed(KeyCode::KeyZ)
        && !(keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    let want_redo = keys.just_pressed(KeyCode::KeyY)
        || (keys.just_pressed(KeyCode::KeyZ)
            && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)));

    if want_undo {
        let Some(cmd) = undo.undo.pop() else {
            return;
        };
        let layer_len = map.layer_len();
        for ch in &cmd.changes {
            if ch.idx < map.tiles.len() {
                map.tiles[ch.idx] = ch.before.clone();
                let layer = (ch.idx / layer_len) as u32;
                let local = ch.idx % layer_len;
                let x = (local % map.width as usize) as u32;
                let y = (local / map.width as usize) as u32;
                apply_tile_change(&mut render, &config, layer, x, y, &ch.after, &ch.before);
            }
        }
        undo.redo.push(cmd);
        return;
    }

    if want_redo {
        let Some(cmd) = undo.redo.pop() else {
            return;
        };
        let layer_len = map.layer_len();
        for ch in &cmd.changes {
            if ch.idx < map.tiles.len() {
                map.tiles[ch.idx] = ch.after.clone();
                let layer = (ch.idx / layer_len) as u32;
                let local = ch.idx % layer_len;
                let x = (local % map.width as usize) as u32;
                let y = (local / map.width as usize) as u32;
                apply_tile_change(&mut render, &config, layer, x, y, &ch.before, &ch.after);
            }
        }
        undo.undo.push(cmd);
        return;
    }
}
