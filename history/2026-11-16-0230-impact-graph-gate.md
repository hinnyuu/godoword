---
date: 2026-11-16T02:30+08:00
keywords: [impact-graph, gate, ci, coverage, drift]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 关系图谱门禁

## 背景

小文件修改可能需更新散布项目各处甚至其他模块的文档/wiki，agent 每次提交易漏掉。

## 决策

项目级 agent 易读的关系图谱：各模块 module.yaml 声明 impact 规则（glob → docs/wiki + history），
项目级 impact-map.yaml 承载跨模块契约边。CI 聚合校验：覆盖率（每个源文件须命中规则，防图谱腐烂）、
漏改门禁（命中文档须同 PR 更新或显式豁免）、链接检查。诚实局限：防漏改不防语义写错，
语义层靠 agent 自查清单 + 人工验收。

## 后果

图谱分布式源头避免手写大文件腐烂。豁免须留痕（docs-nochange 标签 + 理由）。
见 [gates](../docs/project/gates.md)。