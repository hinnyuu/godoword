# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-17T17:40+08:00
- 当前里程碑: M1a（垂直切片：适配器 + 回合循环 + SSE 出声）
- 主线: M0（51d16c3）→ M1 前置 C（bf952a3）→ M1 前置小 PR（f8ff64c）→ M1 蓝图（bde1ba8）→ M1a 进行中

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

- **M1a**（分支 feat/m1a-vertical-slice，蓝图见 [roadmap](roadmap.md#m1-拆分蓝图)）：
  - phase 1 ✅ fixtures 录制 + 适配器 crate 骨架：本地真 key 录脱敏 LLM/TTS 响应
    （llm-openai/tests/fixtures/{models.json,chat-stream-toolcall.sse}、
    tts-qwen-audio/tests/fixtures/{tts-stream.sse,tts-url.json}，signed url 已脱敏）；
    两适配器 crate 骨架（module.yaml/docs/wiki/stub）入 workspace；cargo check/clippy/fmt 洁净。
    上游协议事实已记入各适配器 docs/README.md。
  - phase 2–6 待做：llm-openai 实现 → tts-qwen-audio 实现 → kernel 回合循环 + speak 契约（全量审查）+ SSE
    → godoword 接线 + 会话 REST → 端到端验证 + 文档/history + PR。

## blocked / 待拍板决策

- 无当前阻塞。M1 全部设计决策已定案。
- 远期留 M2 拍板：无模块默认教师模式是否作永久"自由聊天"产品态（dev-debt T4）。

## 下一动作

1. M1a phase 2：llm-openai 适配器实现（chat_stream 流式归一化 + list_models + features）+ fixture 黄金测试。
2. 随后 phase 3–6（见 in-flight）。M1a 单 PR 多 commit（phase 级），phase 6 开 PR → CI 绿 → sign-off → merge。

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