//! 地图尺寸输入控件：交互、键盘输入、应用尺寸变更。

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::TilemapId;

use crate::editor::{UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS, UI_HIGHLIGHT};
use crate::editor::types::{
    EditorConfig, MapSizeApplyButton, MapSizeFocus, MapSizeHeightField, MapSizeHeightText,
    MapSizeInput, MapSizeWidthField, MapSizeWidthText, TileMapData, UndoStack,
};
use crate::editor::world::{rebuild_tilemaps, TilemapRenderParams};

use super::util::resized_map_copy;
use crate::editor::util::despawn_silently;

pub fn map_size_widget_interactions(
    mut input: ResMut<MapSizeInput>,
    mut width_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<MapSizeWidthField>,
            Without<MapSizeHeightField>,
            Without<MapSizeApplyButton>,
        ),
    >,
    mut height_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<MapSizeHeightField>,
            Without<MapSizeWidthField>,
            Without<MapSizeApplyButton>,
        ),
    >,
    mut apply_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<MapSizeApplyButton>,
            Without<MapSizeWidthField>,
            Without<MapSizeHeightField>,
        ),
    >,
) {
    for (interaction, mut bg) in width_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                input.focus = MapSizeFocus::Width;
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::Hovered => {
                if input.focus == MapSizeFocus::Width {
                    *bg = BackgroundColor(UI_HIGHLIGHT);
                } else {
                    *bg = BackgroundColor(UI_BUTTON_HOVER);
                }
            }
            Interaction::None => {
                if input.focus == MapSizeFocus::Width {
                    *bg = BackgroundColor(UI_HIGHLIGHT);
                } else {
                    *bg = BackgroundColor(UI_BUTTON);
                }
            }
        }
    }

    for (interaction, mut bg) in height_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                input.focus = MapSizeFocus::Height;
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::Hovered => {
                if input.focus == MapSizeFocus::Height {
                    *bg = BackgroundColor(UI_HIGHLIGHT);
                } else {
                    *bg = BackgroundColor(UI_BUTTON_HOVER);
                }
            }
            Interaction::None => {
                if input.focus == MapSizeFocus::Height {
                    *bg = BackgroundColor(UI_HIGHLIGHT);
                } else {
                    *bg = BackgroundColor(UI_BUTTON);
                }
            }
        }
    }

    for (interaction, mut bg) in apply_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_BUTTON_PRESS);
                input.apply_requested = true;
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_BUTTON_HOVER);
            }
            Interaction::None => {
                *bg = BackgroundColor(UI_BUTTON);
            }
        }
    }
}

pub fn map_size_text_input(keys: Res<ButtonInput<KeyCode>>, mut input: ResMut<MapSizeInput>) {
    if input.focus == MapSizeFocus::None {
        return;
    }

    let buf = match input.focus {
        MapSizeFocus::Width => &mut input.width_buf,
        MapSizeFocus::Height => &mut input.height_buf,
        MapSizeFocus::None => return,
    };

    // Bevy 0.18 在不同平台/后端下字符输入事件类型可能不同；
    // 这里用 KeyCode 录入数字（主键盘 0-9 + 小键盘 0-9）确保稳定。
    let digit_keys: &[(KeyCode, char)] = &[
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Numpad0, '0'),
        (KeyCode::Numpad1, '1'),
        (KeyCode::Numpad2, '2'),
        (KeyCode::Numpad3, '3'),
        (KeyCode::Numpad4, '4'),
        (KeyCode::Numpad5, '5'),
        (KeyCode::Numpad6, '6'),
        (KeyCode::Numpad7, '7'),
        (KeyCode::Numpad8, '8'),
        (KeyCode::Numpad9, '9'),
    ];

    for (key, ch) in digit_keys {
        if keys.just_pressed(*key) {
            if buf.len() < 5 {
                buf.push(*ch);
            }
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        buf.pop();
    }

    if keys.just_pressed(KeyCode::Escape) {
        input.focus = MapSizeFocus::None;
    }

    if keys.just_pressed(KeyCode::Enter) {
        input.apply_requested = true;
        input.focus = MapSizeFocus::None;
    }
}

pub fn sync_map_size_input_from_config(config: Res<EditorConfig>, mut input: ResMut<MapSizeInput>) {
    // 不在编辑时，跟随当前 config（导入/预设/其它系统修改 map_size 时也能同步到输入框）
    if input.focus != MapSizeFocus::None {
        return;
    }

    let w = config.map_size.x.to_string();
    let h = config.map_size.y.to_string();
    if input.width_buf != w {
        input.width_buf = w;
    }
    if input.height_buf != h {
        input.height_buf = h;
    }
}

pub fn update_map_size_field_text(
    input: Res<MapSizeInput>,
    mut w_q: Query<&mut Text, (With<MapSizeWidthText>, Without<MapSizeHeightText>)>,
    mut h_q: Query<&mut Text, (With<MapSizeHeightText>, Without<MapSizeWidthText>)>,
) {
    let w = if input.focus == MapSizeFocus::Width {
        format!(" {}|", input.width_buf)
    } else {
        format!(" {}", input.width_buf)
    };
    let h = if input.focus == MapSizeFocus::Height {
        format!(" {}|", input.height_buf)
    } else {
        format!(" {}", input.height_buf)
    };

    for mut t in w_q.iter_mut() {
        *t = Text::new(w.clone());
    }
    for mut t in h_q.iter_mut() {
        *t = Text::new(h.clone());
    }
}

pub fn apply_custom_map_size(
    mut render: TilemapRenderParams,
    mut input: ResMut<MapSizeInput>,
    mut config: ResMut<EditorConfig>,
    tile_q: Query<Entity, With<TilemapId>>,
    map: Option<ResMut<TileMapData>>,
    mut undo: ResMut<UndoStack>,
) {
    if !input.apply_requested {
        return;
    }
    input.apply_requested = false;

    let Ok(width) = input.width_buf.parse::<u32>() else {
        return;
    };
    let Ok(height) = input.height_buf.parse::<u32>() else {
        return;
    };
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
