//! Tileset（精灵图）导入与加载。
//!
//! 职责：
//! - 通过文件对话框选择 tileset 图片，并复制到 workspace `assets/tilesets/`
//! - 触发 Bevy AssetServer 加载图片
//! - 图片加载完成后计算 columns/rows，并写入 `TilesetRuntime`

use bevy::prelude::*;
use blake3;

use super::paths::{path_join_asset, workspace_assets_dir};
use super::persistence::{load_tileset_library_from_file, save_tileset_library_to_file, DEFAULT_TILESET_LIBRARY_PATH};
use super::types::{
	EditorConfig, PendingTileset, TileEntities, TileMapData, TilesetEntry, TilesetLibrary,
	TilesetLoading, TilesetRuntime, TilesetRuntimeEntry, DEFAULT_SPRITESHEET, DEFAULT_LAYER_COUNT,
};

/// 启动时创建一张空地图（用于显示网格/承载绘制），不依赖 tileset。
pub fn setup_map(mut commands: Commands, config: Res<EditorConfig>) {
	let map = TileMapData::new(config.map_size.x, config.map_size.y);
	let layers = map.layers.max(DEFAULT_LAYER_COUNT);
	commands.insert_resource(map);
	let tiles = spawn_map_entities_with_layers(&mut commands, &config, layers);
	commands.insert_resource(tiles);
}

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

	open_tileset_impl(
		&asset_server,
		&mut config,
		&mut lib,
		&mut loading,
	);
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
				size.x, size.y, tile_w, tile_h
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

/// 生成地图格子实体（每格一个 Sprite），并返回 `TileEntities`。
///
/// 注意：函数本身不负责清理旧实体，调用方按需先 despawn。
pub fn spawn_map_entities(
	commands: &mut Commands,
	config: &EditorConfig,
) -> TileEntities {
	spawn_map_entities_with_layers(commands, config, DEFAULT_LAYER_COUNT)
}

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
