---
date: 2026-11-17T14:50+08:00
keywords: [ci, dual-backend, workspace, gitleaks, checksum, tests-coverage, handoff, agents, status]
scope: [project, kernel-api]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1 前置小 PR：CI 缺口闭合 + agent 交接基础设施

## 背景

外部评审（/tmp/M1前建议前置小PR.md）通读全仓库后提出三处真缺口 + 交接关切，明确反对再来一轮
治理补强（审计惯性风险）。主开发者认可 §1–3，并追加 agent 交接基础设施（旋转 agent 模型运维必需）。

## 决策

§1 双后端兜底扩 workspace 级：storage job 改 `cargo test --workspace`（设 TEST_PG_URL），
任何读此变量的测试自动获双后端，无清单腐烂；storage.rs 未设 TEST_PG_URL 时显式 skip 提示
（消除静默退化）；convention 明文含共享 SQL 的 crate 走 backends() 夹具。不加 grep 式机械强制
（引用≠真跑，粗检查给假信心，违背"门禁防漏改不防语义写错"哲学）。修复"文档超前事实"：
原 gates/conventions 称模块方言漂移双向捕获，但执行机制此前只覆盖 kernel-api 自身。

§2 gate 纳入 tests/（选 B）：coverage/omission 匹配扩 src/+tests/，module.yaml 加 tests/** 规则。
serde 黄金测试是契约锚点，改动触发文档要求，机械层与契约审查制度对齐。显式决定而非留白。

§3 gitleaks 二进制加 sha256 校验和验证后执行（供应链一致性低垂果实）。

agent 交接基础设施（两层，committed）：AGENTS.md（根，稳定入门地图+阅读顺序+接续协议，短小不复述）
+ docs/project/status.md（live 交接状态：里程碑/done/in-flight/blocked/下一动作/待拍板决策）。
持续维护（非临终遗言，上下文可能突然耗尽）；gate 强制 src/tests 改动同 PR 更新 status（新鲜度）。
与 devenv 相反——交接须随仓库走（跨机器），故 committed 非 gitignored。

## 后果

纪律➀对未来模块真正生效（workspace 级双后端）。新 agent 有切入口与交接棒，上下文耗尽可接续。
M1 拆分（§4）独立待定（status.md 记录）。治理到此为止，下一步全力 M1 实现（垂直切片优先）。
本 PR 非契约级变更（不动 trait/Store/协议类型），不触发全量语义审查。