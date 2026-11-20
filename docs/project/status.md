# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-20T16:14+08:00
- 当前里程碑: **M2 设计定案（history 1614，六轮 R1–R6）→ 待主开发者转第三方审计 → 审计反馈后 P1 风险前置切片**
- 主线: M0（51d16c3）→ M1 前置（bf952a3/f8ff64c）→ 蓝图（bde1ba8）→ M1a（20efa24）→ M1b（9d9c907）→ M1c（3c4dc0b）→ hardening（fa59c4e/a2bdf17）→ 验收（9d15342）→ M1d 设计（bf3a413）→ M1d 实施+审计修复（PR #18）→ M1d 合并（6a64852）→ 漂移清扫（4a0fd7b）→ **M2 设计定案（待审计）**

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
  - **延期远期**：Markdown/HTML 渲染、移动端布局优化；布局拖拽永久放弃。
- **M1d 合并前审计修复轮**（history 1240 一审 + 1320 二审，主开发者拍板全量修复+负例同配+复验通过）：
  一审实验确证 A/B/D/M7/M1/G1 修复 + F1–F7 加固 + M2–M6 前端正确性 + Q3/Q4/Q7/Q9/Q12/Q13 需求覆盖补齐 + i18n 清扫 + 文档漂移修复；
  二审修复代码自身审计（R2-1 prefs 队列快照时机真缺陷 + R2-2–R2-7 加固/文档）；
  契约层未变更；kernel 双后端负例 + godoword handler 级测试 + core vitest 54 + 冒烟 M1d 断言。
  **PR #18 squash 合并 main（6a64852），CI 全 8 绿（含 frontend required）**。
- **M1d 落地后文档漂移清扫**（history 1455）：wiki/占位页/模块 docs 陈旧 M1c 表述修正 + models.json fixture 事实对齐 +
  architecture 占位页开发态定位；TTS 多句 usage 漏计疑似经真 key 探针排除（本网关每请求单 sentence，首帧即全量）。
- **M2 设计定案**（history 1614，需求驱动六轮 R1–R6，主开发者初步认可交第三方审计）：teach-assess + teach-gojuon + teach-free 黄金路径；
  混合式课程 + placement（问卷/迷你测/可选 LLM 短测）+ 全量再生重规划 + 全假名 ~104 + 构建期静态音频（裸 PCM）+ 学习仪表盘/模块路由/懒加载 chunk；
  用量看板 + 真正再生 + M1c 一致性检查入 MVP，白名单缓存 + 角色音色档案推迟；有界 kernel-api 契约新增→契约级全量审查。蓝图见 roadmap。

## in-flight

- M2 设计已落档，待主开发者转第三方审计反馈；**审计反馈前不启动 P1 实施**。

## blocked / 待拍板决策

- **第三方审计反馈**（M2 设计，主开发者转交）：审计意见返回后修订设计/启动 P1。
- **DashScope 输出权利条款**（静态资产 P4 前置）：~104 canonical 音频入 AGPL 仓库前须主开发者确认生成音频可再分发。
- T4 已决议（teach-free 轻量模块，history 1614）；白名单缓存 + 角色音色档案实体推迟（无 M2 消费者，M2+ 方向）。

## 下一动作

- **待第三方审计反馈**（M2 设计 history 1614 / roadmap M2 蓝图）：审计意见返回后修订设计 → 启动 **P1 风险前置切片**
  （模块注册表 + ModuleContext 实现 + 回合接线 + teach-free，模块会话端到端；契约新增此 phase 落 + 契约级全量审查 + 二审）。
- M2 技术接缝（设计已定，P1/P2 落地）：kernel 模块注册表/ModuleContext 实现未立（回合循环现硬编码默认教师）；
  outbox 跨模块订阅（consumes_events）+ event_consumptions 幂等（A5）；SessionEnded 完成≠删除语义（complete_session + sessions.completed_at）；
  ModuleContext LLM 接缝（extractor/assessor）+ ctx 工厂注入 + ModuleKind::Kernel；progress_events 补 outcome/item_ref/difficulty 列（0006）；
  模块迁移版本键 mod_<id>_NNNN 命名空间。详见 history 1614 / roadmap 蓝图。
- 静态资产 P4 前置：DashScope 输出权利条款确认 + ~104 音频构建期合成脚本。

**文档精炼原则**（history 2127）：加固/实现沉淀测试/code/history，live 文档只记当前事实；status 只留开放项；产品本质保持显著（README 愿景权威清单）。

## 开发态临时取舍登记（dev-debt，须回归）

| # | 临时取舍 | 引入 | 回归于 | 状态 |
|---|---|---|---|---|
| T1 | env dev key 旁路 + 固定 dev user | M1a | M1c | **已回归** |
| T2 | 内存 impl 作生产默认 | M1a | M1b/M1c | **已回归** |
| T3 | 容器 root 运行 | M1a | M1b | **已回归** |
| T4 | 无模块默认教师模式 | M1a | M2 决议 | **已决议**（teach-free 轻量模块，history 1614） |

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
- **M2 设计关键事实**（history 1614）：三模块 teach-assess/teach-gojuon/teach-free；混合式课程双通道进度（操练 ExerciseOutcome + 对话提取报告）；
  一课三段式（引入→操练→收尾，完成 session-ended≠删除）；全量再生重规划 version+1（阈值 ≥20 事件/≥3 报告 + 手动，并发锁，失败保留旧版）；
  全假名 ~104 数据驱动 + 构建期静态音频（裸 PCM + manifest，longanlingxin，DashScope 条款待确认）；前端运行时懒加载 chunk（core 共享 chunk）；
  契约新增 ModuleContext.llm/complete_session + ModuleContextFactory + ModuleKind::Kernel（→全量审查）；白名单缓存/角色音色档案推迟。