---
date: 2026-11-21T11:58+08:00
keywords: [m2, p1, acceptance, merge, sign-off, handoff, p2, outbox, delete-semantics, migration-runner]
scope: [project]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "25"
---

# M2 P1 合并验收闭环 + P2 新 agent 交接

## 背景

P1 风险前置切片（history 0010）PR #25 经 CI 首轮 docs-gate 漏改 3 项补修（真实内容非豁免 +
预检时序教训落档 gates/AGENTS）后全 8 绿，主开发者人工 sign-off 两项通过（跨版本迁移升级 pr-24→pr-25
同卷旧会话/文件夹 intact + 真 key teach-free 对话 persona/speak/词级高亮感官），裁决合并。
**合并后不着急启动 P2，先核对交接准备，移交下一个 agent 继续**（主开发者裁决）。

## 决策

- **PR #25 squash 合并 main（27c5720）**：CI 全 8 绿（含新冒烟模块面断言 + cargo-deny noyalib 许可）；
  sign-off 全过；R4 main 冒烟重测（same-tree）。M2 P1 正式闭环。
- **P2 启动方式 = 新 agent 交接**（本会话收工）：status.md 落 P2 开工指引；交接 PR 把自身列为
  merged HEAD 的 in-flight 属自指陈旧（审计 D2），故 in-flight 记「P2 待启动（新 agent 交接）」。
- **D8 全表面核对**：status/roadmap/architecture/README/wiki/user/deploy/contributors/glossary/
  kernel docs+storage+wiki/godoword docs+wiki/teach-free docs+wiki/llm+tts+auth-local docs/kernel-api contract+wiki/
  OpenAPI+schema.d.ts/gates/AGENTS/conventions 逐面核对——P1 事实已单一化，无「P1 待启动/在飞」陈旧残留；
  meta milestone 保持 M1d（M2 未验收，产品里程碑标签随 M2 验收更新）；无 PR 模板文件（无需核对）。

## 后果

- **P2 范围**（roadmap M2 蓝图，kernel 内部为主，**预计无 kernel-api 契约变更**——consumes_events 早在
  ModuleManifest，扇出/幂等/守护/DELETE 翻转/迁移 runner 皆 kernel 实现层；若触 trait 则触发契约审查）：
  0007 其余基底 + outbox 扇出（consumes_events 订阅，现 dispatch_once 按 ev.module_id 串行路由须改扇出）+
  event_consumptions per-subscriber 幂等表（A5）+ 消费守护（B5 有界并发/per-event spawn/超时/重试上限/死信）+
  **DELETE 不发事件语义翻转（B2）**+ 模块迁移 runner（mod_<id>_NNNN，注册序追加）。
- **P2 关键接缝事实**（交接用）：
  - `kernel/src/journal.rs`：dispatch_once 现串行按 ev.module_id 路由（M1c 骨架，main.rs handlers 空 HashMap）；
    OutboxEvent 已含 created_at + outcome/item_ref/difficulty（P1）；end_session 现 DELETE+session-ended 同事务
    （R6/N2 锚定，**B2 翻转须改此处 + 更新 tests/journal.rs end_session_atomicity/end_session_concurrent_unique**）；
    complete_session 已发 session-ended（P1，完成≠删除）。
  - `kernel/src/migrate.rs`：run_migrations(store, &[Migration], backup) 自定义双后端 runner（版本表 + checksum +
    备份 + 每条同事务）；模块迁移按注册序追加，版本键 mod_<id>_NNNN；teach-free 无私有表（迁移走 kernel 0006），
    首个 runner 消费者 assess（P3）。
  - `kernel/src/registry.rs`：ModuleRegistry（manifest/module/factory），manifest.consumes_events 已就绪供扇出订阅。
  - 0006 已落（plan_version INTEGER/completed_at/call_log.module_id/progress 结构化列/teach-free 迁移/audio_assets）；
    P2 其余基底顺延 0007。
- **测试锚定纪律**（P2 须守）：双后端 backends() + 两用户越权负例 + 幂等/毒丸重试上限/并发负例（B5）+
  DELETE vs complete 事件语义负例（B2）+ 迁移幂等/回滚；可靠性声称须失败路径测试（conventions）。
- 详见 [status](../docs/project/status.md)、[roadmap M2 蓝图](../docs/project/roadmap.md#m2-实施蓝图)、
  [history 0010 P1 实现](2026-11-21-0010-m2-p1-module-slice.md)。