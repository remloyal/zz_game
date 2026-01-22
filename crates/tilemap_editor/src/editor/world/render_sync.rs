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
        for y in 0..map.height {
            for x in 0..map.width {
                let tile_idx = map.idx_layer(layer, x, y);
                let entity_idx = tile_entities.idx_layer(layer, x, y);
                let entity = tile_entities.entities[entity_idx];

                let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) else {
                    continue;
                };

                apply_tile_visual(runtime, &map.tiles[tile_idx], &mut sprite, &mut tf, &mut vis, config);
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
