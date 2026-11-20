---
date: 2026-11-20T01:30+08:00
keywords: [m1d, acceptance, bugfix, presets, upstream-401, logout, session-proliferation, test-connection]
scope: [project, kernel, godoword]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1d P8 验收修复（首轮人工验收发现 4 缺陷）

## 背景

P6/P7 全 UI 就绪后主开发者首轮人工验收（设置→对话流）发现 4 缺陷，卡在预设配置/试听步。逐项核实根因修复。

## 决策（缺陷与修复）

- **预设模板空（一键填充失效）**：默认 Config.provider_presets 为空（仅 config.toml 可定义），首跑无模板。
  修：kernel config.rs Config::default 内置 qwen-llm（v1 根）+ qwen-tts（**host 根**，适配器自 append DashScope 路径）默认预设
  （不含 key，config.toml 可覆盖/追加）；defaults 单测更新（2 预设）。同时消除 TTS base_url 误填完整路径困惑（预设含正确端点 + UI 提示）。
- **上游 key 无效→误登出（核心 bug）**：settings.rs upstream_status 把 ProviderError::Auth 映射 **401**，
  前端 401 钩子误判会话过期→登出。修：**上游 Auth→502 Bad Gateway**（401 专属用户会话 AuthUser）；
  core client.ts 401 钩子另排除认证端点（login/register/reset-password 的 401 是凭据错误不跳转）。
- **测试连接对 TTS 失败**：Settings「测试连接」对所有 provider 调 LLM models 端点（TTS 无 /models）。
  修：按 adapter_id 分流（llm-openai→providerModels；tts-qwen-audio→providerVoices）。
- **空会话泛滥**：Home '/' 每次访问自动 createSession（导航即建空会话）。修：Home 不再自动建；
  ChatView sessionId 可空，**首次发送懒创建**（currentId 局部态），访问 '/' 不发送则零会话。

## 后果

4 缺陷修复（内置预设/上游 502/测试分流/懒创建），cargo config 6/6 + godoword check + core tsc + shell svelte-check/build 绿。
全新库重部署复验。教训：**401 语义须专属用户认证**（上游错误 502），前端全局 401 钩子须排除认证端点；
首跑 UX 须内置预设（BYOK 黄金路径起点）；导航不应副作用建资源（懒创建）。详见 [status](../docs/project/status.md)。