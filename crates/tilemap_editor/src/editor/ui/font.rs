//! UI 字体相关系统。

use bevy::prelude::*;

use crate::editor::types::{UiFont, DEFAULT_UI_FONT_PATH};

/// 启动时加载 UI 字体（用于中文）。
pub fn load_ui_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load(DEFAULT_UI_FONT_PATH);
    commands.insert_resource(UiFont(font));
}

/// 把所有 TextFont 的 font 统一设置为 UiFont，避免默认字体缺字导致乱码。
pub fn apply_ui_font_to_all_text(ui_font: Option<Res<UiFont>>, mut q: Query<&mut TextFont>) {
    let Some(ui_font) = ui_font.as_deref() else {
        return;
    };
    for mut tf in q.iter_mut() {
        tf.font = ui_font.0.clone();
    }
}
