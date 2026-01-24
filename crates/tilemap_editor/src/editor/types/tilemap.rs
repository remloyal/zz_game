use bevy::prelude::*;
use std::collections::HashMap;

use tilemap_core::TilesetId;

/// 地图渲染实体索引（bevy_ecs_tilemap 后端）。
#[derive(Resource)]
pub struct TileEntities {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    /// tileset_id -> 每层 tilemap entity（len = layers）
    pub tilemaps: HashMap<TilesetId, Vec<Entity>>,
    /// tileset 创建顺序（用于稳定 z 排序）
    pub tileset_order: Vec<TilesetId>,
}

impl TileEntities {
    pub fn layer_entity(&self, tileset_id: &TilesetId, layer: u32) -> Option<Entity> {
        let list = self.tilemaps.get(tileset_id)?;
        list.get(layer as usize).copied()
    }

    pub fn set_layer_entity(&mut self, tileset_id: TilesetId, layer: u32, entity: Entity) {
        let layers = self.layers.max(1) as usize;
        let entry = self.tilemaps.entry(tileset_id).or_insert_with(|| vec![Entity::PLACEHOLDER; layers]);
        if entry.len() < layers {
            entry.resize(layers, Entity::PLACEHOLDER);
        }
        if (layer as usize) < entry.len() {
            entry[layer as usize] = entity;
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
            for &e in layers {
                if e != Entity::PLACEHOLDER {
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
