use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::TilesetId;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TilesetEntry {
    pub id: TilesetId,
    pub name: String,
    pub category: String,
    /// 相对 `AssetPlugin.file_path` 的路径，例如：`tilesets/foo.png`
    pub asset_path: String,
}

#[derive(Resource, Serialize, Deserialize, Clone, Default)]
pub struct TilesetLibrary {
    pub entries: Vec<TilesetEntry>,
    pub active_id: Option<TilesetId>,
    pub active_category: String,
}

#[derive(Clone)]
pub struct TilesetRuntimeEntry {
    pub texture: Handle<Image>,
    pub columns: u32,
    pub rows: u32,
}

#[derive(Resource, Default)]
pub struct TilesetRuntime {
    pub by_id: HashMap<TilesetId, TilesetRuntimeEntry>,
}

#[derive(Clone)]
pub struct PendingTileset {
    pub id: TilesetId,
    pub texture: Handle<Image>,
}

#[derive(Resource, Default)]
pub struct TilesetLoading {
    pub pending: Vec<PendingTileset>,
}
