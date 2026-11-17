---
date: 2026-11-17T18:28+08:00
keywords: [m1a, vertical-slice, speak, contract, adapter, sse, audit, dev-bypass, tool-calls]
scope: [project, kernel, kernel-api, llm-openai, tts-qwen-audio, godoword]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1a 垂直切片实现 + speak 契约落地 + 全量语义审查

## 背景

M1 蓝图（1720）定案后启动 M1a：用最薄端到端切片前置验证最高集成风险
（qwen 流式归一化 + DashScope TTS 音频管线 + SSE 协议 + speak 回合）。speak 规格落 kernel-api
契约（决策 A）属契约新增，触发强制全量 docs+wiki 语义审查。

## 决策

实现（六 phase，单 PR）：①本地真 key 录脱敏 fixtures（signed url 去除）+ 适配器骨架；
②llm-openai（OpenAI 兼容流式，reasoning_content/tool_calls 按 index 组装/usage/finish，错误归一化）；
③tts-qwen-audio（DashScope 原生 SSE，sentence 事件→裸 PCM/词级时间戳跨帧去重/complete 拼接，URL 模式下载归一化）；
④kernel 最小回合循环（内部接缝 SessionRepo/BindingResolver/AudioCache + 内存 impl + 默认教师 persona +
守护 tool_cap/timeout/单活跃锁/cancel）+ speak 契约（SPEAK_TOOL_NAME/SpeakArgs/speak_tool_spec + 黄金测试）；
⑤godoword 会话 REST + SSE + dev-bypass feature（env key + 固定 dev user，optional 适配器，release 永不启用）；
⑥全量审查 + 文档同步。

审查中发现并修 1 真 bug：回喂 LLM 的 assistant.tool_calls 须从 kernel 形 {id,name,arguments:对象}
归一化为 OpenAI 形 {id,type:"function",function:{name,arguments:字符串}}（llm-openai build_request），
否则上游拒绝第二回合。加回归测试锚定。

本地真 key 全链路验证（真实 HTTP 服务 + 真实 qwen）：回合 finish=stop 零 error，2 次 speak 出声 +
词级时间戳 + 音频回放（PCM 24kHz）+ 转录持久化 + cancel + 会话 CRUD。fixture 黄金测试锚定归一化
（LLM 83 块、TTS 7 事件含 complete=52800 字节，单块+切片跨块缓冲）。

## 后果

M1a 垂直切片成立：协议/音频/回合循环最高风险已消解，后续 M1b/c 铺存储/认证基底有稳固接缝。
dev-debt T1（dev 旁路）/T2（内存生产默认）/T3（root 运行）/T4（默认教师模式）引入并登记，
M1b/M1c/M2 回归。speak 成为版本化契约面。

### 全量语义审查核对矩阵（speak 契约新增 + M1a 实现事实）

| 文件 | speak 契约 | M1a 实现事实 | 处置 |
|---|---|---|---|
| docs/project/architecture.md | 运行时 speak 描述一致 | 当前状态 M0→M1a；bindings NULL→哨兵* | 已改 |
| docs/project/roadmap.md | 蓝图决策 A 一致 | M1/M1a 状态、当前差异更新 | 已改 |
| docs/project/status.md | — | live 状态 M1a 完成 | 已改 |
| docs/project/conventions.md | 契约审查含 speak（已列） | 无冲突 | 一致 |
| docs/project/gates.md | — | 无冲突 | 一致 |
| crates/kernel-api/docs/contract.md | "kernel 定义"→"kernel-api 定义规格/kernel 执行" + SpeakArgs | 当前状态 M1a 实现 | 已改 |
| crates/kernel-api/wiki/README.md | 通用契约说明 | 无冲突 | 一致 |
| crates/kernel/{docs,wiki}/README.md | speak 执行描述 | 新建 M1a 事实 | 新增 |
| crates/llm-openai/{docs,wiki}/README.md | — | 新建 + 实现完成 + tool_calls 归一化 | 新增/已改 |
| crates/tts-qwen-audio/{docs,wiki}/README.md | — | 新建 + 实现完成 | 新增/已改 |
| crates/godoword/{docs,wiki}/README.md | — | M0→M1a 接线 + dev-bypass | 已改 |
| wiki/user/README.md | — | M0→M1a 开发态切片 | 已改 |
| wiki/deploy/quickstart.md | — | M0→M1a（部署机制不变） | 已改 |
| wiki/contributors/README.md | 契约审查清单加 speak | 无冲突 | 已改 |
| README.md | — | 状态 M0→M1a | 已改 |
| AGENTS.md | — | 通用入门，无冲突 | 一致 |

残留搜索（M0/hello/尚无实现/kernel 定义）：docs/wiki 仅余合法历史引用（M0 完成里程碑、契约 M0 定义），
无超前事实或废弃表述。