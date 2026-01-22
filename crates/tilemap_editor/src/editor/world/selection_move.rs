use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::ecs::system::SystemParam;

use std::collections::HashSet;

use crate::editor::types::{
    CellChange, ContextMenuState, EditCommand, EditorConfig, LayerState, SelectionMovePreviewTile,
    SelectionRect, SelectionState, TileEntities, TileMapData, TileRef, TilesetRuntime, ToolKind,
    ToolState, UndoStack, WorldCamera,
};

use super::{apply_tile_visual, cursor_tile_pos, tile_world_center};

#[derive(SystemParam)]
pub(in crate::editor) struct SelectionMoveParams<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    tools: Res<'w, ToolState>,
    layer_state: Res<'w, LayerState>,
    menu: Res<'w, ContextMenuState>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera_q: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<WorldCamera>>,
    config: Res<'w, EditorConfig>,
    runtime: Res<'w, TilesetRuntime>,
    map: Option<ResMut<'w, TileMapData>>,
    tile_entities: Option<Res<'w, TileEntities>>,
    tiles_q: Query<
        'w,
        's,
        (&'static mut Sprite, &'static mut Transform, &'static mut Visibility),
        Without<SelectionMovePreviewTile>,
    >,
    undo: ResMut<'w, UndoStack>,
    selection: ResMut<'w, SelectionState>,
    preview_q: Query<
        'w,
        's,
        (&'static mut Sprite, &'static mut Transform, &'static mut Visibility),
        With<SelectionMovePreviewTile>,
    >,
}

#[derive(Default)]
pub(in crate::editor) struct SelectionMoveDrag {
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
    params: SelectionMoveParams,
    mut drag: Local<SelectionMoveDrag>,
) {
    let SelectionMoveParams {
        buttons,
        keys,
        tools,
        layer_state,
        menu,
        windows,
        camera_q,
        config,
        runtime,
        map,
        tile_entities,
        mut tiles_q,
        mut undo,
        mut selection,
        mut preview_q,
    } = params;

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
        let layer = layer_state.active.min(map.layers.saturating_sub(1));
        drag.buf.clear();
        drag.buf.reserve((w * h) as usize);
        for y in rect.min.y..=rect.max.y {
            for x in rect.min.x..=rect.max.x {
                let idx = map.idx_layer(layer, x, y);
                drag.buf.push(map.tiles[idx].clone());
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
    let layer = layer_state.active.min(map.layers.saturating_sub(1));

    // helper：写一个格子的 after，并记录变更
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
