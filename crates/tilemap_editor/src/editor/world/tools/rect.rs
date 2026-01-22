use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{
    CellChange, ContextMenuState, EditCommand, EditorConfig, EditorState, LayerState, TileEntities,
    TileMapData, TileRef, TilesetLibrary, TilesetRuntime, ToolKind, ToolState, UndoStack,
    WorldCamera,
};

use super::super::{apply_tile_visual, cursor_tile_pos};

pub struct RectDragState {
    pub active: bool,
    pub button: MouseButton,
    pub start: UVec2,
    pub current: UVec2,
}

#[derive(SystemParam)]
pub(crate) struct RectWithMouseParams<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    tools: Res<'w, ToolState>,
    layer_state: Res<'w, LayerState>,
    menu: Res<'w, ContextMenuState>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera_q: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<WorldCamera>>,
    config: Res<'w, EditorConfig>,
    state: Res<'w, EditorState>,
    lib: Res<'w, TilesetLibrary>,
    runtime: Res<'w, TilesetRuntime>,
    map: Option<ResMut<'w, TileMapData>>,
    tile_entities: Option<Res<'w, TileEntities>>,
    tiles_q: Query<'w, 's, (&'static mut Sprite, &'static mut Transform, &'static mut Visibility)>,
    undo: ResMut<'w, UndoStack>,
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
pub fn rect_with_mouse(mut gizmos: Gizmos, params: RectWithMouseParams, mut drag: Local<RectDragState>) {
    let RectWithMouseParams {
        buttons,
        keys,
        tools,
        layer_state,
        menu,
        windows,
        camera_q,
        config,
        state,
        lib,
        runtime,
        map,
        tile_entities,
        mut tiles_q,
        mut undo,
    } = params;

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

    // 绘制预览框
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
    let layer = layer_state.active.min(map.layers.saturating_sub(1));
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let idx = map.idx_layer(layer, x, y);
            let entity_idx = tile_entities.idx_layer(layer, x, y);
            if idx >= map.tiles.len() || entity_idx >= tile_entities.entities.len() {
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
            let entity = tile_entities.entities[entity_idx];
            if let Ok((mut sprite, mut tf, mut vis)) = tiles_q.get_mut(entity) {
                apply_tile_visual(&runtime, &desired, &mut sprite, &mut tf, &mut vis, &config);
            }
        }
    }

    undo.push(EditCommand { changes });
    drag.active = false;
}
