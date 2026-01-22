use bevy::prelude::*;

#[derive(Resource)]
pub struct EditorState {
    pub selected_tile: u32,
}

impl Default for EditorState {
    fn default() -> Self {
        Self { selected_tile: 0 }
    }
}
