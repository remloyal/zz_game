//! tileset 图片加载进度轮询与 runtime 构建。

use bevy::prelude::*;

use super::super::types::{EditorConfig, PendingTileset, TilesetLoading, TilesetRuntime, TilesetRuntimeEntry};

/// 轮询 tileset 图片是否加载完毕；加载完则写入 `TilesetRuntime`。
pub fn progress_spritesheet_loading(
	mut loading: ResMut<TilesetLoading>,
	mut runtime: ResMut<TilesetRuntime>,
	config: Res<EditorConfig>,
	images: Res<Assets<Image>>,
) {
	if loading.pending.is_empty() {
		return;
	}

	let mut still_pending: Vec<PendingTileset> = Vec::new();
	for p in loading.pending.drain(..) {
		if runtime.by_id.contains_key(&p.id) {
			continue;
		}
		let Some(image) = images.get(&p.texture) else {
			still_pending.push(p);
			continue;
		};

		let size = image.size();
		let tile_w = config.tile_size.x.max(1);
		let tile_h = config.tile_size.y.max(1);
		let columns = (size.x as u32) / tile_w;
		let rows = (size.y as u32) / tile_h;
		if columns == 0 || rows == 0 {
			warn!(
				"tileset size too small or tile_size invalid: image={}x{}, tile={}x{}",
				size.x,
				size.y,
				tile_w,
				tile_h
			);
			continue;
		}

		runtime.by_id.insert(
			p.id,
			TilesetRuntimeEntry {
				texture: p.texture,
				columns,
				rows,
			},
		);
	}

	loading.pending = still_pending;
}
