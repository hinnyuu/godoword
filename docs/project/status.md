# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-17T23:47+08:00
- 当前里程碑: **M1b 已合并（9d9c907）→ M1c 待启动（交接新 agent）**
- 主线: M0（51d16c3）→ M1 前置 C（bf952a3）→ 前置小 PR（f8ff64c）→ 蓝图（bde1ba8）→ M1a（20efa24）→ **M1b（9d9c907）**

## done

- M0：脚手架 + 四类文档体系 + 关系图谱门禁 + CI（含容器冒烟）+ kernel-api 契约 + hello 服务；
  人工验收全过（Fedora 44 SELinux/quadlet、真实 Debian 13 docker 含 compose）。
- M1 前置（bf952a3）：存储契约 C 方案 + 纪律➀CI 真测双后端（storage required check）+ 纪律➁方言点设计掉；
  serde 黄金测试；axum/sqlx 耦合声明；R4 same-tree+main 重测；契约级全量审查制度化；SQLite CVE 风险接受。
- M1 前置小 PR（f8ff64c）：双后端 workspace 级 + gate 纳入 tests/ + gitleaks 校验和 + agent 交接基础设施。
- M1 拆分蓝图（bde1ba8）：M1a–M1d 四子里程碑，垂直切片先行；speak 落契约（决策 A）；内部接缝；
  bindings 哨兵 `*`；dev-bypass feature；开发态回归机制。详见 [roadmap](roadmap.md#m1-拆分蓝图)。
- **M1a 垂直切片（20efa24，PR #5）**：llm-openai + tts-qwen-audio 适配器全实现（fixture 黄金 + 本地真 key 活体）；
  kernel 最小回合循环（内部接缝 + 内存 impl + 默认 persona + 守护 + cancel）；speak 契约入 kernel-api（审查矩阵 history 1828）；
  godoword 会话 REST + SSE + dev-bypass。本地真 key 全链路实证。修 tool_calls 回喂归一化 bug。
- M1a 依赖合规（history 1844）：CDLA-Permissive-2.0 白名单 + chrono unmaintained（RUSTSEC-2026-0543）书面风险接受。
- **M1b 存储基底 + 部署初始化（9d9c907，PR #7 已合并，release 重测冒烟 success，Fedora SELinux/quadlet sign-off）**：
  完整 kernel schema（11 表）+ 自定义双后端迁移 runner（版本表/checksum/迁移前备份）+ 主密钥 AES-256-GCM
  + DbBindingResolver（5 级 fallback + 解密，生产路径）+ providers/bindings 仓储 CRUD + 配置（env>file>默认 + 预设 + 脱敏）
  + 自适应降权 + 幂等 entrypoint + provider/binding/config REST + models/voices 测试端点 + utoipa OpenAPI + 冒烟完整 uid/卷矩阵（PASS=29）。
  **不改 kernel-api 契约**。本地真 key 全链路 + entrypoint 幂等实证。dev-debt **T3 回归**、T2 绑定部分回归。详见 history 2206。
- M1b 治理搭车（history 2302）：**人工残留清单纪律化**（conventions + gates 自查第 9 项）——逐项完整命令（具体 tag）
  + 通过标准 + 清理，覆盖全部残留项。源于 M1b 验收批评。

## in-flight

- 本交接 docs PR（分支 docs/m1b-accepted-m1c-handoff）：status 更新到 M1b 已合并 + M1c 启动规划。
  合并后新 agent 从 main 接续 M1c。

## blocked / 待拍板决策

- 无当前阻塞。M1b 全部设计决策已定案并落地（history 2206，不重议）。
- 远期留 M2 拍板：无模块默认教师模式是否作永久"自由聊天"产品态（dev-debt T4）。

## 下一动作（新 agent 接续 M1c）

1. 读 AGENTS.md → 本文件 → roadmap M1 蓝图 → architecture → kernel-api 契约 → kernel/docs（M1b 事实，含 storage.md）
   → godoword/docs。交叉验证 git log（M1b=9d9c907）+ history INDEX（2206 M1b/2302 清单纪律）。
2. 建分支 feat/m1c-session-auth，按下方 phase 推进，单 PR 多 commit，持续维护本文件。
3. M1c 完成 → CI 绿（含 storage 双后端 + auth 流冒烟）→ 人工残留 sign-off（清单纪律）→ merge。

## M1c 启动规划（交接棒）

**范围**（蓝图 M1c）：会话运行时接基底 + 认证 + 管理（无 UI）。回归 dev-debt **T1**（删除 dev-bypass）、
**T2 余部**（会话/音频内存→DB/文件系统）。无 UI → 无人工视觉验收（完整人工感官验收在 M1d）。

**执行 phase**（建议，实现决策重大者交用户）：
1. DB `SessionRepo`（sessions/messages 表 M1b 已建）+ 文件系统 `AudioCache`（`/data/cache`，key=hash(model+voice+text+params)
   + sidecar 时间戳 JSON）替换内存生产默认；内存 impl 仅留测试。双后端测试（backends() 夹具）。
2. 上下文装配/预算/淘汰（truncate 默认 / summarize opt-in，summarizer role 绑定）+ 淘汰前进度提取 + outbox
   （progress_events 表）+ call_log 计量（所有上游调用）。
3. on_disconnect continue（detach 服务端跑完）+ cancel（M1a 已有 cancel）。
4. 新 crate `auth-local`：argon2 密码哈希 + users/local_credentials/invites/tokens 逻辑 + invite 注册（首用户例外成管理员）
   + cookie（HttpOnly/Secure/SameSite）/CSRF/登录限流 + 管理员重置。
5. godoword 认证中间件（替换 `user_access` 门）+ auth/admin REST（register/login/logout/me + admin reset/invites/users）
   + **删除 dev-bypass（T1）** + 适配器转非 optional + 用户级 REST release 开放。
6. 冒烟 auth 流 curl + 会话持久化 + 文档/history。

**待定点**（实现决策，重大者交用户）：
- argon2 crate（`argon2` + `password-hash`）；cookie/CSRF 方案（axum-extra cookie / tower-sessions / 自建）；认证中间件形态。
- AudioCache 文件系统布局 + 缓存命中回放；summarize 淘汰的 LLM 调用路径。
- outbox 异步提取机制（tokio task；评估模块 M2 消费 progress_events）。
- **尽量不改 kernel-api 契约**（AuthProvider trait 已存在）；若契约变更走全量审查。
- schema 表 M1b 已全建（users/local_credentials/oidc_identities/tokens/invites 等），M1c 填逻辑。

**验收**：CI auth 流 + 会话持久化双后端 + 淘汰/outbox（workspace storage required check）+ 冒烟 auth 流 curl；
本地真 key 完整对话 + 淘汰 + 进度脚本；**无人工视觉**（M1d）。人工残留按清单纪律（完整命令+通过标准+清理）。

## M1b 已定案决策（已落地，详见 history 2206，不重议）

①DbBindingResolver 唯一生产绑定路径，dev-bypass 收窄（固定 dev user + env 种入 DB）保留至 M1c；release 用户级 REST 把守至 M1c。
②迁移备份方案 a（SQLite VACUUM INTO / PG 跳过告警）+ 版本表 + checksum + 内嵌。③自适应降权（rootless 保持 0 / rootful 降属主或 chown 1000）。
④选型 aes-gcm+rand/nix/toml/utoipa，主密钥 raw 32/hex 64。⑤不改 kernel-api 契约，服务放 kernel 内部 + godoword。

## 开发态临时取舍登记（dev-debt，须回归）

| # | 临时取舍 | 引入 | 回归于 | 回归断言 | 状态 |
|---|---|---|---|---|---|
| T1 | env dev key 旁路 + 固定 dev user | M1a | **M1c** | dev-bypass feature 及 GODOWORD_DEV_* 从非测试代码删除 | 待回归 |
| T2 | 内存 impl 作生产默认 | M1a | M1b/**M1c** | 生产默认换 DB/文件系统；内存 impl 仅留测试 | 绑定已回归（M1b）；会话/音频待 M1c |
| T3 | 容器 root 运行（无降权/初始化） | M1a | M1b | 自适应降权 + 幂等 entrypoint 上线 | **已回归（M1b）** |
| T4 | 无模块默认教师模式 | M1a | M2 决议 | M2 定永久产品态或退役 | 待 M2 |

机制：dev-bypass feature release 永不启用 + 代码 `// DEV-DEBT: T<n>` 标记 + 每里程碑验收核对逾期阻塞。
永久架构（接缝 trait + 内存测试替身）非债务。详见 [roadmap](roadmap.md#开发态临时取舍回归机制)。

## 近期关键上下文

- 存储：共享 SQL 守双兼容 SQL 子集；`query!` 仅驱动专属，共享用运行时 query_as + 薄分发宏（execute_both! 等）。
  **双兼容类型映射**：PG INTEGER↔i32（非 i64）；COUNT(*)↔i64；SQLite VACUUM INTO 须 raw_sql + 字面量文件名（详见 kernel/docs/storage.md）。
- 契约级变更（trait/Store/协议类型/speak 规格）强制全量 docs+wiki 语义审查 + 核对矩阵。
- kernel 内部接缝（SessionRepo/BindingResolver/AudioCache）已立（M1a）；**M1b BindingResolver 转 DB**；
  SessionRepo/AudioCache DB/文件系统 impl M1c，回合循环不重写。
- kernel M1b 模块：storage（pool+ping）/migrate（runner）/crypto（MasterKey）/repo（providers/bindings CRUD）/binding（DbBindingResolver）/config。
  godoword：init（降权+entrypoint）/settings（provider/binding REST）/openapi/rest（user_access 门 M1c 替换）。
- speak 契约（SPEAK_TOOL_NAME/SpeakArgs/speak_tool_spec）已在 kernel-api；kernel 运行时 prepare_speak 执行。
- API 实测事实：LLM OpenAI 兼容 /compatible-mode/v1（reasoning_content/tool_calls 按 index 组装，回喂须 function 形 + arguments 字符串）；
  TTS DashScope 原生 SpeechSynthesizer（X-DashScope-SSE: enable，sentence 事件，base64 PCM，词级时间戳跨帧去重，音色 longanlingxin/longanlufeng）。
- 风险接受：SQLite CVE RUSTSEC-2026-0459 + chrono unmaintained RUSTSEC-2026-0543（均用户认可，roadmap 跟踪，deny.toml 注释）。
- 本机 2H2G：重构建走 CI，本机 cargo check/test jobs≤2（dev 二进制构建 jobs=1 可行）；cargo-deny 本机未装（以 CI 为准）；
  GitHub 网络抖动容忍重试；**本机 PG 测试用内网镜像库 uhub.service.ucloud.cn/hinnyuu/postgres:16**（devenv.md，不入仓库）。
- 治理：人工残留清单纪律（history 2302）；治理克制——当前最稀缺的是真实 kernel 实现，非更完善治理（外部评审共识）。