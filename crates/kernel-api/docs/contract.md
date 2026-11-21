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
| `TeachingModule` | teach-free / teach-gojuon / teach-dialogue / teach-assess | manifest / persona / tools / routes / execute_tool / handle_event |
| `ModuleContext` | kernel | 模块受控服务：store / emit_progress / current_plan / **llm / complete_session / get_or_synthesize** |
| `ModuleContextFactory` | kernel | **for_session / for_user**（每模块一实例，module_id 构造期绑定） |
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

契约 M0 定义；M1a 新增 speak 工具契约（SPEAK_TOOL_NAME/SpeakArgs/speak_tool_spec）；
M1d utoipa 有意耦合（5 REST-facing 类型 ToSchema）。
**M2 P1 契约新增已落地**（history 2130 定案，本节下方详述）：ModuleContext 扩展 + ModuleContextFactory +
ModuleKind::Kernel + ProgressEvent.difficulty + AssetAudio。
kernel（回合循环）+ llm-openai + tts-qwen-audio 适配器 M1a 已实现；auth-local M1c、teach-free M2 P1；
teach-assess/teach-gojuon P3/P4。token 估算当前为字符启发式（roadmap 换 tokenizer，适配器未来提供 count_tokens）。

## M2 P1 契约新增（已落地，契约级全量审查 + 二审见 history P1 条目）

> 设计定案见 [history M2](../../../history/2026-11-20-1614-m2-design.md) / [history M2 审计修订](../../../history/2026-11-20-1721-m2-audit-revision.md)
> / [history P1 前置审计 + B3/B7 定案](../../../history/2026-11-20-2130-m2-pre-p1-audit-fixes.md)
> / [roadmap M2 蓝图](../../../docs/project/roadmap.md#m2-实施蓝图)。

- **`ModuleContext::session_id() -> Option<SessionId>`（B3a 定案）**：for_user ctx 无会话 → None
  （与 ProgressEvent.session_id Option 一致）；需会话方法对 None fail-closed。
- **`ModuleContext::llm(role, ChatRequest) -> ChatStream`**：kernel 中介（按 user+module+cap+role 解析绑定解密，
  **model 由绑定填，req.model 忽略**；流终止全计量 call_log **含 module_id 归因**；模块永不触 key/上游）。
- **`ModuleContext::complete_session() -> bool`**：完成≠删除（completed_at + session-ended 事件，转录保留触发提取）；
  幂等（重复 false）；无会话 → 错误。**DELETE 隐私删除不发事件**语义翻转归 P2（审计 B2，现 end_session 行为暂不动）。
- **`ModuleContext::get_or_synthesize(key, TtsRequest) -> AssetAudio`**：白名单缓存合成（UX-A3 用户侧标准音频）。
  模块声明稳定 key（如 `gojuon:あ`）；命中持久资产直供（cache_hit=true 零上游）；miss 按用户 TTS 绑定合成→
  持久化 /data/assets（豁免淘汰）→计量（module 归因）。作用域 user+module+key+provider 身份（adapter_id+base_url）+voice+model，
  换音色/服务商懒再生新版旧版保留。无 TTS 绑定 → Unconfigured（模块降级不阻塞课时）。
  `AssetAudio{audio_id, pcm, words, format, sample_rate, cache_hit}`（types.rs，非 REST-facing 无 ToSchema）。
- **`ModuleContextFactory`（for_session/for_user）**：每模块一实例（module_id 构造期绑定），组合根构造模块时注入，
  `TeachingModule::routes()` 签名不动。
- **`ModuleKind::Kernel` 变体**：kernel/module.yaml kind=kernel 反序列化（注册表测试锚定全部 crate 清单）。
  YAML 解析 crate = **noyalib 0.0.63**（B7 定案：维护中/MIT-Apache/forbid unsafe/serde 原生；kernel 与模块 crate 依赖）。
- **`ProgressEvent.difficulty: Option<f32>`——契约形状新增（审计 B1 诚实标注）**：serde default 旧信封兼容；
  kernel 0006 DB 补列（outcome/item_ref/difficulty）；**trait `emit_progress(ProgressInput)` 签名不动**
  （ProgressInput 早含三字段，变的是 kernel journal 内部函数 + DB 列）。
- 角色音色档案实体 **M2+ 推迟**（无黄金路径消费者），不入本次契约新增。
- **用户级资产备份模块钩子**（模块贡献导出/导入）契约新增**归备份里程碑（实施 M2 后）**，不入 M2 P1。