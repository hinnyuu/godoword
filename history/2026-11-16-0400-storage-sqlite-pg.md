---
date: 2026-11-16T04:00+08:00
keywords: [sqlite, postgresql, mysql, storage, sqlx]
scope: [project, kernel]
type: decision
status: accepted
supersedes: []
superseded_by: [2026-11-17-1231-storage-contract-c.md]
pr: ""
---

# 存储：SQLite 默认 + PostgreSQL 外接

## 背景

单容器 all-in-one 默认 SQLite，用户希望支持外接 PostgreSQL/MySQL。

## 决策

v1 = SQLite（WAL，/data 卷）+ PostgreSQL（外接）。MySQL 缓议：每多一个方言 = 所有查询/迁移的
永久维护税，与长期可维护性冲突；真有需求时再加数据库驱动 crate。模块经 ModuleContext.store()
拿 scoped sqlx pool + mod_<id>_* 表前缀（实用派，不造存储抽象 trait）。
迁移顺序 kernel → 模块注册序，迁移前自动备份。

## 后果

sqlx 编译期查询需 CI DATABASE_URL/offline 数据。MySQL 入 roadmap 远期。
见 [architecture](../docs/project/architecture.md)。