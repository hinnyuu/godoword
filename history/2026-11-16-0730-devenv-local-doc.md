---
date: 2026-11-16T07:30+08:00
keywords: [devenv, environment, local-doc, agent, gitignore]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# devenv.md 第四类文档

## 背景

项目在不同开发环境流转（轻量服务器→裸金属→Fedora 笔记本），每个环境的无上下文 agent
都需了解本机环境（硬件/OS/podman 版本/工具链/网络），这些信息不该进仓库。

## 决策

仓库根 devenv.md，.gitignore 排除，严禁提交（含 git add -f），严禁密钥值只记路径。
第四类文档：环境本地操作上下文（非 docs 代码事实/非 history 变更/非 wiki 人类视角）。
固定骨架：created/verified 双时间戳、硬件资源、OS 安全模块、用户 uid 映射、容器运行时、
工具链、网络可达性、开发策略约束、本地路径、已知局限。维护协议：会话开始读取，任务前抽查，
矛盾即重探更新，每次核验更新 verified，重大变化全量重探。agent 发现不存在→探测创建，
存在→持续维护。项目文档可引用但绝不依赖其内容承载项目事实；验收 runbook 环境无关。
门禁：.gitignore + CI 禁入路径 + pre-commit。

## 后果

项目知识经仓库流转，环境知识每到新机器本地再生。首份已在开发机落地。
见 [conventions](../docs/project/conventions.md)。