use crate::editor::types::{Clipboard, PasteState};

/// 计算粘贴预览/落地后的尺寸（考虑旋转）。
///
/// `PasteState.rot`: 0/1/2/3 => 0/90/180/270 度顺时针。
pub(super) fn paste_dims(clipboard: &Clipboard, paste: &PasteState) -> (u32, u32) {
    let w = clipboard.width;
    let h = clipboard.height;
    if w == 0 || h == 0 {
        return (0, 0);
    }

    match paste.rot % 4 {
        1 | 3 => (h, w),
        _ => (w, h),
    }
}

/// 将剪贴板的源坐标 `(sx, sy)` 映射到“变换后的局部坐标系” `(cx, cy)`。
///
/// 约定：先旋转（顺时针），再对旋转后的结果做 flip。
pub(super) fn paste_dst_xy(
    sx: u32,
    sy: u32,
    clipboard: &Clipboard,
    paste: &PasteState,
) -> Option<(u32, u32)> {
    let w = clipboard.width;
    let h = clipboard.height;
    if w == 0 || h == 0 {
        return None;
    }
    if sx >= w || sy >= h {
        return None;
    }

    let rot = paste.rot % 4;
    let (rw, rh) = paste_dims(clipboard, paste);
    if rw == 0 || rh == 0 {
        return None;
    }

    let (mut x, mut y) = match rot {
        0 => (sx, sy),
        // 90° CW: (x, y) -> (h-1-y, x)
        1 => (h.checked_sub(1 + sy)?, sx),
        // 180°: (x, y) -> (w-1-x, h-1-y)
        2 => (w.checked_sub(1 + sx)?, h.checked_sub(1 + sy)?),
        // 270° CW: (x, y) -> (y, w-1-x)
        3 => (sy, w.checked_sub(1 + sx)?),
        _ => return None,
    };

    if paste.flip_x {
        x = rw.checked_sub(1 + x)?;
    }
    if paste.flip_y {
        y = rh.checked_sub(1 + y)?;
    }

    if x >= rw || y >= rh {
        return None;
    }
    Some((x, y))
}
