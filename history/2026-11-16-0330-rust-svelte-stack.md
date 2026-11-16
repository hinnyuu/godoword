---
date: 2026-11-16T03:30+08:00
keywords: [rust, cargo, svelte, vite, frontend, stack]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 技术栈：Rust 后端 + Svelte 前端

## 背景

用户偏好 Rust；需长期 agent 可维护、模块可替换、私有部署极简。前端框架需评估更换代价。

## 决策

后端 Rust：cargo workspace 强制模块边界，编译器守护 agent 重构，单静态二进制 → 极小镜像，
sqlx（SQLite/PG），axum/tokio/reqwest 做 SSE 代理。代价：编译时间、借用检查迭代成本、前端仍需 TS。
前端 Svelte 5 + Vite + TS 纯 SPA（axum 伺服静态，不用 SvelteKit/SSR）。
换框架代价解剖：后端零影响，web/core 框架无关 TS 存活，视图层重写可按模块增量。
纪律：领域逻辑不进组件；后端 OpenAPI（utoipa）→ 前端类型生成。语言学习 UI 对框架专属组件库依赖低。

## 后果

重构建走 CI（开发机 2H2G）。Svelte 为有界可逆决策。见 [architecture](../docs/project/architecture.md)。