# Tilemap Editor 设计（参考 Tiled）

更新时间：2026-01-26

相关文档：
- 操作手册：[docs/tilemap_editor_controls.md](docs/tilemap_editor_controls.md)
- 现有设计（参考 RPG Maker）：[docs/tilemap_editor_design.md](docs/tilemap_editor_design.md)
- 代码定位： [docs/code_tree.md](docs/code_tree.md)

本文目标：
- 以 Tiled 为参考，定义“纯 Tilemap Editor”的功能边界、术语与交互约定。
- 明确一条可执行的迭代路线：先把 **Tile Layer + Tileset** 做到 Tiled 级别可用，再逐步扩展 Object/Image/Group Layer。
- 输出可落地的“数据模型与导入/导出策略”（优先 Tiled JSON 子集）。

---

## 1. 设计取向（与 Tiled 对齐）

- **图层类型**：Tile Layer / Object Layer / Image Layer / Group Layer
- **属性系统**：地图、图层、对象、图块均可配置自定义属性（键值对）
- **Tileset**：外部 tileset 引用（TSX 思路），支持多 tileset
- **数据格式**：面向导出/导入（Tiled JSON/TMX 的子集优先）
- **编辑体验**：工具优先、无“数据库规则”强绑定

补充约束（避免范围膨胀）：
- 本编辑器优先覆盖 **Tiled 的 tilemap 工作流**：Pencil/Rect/Fill/Select/Stamp、图层、tileset、多 tileset、属性与导出。
- 不强绑 RPG Maker 的自动图块/通行数据库；如要做，作为可选的 overlay/元数据层（不阻塞本路线）。

非目标（短期不做 / 明确延后）：
- Script/插件系统（Tiled Extensions）
- 地图对象的复杂编辑器（路径编辑、样条、复杂形状操作）
- 完整 TMX 支持（优先 JSON 子集；TMX 可作为后续互操作）

---

## 1.1 术语与约定（对齐 Tiled）

- **Tile**：一个 tileset 图块（按 `tile_id`/index 标识）
- **Cell**：地图网格的一个格子；在某个图层上可为 None 或一个 TileRef
- **Tileset**：一张 tileset 图片 + 切分参数 + tile 属性表
- **GID**：Tiled 的 Global Tile ID（跨 tileset 的全局编号），高位包含 flip 标记

本项目约定：
- 运行时/编辑器内部使用 `TileRef { tileset_id, index, rot/flip }`，对外导出/导入时再映射为 Tiled 的 GID。
- 编辑器默认以网格为中心：所有工具最终都产出“对 Cell 的批量变更”，统一走 Undo/Redo。

---

## 2. 核心能力清单（Tiled 视角）

### 2.1 地图与图层
- 多图层（Tile Layer / Object Layer / Image Layer / Group Layer）
- 图层可见/锁定/透明度/偏移
- 图层排序与分组

### 2.2 Tileset 与资源
- 多 tileset 引用（本地/内嵌）
- tileset 资源分类/搜索/缩略图缩放
- tile 自定义属性
- tile 动画（帧序列/时长）
- tile 碰撞形状/对象模板（可选）

### 2.3 工具与编辑
- Pencil / Eraser / Rect / Fill / Select / Eyedropper
- Stamp/Brush（图块笔刷）
- 选择复制/粘贴、撤销/重做

### 2.4 视图
- 缩放/平移
- 网格/坐标/HUD 开关
- 视口内渲染优化

### 2.5 导入/导出
- Tiled JSON 子集（优先）
- 自定义 RON（用于运行时）

---

## 2.1 交互对齐清单（Tiled 风格验收项）

右侧画布（地图）：
- 鼠标滚轮缩放；`Space`+左键或中键拖拽平移
- hover 高亮格子与点击/绘制命中 **严格一致**（缩放/平移/旋转后也一致）
- 网格线显示开关（不影响命中）

左侧 tileset/palette：
- 鼠标滚轮在 palette 上时：缩放（优先）或滚动（空白区域可滚动）
- `Space`/中键拖拽：平移/滚动内容，且不触发选中
- hover 高亮与点击选中一致（和右侧命中规则同源）
- palette 与地图交互完全隔离：鼠标在右侧时不更新左侧 hover，反之亦然

工具：
- Pencil/Rect/Fill/Select/Eyedropper 与 Undo/Redo 行为稳定可预期
- “临时工具”（如按住 I 临时吸管）可选，但要明确优先级与冲突规则

---

## 3. 数据模型建议（对齐 Tiled 语义）

### 3.1 地图
- `Map { width, height, tile_width, tile_height, layers, tilesets, properties }`

### 3.2 图层
- `TileLayer { tiles: Vec<Option<TileRef>>, opacity, visible, offset, properties }`
- `ObjectLayer { objects: Vec<MapObject>, properties }`
- `ImageLayer { image, offset, opacity, properties }`
- `GroupLayer { layers: Vec<Layer>, properties }`

### 3.3 Tileset
- `Tileset { id, name, image, tile_width, tile_height, columns, spacing, margin, properties, tile_properties, tile_animations }`

### 3.4 Tile 属性与动画
- `TileProperties { id, properties }`
- `TileAnimation { id, frames: Vec<(tile_id, duration_ms)> }`
- `TileCollision { id, shapes: Vec<Shape> }`（可选）

---

## 3.5 Tiled JSON 子集：建议支持范围（最小可用）

目标：先支持 **导出**，再支持 **导入**（导入比导出更容易踩坑）。

建议支持字段（导出）：
- `Map`：`width/height/tilewidth/tileheight/orientation/renderorder/infinite`（先固定 orthogonal + finite）、`layers`、`tilesets`、`properties`
- `TileLayer`：`type="tilelayer"`、`width/height`、`data`（CSV 或 array；建议 array of u32）
- `Tileset`：建议优先导出为 `tilesets: [{ firstgid, source }]`（TSX 思路），并同时支持内联（`image`/`tilewidth`...）作为 fallback
- `properties`：支持 string/int/float/bool（Tiled 还有 color/file/object 等，后续补）

GID 约定（对齐 Tiled）：
- 用 `firstgid + tile_index` 生成 gid
- flip 标记：
	- 水平翻转：`0x80000000`
	- 垂直翻转：`0x40000000`
	- 对角翻转：`0x20000000`
- 本项目如果内部用 `rot/flip`，导出时映射到上述 bit；导入时再还原

多 tileset：
- 导出时对 tileset 按加载顺序分配 `firstgid`，并保证 gid 空间不冲突
- tile 属性表：导出到 tileset 的 `tiles: [{ id, properties }]`

---

## 4. 与现有实现的差异点

- 现有以 **TileLayer 为主**，尚未引入 Object/Image/Group Layer。
- tileset 仅保存图像与索引，尚未支持 tile 属性与 tileset 属性。
- map.ron 为运行时友好格式，未对齐 Tiled JSON/TMX。

与现有代码的对照（便于落地）：
- 地图与图层：`tilemap_core::TileMapData`（扁平 tiles + layers 计数）
- 编辑器入口：`crates/tilemap_editor/src/main.rs`
- 系统注册点：`crates/tilemap_editor/src/editor/mod.rs`
- 存档与迁移：`tilemap_format` + `tilemap_editor::editor::persistence`

---

## 5. 迭代计划（按 Tiled 路线）

### P0（当前已具备）
- Tile Layer 基本绘制
- Undo/Redo、选择、复制粘贴
- Tileset 管理与缩略图

### P1（Tiled 基础能力补齐）
1) Tile Layer 属性（透明度、偏移）
2) Map/Layer/Tile 属性（键值对）
3) tileset 属性与 tile 属性编辑入口（只做 UI 与存取，不强绑具体用途）
4) 导出 Tiled JSON 子集（TileLayer + Tileset + properties + 多 tileset）
5) （可选）tile 动画数据结构 + 简单编辑入口
6) （可选）tile 碰撞形状/对象模板：先只做数据结构与可视化

### P2（图层类型扩展）
1) Object Layer（点/矩形/多边形）
2) Image Layer（背景图）
3) Group Layer（层级结构）
4) 图层排序与分组 UI

### P3（高级与互操作）
1) 导入 Tiled JSON 子集
2) TSX 外部 tileset 引用
3) 批量属性编辑与模板

---

## 5.1 里程碑验收（建议按 PR/版本切片）

M1：属性系统（无 UI 或最小 UI）
- `tilemap_core` 增加 `Properties`（map/layer/tile）并能序列化
- `tilemap_format` 能保存/读取这些属性（RON 先行）

M2：属性面板 UI（编辑体验闭环）
- 右侧属性面板：可编辑当前 Map/Layer/选中 Tile 的 properties
- Undo/Redo 对属性修改也生效（至少对 map/layer 属性）

M3：导出 Tiled JSON 子集
- 能导出 orthogonal finite map（包含 layers、tilesets、properties）
- 导出结果可被 Tiled 打开并显示出正确 tiles

M4：导入（可选，放到 P3 但可以先做一部分）
- 能导入自己导出的 JSON（round-trip）
- 能导入“常见 Tiled map”（限制条件写清楚，例如不支持 infinite/chunked）

---

## 6. 建议落地顺序

- 先补 **Tileset/Tile 属性** 与 **Tiled JSON 导出**（最小改动且价值高）
- 再扩展 **Object/Image/Group Layer**（新增 UI 与数据结构）
- 最后做 **导入** 与更复杂互操作

---

## 7. 对当前项目的具体建议（可直接动手）

1) 扩展 `tilemap_core`：添加 `MapProperties/LayerProperties/TileProperties`
2) `tilemap_format`：新增 `tiled_json.rs`（导出子集 + gid/tileset 映射）
3) `tilemap_editor`：新增属性面板（右侧 dock/浮层都行，但要可持续扩展）
4) （可选）tileset 与 tile 属性的编辑入口：从 palette 选中 tile 后在属性面板显示

---

如需，我可以按此文档直接开始实现 P1（属性系统 + Tiled JSON 导出），并按上面的 M1~M3 里程碑拆成可合并的 PR。
