# teach-free 文档

> 契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M2 P1 已实现）

自由对话轻量教学模块（**T4 决议** history 1614：非保留无模块态、非退役）。只依赖 kernel-api。

- `TEACH_FREE_ID` = "teach-free"；`PERSONA` = 日语教师 persona（面向中文母语学习者，
  原 kernel `DEFAULT_TEACHER_PERSONA` **移入**，本模块为权威；kernel 常量降为无注册表测试路径 legacy 兜底）。
- `module_yaml()`：module.yaml `include_str!` 单一事实源（`TeachFree::new()` 与组合根注册表共用，noyalib 反序列化）。
- `TeachingModule` 实现：persona 返回 PERSONA；**无专属工具**（speak kernel 统一提供，未知工具报错回喂 LLM）；
  无模块路由（P1；前端包装归 P5）；handle_event 空（转录经 assess 提取，P3）；on_session_start/end 默认空。
- **plan 可选**：未 placement 也能自由对话（persona-only）；有方案时 kernel 回合循环钉住 teacher_prompt（P3 assess 落地后）。
- **既有数据迁移**：kernel 0006 将存量 NULL 模块会话与 default 文件夹迁入 teach-free（B8 哨兵涟漪）；
  组合根新会话缺省 module=teach-free、文件夹查询默认 teach-free。
- 注册：godoword cargo feature `teach-free`（default-on）→ `ModuleRegistry.register_yaml` + `KernelCtxFactory` 注入。

## 测试

注册表测试（kernel/tests/registry.rs）锚定 module.yaml 反序列化；回合接线/ctx 集成测试
（kernel/tests/module_slice.rs，mock 模块同构）；组合根 handler 级测试（godoword rest.rs：缺省模块/完成语义/清单）。

## roadmap 差异

- 前端包装（自由对话入口卡片/模块路由）归 P5；assess 提取消费转录归 P3。
- 角色音色档案实体 M2+ 推迟（自由对话多角色声音为远期方向）。