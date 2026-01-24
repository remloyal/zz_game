//! 左侧 tileset palette：缩略图网格 + 滚动 + 点击选 tile。

use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::PrimaryWindow;

use crate::editor::{LEFT_PANEL_WIDTH_PX, UI_BUTTON, UI_BUTTON_HOVER, UI_HIGHLIGHT};
use crate::editor::tileset::rect_for_tile_index;
use crate::editor::util::despawn_silently;
use crate::editor::types::{
    EditorConfig, EditorState, PaletteRoot, PaletteScroll, PaletteTileButton, TilesetLibrary,
    TilesetRuntime, UiState,
    PaletteSearchClearButton, PaletteSearchField, PaletteSearchInput, PaletteSearchText,
    PaletteZoomButton, PaletteZoomLevel,
};

const PALETTE_TILE_PX_SMALL: f32 = 32.0;
const PALETTE_TILE_PX_MEDIUM: f32 = 40.0;
const PALETTE_TILE_PX_LARGE: f32 = 56.0;
const PALETTE_COL_GAP_PX: f32 = 6.0;
const PALETTE_ROW_GAP_PX: f32 = 6.0;

fn zoom_level_for_tile_px(px: f32) -> PaletteZoomLevel {
    if (px - PALETTE_TILE_PX_SMALL).abs() <= 0.5 {
        PaletteZoomLevel::Small
    } else if (px - PALETTE_TILE_PX_LARGE).abs() <= 0.5 {
        PaletteZoomLevel::Large
    } else {
        PaletteZoomLevel::Medium
    }
}

fn tile_px_for_zoom(level: PaletteZoomLevel) -> f32 {
    match level {
        PaletteZoomLevel::Small => PALETTE_TILE_PX_SMALL,
        PaletteZoomLevel::Medium => PALETTE_TILE_PX_MEDIUM,
        PaletteZoomLevel::Large => PALETTE_TILE_PX_LARGE,
    }
}

fn parse_u32(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse::<u32>().ok()
}

fn palette_filter_matches(index: u32, columns: u32, query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }

    // 10-20 / 20-10
    if let Some((a, b)) = q.split_once('-') {
        if let (Some(a), Some(b)) = (parse_u32(a), parse_u32(b)) {
            let (min, max) = if a <= b { (a, b) } else { (b, a) };
            return index >= min && index <= max;
        }
    }

    // 12
    if let Some(n) = parse_u32(q) {
        return index == n;
    }

    // 关键字：按“index / x,y / x*y / xXy”做子串匹配。
    let idx_s = index.to_string();
    if idx_s.contains(q) {
        return true;
    }
    let x = (index % columns.max(1)) as u32;
    let y = (index / columns.max(1)) as u32;
    let xy1 = format!("{x},{y}");
    let xy2 = format!("{x}x{y}");
    let xy3 = format!("{x}X{y}");
    xy1.contains(q) || xy2.contains(q) || xy3.contains(q)
}



pub fn palette_zoom_button_click(
    mut ui_state: ResMut<UiState>,
    mut scroll_q: Query<&mut ScrollPosition, With<PaletteScroll>>,
    mut q: Query<(&Interaction, &PaletteZoomButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    let mut changed = false;
    for (interaction, btn, mut bg) in q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
                let px = tile_px_for_zoom(btn.0);
                if (ui_state.palette_tile_px - px).abs() > f32::EPSILON {
                    ui_state.palette_tile_px = px;
                    changed = true;
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
            }
            Interaction::None => {
                // 颜色由 sync 系统统一处理
            }
        }
    }

    if changed {
        ui_state.built_for_tileset_path.clear();
        ui_state.built_palette_tile_px = -1.0;
        // 缩放后滚动回顶部，避免“跳帧感”
        for mut scroll in scroll_q.iter_mut() {
            scroll.0.y = 0.0;
        }
    }
}

pub fn sync_palette_zoom_button_styles(
    ui_state: Res<UiState>,
    mut q: Query<(&Interaction, &PaletteZoomButton, &mut BackgroundColor)>,
) {
    let active = zoom_level_for_tile_px(ui_state.palette_tile_px());
    for (interaction, btn, mut bg) in q.iter_mut() {
        if *interaction != Interaction::None {
            continue;
        }
        *bg = if btn.0 == active {
            BackgroundColor(UI_HIGHLIGHT)
        } else {
            BackgroundColor(UI_BUTTON)
        };
    }
}

pub fn palette_search_widget_interactions(
    mut input: ResMut<PaletteSearchInput>,
    mut ui_state: ResMut<UiState>,
    mut scroll_q: Query<&mut ScrollPosition, With<PaletteScroll>>,
    mut field_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<PaletteSearchField>,
            Without<PaletteSearchClearButton>,
        ),
    >,
    mut clear_q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<PaletteSearchClearButton>,
            Without<PaletteSearchField>,
        ),
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

    let mut cleared = false;
    for (interaction, mut bg) in clear_q.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(UI_HIGHLIGHT);
                if !input.buf.is_empty() {
                    input.buf.clear();
                    cleared = true;
                }
            }
            Interaction::Hovered => *bg = BackgroundColor(UI_HIGHLIGHT),
            Interaction::None => *bg = BackgroundColor(UI_BUTTON),
        }
    }

    if cleared {
        input.focused = false;
        ui_state.built_for_tileset_path.clear();
        ui_state.built_palette_filter.clear();
        for mut scroll in scroll_q.iter_mut() {
            scroll.0.y = 0.0;
        }
    }
}

pub fn palette_search_text_input(keys: Res<ButtonInput<KeyCode>>, mut input: ResMut<PaletteSearchInput>, mut ui_state: ResMut<UiState>, mut scroll_q: Query<&mut ScrollPosition, With<PaletteScroll>>) {
    if !input.focused {
        return;
    }

    let mut changed = false;

    // 稳定起见：用 KeyCode 录入常用 ASCII（字母/数字/空格/减号），避免平台差异。
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
        (KeyCode::Comma, ',', ','),
        (KeyCode::Space, ' ', ' '),
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
                changed = true;
            }
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        if input.buf.pop().is_some() {
            changed = true;
        }
    }

    if keys.just_pressed(KeyCode::Escape) {
        input.focused = false;
    }

    if keys.just_pressed(KeyCode::Enter) {
        input.focused = false;
    }

    if changed {
        ui_state.built_for_tileset_path.clear();
        ui_state.built_palette_filter.clear();
        for mut scroll in scroll_q.iter_mut() {
            scroll.0.y = 0.0;
        }
    }
}

pub fn update_palette_search_text(input: Res<PaletteSearchInput>, mut q: Query<&mut Text, With<PaletteSearchText>>) {
    let text = if input.buf.trim().is_empty() {
        if input.focused {
            " <全部>|".to_string()
        } else {
            " <全部>".to_string()
        }
    } else if input.focused {
        format!(" {}|", input.buf)
    } else {
        format!(" {}", input.buf)
    };

    for mut t in q.iter_mut() {
        *t = Text::new(text.clone());
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
        let before = scroll.0.y;
        scroll.0.y = (scroll.0.y + delta_y * speed).max(0.0);

        debug!("palette_scroll_wheel: before_y={:.1} after_y={:.1} delta_y={:.2}", before, scroll.0.y, delta_y);
    }
}

/// 兜底：把 PaletteScroll 的 `ScrollPosition.y` 显式映射为 PaletteRoot 的 `top` 偏移。
///
/// 在某些布局/嵌套情况下，Bevy 的滚动偏移可能不会体现在可见位置上（数值变了但列表看起来不动）。
/// 这个系统保证视觉上一定会跟着滚。
pub fn palette_apply_scroll_position_to_root(
    scroll_q: Query<&ScrollPosition, With<PaletteScroll>>,
    mut root_q: Query<&mut Node, With<PaletteRoot>>,
    mut last_applied_y: Local<f32>,
) {
    let Ok(scroll) = scroll_q.single() else {
        return;
    };
    let Ok(mut node) = root_q.single_mut() else {
        return;
    };

    let y = scroll.0.y;
    if node.position_type != PositionType::Absolute {
        node.position_type = PositionType::Absolute;
        node.left = Val::Px(0.0);
    }
    node.top = Val::Px(-y);

    // 只在 y 真正变化时打日志，避免刷屏。
    if (y - *last_applied_y).abs() > 0.5 {
        debug!("palette_apply_scroll: y={:.1} -> top={:.1}", y, -y);
        *last_applied_y = y;
    }
}

/// 钳制滚动范围，避免可以无限往下滚。
///
/// 由于我们用“ScrollPosition -> PaletteRoot.top 偏移”做兜底滚动，需要自己计算最大可滚动距离。
pub fn palette_clamp_scroll_position(
    ui_state: Res<UiState>,
    scroll_node_q: Query<&ComputedNode, With<PaletteScroll>>,
    tile_node_q: Query<&ComputedNode, With<PaletteTileButton>>,
    mut scroll_q: Query<&mut ScrollPosition, With<PaletteScroll>>,
    mut last_logged_y: Local<f32>,
) {
    let Ok(scroll_node) = scroll_node_q.single() else {
        return;
    };
	// 用任意一个 tile 按钮的实际布局尺寸来推导内容高度（避免 tile_px + padding/gap 估算偏差）。
	let Some(tile_node) = tile_node_q.iter().next() else {
		return;
	};

    let viewport_w = scroll_node.size().x;
    let viewport_h = scroll_node.size().y;

    // UI 布局刚初始化/刚重建时，ComputedNode 可能还未就绪（size 为 0）。
    if viewport_w <= 1.0 || viewport_h <= 1.0 {
        return;
    }

    // 由于我们使用“clip + 手动 top 偏移”，root 的 ComputedNode 高度可能不会反映真实内容高度。
    // 所以这里用「tile 数量 + tile 尺寸 + gap + 可用宽度」推导出理论内容高度来做 clamp。
    let tile_count = ui_state.built_palette_filtered_count;
    if tile_count == 0 {
        for mut scroll in scroll_q.iter_mut() {
            scroll.0.y = 0.0;
        }
        return;
    }

    let tile_w = tile_node.size().x;
    let tile_h = tile_node.size().y;
	if tile_w <= 1.0 || tile_h <= 1.0 {
		return;
	}

	// cols = floor((w + gap) / (tile_w + gap))
    let denom = (tile_w + PALETTE_COL_GAP_PX).max(1.0);
    let cols = (((viewport_w + PALETTE_COL_GAP_PX) / denom).floor() as u32).max(1);
    let rows = (tile_count + cols - 1) / cols;
    let content_h = rows as f32 * tile_h + (rows.saturating_sub(1) as f32) * PALETTE_ROW_GAP_PX;
	let max_y = (content_h - viewport_h).max(0.0);

    for mut scroll in scroll_q.iter_mut() {
        let before = scroll.0.y;
        scroll.0.y = scroll.0.y.clamp(0.0, max_y);
        let after = scroll.0.y;

        // 关键诊断：如果发生“滚一下就回顶”，给出 INFO（默认日志级别也能看到）。
        if before > 1.0 && after <= 0.5 {
            info!(
                "palette_scroll_clamp: snapped_to_top before_y={:.1} max_y={:.1} viewport=({:.1},{:.1}) tile=({:.1},{:.1}) cols={} rows={} tile_count={} filter_count={}",
                before,
                max_y,
                viewport_w,
                viewport_h,
                tile_w,
                tile_h,
                cols,
                rows,
                tile_count,
                ui_state.built_palette_filtered_count
            );
            debug!(
                "palette_scroll_clamp(debug): content_h={:.1} denom={:.1}",
                content_h,
                denom
            );
        }

        // 仅在 y 真正变化时记录一次 debug。
        if (after - *last_logged_y).abs() > 1.0 {
            debug!(
                "palette_scroll_clamp: y {:.1} -> {:.1} (max_y={:.1})",
                before,
                after,
                max_y
            );
            *last_logged_y = after;
        }
    }
}

/// tileset 加载完成后，动态生成左侧 palette（缩略图按钮网格）。
pub fn build_palette_when_ready(
    mut commands: Commands,
    config: Res<EditorConfig>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    mut ui_state: ResMut<UiState>,
    search: Res<PaletteSearchInput>,
    palette_q: Query<Entity, With<PaletteRoot>>,
    palette_children_q: Query<&Children>,
    mut scroll_q: Query<&mut ScrollPosition, With<PaletteScroll>>,
    mut last_tileset_id: Local<String>,
) {
    let Some(active_id) = lib.active_id.as_ref() else {
        return;
    };
    let Some(active) = runtime.by_id.get(active_id) else {
        return;
    };
    // tileset 切换时重置页码
    // 注意：不能用 built_for_tileset_path 来判断“是否切换”，因为它也会被其它系统清空以强制重建。
    if *last_tileset_id != *active_id {
        for mut scroll in scroll_q.iter_mut() {
            scroll.0.y = 0.0;
        }
        *last_tileset_id = active_id.clone();
    }

    let filter = search.buf.trim();
    let tile_px = ui_state.palette_tile_px();
    if ui_state.built_for_tileset_path == *active_id
        && (ui_state.built_palette_tile_px - tile_px).abs() <= 0.5
        && ui_state.built_palette_filter == filter
    {
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
                    despawn_silently(&mut commands, grandchild);
                }
            }
            despawn_silently(&mut commands, child);
        }
    }

    let tile_count = active.columns.saturating_mul(active.rows);
    let image = active.texture.clone();
    let columns = active.columns.max(1);

    let mut filtered_indices: Vec<u32> = Vec::new();
    filtered_indices.reserve(tile_count.min(2048) as usize);
    for idx in 0..tile_count {
        if palette_filter_matches(idx, columns, filter) {
            filtered_indices.push(idx);
        }
    }
    let filtered_count = filtered_indices.len() as u32;
    ui_state.built_palette_filtered_count = filtered_count;

    commands.entity(palette_entity).with_children(|p| {
        for &index in filtered_indices.iter() {
            let rect = rect_for_tile_index(index, columns, config.tile_size);

            p.spawn((
                Button,
                Node {
                    width: Val::Px(tile_px),
                    height: Val::Px(tile_px),
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
    ui_state.built_palette_tile_px = tile_px;
    ui_state.built_palette_filter = filter.to_string();

    // 如果筛选结果为空或很少，确保滚动位置不会卡在“旧的大 y”。
    if filtered_count == 0 {
        for mut scroll in scroll_q.iter_mut() {
            scroll.0.y = 0.0;
        }
    }
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
