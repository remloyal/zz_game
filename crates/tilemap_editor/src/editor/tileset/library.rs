//! tileset 库（TilesetLibrary）的读写、合并。 

use bevy::prelude::*;

use super::super::paths::workspace_assets_dir;
use super::super::persistence::{
	load_tileset_library_from_file, save_tileset_library_to_file, DEFAULT_TILESET_LIBRARY_PATH,
};
use super::super::types::{PendingTileset, TilesetEntry, TilesetLibrary, TilesetLoading};

/// 启动时读取 tileset 库缓存，并预加载所有 tileset（用于回显渲染）。
pub fn load_tileset_library_startup(
	asset_server: Res<AssetServer>,
	mut lib: ResMut<TilesetLibrary>,
	mut loading: ResMut<TilesetLoading>,
) {
	let abs = workspace_assets_dir().join(DEFAULT_TILESET_LIBRARY_PATH);
	match load_tileset_library_from_file(abs.to_string_lossy().as_ref()) {
		Ok(loaded) => {
			*lib = loaded;
		}
		Err(err) => {
			warn!("failed to load tileset library: {err}");
		}
	}

	// 预加载库中所有 tileset
	for e in &lib.entries {
		if e.asset_path.is_empty() {
			continue;
		}
		let tex: Handle<Image> = asset_server.load(e.asset_path.clone());
		loading.pending.push(PendingTileset {
			id: e.id.clone(),
			texture: tex,
		});
	}

	// 没有选中时，默认选第一个
	if lib.active_id.is_none() {
		lib.active_id = lib.entries.first().map(|e| e.id.clone());
	}
}

/// 将 tileset 库写回缓存文件。
pub fn save_tileset_library(lib: &TilesetLibrary) {
	let abs = workspace_assets_dir().join(DEFAULT_TILESET_LIBRARY_PATH);
	if let Err(err) = save_tileset_library_to_file(lib, abs.to_string_lossy().as_ref()) {
		warn!("failed to save tileset library: {err}");
	}
}

/// 从地图文件携带的 tileset 列表合并到本地库，并触发加载（用于“别人拷贝 assets + map.ron 也能回显”）。
pub fn merge_tilesets_from_map(
	asset_server: &AssetServer,
	lib: &mut TilesetLibrary,
	loading: &mut TilesetLoading,
	tilesets: Vec<TilesetEntry>,
) {
	for incoming in tilesets {
		if incoming.id.trim().is_empty() {
			continue;
		}

		match lib.entries.iter_mut().find(|e| e.id == incoming.id) {
			Some(existing) => {
				if existing.asset_path.is_empty() && !incoming.asset_path.is_empty() {
					existing.asset_path = incoming.asset_path.clone();
				}
				if existing.name.trim().is_empty() && !incoming.name.trim().is_empty() {
					existing.name = incoming.name.clone();
				}
				if existing.category.trim().is_empty() && !incoming.category.trim().is_empty() {
					existing.category = incoming.category.clone();
				}
			}
			None => {
				lib.entries.push(incoming.clone());
			}
		}

		if lib.active_id.is_none() {
			lib.active_id = Some(incoming.id.clone());
		}

		if !incoming.asset_path.is_empty() {
			let tex: Handle<Image> = asset_server.load(incoming.asset_path);
			loading.pending.push(PendingTileset {
				id: incoming.id,
				texture: tex,
			});
		}
	}
}
