---
date: 2026-11-19T17:57+08:00
keywords: [m1c, acceptance, merge, product-vision, doc-review, core-requirements, m1d-handoff]
scope: [project]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1c 多轮审计收尾合并验收 + 超规格产品需求文档复核

## 背景

PR #15（二审 R1–R9 + 三审 N1–N7 + 四审 H1/H2 + 五审 EOF）经第六次复核确认代码层可合并签收，
squash merge → main a2bdf17。R4 same-tree/main Release 重测 image+smoke 绿；人工残留 sign-off
（部署矩阵 SELinux/quadlet/docker + 真 key 活体 TTS 音质）主开发者已确认。M1c 收尾完成无阻塞。

主开发者要求合并更新 status 时做**超规格全量 docs/wiki 复核**：一确认加固信息已记录，二确认核心产品需求
在多轮 agent 开发 + 文档改动后仍**完整、显著、以用户实际使用需求为中心**，防未来 agent 接手忘记 godoword 本质。

## 决策

**复核结论**：加固信息已充分记录（architecture 当前事实段 + 模块 docs + history 2241/0003/1324/1529/1626）。
但核心产品需求在**入口面（README/wiki/user）显著度不足**——多轮加固后技术/安全文本占主导，产品愿景被稀释：
README 仅述 LLM 教师+BYOK+模块化，wiki/user 偏当前可用功能；多角色声音/角色扮演/双认证/集中评估/高自定义 WebUI/双栈
散落于 history/roadmap 细节，新 agent 不易一眼 grasp 全貌。**高自定义 WebUI 美化**在 roadmap M1d  scope 缺失（真缺口）。

**补强**（产品本质显著，非新增审计叙事，符合 history 2127 文档精炼原则）：
- README 新增「产品愿景与核心需求」段：BYOK/多角色声音与角色扮演/自适应教学与集中评估/本地+authentik 双认证/
  高自定义 WebUI/双栈私有部署单容器/高度模块化——权威核心需求清单，明示新 agent 接手须始终明确。
- wiki/user 新增「godoword 想成为什么」用户向愿景段。
- architecture 定位段补产品核心需求指针（指向 README 愿景），明确本文记当前架构事实、需求 Why 见 history。
- roadmap M1d scope 补「高自定义/可美化 UI」核心需求。
- status 恢复"M1c 收尾完成无阻塞"，completed 清单按文档精炼原则收敛为 history 指针，M1d 启动规划就绪。

## 后果

M1c 无阻塞，M1d 待主开发者发起设计讨论。六轮复核非阻断观察（summarize usage Finish 后或漏记/主循环与 summarize
Finish 处理不对称/断言助手可增强）+ outbox 消费幂等（A5）留 M2 一致性检查。核心需求权威清单锚定 README，
后续里程碑文档改动须维护其显著度（防漂移）。术语守 glossary（教学模块/评估模块/教学方案/角色扮演/绑定）。