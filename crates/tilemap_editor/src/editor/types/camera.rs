use bevy::prelude::*;

/// 标记“世界相机”（用于世界坐标拾取/绘制）。
///
/// 注意：UI 可能会创建/使用自己的相机。若鼠标拾取系统用 `Query<(&Camera, &GlobalTransform)>`
/// 并 `single()`，当场景存在多相机时会直接失败，从而导致“右侧无法绘制”。
#[derive(Component)]
pub struct WorldCamera;
