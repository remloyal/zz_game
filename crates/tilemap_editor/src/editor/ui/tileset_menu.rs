//! Tileset 选择栏与下拉菜单（库/分类/当前 tileset 显示）。

use bevy::prelude::*;

use crate::editor::{UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS, UI_HIGHLIGHT};
use crate::editor::tileset::save_tileset_library;
use crate::editor::types::{
    EditorState, TilesetActiveLabel, TilesetCategoryCycleButton, TilesetCategoryLabel, TilesetLibrary,
    TilesetMenuRoot, TilesetRuntime, TilesetSelectItem, TilesetToggleButton, UiState,
};

/// tileset 选择栏：更新当前选中 tileset 的显示文本。
pub fn update_tileset_active_label(lib: Res<TilesetLibrary>, mut label_q: Query<&mut Text, With<TilesetActiveLabel>>) {
    if !lib.is_changed() {
        return;
    }

    let label = match lib.active_id.as_ref() {
        Some(id) => lib
            .entries
            .iter()
            .find(|e| &e.id == id)
            .map(|e| {
                if !e.name.trim().is_empty() {
                    e.name.clone()
                } else if !e.asset_path.trim().is_empty() {
                    e.asset_path.clone()
                } else {
                    id.clone()
                }
            })
            .unwrap_or_else(|| id.clone()),
        None => "(未选择)".to_string(),
    };

    for mut t in label_q.iter_mut() {
        *t = Text::new(label.clone());
    }
}

/// tileset 分类标签更新。
pub fn update_tileset_category_label(lib: Res<TilesetLibrary>, mut label_q: Query<&mut Text, With<TilesetCategoryLabel>>) {
    if !lib.is_changed() {
        return;
    }

    let label = if lib.active_category.trim().is_empty() {
        "全部".to_string()
    } else {
        lib.active_category.clone()
    };
    for mut t in label_q.iter_mut() {
        *t = Text::new(label.clone());
    }
}

/// tileset 菜单开关按钮。
pub fn tileset_toggle_button_click(
    mut ui_state: ResMut<UiState>,
    mut q: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<TilesetToggleButton>)>,
) {
    for (interaction, mut bg) in q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                ui_state.tileset_menu_open = !ui_state.tileset_menu_open;
                *bg = BackgroundColor(UI_BUTTON_PRESS);
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

/// 分类切换：循环切换当前分类过滤（全部 -> 各分类 -> 全部）。
pub fn tileset_category_cycle_click(
    mut lib: ResMut<TilesetLibrary>,
    mut ui_state: ResMut<UiState>,
    mut q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<TilesetCategoryCycleButton>),
    >,
) {
    let mut clicked = false;
    for (interaction, mut bg) in q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                clicked = true;
                *bg = BackgroundColor(UI_BUTTON_PRESS);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_BUTTON_HOVER);
            }
            Interaction::None => {
                *bg = BackgroundColor(UI_BUTTON);
            }
        }
    }

    if !clicked {
        return;
    }

    let mut cats: Vec<String> = lib
        .entries
        .iter()
        .map(|e| e.category.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();
    cats.sort();
    cats.dedup();

    let current = lib.active_category.trim().to_string();
    let next = if cats.is_empty() {
        String::new()
    } else if current.is_empty() {
        cats[0].clone()
    } else {
        match cats.iter().position(|c| c == &current) {
            Some(i) if i + 1 < cats.len() => cats[i + 1].clone(),
            _ => String::new(),
        }
    };

    lib.active_category = next;
    save_tileset_library(&lib);
    ui_state.built_tileset_menu_category.clear();
}

/// 根据 UiState 显示/隐藏 tileset 菜单。
pub fn tileset_menu_visibility(ui_state: Res<UiState>, mut menu_q: Query<&mut Visibility, With<TilesetMenuRoot>>) {
    let Ok(mut vis) = menu_q.single_mut() else {
        return;
    };
    *vis = if ui_state.tileset_menu_open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

/// 当菜单打开或库发生变化时，重建 tileset 列表按钮。
pub fn rebuild_tileset_menu_when_needed(
    mut commands: Commands,
    lib: Res<TilesetLibrary>,
    mut ui_state: ResMut<UiState>,
    menu_q: Query<Entity, With<TilesetMenuRoot>>,
    children_q: Query<&Children>,
) {
    if !ui_state.tileset_menu_open {
        return;
    }

    let Ok(menu_entity) = menu_q.single() else {
        return;
    };

    let active = lib.active_id.clone().unwrap_or_default();
    let category = lib.active_category.trim().to_string();
    let needs_rebuild = lib.is_changed()
        || ui_state.built_tileset_menu_count != lib.entries.len()
        || ui_state.built_tileset_menu_active_id != active
        || ui_state.built_tileset_menu_category != category;
    if !needs_rebuild {
        return;
    }

    // 清理旧菜单（按钮只有一层子节点：Text）
    if let Ok(children) = children_q.get(menu_entity) {
        for child in children.iter() {
            if let Ok(grandchildren) = children_q.get(child) {
                for grandchild in grandchildren.iter() {
                    commands.entity(grandchild).despawn();
                }
            }
            commands.entity(child).despawn();
        }
    }

    commands.entity(menu_entity).with_children(|p| {
        let entries: Vec<_> = lib
            .entries
            .iter()
            .filter(|e| category.is_empty() || e.category.trim() == category)
            .collect();

        if entries.is_empty() {
            p.spawn((
                Text::new(if category.is_empty() {
                    "(库为空：点击【打开】导入 tileset)"
                } else {
                    "(该分类下没有 tileset)"
                }),
                TextFont { font_size: 13.0, ..default() },
                TextColor(Color::WHITE),
            ));
            return;
        }

        for e in entries {
            let label = if !e.name.trim().is_empty() {
                e.name.clone()
            } else if !e.asset_path.trim().is_empty() {
                e.asset_path.clone()
            } else {
                e.id.clone()
            };
            let label = if !e.category.trim().is_empty() {
                format!("{}  [{}]", label, e.category)
            } else {
                label
            };

            let is_active = lib.active_id.as_ref().is_some_and(|id| id == &e.id);
            p.spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(32.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if is_active { UI_HIGHLIGHT } else { UI_BUTTON }),
                TilesetSelectItem { id: e.id.clone() },
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(label),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(Color::WHITE),
                ));
            });
        }
    });

    ui_state.built_tileset_menu_count = lib.entries.len();
    ui_state.built_tileset_menu_active_id = active;
    ui_state.built_tileset_menu_category = category;
}

/// 点击菜单项切换当前 tileset。
pub fn tileset_menu_item_click(
    mut lib: ResMut<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    mut editor_state: ResMut<EditorState>,
    mut ui_state: ResMut<UiState>,
    mut q: Query<(&Interaction, &TilesetSelectItem, &mut BackgroundColor), Changed<Interaction>>,
) {
    let mut picked: Option<String> = None;

    for (interaction, item, mut bg) in q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                picked = Some(item.id.clone());
                *bg = BackgroundColor(UI_BUTTON_PRESS);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_BUTTON_HOVER);
            }
            Interaction::None => {
                let is_active = lib.active_id.as_ref().is_some_and(|id| id == &item.id);
                *bg = BackgroundColor(if is_active { UI_HIGHLIGHT } else { UI_BUTTON });
            }
        }
    }

    let Some(id) = picked else {
        return;
    };

    lib.active_id = Some(id.clone());
    save_tileset_library(&lib);
    ui_state.tileset_menu_open = false;
    ui_state.built_for_tileset_path.clear();
    ui_state.built_tileset_menu_active_id.clear();

    if let Some(atlas) = runtime.by_id.get(&id) {
        let tile_count = atlas.columns.saturating_mul(atlas.rows).max(1);
        editor_state.selected_tile = editor_state.selected_tile.min(tile_count - 1);
    }
}
