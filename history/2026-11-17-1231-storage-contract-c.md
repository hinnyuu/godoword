---
date: 2026-11-17T12:31+08:00
keywords: [sqlx, any, storage, contract, sqlite, postgresql, dual-backend, audit, spike]
scope: [project, kernel, kernel-api]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 存储契约修订：sqlx Any 证伪 → C 方案双具体驱动

## 背景

独立审查（/tmp 隔离 spike，sqlx 0.8.6 / bundled SQLite 3.46 / PG 16）证伪 M0 的
`ModuleStore = Pool<Any>`：Any 类型集封闭（仅 bool/int/float/str/blob），uuid/chrono/json
编译期即不支持；query! 宏与 AnyPool 互斥；运行时 SQLite BOOLEAN 解码损坏、PG 原生类型读写全须 cast、
last_insert_id 丢失、SQLite 把 `$1::uuid` 静默解析为 Tcl 命名参数。0400"编译期查询"假设与
0930 的 Pool<Any> 契约互斥。契约实现者当前为零，是变更成本全局最低点。

## 决策

**C 方案**：`Store enum { Sqlite(SqlitePool), Pg(PgPool) }`，薄分发宏（execute_both!/fetch_one_both!
等）+ 运行时 query_as，不隐藏方言。PG 真实需要（MySQL 缓议）。两条纪律防 PG 危害稳定：
➀ CI 真测双后端——storage job（postgres service + SQLite）列 main 分支保护 required check，
方言漂移双向捕获；➁ 方言点收口——双兼容 SQL 子集约定：uuid 主键（灭自增）、应用侧时间戳
（灭 DB 默认值）、JSON 整取回 Rust 不在 SQL 过滤（灭 ->>/json_extract）；单一 PG 风格迁移集靠
SQLite 亲和性双后端通用。共享子集放弃 query! 宏（单宏 pin 死单驱动），编译期校验损失由 CI 双测补回；
query! 仅驱动专属。具体驱动双后端 uuid/chrono/json/bool round-trip 已实测健康。

搭车：R2 SessionEvent/ModuleManifest serde 黄金测试；R3 kernel-api 有意耦合 axum/sqlx 声明
（升级视为契约变更）；R4 digest 铁律改"same-tree 内容 + merge 后 main 镜像重跑冒烟"（release 补 smoke）；
R5 gate 术语豁免不可变 history、链接锚点校验、gitleaks 收窄目录豁免为占位符 regex。
**契约级变更全量语义审查制度化**：凡 kernel-api 契约变更强制全量 docs+wiki 审查 + 核对矩阵，防语义漂移。

## 后果

修订 0930 的 ModuleStore 签名（Any→Store enum）；澄清 0400 编译期查询前提（具体驱动下成立，
共享子集用运行时查询）。0400 核心（SQLite+PG、MySQL 缓议）在 C 下存活，不整体取代，旧条目加修订链接。
kernel-api 契约变更同 PR 更新全部模块文档 + 全量审查（门禁）。module.yaml 的 YAML 解析 crate 选型
留 M1 kernel 评估维护良好的方案（serde_yaml 已废弃，测试暂用 JSON 锚定 Deserialize 契约）。