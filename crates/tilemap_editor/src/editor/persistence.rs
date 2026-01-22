//! 地图的保存/读取（RON）。

use std::path::PathBuf;

use crate::editor::types::{TileMapData, TilesetEntry, TilesetLibrary};

pub const DEFAULT_TILESET_LIBRARY_PATH: &str = "tilesets/library.ron";

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

    let text = tilemap_format::encode_map_ron_v3(map, tilesets)?;
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_map_from_file(path: &str) -> Result<(TileMapData, Vec<TilesetEntry>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

    tilemap_format::decode_map_ron::<TilesetEntry>(&text)
}
