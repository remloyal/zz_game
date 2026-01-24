use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::editor::types::{EditorConfig, TileEntities, TileMapData, TilesetRuntime};
use crate::editor::util::despawn_silently;

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
    runtime: &TilesetRuntime,
    map: &TileMapData,
    tile_entities: &mut TileEntities,
    tile_storage_q: &mut Query<&mut TileStorage>,
    config: &EditorConfig,
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

    let map_size = TilemapSize { x: map.width, y: map.height };
    let tile_size = TilemapTileSize {
        x: config.tile_size.x as f32,
        y: config.tile_size.y as f32,
    };
    let grid_size = TilemapGridSize {
        x: config.tile_size.x as f32,
        y: config.tile_size.y as f32,
    };

    // 收集 tileset_id
    let mut tileset_ids: Vec<String> = map
        .tiles
        .iter()
        .filter_map(|t| t.as_ref().map(|r| r.tileset_id.clone()))
        .collect();
    tileset_ids.sort();
    tileset_ids.dedup();

    for tileset_id in tileset_ids {
        let Some(rt) = runtime.by_id.get(&tileset_id) else {
            continue;
        };

        let order = tile_entities.tileset_index(&tileset_id);
        for layer in 0..tile_entities.layers {
            let map_entity = commands.spawn_empty().id();
            let storage = TileStorage::empty(map_size);
            let z = layer as f32 * 10.0 + order as f32 * 0.01;
            let visible = map
                .layer_data
                .get(layer as usize)
                .map(|d| d.visible)
                .unwrap_or(true);

            let offset = Vec3::new(tile_size.x * 0.5, tile_size.y * 0.5, z);
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

            tile_entities.set_layer_entity(tileset_id.clone(), layer, map_entity);
        }
    }

    // 逐格填充 tile
    for layer in 0..map.layers {
        for y in 0..map.height {
            for x in 0..map.width {
                let idx = map.idx_layer(layer, x, y);
                let Some(tile) = &map.tiles[idx] else {
                    continue;
                };
                let Some(map_entity) = tile_entities.layer_entity(&tile.tileset_id, layer) else {
                    continue;
                };
                let Ok(mut storage) = tile_storage_q.get_mut(map_entity) else {
                    continue;
                };
                let pos = TilePos { x, y };
                if storage.get(&pos).is_some() {
                    continue;
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

    for (tileset_id, layers_entities) in tile_entities.tilemaps.iter() {
        let _ = tileset_id;
        for (layer, &map_entity) in layers_entities.iter().enumerate() {
            if layer >= current.len() {
                continue;
            }
            if let Ok(mut vis) = map_vis_q.get_mut(map_entity) {
                *vis = if current[layer] { Visibility::Visible } else { Visibility::Hidden };
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
