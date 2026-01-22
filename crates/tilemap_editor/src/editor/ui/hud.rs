//! 右上角 HUD 文案。

use bevy::prelude::*;

use crate::editor::types::{
	Clipboard, EditorConfig, EditorState, HudText, PasteState, TileMapData, TilesetLibrary,
	TilesetRuntime, ToolKind, ToolState,
};

/// 更新右上角 HUD（选中 tile、地图路径、图层/工具/剪贴板等状态）。
pub fn update_hud_text(
    mut commands: Commands,
    config: Res<EditorConfig>,
    state: Res<EditorState>,
    lib: Res<TilesetLibrary>,
    runtime: Res<TilesetRuntime>,
    tools: Res<ToolState>,
    layer_state: Res<crate::editor::types::LayerState>,
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
				clipboard.width, clipboard.height, pw, ph
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
