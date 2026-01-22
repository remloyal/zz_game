use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use std::collections::HashMap;

use crate::editor::types::{
    CellChange, ContextMenuState, EditCommand, EditorConfig, EditorState, LayerState, TileEntities,
    TileMapData, TileRef, TilesetLibrary, TilesetRuntime, ToolKind, ToolState, UndoStack,
    WorldCamera,
};

use super::super::{apply_tile_visual, cursor_tile_pos};

/// 鼠标绘制：左键绘制/擦除（右键保留给右键菜单）。
pub fn paint_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    tools: Res<ToolState>,
    layer_state: Res<LayerState>,
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

    let layer = layer_state.active.min(map.layers.saturating_sub(1));
    let idx = map.idx_layer(layer, x, y);
    let entity_idx = tile_entities.idx_layer(layer, x, y);
    if idx >= map.tiles.len() || entity_idx >= tile_entities.entities.len() {
        return;
    }
    let entity = tile_entities.entities[entity_idx];

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
