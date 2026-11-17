---
date: 2026-11-17T12:40+08:00
keywords: [audit, full-review, storage, contract, drift, matrix]
scope: [project]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 存储契约变更全量语义审查（制度化首次执行）

## 背景

存储契约 C 方案属 kernel-api 契约级变更，按新制度（见 1231 条目）强制全量 docs+wiki 语义审查，
超出机械门禁，防语义漂移。

## 决策

全量读所有 docs/wiki，按核对矩阵逐条核对变更事实（F1 Store enum / F2 双兼容 SQL 子集 /
F3 CI 双测 / F4 R4 same-tree / F5 gate 修 / F6 耦合声明 / F7 审查制度 / F8 serde 测试 /
F9 PG-v1·MySQL 远期 / F10 M0 验收完成）：

| 文件 | 核对结果 |
|---|---|
| README.md | F10 状态已改（M0 完成）；余高层无关 |
| docs/project/architecture.md | F1/F2/F3 已改，F9 一致 |
| docs/project/conventions.md | F2/F4/F7 已改，F3 一致 |
| docs/project/roadmap.md | F1/F9/F10 已改，M1 含双后端 |
| docs/project/gates.md | F3/F5/F7 已改 |
| docs/project/glossary.yaml | F1/F2/F3 新术语已入表 |
| docs/project/impact-map.yaml | 规则未变，无关 |
| crates/kernel-api/docs/contract.md | F1/F2/F6 已改 |
| crates/kernel-api/wiki/README.md | 高层一致，无存储细节 |
| crates/godoword/docs·wiki/README.md | hello 服务，一致 |
| wiki/user/README.md | M0 hello，一致 |
| wiki/deploy/quickstart.md | 部署级一致（quadlet 单 PublishPort 已正） |
| wiki/contributors/README.md | F2/F7 已改（新增模块须守双兼容子集 + 契约审查） |
| history/README.md | 一致 |
| history 条目 | 不可变豁免；0400/0930 加修订链接 |

搜剿旧表述（Any pool / 存储抽象 / 同一镜像 digest / scoped sqlx）：当前文档命中均为
"已证伪弃用/不要求"的有意否定陈述，无旧方案背书残留。修正 wiki/contributors（补双兼容子集约定）
与 README（状态）。机械门禁全绿 + 索引 freshness 通过。

## 后果

契约级变更全量审查制度首次执行验证有效。后续 kernel-api 契约变更循此例。