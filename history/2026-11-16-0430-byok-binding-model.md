---
date: 2026-11-16T04:30+08:00
keywords: [byok, providers, bindings, role, resolution, usage]
scope: [project, kernel]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# BYOK 绑定模型与用量计量

## 背景

多 provider/多 key/多模型：用户可能 4 模块用 10 provider、13 key、20 模型。
需默认共享又允许模块差异化。BYOK 用户付费，用量计量 essential。

## 决策

providers(id,user_id,adapter_id,label,base_url,key_enc) 一实例一 key；
bindings(user_id,module_id,capability,role,provider_id,model,params_json)，module_id NULL=用户默认。
解析：模块覆盖 → 用户默认 → 未配置（UI 引导，无服务端兜底）。role ∈ default/summarizer/
extractor/assessor（后增，支持提取/评估用便宜模型）。适配器暴露 list_models/list_voices。
所有上游调用经 kernel 记 call_log（用量看板、预算警告、错误归一化）。
TTS 缓存 key=hash(model+voice+text+params)，存音频+时间戳，回放不重复付费。

## 后果

role 维度由进度提取需求（见后续条目）触发加入。key 持久化=DB 加密，env 仅部署级配置。
见 [architecture](../docs/project/architecture.md)。