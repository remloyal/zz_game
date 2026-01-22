use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::types::{
    Clipboard, EditorConfig, PasteState, SelectionState, TileEntities, ToolKind, ToolState,
    WorldCamera,
};
use crate::editor::{LEFT_PANEL_WIDTH_PX, RIGHT_TOPBAR_HEIGHT_PX};

use super::paste_helpers::paste_dims;

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

    if config.show_grid {
        // 网格线
        for x in 0..=map_w {
            let x_pos = x as f32 * tile_w;
            gizmos.line_2d(Vec2::new(x_pos, 0.0), Vec2::new(x_pos, height_px), grid_color);
        }
        for y in 0..=map_h {
            let y_pos = y as f32 * tile_h;
            gizmos.line_2d(Vec2::new(0.0, y_pos), Vec2::new(width_px, y_pos), grid_color);
        }
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
