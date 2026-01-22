use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{
    ContextMenuState, EditorConfig, SelectionState, TileEntities, ToolKind, ToolState, WorldCamera,
};

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

    let pos = super::cursor_tile_pos(
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
            selection.rect = Some(super::rect_from_two(pos, pos));
        }
        return;
    }

    if selection.dragging {
        if let Some(pos) = pos {
            if left_down {
                selection.current = pos;
                selection.rect = Some(super::rect_from_two(selection.start, selection.current));
            }
        }
        if left_end || !left_down {
            selection.dragging = false;
            selection.rect = Some(super::rect_from_two(selection.start, selection.current));
        }
    }
}
