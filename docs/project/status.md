# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-21T00:10+08:00
- 当前里程碑: **M2 P1 合并验收闭环（PR #25 → main 27c5720，sign-off 全过）→ P2 待启动（新 agent 交接）**
- 主线: M0（51d16c3）→ M1 前置（bf952a3/f8ff64c）→ 蓝图（bde1ba8）→ M1a（20efa24）→ M1b（9d9c907）→ M1c（3c4dc0b）→ hardening（fa59c4e/a2bdf17）→ 验收（9d15342）→ M1d 设计（bf3a413）→ M1d 实施+审计修复（PR #18）→ M1d 合并（6a64852）→ 漂移清扫（4a0fd7b）→ M2 设计定案（1614）→ M2 审计修订合并（b1a19b8）→ 代码修复合并（234a95f）→ 审计闭环 → P1 前置修复合并（9dc5cdf）→ P1 模块切片合并（27c5720）→ **P2 待启动**

## done

- **M0 / M1a / M1b / M1c / M1c-hardening / M1c 多轮审计收尾**：全部合并验收（详见 history INDEX + [roadmap](roadmap.md)）。
- **M1d 完成（Svelte 图形界面里程碑）**：设计 Q1–Q17 需求驱动定案（history 2049）；实施 P0–P8
  （history 2130 P1 音频 / 2230 P2 后端+契约审查 / 2310 P3 web/core / 2345 P4 认证 UI / 0010 P5 设置 / 0040 P6+P7 会话 UI+构建 /
  0130+0210+0240 三轮验收修复 / 0330 完成验收）。
  - web/core 框架无关 TS（音频引擎词级高亮 live/回放 + openapi-fetch 生成类型 CI freshness + SSE + 虚拟路径 + 主题 + i18n zh-CN + 偏好 + 服务层）。
  - web/shell Svelte 5 SPA（认证/管理/引导/设置/对话/会话文件夹目录树 + 主题自定义 + 偏好跨设备）。
  - 后端扩展（0005 虚拟路径文件夹/偏好/会话元数据/音色试听/回放时间戳出口/自助改密）+ **kernel-api utoipa 耦合（5 类型 ToSchema，契约级全量审查）**。
  - Dockerfile 三阶段 node 构建镜像含界面；冒烟 SPA 伺服。
  - 验收：CI 全 8 绿 + 完整产品流 + TTS 音质 + 词级高亮 + 移动功能 + 部署矩阵 pr-18（SELinux/quadlet/docker）sign-off。
  - **延期远期**：Markdown/HTML 渲染、移动端布局优化（**M2 P5 承接**，审计 UX-A2）；布局拖拽永久放弃。
- **M1d 合并前审计修复轮**（history 1240 一审 + 1320 二审，主开发者拍板全量修复+负例同配+复验通过）：
  一审实验确证 A/B/D/M7/M1/G1 修复 + F1–F7 加固 + M2–M6 前端正确性 + Q3/Q4/Q7/Q9/Q12/Q13 需求覆盖补齐 + i18n 清扫 + 文档漂移修复；
  二审修复代码自身审计（R2-1 prefs 队列快照时机真缺陷 + R2-2–R2-7 加固/文档）；
  契约层未变更；kernel 双后端负例 + godoword handler 级测试 + core vitest 54 + 冒烟 M1d 断言。
  **PR #18 squash 合并 main（6a64852），CI 全 8 绿（含 frontend required）**。
- **M1d 落地后文档漂移清扫**（history 1455）：wiki/占位页/模块 docs 陈旧 M1c 表述修正 + models.json fixture 事实对齐 +
  architecture 占位页开发态定位；TTS 多句 usage 漏计疑似经真 key 探针排除（本网关每请求单 sentence，首帧即全量）。
- **M2 设计定案**（history 1614，需求驱动六轮 R1–R6）：teach-assess + teach-gojuon + teach-free 黄金路径；
  混合式课程 + placement（问卷/迷你测/可选 LLM 短测）+ 全量再生重规划 + 全假名 ~104 + 学习仪表盘/模块路由/懒加载 chunk；
  用量看板 + 真正再生 + M1c 一致性检查入 MVP。蓝图见 roadmap。
- **M2 第三方审计 + 修订定案**（history 1721，PR #21 修订轮）：两轮审计（全量代码 + PR #21 三视角）；主开发者裁决——
  UX-A1 gojuon 无方案默认节奏直接学；UX-A2 移动适配入 M2 P5；**UX-A3 静态音频入仓库作废**（DashScope 输出权利不满足 AGPL，
  阻塞项以规避关闭）→**用户侧生成**（每用户自己 key/音色，懒生成+可选一键全量，**白名单 cached-synthesis 回归 M2** +
  持久资产区 /data/assets 豁免淘汰，一次生成永久使用）；**用户级资产备份设计定案**（ZIP 不含 key/恢复向导入/安全硬要求，
  **实施 M2 后**）。审计设计修正 B1–B9（difficulty 契约形状新增诚实标注/DELETE 不发事件/call_log.module_id 入 0006/
  session_id vs for_user P1 必决/outbox 消费守护/重规划锁不跨 LLM 持写事务/YAML crate/迁移哨兵涟漪/mount 单路径）；
  漂移清扫 5 处；代码发现 F1（delete_folder 冲突 500，实验确证）+ O 系列→**另开代码修复 PR**。
- **M2 设计审计通过合并**（PR #21 squash → main b1a19b8）：CI 全 8 绿 + R4 main 冒烟重测通过；主开发者复核「通过」。
- **M2 审计代码修复轮**（history 1802，独立 PR）：F1 delete_folder 上移冲突预检（与 rename 对称 400 + 目标先删释放路径）+
  O2 计量 warn / O3 TTS cancel 记 cancelled / O4 测试端点 adapter 匹配 / O5 binding 枚举校验 / O6 遗留无守卫面 doc(hidden) /
  O7 delete 原语标注 / O8 move rows 校验 / O9 speak rate 钳位+文本上限 / O10 前端 UX 四项（⏹ 真停止/近底自动滚动/暗色横幅/删除先确认）；
  负例同配（双后端 F1 + turn cancelled + handler 级匹配/枚举 + speak 单测）；O1/B2/O11/O12 归属 M2 P6/P2/仪表盘/M4 不动。
- **PR #22 人工 sign-off 轮**（history 1913）：⏹ 真停止/删除先确认/暗色横幅 ✔（PC+移动双侧）；
  滚动信号补全（思考/音频词/折叠展开，原遗漏）；**回合超时归用户级**（主开发者裁决：BYOK 成本/耐心权衡同
  on_disconnect 轴；Preferences.turn_timeout_secs 默认 180s 钳位 [30,3600]，非契约变更；硬边界语义不变）；
  OpenAPI 再生（本地服务 dump，实验垃圾已清）。新增残留：滚动复验 + 超时设置 UI 生效。
- **M2 审计验收全闭环**（history 2031）：PR #22 squash → main 234a95f（CI 全 8 绿两轮）；
  主开发者 sign-off 全通过（首轮 3 项 ✔ + 2 项发现修复后复验全过，PC+移动双侧，PR 评论 20261120-20:17 记录）；
  R4 main 冒烟重测通过。**M2 设计审计 + 代码修复审计正式闭环**。
- **P1 前置全量审计 + 修复轮**（history 2130，PR #24 squash → main 9dc5cdf）：全量审计（侧重 #22/#23）无 P0；
  A1 TTS URL 下载状态检查+32MiB 上限 / A2 LLM arguments 报错不静默 Null+空参{}+index 帽 / A3 断线重附（2049 定案落地）/
  A4 播放失效令牌+失败重置 / A5 历史滚底；负例同配；CI 全 8 绿；sign-off 全过（A3 触发方法修订：DevTools Offline
  不切已建立流，改 ss -K/socat 真断 TCP，服务端 detach 语义实验锚定）。B3/B7 定案（Option/INTEGER/noyalib）。
- **M2 P1 风险前置切片合并验收**（history 0010 实现 + 1316 验收交接，PR #25 squash → main 27c5720）：
  注册表（noyalib）+ ModuleContext/ctx 工厂（llm 中介/complete_session/get_or_synthesize）+ 回合模块接线 +
  teach-free（T4 回归）+ 0006（plan_version INTEGER/call_log.module_id/progress 结构化列/completed_at/teach-free 迁移/audio_assets）+
  REST（modules/complete/module_id 校验）+ OpenAPI 再生 + 冒烟扩展。契约级全量审查 + 二审（含 CI 首轮 docs-gate 漏改 3 项补修 +
  预检时序教训落档）。CI 全 8 绿；主开发者 sign-off 两项全过（跨版本迁移升级 pr-24→pr-25 同卷旧数据 intact +
  真 key teach-free persona/speak/高亮感官）；R4 main 冒烟重测。**M2 P1 正式闭环，P2 待新 agent 启动**。

## in-flight

- **P2 待启动**（主开发者裁决：**新 agent 交接**，本会话收工；开工指引见下一动作）。

## blocked / 待拍板决策

- T4 已决议（teach-free 轻量模块，history 1614）；角色音色档案实体推迟（无 M2 消费者，M2+ 方向）；
  白名单缓存**回归 M2**（用户侧标准音频消费者，history 1721）；DashScope 条款阻塞**已关闭**（静态入仓库作废，用户侧生成规避）。

## 下一动作

- **启动 P2 kernel 基底**（新 agent 交接，主开发者已裁决；预计无 kernel-api 契约变更——consumes_events 早在
  ModuleManifest，扇出/幂等/守护/DELETE 翻转/迁移 runner 皆 kernel 实现层；若触 trait 则触发契约级全量审查）。
  范围：0007 其余基底 + outbox 扇出（consumes_events 订阅，现 dispatch_once 按 ev.module_id 串行路由须改扇出）+
  event_consumptions per-subscriber 幂等表（A5）+ 消费守护（B5 有界并发/per-event spawn/超时/重试上限/死信）+
  **DELETE 不发事件语义翻转（B2）**+ 模块迁移 runner（mod_<id>_NNNN，注册序追加）。
  **新 agent 开工指引**：①读 AGENTS.md → 本文件 → roadmap M2 蓝图 → history 1614/1721/2130/0010/1316 →
  kernel docs（README+storage）+ journal.rs/migrate.rs/registry.rs（心智模型）；②查 git log + 开放 PR 交叉验证
  （无 gh 须显式写「开放 PR 未核验」，审计 D9）；③P2 首任务建议：event_consumptions 表 + 扇出订阅骨架，
  再消费守护（B5），再 DELETE 翻转（B2，**改 journal::end_session + 更新 tests/journal.rs
  end_session_atomicity/end_session_concurrent_unique**），再模块迁移 runner；
  ④测试锚定：backends() 双后端 + 两用户越权负例 + **幂等/毒丸重试上限/并发负例（B5）+ DELETE vs complete 事件语义负例（B2）**+
  迁移幂等/回滚；可靠性声称须失败路径测试（conventions，不能只测 happy path）；
  ⑤环境：2H2G jobs≤2 重构建走 CI（devenv.md），本机 PG 测试容器 gw-pg 已停（按需 podman start gw-pg，TEST_PG_URL 见 devenv），
  真 key 仅 env；⑥status 逐 phase 维护，**提交后**跑 gate --local 预检（漏改按已提交 diff 判定，提交前跑假绿，P1 教训），
  conventional commits（subject 中文）。
- M2 技术接缝（设计已定，P1/P2 落地）：kernel 模块注册表/ModuleContext 实现未立（回合循环现硬编码默认教师）；
  outbox 跨模块订阅（consumes_events）+ event_consumptions 幂等（A5）+ 消费守护（B5 有界并发/毒丸上限）；
  SessionEnded 完成≠删除语义（complete_session 发事件 + sessions.completed_at；**DELETE 不发事件**，B2）；
  ModuleContext LLM 接缝（extractor/assessor）+ cached-synthesis 音频服务 + ctx 工厂注入 + ModuleKind::Kernel；
  progress_events 补 outcome/item_ref/difficulty 列 + **call_log 补 module_id**（0006，B1/B4）；模块迁移版本键 mod_<id>_NNNN 命名空间。
  详见 history 1614/1721 / roadmap 蓝图。
- 用户侧音频机制（P4）：白名单合成服务 + 持久资产区 /data/assets + 资产登记表 + 懒生成/一键全量 + 降级（UX-A3）。
- 用户级资产备份：设计已定案（roadmap 专节），实施 M2 后（M3 前或独立小里程碑）。

**文档精炼原则**（history 2127）：加固/实现沉淀测试/code/history，live 文档只记当前事实；status 只留开放项；产品本质保持显著（README 愿景权威清单）。

## 开发态临时取舍登记（dev-debt，须回归）

| # | 临时取舍 | 引入 | 回归于 | 状态 |
|---|---|---|---|---|
| T1 | env dev key 旁路 + 固定 dev user | M1a | M1c | **已回归** |
| T2 | 内存 impl 作生产默认 | M1a | M1b/M1c | **已回归** |
| T3 | 容器 root 运行 | M1a | M1b | **已回归** |
| T4 | 无模块默认教师模式 | M1a | M2 P1 | **已回归**（teach-free 落地，persona 移入 + 数据迁移，history 1614/0010） |

## 近期关键上下文（M2 交接用）

- **前端架构**：web/core 框架无关 TS（领域逻辑不进组件，shell 调 core 服务层 api/services.ts）+ web/shell Svelte 5（runes 自动检测兼容 legacy lib，
  svelte-spa-router hash 路由）。OpenAPI→openapi-typescript 生成 schema.d.ts 提交 + CI freshness diff（改后端 REST 须再生 openapi.json+schema.d.ts）。
  音频引擎 AudioWorklet（worklet ?worker&url 自包含打包）；**Svelte 5 $state 数组对象须取代理引用 mutate**；用户偏好加载绑定登录态。
- **虚拟路径文件夹**：sessions.path（NFC 归一化 + 固定上限 深度≤8/段≤64/全路径≤1024，协议级常量）；session_folders 登记表（UNIQUE(user,module,path)，
  module 哨兵 default chroot）；前缀匹配 Rust 侧（不用 LIKE）；改名/移动子树批量改写单事务；删非空上移一层绝不删会话。
- **存储**：共享 SQL 守双兼容 SQL 子集；PG INTEGER↔i32 / COUNT(*)↔i64；VACUUM INTO raw_sql+字面量。模块迁移 mod_<id>_* 表 + 注册序追加。
- **契约**：kernel-api 有意耦合 axum/sqlx/utoipa（升级视为契约变更→全量审查）；Store enum 双驱动薄分发宏；speak 契约（SPEAK_TOOL_NAME/SpeakArgs/speak_tool_spec）。
- **认证/授权**：cookie godoword_session（HttpOnly/Lax）+ 变更 X-Requested-With；**401 专属用户会话**（上游错误 502）；对象级归属越权 404。
- **API 实测**：LLM OpenAI 兼容 /compatible-mode/v1（reasoning_content/tool_calls 按 index 组装）；TTS DashScope 原生（base_url=host 根，X-DashScope-SSE，
  sentence 事件 base64 PCM + 词级时间戳跨帧去重，音色 longanlingxin/longanlufeng）。内置默认预设 qwen-llm/qwen-tts。
- 本机 2H2G：重构建走 CI，本机 cargo check/test jobs≤2；npm 慢前端构建优先 CI；cargo-deny 本机未装以 CI 为准；GitHub 网络抖动容忍重试；
  本机 PG 测试用内网镜像 uhub.service.ucloud.cn/hinnyuu/postgres:16（devenv.md，不入仓库）。
- 测试 key 仅经 env（/root/.config/agents/shared/env，QWEN_TOKEN_PLAN_KEY），严禁入仓库/文档/日志；CI 永不用真 key。
- 治理：人工残留清单纪律（history 2302，逐项命令+具体 tag+标准+清理，PR 残留章节代码块格式）；治理克制（最稀缺的是真实实现）。
- **审计修复轮关键事实**（history 1240）：文件夹写操作统一 write_tx（祖先链+改写+删除同事务）；rename 冲突预检+上限重校验；
  改密错旧密 403（401 专属会话）；preview 500 字符/60s/Usage 计量；适配器 connect 10s/read 30s；
  前端树=登记表∪会话路径；auto_play 队列靠引擎 onEnded（pushedMs+markComplete）；prefs 串行队列+清残留变量；
  core session/ux.ts（routeError/parseUsage）；frontend 已入 main required checks + image needs。
- **M2 P1 已落事实**（history 0010/1316，P2 接缝）：模块注册表（kernel/registry.rs，noyalib，manifest.consumes_events 已就绪供扇出）+
  KernelCtxFactory/ModuleContext（llm 中介/complete_session/get_or_synthesize）+ 回合模块接线 + teach-free（persona 权威，feature default-on）+
  0006（plan_version INTEGER/completed_at/call_log.module_id/progress 结构化列/teach-free 迁移/audio_assets）+
  REST（modules/complete/module_id 校验/binding 注册表校验）+ 持久资产区 /data/assets。**journal.rs dispatch_once 现串行按
  ev.module_id 路由（main.rs handlers 空 HashMap 骨架）→ P2 改扇出**；end_session 现 DELETE+事件（B2 翻转点）；
  complete_session 已发事件（完成≠删除）；migrate.rs run_migrations 双后端 runner（模块迁移 mod_<id>_NNNN 注册序追加，P2）。
- **M2 设计关键事实**（history 1614 + 审计修订 1721）：三模块 teach-assess/teach-gojuon/teach-free；混合式课程双通道进度（操练 ExerciseOutcome + 对话提取报告）；
  一课三段式（引入→操练→收尾，完成 session-ended≠删除，**DELETE 不发事件**）；全量再生重规划 version+1（阈值 ≥20 事件/≥3 报告 + 手动，
  并发锁**不跨 LLM 持写事务**，失败保留旧版）；全假名 ~104 数据驱动 + **用户侧标准音频**（白名单 get_or_synthesize + 持久资产区 /data/assets
  豁免淘汰，懒生成+一键全量，作用域 provider 身份+voice+model，降级不阻塞课时）；gojuon 无方案默认节奏（UX-A1）；
  前端运行时懒加载 chunk（core 共享 chunk，mount 单路径）+ **移动适配入 P5**（UX-A2）；
  契约新增 ModuleContext.llm/complete_session/cached-synthesis + ModuleContextFactory + ModuleKind::Kernel + ProgressEvent difficulty（形状新增）；
  0006 含 call_log.module_id（看板归因）；角色音色档案推迟 M2+；**用户级备份设计定案实施 M2 后**（ZIP 不含 key/恢复向）。