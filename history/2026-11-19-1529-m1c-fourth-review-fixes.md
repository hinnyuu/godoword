---
date: 2026-11-19T15:29+08:00
keywords: [m1c, fourth-review, shutdown-lifecycle, lock, finish-reason, summarize, openapi-matrix]
scope: [project, kernel, godoword]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "15"
---

# M1c 第四次复核收尾（H1/H2）+ 测试同源偏差增强

## 背景

PR #15 第三次审计修复（N1–N7）后，第三方定向复核（/tmp/godoword_PR15_第四次复核意见.md，head 73ba317）
确认 OpenAPI/end_session/重置/限流/缺 sidecar/迁移回滚六项通过，summarize 流 Err 主路径通过；
但发现两个阻断项：H1 shutdown 准入—注册—快照竞态、H2 FinishReason::Error 仍固化部分摘要。核实属实，本期修。

## 决策

- **H1**：N6 的 shutting_down 原子标志只约束变量本身，"检查—spawn—注册"与"置标志—取快照"非同一临界区，
  合法交错可在快照后登记新任务（错过 cancel/等待，与 checkpoint 并发）。改 **tasks 互斥锁串行化生命周期**：
  run_turn 锁内复检标志 + spawn + push（锁内不 await）；shutdown 锁内置标志 + 取快照，随后 cancel/wait/abort+join。
  不变量：run_turn 先获锁→handle 先注册必被快照取到；shutdown 先获锁→后续 run_turn 必见 true 拒绝。
  新增 pending_task_count/active_count 观测器（lib pub，集成测试可见）。
- **H2**：summarize_span `Ok(ChatChunk::Finish(_))` 落 `Ok(_)=>{}` 被忽略，TextDelta+Finish(Error) 仍 Some(部分)
  →固化（llm-openai sse 未知 finish reason 映射 Error，真实路径）。显式处理 Finish：**非 Stop（Error/Length/
  ContentFilter/Cancelled）视为失败返回 None**，按 truncate 回退。策略明确：截断/过滤/错误摘要均不固化（不完整不可靠）。
- **非阻断增强**：OpenAPI 测试加**独立精确公开操作表**（HashSet 硬编码），避免与生产 is_public 同源偏差
  （误把受保护端点加入 is_public 时独立表断言失败）；openapi.rs rustdoc 旧"全局 security"同步操作级矩阵。

## 后果

H1/H2 修复 + 确定性回归（注册先于 shutdown 必被 abort/join；barrier 并发 30 轮生命周期不变量；
Finish(Error)/Finish(Length) 不固化负例）。turn 18 测试绿，全 workspace 双后端绿，fmt/clippy 净。
history 不回写（新增本条目）。待验收：PR #15 CI 重跑绿 + 定向复核 + 合并 + R4 main 重测 + 人工 sign-off
→ 恢复"M1c 收尾完成无阻塞" → M1d。
教训：原子标志不等于临界区——生命周期决策（检查/注册/快照）须同一锁串行化；失败通道须穷举
（流 Err 与 Finish(Error) 是相邻两条，修一条须查相邻）。