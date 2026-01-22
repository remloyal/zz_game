use bevy::prelude::*;

use crate::editor::types::{EditorConfig, TileEntities, TileMapData, TilesetRuntime, UndoStack};

use super::apply_map_to_entities;

pub fn undo_redo_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut undo: ResMut<UndoStack>,
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
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
        for ch in &cmd.changes {
            if ch.idx < map.tiles.len() {
                map.tiles[ch.idx] = ch.before.clone();
            }
        }
        undo.redo.push(cmd);
        apply_map_to_entities(&runtime, &map, &tile_entities, &mut tiles_q, &config);
        return;
    }

    if want_redo {
        let Some(cmd) = undo.redo.pop() else {
            return;
        };
        for ch in &cmd.changes {
            if ch.idx < map.tiles.len() {
                map.tiles[ch.idx] = ch.after.clone();
            }
        }
        undo.undo.push(cmd);
        apply_map_to_entities(&runtime, &map, &tile_entities, &mut tiles_q, &config);
        return;
    }
}
