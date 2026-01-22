use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Pencil,
    Eraser,
    Rect,
    Fill,
    Select,
    Paste,
    Eyedropper,
}

impl Default for ToolKind {
    fn default() -> Self {
        Self::Pencil
    }
}

#[derive(Resource)]
pub struct ToolState {
    pub tool: ToolKind,
    /// 通过 Ctrl+V / 右键菜单进入粘贴时，记住进入前的工具，便于粘贴落地后自动恢复。
    pub return_after_paste: Option<ToolKind>,
}

impl Default for ToolState {
    fn default() -> Self {
        Self {
            tool: ToolKind::default(),
            return_after_paste: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftMapMode {
    Blank,
    Wrap,
}

impl Default for ShiftMapMode {
    fn default() -> Self {
        Self::Blank
    }
}

#[derive(Resource, Default)]
pub struct ShiftMapSettings {
    pub mode: ShiftMapMode,
}

#[derive(Component, Clone, Copy)]
pub struct ToolButton(pub ToolKind);

#[derive(Component)]
pub struct ShiftModeButton;

#[derive(Component)]
pub struct ShiftModeLabel;
