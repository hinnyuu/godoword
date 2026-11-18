# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-18T22:41+08:00
- 当前里程碑: **M1c 二审收尾加固实现中（设计分歧已定案 history 2241；M1d 延后至加固复核完成）**
- 主线: M0（51d16c3）→ M1 前置 C（bf952a3）→ 前置小 PR（f8ff64c）→ 蓝图（bde1ba8）→ M1a（20efa24）→ M1b（9d9c907）→ M1c（3c4dc0b）→ M1c-hardening（fa59c4e）→ 二审交接（7fe8bc3）→ **二审收尾加固（feat/m1c-second-hardening 实现中）**

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
- **M1c-hardening 第三方审计驱动加固（fa59c4e，PR #12 已合并，R4 main 重测 success，SELinux/quadlet/docker sign-off）**：
  独立第三方 agent 审计发现"实现存在 ≠ 授权边界/数据完整性/交接真实性完成"，主开发者定案全部本期修。
  P0 对象级授权（session/audio/binding 全路径校验归属，越权 404）+ binding/provider 同主 + 禁用/重置撤销会话 + 持久化失败关闭；
  P1 continue 公开配置 + 硬超时/TTS cancel/退出屏障（WAL checkpoint）+ 淘汰水位（0003）+ 重置 token 化/IP 限流/SessionEnded/上游重试 + 测试端点计量；
  OpenAPI 修正 + 全状态面 reconcile + **门禁 D6/D7 修复 + 合成回归 6 例** + AGENTS 协议加固。两用户负例 + 多回合不重复 + 重置令牌单次锚定。详见 history 1618。

## in-flight

- **feat/m1c-second-hardening 实现中**：全量审计核实 R1–R9 属实 + 独立发现 A1–A6；设计分歧已定案（history 2241）。
  按 P0→P2 修复，每项配失败/并发负例测试。基线全绿（gate/test_gate 6-6/cargo check/kernel+auth-local 测试）。

## blocked / 待拍板决策

- **M1c 二审遗留（核实属实，待修复，详见 history 2127 + 下方紧凑清单）**：首轮加固 happy-path 导向，
  失败/并发/原子性未系统覆盖。**修复完成 + 并发/失败测试 + CI 接入 + 复核前，不恢复"无阻塞/后端已就绪"声明**。
- M1d 延后至二审收尾加固复核完成（主开发者定案：专门加固先于 M1d，避免在缺陷地基上盖 UI）。
- 远期留 M2：默认教师模式产品态（T4）；角色音色档案实体 + 白名单音频复用（M2）。

## 下一动作（实现序列，设计已定案 history 2241）

设计分歧定案：①事务=驱动专属块（沿用 register，不改 kernel-api）；②R4 计费幂等=新表 summary_spans（0004）+ 修迁移原子性；
③R9=Config.trusted_proxies + XFF 最右不可信跳；④密码=8–128 字符 + 必含字母与数字（建议加特殊字符，区分大小写），
统一 validate_password；⑤本期纳入 A2/A3/A4，A5（outbox 消费幂等）留 M2，A6 可选。

1. P0：R1 重置事务+密码校验+双后端并发仅一次负例 → R7 管理员串行化+并发互禁负例。
2. P0：R2 音频失败语义+故障注入（含 A3 孤儿）→ R3 真 abort+join+try_send/关通道+满 mpsc/shutdown 回归（含 A1 永久 busy）。
3. P0：R4 emit+水位同事务+0004 幂等表（含 A2 迁移原子性）+summarize 多回合/各失败点测试 → R6 delete 原子。
4. P0/P1：R5 OpenAPI 系统性标注（cookieAuth/CSRF/403/404）+$ref 全解析测试（A4）→ R8 test_gate 接 CI。
5. P2：R9 sidecar 升级处理+限流器清理+可信代理。
6. 收尾：全状态面 reconcile + history + CI 绿（含新并发/失败回归）+ R4 重测 + 人工残留 sign-off + 二审复核无新回归
   → 恢复"M1c 收尾完成无阻塞" → M1d 设计讨论。

**文档精炼原则**（history 2127）：加固沉淀测试/code/history，live 文档只记当前事实；status 只留开放项；产品本质保持显著。

## 二审遗留收尾（紧凑清单，修复完成即移除；方案见 history 2127）

**P0（恢复无阻塞前必修）**：全部完成（R1 重置事务+密码策略 / R2 音频失败关闭 / R3 退出 abort+背压 / R4 淘汰原子+计费幂等 / R5 OpenAPI / R7 管理员串行化；各配失败/并发负例，双后端绿）。

**P1（一致性/CI/文档）**：全部完成（R6 SessionEnded 删除原子 / R8 test_gate 接 docs-gate CI / 文档漂移 trivial 项 #14 已修）。

**P2（运维/精化）**：全部完成（R9 旧 sidecar 兼容 404 可再生 / 限流器过期清理+容量上限 / 可信代理 XFF 最右不可信跳，Config trusted_proxies + 部署 wiki）。

**R1–R9 代码修复全部落地**（各配失败/并发负例，双后端绿，详见 history 2241 + 各 fix commit）。剩余为验收闭环（见下）。

**重新签收证据**（二审第七节）：重置并发仅一次 + 空密码拒绝 / 音频写失败无假成功 / 淘汰各失败点不丢不重 + summarize 多回合 /
满 mpsc 超时退出 + shutdown abort/join 后 checkpoint / OpenAPI $ref 全解析 + 认证 CSRF 一致 / 管理员并发互禁不失锁 /
test_gate CI 必跑 / 状态面单一事实 / 修复 PR + R4 绿 + 二审复核无新回归。

## M1c-hardening（已完成，详见 history 1618，不重议）

第三方审计 C1-C10/D1-D9 全部本期修复闭环：对象级授权 + binding/provider 同主 + 禁用/重置撤销会话 + 持久化失败关闭（P0）；
continue 公开配置 + 硬超时/TTS cancel/退出屏障 + 淘汰水位（0003）+ 重置 token 化/IP 限流/SessionEnded/上游重试 + 测试端点计量（P1）；
OpenAPI 修正 + 全状态面 reconcile + 门禁 D6/D7 修复（test_gate.py 回归 6 例）+ AGENTS 协议加固。
两用户负例 + 多回合不重复 + 重置令牌单次锚定；CI 7 checks + R4 + 人工残留 sign-off 全过。

## M1d 启动规划（延后至二审收尾加固复核完成，设计待讨论）

**范围**（蓝图 M1d，有 UI）：Svelte 骨架 + 设置/会话/认证 UI。web/core 框架无关 TS（OpenAPI→TS 客户端 + SSE 客户端
+ AudioWorklet 音频引擎 + 词级高亮）+ web/shell Svelte 5 SPA + i18n zh-CN 首发（en 后置）+ 设置 UI（BYOK provider/binding
+ 测试按钮）+ 会话 UI（SSE 文本/音频播放/词级高亮）+ 认证/管理 UI（注册/登录/邀请/重置令牌/禁用）+ Dockerfile node 构建阶段 + 前端 CI job。

**后端基础就绪但有二审遗留**（M1c + hardening）：REST/SSE/cookie 认证/对象级授权/音频回放/词级时间戳已落地；
**OpenAPI 完整性（R5：CreateSessionReq 注册 + cookieAuth/CSRF/403/404 + $ref 解析）须二审收尾修复后，M1d 方可生成 TS 客户端**。
M1d 不改后端契约（若需契约变更走全量审查）。

**待讨论设计**（重大者交用户）：前端构建工具（Vite）/Svelte 5 runes 模式/web/core 与 web/shell 分层/OpenAPI→TS 生成方案
/SSE 客户端重连/AudioWorklet PCM 排队播放/词级高亮同步/i18n 方案/前端测试（vitest）/Dockerfile 多阶段 node 构建。

**验收**：CI 前端 lint/build/vitest + 冒烟 SPA 伺服；**完整人工感官验收**（视觉交互/TTS 音质/BYOK 配置流/登录流/词级高亮）——
M1d 是有 UI 里程碑，人工视觉验收强制（区别 M1a/b/c 无 UI）。人工残留按清单纪律（完整命令+通过标准+清理）。

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