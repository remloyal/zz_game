#![forbid(unsafe_code)]

// 纯逻辑 crate：面向 tilemap 数据结构与编辑算法。
//
// 当前仅提供骨架，后续会逐步从 tilemap_editor 迁移：
// - 多图层 TileMap
// - 工具算法（rect/fill/shift）
// - Undo/Redo（命令栈）

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub type TilesetId = String;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileRef {
    pub tileset_id: TilesetId,
    pub index: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerId {
    Ground,
    Upper,
    Shadow,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TileLayer {
    pub id: LayerId,
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Option<TileRef>>,
}

impl TileLayer {
    pub fn new(id: LayerId, width: u32, height: u32) -> Self {
        Self {
            id,
            width,
            height,
            tiles: vec![None; (width * height) as usize],
        }
    }

    #[inline]
    pub fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct TileMap {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<TileLayer>,
}

impl TileMap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            layers: vec![
                TileLayer::new(LayerId::Ground, width, height),
                TileLayer::new(LayerId::Upper, width, height),
            ],
        }
    }
}
