use bevy::prelude::*;

use super::TileRef;

#[derive(Clone, Debug)]
pub struct CellChange {
    pub idx: usize,
    pub before: Option<TileRef>,
    pub after: Option<TileRef>,
}

#[derive(Clone, Debug, Default)]
pub struct EditCommand {
    pub changes: Vec<CellChange>,
}

#[derive(Resource, Default)]
pub struct UndoStack {
    pub undo: Vec<EditCommand>,
    pub redo: Vec<EditCommand>,
    pub max_len: usize,
}

impl UndoStack {
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn push(&mut self, cmd: EditCommand) {
        if cmd.changes.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push(cmd);
        let max_len = if self.max_len == 0 { 200 } else { self.max_len };
        if self.undo.len() > max_len {
            let drain = self.undo.len() - max_len;
            self.undo.drain(0..drain);
        }
    }
}
