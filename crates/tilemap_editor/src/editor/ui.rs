//! 编辑器 UI：左侧工具栏 + tile palette + HUD。

use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use bevy::window::PrimaryWindow;
use bevy::ecs::message::MessageReader;

use super::{
	LEFT_PANEL_WIDTH_PX, TILE_BUTTON_PX, UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS,
	UI_HIGHLIGHT, UI_PANEL, RIGHT_TOPBAR_HEIGHT_PX,
};
use super::persistence::{load_map_from_file, save_map_to_file};
use super::tileset::{merge_tilesets_from_map, open_tileset_impl, rect_for_tile_index, save_tileset_library};
use super::types::{
	ActionButton, ActionKind, CanvasRoot, EditorConfig, EditorState, HudText,
	Clipboard,
	ContextMenuAction, ContextMenuCommand, ContextMenuDisabled, ContextMenuItem, ContextMenuRoot,
	ContextMenuBackdrop,
	ContextMenuState, SelectionState,
	MapSizeApplyButton, MapSizeFocus, MapSizeHeightField, MapSizeHeightText, MapSizeInput,
	MapSizeWidthField, MapSizeWidthText,
	PaletteRoot, PaletteScroll, PaletteTileButton, TileEntities, TileMapData, TilesetLibrary,
	TilesetActiveLabel, TilesetBar, TilesetCategoryCycleButton, TilesetCategoryLabel, TilesetLoading,
	TilesetMenuRoot, TilesetRuntime, TilesetSelectItem, TilesetToggleButton, UiRoot, UiState,
	DEFAULT_UI_FONT_PATH, PasteState, ShiftMapMode, ShiftMapSettings, ShiftModeButton, ShiftModeLabel,
	ToolButton, ToolKind, ToolState, UiFont, UndoStack,
};
use super::world::apply_map_to_entities;

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

fn resized_map_copy(old: Option<&TileMapData>, width: u32, height: u32) -> TileMapData {
	let mut new_map = TileMapData::new(width, height);
	let Some(old) = old else {
		return new_map;
	};

	let copy_w = old.width.min(width);
	let copy_h = old.height.min(height);
	let layers_to_copy = old.layers.min(new_map.layers).max(1);
	for layer in 0..layers_to_copy {
		for y in 0..copy_h {
			for x in 0..copy_w {
				let src = old.idx_layer(layer, x, y);
				let dst = new_map.idx_layer(layer, x, y);
				if src < old.tiles.len() && dst < new_map.tiles.len() {
					new_map.tiles[dst] = old.tiles[src].clone();
				}
			}
		}
	}
	new_map
}

/// UI 初始化：HUD + 左侧面板。
pub fn setup_ui(mut commands: Commands) {
	commands.spawn((
		Text::new("按 O 或点【打开】导入 tileset"),
		TextFont {
			font_size: 16.0,
			..default()
		},
		TextColor(Color::WHITE),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Px(10.0),
			left: Val::Px(10.0),
			..default()
		},
		HudText,
	));

	spawn_ui_root(&mut commands);
	spawn_context_menu(&mut commands);
}

fn spawn_context_menu(commands: &mut Commands) {
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
		if *interaction == Interaction::Pressed || (*interaction == Interaction::Hovered && buttons.just_released(MouseButton::Left)) {
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

/// 左侧 palette 的鼠标滚轮滚动。
///
/// Bevy UI 的滚动通过修改 `ScrollPosition` 完成；Node 默认就带有 `ScrollPosition` 组件。
pub fn palette_scroll_wheel(
	mut wheel: MessageReader<MouseWheel>,
	windows: Query<&Window, With<PrimaryWindow>>,
	mut scroll_q: Query<&mut ScrollPosition, With<PaletteScroll>>,
) {
	let Ok(window) = windows.single() else {
		return;
	};
	let Some(cursor) = window.cursor_position() else {
		return;
	};

	// 鼠标在左侧面板范围内才滚动（避免影响右侧缩放）
	if cursor.x > LEFT_PANEL_WIDTH_PX {
		return;
	}

	let mut delta_y: f32 = 0.0;
	for ev in wheel.read() {
		// ScrollPosition.y 增大 => 内容向上移动（视觉上向下滚动）
		// 为了符合常见体验：滚轮向下(负) => 视图向下 => ScrollPosition.y 增大
		delta_y += -ev.y;
	}
	if delta_y.abs() < f32::EPSILON {
		return;
	}

	let speed = 40.0;
	for mut scroll in scroll_q.iter_mut() {
		scroll.0.y = (scroll.0.y + delta_y * speed).max(0.0);
	}
}

fn spawn_ui_root(commands: &mut Commands) {
	let root = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				flex_direction: FlexDirection::Row,
				..default()
			},
			// 重要：UI 画在世界之上。这里必须透明，否则会把世界渲染整块盖住。
			BackgroundColor(Color::NONE),
			UiRoot,
		))
		.id();

	let left_panel = commands
		.spawn((
			Node {
				width: Val::Px(LEFT_PANEL_WIDTH_PX),
				height: Val::Percent(100.0),
				flex_direction: FlexDirection::Column,
				padding: UiRect::all(Val::Px(10.0)),
				row_gap: Val::Px(10.0),
				..default()
			},
			BackgroundColor(UI_PANEL),
		))
		.id();

	let toolbar = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Auto,
				flex_direction: FlexDirection::Row,
				flex_wrap: FlexWrap::Wrap,
				column_gap: Val::Px(8.0),
				row_gap: Val::Px(8.0),
				..default()
			},
		))
		.id();

	commands.entity(toolbar).with_children(|p| {
		// 打开
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ActionButton(ActionKind::OpenTileset),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("打开(O)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		// 新建
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ActionButton(ActionKind::NewMap),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("新建(R)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		// 保存
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ActionButton(ActionKind::SaveMap),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("保存(S)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		// 读取
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ActionButton(ActionKind::LoadMap),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("读取(L)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		// 导入地图
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ActionButton(ActionKind::ImportMap),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("导入"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		// 导出地图
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ActionButton(ActionKind::ExportMap),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("导出"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		// --- 工具栏（参考 RM：铅笔/矩形/填充/选择） ---
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ToolButton(ToolKind::Pencil),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("笔刷(1)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ToolButton(ToolKind::Eraser),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("橡皮(6)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ToolButton(ToolKind::Rect),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("矩形(2)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ToolButton(ToolKind::Fill),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("填充(3)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ToolButton(ToolKind::Select),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("选择(4)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		// 不再提供“粘贴工具”按钮：粘贴通过 Ctrl+V / 右键菜单进入，避免与菜单重复。

		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ToolButton(ToolKind::Eyedropper),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("吸管(I)"),
				TextFont {
					font_size: 14.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

	});

	let tileset_bar = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Auto,
				overflow: Overflow::visible(),
				flex_direction: FlexDirection::Row,
				align_items: AlignItems::Center,
				column_gap: Val::Px(8.0),
				padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
				..default()
			},
			BackgroundColor(UI_PANEL),
			ZIndex(1000),
			TilesetBar,
		))
		.id();

	commands.entity(tileset_bar).with_children(|p| {
		p.spawn((
			Text::new("分类:"),
			TextFont {
				font_size: 13.0,
				..default()
			},
			TextColor(Color::WHITE),
		));
		p.spawn((
			Text::new("全部"),
			TextFont {
				font_size: 13.0,
				..default()
			},
			TextColor(Color::WHITE),
			TilesetCategoryLabel,
		));
		p.spawn((
			Button,
			Node {
				height: Val::Px(28.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			TilesetCategoryCycleButton,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("分类"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});
		p.spawn((
			Button,
			Node {
				height: Val::Px(28.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			TilesetToggleButton,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("选择"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});
	});

	let tileset_menu = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				max_height: Val::Px(360.0),
				overflow: Overflow::scroll_y(),
				position_type: PositionType::Absolute,
				top: Val::Px(32.0),
				left: Val::Px(0.0),
				flex_direction: FlexDirection::Column,
				row_gap: Val::Px(6.0),
				padding: UiRect::all(Val::Px(8.0)),
				..default()
			},
			BackgroundColor(UI_PANEL),
			ZIndex(2000),
			Visibility::Hidden,
			TilesetMenuRoot,
		))
		.id();

	let palette_scroll = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				flex_grow: 1.0,
				overflow: Overflow::scroll_y(),
				..default()
			},
			PaletteScroll,
		))
		.id();

	let palette_root = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				flex_direction: FlexDirection::Row,
				flex_wrap: FlexWrap::Wrap,
				column_gap: Val::Px(6.0),
				row_gap: Val::Px(6.0),
				..default()
			},
			PaletteRoot,
		))
		.id();

	let right_panel = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				flex_direction: FlexDirection::Column,
				..default()
			},
			BackgroundColor(Color::NONE),
			CanvasRoot,
		))
		.id();

	let right_topbar = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Px(RIGHT_TOPBAR_HEIGHT_PX),
				flex_direction: FlexDirection::Row,
				flex_wrap: FlexWrap::Wrap,
				align_items: AlignItems::Center,
				padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
				column_gap: Val::Px(8.0),
				row_gap: Val::Px(8.0),
				..default()
			},
			BackgroundColor(UI_PANEL),
		))
		.id();

	let right_content = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				flex_grow: 1.0,
				..default()
			},
			BackgroundColor(Color::NONE),
		))
		.id();

	commands.entity(right_panel).add_child(right_topbar);
	commands.entity(right_panel).add_child(right_content);

	commands.entity(right_topbar).with_children(|p| {
		p.spawn((
			Text::new("地图尺寸:"),
			TextFont {
				font_size: 13.0,
				..default()
			},
			TextColor(Color::WHITE),
		));

		for (w, h) in [(40u32, 25u32), (64u32, 36u32), (100u32, 60u32)] {
			p.spawn((
				Button,
				Node {
					height: Val::Px(36.0),
					padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
					align_items: AlignItems::Center,
					justify_content: JustifyContent::Center,
					..default()
				},
				BackgroundColor(UI_BUTTON),
				ActionButton(ActionKind::SetMapSize { width: w, height: h }),
			))
			.with_children(|p| {
				p.spawn((
					Text::new(format!("{w}x{h}")),
					TextFont {
						font_size: 13.0,
						..default()
					},
					TextColor(Color::WHITE),
				));
			});
		}

		p.spawn((
			Text::new("自定义:"),
			TextFont {
				font_size: 13.0,
				..default()
			},
			TextColor(Color::WHITE),
		));

		// 宽
		p.spawn((
			Button,
			Node {
				width: Val::Px(86.0),
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			MapSizeWidthField,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("W"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
			p.spawn((
				Text::new(""),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
				MapSizeWidthText,
			));
		});

		// 高
		p.spawn((
			Button,
			Node {
				width: Val::Px(86.0),
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			MapSizeHeightField,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("H"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
			p.spawn((
				Text::new(""),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
				MapSizeHeightText,
			));
		});

		// 应用
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			MapSizeApplyButton,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("应用(Enter)"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		// Shift Map 模式：空白 / 环绕
		p.spawn((
			Text::new("Shift:"),
			TextFont {
				font_size: 13.0,
				..default()
			},
			TextColor(Color::WHITE),
		));
		p.spawn((
			Button,
			Node {
				height: Val::Px(36.0),
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			ShiftModeButton,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("空白"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
				ShiftModeLabel,
			));
		});
	});

	commands.entity(palette_scroll).add_child(palette_root);
	commands.entity(tileset_bar).add_child(tileset_menu);
	commands.entity(left_panel).add_child(toolbar);
	commands.entity(left_panel).add_child(tileset_bar);
	commands.entity(left_panel).add_child(palette_scroll);
	commands.entity(root).add_child(left_panel);
	commands.entity(root).add_child(right_panel);
}

pub fn map_size_widget_interactions(
	mut input: ResMut<MapSizeInput>,
	mut width_q: Query<
		(&Interaction, &mut BackgroundColor),
		(
			Changed<Interaction>,
			With<MapSizeWidthField>,
			Without<MapSizeHeightField>,
			Without<MapSizeApplyButton>,
		),
	>,
	mut height_q: Query<
		(&Interaction, &mut BackgroundColor),
		(
			Changed<Interaction>,
			With<MapSizeHeightField>,
			Without<MapSizeWidthField>,
			Without<MapSizeApplyButton>,
		),
	>,
	mut apply_q: Query<
		(&Interaction, &mut BackgroundColor),
		(
			Changed<Interaction>,
			With<MapSizeApplyButton>,
			Without<MapSizeWidthField>,
			Without<MapSizeHeightField>,
		),
	>,
) {
	for (interaction, mut bg) in width_q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				input.focus = MapSizeFocus::Width;
				*bg = BackgroundColor(UI_HIGHLIGHT);
			}
			Interaction::Hovered => {
				if input.focus == MapSizeFocus::Width {
					*bg = BackgroundColor(UI_HIGHLIGHT);
				} else {
					*bg = BackgroundColor(UI_BUTTON_HOVER);
				}
			}
			Interaction::None => {
				if input.focus == MapSizeFocus::Width {
					*bg = BackgroundColor(UI_HIGHLIGHT);
				} else {
					*bg = BackgroundColor(UI_BUTTON);
				}
			}
		}
	}

	for (interaction, mut bg) in height_q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				input.focus = MapSizeFocus::Height;
				*bg = BackgroundColor(UI_HIGHLIGHT);
			}
			Interaction::Hovered => {
				if input.focus == MapSizeFocus::Height {
					*bg = BackgroundColor(UI_HIGHLIGHT);
				} else {
					*bg = BackgroundColor(UI_BUTTON_HOVER);
				}
			}
			Interaction::None => {
				if input.focus == MapSizeFocus::Height {
					*bg = BackgroundColor(UI_HIGHLIGHT);
				} else {
					*bg = BackgroundColor(UI_BUTTON);
				}
			}
		}
	}

	for (interaction, mut bg) in apply_q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				*bg = BackgroundColor(UI_BUTTON_PRESS);
				input.apply_requested = true;
			}
			Interaction::Hovered => {
				*bg = BackgroundColor(UI_BUTTON_HOVER);
			}
			Interaction::None => {
				*bg = BackgroundColor(UI_BUTTON);
			}
		}
	}
}

pub fn tool_button_click(
	mut tools: ResMut<ToolState>,
	mut paste: ResMut<PasteState>,
	mut q: Query<(&Interaction, &ToolButton, &mut BackgroundColor), Changed<Interaction>>,
) {
	let mut picked: Option<ToolKind> = None;
	for (interaction, btn, mut bg) in q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				picked = Some(btn.0);
				*bg = BackgroundColor(UI_BUTTON_PRESS);
			}
			Interaction::Hovered => {
				*bg = BackgroundColor(UI_BUTTON_HOVER);
			}
			Interaction::None => {
				*bg = BackgroundColor(UI_BUTTON);
			}
		}
	}

	if let Some(next) = picked {
		tools.tool = next;
		if next == ToolKind::Paste {
			*paste = PasteState::default();
		}
	}
}

pub fn sync_tool_button_styles(
	tools: Res<ToolState>,
	mut q: Query<(&ToolButton, &Interaction, &mut BackgroundColor)>,
) {
	for (btn, interaction, mut bg) in q.iter_mut() {
		if btn.0 == tools.tool {
			*bg = BackgroundColor(UI_HIGHLIGHT);
			continue;
		}

		*bg = match *interaction {
			Interaction::Pressed => BackgroundColor(UI_BUTTON_PRESS),
			Interaction::Hovered => BackgroundColor(UI_BUTTON_HOVER),
			Interaction::None => BackgroundColor(UI_BUTTON),
		};
	}
}

pub fn shift_mode_button_click(
	mut settings: ResMut<ShiftMapSettings>,
	mut q: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<ShiftModeButton>)>,
) {
	for (interaction, mut bg) in q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				settings.mode = match settings.mode {
					ShiftMapMode::Blank => ShiftMapMode::Wrap,
					ShiftMapMode::Wrap => ShiftMapMode::Blank,
				};
				*bg = BackgroundColor(UI_BUTTON_PRESS);
			}
			Interaction::Hovered => {
				*bg = BackgroundColor(UI_BUTTON_HOVER);
			}
			Interaction::None => {
				*bg = BackgroundColor(UI_BUTTON);
			}
		}
	}
}

pub fn update_shift_mode_label(
	settings: Res<ShiftMapSettings>,
	mut q: Query<&mut Text, With<ShiftModeLabel>>,
) {
	if !settings.is_changed() {
		return;
	}

	let label = match settings.mode {
		ShiftMapMode::Blank => "空白",
		ShiftMapMode::Wrap => "环绕",
	};
	for mut t in q.iter_mut() {
		*t = Text::new(label);
	}
}

pub fn map_size_text_input(
	keys: Res<ButtonInput<KeyCode>>,
	mut input: ResMut<MapSizeInput>,
) {
	if input.focus == MapSizeFocus::None {
		return;
	}

	let buf = match input.focus {
		MapSizeFocus::Width => &mut input.width_buf,
		MapSizeFocus::Height => &mut input.height_buf,
		MapSizeFocus::None => return,
	};

	// Bevy 0.18 在不同平台/后端下字符输入事件类型可能不同；
	// 这里用 KeyCode 录入数字（主键盘 0-9 + 小键盘 0-9）确保稳定。
	let digit_keys: &[(KeyCode, char)] = &[
		(KeyCode::Digit0, '0'),
		(KeyCode::Digit1, '1'),
		(KeyCode::Digit2, '2'),
		(KeyCode::Digit3, '3'),
		(KeyCode::Digit4, '4'),
		(KeyCode::Digit5, '5'),
		(KeyCode::Digit6, '6'),
		(KeyCode::Digit7, '7'),
		(KeyCode::Digit8, '8'),
		(KeyCode::Digit9, '9'),
		(KeyCode::Numpad0, '0'),
		(KeyCode::Numpad1, '1'),
		(KeyCode::Numpad2, '2'),
		(KeyCode::Numpad3, '3'),
		(KeyCode::Numpad4, '4'),
		(KeyCode::Numpad5, '5'),
		(KeyCode::Numpad6, '6'),
		(KeyCode::Numpad7, '7'),
		(KeyCode::Numpad8, '8'),
		(KeyCode::Numpad9, '9'),
	];

	for (key, ch) in digit_keys {
		if keys.just_pressed(*key) {
			if buf.len() < 5 {
				buf.push(*ch);
			}
		}
	}

	if keys.just_pressed(KeyCode::Backspace) {
		buf.pop();
	}

	if keys.just_pressed(KeyCode::Escape) {
		input.focus = MapSizeFocus::None;
	}

	if keys.just_pressed(KeyCode::Enter) {
		input.apply_requested = true;
		input.focus = MapSizeFocus::None;
	}
}

pub fn sync_map_size_input_from_config(config: Res<EditorConfig>, mut input: ResMut<MapSizeInput>) {
	// 不在编辑时，跟随当前 config（导入/预设/其它系统修改 map_size 时也能同步到输入框）
	if input.focus != MapSizeFocus::None {
		return;
	}

	let w = config.map_size.x.to_string();
	let h = config.map_size.y.to_string();
	if input.width_buf != w {
		input.width_buf = w;
	}
	if input.height_buf != h {
		input.height_buf = h;
	}
}

pub fn update_map_size_field_text(
	input: Res<MapSizeInput>,
	mut w_q: Query<&mut Text, (With<MapSizeWidthText>, Without<MapSizeHeightText>)>,
	mut h_q: Query<&mut Text, (With<MapSizeHeightText>, Without<MapSizeWidthText>)>,
) {
	let w = if input.focus == MapSizeFocus::Width {
		format!(" {}|", input.width_buf)
	} else {
		format!(" {}", input.width_buf)
	};
	let h = if input.focus == MapSizeFocus::Height {
		format!(" {}|", input.height_buf)
	} else {
		format!(" {}", input.height_buf)
	};

	for mut t in w_q.iter_mut() {
		*t = Text::new(w.clone());
	}
	for mut t in h_q.iter_mut() {
		*t = Text::new(h.clone());
	}
}

pub fn apply_custom_map_size(
	mut commands: Commands,
	mut input: ResMut<MapSizeInput>,
	mut config: ResMut<EditorConfig>,
	runtime: Res<TilesetRuntime>,
	existing_tiles: Option<Res<TileEntities>>,
	mut sprite_vis_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
	map: Option<ResMut<TileMapData>>,
	mut undo: ResMut<UndoStack>,
) {
	if !input.apply_requested {
		return;
	}
	input.apply_requested = false;

	let Ok(width) = input.width_buf.parse::<u32>() else {
		return;
	};
	let Ok(height) = input.height_buf.parse::<u32>() else {
		return;
	};
	if width == 0 || height == 0 {
		return;
	}

	let old_map = map.as_deref();
	let new_map = resized_map_copy(old_map, width, height);

	config.map_size = UVec2::new(width, height);
	commands.insert_resource(new_map.clone());
	undo.clear();

	// 重建格子实体
	if let Some(existing_tiles) = existing_tiles.as_deref() {
		for &e in &existing_tiles.entities {
			commands.entity(e).despawn();
		}
	}
	commands.remove_resource::<TileEntities>();
	let tiles = super::tileset::spawn_map_entities_with_layers(&mut commands, &config, new_map.layers);
	apply_map_to_entities(&runtime, &new_map, &tiles, &mut sprite_vis_q, &config);
	commands.insert_resource(tiles);
}

/// tileset 加载完成后，动态生成左侧 palette（缩略图按钮网格）。
pub fn build_palette_when_ready(
	mut commands: Commands,
	config: Res<EditorConfig>,
	lib: Res<TilesetLibrary>,
	runtime: Res<TilesetRuntime>,
	mut ui_state: ResMut<UiState>,
	palette_q: Query<Entity, With<PaletteRoot>>,
	palette_children_q: Query<&Children>,
) {
	let Some(active_id) = lib.active_id.as_ref() else {
		return;
	};
	let Some(active) = runtime.by_id.get(active_id) else {
		return;
	};
	if ui_state.built_for_tileset_path == *active_id {
		return;
	}

	let Ok(palette_entity) = palette_q.single() else {
		return;
	};

	// 清理旧 palette（需要递归删除，否则子节点会残留）
	if let Ok(children) = palette_children_q.get(palette_entity) {
		for child in children.iter() {
			// button 的子节点是 ImageNode；这里手动删一层即可
			if let Ok(grandchildren) = palette_children_q.get(child) {
				for grandchild in grandchildren.iter() {
					commands.entity(grandchild).despawn();
				}
			}
			commands.entity(child).despawn();
		}
	}

	let tile_count = active.columns.saturating_mul(active.rows);
	let image = active.texture.clone();
	let columns = active.columns.max(1);

	commands.entity(palette_entity).with_children(|p| {
		for index in 0..tile_count {
			let rect = rect_for_tile_index(index, columns, config.tile_size);

			p.spawn((
				Button,
				Node {
					width: Val::Px(TILE_BUTTON_PX),
					height: Val::Px(TILE_BUTTON_PX),
					padding: UiRect::all(Val::Px(2.0)),
					..default()
				},
				BackgroundColor(UI_BUTTON),
				PaletteTileButton { index },
			))
			.with_children(|p| {
				// ImageNode 支持 rect 裁剪，从同一张 tileset 中取出缩略图
				p.spawn((
					ImageNode::new(image.clone())
						.with_rect(rect)
						.with_mode(NodeImageMode::Stretch),
					Node {
						width: Val::Percent(100.0),
						height: Val::Percent(100.0),
						..default()
					},
				));
			});
		}
	});

	ui_state.built_for_tileset_path = active_id.clone();
}

/// tileset 选择栏：更新当前选中 tileset 的显示文本。
pub fn update_tileset_active_label(
	lib: Res<TilesetLibrary>,
	mut label_q: Query<&mut Text, With<TilesetActiveLabel>>,
) {
	if !lib.is_changed() {
		return;
	}

	let label = match lib.active_id.as_ref() {
		Some(id) => lib
			.entries
			.iter()
			.find(|e| &e.id == id)
			.map(|e| {
				if !e.name.trim().is_empty() {
					e.name.clone()
				} else if !e.asset_path.trim().is_empty() {
					e.asset_path.clone()
				} else {
					id.clone()
				}
			})
			.unwrap_or_else(|| id.clone()),
		None => "(未选择)".to_string(),
	};

	for mut t in label_q.iter_mut() {
		*t = Text::new(label.clone());
	}
}

pub fn update_tileset_category_label(
	lib: Res<TilesetLibrary>,
	mut label_q: Query<&mut Text, With<TilesetCategoryLabel>>,
) {
	if !lib.is_changed() {
		return;
	}

	let label = if lib.active_category.trim().is_empty() {
		"全部".to_string()
	} else {
		lib.active_category.clone()
	};
	for mut t in label_q.iter_mut() {
		*t = Text::new(label.clone());
	}
}

/// tileset 菜单开关按钮。
pub fn tileset_toggle_button_click(
	mut ui_state: ResMut<UiState>,
	mut q: Query<
		(&Interaction, &mut BackgroundColor),
		(Changed<Interaction>, With<TilesetToggleButton>),
	>,
) {
	for (interaction, mut bg) in q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				ui_state.tileset_menu_open = !ui_state.tileset_menu_open;
				*bg = BackgroundColor(UI_BUTTON_PRESS);
			}
			Interaction::Hovered => {
				*bg = BackgroundColor(UI_BUTTON_HOVER);
			}
			Interaction::None => {
				*bg = BackgroundColor(UI_BUTTON);
			}
		}
	}
}

/// 分类切换：循环切换当前分类过滤（全部 -> 各分类 -> 全部）。
pub fn tileset_category_cycle_click(
	mut lib: ResMut<TilesetLibrary>,
	mut ui_state: ResMut<UiState>,
	mut q: Query<
		(&Interaction, &mut BackgroundColor),
		(Changed<Interaction>, With<TilesetCategoryCycleButton>),
	>,
) {
	let mut clicked = false;
	for (interaction, mut bg) in q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				clicked = true;
				*bg = BackgroundColor(UI_BUTTON_PRESS);
			}
			Interaction::Hovered => {
				*bg = BackgroundColor(UI_BUTTON_HOVER);
			}
			Interaction::None => {
				*bg = BackgroundColor(UI_BUTTON);
			}
		}
	}

	if !clicked {
		return;
	}

	let mut cats: Vec<String> = lib
		.entries
		.iter()
		.map(|e| e.category.trim().to_string())
		.filter(|c| !c.is_empty())
		.collect();
	cats.sort();
	cats.dedup();

	let current = lib.active_category.trim().to_string();
	let next = if cats.is_empty() {
		String::new()
	} else if current.is_empty() {
		cats[0].clone()
	} else {
		match cats.iter().position(|c| c == &current) {
			Some(i) if i + 1 < cats.len() => cats[i + 1].clone(),
			_ => String::new(),
		}
	};

	lib.active_category = next;
	save_tileset_library(&lib);
	ui_state.built_tileset_menu_category.clear();
}

/// 根据 UiState 显示/隐藏 tileset 菜单。
pub fn tileset_menu_visibility(
	ui_state: Res<UiState>,
	mut menu_q: Query<&mut Visibility, With<TilesetMenuRoot>>,
) {
	let Ok(mut vis) = menu_q.single_mut() else {
		return;
	};
	*vis = if ui_state.tileset_menu_open {
		Visibility::Visible
	} else {
		Visibility::Hidden
	};
}

/// 当菜单打开或库发生变化时，重建 tileset 列表按钮。
pub fn rebuild_tileset_menu_when_needed(
	mut commands: Commands,
	lib: Res<TilesetLibrary>,
	mut ui_state: ResMut<UiState>,
	menu_q: Query<Entity, With<TilesetMenuRoot>>,
	children_q: Query<&Children>,
) {
	if !ui_state.tileset_menu_open {
		return;
	}

	let Ok(menu_entity) = menu_q.single() else {
		return;
	};

	let active = lib.active_id.clone().unwrap_or_default();
	let category = lib.active_category.trim().to_string();
	let needs_rebuild = lib.is_changed()
		|| ui_state.built_tileset_menu_count != lib.entries.len()
		|| ui_state.built_tileset_menu_active_id != active
		|| ui_state.built_tileset_menu_category != category;
	if !needs_rebuild {
		return;
	}

	// 清理旧菜单（按钮只有一层子节点：Text）
	if let Ok(children) = children_q.get(menu_entity) {
		for child in children.iter() {
			if let Ok(grandchildren) = children_q.get(child) {
				for grandchild in grandchildren.iter() {
					commands.entity(grandchild).despawn();
				}
			}
			commands.entity(child).despawn();
		}
	}

	commands.entity(menu_entity).with_children(|p| {
		let entries: Vec<_> = lib
			.entries
			.iter()
			.filter(|e| category.is_empty() || e.category.trim() == category)
			.collect();

		if entries.is_empty() {
			p.spawn((
				Text::new(if category.is_empty() {
					"(库为空：点击【打开】导入 tileset)"
				} else {
					"(该分类下没有 tileset)"
				}),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
			return;
		}

		for e in entries {
			let label = if !e.name.trim().is_empty() {
				e.name.clone()
			} else if !e.asset_path.trim().is_empty() {
				e.asset_path.clone()
			} else {
				e.id.clone()
			};
			let label = if !e.category.trim().is_empty() {
				format!("{}  [{}]", label, e.category)
			} else {
				label
			};

			let is_active = lib.active_id.as_ref().is_some_and(|id| id == &e.id);
			p.spawn((
				Button,
				Node {
					width: Val::Percent(100.0),
					height: Val::Px(32.0),
					padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
					align_items: AlignItems::Center,
					..default()
				},
				BackgroundColor(if is_active { UI_HIGHLIGHT } else { UI_BUTTON }),
				TilesetSelectItem { id: e.id.clone() },
			))
			.with_children(|p| {
				p.spawn((
					Text::new(label),
					TextFont {
						font_size: 13.0,
						..default()
					},
					TextColor(Color::WHITE),
				));
			});
		}
	});

	ui_state.built_tileset_menu_count = lib.entries.len();
	ui_state.built_tileset_menu_active_id = active;
	ui_state.built_tileset_menu_category = category;
}

/// 点击菜单项切换当前 tileset。
pub fn tileset_menu_item_click(
	mut lib: ResMut<TilesetLibrary>,
	runtime: Res<TilesetRuntime>,
	mut editor_state: ResMut<EditorState>,
	mut ui_state: ResMut<UiState>,
	mut q: Query<(&Interaction, &TilesetSelectItem, &mut BackgroundColor), Changed<Interaction>>,
) {
	let mut picked: Option<String> = None;

	for (interaction, item, mut bg) in q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				picked = Some(item.id.clone());
				*bg = BackgroundColor(UI_BUTTON_PRESS);
			}
			Interaction::Hovered => {
				*bg = BackgroundColor(UI_BUTTON_HOVER);
			}
			Interaction::None => {
				let is_active = lib.active_id.as_ref().is_some_and(|id| id == &item.id);
				*bg = BackgroundColor(if is_active { UI_HIGHLIGHT } else { UI_BUTTON });
			}
		}
	}

	let Some(id) = picked else {
		return;
	};

	lib.active_id = Some(id.clone());
	save_tileset_library(&lib);
	ui_state.tileset_menu_open = false;
	ui_state.built_for_tileset_path.clear();
	ui_state.built_tileset_menu_active_id.clear();

	if let Some(atlas) = runtime.by_id.get(&id) {
		let tile_count = atlas.columns.saturating_mul(atlas.rows).max(1);
		editor_state.selected_tile = editor_state.selected_tile.min(tile_count - 1);
	}
}

/// palette 点击选择 tile。
pub fn palette_tile_click(
	mut state: ResMut<EditorState>,
	mut buttons_q: Query<(&Interaction, &PaletteTileButton, &mut BackgroundColor), Changed<Interaction>>,
) {
	let selected_before = state.selected_tile;
	for (interaction, tile, mut bg) in buttons_q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				state.selected_tile = tile.index;
				*bg = BackgroundColor(UI_HIGHLIGHT);
			}
			Interaction::Hovered => {
				if selected_before != tile.index {
					*bg = BackgroundColor(UI_BUTTON_HOVER);
				}
			}
			Interaction::None => {
				if selected_before == tile.index {
					*bg = BackgroundColor(UI_HIGHLIGHT);
				} else {
					*bg = BackgroundColor(UI_BUTTON);
				}
			}
		}
	}
}

/// 左侧工具栏按钮点击处理。
pub fn action_button_click(
	mut commands: Commands,
	mut action_q: Query<(&Interaction, &ActionButton, &mut BackgroundColor), Changed<Interaction>>,
	asset_server: Res<AssetServer>,
	mut config: ResMut<EditorConfig>,
	mut lib: ResMut<TilesetLibrary>,
	mut tileset_loading: ResMut<TilesetLoading>,
	runtime: Res<TilesetRuntime>,
	tile_entities: Option<Res<TileEntities>>,
	mut ui_state: ResMut<UiState>,
	mut sprite_vis_q: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
	map: Option<ResMut<TileMapData>>,
	mut undo: ResMut<UndoStack>,
) {
	let mut requested: Option<ActionKind> = None;

	for (interaction, action, mut bg) in action_q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				*bg = BackgroundColor(UI_BUTTON_PRESS);
				requested = Some(action.0);
			}
			Interaction::Hovered => {
				*bg = BackgroundColor(UI_BUTTON_HOVER);
			}
			Interaction::None => {
				*bg = BackgroundColor(UI_BUTTON);
			}
		}
	}

	let Some(requested) = requested else {
		return;
	};

	match requested {
		ActionKind::OpenTileset => {
			open_tileset_impl(&asset_server, &mut config, &mut lib, &mut tileset_loading);
			save_tileset_library(&lib);
			ui_state.built_for_tileset_path.clear();
		}
		ActionKind::SaveMap => {
			if let Some(map) = map.as_deref() {
				if let Err(err) = save_map_to_file(map, &lib, config.save_path.as_str()) {
					warn!("save failed: {err}");
				} else {
					info!("saved map: {}", config.save_path);
				}
			}
		}
		ActionKind::LoadMap => {
			let (loaded, tilesets) = match load_map_from_file(config.save_path.as_str()) {
				Ok(m) => m,
				Err(err) => {
					warn!("load failed: {err}");
					return;
				}
			};

			merge_tilesets_from_map(&asset_server, &mut lib, &mut tileset_loading, tilesets);
			save_tileset_library(&lib);
			ui_state.built_for_tileset_path.clear();
			undo.clear();

			// 尺寸变化：更新 config + 重建格子实体
			if config.map_size.x != loaded.width || config.map_size.y != loaded.height {
				if let Some(existing_tiles) = tile_entities.as_deref() {
					for &e in &existing_tiles.entities {
						commands.entity(e).despawn();
					}
				}
				commands.remove_resource::<TileEntities>();
				commands.remove_resource::<TileMapData>();

				config.map_size = UVec2::new(loaded.width, loaded.height);
				let tiles = super::tileset::spawn_map_entities_with_layers(&mut commands, &config, loaded.layers);
				commands.insert_resource(loaded.clone());
				apply_map_to_entities(&runtime, &loaded, &tiles, &mut sprite_vis_q, &config);
				commands.insert_resource(tiles);
				return;
			}

			commands.insert_resource(loaded.clone());
			if let Some(tile_entities) = tile_entities.as_deref() {
				apply_map_to_entities(&runtime, &loaded, tile_entities, &mut sprite_vis_q, &config);
			}
		}
		ActionKind::NewMap => {
			if let (Some(tile_entities), Some(mut map)) = (tile_entities.as_deref(), map) {
				*map = TileMapData::new(map.width, map.height);
				apply_map_to_entities(&runtime, &map, tile_entities, &mut sprite_vis_q, &config);
			}
		}
		ActionKind::SetMapSize { width, height } => {
			if width == 0 || height == 0 {
				return;
			}

			let old_map = map.as_deref();
			let new_map = resized_map_copy(old_map, width, height);

			config.map_size = UVec2::new(width, height);
			commands.insert_resource(new_map.clone());
			undo.clear();

			// 重建格子实体
			if let Some(existing_tiles) = tile_entities.as_deref() {
				for &e in &existing_tiles.entities {
					commands.entity(e).despawn();
				}
			}
			commands.remove_resource::<TileEntities>();
			let tiles = super::tileset::spawn_map_entities_with_layers(&mut commands, &config, new_map.layers);
			apply_map_to_entities(&runtime, &new_map, &tiles, &mut sprite_vis_q, &config);
			commands.insert_resource(tiles);
		}
		ActionKind::ImportMap => {
			let Some(path) = rfd::FileDialog::new()
				.add_filter("RON", &["ron"])
				.pick_file()
			else {
				return;
			};

			let (loaded, tilesets) = match load_map_from_file(path.to_string_lossy().as_ref()) {
				Ok(m) => m,
				Err(err) => {
					warn!("import failed: {err}");
					return;
				}
			};
			merge_tilesets_from_map(&asset_server, &mut lib, &mut tileset_loading, tilesets);
			save_tileset_library(&lib);
			ui_state.built_for_tileset_path.clear();
			undo.clear();

			// 尺寸变化：更新 config + 重建格子实体
			if config.map_size.x != loaded.width || config.map_size.y != loaded.height {
				if let Some(existing_tiles) = tile_entities.as_deref() {
					for &e in &existing_tiles.entities {
						commands.entity(e).despawn();
					}
				}
				commands.remove_resource::<TileEntities>();
				commands.remove_resource::<TileMapData>();

				config.map_size = UVec2::new(loaded.width, loaded.height);
				let tiles = super::tileset::spawn_map_entities_with_layers(&mut commands, &config, loaded.layers);
				commands.insert_resource(loaded.clone());
				apply_map_to_entities(&runtime, &loaded, &tiles, &mut sprite_vis_q, &config);
				commands.insert_resource(tiles);
				return;
			}

			// 更新地图数据（下一帧由绘制系统继续使用）
			commands.insert_resource(loaded.clone());

			// 若当前系统参数里已有 tile_entities，则本帧直接刷新可见性
			if let Some(tile_entities) = tile_entities.as_deref() {
				apply_map_to_entities(&runtime, &loaded, tile_entities, &mut sprite_vis_q, &config);
			}
		}
		ActionKind::ExportMap => {
			let Some(path) = rfd::FileDialog::new()
				.add_filter("RON", &["ron"])
				.set_file_name("map.ron")
				.save_file()
			else {
				return;
			};

			let Some(map) = map.as_deref() else {
				return;
			};

			if let Err(err) = save_map_to_file(map, &lib, path.to_string_lossy().as_ref()) {
				warn!("export failed: {err}");
			} else {
				info!("exported map: {}", path.to_string_lossy());
			}
		}
	}
}

/// 右上角 HUD 文案。
pub fn update_hud_text(
	mut commands: Commands,
	config: Res<EditorConfig>,
	state: Res<EditorState>,
	lib: Res<TilesetLibrary>,
	runtime: Res<TilesetRuntime>,
	tools: Res<ToolState>,
	layer_state: Res<super::types::LayerState>,
	map: Option<Res<TileMapData>>,
	clipboard: Res<Clipboard>,
	paste: Res<PasteState>,
	hud_q: Query<Entity, With<HudText>>,
) {
	let Some(hud_entity) = hud_q.iter().next() else {
		return;
	};

	let tile_count = lib
		.active_id
		.as_ref()
		.and_then(|id| runtime.by_id.get(id).map(|r| r.columns.saturating_mul(r.rows)))
		.unwrap_or(0);

	let (active_layer, total_layers) = map
		.as_deref()
		.map(|m| {
			let total = m.layers.max(1);
			(layer_state.active.min(total - 1), total)
		})
		.unwrap_or((layer_state.active, 1));
	let layer_name = match active_layer {
		0 => "Ground",
		1 => "Upper",
		_ => "Other",
	};

	let mut msg = if tile_count == 0 {
		"未选择 tileset：按 O 或点左上角【打开】导入".to_string()
	} else {
		format!(
			"选中 tile: {}\n地图: {} ({}x{})\n图层: {}/{} ({})\n图块: {}x{} | tiles: {}",
			state.selected_tile,
			config.save_path,
			config.map_size.x,
			config.map_size.y,
			active_layer + 1,
			total_layers,
			layer_name,
			config.tile_size.x,
			config.tile_size.y,
			tile_count
		)
	};

	if tools.tool == ToolKind::Paste {
		let rot = paste.rot % 4;
		let deg = (rot as u32) * 90;
		msg.push_str(&format!(
			"\n\n粘贴变换: {}° | flipX:{} flipY:{}",
			deg,
			if paste.flip_x { "开" } else { "关" },
			if paste.flip_y { "开" } else { "关" },
		));
		if tools.return_after_paste.is_some() {
			msg.push_str("\n模式: 临时粘贴（贴一次自动返回，按 Esc 取消）");
		} else {
			msg.push_str("\n模式: 锁定粘贴（可连续点击粘贴，按 Esc 退出；按 5 进入此模式）");
		}

		if clipboard.width == 0 || clipboard.height == 0 || clipboard.tiles.is_empty() {
			msg.push_str("\n剪贴板: 空（先用选择工具 Ctrl+C 复制一块区域）");
		} else {
			let (pw, ph) = if rot == 1 || rot == 3 {
				(clipboard.height, clipboard.width)
			} else {
				(clipboard.width, clipboard.height)
			};
			msg.push_str(&format!(
				"\n剪贴板: {}x{} | 粘贴尺寸: {}x{}",
				clipboard.width,
				clipboard.height,
				pw,
				ph
			));
		}
	}

	// 非粘贴模式下也展示剪贴板与变换状态（便于“先旋转/翻转再 Ctrl+V”）。
	if tools.tool != ToolKind::Paste {
		if clipboard.width > 0 && clipboard.height > 0 && !clipboard.tiles.is_empty() {
			let rot = paste.rot % 4;
			let deg = (rot as u32) * 90;
			let (pw, ph) = if rot == 1 || rot == 3 {
				(clipboard.height, clipboard.width)
			} else {
				(clipboard.width, clipboard.height)
			};
			msg.push_str(&format!(
				"\n\n剪贴板: {}x{} | 预设粘贴: {}x{} | 变换: {}° flipX:{} flipY:{}",
				clipboard.width,
				clipboard.height,
				pw,
				ph,
				deg,
				if paste.flip_x { "开" } else { "关" },
				if paste.flip_y { "开" } else { "关" },
			));
		}
	}

	commands.entity(hud_entity).insert(Text::new(msg));
}
