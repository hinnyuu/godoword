# 协作者指南

> 面向协助开发者与 agent。开发规范与门禁的权威定义在 [docs/project](../../docs/project/)，
> 本 wiki 提供人类向的入门解释。

## 如何阅读本项目

- 先读 [架构](../../docs/project/architecture.md) 理解三层与依赖纪律。
- 写作前查 [术语表](../../docs/project/glossary.yaml)，遵守 [规范](../../docs/project/conventions.md)。
- 提交前过 [门禁自查清单](../../docs/project/gates.md)。
- 决策背景查 [history](../../history/)（当前事实只在 docs）。

## 如何新增一个教学模块

1. 在 `crates/` 建 crate，依赖 `kernel-api`，实现 `TeachingModule` trait。
2. 写 `module.yaml`（id/kind/requires/session_params/frontend/impact/cross_module）。
3. 建 `docs/`（agent 向：接口契约、内部设计）与 `wiki/`（人类向：模块是什么、怎么用）。
4. 自有迁移放 `migrations/`，表用 `mod_<id>_*` 前缀。SQL 须守**双兼容 SQL 子集**
   （uuid 主键、应用侧时间戳、JSON 整取回 Rust 不在 SQL 过滤），用 `Store` 薄分发宏 +
   运行时 `query_as`；CI 双后端测试（SQLite + PostgreSQL）兜底，违反子集某后端即红。
5. 前端放 `web/modules/<id>/`，领域逻辑进 `web/core`（框架无关）。
6. 在组合根 `godoword` 以 feature 注册。
7. 登记 impact 规则（覆盖率门禁强制），过 docs-gate。

模块互不 import，跨模块只经持久事件与契约接口。失败模块可整 crate 移除，不影响整体。
改动 kernel-api 契约（trait/Store/协议类型）须走**契约级全量 docs+wiki 语义审查**（见 conventions）。

## 开发环境

本机环境事实见仓库根 `devenv.md`（不入仓库）。重构建走 CI，本机 `cargo check` 为主。

## 验收：自动化冒烟 + 人工残留

确定性 CLI 检查（healthz、uid/卷矩阵、双栈、优雅退出等）由 `scripts/acceptance/smoke.sh`
在 CI 自动跑并自动清理，无需人工。人工只验收自动化覆盖不到的项：webui 视觉交互、TTS 音质、
以及需要其他主机的项（SELinux/quadlet/docker）。merge 前置 = 冒烟绿 + 人工对残留 sign-off。
详见 [conventions](../../docs/project/conventions.md)。