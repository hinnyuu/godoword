---
date: 2026-11-18T22:41+08:00
keywords: [m1c, second-hardening, transaction, reset-token, password-policy, eviction, idempotency, backpressure, openapi, trusted-proxy, audit]
scope: [project, kernel, godoword, auth-local]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1c 二审收尾加固：全量审计核实 + 设计分歧定案

## 背景

专门加固 agent 接续（main 7fe8bc3，PR #14 交接）。先全量读后端源码（~5450 行）+ 治理文档，
客观核验二审 R1–R9 **全部属实**（逐项 file:line 证据），基线全绿（gate/test_gate 6-6/index/cargo check/
kernel+auth-local SQLite 测试）。另独立发现二审未提的 A1–A6。就 5 个设计分歧与主开发者讨论后定案。

审计核实要点：R1 重置 SELECT→改密→UPDATE 无事务（auth-local lib.rs:492-513）+ 空密码（reset 无校验，
register:77-81 有，不对称）；R2 audio.put `let _ =` 后无条件 AudioEnd/成功 ToolResult（runtime.rs:864-892）；
R3 shutdown timeout 仅 drop handles=detach 非 abort（runtime.rs:331-344）+ 硬超时后满 mpsc `send().await` 再卡
（runtime.rs:413-428）；R4 emit `let _ =` 与 update_eviction 独立（runtime.rs:593-615），summarize 计费先于两者无幂等；
R5 CreateSessionReq 未注册（openapi.rs:52-69）+ 无 securitySchemes + 变更端点普遍缺 403/404；
R6 SessionEnded emit 吞错 + delete 独立（rest.rs:177-198）；R7 管理员 is_admin→count→set_status 三步独立
（godoword auth.rs:391-403）；R8 test_gate 未接 CI（ci.yaml:13-29）；R9 旧 sidecar owner 必填无 default（audio.rs:30）+
限流器只增不清（auth.rs:114-139）+ 反代 IP 直用 ConnectInfo（auth.rs:209/252）。R9.3 迁移数陈旧已在 #14 修复。

独立新增：A1 满 mpsc 阻塞致 exec/ActiveGuard 永不 drop → 会话永久 busy（加重 R3.2）；
A2 migrate.rs:142-156 多语句迁移无事务 → 半应用损坏风险；A3 FsAudio.put 先 PCM 后 sidecar 半写留孤儿；
A4 R5 缺 403/404 是系统性（所有变更端点）非仅 CreateSessionReq；A5 outbox handle 成功+mark 失败重投（M2 消费幂等）；
A6 ensure_user pub 建 is_admin=true（仅测试用未接生产，可选收窄）。

## 决策

**分歧 1（事务策略）= (a) 驱动专属事务块**：沿用 register 模式（match Store::Sqlite/Pg + pool.begin()），
R1/R4/R6/R7 各原子操作写驱动专属事务块。**不改 kernel-api**（不加 Tx helper），最保守防漂移；接受 ~4 处重复。

**分歧 2（R4 计费幂等）= 新表 + 迁移原子性**：新增 `summary_spans(span_hash TEXT PK, session_id UUID,
fragment TEXT, created_at TIMESTAMPTZ)`（迁移 0004）。summarize LLM 调用前查 span_hash（evicted 消息 ID 集哈希）
命中则复用 fragment 不计费；调用后落 fragment，再与 emit_progress+update_eviction 同事务。**顺带修 A2**：
migrate.rs 每条迁移包事务（防多语句半应用）。

**分歧 3（R9 可信代理）= Config.trusted_proxies**：`Vec<String>`（IP/CIDR）+ env `GODOWORD_TRUSTED_PROXIES`；
XFF 取最右不可信跳客户端 IP；未配则不信任 XFF（直连语义）；ConfigView 暴露。

**分歧 4（密码策略）= 统一强制 + 建议**：auth-local 新增 `validate_password`，register 与 reset 共用。
强制：非空 + 长度 **8–128 字符**（按字符计，上限足以防 argon2 超长 DoS）+ **必须同时含字母与数字**（各至少一，
`char::is_alphabetic`/`is_numeric`，含 Unicode）。**建议（不强制）**：特殊字符+字母+数字组合——surface 到
错误提示/docs/wiki，M1d UI 显示。**区分大小写**：绝不折叠大小写（argon2 天然）。校验失败复用
`AuthError::RegistrationRestricted`（→400），**不改 kernel-api**。

**分歧 5（范围）= 本期纳入 A2/A3/A4，A5 留 M2，A6 可选**：A2 搭 0004 顺手修；A3 入 R2 故障注入；
A4 入 R5 系统性标注。A5 outbox 消费幂等无消费者无法有意义测试，留 M2（R4 稳定事件 ID 铺路）。A6 ensure_user 收窄可选。

## 后果

单 PR 多 commit，分支 feat/m1c-second-hardening，P0→P1→P2，**每项配失败/并发负例测试**（机械锚定）：
①R1 重置事务+密码校验+双后端并发仅一次负例 → R7 管理员串行化+并发互禁负例；
②R2 音频失败语义+故障注入（含 A3）→ R3 真 abort+join+try_send/关通道+满 mpsc/shutdown 回归（含 A1）；
③R4 emit+水位同事务+0004 幂等表（含 A2）+summarize 多回合/各失败点测试 → R6 delete 原子；
④R5 OpenAPI 系统性标注+$ref 全解析测试（A4）→ R8 test_gate 接 CI；
⑤R9 sidecar 升级处理+限流器清理+可信代理。
收尾：全状态面 reconcile + 本条目 + R4 重测 + 人工残留 sign-off + 二审复核无新回归 → 恢复"M1c 收尾完成无阻塞" → M1d。
文档精炼原则（history 2127）：加固沉淀测试/code/history，live 文档只记当前事实，status 只留开放项。