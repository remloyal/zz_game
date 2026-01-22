use bevy::prelude::*;

use crate::editor::types::{
    CellChange, Clipboard, EditCommand, EditorConfig, LayerState, MapSizeFocus, MapSizeInput,
    PasteState, SelectionRect, SelectionState, TileEntities, TileMapData, TileRef, TilesetRuntime,
    ToolKind, ToolState, UndoStack,
};

use super::context_menu;

/// Ctrl+C 复制选择区域到 Clipboard；Ctrl+V 进入粘贴模式；Esc 退出粘贴。
pub fn copy_paste_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    mut tools: ResMut<ToolState>,
    layer_state: Res<LayerState>,
    selection: Res<SelectionState>,
    map: Option<Res<TileMapData>>,
    mut clipboard: ResMut<Clipboard>,
    paste: ResMut<PasteState>,
) {
    // 输入框聚焦时不抢快捷键
    if input.focus != MapSizeFocus::None {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if tools.tool == ToolKind::Paste {
            let back = tools
                .return_after_paste
                .take()
                .unwrap_or(ToolKind::Select);
            tools.tool = back;
            info!("exit paste -> back to {:?}", back);
        }
        return;
    }

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }

    if keys.just_pressed(KeyCode::KeyC) {
        let Some(map) = map else {
            return;
        };
        let Some(rect) = selection.rect else {
            return;
        };
        let layer = layer_state.active.min(map.layers.saturating_sub(1));
        context_menu::copy_selection_to_clipboard(layer, rect, &map, &mut clipboard);
    }

    if keys.just_pressed(KeyCode::KeyV) {
        if clipboard.width == 0 || clipboard.height == 0 || clipboard.tiles.is_empty() {
            info!("enter paste: clipboard empty -> ignored");
            return;
        }
        if tools.tool != ToolKind::Paste {
            tools.return_after_paste = Some(tools.tool);
        }
        tools.tool = ToolKind::Paste;
        info!(
            "enter paste: clipboard {}x{} (tiles={}), keep transform rot={} flip_x={} flip_y={}",
            clipboard.width,
            clipboard.height,
            clipboard.tiles.len(),
            paste.rot % 4,
            paste.flip_x,
            paste.flip_y
        );
    }
}

/// 选择编辑：Ctrl+X 剪切（复制到剪贴板并清空选区），Delete/Backspace 清空选区。
pub fn selection_cut_delete_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    tools: Res<ToolState>,
    layer_state: Res<LayerState>,
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    selection: Res<SelectionState>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    mut clipboard: ResMut<Clipboard>,
    mut undo: ResMut<UndoStack>,
) {
    if input.focus != MapSizeFocus::None {
        return;
    }
    if tools.tool != ToolKind::Select {
        return;
    }
    let Some(rect) = selection.rect else {
        return;
    };

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let want_cut = ctrl && keys.just_pressed(KeyCode::KeyX);
    let want_clear = keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace);
    if !(want_cut || want_clear) {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };
    let layer = layer_state.active.min(map.layers.saturating_sub(1));

    if want_cut {
        let w = rect.width();
        let h = rect.height();
        let mut tiles = Vec::with_capacity((w * h) as usize);
        for y in rect.min.y..=rect.max.y {
            for x in rect.min.x..=rect.max.x {
                let idx = map.idx_layer(layer, x, y);
                tiles.push(map.tiles[idx].clone());
            }
        }
        clipboard.width = w;
        clipboard.height = h;
        clipboard.tiles = tiles;
    }

    let mut cmd = EditCommand::default();
    for y in rect.min.y..=rect.max.y {
        for x in rect.min.x..=rect.max.x {
            let idx = map.idx_layer(layer, x, y);
            if map.tiles[idx].is_none() {
                continue;
            }
            let before = map.tiles[idx].clone();
            map.tiles[idx] = None;
            cmd.changes.push(CellChange {
                idx,
                before,
                after: None,
            });
        }
    }

    if cmd.changes.is_empty() {
        return;
    }

    let layer_len = map.layer_len();
    let layer_offset = (layer as usize) * layer_len;
    for ch in &cmd.changes {
        let local = ch.idx.saturating_sub(layer_offset);
        let x = (local % map.width as usize) as u32;
        let y = (local / map.width as usize) as u32;
        let entity_idx = tile_entities.idx_layer(layer, x, y);
        if entity_idx >= tile_entities.entities.len() {
            continue;
        }
        let entity = tile_entities.entities[entity_idx];
        if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
            super::apply_tile_visual(&runtime, &ch.after, &mut sprite, &mut tf, &mut vis, &config);
        }
    }

    undo.push(cmd);
}

/// 选择辅助：Ctrl+A 全选，Ctrl+D 取消选择。
pub fn selection_selectall_cancel_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    mut tools: ResMut<ToolState>,
    map: Option<Res<TileMapData>>,
    mut selection: ResMut<SelectionState>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }

    // 允许 Ctrl+A / Ctrl+D 在地图尺寸输入框聚焦时依然生效。
    let _ = input;

    if keys.just_pressed(KeyCode::KeyD) {
        if tools.tool == ToolKind::Paste {
            tools.tool = ToolKind::Select;
        }
        selection.dragging = false;
        selection.rect = None;
        return;
    }

    if keys.just_pressed(KeyCode::KeyA) {
        let Some(map) = map else {
            return;
        };
        if map.width == 0 || map.height == 0 {
            return;
        }
        let rect = SelectionRect {
            min: UVec2::ZERO,
            max: UVec2::new(map.width - 1, map.height - 1),
        };
        selection.dragging = false;
        selection.start = rect.min;
        selection.current = rect.max;
        selection.rect = Some(rect);
        tools.tool = ToolKind::Select;
        return;
    }
}

/// 选择区移动：在 Select 工具下按 Alt + 方向键，把选择框内内容整体移动 1 格（可撤销）。
pub fn move_selection_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    tools: Res<ToolState>,
    layer_state: Res<LayerState>,
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    mut undo: ResMut<UndoStack>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    mut selection: ResMut<SelectionState>,
) {
    if input.focus != MapSizeFocus::None {
        return;
    }
    if tools.tool != ToolKind::Select {
        return;
    }
    let Some(rect) = selection.rect else {
        return;
    };

    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if !alt {
        return;
    }

    let mut dx: i32 = 0;
    let mut dy: i32 = 0;
    if keys.just_pressed(KeyCode::ArrowLeft) {
        dx = -1;
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        dx = 1;
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        dy = 1;
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        dy = -1;
    } else {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };

    // 不允许越界移动，避免裁剪导致“选区变形”。
    let new_min_x = rect.min.x as i32 + dx;
    let new_min_y = rect.min.y as i32 + dy;
    let new_max_x = rect.max.x as i32 + dx;
    let new_max_y = rect.max.y as i32 + dy;
    if new_min_x < 0
        || new_min_y < 0
        || new_max_x >= map.width as i32
        || new_max_y >= map.height as i32
    {
        return;
    }

    let new_rect = SelectionRect {
        min: UVec2::new(new_min_x as u32, new_min_y as u32),
        max: UVec2::new(new_max_x as u32, new_max_y as u32),
    };

    // 先备份原内容
    let w = rect.width();
    let h = rect.height();
    let mut buf: Vec<Option<TileRef>> = Vec::with_capacity((w * h) as usize);
    let layer = layer_state.active.min(map.layers.saturating_sub(1));
    for y in rect.min.y..=rect.max.y {
        for x in rect.min.x..=rect.max.x {
            let idx = map.idx_layer(layer, x, y);
            buf.push(map.tiles[idx].clone());
        }
    }

    let mut cmd = EditCommand::default();

    // 清空原区域
    for y in rect.min.y..=rect.max.y {
        for x in rect.min.x..=rect.max.x {
            let idx = map.idx_layer(layer, x, y);
            if map.tiles[idx].is_some() {
                let before = map.tiles[idx].clone();
                map.tiles[idx] = None;
                cmd.changes.push(CellChange {
                    idx,
                    before,
                    after: None,
                });
            }
        }
    }

    // 写入新区域
    for cy in 0..h {
        for cx in 0..w {
            let dst_x = new_rect.min.x + cx;
            let dst_y = new_rect.min.y + cy;
            let after = buf[(cy * w + cx) as usize].clone();
            let idx = map.idx_layer(layer, dst_x, dst_y);
            if map.tiles[idx] == after {
                continue;
            }
            let before = map.tiles[idx].clone();
            map.tiles[idx] = after.clone();
            cmd.changes.push(CellChange { idx, before, after });
        }
    }

    if cmd.changes.is_empty() {
        return;
    }

    // 局部刷新渲染
    let layer_len = map.layer_len();
    let layer_offset = (layer as usize) * layer_len;
    for ch in &cmd.changes {
        let local = ch.idx.saturating_sub(layer_offset);
        let x = (local % map.width as usize) as u32;
        let y = (local / map.width as usize) as u32;
        let entity_idx = tile_entities.idx_layer(layer, x, y);
        if entity_idx >= tile_entities.entities.len() {
            continue;
        }
        let entity = tile_entities.entities[entity_idx];
        if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
            super::apply_tile_visual(&runtime, &ch.after, &mut sprite, &mut tf, &mut vis, &config);
        }
    }

    undo.push(cmd);
    selection.rect = Some(new_rect);
    selection.start = new_rect.min;
    selection.current = new_rect.max;
}
