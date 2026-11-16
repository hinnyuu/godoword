---
date: 2026-11-16T03:00+08:00
keywords: [cicd, conventional-commits, ghcr, acceptance, branch-protection]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# CI/CD 与人工验收

## 背景

长期维护需自动化质量门与可复现部署，且部署验收必须有人工介入。

## 决策

严格 conventional commits；GitHub workflow + ghcr.io。分支测试全绿 → 构建推送
ghcr.io/hinnyuu/godoword:pr-N/sha-xxxx → 开发者人工（agent 协助）podman 实际部署验收 →
squash merge 进 main → main 构建 :latest，release semver。main 分支保护（PR+检查全绿）。
agent 每次须产出验收 runbook（具体命令、初始化预期、测试清单、通过标准）写入 PR 固定章节。
验收对象与合并对象须同一镜像 digest。

## 后果

CI 含 commitlint/fmt/clippy/test/docs-gate/gitleaks/许可证/构建。
人工验收是 merge 前置。见 [conventions](../docs/project/conventions.md)。