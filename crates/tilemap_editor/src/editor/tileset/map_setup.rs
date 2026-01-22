//! 启动时创建空地图与格子实体。

use bevy::prelude::*;

use super::spawn::spawn_map_entities_with_layers;
use super::super::types::{EditorConfig, TileMapData, DEFAULT_LAYER_COUNT};

/// 启动时创建一张空地图（用于显示网格/承载绘制），不依赖 tileset。
pub fn setup_map(mut commands: Commands, config: Res<EditorConfig>) {
	let map = TileMapData::new(config.map_size.x, config.map_size.y);
	let layers = map.layers.max(DEFAULT_LAYER_COUNT);
	commands.insert_resource(map);
	let tiles = spawn_map_entities_with_layers(&mut commands, &config, layers);
	commands.insert_resource(tiles);
}
