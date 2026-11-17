# 架构

> 本文档记录当前提交代码的架构事实，以及与 [roadmap](roadmap.md) 的差异。
> 历史决策见 [history](../../history/)，人类向说明见 [wiki](../../wiki/)。

## 定位

godoword 是私有部署的外语学习 Web 应用（日语优先）。核心能力由 LLM 提供：LLM 担任语言教师，
结合 TTS 完成听说教学。完全 BYOK——用户自填 API key，服务端仅代理调用，key 不下发浏览器。
模块是规范 LLM 的框架，并为 LLM 提供使用 TTS 等能力的标准工具。

## 三层架构

```
kernel-api（契约 crate：trait + 类型，无实现）
   ↑              ↑                ↑
适配器          教学模块         kernel（实现 ModuleContext 等服务）
llm-openai     teach-gojuon         ↑
tts-qwen-audio teach-dialogue   godoword（组合根 bin，feature 注册各模块）
auth-local     teach-assess
```

- **内核 kernel**：配置、存储、加密、会话、agent 运行时、模块注册、事件 outbox、用量计量。
  拆分为 `kernel-api`（薄契约）与 `kernel`（实现）；适配器与教学模块只依赖 `kernel-api`，
  因此 kernel 内部也可整体重写。
- **适配器 adapter**：实现 `LlmProvider` / `TtsProvider` / `AuthProvider`。每款协议一个 crate
  （OpenAI 兼容 LLM、qwen-audio TTS 各独立；TTS 不走 OpenAI 兼容，见 history）。
- **教学模块 teaching module**：实现 `TeachingModule`，自带路由、前端清单、专属工具、进度事件输出。
  `teach-assess` 另实现 `PlanProvider`，是教学核心。

**依赖纪律**：模块互不 import，只依赖 kernel-api；跨模块数据只经持久事件 outbox 与契约接口。
编译期模块化（cargo feature 开关）是开发者维护边界；发行镜像含全部启用模块，用户经 UI 偏好隐藏。

## 模块清单

每 crate 一份 `module.yaml`（id/kind/requires 或 provides/session_params/frontend/impact/cross_module），
kernel 注册表反序列化，docs-gate 读取其 impact 规则构成[关系图谱](gates.md)。
模块目录自治：`crates/<name>/{src,docs/,wiki/,module.yaml,migrations/}`。

## agent 运行时与会话协议

kernel 独占回合循环（教学模块不碰上游 HTTP 与 key）：

```
解析绑定 → 组装系统提示词（模块 persona + plan.teacher_prompt 钉住版本 + 约束 + 工具规格）
→ 上下文装配（系统常驻 + 历史窗口 + 新消息）→ 流式 LLM → 发 delta → 收集 tool_calls
→ 执行（speak → TTS 流 / 模块工具 → TeachingModule）→ 回喂 → 循环至 stop / 守护上限
→ 持久化消息 + usage + 进度事件
```

传输：REST 管理会话 + `POST /api/v1/sessions/{id}/messages` 响应即该回合 SSE 事件流。
事件类型：run-start / text-delta / reasoning-delta / tool-call / tool-result / audio-start /
audio-chunk / audio-end / word-timestamps / usage / error / run-end。
断开行为用户可配（cancel 默认省费 / continue 服务端跑完）；显式 cancel 始终可用；v1 无 mid-run 回放缓冲。
音频管线：流式 TTS 请求 PCM/24000 → audio-chunk 裸 PCM → 前端 AudioWorklet 排队播放，
词级时间戳驱动高亮；缓存回放走完整文件 + 时间戳 JSON。

上下文预算：每会话 token 预算（模块可配），淘汰策略 truncate（默认）/ summarize（opt-in）。
淘汰与会话结束发持久事件，评估模块异步提取教学进度（提取报告）。守护：工具迭代上限、回合超时、
上游重试策略、每会话单活跃回合锁。

完整 trait 签名见 [kernel-api 文档](../../crates/kernel-api/docs/)。

## 绑定与用量

```
providers(id, user_id, adapter_id, label, base_url, key_enc)   一实例一 key
bindings(user_id, module_id, capability, role, provider_id, model, params_json)
```

解析顺序：`(user,module,cap,role) → (user,module,cap,default) → (user,*,cap,role)
→ (user,*,cap,default) → 未配置`（`*` 为全模块默认哨兵，取代 NULL——使 UNIQUE(user,module,cap,role)
在两后端皆正常约束，M1b 已落地）。role ∈ default/summarizer/extractor/assessor，
支持教师用强模型、提取/评估用便宜模型。所有上游调用经 kernel 记入 call_log（用量看板、预算警告）。
音频**默认新鲜合成**（每次新合成，随机 AudioId 存供回放，缓存 TTL+容量上限有界）；复用由模块**显式白名单**
声明稳定 key（M2，取代指纹 hash 推断——同一性由模块权威声明而非指纹推断）+ 静态资产（M2 模块入仓库）。
角色扮演为**横切声音基础设施**（地基 persona/binding/voice 原语 + 模块编排；M2 内核角色音色档案实体；
音色克隆归服务商，未来项目级克隆做成独立模块产 TTS API）。见 history 音频白名单/角色扮演条目。

## 数据与存储

默认 SQLite（WAL，`/data/db`），支持外接 PostgreSQL；MySQL 缓议（见 roadmap）。
存储契约为**双具体驱动 enum** `Store { Sqlite(SqlitePool), Pg(PgPool) }`（sqlx Any 已证伪：
类型集封闭，不支持 uuid/chrono/json，见 history）。模块经 `ModuleContext.store()` 拿 `Store`，
用薄分发宏（`execute_both!` / `fetch_one_both!` / `fetch_optional_both!` / `fetch_all_both!`）
+ 运行时 `query_as` 跑**双兼容 SQL 子集**；编译期 `query!` 宏仅用于驱动专属代码（单宏 pin 死单驱动）。
表前缀约定 `mod_<id>_*`。薄 enum 分发，不隐藏方言。

**双兼容 SQL 子集约定**（纪律➁，把方言点设计掉）：uuid 主键（灭自增方言）、应用侧时间戳
（灭 DB 默认值方言）、JSON 整取回 Rust 不在 SQL 过滤（灭 `->>`/`json_extract` 分裂）；
单一 PG 风格类型名迁移集靠 SQLite 类型亲和性双后端通用。残余真方言点收口 kernel helper。

**CI 真测双后端**（纪律➀）：`storage` job 以 postgres service + SQLite 对两后端跑迁移幂等 +
uuid/chrono/json/bool round-trip + CRUD，列为 main 分支保护 required check；模块写 PG 专属 SQL
则 SQLite 测试红，反之亦然，方言漂移双向捕获，杜绝静默上线。

kernel 拥有 users/sessions/messages/providers/bindings/call_log/progress_events/tokens/invites 等表；
教学模块拥有 `mod_<id>_*` 私有表 + 自有迁移。迁移顺序 kernel → 模块注册序，迁移前自动备份。

## 安全

- 主密钥：env `GODOWORD_MASTER_KEY` 覆盖，否则自动生成 `/data/keys/master.key`（0600）；
  用户 key 以 AES-256-GCM 静态加密（每记录独立 nonce）。威胁模型：防静态泄露，不防运行中服务器被攻破。
- 认证：数据挂稳定内部 user_id；auth-local（argon2）先行，auth-oidc（authentik）M4；
  注册默认 invite（首用户例外成为管理员）；管理员不可读用户 key。
- 会话 cookie HttpOnly/Secure/SameSite；变更请求 CSRF 防御；登录限流。
- 日志默认仅元数据，无内容无 key；gitleaks + 禁入路径门禁。

## 部署

单容器 all-in-one，镜像无状态，一切持久状态入 `/data`（db/keys/cache）。entrypoint 全幂等
（建目录 → 主密钥 → 迁移 → 启动），容器误删重建零丢失。podman rootless 优先，兼容 docker。
自适应降权：应用初始化读 `/data` 属主 + `/proc/self/uid_map`，以属主映射的容器 uid 运行，
bind 目录文件永远归宿主用户。默认双栈监听 `[::]:8080`（兼容 IPv4，IPv6 不可用回退 `0.0.0.0`，
`GODOWORD_LISTEN` 可覆盖）；`/healthz` `/readyz`，优雅退出排空 SSE + WAL checkpoint。
podman bind mount 源目录须自建（与 docker 不同，podman 不自动创建）。
生产文档给 quadlet（podman≥4.4）+ 反代 TLS；快速上手给 podman run + docker-compose。

## 当前状态与 roadmap 差异（M1b）

- 已落地：文档体系、关系图谱门禁、CI 流水线、kernel-api 契约 trait（含 speak 工具契约）、
  kernel 回合循环（内部接缝 + 内存会话/音频 impl + 默认教师 persona + 守护）、llm-openai/tts-qwen-audio 适配器、
  组合根会话 REST + SSE。**M1b 存储基底**：Store pool 初始化（SQLite WAL+外键 / PG 外接）、自定义双后端迁移 runner
  （版本表 + checksum + 迁移前备份）、完整 kernel schema（11 表）、主密钥 AES-256-GCM 加密、DbBindingResolver
  （5 级 fallback + 解密，生产路径）、配置加载（env>file>默认 + 预设 + 脱敏视图）、自适应降权 + 幂等 entrypoint。
  M1a 垂直切片经本地真 key 全链路验证；M1b entrypoint 经本地实证（rootful 降权 + 幂等重启 + 密钥复用 + 备份）。
- 未实现：会话/音频持久化（DB/文件系统 impl，M1c）、认证/用量计量落地（M1c）、provider/binding CRUD REST（M1b phase 6）、
  Svelte 前端（M1d）、教学模块（M2+）。会话/音频仍内存态 + dev-bypass 开发旁路（release 不含，用户级 REST 待 M1c 认证）。
- 已知简化：token 估算用字符启发式（roadmap 换 tokenizer）；web 为静态占位（M1d 起 Svelte）；
  上下文当前全量历史（预算/淘汰 M1c）；on_disconnect 当前仅 cancel（continue M1c）。