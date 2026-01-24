//! 左侧工具栏 ActionButton 的点击处理（打开/新建/保存/读取/导入/导出等）。

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::TilemapId;

use crate::editor::persistence::{load_map_from_file, save_map_to_file};
use crate::editor::tileset::{merge_tilesets_from_map, open_tileset_impl, save_tileset_library};
use crate::editor::types::{
    ActionButton, ActionKind, EditorConfig, TileMapData, TilesetLibrary, TilesetLoading,
    ShiftMapMode, ShiftMapSettings, UiState, UndoStack,
};
use crate::editor::util::despawn_silently;
use crate::editor::world::{apply_tile_change, rebuild_tilemaps, TilemapRenderParams};
use crate::editor::{UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS};

use super::util::resized_map_copy;

/// 左侧工具栏按钮点击处理。
pub fn action_button_click(
    mut render: TilemapRenderParams,
    mut action_q: Query<(&Interaction, &ActionButton, &mut BackgroundColor), Changed<Interaction>>,
    asset_server: Res<AssetServer>,
    mut config: ResMut<EditorConfig>,
    mut lib: ResMut<TilesetLibrary>,
    mut tileset_loading: ResMut<TilesetLoading>,
    mut ui_state: ResMut<UiState>,
    mut shift: ResMut<ShiftMapSettings>,
    tile_q: Query<Entity, With<TilemapId>>,
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
        ActionKind::Undo => {
            let Some(mut map) = map else {
                return;
            };
            let Some(cmd) = undo.undo.pop() else {
                return;
            };
            let layer_len = map.layer_len();
            for ch in &cmd.changes {
                if ch.idx < map.tiles.len() {
                    map.tiles[ch.idx] = ch.before.clone();
                    let layer = (ch.idx / layer_len) as u32;
                    let local = ch.idx % layer_len;
                    let x = (local % map.width as usize) as u32;
                    let y = (local / map.width as usize) as u32;
                    apply_tile_change(&mut render, &config, layer, x, y, &ch.after, &ch.before);
                }
            }
            undo.redo.push(cmd);
        }
        ActionKind::Redo => {
            let Some(mut map) = map else {
                return;
            };
            let Some(cmd) = undo.redo.pop() else {
                return;
            };
            let layer_len = map.layer_len();
            for ch in &cmd.changes {
                if ch.idx < map.tiles.len() {
                    map.tiles[ch.idx] = ch.after.clone();
                    let layer = (ch.idx / layer_len) as u32;
                    let local = ch.idx % layer_len;
                    let x = (local % map.width as usize) as u32;
                    let y = (local / map.width as usize) as u32;
                    apply_tile_change(&mut render, &config, layer, x, y, &ch.before, &ch.after);
                }
            }
            undo.undo.push(cmd);
        }
        ActionKind::ToggleGrid => {
            config.show_grid = !config.show_grid;
        }
        ActionKind::ToggleHover => {
            config.show_hover = !config.show_hover;
        }
        ActionKind::ToggleCursor => {
            config.show_cursor = !config.show_cursor;
        }
        ActionKind::ToggleShiftMode => {
            shift.mode = match shift.mode {
                ShiftMapMode::Blank => ShiftMapMode::Wrap,
                ShiftMapMode::Wrap => ShiftMapMode::Blank,
            };
        }
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
                if let Some(existing_tiles) = render.tile_entities.as_deref() {
                    for e in existing_tiles.all_tilemap_entities() {
                        despawn_silently(&mut render.commands, e);
                    }
                }
                for e in tile_q.iter() {
                    despawn_silently(&mut render.commands, e);
                }

                config.map_size = UVec2::new(loaded.width, loaded.height);
                let mut tiles = crate::editor::tileset::spawn_map_entities_with_layers(
                    &mut render.commands,
                    &config,
                    loaded.layers,
                );
                render.commands.insert_resource(loaded.clone());
                {
                    let TilemapRenderParams {
                        commands,
                        tile_entities: _,
                        runtime,
                        tile_storage_q,
                        ..
                    } = &mut render;
                    rebuild_tilemaps(
                        commands,
                        &tile_q,
                        runtime,
                        &loaded,
                        &mut tiles,
                        tile_storage_q,
                        &config,
                    );
                }
                render.commands.insert_resource(tiles);
                return;
            }

            render.commands.insert_resource(loaded.clone());
            {
                let TilemapRenderParams {
                    commands,
                    tile_entities,
                    runtime,
                    tile_storage_q,
                    ..
                } = &mut render;
                if let Some(tile_entities) = tile_entities.as_mut() {
                    rebuild_tilemaps(
                        commands,
                        &tile_q,
                        runtime,
                        &loaded,
                        &mut *tile_entities,
                        tile_storage_q,
                        &config,
                    );
                }
            }
        }
        ActionKind::NewMap => {
            if let Some(mut map) = map {
                *map = TileMapData::new(map.width, map.height);
                let TilemapRenderParams {
                    commands,
                    tile_entities,
                    runtime,
                    tile_storage_q,
                    ..
                } = &mut render;
                if let Some(tile_entities) = tile_entities.as_mut() {
                    rebuild_tilemaps(
                        commands,
                        &tile_q,
                        runtime,
                        &map,
                        &mut *tile_entities,
                        tile_storage_q,
                        &config,
                    );
                }
            }
        }
        ActionKind::SetMapSize { width, height } => {
            if width == 0 || height == 0 {
                return;
            }

            let old_map = map.as_deref();
            let new_map = resized_map_copy(old_map, width, height);

            config.map_size = UVec2::new(width, height);
            render.commands.insert_resource(new_map.clone());
            undo.clear();

            // 重建格子实体
            if let Some(existing_tiles) = render.tile_entities.as_deref() {
                for e in existing_tiles.all_tilemap_entities() {
                    despawn_silently(&mut render.commands, e);
                }
            }
            for e in tile_q.iter() {
                despawn_silently(&mut render.commands, e);
            }
            let mut tiles = crate::editor::tileset::spawn_map_entities_with_layers(
                &mut render.commands,
                &config,
                new_map.layers,
            );
            {
                let TilemapRenderParams {
                    commands,
                    tile_entities: _,
                    runtime,
                    tile_storage_q,
                    ..
                } = &mut render;
                rebuild_tilemaps(
                    commands,
                    &tile_q,
                    runtime,
                    &new_map,
                    &mut tiles,
                    tile_storage_q,
                    &config,
                );
            }
            render.commands.insert_resource(tiles);
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
                if let Some(existing_tiles) = render.tile_entities.as_deref() {
                    for e in existing_tiles.all_tilemap_entities() {
                        despawn_silently(&mut render.commands, e);
                    }
                }
                for e in tile_q.iter() {
                    despawn_silently(&mut render.commands, e);
                }

                config.map_size = UVec2::new(loaded.width, loaded.height);
                let mut tiles = crate::editor::tileset::spawn_map_entities_with_layers(
                    &mut render.commands,
                    &config,
                    loaded.layers,
                );
                render.commands.insert_resource(loaded.clone());
                {
                    let TilemapRenderParams {
                        commands,
                        tile_entities: _,
                        runtime,
                        tile_storage_q,
                        ..
                    } = &mut render;
                    rebuild_tilemaps(
                        commands,
                        &tile_q,
                        runtime,
                        &loaded,
                        &mut tiles,
                        tile_storage_q,
                        &config,
                    );
                }
                render.commands.insert_resource(tiles);
                return;
            }

            // 更新地图数据（下一帧由绘制系统继续使用）
            render.commands.insert_resource(loaded.clone());
            {
                let TilemapRenderParams {
                    commands,
                    tile_entities,
                    runtime,
                    tile_storage_q,
                    ..
                } = &mut render;
                if let Some(tile_entities) = tile_entities.as_mut() {
                    rebuild_tilemaps(
                        commands,
                        &tile_q,
                        runtime,
                        &loaded,
                        &mut *tile_entities,
                        tile_storage_q,
                        &config,
                    );
                }
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
