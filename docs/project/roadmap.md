# 路线图

> 本文档记录项目目标序列与当前差异。里程碑决策见 [history](../../history/)。

## 里程碑

| 里程碑 | 内容 | 版本 | 状态 |
|---|---|---|---|
| M0 | 脚手架 + CI + 门禁 + 首批文档 + kernel-api 契约 + 最小 hello 镜像 + 容器冒烟测试（CI 集成） | — | 完成 |
| M1 | kernel 完整实现 + auth-local + llm-openai + tts-qwen-audio + 设置 UI（BYOK 可配 + 测试按钮）+ 管理员重置 + schema 前瞻（tokens/users.email）+ Svelte 前端骨架。**拆为 M1a–M1d 四子里程碑**（见下） | — | 进行中（蓝图定案） |
| M2 | teach-assess + teach-gojuon（placement → plan → 自适应课程，黄金路径全通） | — | 未开始 |
| M3 | teach-dialogue | v1.0 | 未开始 |
| M4 | SMTP 自助重置/邮箱验证 + auth-oidc（authentik）+ 账户关联 | v1.1 | 未开始 |

每里程碑结束须人工部署验收（见 [conventions](conventions.md)）。

## 黄金路径

注册（首用户成管理员）→ 配置 provider（预设模板 + 自填 key）→ placement 评估 → 初始教学方案
→ 五十音图自适应课程 → 对话模拟。评估模块订阅进度事件与提取报告，持续重生成教学方案，
为其他教学模块的 LLM 老师制订系统提示词片段。

## M1 拆分蓝图

> 本节是 M1 的实施蓝图（计划，非当前代码事实）；具体实现事实随代码落入各模块 `docs/`。
> 决策依据见 [history](../../history/) M1 拆分条目。

### 拆分原则

1. **风险前置**：最高集成风险（qwen 流式归一化 + DashScope TTS 音频管线 + SSE 协议 + speak 回合）
   放 M1a 用最薄端到端切片验证，而非铺完存储/认证才发现协议跑不通。
2. **零返工接缝**：回合循环从第一天依赖 kernel **内部接缝 trait**（非 kernel-api 契约），
   M1a 用内存/env 实现，M1b/c 换 DB/加密实现——循环逻辑不重写。
3. **验收分层**：无 UI 阶段（M1a/b/c）= CI 自动 + 本地真 key 脚本（人工触发、自动断言）；
   完整人工感官验收只在 M1d（有 UI）。
4. **上下文耗尽友好**：每子里程碑内部再分执行 phase（commit 级），status.md 持续维护，
   任一 phase 边界可交接新 agent。
5. **开发态取舍须回归**：临时旁路/内存生产默认/root 运行等只起临时开发测试作用，
   逐项登记回归里程碑 + 机械守卫（见下），不得静默漂移成永久后门。

### 子里程碑

| 子里程碑 | 范围 | 验收 | 状态 |
|---|---|---|---|
| M1a | 适配器 + 回合循环 + SSE 出声（垂直切片，无持久化/无认证/无 UI）：新 crate `llm-openai`（LlmProvider 全实现）、`tts-qwen-audio`（TtsProvider 全实现，DashScope 原生协议）、`kernel` 最小回合循环（内部接缝 + speak 规格执行 + 基础守护 + SSE）；godoword feature 注册 + 会话 REST + dev 旁路 | CI fixture 黄金 + 回合循环 mock + 守护；本地真 key 出声脚本；冒烟结构性；**无人工视觉** | 未开始 |
| M1b | kernel 存储基底 + 部署初始化（无 UI）：Store pool 初始化 + 迁移编排（自动备份）+ 完整 kernel schema + 加密（主密钥 AES-256-GCM）+ 绑定解析 DB impl + 配置加载（env>file>默认 + 预设）+ **自适应降权 + 幂等 entrypoint** + provider/binding REST + schema 前瞻 | CI 双后端迁移/CRUD/加密/绑定/配置；冒烟完整 uid/卷矩阵；人工残留 SELinux(Fedora)；**无人工视觉** | 未开始 |
| M1c | 会话运行时 + 认证 + 管理（无 UI）：DB SessionRepo/AudioCache 替换内存 + 上下文装配/淘汰/淘汰前进度提取/outbox + call_log 计量 + on_disconnect cancel/continue；新 crate `auth-local`（argon2 + invite/首用户管理员 + cookie/CSRF/限流）+ 管理员重置；godoword 认证中间件 + auth/admin REST | CI auth 流 + 会话持久化双后端 + 淘汰/outbox；本地真 key 完整对话脚本；冒烟 auth 流；**无人工视觉** | 未开始 |
| M1d | Svelte 骨架 + 设置/会话/认证 UI（有 UI）：web/core 框架无关 TS（OpenAPI→TS + SSE 客户端 + AudioWorklet 音频引擎 + 词级高亮）+ web/shell Svelte 5 SPA + i18n zh-CN 首发 + 设置 UI（BYOK + 测试按钮）+ 会话 UI + 认证/管理 UI + Dockerfile node 构建阶段 + 前端 CI job | CI 前端 lint/build/vitest + 冒烟 SPA 伺服；**完整人工验收**（视觉/TTS 音质/BYOK 流/登录流/词级高亮） | 未开始 |

依赖链 M1a → M1b → M1c → M1d（线性，每段把前段接入）。

### 关键设计决策

- **speak 规格落 kernel-api 契约**（决策 A）：新增 `SPEAK_TOOL_NAME` 常量 + `SpeakArgs` 类型化结构
  （LLM 可控子集 text/voice?/rate?/pitch?/instruction?/language_hints?；model/format/sample_rate/word_timestamps
  由 kernel 从绑定/配置填，不暴露给 LLM）+ `speak_tool_spec() -> ToolSpec` + 黄金测试锚定 spec↔SpeakArgs。
  kernel 运行时反序列化 arguments→SpeakArgs→合并绑定→TtsRequest→TtsProvider 执行。
  理由：speak 是众多教学模块共用面，置契约层使演进触发全量审查防漂移，与 TtsRequest 代码锚定防分裂。
  此为契约新增，M1a 执行时走全量 docs+wiki 语义审查；修订 contract.md "kernel 定义"措辞与 P26 对齐。
- **kernel 内部接缝**（非契约）：`SessionRepo`/`BindingResolver`/`AudioCache` 三 trait。
  内存 impl 作测试替身长期保留；M1a 作生产默认是临时态，M1b/c 换 DB/文件系统 impl。
- **bindings 默认哨兵 `*`**（取代 NULL）：`UNIQUE(user,module,cap,role)` 在两后端 NULL 皆互不相等致默认绑定
  唯一性失效；用哨兵 `*` 表"全模块默认"，UNIQUE 正常约束、解析一致。M1b 落地时同步 architecture.md。
- **M1a dev 旁路**：env `GODOWORD_DEV_LLM_KEY`/`GODOWORD_DEV_TTS_KEY` + base_url/model + 固定 dev user，
  装进 cargo feature `dev-bypass`（release 永不启用），M1c 删除回归真认证。
- **fixtures 重录**：M1a 首任务用本地真 key 录制脱敏 fixture（LLM SSE 含 reasoning_content/tool_calls；
  TTS SSE sentence 事件 + base64 PCM + 词级时间戳），入 `crates/<adapter>/tests/fixtures/`，CI 永不用真 key。
- **OpenAPI 时序**：REST 端点随 M1a/b/c 落 utoipa 标注，M1d 生成 TS；SSE 不入 OpenAPI（单独协议文档）。
- **M1 无教学模块**：回合循环用 kernel 默认教师 persona（无模块 persona/无 plan.teacher_prompt），
  sessions.module_id 可空；默认教师模式是否作永久"自由聊天"产品态留 M2 拍板。

### 开发态临时取舍回归机制

| # | 临时取舍 | 引入 | 回归于 | 回归断言 |
|---|---|---|---|---|
| T1 | env dev key 旁路 + 固定 dev user | M1a | M1c | dev-bypass feature 及 `GODOWORD_DEV_*` 从非测试代码删除 |
| T2 | 内存 impl 作生产默认（会话/缓存不持久化） | M1a | M1b/M1c | 生产默认换 DB/文件系统；内存 impl 仅留测试 |
| T3 | 容器 root 运行（无降权/无 /data 初始化） | M1a | M1b | 自适应降权 + 幂等 entrypoint 上线 |
| T4 | 无模块默认教师模式 | M1a | M2 决议 | M2 定永久产品态或退役 |

机制：①dev 旁路装 cargo feature `dev-bypass`，release Dockerfile 永不启用（硬隔离）；
②代码标记 `// DEV-DEBT: T<n> 回归于 M<x>`（可 grep 枚举）；③status.md 登记，每里程碑验收核对，逾期阻塞合并；
④回归 PR 强制断言；⑤CI 守卫 release 构建路径无 dev-bypass 启用。
永久架构（接缝 trait + 内存测试替身）非债务，不需回归。

### 验收策略矩阵

| 阶段 | CI 自动 | 本地真 key 脚本 | 冒烟 | 人工感官 |
|---|---|---|---|---|
| M1a | fixture 黄金 + 回合循环 mock + 守护 | LLM+TTS 出声端到端 | 结构性 | 无 |
| M1b | 双后端迁移/CRUD/加密/绑定/配置 | — | 完整 uid/卷矩阵 | SELinux(Fedora) |
| M1c | auth 流 + 会话持久化 + 淘汰/outbox | 完整对话+淘汰+进度 | auth 流 curl | 无 |
| M1d | 前端 lint/build/vitest | 完整产品流 | SPA 伺服 | **完整人工验收** |

### kernel schema 轮廓（M1b 落地计划，列细节入 kernel/docs/storage.md）

users（email 可空前瞻/is_admin/status）、local_credentials（argon2）、oidc_identities（M4 前瞻，UNIQUE(issuer,subject)）、
providers（key_enc BYTEA：nonce+密文+tag）、bindings（module_id 哨兵 `*`/params_json 整取）、
sessions（module_id/plan_version 可空/on_disconnect）、messages（tool_calls/audio_refs JSONB 整取/reasoning 可空）、
call_log（tokens/characters 可空/latency_ms/status）、progress_events（skill_tags/detail JSONB 整取）、
tokens（前瞻 purpose/token_hash/expires_at/单次）、invites（code_hash/max_uses/used_count）。
音频缓存走文件系统 `/data/cache`（key=hash(model+voice+text+params)）+ sidecar 时间戳 JSON，可再生可丢弃不入库。
双兼容 SQL 子集：uuid 主键应用生成、时间戳应用侧、JSON 整取回 Rust、占位符 `$1`、key_enc BYTEA 亲和。

### REST / SSE 面（按阶段）

- M1a：`POST/GET/DELETE /api/v1/sessions`、`GET /sessions/{id}`、`POST /sessions/{id}/messages`（→SSE）、
  `POST /sessions/{id}/cancel`、`GET /api/v1/audio/{id}`
- M1b：`/api/v1/providers` CRUD、`/api/v1/bindings` CRUD、`GET /api/v1/config`（脱敏只读）、
  `GET /providers/{id}/models`、`GET .../voices`（测试按钮）
- M1c：`/api/v1/auth/{register,login,logout,me}`、`/api/v1/admin/{reset,invites,users}`
- SSE 不入 OpenAPI；REST 全 utoipa 标注，M1d 生成 TS。

## 远期项（未排期）

- tokenizer 计数替代字符启发式估算（适配器按模型族提供 count_tokens）
- anthropic 兼容适配器（llm-anthropic）
- STT 适配器与听说互动（网关已有 qwen-audio-3.0-asr-flash / realtime-plus）
- MySQL 存储驱动（当前 SQLite + PostgreSQL）
- sqlx 支持 patched SQLite（libsqlite3-sys 0.39+ / SQLite 3.54）后升级，移除 cargo-deny
  对 RUSTSEC-2026-0459 的忽略（当前 sqlx 0.8/0.9 均 pin <0.38，无上游修复路径）
- 更多教学模块（卡片闯关、LLM 考官考试等）与失败模块退役
- PWA / 移动端增强
- E2E 自动化测试（当前单测 + fixture + 人工验收）
- mid-run 回放缓冲（若需求出现）
- 会话自动归档、LLM 生成会话标题
- 信封加密（per-user data key）、主密钥轮换工具
- RP-initiated logout（OIDC 会话联动）

## 当前差异（M0 + M1 前置完成，M1 蓝图定案）

- M1 拆分为 M1a–M1d 四子里程碑（见上蓝图），垂直切片先行；speak 规格落 kernel-api 契约（决策 A）；
  开发态临时取舍登记回归机制。M1a 待启动（首任务 fixtures 录制）。

- 存储契约 **C 方案**（Store enum 双具体驱动 + 薄分发宏 + 双兼容 SQL 子集）已定，
  双后端 CI 测试（storage required check）已立；kernel 存储编排实现 M1。sqlx Any 已证伪弃用。
- 协议类型（SessionEvent/ModuleManifest）serde 黄金测试已立；kernel-api 有意耦合 axum/sqlx 已声明。
- 架构契约（kernel-api trait）已定义，但 kernel/适配器/教学模块均未实现。
- 前端为静态占位，Svelte 骨架 M1 起。
- token 估算为字符启发式（roadmap 换 tokenizer）。
- 部署仅 hello 服务，自适应降权/卷初始化完整逻辑 M1 落地。
- M0 人工残留验收已通过：SELinux/quadlet（Fedora 44）、docker 含 compose（真实 Debian 13）。