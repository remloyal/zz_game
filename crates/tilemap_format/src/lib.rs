#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use tilemap_core::{TileMapData, TileRef, DEFAULT_LAYER_COUNT};

#[derive(Serialize, Deserialize)]
struct MapFileV1 {
    width: u32,
    height: u32,
    tiles: Vec<Option<u32>>,
}

#[derive(Serialize, Deserialize)]
struct MapFileV2<TTileset> {
    width: u32,
    height: u32,
    /// 地图所需 tileset 列表（用于跨机器拷贝后自动回显渲染）
    tilesets: Vec<TTileset>,
    tiles: Vec<Option<TileRef>>,
}

#[derive(Serialize, Deserialize)]
struct MapFileV3<TTileset> {
    width: u32,
    height: u32,
    /// 图层数量。tiles 按 layer0..layerN 的顺序扁平存储。
    layers: u32,
    /// 地图所需 tileset 列表（用于跨机器拷贝后自动回显渲染）
    tilesets: Vec<TTileset>,
    tiles: Vec<Option<TileRef>>,
}

pub fn encode_map_ron_v3<TTileset: Serialize>(
    map: &TileMapData,
    tilesets: Vec<TTileset>,
) -> Result<String, String> {
    let v3 = MapFileV3 {
        width: map.width,
        height: map.height,
        layers: map.layers.max(1),
        tilesets,
        tiles: map.tiles.clone(),
    };

    ron::ser::to_string_pretty(&v3, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())
}

pub fn decode_map_ron<TTileset>(text: &str) -> Result<(TileMapData, Vec<TTileset>), String>
where
    for<'de> TTileset: Deserialize<'de>,
{
    // 最新版本：V3（含 layers）
    if let Ok(v3) = ron::from_str::<MapFileV3<TTileset>>(text) {
        let mut map = TileMapData::new_with_layers(v3.width, v3.height, v3.layers.max(DEFAULT_LAYER_COUNT));
        let want_len = map.tiles.len();
        let copy_len = want_len.min(v3.tiles.len());
        map.tiles[..copy_len].clone_from_slice(&v3.tiles[..copy_len]);
        return Ok((map, v3.tilesets));
    }

    // 兼容 V2：只有一层 tiles（width*height）
    if let Ok(v2) = ron::from_str::<MapFileV2<TTileset>>(text) {
        let mut map = TileMapData::new_with_layers(v2.width, v2.height, DEFAULT_LAYER_COUNT);
        let layer_len = map.layer_len();
        let copy_len = layer_len.min(v2.tiles.len());
        map.tiles[..copy_len].clone_from_slice(&v2.tiles[..copy_len]);
        return Ok((map, v2.tilesets));
    }

    // 兼容 V1：tiles 是 Option<u32> 且没有 tilesets
    let v1 = ron::from_str::<MapFileV1>(text).map_err(|e| e.to_string())?;
    let tiles: Vec<Option<TileRef>> = v1
        .tiles
        .into_iter()
        .map(|t| {
            t.map(|index| TileRef {
                tileset_id: String::new(),
                index,
                rot: 0,
                flip_x: false,
                flip_y: false,
            })
        })
        .collect();

    let mut map = TileMapData::new_with_layers(v1.width, v1.height, DEFAULT_LAYER_COUNT);
    let layer_len = map.layer_len();
    let copy_len = layer_len.min(tiles.len());
    map.tiles[..copy_len].clone_from_slice(&tiles[..copy_len]);

    Ok((map, Vec::new()))
}
