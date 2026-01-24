use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{EditorConfig, TileEntities, TileMapData, TilesetRuntime, WorldCamera};
use crate::editor::util::despawn_silently;
use tilemap_core::TileRef;

#[derive(Component)]
pub(crate) struct ChunkFilled;

/// 当 tileset 运行时信息发生变化（图片加载完成/新增 tileset）时，刷新整张地图的渲染。
pub fn refresh_map_on_tileset_runtime_change(
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    map: Option<Res<TileMapData>>,
    mut tile_entities: Option<ResMut<TileEntities>>,
    mut tile_storage_q: Query<&mut TileStorage>,
    tile_q: Query<Entity, With<TilemapId>>,
    mut commands: Commands,
) {
    if !runtime.is_changed() {
        return;
    }
    let (Some(map), Some(mut tile_entities)) = (map.as_deref(), tile_entities.as_mut()) else {
        return;
    };

    rebuild_tilemaps(
        &mut commands,
        &tile_q,
        &runtime,
        map,
        &mut tile_entities,
        &mut tile_storage_q,
        &config,
    );
}

/// 从地图数据重建 tilemap（用于载入/重建）。
pub fn rebuild_tilemaps(
    commands: &mut Commands,
    tile_q: &Query<Entity, With<TilemapId>>,
    _runtime: &TilesetRuntime,
    map: &TileMapData,
    tile_entities: &mut TileEntities,
    _tile_storage_q: &mut Query<&mut TileStorage>,
    _config: &EditorConfig,
) {
    // 清理旧 tile 实体
    for e in tile_q.iter() {
        despawn_silently(commands, e);
    }
    // 清理旧 tilemap 实体
    for e in tile_entities.all_tilemap_entities() {
        despawn_silently(commands, e);
    }
    tile_entities.tilemaps.clear();
    tile_entities.tileset_order.clear();

    // 收集 tileset_id
    let mut tileset_ids: Vec<String> = map
        .tiles
        .iter()
        .filter_map(|t| t.as_ref().map(|r| r.tileset_id.clone()))
        .collect();
    tileset_ids.sort();
    tileset_ids.dedup();

    tile_entities.tileset_order = tileset_ids;
}

fn ensure_chunk_tilemap(
    commands: &mut Commands,
    tile_entities: &mut TileEntities,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tileset_id: &str,
    layer: u32,
    cx: u32,
    cy: u32,
    visible: bool,
) -> Option<Entity> {
    let tileset_id = tileset_id.to_string();
    if let Some(entity) = tile_entities.chunk_entity(&tileset_id, layer, cx, cy) {
        return Some(entity);
    }

    let rt = runtime.by_id.get(&tileset_id)?;
    let chunk_size = tile_entities.chunk_size.max(1);
    let map_size = TilemapSize {
        x: chunk_size,
        y: chunk_size,
    };
    let tile_size = TilemapTileSize {
        x: config.tile_size.x as f32,
        y: config.tile_size.y as f32,
    };
    let grid_size = TilemapGridSize {
        x: config.tile_size.x as f32,
        y: config.tile_size.y as f32,
    };
    let order = tile_entities.tileset_index(&tileset_id);
    let z = layer as f32 * 10.0 + order as f32 * 0.01;
    let origin_x = cx as f32 * chunk_size as f32 * tile_size.x;
    let origin_y = cy as f32 * chunk_size as f32 * tile_size.y;
    let offset = Vec3::new(origin_x + tile_size.x * 0.5, origin_y + tile_size.y * 0.5, z);

    let map_entity = commands.spawn_empty().id();
    let storage = TileStorage::empty(map_size);
    commands.entity(map_entity).insert(TilemapBundle {
        size: map_size,
        storage,
        tile_size,
        grid_size,
        texture: TilemapTexture::Single(rt.texture.clone()),
        transform: Transform::from_translation(offset),
        visibility: if visible { Visibility::Visible } else { Visibility::Hidden },
        ..Default::default()
    });

    tile_entities.set_chunk_entity(tileset_id, layer, cx, cy, map_entity);
    Some(map_entity)
}

/// 根据视野范围裁剪 chunk tilemap 的可见性。
pub fn update_visible_chunks(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Projection, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    runtime: Res<TilesetRuntime>,
    map: Option<Res<TileMapData>>,
    tile_entities: Option<ResMut<TileEntities>>,
    mut tile_storage_q: Query<&mut TileStorage>,
    chunk_filled_q: Query<Option<&ChunkFilled>>,
    mut map_vis_q: Query<&mut Visibility>,
    mut commands: Commands,
) {
    let Some(map) = map.as_deref() else {
        return;
    };
    let Some(mut tile_entities) = tile_entities else {
        return;
    };
    let tile_entities = &mut *tile_entities;
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((proj, tf)) = camera_q.single() else {
        return;
    };
    let Projection::Orthographic(ortho) = proj else {
        return;
    };

    let tile_w = config.tile_size.x as f32;
    let tile_h = config.tile_size.y as f32;
    if tile_w <= 0.0 || tile_h <= 0.0 {
        return;
    }
    let chunk_size = tile_entities.chunk_size.max(1);
    let chunk_w = tile_w * chunk_size as f32;
    let chunk_h = tile_h * chunk_size as f32;

    let world_w = window.width() * ortho.scale;
    let world_h = window.height() * ortho.scale;
    let center = tf.translation().truncate();
    let min = center - Vec2::new(world_w * 0.5, world_h * 0.5);
    let max = center + Vec2::new(world_w * 0.5, world_h * 0.5);

    let max_cx = (map.width.saturating_sub(1) / chunk_size) as i32;
    let max_cy = (map.height.saturating_sub(1) / chunk_size) as i32;
    let mut cx_min = (min.x / chunk_w).floor() as i32;
    let mut cy_min = (min.y / chunk_h).floor() as i32;
    let mut cx_max = (max.x / chunk_w).floor() as i32;
    let mut cy_max = (max.y / chunk_h).floor() as i32;
    cx_min = cx_min.clamp(0, max_cx);
    cy_min = cy_min.clamp(0, max_cy);
    cx_max = cx_max.clamp(0, max_cx);
    cy_max = cy_max.clamp(0, max_cy);

    let layer_count = map.layers.max(1);
    for layer in 0..layer_count {
        let layer_visible = map
            .layer_data
            .get(layer as usize)
            .map(|d| d.visible)
            .unwrap_or(true);

        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                let cx_u = cx as u32;
                let cy_u = cy as u32;
                let start_x = cx_u * chunk_size;
                let start_y = cy_u * chunk_size;
                let end_x = (start_x + chunk_size).min(map.width);
                let end_y = (start_y + chunk_size).min(map.height);

                let mut tiles_by_set: std::collections::HashMap<String, Vec<(u32, u32, TileRef)>> =
                    Default::default();
                for y in start_y..end_y {
                    for x in start_x..end_x {
                        let idx = map.idx_layer(layer, x, y);
                        if let Some(tile) = &map.tiles[idx] {
                            tiles_by_set
                                .entry(tile.tileset_id.clone())
                                .or_default()
                                .push((x, y, tile.clone()));
                        }
                    }
                }

                for (tileset_id, tiles) in tiles_by_set {
                    let Some(map_entity) = ensure_chunk_tilemap(
                        &mut commands,
                        tile_entities,
                        &runtime,
                        &config,
                        &tileset_id,
                        layer,
                        cx_u,
                        cy_u,
                        layer_visible,
                    ) else {
                        continue;
                    };

                    let filled = chunk_filled_q
                        .get(map_entity)
                        .ok()
                        .flatten()
                        .is_some();
                    if !filled {
                        let Ok(mut storage) = tile_storage_q.get_mut(map_entity) else {
                            continue;
                        };
                        for (x, y, tile) in tiles {
                            let lx = x % chunk_size;
                            let ly = y % chunk_size;
                            let pos = TilePos { x: lx, y: ly };
                            if storage.get(&pos).is_some() {
                                continue;
                            }
                            let tile_entity = commands
                                .spawn(TileBundle {
                                    position: pos,
                                    tilemap_id: TilemapId(map_entity),
                                    texture_index: TileTextureIndex(tile.index),
                                    flip: tile_flip_from_ref(&tile),
                                    ..Default::default()
                                })
                                .id();
                            storage.set(&pos, tile_entity);
                        }
                        commands.entity(map_entity).insert(ChunkFilled);
                    }
                }
            }
        }
    }

    for layers in tile_entities.tilemaps.values() {
        for (layer, layer_map) in layers.iter().enumerate() {
            let layer_visible = map
                .layer_data
                .get(layer)
                .map(|d| d.visible)
                .unwrap_or(true);
            for (&(cx, cy), &entity) in layer_map.iter() {
                let in_view = (cx as i32) >= cx_min
                    && (cx as i32) <= cx_max
                    && (cy as i32) >= cy_min
                    && (cy as i32) <= cy_max;
                if let Ok(mut vis) = map_vis_q.get_mut(entity) {
                    *vis = if layer_visible && in_view {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }
    }
}

/// 当用户在 UI 中切换“图层可见性”时，同步 tilemap entity 的可见性。
pub fn sync_layer_visibility_on_layer_data_change(
    map: Option<Res<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut map_vis_q: Query<&mut Visibility>,
    mut prev_visible: Local<Vec<bool>>,
) {
    let (Some(map), Some(tile_entities)) = (map.as_deref(), tile_entities.as_deref()) else {
        prev_visible.clear();
        return;
    };

    let layers = map.layers.min(tile_entities.layers);
    if layers == 0 {
        prev_visible.clear();
        return;
    }

    let mut current: Vec<bool> = Vec::with_capacity(layers as usize);
    for layer in 0..layers {
        current.push(
            map.layer_data
                .get(layer as usize)
                .map(|d| d.visible)
                .unwrap_or(true),
        );
    }

    if prev_visible.len() != current.len() {
        *prev_visible = current.clone();
    }

    for layers in tile_entities.tilemaps.values() {
        for (layer, layer_map) in layers.iter().enumerate() {
            if layer >= current.len() {
                continue;
            }
            for &map_entity in layer_map.values() {
                if let Ok(mut vis) = map_vis_q.get_mut(map_entity) {
                    *vis = if current[layer] { Visibility::Visible } else { Visibility::Hidden };
                }
            }
        }
    }

    *prev_visible = current;
}

fn tile_flip_from_ref(tile: &tilemap_core::TileRef) -> TileFlip {
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
