use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{
    Clipboard, ContextMenuAction, ContextMenuState, EditorConfig, LayerState, PasteState,
    SelectionState, TileMapData, ToolKind, ToolState, UndoStack, WorldCamera,
};

use super::{
    cursor_tile_pos, try_flip_map_tile_x, try_flip_map_tile_y, try_reset_map_tile_transform,
    try_rotate_map_tile_ccw, try_rotate_map_tile_cw, TilemapRenderParams,
};

/// 粘贴变换：Q/E 旋转，H/V 翻转。
pub fn paste_transform_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    tools: Res<ToolState>,
    layer_state: Res<LayerState>,
    clipboard: Res<Clipboard>,
    config: Res<EditorConfig>,
    mut selection: ResMut<SelectionState>,
    map: Option<ResMut<TileMapData>>,
    mut undo: ResMut<UndoStack>,
    mut paste: ResMut<PasteState>,
    mut render: TilemapRenderParams,
) {
    // 避免与 Ctrl+V（进入/重置粘贴）等快捷键冲突。
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl {
        return;
    }

    let action = if keys.just_pressed(KeyCode::KeyQ) {
        Some(ContextMenuAction::PasteRotateCcw)
    } else if keys.just_pressed(KeyCode::KeyE) {
        Some(ContextMenuAction::PasteRotateCw)
    } else if keys.just_pressed(KeyCode::KeyH) {
        Some(ContextMenuAction::PasteFlipX)
    } else if keys.just_pressed(KeyCode::KeyV) {
        Some(ContextMenuAction::PasteFlipY)
    } else {
        None
    };

    let Some(action) = action else {
        return;
    };

    // 需要在“选区变换失败后”继续尝试单格/预设变换：
    // 因此不能提前把 map move 掉。
    let mut map_opt = map;

    // 粘贴模式：永远调整粘贴变换（预览/落地）。
    if tools.tool == ToolKind::Paste {
        match action {
            ContextMenuAction::PasteRotateCcw => paste.rot = (paste.rot + 3) % 4,
            ContextMenuAction::PasteRotateCw => paste.rot = (paste.rot + 1) % 4,
            ContextMenuAction::PasteFlipX => paste.flip_x = !paste.flip_x,
            ContextMenuAction::PasteFlipY => paste.flip_y = !paste.flip_y,
            _ => {}
        }
        info!(
            "paste transform changed (tool=Paste): rot={} ({}deg), flip_x={}, flip_y={}",
            paste.rot % 4,
            (paste.rot as u32 % 4) * 90,
            paste.flip_x,
            paste.flip_y
        );
        return;
    }

    // 选择工具且存在选区：优先对“选区内容”做旋转/翻转/重置（更接近 RM 的使用习惯）。
    if tools.tool == ToolKind::Select {
        if selection.rect.is_some() {
            if let Some(mut map) = map_opt.take() {
                let layer = layer_state.active.min(map.layers.saturating_sub(1));
                let applied = super::selection_transform::apply_selection_transform(
                    action,
                    &mut selection,
                    &mut map,
                    layer,
                    &config,
                    &mut render,
                    &mut undo,
                );
                map_opt = Some(map);
                if applied {
                    info!("selection transform applied: {:?}", action);
                    return;
                }
            }
        }
    }

    // 非粘贴模式：优先作用于鼠标指向的“已有图块”（只要格子里有 tile）。
    let mut did_tile = false;
    let map_pos = (|| {
        let Ok(window) = windows.single() else {
            return None;
        };
        let Some(tile_entities) = render.tile_entities.as_deref() else {
            return None;
        };
        let Ok((camera, camera_transform)) = camera_q.single() else {
            return None;
        };
        cursor_tile_pos(
            window,
            camera,
            camera_transform,
            &config,
            tile_entities.width,
            tile_entities.height,
        )
    })();

    if map_pos.is_some() {
        did_tile = match action {
            ContextMenuAction::PasteRotateCcw => {
                try_rotate_map_tile_ccw(map_pos, map_opt, &mut render, &config, &mut undo)
            }
            ContextMenuAction::PasteRotateCw => {
                try_rotate_map_tile_cw(map_pos, map_opt, &mut render, &config, &mut undo)
            }
            ContextMenuAction::PasteFlipX => {
                try_flip_map_tile_x(map_pos, map_opt, &mut render, &config, &mut undo)
            }
            ContextMenuAction::PasteFlipY => {
                try_flip_map_tile_y(map_pos, map_opt, &mut render, &config, &mut undo)
            }
            _ => false,
        };
    }

    if did_tile {
        info!("map tile transform changed at {:?}", map_pos);
        return;
    }

    // 兜底：若剪贴板有内容，则改“预设粘贴变换”（允许先旋转再 Ctrl+V）。
    if clipboard.width == 0 || clipboard.height == 0 || clipboard.tiles.is_empty() {
        return;
    }

    match action {
        ContextMenuAction::PasteRotateCcw => paste.rot = (paste.rot + 3) % 4,
        ContextMenuAction::PasteRotateCw => paste.rot = (paste.rot + 1) % 4,
        ContextMenuAction::PasteFlipX => paste.flip_x = !paste.flip_x,
        ContextMenuAction::PasteFlipY => paste.flip_y = !paste.flip_y,
        _ => {}
    }
    info!(
        "paste preset transform changed (tool={:?}): rot={} ({}deg), flip_x={}, flip_y={}",
        tools.tool,
        paste.rot % 4,
        (paste.rot as u32 % 4) * 90,
        paste.flip_x,
        paste.flip_y
    );
}

pub(super) fn apply_context_menu_paste_action(
    action: ContextMenuAction,
    tools: &mut ToolState,
    paste: &mut PasteState,
    clipboard: &Clipboard,
    menu: &ContextMenuState,
    config: &EditorConfig,
    map: Option<ResMut<TileMapData>>,
    render: &mut TilemapRenderParams,
    undo: &mut UndoStack,
) {
    match action {
        ContextMenuAction::PasteRotateCcw => {
            // 粘贴模式下：调整粘贴预览/落地变换；非粘贴模式：若右键指向某个已有图块，则旋转该图块。
            if tools.tool == ToolKind::Paste {
                paste.rot = (paste.rot + 3) % 4;
                info!("context cmd: rotate ccw (paste) -> rot={}", paste.rot % 4);
                return;
            }
            if try_rotate_map_tile_ccw(menu.map_pos, map, render, config, undo) {
                info!("context cmd: rotate ccw (tile)");
                return;
            }
            if clipboard.width > 0 && clipboard.height > 0 && !clipboard.tiles.is_empty() {
                paste.rot = (paste.rot + 3) % 4;
                info!("context cmd: rotate ccw (preset) -> rot={}", paste.rot % 4);
            }
        }
        ContextMenuAction::PasteRotateCw => {
            if tools.tool == ToolKind::Paste {
                paste.rot = (paste.rot + 1) % 4;
                info!("context cmd: rotate cw (paste) -> rot={}", paste.rot % 4);
                return;
            }
            if try_rotate_map_tile_cw(menu.map_pos, map, render, config, undo) {
                info!("context cmd: rotate cw (tile)");
                return;
            }
            if clipboard.width > 0 && clipboard.height > 0 && !clipboard.tiles.is_empty() {
                paste.rot = (paste.rot + 1) % 4;
                info!("context cmd: rotate cw (preset) -> rot={}", paste.rot % 4);
            }
        }
        ContextMenuAction::PasteFlipX => {
            if tools.tool == ToolKind::Paste {
                paste.flip_x = !paste.flip_x;
                info!("context cmd: flip x (paste) -> {}", paste.flip_x);
                return;
            }
            if try_flip_map_tile_x(menu.map_pos, map, render, config, undo) {
                info!("context cmd: flip x (tile)");
                return;
            }
            if clipboard.width > 0 && clipboard.height > 0 && !clipboard.tiles.is_empty() {
                paste.flip_x = !paste.flip_x;
                info!("context cmd: flip x (preset) -> {}", paste.flip_x);
            }
        }
        ContextMenuAction::PasteFlipY => {
            if tools.tool == ToolKind::Paste {
                paste.flip_y = !paste.flip_y;
                info!("context cmd: flip y (paste) -> {}", paste.flip_y);
                return;
            }
            if try_flip_map_tile_y(menu.map_pos, map, render, config, undo) {
                info!("context cmd: flip y (tile)");
                return;
            }
            if clipboard.width > 0 && clipboard.height > 0 && !clipboard.tiles.is_empty() {
                paste.flip_y = !paste.flip_y;
                info!("context cmd: flip y (preset) -> {}", paste.flip_y);
            }
        }
        ContextMenuAction::PasteReset => {
            if tools.tool == ToolKind::Paste {
                *paste = PasteState::default();
                info!("context cmd: paste reset");
                return;
            }
            if try_reset_map_tile_transform(menu.map_pos, map, render, config, undo) {
                info!("context cmd: tile transform reset");
                return;
            }
            *paste = PasteState::default();
            info!("context cmd: preset transform reset");
        }
        ContextMenuAction::ExitPaste => {
            let back = tools.return_after_paste.take().unwrap_or(ToolKind::Select);
            tools.tool = back;
            info!("context cmd: exit paste -> back to {:?}", back);
        }
        _ => {}
    }
}
