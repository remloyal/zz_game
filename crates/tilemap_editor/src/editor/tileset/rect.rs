//! tileset 纹理裁剪相关 helper。

use bevy::prelude::*;

/// 计算某个 tile index 在 tileset 纹理中的裁剪 Rect。
///
/// 注意：Bevy 的 `Rect` 原点在纹理左下角。
pub fn rect_for_tile_index(index: u32, columns: u32, tile_size: UVec2) -> Rect {
	let tile_w = tile_size.x as f32;
	let tile_h = tile_size.y as f32;
	let col = index % columns;
	let row = index / columns;

	let min = Vec2::new(col as f32 * tile_w, row as f32 * tile_h);
	let max = Vec2::new(min.x + tile_w, min.y + tile_h);
	Rect { min, max }
}
