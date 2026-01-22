# 代码树（实现概览）

目的：用“树 + 职责”快速定位功能实现位置，配合后续继续拆分 world/ui/persistence。

## Workspace

- zz_game（workspace root）
  - Cargo.toml
    - workspace members：crates/*
  - src/main.rs
    - 游戏本体入口（运行 zz_game）

## Crates

- crates/tilemap_core
  - src/lib.rs
    - Tilemap 的核心数据结构与纯逻辑（当前已迁入）：
      - TileMapData：多图层扁平存储（layers + tiles），提供 idx_layer / topmost_* / ensure_layers
      - TileRef：tileset_id + index + rot/flip
      - DEFAULT_LAYER_COUNT
    - feature：
      - serde：允许核心类型序列化
      - bevy：让核心类型可作为 Bevy Resource（当前 editor 使用）

- crates/tilemap_format
  - src/lib.rs
    - 存档格式与版本迁移（RON）：
      - decode_map_ron：兼容 V1/V2/V3，返回 (TileMapData, tilesets)
      - encode_map_ron_v3：写出最新 V3（包含 layers + tilesets + tiles）

- crates/tilemap_editor
  - src/main.rs
    - 编辑器入口（运行 tilemap_editor）
  - src/editor/mod.rs
    - Editor 插件/系统注册点（把系统挂到 Bevy Schedule）
  - src/editor/types.rs
    - 跨模块共享的 Resource/Component/数据定义（门面 re-export）
  - src/editor/types/
    - tilemap.rs
      - TileEntities（地图格子 sprite 实体索引）
      - LayerState（当前编辑层 active）

  - src/editor/persistence.rs
    - 负责“文件 IO + tileset 收集/回填”
    - Map 的 RON 编解码/迁移由 tilemap_format 提供

  - src/editor/world.rs
    - World 侧总入口（相机/鼠标输入/系统 glue），逐步把大块逻辑拆到子模块

  - src/editor/world/
    - layers.rs
      - 图层快捷键：PgUp/PgDn/L（更新 LayerState.active）
    - render_sync.rs
      - apply_map_to_entities：map → sprite 的全量同步（含多余层隐藏）
      - refresh_map_on_tileset_runtime_change：tileset runtime 变更触发全量刷新
    - selection_transform.rs
      - 选区旋转/翻转/重置（只作用于当前 active layer）
    - selection_move.rs
      - 选区拖拽移动/复制移动（含幽灵预览 + Undo 提交）

## 关键语义（实现约定）

- 多图层
  - 写入：默认写当前 active layer
  - 读取：吸管与单格变换读取 topmost non-empty layer
  - 存档：V3 显式 layers + 扁平 tiles；V1/V2 加载会迁移到 layer0，并 ensure 至至少 2 层

## 后续建议的拆分方向（下一刀）

- 把 world.rs 继续按“工具/输入/渲染/选择/粘贴”等维度拆文件：
  - paint/rect/fill/paste 这几段通常是最大的体积贡献
- 把 tileset 相关逻辑拆出更明确边界：
  - tileset 运行时加载（runtime/loading） vs UI 下拉菜单 vs palette 渲染
