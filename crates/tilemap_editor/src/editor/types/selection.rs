use bevy::prelude::*;

#[derive(Component)]
pub struct SelectionMovePreviewTile;

#[derive(Clone, Copy, Debug, Default)]
pub struct SelectionRect {
    pub min: UVec2,
    pub max: UVec2,
}

impl SelectionRect {
    pub fn width(&self) -> u32 {
        self.max.x.saturating_sub(self.min.x) + 1
    }

    pub fn height(&self) -> u32 {
        self.max.y.saturating_sub(self.min.y) + 1
    }
}

#[derive(Resource, Default)]
pub struct SelectionState {
    pub dragging: bool,
    pub start: UVec2,
    pub current: UVec2,
    pub rect: Option<SelectionRect>,
    /// 是否正在“拖拽移动选区内容”（与 dragging=框选不同）。
    pub moving: bool,
}
