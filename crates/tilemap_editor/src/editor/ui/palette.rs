//! 左侧 tileset palette：缩略图网格 + 滚动 + 点击选 tile。

use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::{LEFT_PANEL_WIDTH_PX, TILE_BUTTON_PX, UI_BUTTON, UI_BUTTON_HOVER, UI_HIGHLIGHT};
use crate::editor::tileset::rect_for_tile_index;
use crate::editor::types::{
    EditorConfig, EditorState, PaletteRoot, PaletteScroll, PaletteTileButton, TilesetLibrary,
    TilesetRuntime, UiState,
};

/// 左侧 palette 的鼠标滚轮滚动。
///
/// Bevy UI 的滚动通过修改 `ScrollPosition` 完成；Node 默认就带有 `ScrollPosition` 组件。
pub fn palette_scroll_wheel(
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scroll_q: Query<&mut ScrollPosition, With<PaletteScroll>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    // 鼠标在左侧面板范围内才滚动（避免影响右侧缩放）
    if cursor.x > LEFT_PANEL_WIDTH_PX {
        return;
    }

    let mut delta_y: f32 = 0.0;
    for ev in wheel.read() {
        // ScrollPosition.y 增大 => 内容向上移动（视觉上向下滚动）
        // 为了符合常见体验：滚轮向下(负) => 视图向下 => ScrollPosition.y 增大
        delta_y += -ev.y;
    }
    if delta_y.abs() < f32::EPSILON {
        return;
    }

    let speed = 40.0;
    for mut scroll in scroll_q.iter_mut() {
        scroll.0.y = (scroll.0.y + delta_y * speed).max(0.0);
    }
}

/// tileset 加载完成后，动态生成左侧 palette（缩略图按钮网格）。
pub fn build_palette_when_ready(
    mut commands: Commands,
    config: Res<EditorConfig>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    mut ui_state: ResMut<UiState>,
    palette_q: Query<Entity, With<PaletteRoot>>,
    palette_children_q: Query<&Children>,
) {
    let Some(active_id) = lib.active_id.as_ref() else {
        return;
    };
    let Some(active) = runtime.by_id.get(active_id) else {
        return;
    };
    if ui_state.built_for_tileset_path == *active_id {
        return;
    }

    let Ok(palette_entity) = palette_q.single() else {
        return;
    };

    // 清理旧 palette（需要递归删除，否则子节点会残留）
    if let Ok(children) = palette_children_q.get(palette_entity) {
        for child in children.iter() {
            // button 的子节点是 ImageNode；这里手动删一层即可
            if let Ok(grandchildren) = palette_children_q.get(child) {
                for grandchild in grandchildren.iter() {
                    commands.entity(grandchild).despawn();
                }
            }
            commands.entity(child).despawn();
        }
    }

    let tile_count = active.columns.saturating_mul(active.rows);
    let image = active.texture.clone();
    let columns = active.columns.max(1);

    commands.entity(palette_entity).with_children(|p| {
        for index in 0..tile_count {
            let rect = rect_for_tile_index(index, columns, config.tile_size);

            p.spawn((
                Button,
                Node {
                    width: Val::Px(TILE_BUTTON_PX),
                    height: Val::Px(TILE_BUTTON_PX),
                    padding: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(UI_BUTTON),
                PaletteTileButton { index },
            ))
            .with_children(|p| {
                // ImageNode 支持 rect 裁剪，从同一张 tileset 中取出缩略图
                p.spawn((
                    ImageNode::new(image.clone())
                        .with_rect(rect)
                        .with_mode(NodeImageMode::Stretch),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
            });
        }
    });

    ui_state.built_for_tileset_path = active_id.clone();
}

/// palette 点击选择 tile。
pub fn palette_tile_click(
    mut state: ResMut<EditorState>,
    mut buttons_q: Query<(&Interaction, &PaletteTileButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    let selected_before = state.selected_tile;
    for (interaction, tile, mut bg) in buttons_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                state.selected_tile = tile.index;
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::Hovered => {
                if selected_before != tile.index {
                    *bg = BackgroundColor(UI_BUTTON_HOVER);
                }
            }
            Interaction::None => {
                if selected_before == tile.index {
                    *bg = BackgroundColor(UI_HIGHLIGHT);
                } else {
                    *bg = BackgroundColor(UI_BUTTON);
                }
            }
        }
    }
}
