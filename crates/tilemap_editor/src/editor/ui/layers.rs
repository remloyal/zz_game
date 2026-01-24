use bevy::prelude::*;

use crate::editor::{UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS, UI_HIGHLIGHT};
use crate::editor::types::{
    LayerActiveLabel, LayerActiveLockLabel, LayerActiveLockToggleButton, LayerActiveVisLabel,
    LayerActiveVisToggleButton, LayerNextButton, LayerPrevButton, LayerState, TileMapData,
    LayerNameApplyButton, LayerNameField, LayerNameInput, LayerNameText,
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

    // 悬浮按钮：同步当前层锁定显示
    let lock_text = if locked { "锁" } else { "解" };
    for mut t in lock_label_q.iter_mut() {
        *t = Text::new(lock_text);
    }

}


/// 图层命名输入：点击输入框/应用按钮。
pub fn layer_name_widget_interactions(
    mut input: ResMut<LayerNameInput>,
    mut field_q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<LayerNameField>, Without<LayerNameApplyButton>),
    >,
    mut apply_q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<LayerNameApplyButton>, Without<LayerNameField>),
    >,
) {
    for (interaction, mut bg) in field_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                input.focused = true;
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::Hovered => {
                if input.focused {
                    *bg = BackgroundColor(UI_HIGHLIGHT);
                } else {
                    *bg = BackgroundColor(UI_BUTTON_HOVER);
                }
            }
            Interaction::None => {
                if input.focused {
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
                input.apply_requested = true;
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

/// 图层命名输入：键盘输入。
pub fn layer_name_text_input(keys: Res<ButtonInput<KeyCode>>, mut input: ResMut<LayerNameInput>) {
    if !input.focused {
        return;
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let key_map: &[(KeyCode, char, char)] = &[
        (KeyCode::Digit0, '0', '0'),
        (KeyCode::Digit1, '1', '1'),
        (KeyCode::Digit2, '2', '2'),
        (KeyCode::Digit3, '3', '3'),
        (KeyCode::Digit4, '4', '4'),
        (KeyCode::Digit5, '5', '5'),
        (KeyCode::Digit6, '6', '6'),
        (KeyCode::Digit7, '7', '7'),
        (KeyCode::Digit8, '8', '8'),
        (KeyCode::Digit9, '9', '9'),
        (KeyCode::Numpad0, '0', '0'),
        (KeyCode::Numpad1, '1', '1'),
        (KeyCode::Numpad2, '2', '2'),
        (KeyCode::Numpad3, '3', '3'),
        (KeyCode::Numpad4, '4', '4'),
        (KeyCode::Numpad5, '5', '5'),
        (KeyCode::Numpad6, '6', '6'),
        (KeyCode::Numpad7, '7', '7'),
        (KeyCode::Numpad8, '8', '8'),
        (KeyCode::Numpad9, '9', '9'),
        (KeyCode::Minus, '-', '_'),
        (KeyCode::NumpadSubtract, '-', '-'),
        (KeyCode::Space, ' ', ' '),
        (KeyCode::Period, '.', '.'),
        (KeyCode::KeyA, 'a', 'A'),
        (KeyCode::KeyB, 'b', 'B'),
        (KeyCode::KeyC, 'c', 'C'),
        (KeyCode::KeyD, 'd', 'D'),
        (KeyCode::KeyE, 'e', 'E'),
        (KeyCode::KeyF, 'f', 'F'),
        (KeyCode::KeyG, 'g', 'G'),
        (KeyCode::KeyH, 'h', 'H'),
        (KeyCode::KeyI, 'i', 'I'),
        (KeyCode::KeyJ, 'j', 'J'),
        (KeyCode::KeyK, 'k', 'K'),
        (KeyCode::KeyL, 'l', 'L'),
        (KeyCode::KeyM, 'm', 'M'),
        (KeyCode::KeyN, 'n', 'N'),
        (KeyCode::KeyO, 'o', 'O'),
        (KeyCode::KeyP, 'p', 'P'),
        (KeyCode::KeyQ, 'q', 'Q'),
        (KeyCode::KeyR, 'r', 'R'),
        (KeyCode::KeyS, 's', 'S'),
        (KeyCode::KeyT, 't', 'T'),
        (KeyCode::KeyU, 'u', 'U'),
        (KeyCode::KeyV, 'v', 'V'),
        (KeyCode::KeyW, 'w', 'W'),
        (KeyCode::KeyX, 'x', 'X'),
        (KeyCode::KeyY, 'y', 'Y'),
        (KeyCode::KeyZ, 'z', 'Z'),
    ];

    for (key, normal, shifted) in key_map {
        if keys.just_pressed(*key) {
            if input.buf.len() < 32 {
                input.buf.push(if shift { *shifted } else { *normal });
            }
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        input.buf.pop();
    }

    if keys.just_pressed(KeyCode::Escape) {
        input.focused = false;
    }

    if keys.just_pressed(KeyCode::Enter) {
        input.apply_requested = true;
        input.focused = false;
    }
}

/// 当未聚焦时，同步当前层名称到输入框。
pub fn sync_layer_name_input_from_map(
    layer_state: Res<LayerState>,
    map: Option<Res<TileMapData>>,
    mut input: ResMut<LayerNameInput>,
) {
    if input.focused {
        return;
    }

    let Some(map) = map.as_deref() else {
        if !input.buf.is_empty() {
            input.buf.clear();
        }
        return;
    };

    let total = map.layers.max(1);
    let active = layer_state.active.min(total.saturating_sub(1));
    let name = map
        .layer_data
        .get(active as usize)
        .map(|d| d.name.as_str())
        .unwrap_or("Layer");

    if input.buf != name {
        input.buf = name.to_string();
    }
}

/// 图层命名输入框文字刷新。
pub fn update_layer_name_field_text(
    input: Res<LayerNameInput>,
    mut q: Query<&mut Text, With<LayerNameText>>,
) {
    let text = if input.focused {
        format!(" {}|", input.buf)
    } else if input.buf.trim().is_empty() {
        " <未命名>".to_string()
    } else {
        format!(" {}", input.buf)
    };

    for mut t in q.iter_mut() {
        *t = Text::new(text.clone());
    }
}

/// 应用图层名称变更。
pub fn apply_layer_name_change(
    mut input: ResMut<LayerNameInput>,
    map: Option<ResMut<TileMapData>>,
    layer_state: Res<LayerState>,
) {
    if !input.apply_requested {
        return;
    }
    input.apply_requested = false;

    let Some(mut map) = map else {
        return;
    };

    let layers = map.layers.max(1);
    map.ensure_layers(layers);
    let active = layer_state.active.min(layers.saturating_sub(1));

    let name = input.buf.trim();
    let name = if name.is_empty() {
        format!("Layer {}", active + 1)
    } else {
        name.to_string()
    };

    if let Some(d) = map.layer_data.get_mut(active as usize) {
        d.name = name;
    }
}
