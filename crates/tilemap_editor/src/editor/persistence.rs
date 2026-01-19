//! 地图的保存/读取（RON）。

use std::path::PathBuf;

use crate::editor::types::{TileMapData, TileRef, TilesetEntry, TilesetLibrary};

pub const DEFAULT_TILESET_LIBRARY_PATH: &str = "tilesets/library.ron";

#[derive(serde::Serialize, serde::Deserialize)]
struct MapFileV1 {
    width: u32,
    height: u32,
    tiles: Vec<Option<u32>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MapFileV2 {
    width: u32,
    height: u32,
    /// 地图所需 tileset 列表（用于跨机器拷贝后自动回显渲染）
    tilesets: Vec<TilesetEntry>,
    tiles: Vec<Option<TileRef>>,
}

pub fn save_tileset_library_to_file(lib: &TilesetLibrary, path: &str) -> Result<(), String> {
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let text = ron::ser::to_string_pretty(lib, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_tileset_library_from_file(path: &str) -> Result<TilesetLibrary, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Ok(TilesetLibrary::default());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    ron::from_str::<TilesetLibrary>(&text).map_err(|e| e.to_string())
}

pub fn save_map_to_file(map: &TileMapData, lib: &TilesetLibrary, path: &str) -> Result<(), String> {
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // 收集地图中实际使用到的 tileset（保证跨机器加载 map.ron 时可以自动把 tileset 加回库并回显）
    use std::collections::HashSet;
    let mut used: HashSet<&str> = HashSet::new();
    for t in &map.tiles {
        if let Some(r) = t.as_ref() {
            if !r.tileset_id.is_empty() {
                used.insert(r.tileset_id.as_str());
            }
        }
    }

    let mut tilesets: Vec<TilesetEntry> = Vec::new();
    for id in used {
        if let Some(e) = lib.entries.iter().find(|e| e.id == id) {
            tilesets.push(e.clone());
        } else {
            tilesets.push(TilesetEntry {
                id: id.to_string(),
                name: id.to_string(),
                category: "default".to_string(),
                asset_path: String::new(),
            });
        }
    }

    let v2 = MapFileV2 {
        width: map.width,
        height: map.height,
        tilesets,
        tiles: map.tiles.clone(),
    };

    let text = ron::ser::to_string_pretty(&v2, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_map_from_file(path: &str) -> Result<(TileMapData, Vec<TilesetEntry>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

    if let Ok(v2) = ron::from_str::<MapFileV2>(&text) {
        return Ok((
            TileMapData {
                width: v2.width,
                height: v2.height,
                tiles: v2.tiles,
            },
            v2.tilesets,
        ));
    }

    // 兼容旧版本：tiles 是 Option<u32>
    let v1 = ron::from_str::<MapFileV1>(&text).map_err(|e| e.to_string())?;
    let tiles = v1
        .tiles
        .into_iter()
        .map(|t| t.map(|index| TileRef {
            tileset_id: String::new(),
            index,
        }))
        .collect();

    Ok((
        TileMapData {
            width: v1.width,
            height: v1.height,
            tiles,
        },
        Vec::new(),
    ))
}
