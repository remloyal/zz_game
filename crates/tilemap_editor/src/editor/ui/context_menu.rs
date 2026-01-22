//! 右键上下文菜单（RPG Maker 风格）。

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::editor::{UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS};
use crate::editor::types::{
    Clipboard, ContextMenuAction, ContextMenuCommand, ContextMenuDisabled, ContextMenuItem,
    ContextMenuRoot, ContextMenuState, SelectionState, TileMapData, ToolKind, ToolState, UndoStack,
    ContextMenuBackdrop,
};

/// UI 初始化时创建右键菜单实体树（初始为空；打开菜单时动态生成菜单项）。
pub(super) fn spawn_context_menu(commands: &mut Commands) {
    // 背景遮罩：用于“点空白关闭菜单”，并避免 world 侧做不精确的 bounds 判断。
    // z-index 要略低于菜单本体，让菜单项能够正常 hover/click。
    commands.spawn((
        Button,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ZIndex(4999),
        ContextMenuBackdrop,
    ));

    let _menu = commands
        .spawn((
            // 让菜单面板本身也参与 hit-test，防止点击面板空白处穿透到 backdrop 触发关闭。
            Button,
            Node {
                min_width: Val::Px(240.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                padding: UiRect::all(Val::Px(4.0)),
                row_gap: Val::Px(2.0),
                flex_direction: FlexDirection::Column,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.08, 0.09, 0.95)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.12)),
            ZIndex(5000),
            ContextMenuRoot,
        ))
        .id();

    // 初始为空：打开菜单时按上下文动态生成。
}

fn menu_signature(tools: &ToolState, undo: &UndoStack, selection: &SelectionState, clipboard: &Clipboard) -> u64 {
    let tool = tools.tool as u64;
    let can_undo = (!undo.undo.is_empty()) as u64;
    let can_redo = (!undo.redo.is_empty()) as u64;
    let has_sel = selection.rect.is_some() as u64;
    let has_clip = (clipboard.width > 0 && clipboard.height > 0 && !clipboard.tiles.is_empty()) as u64;
    (tool << 0) ^ (can_undo << 4) ^ (can_redo << 5) ^ (has_sel << 8) ^ (has_clip << 16)
}

fn spawn_menu_item(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    shortcut: &str,
    action: ContextMenuAction,
    enabled: bool,
) {
    let mut e = commands.spawn((
        Button,
        Node {
            height: Val::Px(30.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        BackgroundColor(if enabled { UI_BUTTON } else { Color::srgb(0.18, 0.18, 0.18) }.into()),
        ContextMenuItem(action),
    ));
    if !enabled {
        e.insert(ContextMenuDisabled);
    }
    let id = e.id();
    commands.entity(parent).add_child(id);
    commands.entity(id).with_children(|b| {
        b.spawn((
            Text::new(label),
            TextFont { font_size: 14.0, ..default() },
            TextColor(if enabled { Color::WHITE } else { Color::srgba(1.0, 1.0, 1.0, 0.35) }),
        ));
        b.spawn((
            Text::new(shortcut),
            TextFont { font_size: 12.0, ..default() },
            TextColor(Color::srgba(1.0, 1.0, 1.0, if enabled { 0.55 } else { 0.25 })),
        ));
    });
}

fn spawn_menu_separator(commands: &mut Commands, parent: Entity) {
    let id = commands
        .spawn((
            Node {
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
        ))
        .id();
    commands.entity(parent).add_child(id);
}

/// 打开菜单时，根据当前上下文动态生成可用菜单项。
pub fn context_menu_rebuild(
    mut commands: Commands,
    tools: Res<ToolState>,
    undo: Res<UndoStack>,
    selection: Res<SelectionState>,
    clipboard: Res<Clipboard>,
    mut state: ResMut<ContextMenuState>,
    map: Option<Res<TileMapData>>,
    menu_q: Query<Entity, With<ContextMenuRoot>>,
    children_q: Query<&Children>,
) {
    if !state.open {
        return;
    }

    let Some(menu) = menu_q.iter().next() else {
        return;
    };
    let sig = menu_signature(&tools, &undo, &selection, &clipboard);
    if sig == state.signature {
        return;
    }
    state.signature = sig;

    if let Ok(children) = children_q.get(menu) {
        fn despawn_tree(commands: &mut Commands, children_q: &Query<&Children>, entity: Entity) {
            if let Ok(children) = children_q.get(entity) {
                // 先复制一份，避免递归过程中 Children 发生变化导致迭代问题
                let to_despawn: Vec<Entity> = children.iter().collect();
                for child in to_despawn {
                    despawn_tree(commands, children_q, child);
                }
            }
            commands.entity(entity).despawn();
        }

        for c in children.iter().collect::<Vec<_>>() {
            despawn_tree(&mut commands, &children_q, c);
        }
    }

    // 动态条目
    let has_sel = selection.rect.is_some();
    let has_clip = clipboard.width > 0 && clipboard.height > 0 && !clipboard.tiles.is_empty();
    let has_tile_under_cursor = state
        .map_pos
        .and_then(|p| {
            let map = map.as_deref()?;
            if p.x >= map.width || p.y >= map.height {
                return None;
            }
            let idx = map.idx(p.x, p.y);
            map.tiles.get(idx).cloned().flatten()
        })
        .is_some();
    let can_undo = !undo.undo.is_empty();
    let can_redo = !undo.redo.is_empty();
    let in_paste = tools.tool == ToolKind::Paste;
    let can_transform = has_clip || has_tile_under_cursor;

    // RPG Maker 风格：Undo/Redo 顶部
    spawn_menu_item(
        &mut commands,
        menu,
        "撤销",
        "Ctrl+Z",
        ContextMenuAction::Undo,
        can_undo,
    );
    spawn_menu_item(
        &mut commands,
        menu,
        "重做",
        "Ctrl+Y",
        ContextMenuAction::Redo,
        can_redo,
    );
    spawn_menu_separator(&mut commands, menu);

    // 通用：粘贴入口（无剪贴板则禁用）
    spawn_menu_item(
        &mut commands,
        menu,
        "粘贴",
        "Ctrl+V",
        ContextMenuAction::EnterPaste,
        has_clip,
    );
    spawn_menu_separator(&mut commands, menu);

    // 选择相关
    spawn_menu_item(
        &mut commands,
        menu,
        "复制",
        "Ctrl+C",
        ContextMenuAction::SelectionCopy,
        has_sel,
    );
    spawn_menu_item(
        &mut commands,
        menu,
        "剪切",
        "Ctrl+X",
        ContextMenuAction::SelectionCut,
        has_sel,
    );
    spawn_menu_item(
        &mut commands,
        menu,
        "删除",
        "Del",
        ContextMenuAction::SelectionDelete,
        has_sel,
    );
    spawn_menu_separator(&mut commands, menu);
    spawn_menu_item(
        &mut commands,
        menu,
        "全选",
        "Ctrl+A",
        ContextMenuAction::SelectionSelectAll,
        true,
    );
    spawn_menu_item(
        &mut commands,
        menu,
        "取消选择",
        "Ctrl+D",
        ContextMenuAction::SelectionDeselect,
        has_sel,
    );

    // 旋转/翻转：右键指向的地图块有内容即可；或用剪贴板预设粘贴变换。
    if can_transform {
        spawn_menu_separator(&mut commands, menu);
        spawn_menu_item(
            &mut commands,
            menu,
            "逆时针旋转",
            "Q",
            ContextMenuAction::PasteRotateCcw,
            can_transform,
        );
        spawn_menu_item(
            &mut commands,
            menu,
            "顺时针旋转",
            "E",
            ContextMenuAction::PasteRotateCw,
            can_transform,
        );
        spawn_menu_item(
            &mut commands,
            menu,
            "水平翻转",
            "H",
            ContextMenuAction::PasteFlipX,
            can_transform,
        );
        spawn_menu_item(
            &mut commands,
            menu,
            "垂直翻转",
            "V",
            ContextMenuAction::PasteFlipY,
            can_transform,
        );
        spawn_menu_item(
            &mut commands,
            menu,
            "重置变换",
            "",
            ContextMenuAction::PasteReset,
            can_transform,
        );
        if in_paste {
            spawn_menu_item(
                &mut commands,
                menu,
                "退出粘贴",
                "Esc",
                ContextMenuAction::ExitPaste,
                true,
            );
        }
    }
}

/// 同步菜单位置/可见性（根据资源 `ContextMenuState`）。
pub fn context_menu_sync(
    windows: Query<&Window, With<PrimaryWindow>>,
    state: Res<ContextMenuState>,
    mut q: Query<&mut Node, (With<ContextMenuRoot>, Without<ContextMenuBackdrop>)>,
    mut backdrop_q: Query<&mut Node, (With<ContextMenuBackdrop>, Without<ContextMenuRoot>)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let Ok(mut node) = q.single_mut() else {
        return;
    };
    let Ok(mut backdrop) = backdrop_q.single_mut() else {
        return;
    };

    if !state.open {
        node.display = Display::None;
        backdrop.display = Display::None;
        return;
    }

    node.display = Display::Flex;
    backdrop.display = Display::Flex;
    let margin = 6.0;
    let x = state.screen_pos.x.clamp(0.0, window.width()).max(margin);
    let y = state.screen_pos.y.clamp(0.0, window.height()).max(margin);
    node.left = Val::Px(x);
    node.top = Val::Px(y);
}

/// 点击遮罩关闭菜单。
pub fn context_menu_backdrop_click(
    mut menu: ResMut<ContextMenuState>,
    buttons: Res<ButtonInput<MouseButton>>,
    backdrop_q: Query<&Interaction, With<ContextMenuBackdrop>>,
) {
    if !menu.open {
        return;
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }

    let Ok(interaction) = backdrop_q.single() else {
        return;
    };

    if *interaction == Interaction::Hovered || *interaction == Interaction::Pressed {
        menu.open = false;
        menu.consume_left_click = true;
    }
}

/// 菜单项 hover/pressed 样式。
pub fn context_menu_item_styles(
    mut q: Query<
        (&Interaction, &mut BackgroundColor, Option<&ContextMenuDisabled>),
        (Changed<Interaction>, With<ContextMenuItem>),
    >,
) {
    for (interaction, mut bg, disabled) in q.iter_mut() {
        if disabled.is_some() {
            *bg = BackgroundColor(Color::srgb(0.18, 0.18, 0.18));
            continue;
        }
        *bg = match *interaction {
            Interaction::Pressed => BackgroundColor(UI_BUTTON_PRESS),
            Interaction::Hovered => BackgroundColor(UI_BUTTON_HOVER),
            Interaction::None => BackgroundColor(UI_BUTTON),
        };
    }
}

/// 菜单项点击：写入 `ContextMenuCommand`，由 world 侧统一执行。
pub fn context_menu_item_click(
    mut menu: ResMut<ContextMenuState>,
    mut cmd: ResMut<ContextMenuCommand>,
    buttons: Res<ButtonInput<MouseButton>>,
    q: Query<(&Interaction, &ContextMenuItem, Option<&ContextMenuDisabled>)>,
) {
    if !menu.open {
        return;
    }

    // 只在鼠标左键刚释放时检查
    if !buttons.just_released(MouseButton::Left) {
        return;
    }

    for (interaction, item, disabled) in q.iter() {
        // Bevy UI: Interaction::Pressed 在 just_released 后才结算，但在某些版本需要检查 Hovered + just_released
        if *interaction == Interaction::Pressed
            || (*interaction == Interaction::Hovered && buttons.just_released(MouseButton::Left))
        {
            if disabled.is_some() {
                // RPG Maker / 原生菜单手感：点到禁用项不关闭菜单。
                menu.consume_left_click = true;
                return;
            }

            cmd.action = Some(item.0);

            menu.open = false;
            menu.consume_left_click = true;
            return;
        }
    }
}
