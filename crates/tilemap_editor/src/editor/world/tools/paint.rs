use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::ecs::system::SystemParam;

use std::collections::HashMap;

use crate::editor::types::{
    CellChange, ContextMenuState, EditCommand, EditorConfig, EditorState, LayerState, TileMapData,
    TileRef, TilesetLibrary, ToolKind, ToolState, UndoStack,
    BrushSettings, WorldCamera, 
};

use super::super::{apply_tile_change, cursor_tile_pos, TilemapRenderParams};

#[derive(SystemParam)]
pub struct PaintWithMouseParams<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub keys: Res<'w, ButtonInput<KeyCode>>,
    pub tools: Res<'w, ToolState>,
    pub brush: Res<'w, BrushSettings>,
    pub layer_state: Res<'w, LayerState>,
    pub menu: Res<'w, ContextMenuState>,
    pub windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub camera_q: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<WorldCamera>>,
    pub config: Res<'w, EditorConfig>,
    pub state: Res<'w, EditorState>,
    pub lib: Res<'w, TilesetLibrary>,
    pub map: Option<ResMut<'w, TileMapData>>,
    pub render: TilemapRenderParams<'w, 's>,
    pub undo: ResMut<'w, UndoStack>,
}

/// 鼠标绘制：左键绘制/擦除（右键保留给右键菜单）。
pub fn paint_with_mouse(
    mut p: PaintWithMouseParams,
    mut stroke: Local<StrokeState>,
) {
    if p.tools.tool != ToolKind::Pencil && p.tools.tool != ToolKind::Eraser {
        return;
    }
    if p.menu.open || p.menu.consume_left_click {
        return;
    }

    // Alt + 拖拽用于“从任意工具框选”，避免与绘制冲突。
    if p.keys.pressed(KeyCode::AltLeft) || p.keys.pressed(KeyCode::AltRight) {
        return;
    }

    // Space 用于平移（Space + 左键拖拽），避免与绘制冲突。
    if p.keys.pressed(KeyCode::Space) {
        return;
    }

    let active_id = if p.tools.tool == ToolKind::Pencil {
        let Some(active_id) = p.lib.active_id.clone() else {
            return;
        };
        Some(active_id)
    } else {
        None
    };
    let Some(mut map) = p.map else {
        return;
    };

    let layer = p.layer_state.active.min(map.layers.saturating_sub(1));
    let layer_locked = map
        .layer_data
        .get(layer as usize)
        .map(|d| d.locked)
        .unwrap_or(false);
    if layer_locked {
        // 若当前正在 stroke 中，直接终止（不提交）。
        stroke.active = false;
        stroke.changes.clear();
        return;
    }

    let left_down = p.buttons.pressed(MouseButton::Left);
    let left_start = p.buttons.just_pressed(MouseButton::Left);
    let left_end = p.buttons.just_released(MouseButton::Left);

    // stroke 结束：提交为一个 undo 命令
    if stroke.active {
        let ended = left_end || !left_down;
        if ended {
            let cmd = stroke.take_command();
            p.undo.push(cmd);
            stroke.active = false;
            return;
        }
    }

    let Ok(window) = p.windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = p.camera_q.single() else {
        return;
    };
    let pos = cursor_tile_pos(
        window,
        camera,
        camera_transform,
        &p.config,
        map.width,
        map.height,
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

    let desired: Option<TileRef> = if p.tools.tool == ToolKind::Eraser {
        None
    } else {
        Some(TileRef {
            tileset_id: active_id.clone().unwrap(),
            index: p.state.selected_tile,
            rot: 0,
            flip_x: false,
            flip_y: false,
        })
    };

    // 没有在绘制中
    if !left_down {
        return;
    }

    let size = p.brush.size.clamp(1, 3);
    for dy in 0..size {
        for dx in 0..size {
            let x = pos.x + dx;
            let y = pos.y + dy;
            if x >= map.width || y >= map.height {
                continue;
            }
            let idx = map.idx_layer(layer, x, y);
            if idx >= map.tiles.len() {
                continue;
            }
            if map.tiles[idx] == desired {
                continue;
            }
            let before = map.tiles[idx].clone();
            map.tiles[idx] = desired.clone();
            stroke.record_change(idx, before.clone(), desired.clone());

            apply_tile_change(&mut p.render, &p.config, layer, x, y, &before, &desired);
        }
    }
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
