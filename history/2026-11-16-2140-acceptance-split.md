---
date: 2026-11-16T21:40+08:00
keywords: [acceptance, smoke-test, ci, podman, automation, human-residual]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: "1"
---

# 验收拆分：自动化冒烟 + 人工残留

## 背景

M0 人工验收中 uid/卷矩阵等纯 CLI、确定性反馈的测试无需人工目视。用户提议将这类测试
并入自动化冒烟（成功自动清理），仅真正需要人的项交人工。规范重要性大于拖一个 CI 周期。

## 决策

拆分判据：确定性 CLI 反馈 + agent 有访问权 + 可客观断言 → 自动化冒烟；
需人类感官/判断（webui 视觉交互、TTS 音质）或 agent 缺访问权（SELinux/quadlet/docker 主机）→ 人工。
scripts/acceptance/smoke.sh 直接 CI 集成（镜像构建后 podman 自动跑，非分阶段）。
覆盖 healthz/meta/静态/双栈 localhost+IPv4/优雅退出/uid 卷矩阵/删容器重建/bind 属主断言。
资源唯一前缀隔离 + 专用端口段（18080+）+ trap EXIT 自动清理（含临时用户每次创建删除、镜像一并删），
只清自有资源绝不碰用户容器。merge 前置 = 冒烟绿 + 人工对残留清单 sign-off（人工介入仍强制但聚焦）。
PR 模板重构为自动化冒烟结果 + 人工残留清单。smoke 脚本作为 M0 最后 commit（CI 发现问题则继续修）。

## 后果

R5/R13 细化（人工聚焦残留）。人工验收随 webui/TTS 演进更重要而非消失。
见 [conventions](../docs/project/conventions.md)、[gates](../docs/project/gates.md)。