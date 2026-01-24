use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::{TileStorage, TilemapId};

use crate::editor::persistence::{load_map_from_file, save_map_to_file};
use crate::editor::tileset::{
    merge_tilesets_from_map, save_tileset_library, spawn_map_entities_with_layers,
};
use crate::editor::types::{
    EditorConfig, TileEntities, TileMapData, TilesetLibrary, TilesetLoading, TilesetRuntime,
    UndoStack,
};
use crate::editor::util::despawn_silently;

use super::rebuild_tilemaps;

/// 保存/读取快捷键：S / L。
pub fn save_load_shortcuts(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<EditorConfig>,
    asset_server: Res<AssetServer>,
    mut lib: ResMut<TilesetLibrary>,
    mut tileset_loading: ResMut<TilesetLoading>,
    runtime: Res<TilesetRuntime>,
    tile_entities: Option<ResMut<TileEntities>>,
    mut tile_storage_q: Query<&mut TileStorage>,
    tile_q: Query<Entity, With<TilemapId>>,
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
        let current_tile_entities = tile_entities;

        if needs_resize {
            if let Some(existing_tiles) = current_tile_entities.as_deref() {
                for e in existing_tiles.all_tilemap_entities() {
                    despawn_silently(&mut commands, e);
                }
            }
            for e in tile_q.iter() {
                despawn_silently(&mut commands, e);
            }

            config.map_size = UVec2::new(loaded.width, loaded.height);
            let tiles = spawn_map_entities_with_layers(&mut commands, &config, loaded.layers);
            commands.insert_resource(loaded.clone());
            let mut tiles = tiles;
            rebuild_tilemaps(
                &mut commands,
                &tile_q,
                &runtime,
                &loaded,
                &mut tiles,
                &mut tile_storage_q,
                &config,
            );
            commands.insert_resource(tiles);
            undo.clear();
            return;
        }

        commands.insert_resource(loaded.clone());
        if let Some(mut tile_entities) = current_tile_entities {
            rebuild_tilemaps(
                &mut commands,
                &tile_q,
                &runtime,
                &loaded,
                &mut tile_entities,
                &mut tile_storage_q,
                &config,
            );
        }
        undo.clear();
    }
}
