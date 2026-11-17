---
date: 2026-11-18T03:29+08:00
keywords: [governance, full-audit, trigger, design-direction, supersede, drift, second-review, gates, conventions]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 全量语义审查触发规则扩展（契约级 + 设计方向级）

## 背景

常设规则原为"凡 kernel-api 契约级变更触发全量 docs+wiki 语义审查"。M1c 设计讨论中音频复用方向
从蓝图指纹 hash 去重**反转**为显式白名单（见同日音频条目），并精化 0100 角色扮演分类——**无 kernel-api
契约变更**，却使 blueprint（1720）/architecture/roadmap/status 的 hash-cache 旧表述与定案直接冲突，
制造跨多文档真实分歧。主开发者指出：重要决策须落档 + 超规格全量审查防仓库内记录漂移，并趁上下文做二审。
拘泥"无契约变更不审"会漏掉这类设计方向反转制造的漂移。

## 决策

**扩展触发条件**（gates.md + conventions.md 同 PR 更新）：全量语义审查由两类变更触发——
①kernel-api 契约级变更（原规则）；②**supersede 蓝图/architecture 既有假设的设计方向变更**
（非契约变更但跨多文档制造真实分歧）。审查方法不变（全量读 + 核对矩阵 + 搜剿旧表述 + 修不一致 + 结论记 history）。

**设计方向级审查须二审**：一审自查难免盲点，趁上下文仍在做二审复核（抓语义残留/矩阵盲点）。

**结论 history 须含触发理由**（契约级 or 设计方向级 supersede），使未来 agent 既不漏审也不滥审
（防"无契约变更不审"漏审，亦防凡讨论皆审的治理膨胀）。

**部分精化 nuance**：新条目精化既有 history 的某部分（非整体取代）时，正文引用澄清，不动旧条目
frontmatter；整体取代才改 status/superseded_by（防误废仍有效决策，如 1720 拆分/speak/接缝、0100 三层架构）。

## 后果

触发规则涵盖设计方向反转，与主旨（防漂移第一）一致；这是套用既有全量审查机制防真实分歧，非新造官僚。
本次 M1c 设计落档即按此规则执行一审 + 二审（结论另记 history）。gates/conventions 节标题广化为
"全量语义审查（契约级与设计方向级）"/"全量语义审查（制度化）"，锚点链接同步更新。
详见 [gates](../docs/project/gates.md)、[conventions](../docs/project/conventions.md)。