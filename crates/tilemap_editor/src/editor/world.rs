//! 世界（World）侧逻辑：相机、鼠标绘制、快捷键、数据 ↔ 渲染同步。
//!
//! 关键点：
//! - 鼠标绘制必须把屏幕坐标转换为世界坐标，需要一个明确的“世界相机”。
//! - UI 体系可能引入额外相机或渲染路径，若使用 `Query<(&Camera, &GlobalTransform)>::single()`
//!   会在多相机时失败，从而导致右侧无法绘制。

use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy_ecs_tilemap::prelude::*;

use super::types::{
    CellChange, EditCommand, EditorConfig, SelectionRect, TileEntities, TileMapData, TileRef,
    TilesetRuntime, UndoStack,
};
use super::{LEFT_PANEL_WIDTH_PX, UI_TOP_RESERVED_PX};
use crate::editor::util::despawn_silently;

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
pub use render_sync::{refresh_map_on_tileset_runtime_change, rebuild_tilemaps, sync_layer_visibility_on_layer_data_change};
pub use selection_move::selection_move_with_mouse;
pub use tools::{fill_with_mouse, paint_with_mouse, rect_with_mouse};

#[derive(SystemParam)]
pub struct TilemapRenderParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub tile_entities: Option<ResMut<'w, TileEntities>>,
    pub runtime: Res<'w, TilesetRuntime>,
    pub tile_storage_q: Query<'w, 's, &'static mut TileStorage>,
}

pub(crate) fn apply_tile_change(
    render: &mut TilemapRenderParams,
    config: &EditorConfig,
    layer: u32,
    x: u32,
    y: u32,
    before: &Option<TileRef>,
    after: &Option<TileRef>,
) {
    let Some(tile_entities) = render.tile_entities.as_mut() else {
        return;
    };

    if let Some(before_tile) = before {
        if after.as_ref().map(|t| &t.tileset_id) != Some(&before_tile.tileset_id) {
            remove_tile_from_tileset(
                &mut render.commands,
                &mut render.tile_storage_q,
                tile_entities,
                &before_tile.tileset_id,
                layer,
                x,
                y,
            );
        }
    }

    if let Some(after_tile) = after {
        set_tile_in_tileset(
            &mut render.commands,
            &mut render.tile_storage_q,
            tile_entities,
            &render.runtime,
            config,
            &after_tile.tileset_id,
            layer,
            x,
            y,
            after_tile,
        );
    } else if let Some(before_tile) = before {
        remove_tile_from_tileset(
            &mut render.commands,
            &mut render.tile_storage_q,
            tile_entities,
            &before_tile.tileset_id,
            layer,
            x,
            y,
        );
    }
}

fn ensure_tilemap_layer(
    commands: &mut Commands,
    tile_entities: &mut TileEntities,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tileset_id: &str,
    layer: u32,
) -> Option<Entity> {
    let tileset_id = tileset_id.to_string();
    if let Some(entity) = tile_entities.layer_entity(&tileset_id, layer) {
        if entity != Entity::PLACEHOLDER {
            return Some(entity);
        }
    }

    let Some(rt) = runtime.by_id.get(&tileset_id) else {
        return None;
    };

    let map_size = TilemapSize {
        x: tile_entities.width,
        y: tile_entities.height,
    };
    let tile_size = TilemapTileSize {
        x: config.tile_size.x as f32,
        y: config.tile_size.y as f32,
    };
    let grid_size = TilemapGridSize {
        x: config.tile_size.x as f32,
        y: config.tile_size.y as f32,
    };
    let storage = TileStorage::empty(map_size);
    let order = tile_entities.tileset_index(&tileset_id);
    let z = layer as f32 * 10.0 + order as f32 * 0.01;
    let offset = Vec3::new(tile_size.x * 0.5, tile_size.y * 0.5, z);

    let map_entity = commands.spawn_empty().id();
    commands.entity(map_entity).insert(TilemapBundle {
        size: map_size,
        storage,
        tile_size,
        grid_size,
        texture: TilemapTexture::Single(rt.texture.clone()),
        transform: Transform::from_translation(offset),
        ..Default::default()
    });

    tile_entities.set_layer_entity(tileset_id, layer, map_entity);
    Some(map_entity)
}

fn remove_tile_from_tileset(
    commands: &mut Commands,
    tile_storage_q: &mut Query<&mut TileStorage>,
    tile_entities: &mut TileEntities,
    tileset_id: &str,
    layer: u32,
    x: u32,
    y: u32,
) {
    let tileset_id = tileset_id.to_string();
    let Some(map_entity) = tile_entities.layer_entity(&tileset_id, layer) else {
        return;
    };
    let Ok(mut storage) = tile_storage_q.get_mut(map_entity) else {
        return;
    };
    let pos = TilePos { x, y };
    if let Some(tile_entity) = storage.get(&pos) {
        despawn_silently(commands, tile_entity);
        storage.remove(&pos);
    }
}

fn set_tile_in_tileset(
    commands: &mut Commands,
    tile_storage_q: &mut Query<&mut TileStorage>,
    tile_entities: &mut TileEntities,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tileset_id: &str,
    layer: u32,
    x: u32,
    y: u32,
    tile: &TileRef,
) {
    let Some(map_entity) = ensure_tilemap_layer(
        commands,
        tile_entities,
        runtime,
        config,
        tileset_id,
        layer,
    ) else {
        return;
    };

    let Ok(mut storage) = tile_storage_q.get_mut(map_entity) else {
        return;
    };
    let pos = TilePos { x, y };
    if let Some(tile_entity) = storage.get(&pos) {
        commands.entity(tile_entity).insert(TileTextureIndex(tile.index));
        commands.entity(tile_entity).insert(tile_flip_from_ref(tile));
        return;
    }

    let tile_entity = commands
        .spawn(TileBundle {
            position: pos,
            tilemap_id: TilemapId(map_entity),
            texture_index: TileTextureIndex(tile.index),
            flip: tile_flip_from_ref(tile),
            ..Default::default()
        })
        .id();
    storage.set(&pos, tile_entity);
}

fn tile_flip_from_ref(tile: &TileRef) -> TileFlip {
    let mut flip = TileFlip {
        x: tile.flip_x,
        y: tile.flip_y,
        d: false,
    };
    match tile.rot % 4 {
        0 => {}
        1 => {
            flip.d = true;
            flip.x = !flip.x;
        }
        2 => {
            flip.x = !flip.x;
            flip.y = !flip.y;
        }
        3 => {
            flip.d = true;
            flip.y = !flip.y;
        }
        _ => {}
    }
    flip
}

fn try_edit_single_map_tile<F>(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    render: &mut TilemapRenderParams,
    config: &EditorConfig,
    undo: &mut UndoStack,
    editor: F,
) -> bool
where
    F: FnOnce(&mut TileRef),
{
    let (Some(pos), Some(mut map)) = (map_pos, map) else {
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
            before: before.clone(),
            after: after.clone(),
        }],
    });

    apply_tile_change(render, config, layer, pos.x, pos.y, &before, &after);
    true
}

fn try_rotate_map_tile_ccw(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    render: &mut TilemapRenderParams,
    config: &EditorConfig,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        render,
        config,
        undo,
        |t| t.rot = (t.rot + 3) % 4,
    )
}

fn try_rotate_map_tile_cw(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    render: &mut TilemapRenderParams,
    config: &EditorConfig,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        render,
        config,
        undo,
        |t| t.rot = (t.rot + 1) % 4,
    )
}

fn try_flip_map_tile_x(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    render: &mut TilemapRenderParams,
    config: &EditorConfig,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        render,
        config,
        undo,
        |t| t.flip_x = !t.flip_x,
    )
}

fn try_flip_map_tile_y(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    render: &mut TilemapRenderParams,
    config: &EditorConfig,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        render,
        config,
        undo,
        |t| t.flip_y = !t.flip_y,
    )
}

fn try_reset_map_tile_transform(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    render: &mut TilemapRenderParams,
    config: &EditorConfig,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        render,
        config,
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


