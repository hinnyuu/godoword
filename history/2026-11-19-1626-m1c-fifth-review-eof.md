---
date: 2026-11-19T16:26+08:00
keywords: [m1c, fifth-review, summarize, eof, saw-stop, finish-reason]
scope: [project, kernel]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "15"
---

# M1c 第五次复核收尾：summarize 无 Finish 静默 EOF

## 背景

PR #15 第四次复核修复（H1/H2）后，第三方交叉复核（/tmp/godoword_PR15_第五次复核意见.md，head 6f9fd2c）
确认 shutdown tasks 锁不变量、Finish(Error/Length) 失败关闭、OpenAPI 独立矩阵均通过；追加确认一个相邻失败通道：
**部分文本 + 无 Finish 静默 EOF 仍被当成功固化**。核实属实（summarize_span 无 saw_stop；llm-openai sse
[DONE]/自然 EOF 可不发 Finish，真实路径）。本期修。

## 决策

summarize_span 显式维护 `saw_stop`：**仅实际收到 FinishReason::Stop 才算完整成功**。
非 Stop Finish 设 err 并于首个 Finish 后停读（避免 Stop 后处理额外文本）；EOF 未收 Stop 补 err。
成功条件 `saw_stop && !err && !summary.is_empty()`（取代原 `!err && !empty`）。失败/未完整结束按 truncate 回退，
不存储/复用部分 fragment。测试重构为共享 Arc 计数 + 通用断言助手，表驱动覆盖 Error/Length/ContentFilter/
Cancelled/ToolCalls 五种非 Stop Finish + 无 Finish 静默 EOF 负例。

## 后果

turn 18 测试绿，全 workspace 双后端绿，fmt/clippy 净。history 不回写（新增本条目）。
待验收：PR #15 CI 重跑绿 + 定向复核 + 合并 + R4 main 重测 + 人工 sign-off → 恢复"M1c 收尾完成无阻塞" → M1d。
教训：失败通道须穷举相邻路径——流 Err、显式非 Stop Finish、无 Finish EOF 是三条独立通道，
"仅 Stop 成功"须同时覆盖"根本没有 Finish"。修一条须查相邻（与第四次 H2 教训一致）。