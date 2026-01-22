use bevy::prelude::*;

use crate::editor::types::{EditorConfig, TileEntities, TileMapData};
use crate::editor::types::TilesetRuntime;

use super::apply_tile_visual;

/// 当 tileset 运行时信息发生变化（图片加载完成/新增 tileset）时，刷新整张地图的渲染。
///
/// 这能保证“先加载 map + tileset 还在异步加载中”时，加载完成后自动回显。
pub fn refresh_map_on_tileset_runtime_change(
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    map: Option<Res<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
) {
    if !runtime.is_changed() {
        return;
    }

    let (Some(map), Some(tile_entities)) = (map.as_deref(), tile_entities.as_deref()) else {
        return;
    };

    apply_map_to_entities(&runtime, map, tile_entities, &mut tiles_q, &config);
}

/// 从地图数据同步到格子实体（rect + 可见性）。
pub fn apply_map_to_entities(
    runtime: &TilesetRuntime,
    map: &TileMapData,
    tile_entities: &TileEntities,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    config: &EditorConfig,
) {
    let layers_to_draw = map.layers.min(tile_entities.layers);
    for layer in 0..layers_to_draw {
        let layer_visible = map
            .layer_data
            .get(layer as usize)
            .map(|d| d.visible)
            .unwrap_or(true);
        for y in 0..map.height {
            for x in 0..map.width {
                let tile_idx = map.idx_layer(layer, x, y);
                let entity_idx = tile_entities.idx_layer(layer, x, y);
                let entity = tile_entities.entities[entity_idx];

                let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) else {
                    continue;
                };

                apply_tile_visual(runtime, &map.tiles[tile_idx], &mut sprite, &mut tf, &mut vis, config);
                if !layer_visible {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }

    // 若实体层数多于地图层数（例如地图旧格式只有 1 层但实体是 2 层），确保多余层不可见。
    for layer in layers_to_draw..tile_entities.layers {
        for y in 0..tile_entities.height {
            for x in 0..tile_entities.width {
                let entity_idx = tile_entities.idx_layer(layer, x, y);
                let entity = tile_entities.entities[entity_idx];
                if let Ok((_sprite, _tf, mut vis)) = tiles_q.get_mut(entity) {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

/// 当用户在 UI 中切换“图层可见性”时，只同步受影响的 layer。
///
/// 这避免了每次绘制都全量重刷，但仍能保证 toggle 可见性时立即生效。
pub fn sync_layer_visibility_on_layer_data_change(
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    map: Option<Res<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
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

    // 初始化：首次运行或 layers 数变了，直接全量应用一次。
    if prev_visible.len() != current.len() {
        *prev_visible = current;
        apply_map_to_entities(&runtime, map, tile_entities, &mut tiles_q, &config);
        return;
    }

    // 找到变更的 layer
    let mut any_changed = false;
    for (i, &v) in current.iter().enumerate() {
        if prev_visible[i] != v {
            any_changed = true;
            break;
        }
    }
    if !any_changed {
        return;
    }

    for layer in 0..layers {
        let now_visible = current[layer as usize];
        if prev_visible[layer as usize] == now_visible {
            continue;
        }

        if !now_visible {
            // 仅隐藏该层实体（不改 sprite），低成本。
            for y in 0..tile_entities.height {
                for x in 0..tile_entities.width {
                    let entity_idx = tile_entities.idx_layer(layer, x, y);
                    let entity = tile_entities.entities[entity_idx];
                    if let Ok((_sprite, _tf, mut vis)) = tiles_q.get_mut(entity) {
                        *vis = Visibility::Hidden;
                    }
                }
            }
        } else {
            // 重新应用该层 tile（恢复 None/Some 的可见性）。
            for y in 0..map.height {
                for x in 0..map.width {
                    let tile_idx = map.idx_layer(layer, x, y);
                    let entity_idx = tile_entities.idx_layer(layer, x, y);
                    let entity = tile_entities.entities[entity_idx];
                    if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
                        apply_tile_visual(
                            &runtime,
                            &map.tiles[tile_idx],
                            &mut sprite,
                            &mut tf,
                            &mut vis,
                            &config,
                        );
                    }
                }
            }
        }
    }

    *prev_visible = current;
}
