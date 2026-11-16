---
date: 2026-11-16T09:00+08:00
keywords: [session, sse, runtime, disconnect, extraction, history, eviction]
scope: [project, kernel, kernel-api]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 会话协议、运行时与进度提取

## 背景

契约轮第一步：定义后端→前端核心 UX 契约与 agent 运行时策略。

## 决策

传输 REST 管理会话 + POST /api/v1/sessions/{id}/messages 响应即该回合 SSE 流（OpenAI 式）。
事件：run-start/text-delta/reasoning-delta/tool-call/tool-result/audio-start/audio-chunk/
audio-end/word-timestamps/usage/error/run-end。断开行为用户可配 on_disconnect=cancel(默认省费)
|continue(服务端跑完全量持久化+音频入缓存)，detach 上限每用户 2；显式 cancel 始终可用；
v1 无 mid-run 回放缓冲。音频管线流式 PCM/24000 → AudioWorklet，词级时间戳高亮，缓存回放完整文件。
运行时：系统提示词常驻 + 历史窗口；token 预算字符启发式（roadmap 换 tokenizer）；
淘汰 truncate(默认)/summarize(opt-in)；工具上限 8/回合超时/上游重试/单活跃回合锁。
进度提取：转录持久化故淘汰只移上下文不删 DB → 异步；kernel 发 session-evicted/session-ended
持久事件，评估模块订阅→用自身绑定（role=extractor）调 LLM 提取→提取报告喂方案。
绑定加 role 维度（细化 byok-binding 条目）。会话历史：GET 列表 + 继续旧会话采用当前 plan 版本，
shell 标准历史组件，转录回顾+音频回放。

## 后果

结构化进度事件记练习对错，提取报告记对话涌现证据，评估两者都吃。
见 [architecture](../docs/project/architecture.md)。