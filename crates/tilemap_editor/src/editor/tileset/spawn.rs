//! 地图格子实体（TileEntities）生成。

use bevy::prelude::*;

use super::super::types::EditorConfig;
use super::super::types::{TileEntities, DEFAULT_CHUNK_SIZE};

/// 初始化地图渲染容器（bevy_ecs_tilemap 后端）。
///
/// 注意：函数本身不负责清理旧实体，调用方按需先 despawn。
pub fn spawn_map_entities_with_layers(
	commands: &mut Commands,
	config: &EditorConfig,
	layers: u32,
) -> TileEntities {
	let width = config.map_size.x;
	let height = config.map_size.y;
	let layers = layers.max(1);
	let _ = commands; // tilemap 实体由渲染同步系统按需创建
	TileEntities {
		width,
		height,
		layers,
		chunk_size: DEFAULT_CHUNK_SIZE,
		tilemaps: Default::default(),
		tileset_order: Default::default(),
	}
}
