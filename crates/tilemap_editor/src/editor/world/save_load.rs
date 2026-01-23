use bevy::prelude::*;

use crate::editor::persistence::{load_map_from_file, save_map_to_file};
use crate::editor::tileset::{
    merge_tilesets_from_map, save_tileset_library, spawn_map_entities_with_layers,
};
use crate::editor::types::{
    EditorConfig, TileEntities, TileMapData, TilesetLibrary, TilesetLoading, TilesetRuntime,
    UndoStack,
};
use crate::editor::util::despawn_silently;

use super::apply_map_to_entities;

/// 保存/读取快捷键：S / L。
pub fn save_load_shortcuts(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<EditorConfig>,
    asset_server: Res<AssetServer>,
    mut lib: ResMut<TilesetLibrary>,
    mut tileset_loading: ResMut<TilesetLoading>,
    runtime: Res<TilesetRuntime>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    map: Option<ResMut<TileMapData>>,
    mut undo: ResMut<UndoStack>,
) {
    if keys.just_pressed(KeyCode::KeyS) {
        let Some(map) = map.as_deref() else {
            return;
        };
        if let Err(err) = save_map_to_file(map, &lib, config.save_path.as_str()) {
            warn!("save failed: {err}");
        } else {
            info!("saved map: {}", config.save_path);
        }
    }

    if keys.just_pressed(KeyCode::KeyL) {
        let (loaded, tilesets) = match load_map_from_file(config.save_path.as_str()) {
            Ok(m) => m,
            Err(err) => {
                warn!("load failed: {err}");
                return;
            }
        };

        merge_tilesets_from_map(&asset_server, &mut lib, &mut tileset_loading, tilesets);
        save_tileset_library(&lib);

        let needs_resize = config.map_size.x != loaded.width || config.map_size.y != loaded.height;
        let current_tile_entities = tile_entities.as_deref();

        if needs_resize {
            if let Some(existing_tiles) = current_tile_entities {
                for &e in &existing_tiles.entities {
                    despawn_silently(&mut commands, e);
                }
            }
            commands.remove_resource::<TileEntities>();
            commands.remove_resource::<TileMapData>();

            config.map_size = UVec2::new(loaded.width, loaded.height);
            let tiles = spawn_map_entities_with_layers(&mut commands, &config, loaded.layers);
            commands.insert_resource(loaded.clone());
            apply_map_to_entities(&runtime, &loaded, &tiles, &mut tiles_q, &config);
            commands.insert_resource(tiles);
            undo.clear();
            return;
        }

        commands.insert_resource(loaded.clone());
        if let Some(tile_entities) = tile_entities.as_deref() {
            apply_map_to_entities(&runtime, &loaded, tile_entities, &mut tiles_q, &config);
        }
        undo.clear();
    }
}
