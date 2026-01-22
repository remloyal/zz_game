use bevy::prelude::*;

use crate::editor::types::{LayerState, MapSizeFocus, MapSizeInput, TileMapData};

/// 图层快捷键：PgUp/PgDn 切换当前编辑层；L 在 0/1 间切换（存在两层时）。
pub fn layer_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MapSizeInput>,
    map: Option<Res<TileMapData>>,
    mut layer_state: ResMut<LayerState>,
) {
    // 正在输入地图尺寸时，避免抢走按键。
    if input.focus != MapSizeFocus::None {
        return;
    }
    let Some(map) = map.as_deref() else {
        return;
    };
    if map.layers == 0 {
        return;
    }

    // 安全夹紧：地图层数变化（加载/重建）时避免越界。
    let max_layer = map.layers - 1;
    if layer_state.active > max_layer {
        layer_state.active = max_layer;
    }

    if keys.just_pressed(KeyCode::PageDown) {
        layer_state.active = layer_state.active.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::PageUp) {
        layer_state.active = (layer_state.active + 1).min(max_layer);
    }
    if keys.just_pressed(KeyCode::KeyL) && map.layers >= 2 {
        layer_state.active = if layer_state.active == 0 { 1 } else { 0 };
    }
}
