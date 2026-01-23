use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{EditorConfig, PanState, WorldCamera};
use crate::editor::{LEFT_PANEL_WIDTH_PX, UI_TOP_RESERVED_PX};

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
    if cursor.y <= UI_TOP_RESERVED_PX {
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
    if cursor.y <= UI_TOP_RESERVED_PX {
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
