#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use tilemap_core::{LayerData, TileMapData, TileRef};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TileEntry {
    x: u32,
    y: u32,
    layer: u32,
    /// tileset 在 tileset_ids 中的索引
    tileset: u32,
    index: u32,
    #[serde(default)]
    rot: u8,
    #[serde(default)]
    flip_x: bool,
    #[serde(default)]
    flip_y: bool,
}

#[derive(Serialize, Deserialize)]
struct MapFileV4<TTileset> {
    width: u32,
    height: u32,
    /// 图层数量。tiles 按 layer0..layerN 的顺序扁平存储。
    layers: u32,
    #[serde(default)]
    layer_data: Vec<LayerData>,
    /// 地图所需 tileset 列表（用于跨机器拷贝后自动回显渲染）
    tilesets: Vec<TTileset>,
    /// tileset 的 id 列表，与 tilesets 同序
    tileset_ids: Vec<String>,
    /// 稀疏存储，仅保存已绘制的 tile
    tiles: Vec<TileEntry>,
}

pub fn encode_map_ron_v3<TTileset: Serialize>(
    map: &TileMapData,
    tilesets: Vec<TTileset>,
    tileset_ids: Vec<String>,
) -> Result<String, String> {
    if tilesets.len() != tileset_ids.len() {
        return Err("tilesets 与 tileset_ids 数量不一致".to_string());
    }

    let mut id_to_index = std::collections::HashMap::new();
    for (i, id) in tileset_ids.iter().enumerate() {
        id_to_index.insert(id.as_str(), i as u32);
    }

    let layer_len = map.layer_len();
    let mut tiles = Vec::new();
    for (i, tile_opt) in map.tiles.iter().enumerate() {
        let Some(tile) = tile_opt.as_ref() else {
            continue;
        };
        let Some(tileset_index) = id_to_index.get(tile.tileset_id.as_str()) else {
            return Err(format!("tileset_id 不在 tileset_ids 列表中: {}", tile.tileset_id));
        };
        let layer = (i / layer_len) as u32;
        let rem = (i % layer_len) as u32;
        let x = rem % map.width;
        let y = rem / map.width;
        tiles.push(TileEntry {
            x,
            y,
            layer,
            tileset: *tileset_index,
            index: tile.index,
            rot: tile.rot,
            flip_x: tile.flip_x,
            flip_y: tile.flip_y,
        });
    }

    let v4 = MapFileV4 {
        width: map.width,
        height: map.height,
        layers: map.layers.max(1),
        layer_data: map.layer_data.clone(),
        tilesets,
        tileset_ids,
        tiles,
    };

    ron::ser::to_string_pretty(&v4, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())
}

pub fn decode_map_ron<TTileset>(text: &str) -> Result<(TileMapData, Vec<TTileset>), String>
where
    for<'de> TTileset: Deserialize<'de>,
{
    let v4 = ron::from_str::<MapFileV4<TTileset>>(text).map_err(|e| e.to_string())?;
    let mut map = TileMapData::new_with_layers(v4.width, v4.height, v4.layers.max(1));

    if !v4.layer_data.is_empty() {
        map.layer_data = v4.layer_data;
        if map.layer_data.len() < map.layers as usize {
            for i in map.layer_data.len()..map.layers as usize {
                map.layer_data.push(LayerData {
                    name: format!("Layer {}", i + 1),
                    visible: true,
                    locked: false,
                });
            }
        }
    }

    for tile in v4.tiles {
        if tile.layer >= map.layers || tile.x >= map.width || tile.y >= map.height {
            continue;
        }
        let Some(tileset_id) = v4.tileset_ids.get(tile.tileset as usize) else {
            continue;
        };
        let idx = map.idx_layer(tile.layer, tile.x, tile.y);
        map.tiles[idx] = Some(TileRef {
            tileset_id: tileset_id.clone(),
            index: tile.index,
            rot: tile.rot,
            flip_x: tile.flip_x,
            flip_y: tile.flip_y,
        });
    }

    Ok((map, v4.tilesets))
}
