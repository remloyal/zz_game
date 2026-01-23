use bevy::prelude::*;

use crate::editor::{UI_BUTTON, UI_HIGHLIGHT};
use crate::editor::types::{
    LayerActiveLabel, LayerActiveLockLabel, LayerActiveLockToggleButton, LayerActiveVisLabel,
    LayerActiveVisToggleButton, LayerNextButton, LayerPrevButton, LayerState, TileMapData,
};

/// 右上角：上一层/下一层按钮。
pub fn layer_topbar_buttons(
    mut layer_state: ResMut<LayerState>,
    map: Option<ResMut<TileMapData>>,
    mut prev_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<LayerPrevButton>,
            Without<LayerNextButton>,
            Without<LayerActiveVisToggleButton>,
            Without<LayerActiveLockToggleButton>,
        ),
    >,
    mut next_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<LayerNextButton>,
            Without<LayerPrevButton>,
            Without<LayerActiveVisToggleButton>,
            Without<LayerActiveLockToggleButton>,
        ),
    >,
    mut vis_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<LayerActiveVisToggleButton>,
            Without<LayerPrevButton>,
            Without<LayerNextButton>,
            Without<LayerActiveLockToggleButton>,
        ),
    >,
    mut lock_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<LayerActiveLockToggleButton>,
            Without<LayerPrevButton>,
            Without<LayerNextButton>,
            Without<LayerActiveVisToggleButton>,
        ),
    >,
) {
    let mut map = map;
    let total_layers = map.as_deref().map(|m| m.layers.max(1)).unwrap_or(1);

    for (interaction, mut bg) in prev_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
                if total_layers > 0 {
                    layer_state.active = layer_state.active.saturating_sub(1);
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::None => {
                *bg = BackgroundColor(UI_BUTTON);
            }
        }
    }

    for (interaction, mut bg) in next_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
                if total_layers > 0 {
                    let max_layer = total_layers.saturating_sub(1);
                    layer_state.active = (layer_state.active + 1).min(max_layer);
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::None => {
                *bg = BackgroundColor(UI_BUTTON);
            }
        }
    }

    // 当前层显隐切换
    for (interaction, mut bg) in vis_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_HIGHLIGHT);

                if let Some(map) = map.as_mut() {
                    // 确保 layer_data 长度与 layers 对齐（兼容旧数据）
                    let layers = map.layers.max(1);
                    map.ensure_layers(layers);

                    let active = layer_state.active.min(layers.saturating_sub(1));
                    layer_state.active = active;
                    if let Some(d) = map.layer_data.get_mut(active as usize) {
                        d.visible = !d.visible;
                    }
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::None => {
                // 正常态颜色：由当前层 visible 决定
                let visible = map
                    .as_deref()
                    .and_then(|m| m.layer_data.get(layer_state.active as usize))
                    .map(|d| d.visible)
                    .unwrap_or(true);
                *bg = if visible {
                    BackgroundColor(Color::srgba(0.2, 0.8, 0.2, 0.8))
                } else {
                    BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.8))
                };
            }
        }
    }

    // 当前层锁定切换
    for (interaction, mut bg) in lock_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_HIGHLIGHT);

                if let Some(map) = map.as_mut() {
                    let layers = map.layers.max(1);
                    map.ensure_layers(layers);

                    let active = layer_state.active.min(layers.saturating_sub(1));
                    layer_state.active = active;
                    if let Some(d) = map.layer_data.get_mut(active as usize) {
                        d.locked = !d.locked;
                    }
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::None => {
                let locked = map
                    .as_deref()
                    .and_then(|m| m.layer_data.get(layer_state.active as usize))
                    .map(|d| d.locked)
                    .unwrap_or(false);
                *bg = if locked {
                    BackgroundColor(Color::srgba(0.8, 0.2, 0.2, 0.8))
                } else {
                    BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.8))
                };
            }
        }
    }
}

/// 右上角：当前层标签文本。
pub fn update_layer_topbar_label(
    mut layer_state: ResMut<LayerState>,
    map: Option<Res<TileMapData>>,
    mut q: Query<
        &mut Text,
        (
            With<LayerActiveLabel>,
            Without<LayerActiveVisLabel>,
            Without<LayerActiveLockLabel>,
        ),
    >,
    mut vis_label_q: Query<
        &mut Text,
        (
            With<LayerActiveVisLabel>,
            Without<LayerActiveLabel>,
            Without<LayerActiveLockLabel>,
        ),
    >,
    mut vis_btn_q: Query<
        (&Interaction, &mut BackgroundColor),
        (With<LayerActiveVisToggleButton>, Without<LayerActiveLockToggleButton>),
    >,
    mut lock_label_q: Query<
        &mut Text,
        (
            With<LayerActiveLockLabel>,
            Without<LayerActiveLabel>,
            Without<LayerActiveVisLabel>,
        ),
    >,
    mut lock_btn_q: Query<
        (&Interaction, &mut BackgroundColor),
        (With<LayerActiveLockToggleButton>, Without<LayerActiveVisToggleButton>),
    >,
) {
    let Some(map) = map.as_deref() else {
        for mut t in q.iter_mut() {
            *t = Text::new("-/-");
        }
        for mut t in vis_label_q.iter_mut() {
            *t = Text::new("-");
        }
        for mut t in lock_label_q.iter_mut() {
            *t = Text::new("-");
        }
        return;
    };

    let total = map.layers.max(1);
    if layer_state.active >= total {
        layer_state.active = total - 1;
    }

    let active = layer_state.active;
    let name = map
        .layer_data
        .get(active as usize)
        .map(|d| d.name.as_str())
        .unwrap_or("Layer");
    let visible = map
        .layer_data
        .get(active as usize)
        .map(|d| d.visible)
        .unwrap_or(true);
    let locked = map
        .layer_data
        .get(active as usize)
        .map(|d| d.locked)
        .unwrap_or(false);

    let mut suffix = String::new();
    if !visible {
        suffix.push_str(" 隐藏");
    }
    if locked {
        suffix.push_str(" 锁定");
    }

    let label = format!("{}/{} {}{}", active + 1, total, name, suffix);
    for mut t in q.iter_mut() {
        *t = Text::new(label.clone());
    }

    // 悬浮按钮：同步当前层显隐显示
    let vis_text = if visible { "显" } else { "隐" };
    for mut t in vis_label_q.iter_mut() {
        *t = Text::new(vis_text);
    }

    // 悬浮按钮：同步当前层锁定显示
    let lock_text = if locked { "锁" } else { "解" };
    for mut t in lock_label_q.iter_mut() {
        *t = Text::new(lock_text);
    }

    // 仅在 Interaction::None 时刷新“正常态颜色”，避免覆盖 hover 高亮
    for (interaction, mut bg) in vis_btn_q.iter_mut() {
        if *interaction == Interaction::None {
            *bg = if visible {
                BackgroundColor(Color::srgba(0.2, 0.8, 0.2, 0.8))
            } else {
                BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.8))
            };
        }
    }

    for (interaction, mut bg) in lock_btn_q.iter_mut() {
        if *interaction == Interaction::None {
            *bg = if locked {
                BackgroundColor(Color::srgba(0.8, 0.2, 0.2, 0.8))
            } else {
                BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.8))
            };
        }
    }
}
