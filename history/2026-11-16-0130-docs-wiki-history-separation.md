---
date: 2026-11-16T01:30+08:00
keywords: [docs, wiki, history, separation, current-facts]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 文档/wiki/历史严格分离

## 背景

过去项目在文档与 wiki 内记载历史变更，长期导致文档膨胀、agent 注意力稀释甚至上下文爆炸、
开发者被历史绕晕、agent 混淆历史方案与当前方案。

## 决策

三类严格分离：docs（agent/主开发者，开发规范，项目级+模块级）、wiki（人类，用户/协作者视角，
充足解释）、history（独立，为什么变）。docs/wiki 只记"当前提交代码事实"与"与 roadmap 的差异/待办"，
严禁堆积历史。history 只记"为什么变"，不复述当前事实。

## 后果

需要关系图谱门禁保证 agent 提交时同步更新散布各处的文档（见后续条目）。
后增第四类 devenv.md（环境本地，不入仓库）。见 [conventions](../docs/project/conventions.md)。