# 路线图

> 本文档记录项目目标序列与当前差异。里程碑决策见 [history](../../history/)。

## 里程碑

| 里程碑 | 内容 | 版本 | 状态 |
|---|---|---|---|
| M0 | 脚手架 + CI + 门禁 + 首批文档 + kernel-api 契约 + 最小 hello 镜像 + 容器冒烟测试（CI 集成） | — | 完成 |
| M1 | kernel 完整实现 + auth-local + llm-openai + tts-qwen-audio + 设置 UI（BYOK 可配 + 测试按钮）+ 管理员重置 + schema 前瞻（tokens/users.email）+ Svelte 前端骨架。**拆为 M1a–M1d 四子里程碑**（见下） | — | 完成（M1a/M1b/M1c/M1d 全部完成） |
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
音频缓存走文件系统 `/data/cache`（随机 AudioId + sidecar 时间戳 JSON，可再生可丢弃不入库，TTL+容量上限有界）。
**默认新鲜合成**（每次新合成存供回放）；复用由模块**显式白名单**声明稳定 key（M2，取代指纹 hash 推断——
不靠指纹推断同一性，由模块权威声明）+ 静态资产（M2 模块入仓库 canonical 音频，零 TTS）。见 history 音频白名单条目。
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
- M2 一致性检查（多轮审计非阻断观察留档）：summarize 与主回合循环 Finish 处理对称化（usage 计量完整性，
  Finish 后 usage chunk 不漏记）+ outbox 消费侧幂等（A5，模块消费者落地时）+ 淘汰断言助手增强
- 角色扮演横切声音基础设施（地基 persona/binding/voice 原语 + 模块编排 + 内核角色音色档案实体；
  音色克隆归服务商，未来项目级克隆做成独立模块产 TTS API）——M2+ 方向，见 history 角色扮演条目
- PWA / 移动端增强
- 移动端布局优化（M1d 移动视口功能可用但侧栏固定宽挤压对话区；侧栏响应式折叠/抽屉化专项调整，主开发者认可早期测试后延）
- 对话 Markdown/HTML 渲染（marked/markdown-it + DOMPurify sanitization 防 XSS，M1d 纯文本 pre-wrap，未来与 HTML 支持一起做）
- E2E 自动化测试（当前单测 + fixture + 人工验收）
- mid-run 回放缓冲（若需求出现）
- 会话自动归档、LLM 生成会话标题
- 信封加密（per-user data key）、主密钥轮换工具
- RP-initiated logout（OIDC 会话联动）

## 当前差异（M1 全部完成 M1a–M1d，M2 待启动）

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