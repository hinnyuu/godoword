---
date: 2026-11-18T03:40+08:00
keywords: [full-audit, m1c, design-direction, reconciliation, matrix, drift, second-review]
scope: [project, kernel, godoword, kernel-api, llm-openai, tts-qwen-audio]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1c 设计落档全量语义审查（一审）

## 背景

M1c 设计决策定案（history 0329 三条目）后，音频方向 supersede 蓝图指纹 hash 假设 + 精化 0100 角色扮演分类，
跨多文档制造真实分歧。按新触发规则（history 0329 触发规则条目，设计方向级）执行全量 docs+wiki 语义审查。
本条目记一审 + 二审结论（二审趁上下文复核，提交后）。

## 决策

**触发理由**：设计方向级 supersede（音频 hash→白名单 supersede 1720/roadmap/architecture 旧假设；
角色扮演精化 0100），非 kernel-api 契约变更。**审查方法**：全量读 21 个 docs/wiki 文件（非抽查）+ 核对矩阵 +
搜剿旧表述 + 交叉核对 + 修不一致。

**变更事实列**：F1 音频白名单取代指纹（默认新鲜合成+白名单 M2+静态资产 M2+缓存有界）；F2 角色扮演横切基础设施
（地基原语+模块编排+内核实体 M2+克隆 provider 侧）；F3 M1c 六决策（契约姿态/continue/认证 A/全量 summarize/
outbox kernel 哨兵/call_log）；F4 全量审查触发规则扩展（契约级+设计方向级+二审）；F5 适配器已实现（既存漂移修复）。

**核对矩阵**（行=docs/wiki 文件，列=F1–F5；改=本 PR 修改，一致=核对无误，—=不涉及）：

| 文件 | F1 | F2 | F3 | F4 | F5 |
|---|---|---|---|---|---|
| docs/project/architecture.md | 改 | 改 | 一致 | 一致 | — |
| docs/project/conventions.md | — | — | — | 改 | — |
| docs/project/gates.md | — | — | — | 改 | — |
| docs/project/glossary.yaml | 改 | 改 | — | — | — |
| docs/project/impact-map.yaml | — | — | — | — | — |
| docs/project/roadmap.md | 改 | 改 | 一致 | 一致 | — |
| docs/project/status.md | 改 | 改 | 改 | 改 | — |
| wiki/contributors/README.md | — | — | — | 改 | — |
| wiki/deploy/quickstart.md | 一致 | — | 一致 | — | — |
| wiki/user/README.md | 一致 | — | 一致 | — | — |
| kernel-api/docs/contract.md | — | — | 一致 | 一致 | — |
| kernel-api/wiki/README.md | — | — | 一致 | — | — |
| kernel/docs/README.md | 改 | — | 一致 | — | — |
| kernel/docs/storage.md | — | — | 改 | — | — |
| kernel/wiki/README.md | 一致 | — | 一致 | — | 改 |
| godoword/docs/README.md | — | — | 一致 | — | — |
| godoword/wiki/README.md | 一致 | — | 一致 | — | — |
| llm-openai/docs/README.md | — | — | — | — | 一致 |
| llm-openai/wiki/README.md | — | — | — | — | 改 |
| tts-qwen-audio/docs/README.md | — | — | — | — | 一致 |
| tts-qwen-audio/wiki/README.md | — | — | — | — | 改 |

**发现与修复**：
- hash-cache 旧表述 3 处（roadmap:102 / architecture:75 / status:54）→ 白名单模型修正。
- 触发规则 gates/conventions 节广化 + 锚点链接更新 + contributors wiki 表述同步。
- glossary 入表 5 新术语（白名单缓存/新鲜合成/静态资产/角色扮演/回放）。
- storage.md 补 progress_events kernel 哨兵约定。
- **既存漂移（非本次决策相关，全量审查抓获）**：llm-openai/wiki + tts-qwen-audio/wiki 仍写"M1a 骨架，实现进行中"
  但 docs 明确已实现 → 修正为已实现；kernel/wiki "当前为 M1a" → M1b 当前事实。
- 1720/0100 部分精化非整体取代，新条目正文引用澄清，不动旧条目 frontmatter。

## 后果

仓库 docs/wiki 与 M1c 定案一致，hash-cache 旧假设残留清零，既存适配器 wiki 漂移一并修复。
impact-map 无需改（auth-local crate M1c 实现时登记 impact 规则）。kernel-api 契约未变（contract.md 核对一致）。

**二审复核**（趁上下文，抓一审盲点）：发现 roadmap M1c 行 + kernel/docs 差异节"淘汰前进度提取 M1c"措辞模糊，
可能误导 M1c agent 建 LLM 提取逻辑（实属 M2 评估模块）→ 精化为"淘汰前 span 入 outbox M1c（LLM 提取 M2）"。
全文"提取"表述核对一致（architecture/roadmap 黄金路径皆归评估模块）。缓存默认值（TTL 30 天/容量 1GB）、
detach 上限（4）、cookie 属性跨文档一致；核对矩阵 21 文件完整；history 链接/锚点 gate 验证通过。二审无其他分歧。
详见 [gates](../docs/project/gates.md)、[conventions](../docs/project/conventions.md)。