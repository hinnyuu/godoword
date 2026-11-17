---
date: 2026-11-17T18:44+08:00
keywords: [cargo-deny, chrono, rustsec, license, webpki-roots, cdla, risk-acceptance, m1a]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1a 依赖合规：CDLA-Permissive-2.0 白名单 + chrono unmaintained 风险接受

## 背景

M1a 引入 reqwest（rustls-tls）等新依赖后，CI rust job 的 cargo-deny 两项失败（本机仅跑
clippy/test 未跑 cargo-deny，故 PR 首跑才暴露）：①webpki-roots 许可证 CDLA-Permissive-2.0
不在白名单；②chrono unmaintained advisory（RUSTSEC-2026-0543）。主开发者裁决两项均认可。

## 决策

**许可证白名单扩充**：补 `CDLA-Permissive-2.0`（Community Data License Agreement Permissive 2.0）。
唯一被拒包 webpki-roots v1.0.12 是 reqwest rustls-tls 引入的 Mozilla CA 根证书**数据包**（纯数据非代码），
CDLA-Permissive-2.0 为宽松数据许可证（无 copyleft，AGPL 兼容），生态广泛使用。个案审通过，非风险接受。

**chrono unmaintained 书面风险接受**（沿用 SQLite CVE 模式）：deny.toml ignore RUSTSEC-2026-0543。
chrono 维护者宣布退出维护（**维护状态告警，非安全漏洞**），所有版本受影响，无安全升级路径
（官方建议 jiff/time）。sqlx-core 传递依赖 chrono，kernel-api 契约类型直接用 chrono::DateTime——
即便换直接 usage，sqlx 仍带 chrono 入树，替换不可行（需 sqlx 上游切换）。chrono 0.4 成熟稳定、
生态海量使用，风险为未来修复可能不及时（轻于 SQLite CVE 的实际漏洞）。roadmap 远期跟踪：
监控社区 fork/复活或 sqlx 切换 jiff/time，届时迁移并移除忽略。

## 后果

cargo-deny 恢复绿（advisories/licenses）。两项忽略/白名单均有论证注释 + roadmap 跟踪 + 本决策留痕，
非静默绕过门禁。advisory 忽略须主开发者书面认可的纪律再次贯彻（与 SQLite CVE 一致）。