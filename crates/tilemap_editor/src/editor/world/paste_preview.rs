use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{
    Clipboard, EditorConfig, PastePreview, PastePreviewTile, PasteState, TileEntities, TileRef,
    TilesetRuntime, ToolKind, ToolState, WorldCamera,
};
use crate::editor::util::despawn_silently;
use crate::editor::tileset::rect_for_tile_index;

use super::{cursor_tile_pos, tile_world_center};
use super::paste_helpers::{paste_dims, paste_dst_xy};

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
            despawn_silently(&mut commands, e);
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
            apply_preview_tile_visual(&runtime, &transformed[i], &mut sprite, &mut tf, &mut vis, &config);
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.55);
        }
    }
}

fn apply_preview_tile_visual(
    runtime: &TilesetRuntime,
    tile: &Option<TileRef>,
    sprite: &mut Sprite,
    tf: &mut Transform,
    vis: &mut Visibility,
    config: &EditorConfig,
) {
    match tile {
        Some(TileRef { tileset_id, index, rot, flip_x, flip_y }) => {
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
