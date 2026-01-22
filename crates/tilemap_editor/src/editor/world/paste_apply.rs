use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{
    CellChange, Clipboard, ContextMenuState, EditCommand, EditorConfig, LayerState, PasteState,
    TileEntities, TileMapData, TileRef, TilesetRuntime, ToolKind, ToolState, UndoStack, WorldCamera,
};

use super::{apply_tile_visual, cursor_tile_pos};
use super::paste_helpers::{paste_dims, paste_dst_xy};

#[derive(SystemParam)]
pub(in crate::editor) struct PasteWithMouseParams<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    tools: ResMut<'w, ToolState>,
    layer_state: Res<'w, LayerState>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera_q: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<WorldCamera>>,
    config: Res<'w, EditorConfig>,
    runtime: Res<'w, TilesetRuntime>,
    clipboard: Res<'w, Clipboard>,
    paste: Res<'w, PasteState>,
    menu: Res<'w, ContextMenuState>,
    map: Option<ResMut<'w, TileMapData>>,
    tile_entities: Option<Res<'w, TileEntities>>,
    tiles_q: Query<'w, 's, (&'static mut Sprite, &'static mut Transform, &'static mut Visibility)>,
    undo: ResMut<'w, UndoStack>,
}

/// 粘贴模式：左键把 Clipboard 贴到鼠标所在格子（作为左上角），并生成 Undo。
pub fn paste_with_mouse(params: PasteWithMouseParams) {
    let PasteWithMouseParams {
        buttons,
        keys,
        mut tools,
        layer_state,
        windows,
        camera_q,
        config,
        runtime,
        clipboard,
        paste,
        menu,
        map,
        tile_entities,
        mut tiles_q,
        mut undo,
    } = params;

    if tools.tool != ToolKind::Paste {
        return;
    }
    if menu.open || menu.consume_left_click {
        return;
    }
    if keys.pressed(KeyCode::Space) {
        return;
    }
    if clipboard.width == 0 || clipboard.height == 0 || clipboard.tiles.is_empty() {
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };

    let Some(pos) = cursor_tile_pos(
        window,
        camera,
        camera_transform,
        &config,
        tile_entities.width,
        tile_entities.height,
    ) else {
        return;
    };

    info!(
        "paste click at map ({}, {}): clipboard {}x{} tiles={} | rot={} flip_x={} flip_y={}",
        pos.x,
        pos.y,
        clipboard.width,
        clipboard.height,
        clipboard.tiles.len(),
        paste.rot % 4,
        paste.flip_x,
        paste.flip_y
    );

    let mut cmd = EditCommand::default();
    let layer = layer_state.active.min(map.layers.saturating_sub(1));
    let (pw, ph) = paste_dims(&clipboard, &paste);
    let mut attempted = 0u32;
    let mut oob = 0u32;
    let mut same = 0u32;
    let mut sampled = 0u32;

    // 遍历源剪贴板 → 映射到变换后的目标坐标（这样旋转/翻转更直观且不易写错）。
    for sy in 0..clipboard.height {
        for sx in 0..clipboard.width {
            let Some((cx, cy)) = paste_dst_xy(sx, sy, &clipboard, &paste) else {
                continue;
            };
            debug_assert!(cx < pw && cy < ph);
            attempted += 1;

            let dst_x = pos.x + cx;
            let dst_y = pos.y + cy;
            if dst_x >= tile_entities.width || dst_y >= tile_entities.height {
                oob += 1;
                continue;
            }

            let src_idx = (sy * clipboard.width + sx) as usize;
            let after = clipboard.tiles.get(src_idx).cloned().unwrap_or(None);
            let dst_idx = map.idx_layer(layer, dst_x, dst_y);
            if map.tiles[dst_idx] == after {
                same += 1;
                continue;
            }

            if sampled < 8 {
                sampled += 1;
                let after_label = match &after {
                    Some(t) => format!("{}:{}", t.tileset_id, t.index),
                    None => "None".to_string(),
                };
                let before_label = match &map.tiles[dst_idx] {
                    Some(t) => format!("{}:{}", t.tileset_id, t.index),
                    None => "None".to_string(),
                };
                info!(
                    "paste sample: src({},{}) -> local({},{}) -> dst({},{}) before={} after={}",
                    sx, sy, cx, cy, dst_x, dst_y, before_label, after_label
                );
            }

            let before = map.tiles[dst_idx].clone();
            map.tiles[dst_idx] = after.clone();
            cmd.changes.push(CellChange {
                idx: dst_idx,
                before,
                after,
            });
        }
    }

    if cmd.changes.is_empty() {
        info!(
            "paste result: no changes (attempted={}, oob={}, same={}) pw={} ph={}",
            attempted, oob, same, pw, ph
        );
        return;
    }

    info!(
        "paste result: changes={} (attempted={}, oob={}, same={}) pw={} ph={}",
        cmd.changes.len(),
        attempted,
        oob,
        same,
        pw,
        ph
    );

    // 局部刷新渲染
    let mut missing_atlas = 0u32;
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
            if let Some(TileRef { tileset_id, .. }) = &ch.after {
                if runtime.by_id.get(tileset_id).is_none() {
                    missing_atlas += 1;
                }
            }
            apply_tile_visual(&runtime, &ch.after, &mut sprite, &mut tf, &mut vis, &config);
        }
    }

    if missing_atlas > 0 {
        warn!(
            "paste: {} tiles refer to missing tileset atlas (likely not loaded)",
            missing_atlas
        );
    }

    undo.push(cmd);

    // 贴完后的工具行为：
    // - Ctrl+V/菜单进入的“临时粘贴”（return_after_paste 有值）：贴一次就自动回原工具。
    // - 用户显式切到 Paste：允许连续多次粘贴，按 Esc/菜单退出。
    if let Some(back) = tools.return_after_paste.take() {
        tools.tool = back;
    }
}
