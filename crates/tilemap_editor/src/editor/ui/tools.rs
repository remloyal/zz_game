//! 工具栏交互：工具选择、ShiftMap 模式切换等。

use bevy::prelude::*;

use crate::editor::{UI_BUTTON, UI_BUTTON_HOVER, UI_BUTTON_PRESS, UI_HIGHLIGHT};
use crate::editor::types::{
    PasteState, ShiftMapMode, ShiftMapSettings, ShiftModeButton, ShiftModeLabel, ToolButton,
    ToolKind, ToolState,
};

/// 工具按钮点击：切换当前工具；进入粘贴工具时重置粘贴变换。
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

/// 工具按钮样式同步：高亮当前工具。
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

/// Shift Map 模式按钮：Blank <-> Wrap 切换。
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

/// Shift Map 模式文本更新。
pub fn update_shift_mode_label(settings: Res<ShiftMapSettings>, mut q: Query<&mut Text, With<ShiftModeLabel>>) {
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
