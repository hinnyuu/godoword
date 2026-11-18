# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-18T03:29+08:00
- 当前里程碑: **M1c 实现进行中（feat/m1c-session-auth）——phase 1 完成（会话/音频持久化基底）**
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

- M1c 实现分支 feat/m1c-session-auth（设计已定案 history 0329，不重议）。
  **phase 1 完成**：SessionRepo 接缝扩展（SessionMeta/OnDisconnect + create 携带 meta + meta()/history_with_ids）；
  DbSessions（sessions/messages 双兼容 SQL，seq 单活跃锁内分配，级联删）；FsAudio（/data/cache 原子写 + sidecar
  + TTL/容量有界周期清理）；main/AppState/REST 转 trait 对象。内存 impl 降为测试替身（T2 余部回归）。
  **phase 2 完成**：context（字符启发式 token 估算 + truncate 装配）；runtime 每会话 params + summarize 淘汰
  （Summarizer role + 失败回退）+ 淘汰 span 入 outbox（SessionEvicted，module_id 哨兵 kernel）+ call_log 计量
  （LLM/TTS/summarize，adapter_id 取 trait id）；journal（record_call/emit_progress/pending/dispatch_once）
  + outbox dispatcher 骨架（main 空注册表，M2 消费）。双后端测试全绿（session/journal 独立 PG schema 隔离；
  计量测试用文件库 + ensure_user 避外键违约）+ clippy/fmt 干净。
  **phase 3 完成**：run_turn 执行/投递分离——执行体 spawn tokio task 经有界 mpsc 投递，SSE 订阅 receiver；
  断开（receiver drop）读 on_disconnect：cancel 置标志 / continue 取 detach permit 排空跑完（permit 满降级 cancel）；
  活跃锁归执行体（task 结束释放）；detach semaphore 默认 4（Config max_detached_turns 可配）；cancel_all + 优雅退出接线。
  断开 continue/cancel 测试（current_thread 确定时序）+ clippy/fmt 干净。
  **phase 4 完成**：新 crate auth-local（kind=adapter provides=None，依赖 kernel-api only）——argon2id hash/verify、
  register（首用户事务锁成管理员：SQLite 单写者/PG advisory xact lock；username 唯一；invite 校验用尽/过期）、
  verify_login（禁用拒绝）、会话令牌 create/validate/delete（auth_sessions 表，opaque token sha256）、
  create_invite、list_users/set_user_status/is_admin/admin_reset_password、LocalAuth impl AuthProvider。
  kernel 0002 迁移（users 增 username + UNIQUE 索引 + auth_sessions 表）；storage 测试适配 2 迁移。
  auth-local 双后端测试全绿（独立 PG schema）+ clippy/fmt 干净。下一：phase 5（godoword 认证中间件 + REST + 删 dev-bypass）。

## blocked / 待拍板决策

- 无当前阻塞。M1c 设计决策已定案（history 0329，不重议）；M1b 设计已落地（history 2206）。
- 远期留 M2 拍板：无模块默认教师模式是否作永久"自由聊天"产品态（dev-debt T4）；角色音色档案实体形态（M2）。

## 下一动作（设计落档合并后接续 M1c 实现）

1. 读本文件 M1c 启动规划（设计已定案）→ history 0329 三条目（M1c 决策/音频白名单/角色扮演）+ 触发规则条目
   → roadmap M1 蓝图 → architecture → kernel-api 契约 → kernel/docs → godoword/docs。交叉验证 git log + history INDEX。
2. 建分支 feat/m1c-session-auth，按上方 phase 推进，单 PR 多 commit，持续维护本文件。已定案决策不重议。
3. M1c 完成 → CI 绿（含 storage 双后端 + auth 流冒烟）→ 人工残留 sign-off（清单纪律）→ merge。

## M1c 启动规划（交接棒，设计已定案）

**范围**（蓝图 M1c）：会话运行时接基底 + 认证 + 管理（无 UI）。回归 dev-debt **T1**（删除 dev-bypass）、
**T2 余部**（会话/音频内存→DB/文件系统）。无 UI → 无人工视觉验收（完整人工感官验收在 M1d）。

**设计决策已定案**（详见 history 2026-11-18-0329 三条目 + 触发规则条目，不重议）：
①不改 kernel-api 契约（auth-local 实现 authenticate seam + crate 函数注册/邀请/重置；extra_routes 留 M4 OIDC；
内部接缝 SessionRepo/AudioCache 扩展非契约）。②continue 执行/投递分离（tokio task + 有界 mpsc，活跃锁归 task，
detach semaphore 默认 4 可配、try_acquire 失败降级 cancel）。③音频**默认新鲜合成** + 文件系统回放存储（随机 AudioId）
+ 缓存有界（TTL 30 天/容量 1GB 可配 + 周期清理）；**显式白名单复用 + 静态资产 M2**（取代指纹 hash 推断）。
④认证方案 A（auth_sessions 表 0002 + opaque token cookie，Secure 可配默认 true，SameSite=Lax + 自定义头 CSRF，
登录限流内存计数，首用户事务锁，argon2id，管理员重置 token 化）。⑤全量 summarize（Summarizer role + 失败回退 truncate + mock）。
⑥outbox（kernel 生命周期事件 module_id 哨兵 `kernel`）+ call_log（adapter_id 取 trait id()）+ dispatcher 骨架（M2 消费）。

**执行 phase**（单 PR 多 commit，持续维护本文件）：
1. DB `SessionRepo`（sessions/messages 表 M1b 已建，增读 on_disconnect/params/module_id；seq 单活跃锁内
   COALESCE(MAX(seq),0)+1 i32）+ 文件系统 `FsAudio`（`/data/cache/<id>.pcm` + sidecar，原子写，TTL+容量有界）
   替换内存生产默认（新鲜合成随机 AudioId）；内存 impl 仅留测试。双后端测试（backends() 夹具）。
2. 上下文装配/预算/淘汰（truncate 默认 / summarize opt-in）+ 淘汰前 span 入 outbox（SessionEvicted）
   + call_log 计量（所有上游调用）+ dispatcher 骨架。
3. on_disconnect continue（task + mpsc 执行/投递分离，detach 上限）+ cancel 非回归。
4. 新 crate `auth-local`：argon2id + users/local_credentials/invites/tokens/auth_sessions 逻辑 + invite 注册
   （首用户事务锁成管理员）+ cookie/CSRF/登录限流 + 管理员重置 token 化。schema 0002 增 auth_sessions。
5. godoword 认证中间件（替换 `user_access` 门，per-request AuthUser extractor）+ auth/admin REST
   + **删除 dev-bypass（T1）** + 适配器转非 optional + 用户级 REST release 开放（未认证 401）。
6. 冒烟 auth 流 curl（503→401 + 注册首用户→登录→CRUD→会话）+ 会话持久化 + 文档/history。

**已定案不重议**：契约姿态/continue/音频白名单/认证 A/全量 summarize/outbox 边界（history 0329）。
角色扮演分层（横切基础设施 + 内核角色音色档案实体 M2 + 克隆 provider 侧/独立模块产 TTS API）M2 方向，
M1c 音频机制无关其形态、零返工。schema 0001 不动（checksum 防漂移），0002 增 auth_sessions；角色音色档案实体 M2（0003+）。

**验收**：CI auth 流 + 会话持久化双后端 + 淘汰/outbox（workspace storage required check）+ 冒烟 auth 流 curl；
本地真 key 完整对话（走真认证 register→provider/binding→回合）+ 淘汰 + 进度脚本；**无人工视觉**（M1d）。
人工残留按清单纪律（完整命令+通过标准+清理）。

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
- 全量语义审查触发：契约级变更（trait/Store/协议类型/speak 规格）**或 supersede 蓝图/architecture 假设的设计方向变更**
  （history 0329 触发规则）+ 核对矩阵；设计方向级须趁上下文二审复核。
- **M1c 设计已定案**（history 0329 三条目，不重议）：不改契约 + continue（task+mpsc，detach 上限）+ 音频默认新鲜合成
  + 文件系统回放存储 + 缓存有界（**显式白名单复用 + 静态资产 M2，取代指纹 hash**）+ 认证 A（auth_sessions 0002）
  + 全量 summarize + outbox（module_id 哨兵 `kernel`）/call_log。**角色扮演**横切声音基础设施（地基原语 + 模块编排，
  内核角色音色档案实体 M2，克隆 provider 侧），M1c 音频机制无关其形态、零返工。
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