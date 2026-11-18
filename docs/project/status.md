# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-18T12:48+08:00
- 当前里程碑: **M1c 已合并（3c4dc0b）但第三方审计发现收尾阻塞 → M1c-hardening 加固子里程碑进行中（M1d 暂缓至加固后）**
- 主线: M0（51d16c3）→ M1 前置 C（bf952a3）→ 前置小 PR（f8ff64c）→ 蓝图（bde1ba8）→ M1a（20efa24）→ M1b（9d9c907）→ M1c（3c4dc0b）→ **M1c-hardening（进行中）**

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
- **M1c 设计落档（1b98fa3，PR #9）**：六设计决策 + 音频显式白名单（supersede 蓝图指纹 hash）+ 角色扮演横切基础设施分层
  （history 0329 三条目）；全量审查触发规则扩展（契约级 + 设计方向级 + 二审）；一审 + 二审全量 reconciliation（核对矩阵）。
- **M1c 会话运行时接基底 + 认证 + 管理（3c4dc0b，PR #10 已合并，R4 main 重测 success，SELinux/quadlet/docker sign-off）**：
  DbSessions/FsAudio（会话/音频持久化，T2 回归）+ 上下文预算/truncate+summarize 淘汰 + outbox（SessionEvicted，kernel 哨兵）
  + call_log 计量 + continue 执行/投递分离（task+mpsc，detach 上限）+ auth-local（argon2id + 邀请注册首用户管理员 + cookie 会话
  + CSRF + 限流 + 管理员重置）+ godoword 认证中间件 + auth/admin REST + **删 dev-bypass（T1 回归）**。kernel 0002 迁移
  （users.username + auth_sessions）。**不改 kernel-api 契约**。本地真 key 走真认证全链路 + auth 流端到端实证。详见 history 1129。

## in-flight

- M1c-hardening 加固分支（feat/m1c-hardening）：修复第三方审计（/tmp/godoword_M1c_AGENTS第三方审计意见.md，HEAD 03ff4e3）
  发现的收尾阻塞。全部本期修（用户定案，避免延期漂移）；M1d 讨论暂缓至加固后。
  **H1（P0 C1-C4）完成**：SessionRepo/AudioCache 接缝全路径携带 user_id 对象级授权（DbSessions/MemSessions/FsAudio）；
  runtime 持久化失败关闭（meta/history/append 错误中止，不调计费上游）；binding/provider 同主（create/update/resolve 归属校验）；
  validate_session 联查 active + 禁用/重置撤销会话 + is_admin 须 active + 管理员状态路由（防自禁/防禁最后管理员）。
  两用户负例（repo 层 + REST 层越权 404/跨用户 provider 400/禁用旧 cookie 401）+ 双后端全绿 + clippy/fmt 干净。
  **H2（P1 C5-C9）完成**：C5 create_session 公开 on_disconnect/params；C6 硬超时（timeout 包裹执行体）+ TTS 流 cancel
  + detached 退出屏障（tasks 注册 + shutdown 有界等待 + WAL checkpoint）；C7 淘汰水位（0003 sessions.summary/evicted_up_to，
  滚动摘要 + 水位推进 + outbox 一次，多回合不重复测试）；C8 重置 token 化（tokens 表单次）+ IP 限流维度 + SessionEnded
  + 上游退避重试（LLM/TTS 可重试错误 bounded 2）；C9 测试端点计量。双后端全绿 + clippy/fmt 干净。
  **H3-H6 完成**：OpenAPI 清 503/dev-bypass 改 401 + 403/CSRF + SSE 精确表述；全状态面 reconcile（roadmap 表/README/web/wiki/
  module.yaml/rustdoc/PR 模板清理列）+ **门禁 D6/D7 修复 + test_gate.py 合成回归 6 例**；AGENTS 协议加固（冲突裁决/gh 降级/
  缺陷≠重议/收尾定向核对/不自指）；smoke 补两用户越权 404 + admin 面 403 + 限流 429 断言（C10 声明与脚本一致）。
  本地真 key 全链路 + 加固 REST 验证（continue 公开配置/越权 404/限流 429/重置令牌单次）全过。
  **M1c-hardening 六阶段全落地**，待 CI 绿 + R4 重测 + 人工残留 sign-off → 恢复"M1c 收尾完成无阻塞" → M1d。

## blocked / 待拍板决策

- **M1c 收尾阻塞（第三方审计核实属实，修复中）**：
  - C1【高】session/audio 只验登录不验资源归属（rest.rs 丢弃 AuthUser，DbSessions 按 id 查询）→ 跨用户读/删/写/取消/回放。
  - C2【高】binding 可引用他人 provider 并解密用其 key/额度（create/update/resolve 无归属校验）。
  - C3【高】禁用账户旧 cookie 仍有效（validate_session 不查 status，set_user_status 不撤销会话）+ 无管理员状态路由。
  - C4【高】持久化失败被吞（create_session/append let _ =）→ 假成功 + 未授权上游计费。
  - C5–C10【中高/中】continue 无公开配置入口、cancel/timeout 非硬边界 + 无 WAL checkpoint、淘汰无持久水位（重复摘要/outbox）、
    设计漂移（重置未 token 化/限流无 IP/SessionEnded 未发/上游无重试）、provider_models 未计量、smoke 过度声明（无 429 断言）。
  - D1–D9 文档漂移（roadmap 表自相矛盾/README/web M0/module.yaml/rustdoc/OpenAPI 503 残留/PR 模板）+ **门禁实现缺陷
    （D6 kernel-api 跨模块规则不可达 + crates/*/docs/ 通配恒 false；D7 migration/web 盲区）**。
- 加固设计决策（C7 淘汰水位 schema、C6 退出屏障、C5 continue API、C2 校验层）见下方规划，重大者交用户。
- 远期留 M2：默认教师模式产品态（T4）；角色音色档案实体 + 白名单音频复用（M2）。

## 下一动作（M1c-hardening，M1d 暂缓）

1. 读本文件 M1c-hardening 规划 → 第三方审计意见 → history 0329/1129（已定设计）→ 相关源码。
2. 按 P0（授权/数据完整性）→ P1（核心语义 + OpenAPI + reconcile + 门禁修复）→ P2（AGENTS 协议）推进，单 PR 多 commit，持续维护本文件。
3. 加固完成 → 审计第九节最低证据清单齐备 → CI 绿 + R4 重测 + 人工残留 sign-off → 恢复"M1c 收尾完成无阻塞" → M1d 设计讨论。

## M1c-hardening 规划（加固子里程碑，全部本期修）

**P0 授权/数据完整性（必修 + 两用户负例）**：
- C1 对象级授权：SessionRepo 生产路径携带校验 user_id（exists/meta/history/append/delete）；audio 经持久关联校验归属；
  messages/cancel 先验 session 归属；REST 不再丢弃 AuthUser。负例期望 404/403 不泄露存在性。
- C2 binding/provider 同主：create/update binding 验 provider 归属 + resolve 解密带 user 条件（纵深防御）；跨用户负例确保不解密不调上游。
- C3 禁用/重置撤销访问：validate_session 联查 active；禁用/重置撤销目标 auth_sessions；管理员状态管理 REST（防唯一管理员失管）+ 旧 cookie 负例。
- C4 持久化失败关闭：create/读/append 不吞错；前置不满足不调计费上游；不存在 session/DB 失败/continue 中断负例。

**P1 核心语义（M1d 冻结前）**：
- C5 continue 公开受控配置（create_session 接受 on_disconnect/params）+ detach 饱和降级测试。
- C6 全路径可响应 cancel（含 TTS 流）+ 硬 timeout（包裹上游）+ detached 退出屏障（跟踪 JoinHandle + WAL checkpoint 取舍明示）。
- C7 淘汰持久水位/去重（schema 0003：sessions 滚动摘要 + 淘汰水位）+ 多回合 summarize/truncate 测试（同 span 不重复计费/outbox）。
- C8 对齐已定设计：管理员重置 token 化（tokens 表）+ 限流 IP 维度 + SessionEnded 发射 + 上游重试/退避（本期全修，不延期）。
- C9 统一计量（provider_models 等纳入 record_call；写入失败语义明示）。
- D4 OpenAPI 修正（401/403/CSRF、清 503、SSE「path 入/事件 schema 不入」精确）→ M1d 生成 TS 前提。

**P1 reconcile + 门禁修复**：
- D1/D3/D5 全状态面单一事实（roadmap 里程碑表/README/wiki/user 头注/web 占位页/module.yaml/rustdoc/PR 模板清理命令列 + smoke 描述）。
- D6/D7 门禁修复（项目规则匹配所有变更文件不只 PROJECT_PREFIXES；crates/*/docs/ 逐模块展开 + 前缀匹配；migrations 纳入 coverage/omission/status；web 纳入 status）
  + 合成回归测试（kernel-api 跨模块/migration-only/web-only）。

**P2 AGENTS 协议加固（保持短小）**：冲突裁决（git 主线/源码实现事实/status live/history 为什么，冲突标 blocked）；
里程碑收尾定向全表面核对；gh 降级（gh→Web/REST→显式未核验）；交接 PR 不自指；缺陷≠重议。

**验收**：审计第九节最低证据清单（两用户负例/跨用户 provider 拒绝/禁用旧 cookie 失效/无假成功/continue 公开可用 + detach 降级/
TTS cancel + 硬 timeout + detached 退出/多回合不重复/OpenAPI 一致/smoke 声明与脚本一致/门禁回归/状态面单一事实/CI + R4 重测 + 人工残留可核对记录）。

## M1d 启动规划（加固后讨论，暂缓）

**范围**（蓝图 M1d，有 UI）：Svelte 骨架 + 设置/会话/认证 UI（web/core 框架无关 TS + AudioWorklet + 词级高亮 + i18n zh-CN）
+ 前端 CI + Dockerfile node 构建。**后端就绪声明撤回**：须待 M1c-hardening 修复授权/OpenAPI/continue 语义后重新评估。
M1d 设计讨论在加固完成后进行（避免混杂）；完整人工感官验收强制。

## M1c 已定案决策（已落地，详见 history 0329/1129，不重议）

①不改 kernel-api 契约（内部接缝扩展）。②continue 执行/投递分离（task+mpsc，detach 上限默认 4）。③音频默认新鲜合成
+ 文件系统回放 + 缓存有界；显式白名单复用 + 静态资产 M2（取代指纹 hash）。④认证 A（auth_sessions 0002 + cookie Secure 可配
+ SameSite=Lax + CSRF 自定义头 + 限流 + 首用户事务锁 + argon2id）。⑤全量 summarize（失败回退 truncate）。⑥outbox kernel 哨兵
+ call_log + dispatcher 骨架（M2 消费）。角色扮演横切基础设施（地基原语 + 模块编排 + 内核角色音色档案实体 M2 + 克隆 provider 侧）。

## M1b 已定案决策（已落地，详见 history 2206，不重议）

①DbBindingResolver 唯一生产绑定路径，dev-bypass 收窄（固定 dev user + env 种入 DB）保留至 M1c；release 用户级 REST 把守至 M1c。
②迁移备份方案 a（SQLite VACUUM INTO / PG 跳过告警）+ 版本表 + checksum + 内嵌。③自适应降权（rootless 保持 0 / rootful 降属主或 chown 1000）。
④选型 aes-gcm+rand/nix/toml/utoipa，主密钥 raw 32/hex 64。⑤不改 kernel-api 契约，服务放 kernel 内部 + godoword。

## 开发态临时取舍登记（dev-debt，须回归）

| # | 临时取舍 | 引入 | 回归于 | 回归断言 | 状态 |
|---|---|---|---|---|---|
| T1 | env dev key 旁路 + 固定 dev user | M1a | **M1c** | dev-bypass feature 及 GODOWORD_DEV_* 从非测试代码删除 | **已回归（M1c）** |
| T2 | 内存 impl 作生产默认 | M1a | M1b/**M1c** | 生产默认换 DB/文件系统；内存 impl 仅留测试 | **已回归（M1b 绑定/M1c 会话音频）** |
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