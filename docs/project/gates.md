# 门禁

> 关系图谱门禁防止文档漂移。机制设计见 [history](../../history/) 相关决策。
> 诚实局限：门禁防"漏改"，不防"语义写错"；语义一致性靠 agent 自查 + 人工验收。

## 关系图谱

分布式源头：各模块 `module.yaml` 的 `impact` 规则（glob → docs/wiki 清单 + history 要求）
与 `cross_module` 契约边。项目级 [impact-map.yaml](impact-map.yaml) 承载跨模块与基础设施规则
（如 kernel-api 变更牵连全部模块文档）。CI 聚合二者校验。

## CI 检查清单

1. **commitlint**：PR 标题与 commits 符合 conventional（type/scope 英文，subject 中文）。
2. **覆盖率**：`crates/*/src/` 每个文件须命中本 crate impact 规则；
   `scripts/`、`.github/workflows/`、`docker/`、`deploy/`、`web/` 每个文件须命中项目规则。
   新文件未登记 → 红（防图谱腐烂）。Cargo.toml/module.yaml/docs 自身豁免覆盖率。
3. **漏改门禁**：改动文件命中的 docs/wiki 须在同 PR 更新，或显式豁免。
4. **链接检查**：docs/wiki/history 内部相对链接有效。
5. **术语门禁**：扫描 [glossary.yaml](glossary.yaml) 禁用别名（glossary.yaml 自身除外）。
6. **中文门禁**：解释型 markdown 须含 CJK 字符（纯英文文档 → 红）。
7. **禁入路径**：`devenv.md`、`*.key`、`.env*`（除 .env.example）等不得出现在提交。
8. **gitleaks**：密钥扫描。
9. **依赖许可证**：cargo-deny 白名单（MIT/Apache-2.0/BSD/ISC 等），其余个案审。
10. **Rust**：fmt / clippy / check / test。**前端**：lint / build（M1 起）。
11. **构建推送**：镜像推送 `ghcr.io/hinnyuu/godoword:pr-N` 与 `sha-xxxx`。

本地预检：`python3 scripts/docs-gate/gate.py --local`（检查 2–7，无需网络）。

## agent 提交前自查清单

1. `git diff --name-only` 列出改动文件。
2. 对每个改动源文件查 impact 规则，列出应更新的 docs/wiki。
3. 逐一确认已更新，或走豁免流程。
4. 行为/接口/决策变更 → 写 history 条目（[格式](conventions.md#历史格式)）。
5. 新术语 → glossary.yaml 入表。
6. 跑 `gate.py --local` 预检。
7. 确认无密钥、无禁入文件。
8. commit message 合规（type/scope 英文 + subject 中文）。

## 豁免流程

确无需更新文档时：PR 加标签 `docs-nochange` + PR 评论说明理由。门禁记录豁免留痕，
人工验收审视豁免合理性。豁免不是常态，频繁豁免须反思图谱规则是否过宽。

## 图谱维护

新增模块/文件须登记 impact 规则（覆盖率检查强制）。模块退役须清理其规则与跨模块边。
图谱规则本身变更（gates/impact-map/glossary）须更新本文档并记 history。