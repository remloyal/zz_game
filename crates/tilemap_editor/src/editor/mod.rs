//! Tilemap Editor（类似 RPG Maker 的基础版）
//!
//! 目标：
//! - 左侧 tileset 调色板（palette）选择 tile
//! - 右侧画布绘制/擦除
//! - 保存/读取地图（RON）
//!
//! 说明：
//! - 本 crate 使用 Bevy 0.18。
//! - UI 与世界渲染共存时，必须明确指定“世界相机”，否则鼠标拾取可能取到 UI 相机导致绘制失效。

mod paths;
mod persistence;
mod tileset;
mod types;
mod ui;
mod world;

mod app;

use bevy::prelude::Color;

/// UI 相关常量
pub const LEFT_PANEL_WIDTH_PX: f32 = 320.0;
pub const TILE_BUTTON_PX: f32 = 40.0;

/// 右侧画布顶部工具条高度（用于避免 UI 区域误绘制/缩放/拖拽）。
pub const RIGHT_TOPBAR_HEIGHT_PX: f32 = 56.0;

pub const UI_BG: Color = Color::srgb(0.12, 0.12, 0.12);
pub const UI_PANEL: Color = Color::srgb(0.16, 0.16, 0.16);
pub const UI_HIGHLIGHT: Color = Color::srgb(0.25, 0.45, 0.95);
pub const UI_BUTTON: Color = Color::srgb(0.22, 0.22, 0.22);
pub const UI_BUTTON_HOVER: Color = Color::srgb(0.28, 0.28, 0.28);
pub const UI_BUTTON_PRESS: Color = Color::srgb(0.35, 0.35, 0.35);

pub use app::run;
