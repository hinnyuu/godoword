# 路线图

> 本文档记录项目目标序列与当前差异。里程碑决策见 [history](../../history/)。

## 里程碑

| 里程碑 | 内容 | 版本 | 状态 |
|---|---|---|---|
| M0 | 脚手架 + CI + 门禁 + 首批文档 + kernel-api 契约 + 最小 hello 镜像 + 容器冒烟测试（CI 集成） | — | 完成 |
| M1 | kernel 完整实现 + auth-local + llm-openai + tts-qwen-audio + 设置 UI（BYOK 可配 + 测试按钮）+ 管理员重置 + schema 前瞻（tokens/users.email）+ Svelte 前端骨架。**拆为 M1a–M1d 四子里程碑**（见下） | — | 完成（M1a/M1b/M1c/M1d 全部完成） |
| M2 | teach-assess + teach-gojuon + teach-free（placement → plan → 自适应课程，黄金路径全通） | — | 设计审计修订（PR #21） |
| M2+ | 用户级资产备份（导出/导入，设计定案见 roadmap 专节） | — | 设计定案（实施 M2 后） |
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
| M1a | 适配器 + 回合循环 + SSE 出声（垂直切片，无持久化/无认证/无 UI）：新 crate `llm-openai`（LlmProvider 全实现）、`tts-qwen-audio`（TtsProvider 全实现，DashScope 原生协议）、`kernel` 最小回合循环（内部接缝 + speak 规格执行 + 基础守护 + SSE）；godoword feature 注册 + 会话 REST + dev 旁路 | CI fixture 黄金 + 回合循环 mock + 守护；本地真 key 出声脚本；冒烟结构性；**无人工视觉** | 完成 |
| M1b | kernel 存储基底 + 部署初始化（无 UI）：Store pool 初始化 + 迁移编排（自动备份）+ 完整 kernel schema + 加密（主密钥 AES-256-GCM）+ 绑定解析 DB impl + 配置加载（env>file>默认 + 预设）+ **自适应降权 + 幂等 entrypoint** + provider/binding REST + schema 前瞻 | CI 双后端迁移/CRUD/加密/绑定/配置；冒烟完整 uid/卷矩阵；人工残留 SELinux(Fedora)；**无人工视觉** | 完成 |
| M1c | 会话运行时 + 认证 + 管理（无 UI）：DB SessionRepo/AudioCache 替换内存（音频默认新鲜合成 + 文件系统回放 + 缓存有界）+ 上下文装配/淘汰（水位制）/淘汰前 span 入 outbox（LLM 提取 M2）+ call_log 计量 + on_disconnect cancel/continue；新 crate `auth-local`（argon2id + invite/首用户管理员 + cookie/CSRF/限流）+ 管理员重置 token 化；godoword 认证中间件 + auth/admin REST。**M1c-hardening 加固**：对象级授权 + 持久化失败关闭 + 硬超时/退出屏障 + 上游重试（第三方审计 C1-C9） | CI auth 流 + 会话持久化双后端 + 淘汰/outbox + 两用户越权负例；本地真 key 完整对话脚本；冒烟 auth 流；**无人工视觉** | 完成（含加固） |
| M1d | Svelte 骨架 + 设置/会话/认证 UI（有 UI）：web/core 框架无关 TS（OpenAPI→TS + SSE 客户端 + AudioWorklet 音频引擎 + 词级高亮）+ web/shell Svelte 5 SPA + i18n zh-CN 首发 + **高自定义/可美化 UI**（用户可定制界面，核心需求见 [README 产品愿景](../../README.md#产品愿景与核心需求)）+ 设置 UI（BYOK + 测试按钮）+ 会话 UI + 认证/管理 UI + Dockerfile node 构建阶段 + 前端 CI job | CI 前端 lint/build/vitest + 冒烟 SPA 伺服；**完整人工验收**（视觉/TTS 音质/BYOK 流/登录流/词级高亮） | 完成 |

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
- **OpenAPI 时序**：REST 端点随 M1a/b/c 落 utoipa 标注，M1d 生成 TS；SSE /messages path 入 OpenAPI，SessionEvent 事件 schema 不入（单独协议文档）。
- **M1 无教学模块**：回合循环用 kernel 默认教师 persona（无模块 persona/无 plan.teacher_prompt），
  sessions.module_id 可空；默认教师模式产品态留 M2 拍板（M2 决议为 teach-free 自由对话模块，history 1614）。

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
音频缓存走文件系统 `/data/cache`（随机 AudioId + sidecar 时间戳 JSON，可再生可丢弃不入库，TTL+容量上限有界）。
**默认新鲜合成**（每次新合成存供回放）；复用由模块**显式白名单**声明稳定 key（取代指纹 hash 推断——
不靠指纹推断同一性，由模块权威声明；**M2 回归**：用户侧 gojuon 标准音频即消费者，持久资产区豁免淘汰，见 M2 蓝图/审计修订）。
双兼容 SQL 子集：uuid 主键应用生成、时间戳应用侧、JSON 整取回 Rust、占位符 `$1`、key_enc BYTEA 亲和。

### REST / SSE 面（按阶段）

- M1a：`POST/GET/DELETE /api/v1/sessions`、`GET /sessions/{id}`、`POST /sessions/{id}/messages`（→SSE）、
  `POST /sessions/{id}/cancel`、`GET /api/v1/audio/{id}`
- M1b：`/api/v1/providers` CRUD、`/api/v1/bindings` CRUD、`GET /api/v1/config`（脱敏只读）、
  `GET /providers/{id}/models`、`GET .../voices`（测试按钮）
- M1c：`/api/v1/auth/{register,login,logout,me}`、`/api/v1/admin/{reset,invites,users}`
- SSE /messages path 入 OpenAPI、SessionEvent 事件 schema 不入（单独协议文档）；REST 全 utoipa 标注，M1d 生成 TS。

## M1d 实施蓝图

> 本节是 M1d 的实施蓝图（计划，非当前代码事实）；决策依据见 [history M1d 设计](../../history/2026-11-19-2049-m1d-design.md)。
> 需求驱动多轮讨论定案（Q1–Q17），具体实现事实随代码落入各模块 `docs/`。

### 范围与姿态

完整图形界面（认证/管理/引导/设置/会话 + 主题引擎 + 高自定义 WebUI）。后端 REST 面扩展 +
**一次有界 kernel-api 耦合变更**（5 个 REST-facing 类型加 `utoipa::ToSchema` derive + utoipa 有意耦合）→
**契约级全量语义审查**。trait 签名/协议 serde 形状不变。单实施 PR 多 phase（音频 spike 前置），status 逐 phase 维护。
**布局拖拽/组件重排永久放弃**（长期稳定优先，不入 roadmap）。

### 关键设计决策

- **会话文件夹 = 虚拟路径**：`sessions.path TEXT`（NULL=未分类）存文件夹链（`/日语/五十音`），路径即身份，前端 trie 渲染；
  会话以标题（首条用户消息截断）为叶子，会话名不进 path。**嵌套 M1d 即做**（trie 渲染 + 前缀批量改写 + 子串检查，天然无环）。
  空文件夹持久登记表 `session_folders(user_id,module_id,path,UNIQUE(user_id,module_id,path))`（path NOT NULL 避 NULL-UNIQUE 陷阱）。
  **模块 chroot**：module 隐式根命名空间（非可见路径段），M1 无模块=哨兵 `default`，M2 每模块一套树。
  删非空文件夹→子会话上移一层（绝不级联删会话）。服务端 NFC 归一化（日语浊音陷阱）+ 校验；
  **上限固定常量**（深度≤8/段≤64 字符/全路径≤1024，不进 config）；校验仅写入时，已存数据永不回溯失效（放宽安全/收紧需 grandfather）。
  前缀匹配 Rust 侧（不用 LIKE，守 JSON 整取纪律）。
- **偏好跨设备随账户**：`user_preferences(user_id PK,prefs_json,updated_at)` + GET/PUT REST（符 history 0530 用户级配置仅 DB+webui）。
- **音频**：默认手动点播 + 自动播放开关；音色试听计量端点；回放时间戳出口 `GET /audio/{id}/meta`（补 P25 回放高亮需求）；
  AudioWorklet 风险前置 spike（AudioContext sampleRate:24000 + 环形缓冲 + 高亮播放时钟）。
- **设置页**：简化三卡片（默认 LLM/TTS+音色/可选摘要模型）+ 高级折叠完整绑定矩阵。
- **认证**：自助改密（验旧密→统一策略→撤销其他会话保留当前）+ 显示名编辑；重置独立公开页。
- **类型生成**：openapi-typescript + openapi-fetch，生成类型提交 + CI freshness diff；后端补全 OpenAPI schema（B，单一事实源防漂移）。
- **前端分层**（history 0330）：web/core 框架无关 TS（api/sse/audio/session/theme/i18n/prefs，vitest 重测）+
  web/shell Svelte 5 runes SPA（组件轻测，视觉人工）；领域逻辑不进组件；不引外部状态库；
  SSE fetch ReadableStream + eventsource-parser；i18n svelte-i18n zh-CN 首发；主题 CSS 变量引擎；
  路由 history 模式 + axum ServeDir index.html fallback（修 SPA 深链 404）。
- **用量**：M1d 仅回合级 usage 脚注（看板 M2）；**响应式**含移动视口（人工真机验收）；首次使用引导清单。

### REST 扩展面（M1d，均 AuthUser + 对象级归属，不动 trait 签名）

- `GET/PUT /api/v1/preferences`；`GET/POST /folders`、`PUT/DELETE /folders/{id}`
- `GET /sessions?folder=&page=`（元数据：title/created_at/last_active/message_count/path）、`PUT /sessions/{id}/path`、`GET /sessions/{id}`（+path）
- `GET /audio/{id}/meta`（words/format/sample_rate）；`POST /providers/{id}/preview`（试听→audio_id，计量）
- `POST /auth/change-password`；`PUT /auth/me`
- 0005 迁移：user_preferences + session_folders + sessions.path。schema 补全：kernel-api 5 类型 + auth-local UserInfo + godoword 新 DTO ToSchema。

### Phase 拆分（commit 级，风险前置，上下文耗尽友好）

| Phase | 内容 |
|---|---|
| P0 | 设计落档 PR（roadmap 蓝图 + history + status + glossary + conventions 前端纪律） |
| P1 | 音频 spike（web/core audio 引擎 + worklet 出声 + 高亮同步，本地真 key + 移动浏览器实测，跑通再铺 UI） |
| P2 | 后端扩展（kernel-api ToSchema + 0005 + path/preferences/folders/sessions-meta + auth-local + REST + schema 补全 + 双后端测试 + 契约级全量审查） |
| P3 | web/core（api/sse/session/theme/i18n/prefs + vitest + 类型 freshness） |
| P4 | web/shell 认证/管理/引导 UI |
| P5 | web/shell 设置 UI（provider/绑定/试听/主题/偏好） |
| P6 | web/shell 会话 UI（对话 + 文件夹树 + 音频卡片 + 高亮 live/回放 + usage） |
| P7 | Dockerfile node 阶段 + SPA fallback + 前端 CI job + 冒烟扩展 |
| P8 | 验收（CI 全绿 + R4 main 重测 + 人工残留 sign-off + 全状态面 reconcile + history 实现条目） |

### 验收策略

CI 前端 lint/svelte-check/build/vitest + openapi 类型 freshness + rust + storage 双后端 + 冒烟（SPA 伺服 + 深链 fallback + 新 schema）+
docs-gate/test_gate + commitlint + secrets + cargo-deny（含 unicode-normalization）。本地真 key 完整产品流。
**完整人工感官验收**（视觉/TTS 音质/BYOK 流/登录重置流/词级高亮 live+回放/移动视口 AudioWorklet 真机/自动播放手势/主题自定义）；
Dockerfile 变更→部署矩阵（SELinux/quadlet/docker）重 sign-off。人工残留按清单纪律（逐项命令+具体 tag+标准+清理）。

## M2 实施蓝图

> 本节是 M2 的实施蓝图（计划，非当前代码事实）；决策依据见 [history M2 设计](../../history/2026-11-20-1614-m2-design.md)
> 与 [history M2 审计修订](../../history/2026-11-20-1721-m2-audit-revision.md)（第三方审计 + 主开发者裁决，精化 1614）。
> 需求驱动六轮讨论定案（R1–R6）+ 审计修订轮，**设计审计通过后方启动 P1**。具体实现事实随代码落入各模块 `docs/`。

### 范围与姿态

teach-assess（评估核心）+ teach-gojuon（自适应五十音）+ teach-free（自由对话模块，T4 决议）黄金路径全通。
携带动项入 MVP：用量看板 + M1c 一致性检查 + 真正再生；**白名单缓存回归 M2**（用户侧标准音频的消费者，审计修订）；
**推迟角色音色档案实体**（无黄金路径消费者，M2+ 方向）。**用户级资产备份设计已定案、实施 M2 后**（见下节）。
**风险前置垂直切片**（承 M1a）：P1 先最薄模块集成切片跑通端到端，再铺 assess/gojuon。
含**一次有界 kernel-api 契约新增**（ModuleContext LLM 接缝/complete_session/ctx 工厂/ModuleKind::Kernel/
cached-synthesis 音频服务 + ProgressEvent difficulty 形状新增）→**契约级全量语义审查 + 二审**。

### 关键设计决策

- **混合式课程**：结构化操练卡（确定性判定→结构化进度事件 ExerciseOutcome）+ LLM 对话课（persona + plan 钉住→提取报告）。双通道进度。
  操练判定在模块前端（确定性），进度事件信任客户端（自学工具自己的数据，非安全边界——审计修订明示信任姿态）。
- **teach-free（T4）**：自由对话成轻量教学模块，内核默认 persona 移入；既有无模块会话/default 文件夹迁入；plan 可选
  （未 placement 也能自由对话，persona-only，转录仍喂评估）。迁移哨兵涟漪（DEFAULT_NAMESPACE/文件夹查询默认/列表过滤/前端 module 参数）P1/P5 核对。
- **Placement**：自评问卷 + 确定性迷你测（标记已会）→ assessor 初始方案；可选 LLM 互动短测精炼。零基础不被迫考试。
- **gojuon 无方案默认节奏（审计修订 UX-A1）**：`current_plan=None` 允许默认节奏直接学（固定顺序、不跳过），placement 随时可补，不强制。
- **自适应**：固定教学顺序 + 跳过已会 + plan 驱动侧重/节奏/讲解；复习 = 操练加权 + 专项复习课。
- **一课三段式**：引入对话→操练卡→收尾对话；完成发 session-ended（完成≠删除，转录保留触发提取）。
  **DELETE 隐私删除不发事件不提取**（审计修订 B2：现 end_session 删除发事件行为须改，**M2 P2 与 complete_session 同落**，R6/N2 测试随 P2 更新）。
- **操练题型**：识别 + 听音 + 认读（v1 无 STT，确定性判定）；课时粒度一行/组 ~5 项。
- **评估机制**：提取报告 = 结构化观察条目 {skill_tag,evidence,outcome_hint,confidence}；重规划 = 全量再生自动应用 version+1
  （并发锁 + 失败保留旧版；**锁不得在写事务内跨 LLM 调用持有**——短事务认领行 + 进程内 per-user 异步锁，审计 B6）；
  阈值 ≥20 事件 或 ≥3 报告（可配）+ 手动。
- **内容**：全假名 ~104（清音/濁音/半濁音/拗音），数据驱动。
- **标准音频 = 用户侧生成（审计修订 UX-A3，静态入仓库作废）**：DashScope 输出权利不满足 AGPL 再分发→
  每用户用自己 TTS key/音色生成，**一次生成永久使用**（持久卷损毁除外，按需懒再生）。**白名单 cached-synthesis 承载**
  （history 0329 机制回归）：模块声明稳定 key（`gojuon:あ`）→ kernel get_or_synthesize（命中直供/未命中合成→持久化→
  call_log 模块归因计量）；作用域含 provider 身份（adapter_id+base_url）+ voice + model，换音色/服务商→懒再生新版旧版保留。
  **持久资产区 `/data/assets/`**（豁免 TTL/容量淘汰，与 `/data/cache` 回放缓存正交）+ kernel 资产登记表。
  **懒生成 + 可选一键全量**（~104 项 ~520 字符 ~5MB/用户）；可后台预取下一课。**降级不阻塞课时**：无 TTS 绑定/合成失败→
  识别+认读照常，听音暂缺提示配置/重试。
- **前端**：学习仪表盘首页 + /m/<id> 模块路由；**运行时懒加载 chunk**（web/modules/<id> 独立入口 + core 共享 chunk；
  mount 单路径：模块 import 共享 core 单例，services 仅传路由/nav 上下文，审计 B9）；
  方案面板 + 掌握度热力图（提取报告内部）；历史组件按命名空间复用。**移动适配入 M2（审计 UX-A2）**：
  新 UI 同步做好手机适配，M1d 侧栏固定宽挤压一并处理（P5）。
- **用量看板**：call_log 按模块/能力/模型聚合 + 近期调用（示数量不示金额；**0006 补 call_log.module_id 归因**，审计 B4）。
  **真正再生**：再生端点替换失败回合。

### 模块架构与契约变更

- **kernel 模块注册表**：module.yaml 运行时反序列化（ModuleKind 补 Kernel；YAML crate 选维护中方案，serde_yaml 废弃，审计 B7）
  + feature 注册 + 迁移编排（kernel→模块注册序，版本键 mod_<id>_NNNN）。
- **ModuleContext 实现 + 契约新增**：store/emit_progress/current_plan 落地 + `llm(role,req)->ChatStream`（kernel 中介，全计量含 module_id，模块不触 key）
  + `complete_session()` + **cached-synthesis 音频服务**（get_or_synthesize，持久资产区）；**ModuleContextFactory**
  （for_session/for_user，组合根构造期注入，routes() 签名不动）。**P1 契约审查必决项（审计 B3）**：
  session_id() 非可选 vs for_user 无会话张力（Option/独立 UserContext/哨兵）；plan_version TEXT vs u32 类型对齐（B4）。
- **ProgressEvent difficulty（审计 B1 诚实标注）**：契约形状**新增** difficulty（现仅 ProgressInput 有，原「形状已有」声称不成立）；
  emit_progress 签名扩 outcome/item_ref/difficulty；0006 DB 补列。
- **回合循环接线**：模块 persona + plan.teacher_prompt 钉住 + 模块工具分发（speak kernel 统一）+ on_session_start/end + 绑定解析带 module_id。
- **outbox 扇出 + 幂等（A5）+ 消费守护（审计 B5）**：consumes_events 订阅扇出 + kernel 级 event_consumptions per-subscriber 投递表；
  **有界并发/per-event spawn + 超时 + 重试上限/死信**（防慢提取阻塞全用户/毒丸饿死后续）。
- **kernel 0006**：progress_events 补 outcome/item_ref/difficulty 列 + **call_log 补 module_id 列**（审计 B4）
  + OutboxEvent 取回 created_at；sessions 补 completed_at。
- **删除事件语义（审计 B2）**：DELETE 隐私删除不发事件；complete_session 发 session-ended（触发提取）。
- **私有表**：mod_teach_assess_*（plans/reports/聚合）+ mod_teach_gojuon_*（item_progress/lessons）+ teach-free 迁移
  + kernel 资产登记表（user/module/稳定 key/provider 身份/voice）。

### REST 扩展面（M2，均 AuthUser + 对象级归属）

- 模块注册/清单 `GET /api/v1/modules`；模块路由挂载 `/api/v1/m/{id}/`（TeachingModule.routes）
- teach-assess：placement（问卷/迷你测/LLM 短测）+ `GET /plan`（当前方案）+ `POST /plan/reassess`（手动重规划）+ 版本历史
- teach-gojuon：课程数据/课时（新授/复习）/操练判定/进度/掌握度 + **用户音频资产路由**（标准音频直供 + 一键全量生成 + 单项再生）
- 会话：`POST /sessions` 带 module_id；`POST /sessions/{id}/complete`（完成语义）；`POST /sessions/{id}/regenerate`（再生）
- 用量：`GET /api/v1/usage`（按模块/能力/模型聚合 + 近期调用，call_log.module_id 归因）
- 模块迁移 mod_<id>_* 表 + 注册序追加。schema 补全：新 DTO ToSchema（OpenAPI 单一事实源）。

### Phase 拆分（commit 级，风险前置，上下文耗尽友好）

| Phase | 内容 |
|---|---|
| P0 | 设计落档 PR（roadmap 蓝图 + history + status + glossary + conventions + contract.md 预告）+ **审计修订轮**（本 PR） |
| P1 | **风险前置切片**：注册表 + ModuleContext 实现 + 回合接线 + teach-free → 模块会话端到端（契约新增 + 全量审查 + 二审；B3 必决项此 phase 定） |
| P2 | kernel 基底：0006（含 call_log.module_id）+ outbox 扇出 + event_consumptions 幂等 + 消费守护（B5）+ complete_session + 删除事件语义（B2）+ 模块迁移 runner |
| P3 | teach-assess 后端：mod 表 + placement + PlanProvider + 提取 + 重规划 + 阈值/锁（B6 约束） |
| P4 | teach-gojuon 后端：课程数据 + 课时调度 + 操练判定 + 进度 + **用户侧音频机制**（白名单合成服务 + 持久资产区 + 懒生成/一键全量 + 降级）+ 模块 REST |
| P5 | 前端：懒加载集成（B9 mount 单路径 spike）+ 仪表盘 + 模块路由 + assess UI + gojuon UI + teach-free 包装 + **移动适配（UX-A2）** |
| P6 | 用量看板 + 再生 API + M1c 一致性检查（含主回合/TTS EOF 对称化） |
| P7 | Dockerfile 资产 + CI 扩展 + 冒烟扩展 |
| P8 | 验收（CI 全绿 + 真 key 黄金路径脚本 + 人工感官 + 部署矩阵 + R4 + 全状态面 reconcile） |

### 验收策略

CI 模块双后端测试（mod 表 CRUD/迁移）+ 前端 vitest/svelte-check/build（含懒加载 chunk）+ 冒烟扩展（模块 REST/placement 流）+
契约 serde 黄金 + 全量审查矩阵。本地真 key 黄金路径脚本（register→placement→plan→gojuon 一课→提取→重规划→复习课）。
**完整人工感官验收**（黄金路径 UX + **用户侧生成音质** + 操练交互 + **移动端（UX-A2）** + 方案演进可见）；Dockerfile 变更→部署矩阵重 sign-off。
人工残留按清单纪律。可靠性负例锚定（重规划并发/失败保留旧版/**锁不阻塞写（B6）**、提取失败留待/**毒丸重试上限（B5）**、
outbox 重投幂等、**完成 vs 删除事件语义（B2）**、跨用户越权、模块迁移回滚、再生原子、**call_log 模块归因（B4）**、
**白名单命中/miss 合成/持久资产豁免淘汰/换音色懒再生**、**删文件夹冲突 400（F1，代码修复 PR）**）。

## 用户级资产备份（设计定案，实施 M2 后）

> 决策依据见 [history M2 审计修订](../../history/2026-11-20-1721-m2-audit-revision.md)。M2 后 M3 前或独立小里程碑实施；
> 本轮仅定案设计，M2 不实施（范围克制）。

- **范围**：用户级 kernel 数据（providers **不含 key**/bindings/sessions+转录/folders/preferences/progress/call_log）
  + 模块私有数据（**模块经契约钩子贡献导出/导入——契约新增归本里程碑**）+ 音频（持久资产必含，回放缓存可选勾选）。
- **格式**：单 ZIP（manifest.json 版本/用户/时间 + data.json + audio/*.pcm+sidecar）；schema 版本向前兼容（新包旧应用拒绝）。
- **密钥**：v1 不含 key（key_enc 主密钥加密跨部署不可解，导入重填，BYOK 教义不破）；passphrase 加密含 key 留可选增强。
- **导入**：恢复向——仅空账户或显式「覆盖我的数据」确认（事务原子先删后导）；**不做合并**；仅用户自助，管理员不可代操作。
- **安全硬要求**：zip-slip 防护 + 解压炸弹上限（条目数+总量）+ 上传体积上限 + 单事务原子 + 音频暂存失败清理（失败关闭）。
- **价值**：寄部署用户无卷访问权→用户级导出=数据可携带权，与整卷备份（部署级）正交互补。

## 远期项（未排期）

- tokenizer 计数替代字符启发式估算（适配器按模型族提供 count_tokens）
- anthropic 兼容适配器（llm-anthropic）
- STT 适配器与听说互动（网关已有 qwen-audio-3.0-asr-flash / realtime-plus）
- MySQL 存储驱动（当前 SQLite + PostgreSQL）
- sqlx 支持 patched SQLite（libsqlite3-sys 0.39+ / SQLite 3.54）后升级，移除 cargo-deny
  对 RUSTSEC-2026-0459 的忽略（当前 sqlx 0.8/0.9 均 pin <0.38，无上游修复路径）
- chrono unmaintained（RUSTSEC-2026-0543）风险接受跟踪：监控社区 fork/复活或 sqlx 切换
  jiff/time，届时迁移并移除 cargo-deny 忽略（当前 sqlx 传递依赖 + 契约类型用 chrono，替换不可行）
- 更多教学模块（卡片闯关、LLM 考官考试等）与失败模块退役
- M2 一致性检查（入 M2 P6，多轮审计非阻断观察）：summarize 与主回合循环 Finish 处理对称化（usage 计量完整性，
  Finish 后 usage chunk 不漏记；**含 TTS 静默 EOF 部分音频不得当完整成功**，审计 O1）+ outbox 消费侧幂等（A5，模块消费者落地时）+ 淘汰断言助手增强
- 角色扮演横切声音基础设施（地基 persona/binding/voice 原语 + 模块编排 + 内核角色音色档案实体；
  音色克隆归服务商，未来项目级克隆做成独立模块产 TTS API）——M2+ 方向（角色音色档案实体 M2 推迟，无黄金路径消费者），见 history 角色扮演条目
- PWA / 移动端增强
- 移动端布局优化（**M2 P5 承接**：新 UI 移动适配 + M1d 侧栏固定宽挤压一并处理，审计 UX-A2 裁决；剩余 PWA 级增强仍远期）
- 对话 Markdown/HTML 渲染（marked/markdown-it + DOMPurify sanitization 防 XSS，M1d 纯文本 pre-wrap，未来与 HTML 支持一起做）
- E2E 自动化测试（当前单测 + fixture + 人工验收）
- mid-run 回放缓冲（若需求出现）
- 会话自动归档、LLM 生成会话标题
- 信封加密（per-user data key）、主密钥轮换工具
- RP-initiated logout（OIDC 会话联动）

## 当前差异（M1 全部完成 M1a–M1d，M2 设计审计修订 PR #21）

- **M2 设计定案 + 审计修订**（history 1614 六轮 R1–R6 + history 1721 第三方审计修订）：teach-assess + teach-gojuon + teach-free 黄金路径；
  混合式课程（操练卡 + 对话课双通道进度）；placement（问卷 + 迷你测 + 可选 LLM 短测）→ 全量再生重规划（version+1，阈值 + 手动）；
  全假名 ~104 数据驱动 + **用户侧标准音频**（白名单 cached-synthesis 回归 M2 + 持久资产区，静态入仓库因 AGPL 前提作废）；
  gojuon 无方案默认节奏（UX-A1）；学习仪表盘 + 模块路由 + 运行时懒加载 chunk + **移动适配入 M2（UX-A2）**；
  用量看板（call_log.module_id 归因）+ 真正再生 + M1c 一致性检查入 MVP；角色音色档案推迟 M2+。
  **有界 kernel-api 契约新增**（ModuleContext LLM 接缝/complete_session/ctx 工厂/ModuleKind::Kernel/cached-synthesis +
  ProgressEvent difficulty 形状新增）→契约级全量审查 + 二审。**用户级资产备份设计定案、实施 M2 后**（见上节）。
  审计修订在 PR #21，**设计审计通过后启动 P1**；代码审计发现（F1/O 系列）另开修复 PR。

- **M1d 完成**（history 2049 设计 + 2130/2230/2310/2345/0010/0040 实现 + 0130/0210/0240 验收修复）：
  Svelte 5 SPA 完整图形界面（认证/管理/引导/设置/对话/会话文件夹）+ web/core 框架无关 TS（音频引擎词级高亮 live/回放 +
  openapi-fetch 生成类型 CI freshness + SSE + 主题引擎 + i18n zh-CN + 偏好跨设备）+ 后端扩展（虚拟路径文件夹/偏好/会话元数据/
  音色试听/回放时间戳出口/自助改密）+ **kernel-api utoipa 耦合（5 类型 ToSchema，契约级全量审查）** + Dockerfile 三阶段 node 构建。
  布局拖拽永久放弃；Markdown/HTML 渲染 + 移动布局优化延期远期。
  **合并前审计修复轮**（history 1240 一审 + 1320 二审 + 1345 合并验收）：文件夹祖先链原子登记/改名冲突预检/上限重校验、改密 403 语义、
  试听上限+超时+完整计量、适配器超时、错误分流/usage 数字/三卡片/布局偏好/改名 UI/i18n 清扫、frontend required check（G1）；
  负例测试同配（双后端 + handler 级 + vitest + 冒烟 M1d 面）。

- M1 拆分为 M1a–M1d 四子里程碑（见上蓝图），垂直切片先行；speak 规格落 kernel-api 契约（决策 A）；
  开发态临时取舍登记回归机制。**M1a 完成**：适配器 + 回合循环 + SSE 出声。**M1b 完成**：存储基底 + 加密 + 绑定 DB + 配置 + 自适应降权。
  **M1c 完成**：会话/音频持久化 + 上下文预算/淘汰 + outbox/call_log + continue detach + auth-local 认证 + 管理（本地真 key 走真认证验证）。
- 存储契约 **C 方案**（Store enum 双具体驱动 + 薄分发宏 + 双兼容 SQL 子集）已定，
  双后端 CI 测试（storage required check）已立；**kernel 存储编排 M1b 已实现**（自定义双后端迁移 runner + 备份）。sqlx Any 已证伪弃用。
- 协议类型（SessionEvent/ModuleManifest）serde 黄金测试已立；speak 契约黄金测试 M1a 已立；
  kernel-api 有意耦合 axum/sqlx/utoipa 已声明（M1d 加 utoipa，5 REST-facing 类型 ToSchema）。
- kernel 回合循环 + llm-openai + tts-qwen-audio M1a 已实现；M1b 绑定解析转 DB（加密）；
  **M1c 会话/音频持久化（DbSessions/FsAudio）+ 认证落地**（dev-debt T1/T2 回归）；教学模块 M2+。
- **前端 M1d 完成**：Svelte 5 SPA（web/core 框架无关 + web/shell），Dockerfile node 阶段构建镜像含界面；hash 路由静态伺服。
- token 估算为字符启发式（roadmap 换 tokenizer）。
- 部署 M1b 含完整 entrypoint（自适应降权/卷初始化/主密钥/迁移备份）+ provider/binding/config REST + OpenAPI；
  **M1c 用户级 REST release 开放（cookie 会话认证，未认证 401）**。
- M0/M1b/M1c/M1d 人工残留验收均已通过：SELinux :Z 正例/负例 + quadlet（Fedora 44）、docker 含 compose（真实 Debian 13）；
  M1d 完整产品流 + TTS 音质 + 词级高亮 live/回放 + 移动功能（布局优化延期）+ 部署矩阵 pr-18 重 sign-off。