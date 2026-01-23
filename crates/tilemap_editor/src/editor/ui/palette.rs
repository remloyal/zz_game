//! 左侧 tileset palette：缩略图网格 + 滚动 + 点击选 tile。

use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::{LEFT_PANEL_WIDTH_PX, TILE_BUTTON_PX, UI_BUTTON, UI_BUTTON_HOVER, UI_HIGHLIGHT};
use crate::editor::tileset::rect_for_tile_index;
use crate::editor::types::{
    EditorConfig, EditorState, PaletteRoot, PaletteScroll, PaletteTileButton, TilesetLibrary,
    PaletteNextPageButton, PalettePageLabel, PalettePrevPageButton, TilesetRuntime, UiState,
};

fn page_count(tile_count: u32, page_size: u32) -> u32 {
    let page_size = page_size.max(1);
    (tile_count + page_size - 1) / page_size
}

fn clamp_page(page: u32, tile_count: u32, page_size: u32) -> u32 {
    let pc = page_count(tile_count, page_size).max(1);
    page.min(pc - 1)
}

/// palette 分页按钮：上一页/下一页。
pub fn palette_page_buttons(
    mut ui_state: ResMut<UiState>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    mut scroll_q: Query<&mut ScrollPosition, With<PaletteScroll>>,
    mut prev_q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<PalettePrevPageButton>, Without<PaletteNextPageButton>),
    >,
    mut next_q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<PaletteNextPageButton>, Without<PalettePrevPageButton>),
    >,
) {
    let Some(active_id) = lib.active_id.as_ref() else {
        return;
    };
    let Some(active) = runtime.by_id.get(active_id) else {
        return;
    };

    let tile_count = active.columns.saturating_mul(active.rows);
    let page_size = ui_state.palette_page_size().max(1);
    let max_page = clamp_page(u32::MAX, tile_count, page_size);
    ui_state.palette_page = ui_state.palette_page.min(max_page);

    let mut changed = false;

    for (interaction, mut bg) in prev_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
                let new_page = ui_state.palette_page.saturating_sub(1);
                if new_page != ui_state.palette_page {
                    ui_state.palette_page = new_page;
                    changed = true;
                }
            }
            Interaction::Hovered => *bg = BackgroundColor(UI_HIGHLIGHT),
            Interaction::None => *bg = BackgroundColor(UI_BUTTON),
        }
    }

    for (interaction, mut bg) in next_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
                let new_page = (ui_state.palette_page + 1).min(max_page);
                if new_page != ui_state.palette_page {
                    ui_state.palette_page = new_page;
                    changed = true;
                }
            }
            Interaction::Hovered => *bg = BackgroundColor(UI_HIGHLIGHT),
            Interaction::None => *bg = BackgroundColor(UI_BUTTON),
        }
    }

    if changed {
        // 强制 rebuild
        ui_state.built_for_tileset_path.clear();
        ui_state.built_palette_page = u32::MAX;
        // 翻页后滚动条回到顶部
        for mut scroll in scroll_q.iter_mut() {
            scroll.0.y = 0.0;
        }
    }
}

/// 页码标签同步：显示“cur/total”。
pub fn update_palette_page_label(
    ui_state: Res<UiState>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    mut q: Query<&mut Text, With<PalettePageLabel>>,
) {
    let Some(active_id) = lib.active_id.as_ref() else {
        for mut t in q.iter_mut() {
            *t = Text::new("-/-");
        }
        return;
    };
    let Some(active) = runtime.by_id.get(active_id) else {
        for mut t in q.iter_mut() {
            *t = Text::new("-/-");
        }
        return;
    };
    let tile_count = active.columns.saturating_mul(active.rows);
    let pc = page_count(tile_count, ui_state.palette_page_size()).max(1);
    let cur = ui_state.palette_page.min(pc - 1) + 1;
    let label = format!("{}/{}", cur, pc);
    for mut t in q.iter_mut() {
        *t = Text::new(label.clone());
    }
}

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
    // tileset 切换时重置页码
	if ui_state.built_for_tileset_path != *active_id {
		ui_state.palette_page = 0;
		ui_state.built_palette_page = u32::MAX;
	}

    if ui_state.built_for_tileset_path == *active_id && ui_state.built_palette_page == ui_state.palette_page {
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
    let page_size = ui_state.palette_page_size();
    let max_page = clamp_page(u32::MAX, tile_count, page_size);
    ui_state.palette_page = ui_state.palette_page.min(max_page);
    let start = ui_state.palette_page.saturating_mul(page_size);
    let end = (start + page_size).min(tile_count);

    commands.entity(palette_entity).with_children(|p| {
        for index in start..end {
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
	ui_state.built_palette_page = ui_state.palette_page;
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
