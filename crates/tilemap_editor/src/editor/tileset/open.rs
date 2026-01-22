//! tileset 导入（文件对话框选择 → 复制到 assets → 触发加载）。

use bevy::prelude::*;
use blake3;

use super::super::paths::{path_join_asset, workspace_assets_dir};
use super::super::types::{
	EditorConfig, PendingTileset, TilesetEntry, TilesetLibrary, TilesetLoading, DEFAULT_SPRITESHEET,
};

/// 按 `O` 快捷键打开 tileset。
pub fn open_spritesheet_shortcut(
	_commands: Commands,
	keys: Res<ButtonInput<KeyCode>>,
	asset_server: Res<AssetServer>,
	mut config: ResMut<EditorConfig>,
	mut lib: ResMut<TilesetLibrary>,
	mut loading: ResMut<TilesetLoading>,
) {
	if !keys.just_pressed(KeyCode::KeyO) {
		return;
	}

	open_tileset_impl(&asset_server, &mut config, &mut lib, &mut loading);
}

/// 选择 tileset → 复制到 `assets/tilesets/` → 加载。
///
/// 这里做“复制到 assets”的原因：Bevy 的 AssetServer 默认只读 `AssetPlugin.file_path` 指向的目录。
pub fn open_tileset_impl(
	asset_server: &AssetServer,
	config: &mut EditorConfig,
	lib: &mut TilesetLibrary,
	loading: &mut TilesetLoading,
) {
	let Some(picked) = rfd::FileDialog::new()
		.add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
		.pick_file()
	else {
		return;
	};

	let asset_root = workspace_assets_dir();
	let import_dir = asset_root.join(&config.tileset_import_dir);
	if let Err(err) = std::fs::create_dir_all(&import_dir) {
		warn!("failed to create import dir: {err}");
		return;
	}

	let file_name = picked
		.file_name()
		.map(|s| s.to_string_lossy().to_string())
		.unwrap_or_else(|| DEFAULT_SPRITESHEET.to_string());
	let dest_abs = import_dir.join(&file_name);
	if let Err(err) = std::fs::copy(&picked, &dest_abs) {
		warn!("failed to copy tileset: {err}");
		return;
	}

	let bytes = match std::fs::read(&dest_abs) {
		Ok(b) => b,
		Err(err) => {
			warn!("failed to read tileset for hashing: {err}");
			return;
		}
	};
	let id = blake3::hash(&bytes).to_hex().to_string();

	let rel = path_join_asset(&config.tileset_import_dir, &file_name);
	info!("imported tileset: id={id} path={rel}");

	let name = dest_abs
		.file_stem()
		.map(|s| s.to_string_lossy().to_string())
		.unwrap_or_else(|| file_name.clone());

	if let Some(existing) = lib.entries.iter_mut().find(|e| e.id == id) {
		existing.asset_path = rel.clone();
		if existing.name.trim().is_empty() {
			existing.name = name.clone();
		}
	} else {
		lib.entries.push(TilesetEntry {
			id: id.clone(),
			name,
			category: "default".to_string(),
			asset_path: rel.clone(),
		});
	}
	lib.active_id = Some(id.clone());

	let texture: Handle<Image> = asset_server.load(rel);
	loading.pending.push(PendingTileset { id, texture });
}
