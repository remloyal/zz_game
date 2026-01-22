#![forbid(unsafe_code)]

#[cfg(feature = "bevy")]
use bevy::prelude::Resource;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// 稳定 tileset id。
///
/// 约定：使用导入图片内容的 hash（或至少是文件名+hash）生成，保证跨机器/拷贝时一致。
pub type TilesetId = String;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileRef {
    pub tileset_id: TilesetId,
    pub index: u32,
    /// 0,1,2,3 => 0/90/180/270 度顺时针。
    #[cfg_attr(feature = "serde", serde(default))]
    pub rot: u8,
    #[cfg_attr(feature = "serde", serde(default))]
    pub flip_x: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub flip_y: bool,
}

pub const DEFAULT_LAYER_COUNT: u32 = 2;

#[cfg_attr(feature = "bevy", derive(Resource))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct TileMapData {
    pub width: u32,
    pub height: u32,
    /// 图层数量。tiles 按 layer0..layerN 的顺序扁平存储。
    #[cfg_attr(feature = "serde", serde(default = "default_layers"))]
    pub layers: u32,
    pub tiles: Vec<Option<TileRef>>,
}

fn default_layers() -> u32 {
    1
}

impl TileMapData {
    pub fn new(width: u32, height: u32) -> Self {
        Self::new_with_layers(width, height, DEFAULT_LAYER_COUNT)
    }

    pub fn new_with_layers(width: u32, height: u32, layers: u32) -> Self {
        let layers = layers.max(1);
        Self {
            width,
            height,
            layers,
            tiles: vec![None; (width * height * layers) as usize],
        }
    }

    pub fn layer_len(&self) -> usize {
        (self.width * self.height) as usize
    }

    pub fn idx_layer(&self, layer: u32, x: u32, y: u32) -> usize {
        (layer as usize) * self.layer_len() + (y * self.width + x) as usize
    }

    /// 兼容旧调用：等价于 layer 0。
    pub fn idx(&self, x: u32, y: u32) -> usize {
        self.idx_layer(0, x, y)
    }

    /// 将地图升级为至少 `layers` 层：旧数据保持在 layer0，新层填 None。
    pub fn ensure_layers(&mut self, layers: u32) {
        let layers = layers.max(1);
        if self.layers >= layers {
            return;
        }
        let len = self.layer_len();
        self.tiles.resize(len * layers as usize, None);
        self.layers = layers;
    }

    pub fn topmost_layer_at(&self, x: u32, y: u32) -> Option<u32> {
        if self.layers == 0 {
            return None;
        }
        for layer in (0..self.layers).rev() {
            let idx = self.idx_layer(layer, x, y);
            if self.tiles.get(idx).and_then(|t| t.as_ref()).is_some() {
                return Some(layer);
            }
        }
        None
    }

    pub fn topmost_idx_at(&self, x: u32, y: u32) -> Option<usize> {
        let layer = self.topmost_layer_at(x, y)?;
        Some(self.idx_layer(layer, x, y))
    }
}
