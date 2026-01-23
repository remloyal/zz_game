//! UI 根节点与静态 UI 树构建。

use bevy::prelude::*;

use crate::editor::{LEFT_PANEL_WIDTH_PX, RIGHT_TOPBAR_HEIGHT_PX, UI_BUTTON, UI_PANEL, UI_TOP_RESERVED_PX};

use crate::editor::types::{
	CanvasRoot, HudText,
	MenuButton, MenuId,
		LayerPrevButton, LayerNextButton, LayerActiveLabel, LayerActiveVisLabel, LayerActiveVisToggleButton,
		LayerActiveLockLabel, LayerActiveLockToggleButton,
		BrushSizeButton,
	PaletteRoot, PaletteScroll,
	PaletteSearchClearButton, PaletteSearchField, PaletteSearchText,
	PaletteZoomButton, PaletteZoomLevel,
	ShiftModeButton, ShiftModeLabel,
	TilesetBar, TilesetCategoryCycleButton, TilesetCategoryLabel, TilesetMenuRoot, TilesetToggleButton,
	ToolButton, ToolKind,
	UiRoot,
};
use crate::editor::MENUBAR_HEIGHT_PX;

/// UI 初始化：HUD + 左侧面板。
pub fn setup_ui(mut commands: Commands) {
	commands.spawn((
		Text::new("按 O 或点【打开】导入 tileset"),
		TextFont {
			font_size: 13.0,
			..default()
		},
		TextColor(Color::WHITE),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Px(UI_TOP_RESERVED_PX + 8.0),
			right: Val::Px(10.0),
			max_width: Val::Px(420.0),
			padding: UiRect::all(Val::Px(8.0)),
			..default()
		},
		BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
		ZIndex(800),
		bevy::ui::FocusPolicy::Pass,
		HudText,
	));

	spawn_ui_root(&mut commands);
	super::context_menu::spawn_context_menu(&mut commands);
}

fn spawn_ui_root(commands: &mut Commands) {
	let root = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Percent(100.0),
				flex_direction: FlexDirection::Column,
				..default()
			},
			// 重要：UI 画在世界之上。这里必须透明，否则会把世界渲染整块盖住。
			BackgroundColor(Color::NONE),
			UiRoot,
		))
		.id();

	let menubar = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Px(MENUBAR_HEIGHT_PX),
				flex_direction: FlexDirection::Row,
				align_items: AlignItems::Center,
				padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
				column_gap: Val::Px(6.0),
				..default()
			},
			BackgroundColor(UI_PANEL),
		))
		.id();

	commands.entity(menubar).with_children(|p| {
		for (label, id) in [
			("File", MenuId::File),
			("Edit", MenuId::Edit),
			("View", MenuId::View),
			("Map", MenuId::Map),
			("Layer", MenuId::Layer),
			("Help", MenuId::Help),
		] {
			p.spawn((
				Button,
				Node {
					height: Val::Px(24.0),
					padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
					align_items: AlignItems::Center,
					justify_content: JustifyContent::Center,
					..default()
				},
				BackgroundColor(UI_BUTTON),
				MenuButton(id),
			))
			.with_children(|p| {
				p.spawn((
					Text::new(label),
					TextFont { font_size: 13.0, ..default() },
					TextColor(Color::WHITE),
				));
			});
		}
	});

	let main_row = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				flex_grow: 1.0,
				flex_direction: FlexDirection::Row,
				..default()
			},
			BackgroundColor(Color::NONE),
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

		// 当前层锁定
		p.spawn((
			Button,
			Node {
				height: Val::Px(28.0),
				padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			LayerActiveLockToggleButton,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("解"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
				LayerActiveLockLabel,
			));
		});
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
				min_height: Val::Px(0.0),
				flex_direction: FlexDirection::Column,
				align_items: AlignItems::FlexStart,
				overflow: Overflow::clip_y(),
				..default()
			},
			ScrollPosition::default(),
			PaletteScroll,
		))
		.id();

	let palette_root = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Auto,
				position_type: PositionType::Absolute,
				top: Val::Px(0.0),
				left: Val::Px(0.0),
				flex_direction: FlexDirection::Row,
				flex_wrap: FlexWrap::Wrap,
				justify_content: JustifyContent::FlexStart,
				align_items: AlignItems::FlexStart,
				align_content: AlignContent::FlexStart,
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
		// 笔刷尺寸（P1）
		p.spawn((
			Text::new("笔刷:"),
			TextFont {
				font_size: 13.0,
				..default()
			},
			TextColor(Color::WHITE),
		));
		for size in [1u32, 2u32, 3u32] {
			p.spawn((
				Button,
				Node {
					height: Val::Px(28.0),
					padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
					align_items: AlignItems::Center,
					justify_content: JustifyContent::Center,
					..default()
				},
				BackgroundColor(UI_BUTTON),
				BrushSizeButton(size),
			))
			.with_children(|p| {
				p.spawn((
					Text::new(format!("{size}x{size}")),
					TextFont {
						font_size: 13.0,
						..default()
					},
					TextColor(Color::WHITE),
				));
			});
		}

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

	// 悬浮：地图右上角图层切换（不是顶栏）
	let layer_overlay = commands
		.spawn((
			Node {
				position_type: PositionType::Absolute,
				top: Val::Px(10.0),
				right: Val::Px(10.0),
				flex_direction: FlexDirection::Row,
				align_items: AlignItems::Center,
				padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
				column_gap: Val::Px(6.0),
				..default()
			},
			BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
		))
		.id();

	commands.entity(layer_overlay).with_children(|p| {
		p.spawn((
			Text::new("层"),
			TextFont {
				font_size: 13.0,
				..default()
			},
			TextColor(Color::WHITE),
		));
		p.spawn((
			Button,
			Node {
				height: Val::Px(28.0),
				padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			LayerPrevButton,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("←"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});

		p.spawn((
			Text::new("1/1"),
			TextFont {
				font_size: 13.0,
				..default()
			},
			TextColor(Color::WHITE),
			LayerActiveLabel,
		));

		// 当前层显隐
		p.spawn((
			Button,
			Node {
				height: Val::Px(28.0),
				padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			LayerActiveVisToggleButton,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("显"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
				LayerActiveVisLabel,
			));
		});

		p.spawn((
			Button,
			Node {
				height: Val::Px(28.0),
				padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(UI_BUTTON),
			LayerNextButton,
		))
		.with_children(|p| {
			p.spawn((
				Text::new("→"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
		});
	});

	commands.entity(right_content).add_child(layer_overlay);

	commands.entity(palette_scroll).add_child(palette_root);
	commands.entity(tileset_bar).add_child(tileset_menu);
	commands.entity(left_panel).add_child(toolbar);
	commands.entity(left_panel).add_child(tileset_bar);

	// palette 顶部条（分页 + 缩略图缩放 + 搜索）
	let palette_pager = commands
		.spawn((
			Node {
				width: Val::Percent(100.0),
				flex_direction: FlexDirection::Column,
				row_gap: Val::Px(6.0),
				..default()
			},
			BackgroundColor(Color::NONE),
		))
		.id();
	commands.entity(palette_pager).with_children(|p| {
		// Row 1: 缩略图缩放
		p.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Px(28.0),
				flex_direction: FlexDirection::Row,
				align_items: AlignItems::Center,
				column_gap: Val::Px(8.0),
				..default()
			},
			BackgroundColor(Color::NONE),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("Palette:"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));

			p.spawn((
				Text::new("缩略图:"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
			for (label, level) in [
				("小", PaletteZoomLevel::Small),
				("中", PaletteZoomLevel::Medium),
				("大", PaletteZoomLevel::Large),
			] {
				p.spawn((
					Button,
					Node {
						height: Val::Px(24.0),
						padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
						align_items: AlignItems::Center,
						justify_content: JustifyContent::Center,
						..default()
					},
					BackgroundColor(UI_BUTTON),
					PaletteZoomButton(level),
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

		// Row 2: 搜索
		p.spawn((
			Node {
				width: Val::Percent(100.0),
				height: Val::Px(28.0),
				flex_direction: FlexDirection::Row,
				align_items: AlignItems::Center,
				column_gap: Val::Px(8.0),
				..default()
			},
			BackgroundColor(Color::NONE),
		))
		.with_children(|p| {
			p.spawn((
				Text::new("筛选:"),
				TextFont {
					font_size: 13.0,
					..default()
				},
				TextColor(Color::WHITE),
			));
			p.spawn((
				Button,
				Node {
					width: Val::Px(160.0),
					height: Val::Px(24.0),
					padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
					align_items: AlignItems::Center,
					justify_content: JustifyContent::FlexStart,
					..default()
				},
				BackgroundColor(UI_BUTTON),
				PaletteSearchField,
			))
			.with_children(|p| {
				p.spawn((
					Text::new(" <全部>"),
					TextFont {
						font_size: 13.0,
						..default()
					},
					TextColor(Color::WHITE),
					PaletteSearchText,
				));
			});
			p.spawn((
				Button,
				Node {
					height: Val::Px(24.0),
					padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
					align_items: AlignItems::Center,
					justify_content: JustifyContent::Center,
					..default()
				},
				BackgroundColor(UI_BUTTON),
				PaletteSearchClearButton,
			))
			.with_children(|p| {
				p.spawn((
					Text::new("×"),
					TextFont {
						font_size: 13.0,
						..default()
					},
					TextColor(Color::WHITE),
				));
			});
		});
	});

	commands.entity(left_panel).add_child(palette_pager);
	commands.entity(left_panel).add_child(palette_scroll);
	commands.entity(main_row).add_child(left_panel);
	commands.entity(main_row).add_child(right_panel);
	commands.entity(root).add_child(menubar);
	commands.entity(root).add_child(main_row);
}
