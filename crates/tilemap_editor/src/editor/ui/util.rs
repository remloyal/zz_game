//! UI 内部复用的 helper。

use crate::editor::types::TileMapData;

/// 调整地图尺寸时，把旧地图内容尽量拷贝到新地图。
///
/// 规则：
/// - 拷贝范围是新旧宽高的交集。
/// - 拷贝 layer 数是 `min(old.layers, new.layers)`（至少 1）。
pub(super) fn resized_map_copy(old: Option<&TileMapData>, width: u32, height: u32) -> TileMapData {
    let mut new_map = TileMapData::new(width, height);
    let Some(old) = old else {
        return new_map;
    };

    let copy_w = old.width.min(width) as usize;
    let copy_h = old.height.min(height);
    if copy_w == 0 || copy_h == 0 {
        return new_map;
    }

    let old_layer_len = old.layer_len();
    let new_layer_len = new_map.layer_len();
    if old_layer_len == 0 || new_layer_len == 0 {
        return new_map;
    }

    // 旧数据可能来自老版本/外部编辑器，tiles 长度不一定严格匹配 width*height*layers。
    // 这里做一次“可安全拷贝的层数”裁剪，避免越界 panic。
    let max_old_layers_by_len = (old.tiles.len() / old_layer_len) as u32;
    let max_new_layers_by_len = (new_map.tiles.len() / new_layer_len) as u32;
    let layers_to_copy = old
        .layers
        .min(new_map.layers)
        .min(max_old_layers_by_len)
        .min(max_new_layers_by_len)
        .max(1);

    for layer in 0..layers_to_copy {
        let old_layer_base = (layer as usize) * old_layer_len;
        let new_layer_base = (layer as usize) * new_layer_len;

        for y in 0..copy_h {
            let old_row = old_layer_base + (y as usize) * (old.width as usize);
            let new_row = new_layer_base + (y as usize) * (new_map.width as usize);
            let src = &old.tiles[old_row..old_row + copy_w];
            let dst = &mut new_map.tiles[new_row..new_row + copy_w];
            dst.clone_from_slice(src);
        }
    }

    new_map
}
