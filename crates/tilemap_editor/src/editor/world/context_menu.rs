use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{
    CellChange, Clipboard, ContextMenuAction, ContextMenuCommand, ContextMenuState, EditCommand,
    EditorConfig, LayerState, PasteState, SelectionRect, SelectionState, TileEntities, TileMapData,
    TilesetRuntime, ToolKind, ToolState, UndoStack, WorldCamera,
};
use crate::editor::{LEFT_PANEL_WIDTH_PX, UI_TOP_RESERVED_PX};

use super::{apply_map_to_entities, apply_tile_visual, cursor_tile_pos};

/// 右键菜单：先支持粘贴模式的变换控制（后续可扩展到其他工具）。
pub fn context_menu_open_close(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    tools: Res<ToolState>,
    _clipboard: Res<Clipboard>,
    config: Res<EditorConfig>,
    tile_entities: Option<Res<TileEntities>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    mut menu: ResMut<ContextMenuState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    if keys.just_pressed(KeyCode::Escape) {
        if menu.open {
            menu.open = false;
            menu.consume_left_click = true;
        }
        return;
    }

    if buttons.just_pressed(MouseButton::Right) {
        let Some(pos) = window.cursor_position() else {
            return;
        };
        // 只在画布区域打开（避免左侧/顶栏误触）
        if pos.x <= LEFT_PANEL_WIDTH_PX {
            return;
        }
        if pos.y <= UI_TOP_RESERVED_PX {
            return;
        }
        menu.open = true;
        menu.signature = 0;
        menu.consume_left_click = false;
        menu.screen_pos = pos;
        menu.map_pos = None;
        if let Some(tile_entities) = tile_entities.as_deref() {
            if let Ok((camera, camera_transform)) = camera_q.single() {
                menu.map_pos = cursor_tile_pos(
                    window,
                    camera,
                    camera_transform,
                    &config,
                    tile_entities.width,
                    tile_entities.height,
                );
            }
        }
        info!(
            "context menu open at screen ({:.1}, {:.1}) tool={:?}",
            pos.x, pos.y, tools.tool
        );
    }

    // “点空白关闭”交给 UI 侧的 ContextMenuBackdrop 处理，避免 world 侧做不精确的 bounds 判断。
}

pub(super) fn copy_selection_to_clipboard(
    layer: u32,
    rect: SelectionRect,
    map: &TileMapData,
    clipboard: &mut Clipboard,
) {
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

pub(super) fn clear_selection_to_none(
    layer: u32,
    rect: SelectionRect,
    map: &mut TileMapData,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tile_entities: &TileEntities,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) {
    let mut cmd = EditCommand::default();
    for y in rect.min.y..=rect.max.y {
        for x in rect.min.x..=rect.max.x {
            let idx = map.idx_layer(layer, x, y);
            if map.tiles[idx].is_none() {
                continue;
            }
            let before = map.tiles[idx].clone();
            let after = None;
            map.tiles[idx] = None;
            cmd.changes.push(CellChange { idx, before, after });
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
            apply_tile_visual(runtime, &ch.after, &mut sprite, &mut tf, &mut vis, config);
        }
    }
    undo.push(cmd);
}

pub fn apply_context_menu_command(
    mut cmd: ResMut<ContextMenuCommand>,
    mut tools: ResMut<ToolState>,
    mut paste: ResMut<PasteState>,
    mut selection: ResMut<SelectionState>,
    mut clipboard: ResMut<Clipboard>,
    menu: Res<ContextMenuState>,
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    layer_state: Res<LayerState>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    mut undo: ResMut<UndoStack>,
) {
    let Some(action) = cmd.action.take() else {
        return;
    };

    match action {
        ContextMenuAction::Undo => {
            let Some(mut map) = map else {
                return;
            };
            let Some(tile_entities) = tile_entities.as_deref() else {
                return;
            };

            let Some(cmd) = undo.undo.pop() else {
                return;
            };
            for ch in &cmd.changes {
                if ch.idx < map.tiles.len() {
                    map.tiles[ch.idx] = ch.before.clone();
                }
            }
            undo.redo.push(cmd);
            apply_map_to_entities(&runtime, &map, tile_entities, &mut tiles_q, &config);
            info!("context cmd: undo");
        }
        ContextMenuAction::Redo => {
            let Some(mut map) = map else {
                return;
            };
            let Some(tile_entities) = tile_entities.as_deref() else {
                return;
            };

            let Some(cmd) = undo.redo.pop() else {
                return;
            };
            for ch in &cmd.changes {
                if ch.idx < map.tiles.len() {
                    map.tiles[ch.idx] = ch.after.clone();
                }
            }
            undo.undo.push(cmd);
            apply_map_to_entities(&runtime, &map, tile_entities, &mut tiles_q, &config);
            info!("context cmd: redo");
        }
        ContextMenuAction::EnterPaste => {
            if clipboard.width > 0 && clipboard.height > 0 && !clipboard.tiles.is_empty() {
                if tools.tool != ToolKind::Paste {
                    tools.return_after_paste = Some(tools.tool);
                }
                tools.tool = ToolKind::Paste;
                info!(
                    "context cmd: enter paste (keep transform rot={} flip_x={} flip_y={})",
                    paste.rot % 4,
                    paste.flip_x,
                    paste.flip_y
                );
            } else {
                info!("context cmd: enter paste ignored (clipboard empty)");
            }
        }
        ContextMenuAction::SelectionCopy => {
            let Some(map) = map.as_deref() else {
                return;
            };
            let Some(rect) = selection.rect else {
                return;
            };
            let layer = layer_state.active.min(map.layers.saturating_sub(1));
            copy_selection_to_clipboard(layer, rect, map, &mut clipboard);
            tools.tool = ToolKind::Select;
            info!("context cmd: selection copy {}x{}", clipboard.width, clipboard.height);
        }
        ContextMenuAction::SelectionCut => {
            let Some(mut map) = map else {
                return;
            };
            let Some(tile_entities) = tile_entities.as_deref() else {
                return;
            };
            let Some(rect) = selection.rect else {
                return;
            };
            let layer = layer_state.active.min(map.layers.saturating_sub(1));
            copy_selection_to_clipboard(layer, rect, &map, &mut clipboard);
            clear_selection_to_none(
                layer,
                rect,
                &mut map,
                &runtime,
                &config,
                tile_entities,
                &mut tiles_q,
                &mut undo,
            );
            tools.tool = ToolKind::Select;
            info!("context cmd: selection cut {}x{}", clipboard.width, clipboard.height);
        }
        ContextMenuAction::SelectionDelete => {
            let Some(mut map) = map else {
                return;
            };
            let Some(tile_entities) = tile_entities.as_deref() else {
                return;
            };
            let Some(rect) = selection.rect else {
                return;
            };
            let layer = layer_state.active.min(map.layers.saturating_sub(1));
            clear_selection_to_none(
                layer,
                rect,
                &mut map,
                &runtime,
                &config,
                tile_entities,
                &mut tiles_q,
                &mut undo,
            );
            tools.tool = ToolKind::Select;
            info!("context cmd: selection delete");
        }
        ContextMenuAction::SelectionSelectAll => {
            let Some(map) = map.as_deref() else {
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
            info!("context cmd: select all");
        }
        ContextMenuAction::SelectionDeselect => {
            selection.dragging = false;
            selection.rect = None;
            tools.tool = ToolKind::Select;
            info!("context cmd: deselect");
        }
        ContextMenuAction::PasteRotateCcw
        | ContextMenuAction::PasteRotateCw
        | ContextMenuAction::PasteFlipX
        | ContextMenuAction::PasteFlipY
        | ContextMenuAction::PasteReset
        | ContextMenuAction::ExitPaste => {
            super::paste_transform::apply_context_menu_paste_action(
                action,
                &mut tools,
                &mut paste,
                &clipboard,
                &menu,
                &runtime,
                &config,
                map,
                tile_entities.as_deref(),
                &mut tiles_q,
                &mut undo,
            );
        }
    }
}

pub fn context_menu_clear_consumption(
    buttons: Res<ButtonInput<MouseButton>>,
    mut menu: ResMut<ContextMenuState>,
) {
    if menu.consume_left_click && !buttons.pressed(MouseButton::Left) {
        menu.consume_left_click = false;
    }
}
