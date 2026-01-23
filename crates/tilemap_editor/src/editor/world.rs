//! 世界（World）侧逻辑：相机、鼠标绘制、快捷键、数据 ↔ 渲染同步。
//!
//! 关键点：
//! - 鼠标绘制必须把屏幕坐标转换为世界坐标，需要一个明确的“世界相机”。
//! - UI 体系可能引入额外相机或渲染路径，若使用 `Query<(&Camera, &GlobalTransform)>::single()`
//!   会在多相机时失败，从而导致右侧无法绘制。

use bevy::prelude::*;

use super::tileset::rect_for_tile_index;
use super::types::{
    CellChange, EditCommand, EditorConfig, SelectionRect, TileEntities, TileMapData, TileRef,
    TilesetRuntime, UndoStack,
};
use super::{LEFT_PANEL_WIDTH_PX, UI_TOP_RESERVED_PX};

mod layers;
mod context_menu;
mod camera;
mod canvas;
mod undo;
mod shortcuts;
mod eyedropper;
mod shift_map;
mod selection_shortcuts;
mod selection_box;
mod save_load;
mod paste_helpers;
mod paste_apply;
mod paste_preview;
mod paste_transform;
mod render_sync;
mod selection_move;
mod selection_transform;
mod tools;

pub use camera::{camera_pan, camera_zoom, recenter_camera_on_map_change, setup_world};
pub use canvas::draw_canvas_helpers;
pub use undo::undo_redo_shortcuts;
pub use shortcuts::{keyboard_shortcuts, tool_shortcuts};
pub use eyedropper::{eyedropper_hold_shortcut, eyedropper_with_mouse};
pub use shift_map::shift_map_shortcuts;
pub use selection_shortcuts::{
    copy_paste_shortcuts, move_selection_shortcuts, selection_cut_delete_shortcuts,
    selection_selectall_cancel_shortcuts,
};
pub use selection_box::select_with_mouse;
pub use save_load::save_load_shortcuts;
pub use layers::layer_shortcuts;
pub use context_menu::{apply_context_menu_command, context_menu_clear_consumption, context_menu_open_close};
pub use paste_apply::paste_with_mouse;
pub use paste_preview::update_paste_preview;
pub use paste_transform::paste_transform_shortcuts;
pub use render_sync::{apply_map_to_entities, refresh_map_on_tileset_runtime_change};
pub use render_sync::sync_layer_visibility_on_layer_data_change;
pub use selection_move::selection_move_with_mouse;
pub use tools::{fill_with_mouse, paint_with_mouse, rect_with_mouse};

fn apply_tile_visual(
    runtime: &TilesetRuntime,
    tile: &Option<TileRef>,
    sprite: &mut Sprite,
    tf: &mut Transform,
    vis: &mut Visibility,
    config: &EditorConfig,
) {
    match tile {
        Some(TileRef {
            tileset_id,
            index,
            rot,
            flip_x,
            flip_y,
        }) => {
            let Some(atlas) = runtime.by_id.get(tileset_id) else {
                sprite.rect = None;
                sprite.flip_x = false;
                sprite.flip_y = false;
                tf.rotation = Quat::IDENTITY;
                *vis = Visibility::Hidden;
                return;
            };
            sprite.image = atlas.texture.clone();
            sprite.rect = Some(rect_for_tile_index(*index, atlas.columns, config.tile_size));
            sprite.flip_x = *flip_x;
            sprite.flip_y = *flip_y;
            let r = (*rot % 4) as f32;
            tf.rotation = Quat::from_rotation_z(-r * std::f32::consts::FRAC_PI_2);
            *vis = Visibility::Visible;
        }
        None => {
            sprite.flip_x = false;
            sprite.flip_y = false;
            tf.rotation = Quat::IDENTITY;
            *vis = Visibility::Hidden;
        }
    }
}

fn try_edit_single_map_tile<F>(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
    editor: F,
) -> bool
where
    F: FnOnce(&mut TileRef),
{
    let (Some(pos), Some(mut map), Some(tile_entities)) = (map_pos, map, tile_entities) else {
        return false;
    };
    let Some(layer) = map.topmost_layer_at(pos.x, pos.y) else {
        return false;
    };
    let idx = map.idx_layer(layer, pos.x, pos.y);
    if idx >= map.tiles.len() {
        return false;
    }
    let Some(mut after_tile) = map.tiles[idx].clone() else {
        return false;
    };
    let before = map.tiles[idx].clone();
    editor(&mut after_tile);
    after_tile.rot %= 4;
    let after = Some(after_tile.clone());
    if before == after {
        return false;
    }

    map.tiles[idx] = after.clone();
    undo.push(EditCommand {
        changes: vec![CellChange {
            idx,
            before,
            after: after.clone(),
        }],
    });

    let entity_idx = tile_entities.idx_layer(layer, pos.x, pos.y);
    if entity_idx >= tile_entities.entities.len() {
        return true;
    }
    let entity = tile_entities.entities[entity_idx];
    if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
        apply_tile_visual(runtime, &after, &mut sprite, &mut tf, &mut vis, config);
    }
    true
}

fn try_rotate_map_tile_ccw(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| t.rot = (t.rot + 3) % 4,
    )
}

fn try_rotate_map_tile_cw(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| t.rot = (t.rot + 1) % 4,
    )
}

fn try_flip_map_tile_x(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| t.flip_x = !t.flip_x,
    )
}

fn try_flip_map_tile_y(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| t.flip_y = !t.flip_y,
    )
}

fn try_reset_map_tile_transform(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| {
            t.rot = 0;
            t.flip_x = false;
            t.flip_y = false;
        },
    )
}

fn tile_world_center(x: u32, y: u32, tile_size: UVec2, z: f32) -> Vec3 {
    let tile_w = tile_size.x as f32;
    let tile_h = tile_size.y as f32;
    let world_x = (x as f32 + 0.5) * tile_w;
    let world_y = (y as f32 + 0.5) * tile_h;
    Vec3::new(world_x, world_y, z)
}

pub(crate) fn cursor_tile_pos(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    config: &EditorConfig,
    map_w: u32,
    map_h: u32,
) -> Option<UVec2> {
    let cursor_pos = window.cursor_position()?;
    if cursor_pos.x <= LEFT_PANEL_WIDTH_PX {
        return None;
    }
    if cursor_pos.y <= UI_TOP_RESERVED_PX {
        return None;
    }

    let world_pos = camera
        .viewport_to_world_2d(camera_transform, cursor_pos)
        .ok()?;

    let tile_w = config.tile_size.x as f32;
    let tile_h = config.tile_size.y as f32;
    if tile_w <= 0.0 || tile_h <= 0.0 {
        return None;
    }

    let x = (world_pos.x / tile_w).floor() as i32;
    let y = (world_pos.y / tile_h).floor() as i32;
    if x < 0 || y < 0 {
        return None;
    }
    let (x, y) = (x as u32, y as u32);
    if x >= map_w || y >= map_h {
        return None;
    }
    Some(UVec2::new(x, y))
}

fn rect_from_two(a: UVec2, b: UVec2) -> SelectionRect {
    SelectionRect {
        min: UVec2::new(a.x.min(b.x), a.y.min(b.y)),
        max: UVec2::new(a.x.max(b.x), a.y.max(b.y)),
    }
}


