//! 顶部菜单栏（分类 + 悬浮下拉）。

use bevy::prelude::*;

use crate::editor::types::{
	ActionButton, ActionKind, MenuBackdrop, MenuButton, MenuDropdown, MenuId, MenuItem, MenuState,
	UiRoot,
};
use crate::editor::util::despawn_silently;
use crate::editor::{MENUBAR_HEIGHT_PX, UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS, UI_HIGHLIGHT, UI_PANEL};

const MENU_WIDTH_PX: f32 = 180.0;
const MENU_ITEM_HEIGHT_PX: f32 = 28.0;

fn menu_x(id: MenuId) -> f32 {
	match id {
		MenuId::File => 8.0,
		MenuId::Edit => 68.0,
		MenuId::View => 128.0,
		MenuId::Map => 188.0,
		MenuId::Layer => 248.0,
		MenuId::Help => 318.0,
	}
}

pub fn menubar_button_interactions(
	mut state: ResMut<MenuState>,
	mut q: Query<(&Interaction, &MenuButton, &mut BackgroundColor), Changed<Interaction>>,
) {
	for (interaction, btn, mut bg) in q.iter_mut() {
		match *interaction {
			Interaction::Pressed => {
				state.open = if state.open == Some(btn.0) { None } else { Some(btn.0) };
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

pub fn menubar_sync_button_styles(state: Res<MenuState>, mut q: Query<(&MenuButton, &Interaction, &mut BackgroundColor)>) {
	for (btn, interaction, mut bg) in q.iter_mut() {
		if state.open == Some(btn.0) {
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

pub fn menubar_rebuild_dropdown_when_needed(
	mut commands: Commands,
	state: Res<MenuState>,
	root_q: Query<Entity, With<UiRoot>>,
	dropdown_q: Query<Entity, Or<(With<MenuDropdown>, With<MenuBackdrop>)>>,
	children_q: Query<&Children>,
) {
	if !state.is_changed() {
		return;
	}

	fn collect_descendants(root: Entity, children_q: &Query<&Children>, out: &mut Vec<Entity>) {
		let Ok(children) = children_q.get(root) else {
			return;
		};
		for c in children.iter() {
			out.push(c);
			collect_descendants(c, children_q, out);
		}
	}

	for e in dropdown_q.iter() {
		let mut all = vec![e];
		collect_descendants(e, &children_q, &mut all);
		for d in all {
			despawn_silently(&mut commands, d);
		}
	}

	let Some(open) = state.open else {
		return;
	};

	let Ok(root) = root_q.single() else {
		return;
	};

	let x = menu_x(open);

	commands.entity(root).with_children(|p| {
		// 全屏 backdrop：点击关闭。
		p.spawn((
			Button,
			Node {
				position_type: PositionType::Absolute,
				top: Val::Px(0.0),
				left: Val::Px(0.0),
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				..default()
			},
			BackgroundColor(Color::NONE),
			ZIndex(900),
			MenuBackdrop,
		));

		// 下拉菜单
		p.spawn((
			Node {
				position_type: PositionType::Absolute,
				top: Val::Px(MENUBAR_HEIGHT_PX),
				left: Val::Px(x),
				width: Val::Px(MENU_WIDTH_PX),
				height: Val::Auto,
				flex_direction: FlexDirection::Column,
				padding: UiRect::all(Val::Px(6.0)),
				row_gap: Val::Px(4.0),
				..default()
			},
			BackgroundColor(UI_PANEL),
			ZIndex(1000),
			MenuDropdown,
		))
		.with_children(|m| {
			macro_rules! item {
				($label:expr, $action:expr) => {
					m.spawn((
						Button,
						Node {
							width: Val::Percent(100.0),
							height: Val::Px(MENU_ITEM_HEIGHT_PX),
							padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
							align_items: AlignItems::Center,
							..default()
						},
						BackgroundColor(UI_BUTTON),
						ActionButton($action),
						MenuItem,
					))
					.with_children(|p| {
						p.spawn((
							Text::new($label),
							TextFont { font_size: 13.0, ..default() },
							TextColor(Color::WHITE),
						));
					});
				};
			}
			macro_rules! label {
				($text:expr) => {
					m.spawn((
						Node {
							width: Val::Percent(100.0),
							height: Val::Auto,
							padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
							..default()
						},
					))
					.with_children(|p| {
						p.spawn((
							Text::new($text),
							TextFont { font_size: 12.0, ..default() },
							TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
						));
					});
				};
			}

			match open {
				MenuId::File => {
					item!("打开 Tileset…", ActionKind::OpenTileset);
					item!("新建地图", ActionKind::NewMap);
					item!("保存地图", ActionKind::SaveMap);
					item!("读取地图", ActionKind::LoadMap);
					item!("导入地图…", ActionKind::ImportMap);
					item!("导出地图…", ActionKind::ExportMap);
				}
				MenuId::Edit => {
					item!("撤销 (Ctrl+Z)", ActionKind::Undo);
					item!("重做 (Ctrl+Y)", ActionKind::Redo);
				}
				MenuId::View => {
					item!("网格开关", ActionKind::ToggleGrid);
				}
				MenuId::Map => {
					item!("地图尺寸: 40x25", ActionKind::SetMapSize { width: 40, height: 25 });
					item!("地图尺寸: 64x36", ActionKind::SetMapSize { width: 64, height: 36 });
					item!("地图尺寸: 100x60", ActionKind::SetMapSize { width: 100, height: 60 });
					item!("Shift 模式切换", ActionKind::ToggleShiftMode);
				}
				MenuId::Layer => {
					label!("图层相关先用右上角悬浮控件");
				}
				MenuId::Help => {
					label!("见 docs/tilemap_editor_controls.md");
				}
			}
		});
	});
}

pub fn menubar_backdrop_click_to_close(
	mut state: ResMut<MenuState>,
	mut q: Query<&Interaction, (Changed<Interaction>, With<MenuBackdrop>)>,
) {
	for interaction in q.iter_mut() {
		if *interaction == Interaction::Pressed {
			state.open = None;
		}
	}
}

pub fn menubar_close_when_menu_item_pressed(
	mut state: ResMut<MenuState>,
	q: Query<&Interaction, (Changed<Interaction>, With<MenuItem>)>,
) {
	for interaction in q.iter() {
		if *interaction == Interaction::Pressed {
			state.open = None;
			break;
		}
	}
}
