# Tilemap Editor 设计（参考 RPG Maker MV/MZ）

相关文档：
- 操作手册：[docs/tilemap_editor_controls.md](docs/tilemap_editor_controls.md)

本文目标：
- 对照 RPG Maker MV/MZ 的地图编辑器，梳理当前编辑器缺失的“基础能力”。
- 给出分阶段（P0/P1/P2）的功能路线图。
- 设计分库（crate）与分模块边界，便于后续持续迭代。

> 注：RPG Maker 的完整编辑器包含“图块模式 + 事件模式 + 数据库配置”等大量能力；本项目先聚焦 tilemap 绘制部分，把事件/通行等作为可选扩展。

---

## 1. 现状盘点（当前 tilemap_editor 已有）

**地图**
- 多图层地图数据：`TileMapData { width, height, layers, tiles: Vec<Option<TileRef>> }`（tiles 按 layer 扁平存储）
- 图层语义：写入默认 current active layer；读取（吸管/单格变换）默认 topmost non-empty layer
- 图层切换快捷键（PgUp/PgDn/L）+ HUD 显示当前层
- 预设尺寸 + 自定义宽高输入 + 应用
- 新建（清空）
- 保存/读取（MapFileV3，兼容旧版本迁移）
- 导入/导出（文件选择器）

**绘制/视图**
- 右侧画布：鼠标左键绘制、右键擦除
- 网格与 hover 高亮（Gizmos）
- 缩放（滚轮）、平移（中键或 Space+左键）
- 视野内分块加载图块实体（chunk），大地图仅加载视野内区域

**Tileset**
- 导入图片为 tileset（复制到 assets）
- tileset 库持久化（library.ron），支持分类过滤
- 下拉菜单选择 tileset（悬浮菜单）
- palette 缩略图按钮网格（按 tile index 选择）

---

## 2. 对照 RPG Maker MV/MZ：缺失的“基础能力”

下面按“对编辑器可用性影响”排序。

### P0（强烈建议补齐，否则很难像编辑器）

P0 当前已基本补齐：多图层（至少 2 层）、工具（Rect/Fill/Select/Eyedropper 等）、Undo/Redo、选择复制粘贴、Shift Map。

### P1（做完 P0 后，体验会明显接近 RM）

6) **笔刷尺寸/形状**
- 1x1、2x2、3x3；或“按 stamp 大小绘制”。

7) **图层可见性/锁定**
- 当前层可编辑，其它层只显示或隐藏。
- 建议补齐“图层面板 UI”（当前层、可见性、锁定、重命名）。

8) **更完善的 tileset 管理**
- RM 有 A1~A5、B~E 标签页概念（含自动图块）。
- 本项目可替代为：
	- tileset 资源分组（tabs / categories）
	- palette 搜索/缩略图缩放

9) **网格/辅助显示开关**
- 网格显示开关、坐标显示、当前鼠标所在格坐标。

### P2（高级/可选，是否做取决于游戏需求）

10) **自动图块（Autotile）**
- RM 的核心特色之一：边缘自动拼接。
- 需要 tileset 元数据（autotile 定义）+ 运行时规则。

11) **碰撞/通行/区域（RegionId / Passage / TerrainTag 等）**
- RM 的“地图属性绘制模式”（不是图块本身）。
- 可作为独立的 overlay 层/元数据层。

12) **事件编辑模式**
- RM 的事件系统复杂；建议后置，不要阻塞 tilemap 编辑器本体。

---

## 3. 推荐的目标数据模型（面向多层+元数据）

### 3.1 Tile 与图层
当前实现采用“最小侵入”的扁平方案：

- `TileMapData { width, height, layers, tiles: Vec<Option<TileRef>> }`
	- `tiles` 按 `layer0..layerN` 顺序扁平存储
	- `idx_layer(layer,x,y)` 寻址

优点：迁移成本低，工具/渲染同步更直接；后续如需更强语义（层名/锁定/可见性等），可再在上层引入 Layer 元数据表。

### 3.2 元数据层（可选）
把“通行/区域/地形标记”等当作独立网格层：
- `RegionLayer: Vec<u8>`
- `CollisionLayer: Vec<bool>`

优点：工具/撤销/保存/渲染叠加都更清晰。

---

## 4. 工具系统设计（对齐 RM 的工作流）

建议把当前的“鼠标左键写 tile / 右键擦除”升级为工具状态机：

- `ToolKind`：`Pencil`/`Eraser`/`Rect`/`Fill`/`Select`/`Eyedropper`/`Stamp`
- `ToolState`：正在拖拽的起点、当前预览矩形、选择框、stamp 数据等

关键点：
- 工具只负责“生成变更（TileDiff 或 Command）”，真正写入 map 由统一的 apply 入口完成。
- 右侧画布需要“预览层”（比如拖拽矩形时显示轮廓/半透明预览）。

---

## 5. Undo/Redo（建议用命令栈）

最稳妥的方案：
- `Command` 记录一批格子的 before/after：`Vec<CellChange { idx, before, after }>`
- `UndoStack`：`undo: Vec<Command>`, `redo: Vec<Command>`

把一次连续拖拽的绘制合并成一个 command（鼠标按下开始、松开提交）。

---

## 6. UI 结构建议（借鉴 RM，但适配当前布局）

保持你现在的“左侧面板 + 右侧画布”挺合理，建议补齐：

- 左侧顶部：
	- 工具栏（铅笔/橡皮/矩形/填充/选择/吸管/盖章）
	- 图层面板（当前层、可见性、锁定）
- 左侧中部：tileset 选择 + 分类/过滤
- 左侧下部：palette（搜索、缩放缩略图）
- 右侧顶部：地图属性（尺寸、当前层、网格开关、撤销/重做）
- 右侧画布：预览覆盖层（矩形/选择框/stamp preview）

---

## 7. 分库（crate）与分模块建议

### 7.1 推荐的 crate 划分（逐步迁移）

1) `tilemap_core`（纯逻辑，无 Bevy 依赖）

现状：已创建并开始承载核心类型（`TileMapData`/`TileRef`/索引方法）。

说明：为了让编辑器以最小改动继续把 `TileMapData` 当作 Bevy `Resource` 使用，当前 `tilemap_core` 提供可选 `bevy` feature；后续如果希望“严格纯逻辑”，可以再做一层 wrapper/newtype 逐步去 Bevy 依赖。

2) `tilemap_format`（序列化/版本迁移）
- RON/JSON 存取
- 版本迁移（V1→V2→V3…）

3) `tilemap_bevy`（渲染与 Bevy 集成）
- 把 `TileMap` 渲染成 Sprite/Atlas
- 摄像机/坐标拾取辅助

4) `tilemap_editor`（应用层）
- UI（Bevy UI）
- 工具系统（输入→command→apply）
- 文件对话框（rfd）、tileset 导入

> 现在的 `tilemap_editor` 里已经混合了 core+io+bevy+app；建议按“先抽纯逻辑、再抽 IO”顺序迁移，风险最低。

### 7.2 如果暂时不拆 crate：至少拆 module
- `editor/core/*`：map/layer/commands
- `editor/io/*`：persistence
- `editor/tools/*`：tool state machine
- `editor/ui/*`：UI nodes & interactions
- `editor/render/*`：world sync（entities）

---

## 8. 推荐实施顺序（最小可行重构）

**第 1 阶段（P0 体验闭环）**
- 引入 `LayerId + layers`，先做 2 层
- 增加 Rect/Fill/Select/Eyedropper
- 引入 Undo/Redo
- 增加 Shift Map

**第 2 阶段（P1 体验增强）**
- 图层可见/锁定
- brush size、palette 分页
- 网格/坐标显示开关

**第 3 阶段（P2 扩展）**
- autotile
- region/collision overlay

---

## 9. 本仓库下一步建议（我可以直接动手）

已启动的“风险最小”重构：
- 已创建 `crates/tilemap_core` 并迁移 `TileMapData/TileRef`（编辑器通过依赖 + re-export 平滑接入）。
- 已开始把 `types.rs` 做子模块化（例如 tilemap 相关 Resource 先拆出）。

下一步可以继续：
- 把 persistence（MapFileV1/V2/V3 与升级逻辑）抽到 `tilemap_format`。
- 把 world.rs 按功能拆成 `editor/tools/*`、`editor/render/*`、`editor/input/*`，并保持 system 注册点不变。