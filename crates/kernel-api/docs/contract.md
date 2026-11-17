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
  单一 PG 风格迁移集靠 SQLite 亲和性双后端通用。CI `storage` job 真测双后端（required check）。
- `SessionEvent` 为后端→前端 SSE 协议（tag=event, content=data），回合内全序。
- `ProgressEvent` 为持久 outbox 信封；SessionEvicted 的 span 存于 detail。
- `speak` 工具规格由 kernel 定义并经 TtsProvider 执行，教学模块不实现 speak。

## 有意耦合声明（R3）

kernel-api **有意耦合 axum**（`TeachingModule::routes() -> Router`）**与 sqlx**（`Store` 双驱动）。
全应用即 axum，模块可重写不要求框架中立。**升级 axum/sqlx 视为契约变更**，须走契约级全量审查；
未来 agent 不得自作主张"解耦"。此耦合是务实决策，非疏忽。

## 当前状态与 roadmap 差异

契约为 M0 定义，尚无实现 crate。kernel/适配器/教学模块 M1+ 落地。
token 估算当前为字符启发式（roadmap 换 tokenizer，适配器未来提供 count_tokens）。