//! 左侧工具栏 ActionButton 的点击处理（打开/新建/保存/读取/导入/导出等）。

use bevy::prelude::*;

use crate::editor::persistence::{load_map_from_file, save_map_to_file};
use crate::editor::tileset::{merge_tilesets_from_map, open_tileset_impl, save_tileset_library};
use crate::editor::types::{
    ActionButton, ActionKind, EditorConfig, TileEntities, TileMapData, TilesetLibrary, TilesetLoading,
    TilesetRuntime, UiState, UndoStack,
};
use crate::editor::world::apply_map_to_entities;
use crate::editor::{UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS};

use super::util::resized_map_copy;

/// 左侧工具栏按钮点击处理。
pub fn action_button_click(
    mut commands: Commands,
    mut action_q: Query<(&Interaction, &ActionButton, &mut BackgroundColor), Changed<Interaction>>,
    asset_server: Res<AssetServer>,
    mut config: ResMut<EditorConfig>,
    mut lib: ResMut<TilesetLibrary>,
    mut tileset_loading: ResMut<TilesetLoading>,
    runtime: Res<TilesetRuntime>,
    tile_entities: Option<Res<TileEntities>>,
    mut ui_state: ResMut<UiState>,
    mut sprite_vis_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    map: Option<ResMut<TileMapData>>,
    mut undo: ResMut<UndoStack>,
) {
    let mut requested: Option<ActionKind> = None;

    for (interaction, action, mut bg) in action_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_BUTTON_PRESS);
                requested = Some(action.0);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_BUTTON_HOVER);
            }
            Interaction::None => {
                *bg = BackgroundColor(UI_BUTTON);
            }
        }
    }

    let Some(requested) = requested else {
        return;
    };

    match requested {
        ActionKind::OpenTileset => {
            open_tileset_impl(&asset_server, &mut config, &mut lib, &mut tileset_loading);
            save_tileset_library(&lib);
            ui_state.built_for_tileset_path.clear();
        }
        ActionKind::SaveMap => {
            if let Some(map) = map.as_deref() {
                if let Err(err) = save_map_to_file(map, &lib, config.save_path.as_str()) {
                    warn!("save failed: {err}");
                } else {
                    info!("saved map: {}", config.save_path);
                }
            }
        }
        ActionKind::LoadMap => {
            let (loaded, tilesets) = match load_map_from_file(config.save_path.as_str()) {
                Ok(m) => m,
                Err(err) => {
                    warn!("load failed: {err}");
                    return;
                }
            };

            merge_tilesets_from_map(&asset_server, &mut lib, &mut tileset_loading, tilesets);
            save_tileset_library(&lib);
            ui_state.built_for_tileset_path.clear();
            undo.clear();

            // 尺寸变化：更新 config + 重建格子实体
            if config.map_size.x != loaded.width || config.map_size.y != loaded.height {
                if let Some(existing_tiles) = tile_entities.as_deref() {
                    for &e in &existing_tiles.entities {
                        commands.entity(e).despawn();
                    }
                }
                commands.remove_resource::<TileEntities>();
                commands.remove_resource::<TileMapData>();

                config.map_size = UVec2::new(loaded.width, loaded.height);
                let tiles = crate::editor::tileset::spawn_map_entities_with_layers(&mut commands, &config, loaded.layers);
                commands.insert_resource(loaded.clone());
                apply_map_to_entities(&runtime, &loaded, &tiles, &mut sprite_vis_q, &config);
                commands.insert_resource(tiles);
                return;
            }

            commands.insert_resource(loaded.clone());
            if let Some(tile_entities) = tile_entities.as_deref() {
                apply_map_to_entities(&runtime, &loaded, tile_entities, &mut sprite_vis_q, &config);
            }
        }
        ActionKind::NewMap => {
            if let (Some(tile_entities), Some(mut map)) = (tile_entities.as_deref(), map) {
                *map = TileMapData::new(map.width, map.height);
                apply_map_to_entities(&runtime, &map, tile_entities, &mut sprite_vis_q, &config);
            }
        }
        ActionKind::SetMapSize { width, height } => {
            if width == 0 || height == 0 {
                return;
            }

            let old_map = map.as_deref();
            let new_map = resized_map_copy(old_map, width, height);

            config.map_size = UVec2::new(width, height);
            commands.insert_resource(new_map.clone());
            undo.clear();

            // 重建格子实体
            if let Some(existing_tiles) = tile_entities.as_deref() {
                for &e in &existing_tiles.entities {
                    commands.entity(e).despawn();
                }
            }
            commands.remove_resource::<TileEntities>();
            let tiles = crate::editor::tileset::spawn_map_entities_with_layers(&mut commands, &config, new_map.layers);
            apply_map_to_entities(&runtime, &new_map, &tiles, &mut sprite_vis_q, &config);
            commands.insert_resource(tiles);
        }
        ActionKind::ImportMap => {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("RON", &["ron"])
                .pick_file()
            else {
                return;
            };

            let (loaded, tilesets) = match load_map_from_file(path.to_string_lossy().as_ref()) {
                Ok(m) => m,
                Err(err) => {
                    warn!("import failed: {err}");
                    return;
                }
            };
            merge_tilesets_from_map(&asset_server, &mut lib, &mut tileset_loading, tilesets);
            save_tileset_library(&lib);
            ui_state.built_for_tileset_path.clear();
            undo.clear();

            // 尺寸变化：更新 config + 重建格子实体
            if config.map_size.x != loaded.width || config.map_size.y != loaded.height {
                if let Some(existing_tiles) = tile_entities.as_deref() {
                    for &e in &existing_tiles.entities {
                        commands.entity(e).despawn();
                    }
                }
                commands.remove_resource::<TileEntities>();
                commands.remove_resource::<TileMapData>();

                config.map_size = UVec2::new(loaded.width, loaded.height);
                let tiles = crate::editor::tileset::spawn_map_entities_with_layers(&mut commands, &config, loaded.layers);
                commands.insert_resource(loaded.clone());
                apply_map_to_entities(&runtime, &loaded, &tiles, &mut sprite_vis_q, &config);
                commands.insert_resource(tiles);
                return;
            }

            // 更新地图数据（下一帧由绘制系统继续使用）
            commands.insert_resource(loaded.clone());

            // 若当前系统参数里已有 tile_entities，则本帧直接刷新可见性
            if let Some(tile_entities) = tile_entities.as_deref() {
                apply_map_to_entities(&runtime, &loaded, tile_entities, &mut sprite_vis_q, &config);
            }
        }
        ActionKind::ExportMap => {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("RON", &["ron"])
                .set_file_name("map.ron")
                .save_file()
            else {
                return;
            };

            let Some(map) = map.as_deref() else {
                return;
            };

            if let Err(err) = save_map_to_file(map, &lib, path.to_string_lossy().as_ref()) {
                warn!("export failed: {err}");
            } else {
                info!("exported map: {}", path.to_string_lossy());
            }
        }
    }
}
