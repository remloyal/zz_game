use bevy::prelude::*;
use std::collections::HashMap;

use tilemap_core::TilesetId;

pub const DEFAULT_CHUNK_SIZE: u32 = 64;

/// 地图渲染实体索引（bevy_ecs_tilemap 后端）。
#[derive(Resource)]
pub struct TileEntities {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    /// 每个 tilemap chunk 的边长（格子数）
    pub chunk_size: u32,
    /// tileset_id -> 每层 tilemap chunk entity（len = layers）
    pub tilemaps: HashMap<TilesetId, Vec<HashMap<(u32, u32), Entity>>>,
    /// tileset 创建顺序（用于稳定 z 排序）
    pub tileset_order: Vec<TilesetId>,
}

impl TileEntities {
    pub fn chunk_entity(
        &self,
        tileset_id: &TilesetId,
        layer: u32,
        cx: u32,
        cy: u32,
    ) -> Option<Entity> {
        let layers = self.tilemaps.get(tileset_id)?;
        let layer_map = layers.get(layer as usize)?;
        layer_map.get(&(cx, cy)).copied()
    }

    pub fn set_chunk_entity(
        &mut self,
        tileset_id: TilesetId,
        layer: u32,
        cx: u32,
        cy: u32,
        entity: Entity,
    ) {
        let layers = self.layers.max(1) as usize;
        let entry = self
            .tilemaps
            .entry(tileset_id)
            .or_insert_with(|| vec![HashMap::new(); layers]);
        if entry.len() < layers {
            entry.resize_with(layers, HashMap::new);
        }
        if let Some(layer_map) = entry.get_mut(layer as usize) {
            layer_map.insert((cx, cy), entity);
        }
    }

    pub fn tileset_index(&mut self, tileset_id: &TilesetId) -> usize {
        if let Some(i) = self.tileset_order.iter().position(|id| id == tileset_id) {
            return i;
        }
        self.tileset_order.push(tileset_id.clone());
        self.tileset_order.len() - 1
    }

    pub fn all_tilemap_entities(&self) -> Vec<Entity> {
        let mut out = Vec::new();
        for layers in self.tilemaps.values() {
            for layer_map in layers {
                for &e in layer_map.values() {
                    out.push(e);
                }
            }
        }
        out
    }
}

/// 当前编辑图层（0=底层，1=上层…）。
#[derive(Resource, Clone, Copy)]
pub struct LayerState {
    pub active: u32,
}

impl Default for LayerState {
    fn default() -> Self {
        Self { active: 0 }
    }
}
