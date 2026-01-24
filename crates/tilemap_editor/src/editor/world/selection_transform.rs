use bevy::prelude::*;

use std::collections::HashSet;

use crate::editor::types::{
    CellChange, ContextMenuAction, EditCommand, EditorConfig, SelectionRect, SelectionState,
    TileMapData, TileRef, UndoStack,
};

use super::{apply_tile_change, TilemapRenderParams};

fn mat_mul(a: [[i32; 2]; 2], b: [[i32; 2]; 2]) -> [[i32; 2]; 2] {
    [
        [a[0][0] * b[0][0] + a[0][1] * b[1][0], a[0][0] * b[0][1] + a[0][1] * b[1][1]],
        [a[1][0] * b[0][0] + a[1][1] * b[1][0], a[1][0] * b[0][1] + a[1][1] * b[1][1]],
    ]
}

fn rot_mat_cw(rot: u8) -> [[i32; 2]; 2] {
    match rot % 4 {
        0 => [[1, 0], [0, 1]],
        1 => [[0, 1], [-1, 0]],
        2 => [[-1, 0], [0, -1]],
        _ => [[0, -1], [1, 0]],
    }
}

fn flip_mat(flip_x: bool, flip_y: bool) -> [[i32; 2]; 2] {
    let sx = if flip_x { -1 } else { 1 };
    let sy = if flip_y { -1 } else { 1 };
    [[sx, 0], [0, sy]]
}

fn tile_orientation_mat(rot: u8, flip_x: bool, flip_y: bool) -> [[i32; 2]; 2] {
    // 渲染约定：Sprite 先 flip（本地坐标），再由 Transform 做旋转。
    // 因此矩阵为：M = R(rot_cw) * F(flip_x, flip_y)
    mat_mul(rot_mat_cw(rot), flip_mat(flip_x, flip_y))
}

fn mat_to_tile_orientation(m: [[i32; 2]; 2]) -> Option<(u8, bool, bool)> {
    for rot in 0u8..=3 {
        for flip_x in [false, true] {
            for flip_y in [false, true] {
                if tile_orientation_mat(rot, flip_x, flip_y) == m {
                    return Some((rot, flip_x, flip_y));
                }
            }
        }
    }
    None
}

fn apply_group_transform_to_tile(tile: &mut TileRef, group: [[i32; 2]; 2]) {
    let m = tile_orientation_mat(tile.rot, tile.flip_x, tile.flip_y);
    let m2 = mat_mul(group, m);
    if let Some((rot, flip_x, flip_y)) = mat_to_tile_orientation(m2) {
        tile.rot = rot;
        tile.flip_x = flip_x;
        tile.flip_y = flip_y;
    }
}

fn selection_rotate_cw_mapping(sx: u32, sy: u32, _w: u32, h: u32) -> (u32, u32) {
    // (sx,sy) in w*h -> (dx,dy) in h*w
    (h - 1 - sy, sx)
}

fn selection_rotate_ccw_mapping(sx: u32, sy: u32, w: u32, _h: u32) -> (u32, u32) {
    // (sx,sy) in w*h -> (dx,dy) in h*w
    (sy, w - 1 - sx)
}

fn selection_flip_x_mapping(sx: u32, sy: u32, w: u32, _h: u32) -> (u32, u32) {
    (w - 1 - sx, sy)
}

fn selection_flip_y_mapping(sx: u32, sy: u32, _w: u32, h: u32) -> (u32, u32) {
    (sx, h - 1 - sy)
}

pub(super) fn apply_selection_transform(
    action: ContextMenuAction,
    selection: &mut SelectionState,
    map: &mut TileMapData,
    layer: u32,
    config: &EditorConfig,
    render: &mut TilemapRenderParams,
    undo: &mut UndoStack,
) -> bool {
    let Some(rect) = selection.rect else {
        return false;
    };
    let w = rect.width();
    let h = rect.height();
    if w == 0 || h == 0 {
        return false;
    }

    let (new_w, new_h) = match action {
        ContextMenuAction::PasteRotateCw | ContextMenuAction::PasteRotateCcw => (h, w),
        _ => (w, h),
    };

    let new_max_x = rect.min.x + new_w - 1;
    let new_max_y = rect.min.y + new_h - 1;
    if new_max_x >= map.width || new_max_y >= map.height {
        return false;
    }
    let new_rect = SelectionRect {
        min: rect.min,
        max: UVec2::new(new_max_x, new_max_y),
    };

    // 读取源 buffer（w*h）
    let mut src: Vec<Option<TileRef>> = Vec::with_capacity((w * h) as usize);
    for sy in 0..h {
        for sx in 0..w {
            let x = rect.min.x + sx;
            let y = rect.min.y + sy;
            let idx = map.idx_layer(layer, x, y);
            src.push(map.tiles[idx].clone());
        }
    }

    // 生成 dst buffer（new_w*new_h）
    let mut dst: Vec<Option<TileRef>> = vec![None; (new_w * new_h) as usize];

    let (mapping, group_mat): (fn(u32, u32, u32, u32) -> (u32, u32), [[i32; 2]; 2]) = match action {
        ContextMenuAction::PasteRotateCw => (selection_rotate_cw_mapping, rot_mat_cw(1)),
        ContextMenuAction::PasteRotateCcw => (selection_rotate_ccw_mapping, rot_mat_cw(3)),
        ContextMenuAction::PasteFlipX => (selection_flip_x_mapping, flip_mat(true, false)),
        ContextMenuAction::PasteFlipY => (selection_flip_y_mapping, flip_mat(false, true)),
        ContextMenuAction::PasteReset => (|sx, sy, _w, _h| (sx, sy), [[1, 0], [0, 1]]),
        _ => return false,
    };

    for sy in 0..h {
        for sx in 0..w {
            let i = (sy * w + sx) as usize;
            let mut tile = src[i].clone();
            if let Some(t) = tile.as_mut() {
                match action {
                    ContextMenuAction::PasteReset => {
                        t.rot = 0;
                        t.flip_x = false;
                        t.flip_y = false;
                    }
                    _ => {
                        apply_group_transform_to_tile(t, group_mat);
                    }
                }
            }

            let (dx, dy) = mapping(sx, sy, w, h);
            if dx < new_w && dy < new_h {
                dst[(dy * new_w + dx) as usize] = tile;
            }
        }
    }

    // 写回：只更新 old_rect ∪ new_rect。
    // 注意：两者“并集”的包围盒会包含额外格子（例如 5x2 旋转成 2x5），
    // 若直接遍历包围盒会误清空选区外的内容。
    let mut touched: HashSet<usize> = HashSet::new();
    let mut cmd = EditCommand::default();

    let mut apply_cell = |x: u32, y: u32, after: Option<TileRef>, map: &mut TileMapData, cmd: &mut EditCommand| {
        let idx = map.idx_layer(layer, x, y);
        if !touched.insert(idx) {
            return;
        }
        let before = map.tiles[idx].clone();
        if before != after {
            map.tiles[idx] = after.clone();
            cmd.changes.push(CellChange { idx, before, after });
        }
    };

    // 1) old_rect：不在 new_rect 的格子要清空；重叠格子写入 new_rect 的结果。
    for y in rect.min.y..=rect.max.y {
        for x in rect.min.x..=rect.max.x {
            let after = if x >= new_rect.min.x && x <= new_rect.max.x && y >= new_rect.min.y && y <= new_rect.max.y {
                let lx = x - new_rect.min.x;
                let ly = y - new_rect.min.y;
                dst[(ly * new_w + lx) as usize].clone()
            } else {
                None
            };
            apply_cell(x, y, after, map, &mut cmd);
        }
    }

    // 2) new_rect：old_rect 外的新扩展区域也需要写入。
    for y in new_rect.min.y..=new_rect.max.y {
        for x in new_rect.min.x..=new_rect.max.x {
            let lx = x - new_rect.min.x;
            let ly = y - new_rect.min.y;
            let after = dst[(ly * new_w + lx) as usize].clone();
            apply_cell(x, y, after, map, &mut cmd);
        }
    }

    if cmd.changes.is_empty() {
        // 即使没有地图改动，也认为“选区变换”被处理了，避免继续把同一按键作用到单格/预设粘贴。
        if matches!(action, ContextMenuAction::PasteRotateCw | ContextMenuAction::PasteRotateCcw) {
            selection.rect = Some(new_rect);
            selection.start = new_rect.min;
            selection.current = new_rect.max;
        }
        return true;
    }

    let layer_len = map.layer_len();
    let layer_offset = (layer as usize) * layer_len;
    for ch in &cmd.changes {
        let local = ch.idx.saturating_sub(layer_offset);
        let x = (local % map.width as usize) as u32;
        let y = (local / map.width as usize) as u32;
        apply_tile_change(render, config, layer, x, y, &ch.before, &ch.after);
    }
    undo.push(cmd);

    if matches!(action, ContextMenuAction::PasteRotateCw | ContextMenuAction::PasteRotateCcw) {
        selection.rect = Some(new_rect);
        selection.start = new_rect.min;
        selection.current = new_rect.max;
    }
    true
}
