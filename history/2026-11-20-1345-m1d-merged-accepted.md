---
date: 2026-11-20T13:45+08:00
keywords: [m1d, acceptance, merge, audit, second-audit, r4, m2-handoff]
scope: [project, kernel, kernel-api, godoword, auth-local]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "18"
---

# M1d 合并验收闭环（审计修复轮一审/二审 + R4 main 重测通过）

## 背景

M1d 实施 PR #18 经主开发者完整人工验收（产品流/音质/移动功能/部署矩阵 sign-off，history 0330）后，
主开发者要求 M2 启动前先全量审计 + M1d 侧重审计（六维度），发现全量修复（一审 history 1240）+
修复代码自身二审（history 1320）+ 新增 UI 面复验通过。

## 决策

**合并**：PR #18 squash → main `6a64852`（feat(godoword): M1d 前端实施 + 合并前审计修复轮）。
- CI 全 8 绿（run 44682570093：commitlint/docs-gate/frontend/image/rust/secrets/smoke/storage；
  frontend 已入 main required checks + image needs，审计 G1 修复生效）。
- **R4 main 重测通过**（release run 44683464930：image 构建 + 冒烟 same-tree 重跑绿）。
- 审计修复轮交付：一审确证缺陷 A/B/D/M7/M1/G1 + 加固 F1–F7 + 前端 M2–M6 + 需求覆盖 Q3/Q4/Q7/Q9/Q12/Q13 补齐 + i18n 清扫；
  二审 R2-1（prefs 队列快照时机真缺陷）–R2-7；负例测试同配（双后端 + handler 级 + vitest 54 + 冒烟 M1d 面）。
- 契约层（kernel-api）审计轮未变更；OpenAPI 快照再生 + 类型 freshness 保持。

**M1 全部完成**（M1a 适配器+回合 / M1b 存储基底 / M1c 会话+认证 / M1d 图形界面+审计闭环）。
dev-debt T1/T2/T3 已回归，T4（默认教师模式产品态）留 M2 决议。

## 后果

M1d 正式闭环。下一里程碑 **M2**（teach-assess 评估模块 + teach-gojuon 五十音，placement→plan→自适应课程黄金路径）
设计讨论待主开发者发起（评估实现细节另开专轮，history 0630）。M2 携带留档：默认教师模式产品态（T4）、
角色音色档案实体 + 白名单音频复用 + 静态资产、M1c 非阻断观察一致性检查、outbox 消费幂等（A5）、
重试再生语义（history 1240 取舍）、BYOK 转发面公网场景再议（R2-7，M4 OIDC）。
详见 [status](../docs/project/status.md)、[roadmap](../docs/project/roadmap.md)、[history INDEX](INDEX.md)。