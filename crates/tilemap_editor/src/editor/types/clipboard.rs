use bevy::prelude::*;

use super::TileRef;

#[derive(Resource, Default, Clone)]
pub struct Clipboard {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Option<TileRef>>,
}

#[derive(Resource, Default, Clone, Copy)]
pub struct PasteState {
    /// 0,1,2,3 => 0/90/180/270 度顺时针。
    pub rot: u8,
    pub flip_x: bool,
    pub flip_y: bool,
}
