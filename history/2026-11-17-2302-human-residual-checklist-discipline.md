---
date: 2026-11-17T23:02+08:00
keywords: [acceptance, human-residual, checklist, discipline, selinux, quadlet, runbook, pr]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: "7"
---

# 人工残留验收清单纪律化（M1b 验收批评触发）

## 背景

M1b（PR #7）Fedora 真机人工残留验收测试层面全过（SELinux :Z 正例、不带 :Z 负例日志、quadlet 均正常），
但主开发者批评 PR 的「人工残留验收清单」质量：①指引命令不全——只列 :Z 测试命令，缺不带 :Z 与 quadlet
的详细命令，更缺各项对应的清理命令；②镜像 tag 笼统写 `pr-N`，应直接写明具体 `pr-7`。
指出 PR 人工测试部分的说明性文字也需纪律化。

## 决策

**人工残留清单纪律**入 [conventions](../docs/project/conventions.md#验收拆分自动化冒烟--人工残留)：
PR 残留清单**逐项**须含三件——①完整可复制命令（含具体镜像 tag 如 pr-7，禁笼统 pr-N；含前置准备）；
②明确通过标准；③测试后清理命令（容器/卷/单元/临时目录，只清自有资源）。**覆盖全部残留项**
（SELinux 须含 :Z 正例 + 不带 :Z 负例 + quadlet，不得只列部分），命令与清理一一对应。
agent 撰写后逐项自检完整性；[gates](../docs/project/gates.md) 提交前自查清单加第 9 项核对。
M1b PR #7 残留清单按此重写（:Z/不带:Z/quadlet 三段各含完整命令 + 通过标准 + 清理，tag=pr-7）。

## 后果

人工残留清单从"随手列几步"升级为可执行、可清理、全覆盖的纪律化 runbook，防再犯。
机械门禁无法校验 PR body 清单完整性（非仓库内容），靠 agent 自查 + 人工审视可执行性兜底。
与自动化冒烟的「资源隔离 + trap 自动清理」纪律对齐——人工项亦须明确清理，不留残留资源。