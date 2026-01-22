//! 地图格子实体（TileEntities）生成。

use bevy::prelude::*;

use super::super::types::{EditorConfig, TileEntities};

/// 生成地图格子实体（每格每层一个 Sprite），并返回 `TileEntities`。
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
	let tile_w = config.tile_size.x as f32;
	let tile_h = config.tile_size.y as f32;

	let mut entities = Vec::with_capacity((width * height * layers) as usize);

	for layer in 0..layers {
		let z = layer as f32 * 0.1;
		for y in 0..height {
			for x in 0..width {
				let world_x = (x as f32 + 0.5) * tile_w;
				let world_y = (y as f32 + 0.5) * tile_h;

				// 初始隐藏，只有实际绘制后才显示
				let entity = commands
					.spawn((
						Sprite {
							image: Handle::<Image>::default(),
							rect: None,
							..default()
						},
						Transform::from_translation(Vec3::new(world_x, world_y, z)),
						Visibility::Hidden,
					))
					.id();

				entities.push(entity);
			}
		}
	}

	TileEntities {
		width,
		height,
		layers,
		entities,
	}
}
