---
date: 2026-11-16T02:00+08:00
keywords: [history, markdown, frontmatter, timestamp, glossary, immutable]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 历史文本方案 v2

## 背景

历史需 agent 与人类易读易搜索。曾考虑随仓库提交的数据库。

## 决策

反对数据库（二进制合并冲突、agent 需工具读、git 膨胀），用 markdown + YAML frontmatter。
时间戳精确到分钟 + 显式时区（文件名 YYYY-MM-DD-HHMM-slug.md 避免冒号）。
字段 date/keywords/scope/type/status/supersedes/superseded_by/pr。正文不可变（append-only），
唯一例外：被取代时更新 status/superseded_by（CI 可自动）。取代链防混淆历史/当前。
CI 从 frontmatter 生成 INDEX.md/json。术语表 glossary.yaml（规范词+禁用别名）CI 机械查；
解释型文字用中文（CI 粗检 CJK + agent 自查）；commit subject 中文、rustdoc 中文。

## 后果

结构化查询由派生索引满足。用词统一从纪律变为机械门禁。见 [conventions](../docs/project/conventions.md)。