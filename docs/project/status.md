# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-17T18:28+08:00
- 当前里程碑: M1a 完成（待 PR 合并）→ M1b 待启动
- 主线: M0（51d16c3）→ M1 前置 C（bf952a3）→ M1 前置小 PR（f8ff64c）→ M1 蓝图（bde1ba8）→ M1a 完成

## done

- M0：脚手架 + 四类文档体系 + 关系图谱门禁 + CI（含容器冒烟）+ kernel-api 契约 + hello 服务；
  人工验收全过（Fedora 44 SELinux/quadlet、真实 Debian 13 docker 含 compose）。
- M1 前置（bf952a3）：存储契约 C 方案（Store enum 双具体驱动 + 薄分发宏 + 双兼容 SQL 子集）；
  纪律➀ CI 真测双后端（storage required check）+ 纪律➁ 方言点设计掉；serde 黄金测试；
  axum/sqlx 耦合声明；R4 same-tree+main 重测；R5 gate 三修；契约级全量审查制度化（首次执行）；
  SQLite CVE RUSTSEC-2026-0459 书面风险接受。
- M1 前置小 PR（f8ff64c，PR #3 已合并）：双后端 workspace 级 + gate 纳入 tests/ + gitleaks 校验和
  + agent 交接基础设施（AGENTS.md + status.md + gate 新鲜度）。release 对 main 镜像重测冒烟 success。
- **M1 拆分蓝图定案**（详见 [roadmap](roadmap.md#m1-拆分蓝图)）：拆 M1a–M1d 四子里程碑，垂直切片先行；
  speak 规格落 kernel-api 契约（决策 A：SpeakArgs + speak_tool_spec + 黄金测试）；kernel 内部接缝
  （SessionRepo/BindingResolver/AudioCache）；bindings 默认哨兵 `*`；dev 旁路装 dev-bypass feature；
  开发态临时取舍回归机制。

## in-flight

- **M1a 完成**（分支 feat/m1a-vertical-slice，PR 待开）：llm-openai + tts-qwen-audio 适配器全实现
  （fixture 黄金测试 + 本地真 key 活体验证）；kernel 最小回合循环（内部接缝 + 内存 impl + 默认 persona +
  守护 tool_cap/timeout/单活跃锁/cancel）；speak 契约入 kernel-api（决策 A，全量审查已做）；
  godoword 会话 REST + SSE + dev-bypass。**本地真 key 全链路验证通过**：回合 finish=stop 无 error，
  2 次 speak 出声 + 词级时间戳 + 音频回放 + 转录持久化。修 1 真 bug（tool_calls 回喂 OpenAI 归一化）。
  全 workspace fmt/clippy/test 绿（28 通过 + 4 ignored 活体）。

## blocked / 待拍板决策

- 无当前阻塞。M1 全部设计决策已定案。
- 远期留 M2 拍板：无模块默认教师模式是否作永久"自由聊天"产品态（dev-debt T4）。

## 下一动作

1. M1a PR：CI 七项绿 → 用户 sign-off（无人工残留，纯开发态切片）→ squash merge。
2. **M1b 启动**：kernel 存储基底（Store pool + 迁移编排 + 完整 schema + 加密 + 绑定 DB impl + 配置 +
   自适应降权/部署初始化 + provider/binding REST）。回归 dev-debt T2/T3。

## 开发态临时取舍登记（dev-debt，须回归）

| # | 临时取舍 | 引入 | 回归于 | 回归断言 |
|---|---|---|---|---|
| T1 | env dev key 旁路 + 固定 dev user | M1a | M1c | dev-bypass feature 及 GODOWORD_DEV_* 从非测试代码删除 |
| T2 | 内存 impl 作生产默认 | M1a | M1b/M1c | 生产默认换 DB/文件系统；内存 impl 仅留测试 |
| T3 | 容器 root 运行（无降权/初始化） | M1a | M1b | 自适应降权 + 幂等 entrypoint 上线 |
| T4 | 无模块默认教师模式 | M1a | M2 决议 | M2 定永久产品态或退役 |

机制：dev-bypass feature release 永不启用 + 代码 `// DEV-DEBT: T<n>` 标记 + 每里程碑验收核对逾期阻塞。
永久架构（接缝 trait + 内存测试替身）非债务。详见 [roadmap](roadmap.md#开发态临时取舍回归机制)。

## 近期关键上下文

- 存储：共享 SQL 守双兼容 SQL 子集；`query!` 仅驱动专属，共享用运行时 query_as + 薄分发宏。
- 契约级变更（trait/Store/协议类型/speak 规格）强制全量 docs+wiki 语义审查 + 核对矩阵。
- speak 决策 A：kernel-api 新增 SpeakArgs/speak_tool_spec/SPEAK_TOOL_NAME + 黄金测试；M1a 执行时全量审查，
  修订 contract.md "kernel 定义"措辞与 P26 对齐。
- API 实测事实（适配器实现依据）：LLM OpenAI 兼容 /compatible-mode/v1（reasoning_content/tool_calls）；
  TTS DashScope 原生 /api/v1/services/audio/tts/SpeechSynthesizer（X-DashScope-SSE: enable，sentence 事件，
  base64 PCM 拼接，词级时间戳仅流式，音色 longanlingxin/longanlufeng，日语 language_hints:["ja"] 可用）。
- SQLite CVE 风险接受：本地向量需打开不可信 DB+wal，godoword 自建自有 DB 前提不成立；
  sqlx 0.8/0.9 均 pin libsqlite3-sys <0.38 无上游修复，roadmap 跟踪。
- 治理克制：当前最稀缺的是第一个真实 kernel 实现，非更完善治理（外部评审共识）。