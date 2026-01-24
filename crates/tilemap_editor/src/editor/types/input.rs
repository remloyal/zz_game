use bevy::prelude::*;

/// 画布平移（拖拽）状态。
#[derive(Resource, Default)]
pub struct PanState {
    pub active: bool,
    pub last_cursor: Option<Vec2>,
}
