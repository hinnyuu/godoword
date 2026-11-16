---
date: 2026-11-16T01:00+08:00
keywords: [modularity, kernel, adapter, teaching, compile-time]
scope: [project, kernel-api]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 三层模块化架构

## 背景

单人开发者 + agent 长期维护，要求模块高度独立，无上下文 agent 仅凭模块文档即可维护，
最坏情况整模块重写不影响整体。

## 决策

三层：内核 kernel（配置/存储/加密/会话/运行时/注册/outbox/计量）、适配器 adapter
（LlmProvider/TtsProvider/AuthProvider）、教学模块 teaching（TeachingModule）。
模块目录自治 crates/<name>/{src,docs,wiki,module.yaml,migrations}，互不 import，
只依赖 kernel-api 契约。v1 编译期模块化（cargo feature），运行时 wasm 插件搁置。
"webui 美化"归内核 shell/主题，"角色扮演"归内核 persona 能力，均非独立模块。

## 后果

kernel 后拆为 kernel-api（薄契约）+ kernel（实现），见后续条目。
编译期模块化是开发者维护边界，用户经 UI 偏好隐藏模块。见 [architecture](../docs/project/architecture.md)。