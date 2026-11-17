---
date: 2026-11-16T09:30+08:00
keywords: [kernel-api, traits, llmprovider, ttsprovider, authprovider, teachingmodule, planprovider]
scope: [kernel-api]
type: decision
status: accepted
supersedes: []
superseded_by: [2026-11-17-1231-storage-contract-c.md]
pr: ""
---

# kernel-api 契约 trait 签名

## 背景

契约轮第二步：定义模块间标准接口，使模块可整 crate 重写替换。

## 决策

kernel-api 薄契约 crate（trait+类型，无实现）。LlmProvider（id/features/list_models/
chat_stream→ChatChunk：TextDelta/ReasoningDelta/ToolCalls 组装/Usage/Finish）；
TtsProvider（id/models/list_voices/synthesize_stream→AudioEvent：Start/Chunk/Words 增量/End/Usage，
URL 模式适配器内部归一化为流）；AuthProvider（统一 enum AuthRequest→AuthOutcome：
Authenticated/Redirect/PendingLink + extra_routes）；TeachingModule（manifest/persona/tools/
session_params/routes/frontend/execute_tool/handle_event 默认空/on_session_start/end）；
ModuleContext（kernel 实现：user_id/session_id/store scoped sqlx/emit_progress/current_plan）；
PlanProvider 薄接口 current_plan。ProviderConn{base_url,api_key:SecretString,extra} kernel 解密传入，
适配器不落盘 key。ProgressEvent 信封 + ProviderError 归一化。speak 工具 kernel-api 定义、
kernel 经 TtsProvider 执行。存储实用派（scoped sqlx pool，不造抽象）。

## 后果

适配器/教学模块只依赖 kernel-api，互不 import。签名见 crates/kernel-api/docs/。