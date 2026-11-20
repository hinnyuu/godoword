# kernel-api 契约

> 本 crate 只含 trait 与共享类型，无实现。适配器与教学模块只依赖本 crate，互不 import。
> 设计决策见 [history](../../../history/2026-11-16-0930-kernel-api-traits.md)，
> 架构上下文见 [architecture](../../../docs/project/architecture.md)。

## trait 一览

| trait | 实现者 | 职责 |
|---|---|---|
| `LlmProvider` | llm-openai / 未来 llm-anthropic | list_models / chat_stream → ChatChunk 流 |
| `TtsProvider` | tts-qwen-audio / 每款 TTS | models / list_voices / synthesize_stream → AudioEvent 流 |
| `AuthProvider` | auth-local / auth-oidc | authenticate（统一 enum）/ extra_routes |
| `TeachingModule` | teach-gojuon / teach-dialogue / teach-assess | manifest / persona / tools / routes / execute_tool / handle_event |
| `ModuleContext` | kernel | 模块受控服务：store / emit_progress / current_plan |
| `PlanProvider` | teach-assess | current_plan（薄接口） |

## 关键约定

- `ProviderConn` 由 kernel 解析绑定并解密后传入；`api_key` 为 SecretString，Debug 自动脱敏，
  适配器绝不持久化 key。
- `ChatChunk` 归一化 qwen 的 reasoning_content（ReasoningDelta）与 tool_calls（组装完整后发出）。
- `AudioEvent` 归一化流式 PCM 与词级时间戳（增量追加）；URL 返回模式由适配器内部下载归一化为流。
- `ModuleStore` = `Store` enum `{ Sqlite(SqlitePool), Pg(PgPool) }`（双具体驱动，sqlx Any 已证伪），
  表前缀约定 `mod_<id>_*`。薄分发宏 `execute_both!` / `fetch_one_both!` / `fetch_optional_both!` /
  `fetch_all_both!` + 运行时 `query_as` 跑双兼容 SQL 子集；`query!` 仅驱动专属代码。
- **双兼容 SQL 子集**：uuid 主键、应用侧时间戳、JSON 整取回 Rust（不在 SQL 过滤）；
  单一 PG 风格迁移集靠 SQLite 亲和性双后端通用。CI `storage` job 跑 **workspace 级**
  `cargo test --workspace`（设 `TEST_PG_URL`）真测双后端（required check）；含共享 SQL 的 crate
  走 `backends()` 夹具（SQLite 恒跑 + PG 由 `TEST_PG_URL` 驱动，未设时显式 skip）。
- `SessionEvent` 为后端→前端 SSE 协议（tag=event, content=data），回合内全序。
- `ProgressEvent` 为持久 outbox 信封；SessionEvicted 的 span 存于 detail。
- `speak` 工具规格由 **kernel-api 定义**（`SPEAK_TOOL_NAME` / `SpeakArgs` / `speak_tool_spec()`，
  黄金测试锚定 spec↔SpeakArgs），kernel 运行时反序列化 arguments→SpeakArgs→合并绑定→TtsRequest
  经 TtsProvider 执行，教学模块不实现 speak。规格落契约层使演进触发全量审查防漂移（决策 A）。
  LLM 可控子集为 text/voice/rate/pitch/instruction/language_hints；model/format/sample_rate/word_timestamps
  由 kernel 从绑定/配置填，不暴露给 LLM。

## 有意耦合声明（R3 + M1d）

kernel-api **有意耦合 axum**（`TeachingModule::routes() -> Router`）、**sqlx**（`Store` 双驱动）
与 **utoipa**（M1d：REST-facing 类型 `ModelInfo`/`VoiceInfo`/`ChatMessage`/`WordTimestamp`/`AudioFormat`
派生 `ToSchema`，供 OpenAPI schema 补全→前端类型生成单一事实源，history 2049 决策 B）。
全应用即 axum，模块可重写不要求框架中立。**升级 axum/sqlx/utoipa 视为契约变更**，须走契约级全量审查；
未来 agent 不得自作主张"解耦"。此耦合是务实决策，非疏忽。ToSchema 仅 derive，不改 serde 线缆形状/trait 签名。

## 当前状态与 roadmap 差异

契约 M0 定义；M1a 新增 speak 工具契约（SPEAK_TOOL_NAME/SpeakArgs/speak_tool_spec）。
kernel（回合循环）+ llm-openai + tts-qwen-audio 适配器 M1a 已实现；auth-local M1c、教学模块 M2+。
token 估算当前为字符启发式（roadmap 换 tokenizer，适配器未来提供 count_tokens）。

## M2 契约新增预告（待 P1 落地 + 契约级全量审查）

> 设计定案见 [history M2](../../../history/2026-11-20-1614-m2-design.md) / [history M2 审计修订](../../../history/2026-11-20-1721-m2-audit-revision.md)
> / [roadmap M2 蓝图](../../../docs/project/roadmap.md#m2-实施蓝图)。
> 以下为**预告**（非当前契约事实），P1 落地时触发契约级全量语义审查 + 二审。

- `ModuleContext` 新增 `llm(role, ChatRequest) -> ChatStream`：kernel 中介 LLM 调用（按 user+module+cap+role 解析绑定解密，
  全计量 call_log **含 module_id 归因**，模块永不触 key/上游）+ `complete_session()`（完成语义，转录保留，发 session-ended；
  **DELETE 隐私删除不发事件**——现 end_session 删除发事件行为须改，审计 B2）+ **cached-synthesis 音频服务**
  （`get_or_synthesize`，模块声明稳定 key，持久资产区 `/data/assets` 豁免淘汰，白名单回归 M2，审计 UX-A3）。
- 新增 `ModuleContextFactory`（`for_session(user, session)` / `for_user(user)`）：组合根构造模块时注入，`TeachingModule::routes()` 签名不动。
  **P1 必决项（审计 B3）**：现 `ModuleContext::session_id() -> SessionId` 非可选 vs for_user 无会话（placement/plan 路由/outbox 消费）张力
  ——Option/独立 UserContext/哨兵三选一；`sessions.plan_version TEXT` vs `LearningPlan.version u32` vs `RunStart.plan_version Option<u32>` 类型对齐（B4）。
- `ModuleKind` 补 `Kernel` 变体（module.yaml kind=kernel 反序列化；YAML crate 选维护中方案，serde_yaml 废弃，审计 B7）。
- **`ProgressEvent` 补 `difficulty` 字段——契约形状新增（审计 B1 诚实标注）**：现 ProgressEvent 有 outcome/item_ref 但**无 difficulty**
  （仅 ProgressInput 有），原「契约形状已有，非契约变更」声称不成立；kernel 0006 DB 补列 + `emit_progress` 签名扩 outcome/item_ref/difficulty。
- 角色音色档案实体 **M2+ 推迟**（无黄金路径消费者），不入本次契约新增。
- **用户级资产备份模块钩子**（模块贡献导出/导入）契约新增**归备份里程碑（实施 M2 后）**，不入 M2 P1。