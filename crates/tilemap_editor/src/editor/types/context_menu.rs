use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextMenuAction {
    Undo,
    Redo,
    EnterPaste,
    SelectionCopy,
    SelectionCut,
    SelectionDelete,
    SelectionSelectAll,
    SelectionDeselect,

    PasteRotateCcw,
    PasteRotateCw,
    PasteFlipX,
    PasteFlipY,
    PasteReset,
    ExitPaste,
}

#[derive(Resource, Default)]
pub struct ContextMenuState {
    pub open: bool,
    /// UI 屏幕坐标（原点左上）。
    pub screen_pos: Vec2,
    /// 打开菜单时鼠标所在的地图格子坐标（若不在地图上则为 None）。
    pub map_pos: Option<UVec2>,
    /// 用于“点击菜单项/点击空白关闭”时，避免同一帧触发画布左键操作。
    pub consume_left_click: bool,
    /// 用于 UI 动态重建菜单：状态签名（工具/选区/剪贴板等）变化时重建。
    pub signature: u64,
}

#[derive(Component)]
pub struct ContextMenuRoot;

#[derive(Component)]
pub struct ContextMenuBackdrop;

#[derive(Component, Clone, Copy)]
pub struct ContextMenuItem(pub ContextMenuAction);

#[derive(Component)]
pub struct ContextMenuDisabled;

#[derive(Resource, Default)]
pub struct ContextMenuCommand {
    pub action: Option<ContextMenuAction>,
}

#[derive(Resource, Default)]
pub struct PastePreview {
    pub entities: Vec<Entity>,
    pub dims: (u32, u32),
}

#[derive(Component)]
pub struct PastePreviewTile;
