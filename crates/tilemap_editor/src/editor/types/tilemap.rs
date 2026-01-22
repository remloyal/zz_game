use bevy::prelude::*;

#[derive(Resource)]
pub struct TileEntities {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub entities: Vec<Entity>,
}

impl TileEntities {
    pub fn layer_len(&self) -> usize {
        (self.width * self.height) as usize
    }

    pub fn idx_layer(&self, layer: u32, x: u32, y: u32) -> usize {
        (layer as usize) * self.layer_len() + (y * self.width + x) as usize
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
