//! 世界（World）侧逻辑：相机、鼠标绘制、快捷键、数据 ↔ 渲染同步。
//!
//! 关键点：
//! - 鼠标绘制必须把屏幕坐标转换为世界坐标，需要一个明确的“世界相机”。
//! - UI 体系可能引入额外相机或渲染路径，若使用 `Query<(&Camera, &GlobalTransform)>::single()`
//!   会在多相机时失败，从而导致右侧无法绘制。

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use bevy::window::PrimaryWindow;
use bevy::ecs::message::MessageReader;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use super::persistence::{load_map_from_file, save_map_to_file};
use super::tileset::{merge_tilesets_from_map, rect_for_tile_index, save_tileset_library};
use super::types::{
    CellChange, EditCommand, EditorConfig, EditorState, PanState, TileEntities, TileMapData, TileRef,
    Clipboard, ContextMenuAction, ContextMenuCommand, ContextMenuState, MapSizeFocus, MapSizeInput, PasteState,
    SelectionRect, SelectionState, PastePreview, PastePreviewTile, SelectionMovePreviewTile, ShiftMapMode, ShiftMapSettings,
    TilesetLibrary, TilesetLoading, TilesetRuntime, ToolKind, ToolState, UndoStack,
    WorldCamera,
};
use super::{LEFT_PANEL_WIDTH_PX, RIGHT_TOPBAR_HEIGHT_PX};

fn apply_tile_visual(
    runtime: &TilesetRuntime,
    tile: &Option<TileRef>,
    sprite: &mut Sprite,
    tf: &mut Transform,
    vis: &mut Visibility,
    config: &EditorConfig,
) {
    match tile {
        Some(TileRef {
            tileset_id,
            index,
            rot,
            flip_x,
            flip_y,
        }) => {
            let Some(atlas) = runtime.by_id.get(tileset_id) else {
                sprite.rect = None;
                sprite.flip_x = false;
                sprite.flip_y = false;
                tf.rotation = Quat::IDENTITY;
                *vis = Visibility::Hidden;
                return;
            };
            sprite.image = atlas.texture.clone();
            sprite.rect = Some(rect_for_tile_index(*index, atlas.columns, config.tile_size));
            sprite.flip_x = *flip_x;
            sprite.flip_y = *flip_y;
            let r = (*rot % 4) as f32;
            tf.rotation = Quat::from_rotation_z(-r * std::f32::consts::FRAC_PI_2);
            *vis = Visibility::Visible;
        }
        None => {
            sprite.flip_x = false;
            sprite.flip_y = false;
            tf.rotation = Quat::IDENTITY;
            *vis = Visibility::Hidden;
        }
    }
}

fn try_edit_single_map_tile<F>(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
    editor: F,
) -> bool
where
    F: FnOnce(&mut TileRef),
{
    let (Some(pos), Some(mut map), Some(tile_entities)) = (map_pos, map, tile_entities) else {
        return false;
    };
    let idx = map.idx(pos.x, pos.y);
    if idx >= map.tiles.len() {
        return false;
    }
    let Some(mut after_tile) = map.tiles[idx].clone() else {
        return false;
    };
    let before = map.tiles[idx].clone();
    editor(&mut after_tile);
    after_tile.rot %= 4;
    let after = Some(after_tile.clone());
    if before == after {
        return false;
    }
    map.tiles[idx] = after.clone();
    undo.push(EditCommand {
        changes: vec![CellChange { idx, before, after: after.clone() }],
    });

    let entity = tile_entities.entities[idx];
    if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
        apply_tile_visual(runtime, &after, &mut sprite, &mut tf, &mut vis, config);
    }
    true
}

fn try_rotate_map_tile_ccw(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| t.rot = (t.rot + 3) % 4,
    )
}

fn try_rotate_map_tile_cw(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| t.rot = (t.rot + 1) % 4,
    )
}

fn try_flip_map_tile_x(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| t.flip_x = !t.flip_x,
    )
}

fn try_flip_map_tile_y(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| t.flip_y = !t.flip_y,
    )
}

fn try_reset_map_tile_transform(
    map_pos: Option<UVec2>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<&TileEntities>,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    undo: &mut UndoStack,
) -> bool {
    try_edit_single_map_tile(
        map_pos,
        map,
        tile_entities,
        runtime,
        config,
        tiles_q,
        undo,
        |t| {
            t.rot = 0;
            t.flip_x = false;
            t.flip_y = false;
        },
    )
}

fn cursor_tile_pos(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    config: &EditorConfig,
    map_w: u32,
    map_h: u32,
) -> Option<UVec2> {
    let cursor_pos = window.cursor_position()?;

    // 左侧 UI 面板区域不响应
    if cursor_pos.x <= LEFT_PANEL_WIDTH_PX {
        return None;
    }
    // 右侧顶部 UI 工具条区域不响应
    if cursor_pos.y <= RIGHT_TOPBAR_HEIGHT_PX {
        return None;
    }

    let world_pos = camera
        .viewport_to_world_2d(camera_transform, cursor_pos)
        .ok()?;

    let tile_w = config.tile_size.x as f32;
    let tile_h = config.tile_size.y as f32;
    if tile_w <= 0.0 || tile_h <= 0.0 {
        return None;
    }

    let x = (world_pos.x / tile_w).floor() as i32;
    let y = (world_pos.y / tile_h).floor() as i32;
    if x < 0 || y < 0 {
        return None;
    }
    let (x, y) = (x as u32, y as u32);
    if x >= map_w || y >= map_h {
        return None;
    }

    Some(UVec2::new(x, y))
}

fn rect_from_two(a: UVec2, b: UVec2) -> SelectionRect {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = a.x.max(b.x);
    let max_y = a.y.max(b.y);
    SelectionRect {
        min: UVec2::new(min_x, min_y),
        max: UVec2::new(max_x, max_y),
    }
}

fn paste_dims(clipboard: &Clipboard, paste: &PasteState) -> (u32, u32) {
    let rot = paste.rot % 4;
    if rot == 1 || rot == 3 {
        (clipboard.height, clipboard.width)
    } else {
        (clipboard.width, clipboard.height)
    }
}

fn paste_dst_xy(sx: u32, sy: u32, clipboard: &Clipboard, paste: &PasteState) -> Option<(u32, u32)> {
    if sx >= clipboard.width || sy >= clipboard.height {
        return None;
    }

    // 约定：先旋转（顺时针 rot），再在“旋转后坐标系”里翻转。
    let rot = paste.rot % 4;
    let (pw, ph) = paste_dims(clipboard, paste);

    let (mut x, mut y) = match rot {
        0 => (sx, sy),
        // 90° CW: (x, y) -> (h-1-y, x)
        1 => (clipboard.height - 1 - sy, sx),
        // 180°: (x, y) -> (w-1-x, h-1-y)
        2 => (clipboard.width - 1 - sx, clipboard.height - 1 - sy),
        // 270° CW: (x, y) -> (y, w-1-x)
        3 => (sy, clipboard.width - 1 - sx),
        _ => (sx, sy),
    };

    if paste.flip_x {
        x = pw - 1 - x;
    }
    if paste.flip_y {
        y = ph - 1 - y;
    }

    if x >= pw || y >= ph {
        return None;
    }
    Some((x, y))
}

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
        3 => [[0, -1], [1, 0]],
        _ => [[1, 0], [0, 1]],
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
    } else {
        // 理论上不会发生（我们枚举了所有组合）。兜底不改变。
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

fn apply_selection_transform(
    action: ContextMenuAction,
    selection: &mut SelectionState,
    map: &mut TileMapData,
    tile_entities: &TileEntities,
    runtime: &TilesetRuntime,
    config: &EditorConfig,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
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
            src.push(map.tiles[map.idx(x, y)].clone());
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
        let idx = map.idx(x, y);
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

    for ch in &cmd.changes {
        let entity = tile_entities.entities[ch.idx];
        if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
            apply_tile_visual(runtime, &ch.after, &mut sprite, &mut tf, &mut vis, config);
        }
    }
    undo.push(cmd);

    if matches!(action, ContextMenuAction::PasteRotateCw | ContextMenuAction::PasteRotateCcw) {
        selection.rect = Some(new_rect);
        selection.start = new_rect.min;
        selection.current = new_rect.max;
    }
    true
}

fn tile_world_center(x: u32, y: u32, tile_size: UVec2, z: f32) -> Vec3 {
    let tile_w = tile_size.x as f32;
    let tile_h = tile_size.y as f32;
    let world_x = (x as f32 + 0.5) * tile_w;
    let world_y = (y as f32 + 0.5) * tile_h;
    Vec3::new(world_x, world_y, z)
}

#[derive(Default)]
pub(crate) struct SelectionMoveDrag {
    active: bool,
    copy: bool,
    start: UVec2,
    current: UVec2,
    rect: SelectionRect,
    buf: Vec<Option<TileRef>>,
	preview_entities: Vec<Entity>,
	preview_dims: (u32, u32),
}

fn point_in_rect(p: UVec2, r: SelectionRect) -> bool {
    p.x >= r.min.x && p.x <= r.max.x && p.y >= r.min.y && p.y <= r.max.y
}

fn clamp_i32(v: i32, lo: i32, hi: i32) -> i32 {
    v.max(lo).min(hi)
}

fn rect_shift(rect: SelectionRect, dx: i32, dy: i32) -> SelectionRect {
    let min_x = (rect.min.x as i32 + dx) as u32;
    let min_y = (rect.min.y as i32 + dy) as u32;
    let max_x = (rect.max.x as i32 + dx) as u32;
    let max_y = (rect.max.y as i32 + dy) as u32;
    SelectionRect {
        min: UVec2::new(min_x, min_y),
        max: UVec2::new(max_x, max_y),
    }
}

/// 选区内容拖拽：在 Select 工具下，按住左键在选区内拖动来移动内容；按住 Ctrl 拖动为复制移动。
///
/// - 拖拽中显示半透明“幽灵预览”
/// - 松开鼠标提交 Undo
pub fn selection_move_with_mouse(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tools: Res<ToolState>,
    menu: Res<ContextMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    runtime: Res<TilesetRuntime>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility), Without<SelectionMovePreviewTile>>,
    mut undo: ResMut<UndoStack>,
    mut selection: ResMut<SelectionState>,
    mut preview_q: Query<(&mut Sprite, &mut Transform, &mut Visibility), With<SelectionMovePreviewTile>>,
    mut drag: Local<SelectionMoveDrag>,
) {
    // 默认隐藏预览（若需要会在后续显示）。
    if !drag.active {
        for &e in &drag.preview_entities {
            if let Ok((_s, _t, mut v)) = preview_q.get_mut(e) {
                *v = Visibility::Hidden;
            }
        }
        selection.moving = false;
    }

    if tools.tool != ToolKind::Select {
        drag.active = false;
        selection.moving = false;
        return;
    }
    if menu.open || menu.consume_left_click {
        drag.active = false;
        selection.moving = false;
        return;
    }
    // Space 用于平移（Space + 左键拖拽）。
    if keys.pressed(KeyCode::Space) {
        drag.active = false;
        selection.moving = false;
        return;
    }
    // Alt+拖拽保留给“从任意工具框选”。
    if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
        drag.active = false;
        selection.moving = false;
        return;
    }

    let Some(rect) = selection.rect else {
        drag.active = false;
        selection.moving = false;
        return;
    };
    let Some(mut map) = map else {
        drag.active = false;
        selection.moving = false;
        return;
    };
    let Some(tile_entities) = tile_entities else {
        drag.active = false;
        selection.moving = false;
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };

    let left_start = buttons.just_pressed(MouseButton::Left);
    let left_down = buttons.pressed(MouseButton::Left);
    let left_end = buttons.just_released(MouseButton::Left);

    let pos = cursor_tile_pos(
        window,
        camera,
        camera_transform,
        &config,
        tile_entities.width,
        tile_entities.height,
    );

    // 开始拖拽：必须点击在当前选区内。
    if !drag.active {
        if !left_start {
            return;
        }
        let Some(pos) = pos else {
            return;
        };
        if !point_in_rect(pos, rect) {
            return;
        }

        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        drag.active = true;
        drag.copy = ctrl;
        drag.start = pos;
        drag.current = pos;
        drag.rect = rect;

        let w = rect.width();
        let h = rect.height();
        drag.buf.clear();
        drag.buf.reserve((w * h) as usize);
        for y in rect.min.y..=rect.max.y {
            for x in rect.min.x..=rect.max.x {
                drag.buf.push(map.tiles[map.idx(x, y)].clone());
            }
        }

        selection.moving = true;
    }

    if !drag.active {
        return;
    }

    // 拖拽中：更新 current（仅当鼠标在画布内）。
    if let Some(pos) = pos {
        if left_down {
            drag.current = pos;
        }
    }

    // 计算偏移，并限制在地图范围内。
    let mut dx = drag.current.x as i32 - drag.start.x as i32;
    let mut dy = drag.current.y as i32 - drag.start.y as i32;
    let min_dx = -(drag.rect.min.x as i32);
    let max_dx = map.width as i32 - 1 - drag.rect.max.x as i32;
    let min_dy = -(drag.rect.min.y as i32);
    let max_dy = map.height as i32 - 1 - drag.rect.max.y as i32;
    dx = clamp_i32(dx, min_dx, max_dx);
    dy = clamp_i32(dy, min_dy, max_dy);

    let new_rect = rect_shift(drag.rect, dx, dy);
    let w = drag.rect.width();
    let h = drag.rect.height();

    // 确保预览实体数量匹配。
    let want = (w * h) as usize;
    if drag.preview_dims != (w, h) || drag.preview_entities.len() != want {
        for &e in &drag.preview_entities {
            commands.entity(e).despawn();
        }
        drag.preview_entities.clear();
        drag.preview_dims = (w, h);
        for _ in 0..want {
            let e = commands
                .spawn((
                    Sprite {
                        image: Handle::<Image>::default(),
                        rect: None,
                        color: Color::srgba(1.0, 1.0, 1.0, 0.60),
                        ..default()
                    },
                    Transform::from_translation(Vec3::ZERO),
                    Visibility::Hidden,
                    SelectionMovePreviewTile,
                ))
                .id();
            drag.preview_entities.push(e);
        }
    }

    // 更新预览 sprite：直接把 buf 按相对坐标贴到 new_rect。
    for cy in 0..h {
        for cx in 0..w {
            let i = (cy * w + cx) as usize;
            let e = drag.preview_entities[i];
            let Ok((mut sprite, mut tf, mut vis)) = preview_q.get_mut(e) else {
                continue;
            };
            let dst_x = new_rect.min.x + cx;
            let dst_y = new_rect.min.y + cy;
            tf.translation = tile_world_center(dst_x, dst_y, config.tile_size, 6.0);
            apply_tile_visual(&runtime, &drag.buf[i], &mut sprite, &mut tf, &mut vis, &config);
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.60);
        }
    }

    // 结束拖拽：提交变更。
    let ended = left_end || !left_down;
    if !ended {
        return;
    }

    // 隐藏预览
    for &e in &drag.preview_entities {
        if let Ok((_s, _t, mut v)) = preview_q.get_mut(e) {
            *v = Visibility::Hidden;
        }
    }

    let mut cmd = EditCommand::default();
    let mut touched: HashSet<usize> = HashSet::new();

    // helper：写一个格子的 after，并记录变更
    let mut apply_cell = |x: u32, y: u32, after: Option<TileRef>, map: &mut TileMapData, cmd: &mut EditCommand| {
        let idx = map.idx(x, y);
        if !touched.insert(idx) {
            return;
        }
        let before = map.tiles[idx].clone();
        if before != after {
            map.tiles[idx] = after.clone();
            cmd.changes.push(CellChange { idx, before, after });
        }
    };

    // 计算 new_rect 内某格对应的 buf tile
    let buf_at = |x: u32, y: u32, new_rect: SelectionRect, w: u32, buf: &Vec<Option<TileRef>>| -> Option<TileRef> {
        let lx = x - new_rect.min.x;
        let ly = y - new_rect.min.y;
        buf[(ly * w + lx) as usize].clone()
    };

    if drag.copy {
        // 复制移动：只写入 new_rect，原区域保留。
        for y in new_rect.min.y..=new_rect.max.y {
            for x in new_rect.min.x..=new_rect.max.x {
                let after = buf_at(x, y, new_rect, w, &drag.buf);
                apply_cell(x, y, after, &mut map, &mut cmd);
            }
        }
    } else {
        // 移动：更新 old_rect ∪ new_rect。
        for y in drag.rect.min.y..=drag.rect.max.y {
            for x in drag.rect.min.x..=drag.rect.max.x {
                let after = if point_in_rect(UVec2::new(x, y), new_rect) {
                    buf_at(x, y, new_rect, w, &drag.buf)
                } else {
                    None
                };
                apply_cell(x, y, after, &mut map, &mut cmd);
            }
        }
        for y in new_rect.min.y..=new_rect.max.y {
            for x in new_rect.min.x..=new_rect.max.x {
                let after = buf_at(x, y, new_rect, w, &drag.buf);
                apply_cell(x, y, after, &mut map, &mut cmd);
            }
        }
    }

    // 刷新渲染
    for ch in &cmd.changes {
        let entity = tile_entities.entities[ch.idx];
        if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
            apply_tile_visual(&runtime, &ch.after, &mut sprite, &mut tf, &mut vis, &config);
        }
    }

    undo.push(cmd);

    // 更新选择框
    selection.rect = Some(new_rect);
    selection.start = new_rect.min;
    selection.current = new_rect.max;

    drag.active = false;
    selection.moving = false;
}

/// 粘贴“幽灵预览”：在鼠标下方显示将要贴的图块（半透明），旋转/翻转会立即可见。
pub fn update_paste_preview(
    mut commands: Commands,
    tools: Res<ToolState>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    runtime: Res<TilesetRuntime>,
    clipboard: Res<Clipboard>,
    paste: Res<PasteState>,
    tile_entities: Option<Res<TileEntities>>,
    mut preview: ResMut<PastePreview>,
    mut q: Query<(&mut Sprite, &mut Transform, &mut Visibility), With<PastePreviewTile>>,
) {
    // 仅在粘贴工具下显示。
    if tools.tool != ToolKind::Paste {
        for &e in &preview.entities {
            if let Ok((_s, _t, mut v)) = q.get_mut(e) {
                *v = Visibility::Hidden;
            }
        }
        return;
    }
    if keys.pressed(KeyCode::Space) {
        return;
    }
    let Some(tile_entities) = tile_entities.as_deref() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };

    if clipboard.width == 0 || clipboard.height == 0 || clipboard.tiles.is_empty() {
        return;
    }

    let Some(pos) = cursor_tile_pos(
        window,
        camera,
        camera_transform,
        &config,
        tile_entities.width,
        tile_entities.height,
    ) else {
        for &e in &preview.entities {
            if let Ok((_s, _t, mut v)) = q.get_mut(e) {
                *v = Visibility::Hidden;
            }
        }
        return;
    };

    let (pw, ph) = paste_dims(&clipboard, &paste);
    let want = (pw * ph) as usize;

    if preview.dims != (pw, ph) || preview.entities.len() != want {
        for &e in &preview.entities {
            commands.entity(e).despawn();
        }
        preview.entities.clear();
        preview.dims = (pw, ph);

        for _ in 0..want {
            let e = commands
                .spawn((
                    Sprite {
                        image: Handle::<Image>::default(),
                        rect: None,
                        color: Color::srgba(1.0, 1.0, 1.0, 0.55),
                        ..default()
                    },
                    Transform::from_translation(Vec3::ZERO),
                    Visibility::Hidden,
                    PastePreviewTile,
                ))
                .id();
            preview.entities.push(e);
        }
    }

    // 先生成变换后的“目标局部格子”数组，再逐格写到 preview sprites。
    let mut transformed: Vec<Option<TileRef>> = vec![None; want];
    for sy in 0..clipboard.height {
        for sx in 0..clipboard.width {
            let Some((cx, cy)) = paste_dst_xy(sx, sy, &clipboard, &paste) else {
                continue;
            };
            let src_idx = (sy * clipboard.width + sx) as usize;
            let dst_idx = (cy * pw + cx) as usize;
            if dst_idx < transformed.len() {
                transformed[dst_idx] = clipboard.tiles.get(src_idx).cloned().unwrap_or(None);
            }
        }
    }

    for cy in 0..ph {
        for cx in 0..pw {
            let i = (cy * pw + cx) as usize;
            let e = preview.entities[i];
            let Ok((mut sprite, mut tf, mut vis)) = q.get_mut(e) else {
                continue;
            };

            let dst_x = pos.x + cx;
            let dst_y = pos.y + cy;
            if dst_x >= tile_entities.width || dst_y >= tile_entities.height {
                *vis = Visibility::Hidden;
                continue;
            }

            tf.translation = tile_world_center(dst_x, dst_y, config.tile_size, 5.0);
            apply_tile_visual(&runtime, &transformed[i], &mut sprite, &mut tf, &mut vis, &config);
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.55);
        }
    }
}

pub fn undo_redo_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut undo: ResMut<UndoStack>,
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };

    let want_undo = keys.just_pressed(KeyCode::KeyZ)
        && !(keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    let want_redo = keys.just_pressed(KeyCode::KeyY)
        || (keys.just_pressed(KeyCode::KeyZ)
            && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)));

    if want_undo {
        let Some(cmd) = undo.undo.pop() else {
            return;
        };
        for ch in &cmd.changes {
            if ch.idx < map.tiles.len() {
                map.tiles[ch.idx] = ch.before.clone();
            }
        }
        undo.redo.push(cmd);
        apply_map_to_entities(&runtime, &map, &tile_entities, &mut tiles_q, &config);
        return;
    }

    if want_redo {
        let Some(cmd) = undo.redo.pop() else {
            return;
        };
        for ch in &cmd.changes {
            if ch.idx < map.tiles.len() {
                map.tiles[ch.idx] = ch.after.clone();
            }
        }
        undo.undo.push(cmd);
        apply_map_to_entities(&runtime, &map, &tile_entities, &mut tiles_q, &config);
        return;
    }
}

/// 当地图尺寸变化时，把相机移动到地图中心（避免切换尺寸后内容在屏幕外）。
pub fn recenter_camera_on_map_change(
    config: Res<EditorConfig>,
    mut last_size: Local<UVec2>,
    mut cam_q: Query<&mut Transform, With<WorldCamera>>,
) {
    if *last_size == config.map_size {
        return;
    }

    *last_size = config.map_size;

    let Ok(mut tf) = cam_q.single_mut() else {
        return;
    };

    let cam_x = (config.map_size.x as f32 * config.tile_size.x as f32) * 0.5;
    let cam_y = (config.map_size.y as f32 * config.tile_size.y as f32) * 0.5;
    tf.translation.x = cam_x;
    tf.translation.y = cam_y;
}

/// 画布平移（拖拽）：中键拖动，或 Space + 左键拖动。
pub fn camera_pan(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    mut cam_tf_q: Query<&mut Transform, With<WorldCamera>>,
    mut pan: ResMut<PanState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        pan.active = false;
        pan.last_world = None;
        return;
    };

    // 左侧面板不触发平移
    if cursor.x <= LEFT_PANEL_WIDTH_PX {
        pan.active = false;
        pan.last_world = None;
        return;
    }

    // 右侧顶部 UI 工具条不触发平移
    if cursor.y <= RIGHT_TOPBAR_HEIGHT_PX {
        pan.active = false;
        pan.last_world = None;
        return;
    }

    let want_pan = buttons.pressed(MouseButton::Middle)
        || (keys.pressed(KeyCode::Space) && buttons.pressed(MouseButton::Left));

    if !want_pan {
        pan.active = false;
        pan.last_world = None;
        return;
    }

    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };

    if !pan.active {
        pan.active = true;
        pan.last_world = Some(world);
        return;
    }

    let Some(last_world) = pan.last_world else {
        pan.last_world = Some(world);
        return;
    };

    let delta = last_world - world;
    if delta.length_squared() > 0.0 {
        if let Ok(mut tf) = cam_tf_q.single_mut() {
            tf.translation.x += delta.x;
            tf.translation.y += delta.y;
        }
    }

    pan.last_world = Some(world);
}

/// 在画布上绘制辅助线（网格 + hover 高亮）。
///
/// 这会让右侧“全黑空白”的区域更像编辑器画布，同时帮助对齐绘制。
pub fn draw_canvas_helpers(
    mut gizmos: Gizmos,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    tile_entities: Option<Res<TileEntities>>,
	tools: Res<ToolState>,
	selection: Res<SelectionState>,
	clipboard: Res<Clipboard>,
    paste: Res<PasteState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };

    let tile_w = config.tile_size.x as f32;
    let tile_h = config.tile_size.y as f32;
    if tile_w <= 0.0 || tile_h <= 0.0 {
        return;
    }

    // 有 TileEntities 时使用其实际尺寸（更可靠）；否则回退到 config.map_size。
    let (map_w, map_h) = if let Some(te) = tile_entities.as_deref() {
        (te.width, te.height)
    } else {
        (config.map_size.x, config.map_size.y)
    };
    if map_w == 0 || map_h == 0 {
        return;
    }

    let width_px = map_w as f32 * tile_w;
    let height_px = map_h as f32 * tile_h;

    let grid_color = Color::srgba(1.0, 1.0, 1.0, 0.12);
    let border_color = Color::srgba(1.0, 1.0, 1.0, 0.30);

    // 网格线
    for x in 0..=map_w {
        let x_pos = x as f32 * tile_w;
        gizmos.line_2d(Vec2::new(x_pos, 0.0), Vec2::new(x_pos, height_px), grid_color);
    }
    for y in 0..=map_h {
        let y_pos = y as f32 * tile_h;
        gizmos.line_2d(Vec2::new(0.0, y_pos), Vec2::new(width_px, y_pos), grid_color);
    }

    // 边界加粗（用更亮的颜色再画一遍）
    gizmos.line_2d(Vec2::new(0.0, 0.0), Vec2::new(width_px, 0.0), border_color);
    gizmos.line_2d(
        Vec2::new(0.0, height_px),
        Vec2::new(width_px, height_px),
        border_color,
    );
    gizmos.line_2d(Vec2::new(0.0, 0.0), Vec2::new(0.0, height_px), border_color);
    gizmos.line_2d(
        Vec2::new(width_px, 0.0),
        Vec2::new(width_px, height_px),
        border_color,
    );

    // 选择框：不要求鼠标在画布内，避免“按了快捷键但光标在 UI 上看不到”。
    if let Some(rect) = selection.rect {
        let sx0 = rect.min.x as f32 * tile_w;
        let sy0 = rect.min.y as f32 * tile_h;
        let sx1 = (rect.max.x as f32 + 1.0) * tile_w;
        let sy1 = (rect.max.y as f32 + 1.0) * tile_h;
        let c = Color::srgba(1.0, 1.0, 0.0, 0.85);
        gizmos.line_2d(Vec2::new(sx0, sy0), Vec2::new(sx1, sy0), c);
        gizmos.line_2d(Vec2::new(sx1, sy0), Vec2::new(sx1, sy1), c);
        gizmos.line_2d(Vec2::new(sx1, sy1), Vec2::new(sx0, sy1), c);
        gizmos.line_2d(Vec2::new(sx0, sy1), Vec2::new(sx0, sy0), c);
    }

    // hover 格子高亮（仅在鼠标在右侧画布区域时）
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    if cursor_pos.x <= LEFT_PANEL_WIDTH_PX {
        return;
    }
    if cursor_pos.y <= RIGHT_TOPBAR_HEIGHT_PX {
        return;
    }

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    let x = (world_pos.x / tile_w).floor() as i32;
    let y = (world_pos.y / tile_h).floor() as i32;
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    if x >= map_w || y >= map_h {
        return;
    }

    let x0 = x as f32 * tile_w;
    let y0 = y as f32 * tile_h;
    let x1 = x0 + tile_w;
    let y1 = y0 + tile_h;
    let hover_color = Color::srgba(0.25, 0.45, 0.95, 0.85);
    gizmos.line_2d(Vec2::new(x0, y0), Vec2::new(x1, y0), hover_color);
    gizmos.line_2d(Vec2::new(x1, y0), Vec2::new(x1, y1), hover_color);
    gizmos.line_2d(Vec2::new(x1, y1), Vec2::new(x0, y1), hover_color);
    gizmos.line_2d(Vec2::new(x0, y1), Vec2::new(x0, y0), hover_color);

    // 粘贴预览（Paste 工具）：以鼠标所在格子为左上角
    if tools.tool == ToolKind::Paste && clipboard.width > 0 && clipboard.height > 0 {
        let Some(cursor_pos) = window.cursor_position() else {
            return;
        };
        if cursor_pos.x <= LEFT_PANEL_WIDTH_PX || cursor_pos.y <= RIGHT_TOPBAR_HEIGHT_PX {
            return;
        }
        let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
            return;
        };
        let px = (world_pos.x / tile_w).floor() as i32;
        let py = (world_pos.y / tile_h).floor() as i32;
        if px < 0 || py < 0 {
            return;
        }
        let (px, py) = (px as u32, py as u32);
        let (pw, ph) = paste_dims(&clipboard, &paste);
        let x1 = (px as f32 + pw as f32) * tile_w;
        let y1 = (py as f32 + ph as f32) * tile_h;
        let x0 = px as f32 * tile_w;
        let y0 = py as f32 * tile_h;
        let c = Color::srgba(0.2, 1.0, 0.2, 0.75);
        gizmos.line_2d(Vec2::new(x0, y0), Vec2::new(x1, y0), c);
        gizmos.line_2d(Vec2::new(x1, y0), Vec2::new(x1, y1), c);
        gizmos.line_2d(Vec2::new(x1, y1), Vec2::new(x0, y1), c);
        gizmos.line_2d(Vec2::new(x0, y1), Vec2::new(x0, y0), c);
    }
}

/// 世界相机缩放（右侧区域鼠标滚轮）。
///
/// `OrthographicProjection.scale` 越小越“放大”（zoom in），越大越“缩小”（zoom out）。
pub fn camera_zoom(
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut proj_q: Query<&mut Projection, With<WorldCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    // 左侧面板滚轮用于 UI 滚动，避免同时触发缩放
    if cursor.x <= LEFT_PANEL_WIDTH_PX {
        return;
    }

    // 右侧顶部 UI 工具条区域不触发缩放
    if cursor.y <= RIGHT_TOPBAR_HEIGHT_PX {
        return;
    }

    let mut delta: f32 = 0.0;
    for ev in wheel.read() {
        delta += ev.y;
    }
    if delta.abs() < f32::EPSILON {
        return;
    }

    let Ok(mut proj) = proj_q.single_mut() else {
        return;
    };

    // wheel up (positive) => zoom in => ortho.scale smaller
    let factor: f32 = (1.0 - delta * 0.1).clamp(0.5, 2.0);
    if let Projection::Orthographic(ref mut ortho) = *proj {
        ortho.scale = (ortho.scale * factor).clamp(0.25, 8.0);
    }
}

/// 初始化世界相机。
pub fn setup_world(mut commands: Commands, config: Res<EditorConfig>) {
    // 相机移动到地图中心，避免地图在屏幕外。
    let cam_x = (config.map_size.x as f32 * config.tile_size.x as f32) * 0.5;
    let cam_y = (config.map_size.y as f32 * config.tile_size.y as f32) * 0.5;
    commands.spawn((
        Camera2d,
        Transform::from_translation(Vec3::new(cam_x, cam_y, 1000.0)),
        WorldCamera,
    ));
}

/// 键盘快捷键：选择 tile（[ / ]）+ 清空地图（R）。
pub fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    mut state: ResMut<EditorState>,
    mut undo: ResMut<UndoStack>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    map: Option<ResMut<TileMapData>>,
    config: Res<EditorConfig>,
) {
    let tile_count = lib
        .active_id
        .as_ref()
        .and_then(|id| runtime.by_id.get(id))
        .map(|a| a.columns.saturating_mul(a.rows))
        .unwrap_or(1)
        .max(1);

    if keys.just_pressed(KeyCode::BracketLeft) {
        state.selected_tile = state.selected_tile.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        state.selected_tile = (state.selected_tile + 1).min(tile_count - 1);
    }

    // 清空地图（做成可 Undo 的命令）。
    if keys.just_pressed(KeyCode::KeyR) {
        if let (Some(tile_entities), Some(mut map)) = (tile_entities.as_deref(), map) {
            let mut changes: Vec<CellChange> = Vec::new();
            for (idx, cell) in map.tiles.iter_mut().enumerate() {
                let before = cell.clone();
                if before.is_some() {
                    *cell = None;
                    changes.push(CellChange {
                        idx,
                        before,
                        after: None,
                    });
                }
            }
            undo.push(EditCommand { changes });
            apply_map_to_entities(&runtime, &map, tile_entities, &mut tiles_q, &config);
        }
    }
}

/// 工具快捷键：1/2/3/4/5/6 切换（笔刷/矩形/填充/选择/粘贴/橡皮）。
pub fn tool_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    mut tools: ResMut<ToolState>,
) {
    // 正在输入地图尺寸时，数字键留给输入框。
    if input.focus != MapSizeFocus::None {
        return;
    }

    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        tools.tool = ToolKind::Pencil;
    } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
        tools.tool = ToolKind::Rect;
    } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
        tools.tool = ToolKind::Fill;
    } else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
        tools.tool = ToolKind::Select;
    } else if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5) {
        tools.tool = ToolKind::Paste;
	} else if keys.just_pressed(KeyCode::Digit6) || keys.just_pressed(KeyCode::Numpad6) {
		tools.tool = ToolKind::Eraser;
    }
}

/// 按住 I 临时切换为吸管（松开恢复到原工具）。
pub fn eyedropper_hold_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    mut tools: ResMut<ToolState>,
    mut prev: Local<Option<ToolKind>>,
) {
    if input.focus != MapSizeFocus::None {
        return;
    }

    if keys.just_pressed(KeyCode::KeyI) {
        if *prev == None {
            *prev = Some(tools.tool);
        }
        tools.tool = ToolKind::Eyedropper;
    }

    if keys.just_released(KeyCode::KeyI) {
        if let Some(back) = prev.take() {
            // 若用户手动点了吸管按钮，就不要强制切回
            if tools.tool == ToolKind::Eyedropper {
                tools.tool = back;
            }
        }
    }
}

/// 吸管工具：点击格子后把该格子的 tile 设为当前选择。
pub fn eyedropper_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tools: Res<ToolState>,
    menu: Res<ContextMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    map: Option<Res<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut state: ResMut<EditorState>,
    mut lib: ResMut<TilesetLibrary>,
) {
    if tools.tool != ToolKind::Eyedropper {
        return;
    }
    if menu.open || menu.consume_left_click {
        return;
    }
    // Alt + 拖拽用于“从任意工具框选”，避免与吸管冲突。
    if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
        return;
    }
    if keys.pressed(KeyCode::Space) {
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(map) = map else {
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

    let idx = map.idx(pos.x, pos.y);
    let Some(tile) = map.tiles.get(idx).cloned().flatten() else {
        return;
    };

    state.selected_tile = tile.index;
    lib.active_id = Some(tile.tileset_id.clone());
    if let Some(entry) = lib.entries.iter().find(|e| e.id == tile.tileset_id) {
        let cat = entry.category.trim();
        if !cat.is_empty() {
            lib.active_category = cat.to_string();
        }
    }
}

/// Shift Map：Ctrl + 方向键整体平移一格（空出来的格子填 None），并可撤销。
pub fn shift_map_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    settings: Res<ShiftMapSettings>,
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    mut undo: ResMut<UndoStack>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
) {
    // 输入框聚焦时不抢快捷键
    if input.focus != MapSizeFocus::None {
        return;
    }

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
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

    let w = map.width;
    let h = map.height;
    if w == 0 || h == 0 {
        return;
    }

    let mut new_tiles = vec![None; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (dst_x, dst_y) = match settings.mode {
                ShiftMapMode::Blank => {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    (nx as u32, ny as u32)
                }
                ShiftMapMode::Wrap => {
                    let nx = (x as i32 + dx).rem_euclid(w as i32) as u32;
                    let ny = (y as i32 + dy).rem_euclid(h as i32) as u32;
                    (nx, ny)
                }
            };

            let src = map.idx(x, y);
            let dst = map.idx(dst_x, dst_y);
            new_tiles[dst] = map.tiles[src].clone();
        }
    }

    let mut cmd = EditCommand::default();
    for i in 0..map.tiles.len() {
        let before = map.tiles[i].clone();
        let after = new_tiles[i].clone();
        if before != after {
            cmd.changes.push(CellChange { idx: i, before, after });
        }
    }

    if cmd.changes.is_empty() {
        return;
    }

    map.tiles = new_tiles;
    undo.push(cmd);
    apply_map_to_entities(&runtime, &map, &tile_entities, &mut tiles_q, &config);
}

/// 选择区移动：在 Select 工具下按 Alt + 方向键，把选择框内内容整体移动 1 格（可撤销）。
pub fn move_selection_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    tools: Res<ToolState>,
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
    for y in rect.min.y..=rect.max.y {
        for x in rect.min.x..=rect.max.x {
            buf.push(map.tiles[map.idx(x, y)].clone());
        }
    }

    let mut cmd = EditCommand::default();

    // 清空原区域
    for y in rect.min.y..=rect.max.y {
        for x in rect.min.x..=rect.max.x {
            let idx = map.idx(x, y);
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
            let idx = map.idx(dst_x, dst_y);
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
    for ch in &cmd.changes {
        let entity = tile_entities.entities[ch.idx];
        if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
            apply_tile_visual(&runtime, &ch.after, &mut sprite, &mut tf, &mut vis, &config);
        }
    }

    undo.push(cmd);
    selection.rect = Some(new_rect);
    selection.start = new_rect.min;
    selection.current = new_rect.max;
}

/// 选择工具：拖拽框选矩形。
pub fn select_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut tools: ResMut<ToolState>,
    menu: Res<ContextMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    tile_entities: Option<Res<TileEntities>>,
    mut selection: ResMut<SelectionState>,
) {
    // 正在拖拽移动选区内容时，不要同时开始框选。
    if selection.moving {
        return;
    }
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let allow_select = tools.tool == ToolKind::Select || (alt && tools.tool != ToolKind::Paste);
    if !allow_select {
        return;
    }
    if menu.open || menu.consume_left_click {
        return;
    }
    if keys.pressed(KeyCode::Space) {
        return;
    }
    let Some(tile_entities) = tile_entities else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };

    let left_start = buttons.just_pressed(MouseButton::Left);
    let left_down = buttons.pressed(MouseButton::Left);
    let left_end = buttons.just_released(MouseButton::Left);

    let pos = cursor_tile_pos(
        window,
        camera,
        camera_transform,
        &config,
        tile_entities.width,
        tile_entities.height,
    );

    if !selection.dragging {
        if left_start {
            let Some(pos) = pos else {
                return;
            };

            // Alt+拖拽：从其它工具进入后，切到选择工具。
            if alt && tools.tool != ToolKind::Select {
                tools.tool = ToolKind::Select;
            }

            selection.dragging = true;
            selection.start = pos;
            selection.current = pos;
            selection.rect = Some(rect_from_two(pos, pos));
        }
        return;
    }

    if selection.dragging {
        if let Some(pos) = pos {
            if left_down {
                selection.current = pos;
                selection.rect = Some(rect_from_two(selection.start, selection.current));
            }
        }
        if left_end || !left_down {
            selection.dragging = false;
            selection.rect = Some(rect_from_two(selection.start, selection.current));
        }
    }
}

/// Ctrl+C 复制选择区域到 Clipboard；Ctrl+V 进入粘贴模式；Esc 退出粘贴。
pub fn copy_paste_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    mut tools: ResMut<ToolState>,
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
			let back = tools.return_after_paste.take().unwrap_or(ToolKind::Select);
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
        let w = rect.width();
        let h = rect.height();
        let mut tiles = Vec::with_capacity((w * h) as usize);
        for y in rect.min.y..=rect.max.y {
            for x in rect.min.x..=rect.max.x {
                let idx = map.idx(x, y);
                tiles.push(map.tiles[idx].clone());
            }
        }
        clipboard.width = w;
        clipboard.height = h;
        clipboard.tiles = tiles;
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

/// 粘贴变换：Q/E 旋转，H/V 翻转。
pub fn paste_transform_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    tools: Res<ToolState>,
    clipboard: Res<Clipboard>,
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    mut selection: ResMut<SelectionState>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    mut undo: ResMut<UndoStack>,
    mut paste: ResMut<PasteState>,
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
            if let (Some(mut map), Some(tile_entities)) = (map_opt.take(), tile_entities.as_deref()) {
                let applied = apply_selection_transform(
                    action,
                    &mut selection,
                    &mut map,
                    tile_entities,
                    &runtime,
                    &config,
                    &mut tiles_q,
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
        let Some(tile_entities) = tile_entities.as_deref() else {
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
                try_rotate_map_tile_ccw(map_pos, map_opt, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo)
            }
            ContextMenuAction::PasteRotateCw => {
                try_rotate_map_tile_cw(map_pos, map_opt, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo)
            }
            ContextMenuAction::PasteFlipX => {
                try_flip_map_tile_x(map_pos, map_opt, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo)
            }
            ContextMenuAction::PasteFlipY => {
                try_flip_map_tile_y(map_pos, map_opt, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo)
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
        if pos.y <= RIGHT_TOPBAR_HEIGHT_PX {
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
        info!("context menu open at screen ({:.1}, {:.1}) tool={:?}", pos.x, pos.y, tools.tool);
    }

    // “点空白关闭”交给 UI 侧的 ContextMenuBackdrop 处理，避免 world 侧做不精确的 bounds 判断。
}

fn copy_selection_to_clipboard(rect: SelectionRect, map: &TileMapData, clipboard: &mut Clipboard) {
    let w = rect.width();
    let h = rect.height();
    let mut tiles = Vec::with_capacity((w * h) as usize);
    for y in rect.min.y..=rect.max.y {
        for x in rect.min.x..=rect.max.x {
            let idx = map.idx(x, y);
            tiles.push(map.tiles[idx].clone());
        }
    }
    clipboard.width = w;
    clipboard.height = h;
    clipboard.tiles = tiles;
}

fn clear_selection_to_none(
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
            let idx = map.idx(x, y);
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
    for ch in &cmd.changes {
        let entity = tile_entities.entities[ch.idx];
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
            let Some(mut map) = map else { return; };
            let Some(tile_entities) = tile_entities.as_deref() else { return; };

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
            let Some(mut map) = map else { return; };
            let Some(tile_entities) = tile_entities.as_deref() else { return; };

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
            let Some(map) = map.as_deref() else { return; };
            let Some(rect) = selection.rect else { return; };
            copy_selection_to_clipboard(rect, map, &mut clipboard);
            tools.tool = ToolKind::Select;
            info!("context cmd: selection copy {}x{}", clipboard.width, clipboard.height);
        }
        ContextMenuAction::SelectionCut => {
            let Some(mut map) = map else { return; };
            let Some(tile_entities) = tile_entities.as_deref() else { return; };
            let Some(rect) = selection.rect else { return; };
            copy_selection_to_clipboard(rect, &map, &mut clipboard);
            clear_selection_to_none(rect, &mut map, &runtime, &config, tile_entities, &mut tiles_q, &mut undo);
            tools.tool = ToolKind::Select;
            info!("context cmd: selection cut {}x{}", clipboard.width, clipboard.height);
        }
        ContextMenuAction::SelectionDelete => {
            let Some(mut map) = map else { return; };
            let Some(tile_entities) = tile_entities.as_deref() else { return; };
            let Some(rect) = selection.rect else { return; };
            clear_selection_to_none(rect, &mut map, &runtime, &config, tile_entities, &mut tiles_q, &mut undo);
            tools.tool = ToolKind::Select;
            info!("context cmd: selection delete");
        }
        ContextMenuAction::SelectionSelectAll => {
            let Some(map) = map.as_deref() else { return; };
            if map.width == 0 || map.height == 0 { return; }
            let rect = SelectionRect { min: UVec2::ZERO, max: UVec2::new(map.width - 1, map.height - 1) };
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
        ContextMenuAction::PasteRotateCcw => {
            // 粘贴模式下：调整粘贴预览/落地变换；非粘贴模式：若右键指向某个已有图块，则旋转该图块。
            if tools.tool == ToolKind::Paste {
                paste.rot = (paste.rot + 3) % 4;
                info!("context cmd: rotate ccw (paste) -> rot={}", paste.rot % 4);
                return;
            }
            if try_rotate_map_tile_ccw(menu.map_pos, map, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo) {
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
            if try_rotate_map_tile_cw(menu.map_pos, map, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo) {
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
            if try_flip_map_tile_x(menu.map_pos, map, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo) {
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
            if try_flip_map_tile_y(menu.map_pos, map, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo) {
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
            if try_reset_map_tile_transform(menu.map_pos, map, tile_entities.as_deref(), &runtime, &config, &mut tiles_q, &mut undo) {
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

/// 选择编辑：Ctrl+X 剪切（复制到剪贴板并清空选区），Delete/Backspace 清空选区。
pub fn selection_cut_delete_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    tools: Res<ToolState>,
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

    if want_cut {
        let w = rect.width();
        let h = rect.height();
        let mut tiles = Vec::with_capacity((w * h) as usize);
        for y in rect.min.y..=rect.max.y {
            for x in rect.min.x..=rect.max.x {
                let idx = map.idx(x, y);
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
            let idx = map.idx(x, y);
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

    for ch in &cmd.changes {
        let entity = tile_entities.entities[ch.idx];
		if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
			apply_tile_visual(&runtime, &ch.after, &mut sprite, &mut tf, &mut vis, &config);
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

/// 粘贴模式：左键把 Clipboard 贴到鼠标所在格子（作为左上角），并生成 Undo。
pub fn paste_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut tools: ResMut<ToolState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    runtime: Res<TilesetRuntime>,
    clipboard: Res<Clipboard>,
    paste: Res<PasteState>,
    menu: Res<ContextMenuState>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    mut undo: ResMut<UndoStack>,
) {
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
            let dst_idx = map.idx(dst_x, dst_y);
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
                    sx,
                    sy,
                    cx,
                    cy,
                    dst_x,
                    dst_y,
                    before_label,
                    after_label
                );
            }
            let before = map.tiles[dst_idx].clone();
            map.tiles[dst_idx] = after.clone();
            cmd.changes.push(CellChange { idx: dst_idx, before, after });
        }
    }

    if cmd.changes.is_empty() {
        info!(
            "paste result: no changes (attempted={}, oob={}, same={}) pw={} ph={}",
            attempted,
            oob,
            same,
            pw,
            ph
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
    for ch in &cmd.changes {
        let entity = tile_entities.entities[ch.idx];
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
		warn!("paste sprite refresh: missing atlas for {} cells", missing_atlas);
	}

    undo.push(cmd);

    // 贴完后的工具行为：
    // - Ctrl+V/菜单进入的“临时粘贴”（return_after_paste 有值）：贴一次就自动回原工具。
    // - 用户显式切到 Paste（如按 5）：允许连续多次粘贴，按 Esc/菜单退出。
    if let Some(back) = tools.return_after_paste.take() {
        tools.tool = back;
    }
}

/// 保存/读取快捷键：S / L。
pub fn save_load_shortcuts(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<EditorConfig>,
    asset_server: Res<AssetServer>,
    mut lib: ResMut<TilesetLibrary>,
    mut tileset_loading: ResMut<TilesetLoading>,
    runtime: Res<TilesetRuntime>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    map: Option<ResMut<TileMapData>>,
	mut undo: ResMut<UndoStack>,
) {
    if keys.just_pressed(KeyCode::KeyS) {
        let Some(map) = map.as_deref() else {
            return;
        };
        if let Err(err) = save_map_to_file(map, &lib, config.save_path.as_str()) {
            warn!("save failed: {err}");
        } else {
            info!("saved map: {}", config.save_path);
        }
    }

    if keys.just_pressed(KeyCode::KeyL) {
        let (loaded, tilesets) = match load_map_from_file(config.save_path.as_str()) {
            Ok(m) => m,
            Err(err) => {
                warn!("load failed: {err}");
                return;
            }
        };

        merge_tilesets_from_map(&asset_server, &mut lib, &mut tileset_loading, tilesets);
        save_tileset_library(&lib);

        let needs_resize = config.map_size.x != loaded.width || config.map_size.y != loaded.height;
        let current_tile_entities = tile_entities.as_deref();

        if needs_resize {
            if let Some(existing_tiles) = current_tile_entities {
                for &e in &existing_tiles.entities {
                    commands.entity(e).despawn();
                }
            }
            commands.remove_resource::<TileEntities>();
            commands.remove_resource::<TileMapData>();

            config.map_size = UVec2::new(loaded.width, loaded.height);
            let tiles = super::tileset::spawn_map_entities(&mut commands, &config);
            commands.insert_resource(loaded.clone());
            apply_map_to_entities(&runtime, &loaded, &tiles, &mut tiles_q, &config);
            commands.insert_resource(tiles);
			undo.clear();
            return;
        }

        commands.insert_resource(loaded.clone());
        if let Some(tile_entities) = tile_entities.as_deref() {
            apply_map_to_entities(&runtime, &loaded, tile_entities, &mut tiles_q, &config);
        }
		undo.clear();
    }
}

/// 当 tileset 运行时信息发生变化（图片加载完成/新增 tileset）时，刷新整张地图的渲染。
///
/// 这能保证“先加载 map + tileset 还在异步加载中”时，加载完成后自动回显。
pub fn refresh_map_on_tileset_runtime_change(
    runtime: Res<TilesetRuntime>,
    config: Res<EditorConfig>,
    map: Option<Res<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
) {
    if !runtime.is_changed() {
        return;
    }

    let (Some(map), Some(tile_entities)) = (map.as_deref(), tile_entities.as_deref()) else {
        return;
    };

    apply_map_to_entities(&runtime, map, tile_entities, &mut tiles_q, &config);
}

/// 从地图数据同步到格子实体（rect + 可见性）。
pub fn apply_map_to_entities(
    runtime: &TilesetRuntime,
    map: &TileMapData,
    tile_entities: &TileEntities,
    tiles_q: &mut Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    config: &EditorConfig,
) {
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = map.idx(x, y);
            let entity = tile_entities.entities[idx];

            let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) else {
                continue;
            };

			apply_tile_visual(runtime, &map.tiles[idx], &mut sprite, &mut tf, &mut vis, config);
        }
    }
}

/// 鼠标绘制：左键绘制/擦除（右键保留给右键菜单）。
pub fn paint_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
	tools: Res<ToolState>,
	menu: Res<ContextMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    state: Res<EditorState>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
	mut undo: ResMut<UndoStack>,
	mut stroke: Local<StrokeState>,
) {
    if tools.tool != ToolKind::Pencil && tools.tool != ToolKind::Eraser {
		return;
	}
    if menu.open || menu.consume_left_click {
        return;
    }

    // Alt + 拖拽用于“从任意工具框选”，避免与绘制冲突。
    if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
        return;
    }

    // Space 用于平移（Space + 左键拖拽），避免与绘制冲突。
    if keys.pressed(KeyCode::Space) {
        return;
    }

    let active_id = if tools.tool == ToolKind::Pencil {
        let Some(active_id) = lib.active_id.clone() else {
            return;
        };
        Some(active_id)
    } else {
        None
    };
    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };

    let left_down = buttons.pressed(MouseButton::Left);
    let left_start = buttons.just_pressed(MouseButton::Left);
    let left_end = buttons.just_released(MouseButton::Left);

    // stroke 结束：提交为一个 undo 命令
    if stroke.active {
        let ended = left_end || !left_down;
        if ended {
            let cmd = stroke.take_command();
            undo.push(cmd);
            stroke.active = false;
            return;
        }
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };
	let pos = cursor_tile_pos(
		window,
		camera,
		camera_transform,
		&config,
		tile_entities.width,
		tile_entities.height,
	);

    // 开始一次 stroke：必须在画布区域内按下
    if !stroke.active {
		if pos.is_some() {
			if left_start {
				stroke.begin(MouseButton::Left);
			}
		}
    }

    let Some(pos) = pos else {
		return;
	};
	let (x, y) = (pos.x, pos.y);

    let idx = map.idx(x, y);
    let entity = tile_entities.entities[idx];

    // 没有在绘制中
    if !left_down {
        return;
    }

    let desired: Option<TileRef> = if tools.tool == ToolKind::Eraser {
        None
    } else {
        Some(TileRef {
            tileset_id: active_id.clone().unwrap(),
            index: state.selected_tile,
			rot: 0,
			flip_x: false,
			flip_y: false,
        })
    };

    if map.tiles[idx] == desired {
        return;
    }

    let before = map.tiles[idx].clone();
    map.tiles[idx] = desired.clone();

    stroke.record_change(idx, before.clone(), desired.clone());

    // 局部刷新渲染（单格），避免每帧全量 apply
    if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
        apply_tile_visual(&runtime, &desired, &mut sprite, &mut tf, &mut vis, &config);
    }
}

pub struct RectDragState {
    pub active: bool,
    pub button: MouseButton,
    pub start: UVec2,
    pub current: UVec2,
}

impl Default for RectDragState {
    fn default() -> Self {
        Self {
            active: false,
            button: MouseButton::Left,
            start: UVec2::ZERO,
            current: UVec2::ZERO,
        }
    }
}

/// 矩形工具：拖拽框选并一次性填充/擦除。
pub fn rect_with_mouse(
    mut gizmos: Gizmos,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tools: Res<ToolState>,
    menu: Res<ContextMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    state: Res<EditorState>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    mut undo: ResMut<UndoStack>,
    mut drag: Local<RectDragState>,
) {
    if tools.tool != ToolKind::Rect {
        drag.active = false;
        return;
    }
	if menu.open || menu.consume_left_click {
		drag.active = false;
		return;
	}

    // Alt + 拖拽用于“从任意工具框选”，避免与 Rect 冲突。
    if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
        drag.active = false;
        return;
    }

    // Space 用于平移（Space + 左键拖拽），避免与绘制冲突。
    if keys.pressed(KeyCode::Space) {
        drag.active = false;
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

    let left_start = buttons.just_pressed(MouseButton::Left);
    let left_down = buttons.pressed(MouseButton::Left);
    let left_end = buttons.just_released(MouseButton::Left);

    let pos = cursor_tile_pos(
        window,
        camera,
        camera_transform,
        &config,
        tile_entities.width,
        tile_entities.height,
    );

    // 开始拖拽（左键填充；右键保留给菜单）
    if !drag.active {
        let Some(pos) = pos else {
            return;
        };
        if left_start {
            drag.active = true;
            drag.button = MouseButton::Left;
            drag.start = pos;
            drag.current = pos;
        }
    }

    if !drag.active {
        return;
    }

    // 更新当前点（仅当鼠标仍在画布内）
    if let Some(pos) = pos {
        if drag.button == MouseButton::Left && left_down {
            drag.current = pos;
        }
    }

    let min_x = drag.start.x.min(drag.current.x);
    let max_x = drag.start.x.max(drag.current.x);
    let min_y = drag.start.y.min(drag.current.y);
    let max_y = drag.start.y.max(drag.current.y);

    // 开始框选
    let tile_w = config.tile_size.x as f32;
    let tile_h = config.tile_size.y as f32;
    let x0 = min_x as f32 * tile_w;
    let y0 = min_y as f32 * tile_h;
    let x1 = (max_x as f32 + 1.0) * tile_w;
    let y1 = (max_y as f32 + 1.0) * tile_h;
    let preview_color = Color::srgba(0.25, 0.45, 0.95, 0.95);
    gizmos.line_2d(Vec2::new(x0, y0), Vec2::new(x1, y0), preview_color);
    gizmos.line_2d(Vec2::new(x1, y0), Vec2::new(x1, y1), preview_color);
    gizmos.line_2d(Vec2::new(x1, y1), Vec2::new(x0, y1), preview_color);
    gizmos.line_2d(Vec2::new(x0, y1), Vec2::new(x0, y0), preview_color);

    // 结束拖拽：提交命令
    let ended = left_end || !left_down;
    if !ended {
        return;
    }

	let erase = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let desired: Option<TileRef> = if erase {
        None
    } else {
        let Some(active_id) = lib.active_id.clone() else {
            drag.active = false;
            return;
        };
        Some(TileRef {
            tileset_id: active_id,
            index: state.selected_tile,
            rot: 0,
            flip_x: false,
            flip_y: false,
        })
    };

    let mut changes: Vec<CellChange> = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let idx = map.idx(x, y);
            if idx >= map.tiles.len() {
                continue;
            }
            if map.tiles[idx] == desired {
                continue;
            }

            let before = map.tiles[idx].clone();
            map.tiles[idx] = desired.clone();
            changes.push(CellChange {
                idx,
                before: before.clone(),
                after: desired.clone(),
            });

            // 局部刷新渲染
            let entity = tile_entities.entities[idx];
            if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
                apply_tile_visual(&runtime, &desired, &mut sprite, &mut tf, &mut vis, &config);
            }
        }
    }

    undo.push(EditCommand { changes });
    drag.active = false;
}

/// 油漆桶（Flood Fill）：点击格子后，按 4 邻接填充“同类 tile”的连通区域。
///
/// - 左键：填充为当前选择的 tile
/// - 右键：保留给右键菜单
pub fn fill_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tools: Res<ToolState>,
    menu: Res<ContextMenuState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    state: Res<EditorState>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
    mut undo: ResMut<UndoStack>,
) {
    if tools.tool != ToolKind::Fill {
        return;
    }
	if menu.open || menu.consume_left_click {
		return;
	}

    // Alt + 拖拽用于“从任意工具框选”，避免与 Flood Fill 冲突。
    if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
        return;
    }

    // Space 用于平移（Space + 左键拖拽），避免与点击填充冲突。
    if keys.pressed(KeyCode::Space) {
        return;
    }

    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };

    let left_start = buttons.just_pressed(MouseButton::Left);
    if !left_start {
        return;
    }

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

    let erase = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let desired: Option<TileRef> = if erase {
        None
    } else {
        let Some(active_id) = lib.active_id.clone() else {
            return;
        };
        Some(TileRef {
            tileset_id: active_id,
            index: state.selected_tile,
            rot: 0,
            flip_x: false,
            flip_y: false,
        })
    };

    let start_idx = map.idx(pos.x, pos.y);
    let target = map.tiles[start_idx].clone();
    if target == desired {
        return;
    }

    let w = tile_entities.width;
    let h = tile_entities.height;
    let mut visited = vec![false; (w * h) as usize];
    let mut q = VecDeque::new();
    visited[start_idx] = true;
    q.push_back((pos.x, pos.y));

    let mut cmd = EditCommand::default();

    while let Some((x, y)) = q.pop_front() {
        let idx = map.idx(x, y);
        if map.tiles[idx] != target {
            continue;
        }

        let before = map.tiles[idx].clone();
        map.tiles[idx] = desired.clone();
        cmd.changes.push(CellChange {
            idx,
            before,
            after: desired.clone(),
        });

        let push = |nx: i32, ny: i32, visited: &mut [bool], q: &mut VecDeque<(u32, u32)>, w: u32, h: u32| {
            if nx < 0 || ny < 0 {
                return;
            }
            let (nx, ny) = (nx as u32, ny as u32);
            if nx >= w || ny >= h {
                return;
            }
            let nidx = (ny * w + nx) as usize;
            if visited[nidx] {
                return;
            }
            visited[nidx] = true;
            q.push_back((nx, ny));
        };

        push(x as i32 - 1, y as i32, &mut visited, &mut q, w, h);
        push(x as i32 + 1, y as i32, &mut visited, &mut q, w, h);
        push(x as i32, y as i32 - 1, &mut visited, &mut q, w, h);
        push(x as i32, y as i32 + 1, &mut visited, &mut q, w, h);
    }

    // 局部刷新渲染（只刷改动格子）
    for ch in &cmd.changes {
        let entity = tile_entities.entities[ch.idx];
        if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
			apply_tile_visual(&runtime, &ch.after, &mut sprite, &mut tf, &mut vis, &config);
        }
    }

    undo.push(cmd);
}

pub struct StrokeState {
    pub active: bool,
    pub button: MouseButton,
    changes: HashMap<usize, CellChange>,
}

impl Default for StrokeState {
    fn default() -> Self {
        Self {
            active: false,
            button: MouseButton::Left,
            changes: HashMap::new(),
        }
    }
}

impl StrokeState {
    pub fn begin(&mut self, button: MouseButton) {
        self.active = true;
        self.button = button;
        self.changes.clear();
    }

    pub fn record_change(&mut self, idx: usize, before: Option<TileRef>, after: Option<TileRef>) {
        self.changes
            .entry(idx)
            .and_modify(|c| c.after = after.clone())
            .or_insert(CellChange { idx, before, after });
    }

    pub fn take_command(&mut self) -> EditCommand {
        let mut changes: Vec<CellChange> = self
            .changes
            .drain()
            .map(|(_, v)| v)
            .filter(|c| c.before != c.after)
            .collect();
        changes.sort_by_key(|c| c.idx);
        EditCommand { changes }
    }
}
