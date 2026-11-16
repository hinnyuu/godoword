---
date: 2026-11-16T08:00+08:00
keywords: [audit, session-protocol, outbox, metering, context-budget, naming, agpl]
scope: [project, kernel-api]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 架构自审补强

## 背景

进入契约设计前全面自审，排查疏漏。核心架构无致命矛盾，疏漏集中在运行时契约与运维细节。

## 决策

补强：会话事件协议须定义（SSE 事件类型/取消/重连）；进度事件须持久 outbox（内存总线重启丢事件
→ 评估盲区），细化早期"事件总线"机制；用量计量 call_log + 预算警告 + 错误归一化（BYOK essential）；
运行时上下文预算 + 淘汰策略；评估冷启动 placement + 版本钉住；认证安全细则（cookie/CSRF/限流/
注册模式/管理员不可读 key）；roadmap 须 committed；测试策略（mock 适配器 + 脱敏 fixture + 人工验收，
CI 不用真 key）；UI i18n zh-CN 首发；转录存+可删；AGPL 页脚声明 + 依赖许可证检查；
数据生命周期（迁移备份/模块退役保留表/级联删除）；运维基线（healthz/优雅退出/日志脱敏/反代 TLS）；
命名规范（适配器 <capability>-<name>，教学 teach-<name>，表 mod_<name>_*，API /api/v1）；
kernel 拆 kernel-api/kernel 防上帝对象。发现 quadlet 需 podman≥4.4 与开发机不符。

## 后果

上述纳入契约与后续里程碑。进度持久 outbox 取代早期纯内存总线设想（见 modular-architecture 条目）。
见 [architecture](../docs/project/architecture.md)、[gates](../docs/project/gates.md)。