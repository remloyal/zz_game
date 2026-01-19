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

use super::persistence::{load_map_from_file, save_map_to_file};
use super::tileset::{merge_tilesets_from_map, rect_for_tile_index, save_tileset_library};
use super::types::{
    EditorConfig, EditorState, PanState, TileEntities, TileMapData, TileRef, TilesetLibrary,
    TilesetLoading, TilesetRuntime, WorldCamera,
};
use super::{LEFT_PANEL_WIDTH_PX, RIGHT_TOPBAR_HEIGHT_PX};

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
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Visibility)>,
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

    // 立即同步渲染，避免“数据清了但画面没变”。
    if keys.just_pressed(KeyCode::KeyR) {
        if let (Some(tile_entities), Some(mut map)) = (tile_entities.as_deref(), map) {
            *map = TileMapData::new(map.width, map.height);
            apply_map_to_entities(&runtime, &map, tile_entities, &mut tiles_q, &config);
        }
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
    mut tiles_q: Query<(&mut Sprite, &mut Visibility)>,
    map: Option<ResMut<TileMapData>>,
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
            return;
        }

        commands.insert_resource(loaded.clone());
        if let Some(tile_entities) = tile_entities.as_deref() {
            apply_map_to_entities(&runtime, &loaded, tile_entities, &mut tiles_q, &config);
        }
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
    mut tiles_q: Query<(&mut Sprite, &mut Visibility)>,
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
    tiles_q: &mut Query<(&mut Sprite, &mut Visibility)>,
    config: &EditorConfig,
) {
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = map.idx(x, y);
            let entity = tile_entities.entities[idx];

            let Ok((mut sprite, mut vis)) = tiles_q.get_mut(entity) else {
                continue;
            };

            match map.tiles[idx] {
                Some(TileRef { ref tileset_id, index }) => {
					let Some(atlas) = runtime.by_id.get(tileset_id) else {
						sprite.rect = None;
						*vis = Visibility::Hidden;
						continue;
					};
					sprite.image = atlas.texture.clone();
					sprite.rect = Some(rect_for_tile_index(index, atlas.columns, config.tile_size));
					*vis = Visibility::Visible;
				}
                None => {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

/// 鼠标绘制：左键绘制所选 tile，右键擦除。
pub fn paint_with_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    config: Res<EditorConfig>,
    state: Res<EditorState>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    map: Option<ResMut<TileMapData>>,
    tile_entities: Option<Res<TileEntities>>,
    mut tiles_q: Query<(&mut Sprite, &mut Visibility)>,
) {
    // Space 用于平移（Space + 左键拖拽），避免与绘制冲突。
    if keys.pressed(KeyCode::Space) {
        return;
    }

    let Some(active_id) = lib.active_id.clone() else {
        return;
    };
    let Some(active_atlas) = runtime.by_id.get(&active_id) else {
        return;
    };
    let Some(mut map) = map else {
        return;
    };
    let Some(tile_entities) = tile_entities else {
        return;
    };

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // 左侧 UI 面板区域不响应绘制
    if cursor_pos.x <= LEFT_PANEL_WIDTH_PX {
        return;
    }

    // 右侧顶部 UI 工具条区域不响应绘制
    if cursor_pos.y <= RIGHT_TOPBAR_HEIGHT_PX {
        return;
    }

    let Ok((camera, camera_transform)) = camera_q.single() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    let tile_w = config.tile_size.x as f32;
    let tile_h = config.tile_size.y as f32;
    if tile_w <= 0.0 || tile_h <= 0.0 {
        return;
    }

    let x = (world_pos.x / tile_w).floor() as i32;
    let y = (world_pos.y / tile_h).floor() as i32;
    if x < 0 || y < 0 {
        return;
    }

    let (x, y) = (x as u32, y as u32);
    if x >= tile_entities.width || y >= tile_entities.height {
        return;
    }

    let idx = map.idx(x, y);
    let entity = tile_entities.entities[idx];

    if buttons.just_pressed(MouseButton::Left) || buttons.pressed(MouseButton::Left) {
        map.tiles[idx] = Some(TileRef {
			tileset_id: active_id.clone(),
			index: state.selected_tile,
		});
        if let Ok((mut sprite, mut vis)) = tiles_q.get_mut(entity) {
            sprite.image = active_atlas.texture.clone();
            sprite.rect = Some(rect_for_tile_index(state.selected_tile, active_atlas.columns, config.tile_size));
            *vis = Visibility::Visible;
        }
    }

    if buttons.just_pressed(MouseButton::Right) || buttons.pressed(MouseButton::Right) {
        map.tiles[idx] = None;
        if let Ok((_sprite, mut vis)) = tiles_q.get_mut(entity) {
            *vis = Visibility::Hidden;
        }
    }
}
