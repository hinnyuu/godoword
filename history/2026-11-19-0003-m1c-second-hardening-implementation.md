---
date: 2026-11-19T00:03+08:00
keywords: [m1c, second-hardening, implementation, transaction, password-policy, eviction, idempotency, backpressure, openapi, trusted-proxy, executor-lifetime]
scope: [project, kernel, godoword, auth-local]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1c 二审收尾加固实现（R1–R9 + A2/A3）

## 背景

按 history 2241 定案实现二审收尾加固。分支 feat/m1c-second-hardening，单 PR 多 commit，每项配失败/并发负例测试。
全量 workspace 双后端测试绿（SQLite + PG），fmt/clippy --workspace -D warnings 净，gate/test_gate 6-6 绿。

## 决策（实现要点与精化）

- **R1**：auth-local `validate_password` 统一 register/reset（8–128 字符 + 必含字母与数字，`is_alphabetic`/`is_numeric`，
  区分大小写绝不折叠，建议特殊字符不强制）；失败复用 `AuthError::RegistrationRestricted`（→400），**不改 kernel-api**。
  `reset_with_token` 驱动专属事务：先校验（弱密码不消费令牌）→ 条件 UPDATE 认领（used_at IS NULL 查 rows_affected）
  → 改密 → 撤销 → 提交。`admin_reset_password` 亦事务原子。
- **R7**：`set_user_status_guarded`（防自禁/防禁最后活跃管理员/禁用撤销）。PG `pg_advisory_xact_lock` 串行化；
  **SQLite 精化**：原拟 BEGIN IMMEDIATE 手动连接，但 `&mut SqliteConnection` 的 sqlx Executor lifetime 泛型不足
  （Send future 报 "Executor is not general enough"）→ 改用 `Transaction`（同 register helper，lifetime 泛型足够）
  + **守卫嵌入条件 UPDATE 子查询**（首语句即写获写锁，子查询写时见最新已提交态，并发互禁串行化）。
  教训：sqlx 共享 Send future 用 Transaction 而非裸连接引用。
- **R2/A3**：runtime `AudioEvent::End` put 失败 → storage Error + ToolResult ok=false（不发假 AudioEnd/成功）；
  FsAudio.put sidecar 失败清理孤儿 PCM。
- **R3/A1**：shutdown cancel → 有界等待 → **超时 abort+join**（真屏障，调用方 checkpoint 无并发写）；
  硬超时处理器 **try_send** 非阻塞（满 mpsc 不永久阻塞，exec drop 释放 ActiveGuard，修会话永久 busy）；`with_turn_timeout`。
- **R4/A2**：`evict_session` outbox+水位同事务（失败整体回滚）；`summary_spans`（0004，span_hash PK + session_id CASCADE）
  计费幂等（lookup 命中复用免重计费 → LLM → store fragment → evict）；migrate 每条迁移同事务（apply_migration）；
  移除被取代的 `SessionRepo.update_eviction`。淘汰测试改 DbSessions（生产同 Store）。
- **R6**：`end_session` 读 module_id（失败关闭）→ session-ended outbox + DELETE 同事务（事件提交后才可见，dispatcher 不提前消费）。
- **R5**：注册 CreateSessionReq；`build_openapi` 程序化集中注入 cookieAuth securityScheme + 全局 security +
  所有 /api/v1 变更操作 CSRF header/403（DRY，与 csrf_guard 一致）；messages/cancel/delete 补 404；
  单元测试递归校验所有本地 $ref 可解析。smoke 增 CreateSessionReq/cookieAuth 断言。
- **R8**：test_gate.py 接 docs-gate CI job。
- **R9**：sidecar owner Option + serde default（旧/损坏 → None 404 可再生）；RateLimiter window/max_entries + prune_locked
  （过期清理 + 容量上限淘汰最旧）；Config.trusted_proxies + env + ConfigView；godoword `client_ip`（非可信对端不采 XFF；
  可信代理取 XFF 最右不可信跳）+ `parse_trusted`（**ipnet** 裸 IP/CIDR）；login/register IP 限流经 client_ip。
  新增 ipnet 直接依赖（MIT OR Apache-2.0，白名单内，原 reqwest 传递依赖）。

## 后果

R1–R9 + A2/A3 代码修复完成，各负例测试双后端锚定（重置并发仅一次/弱密码拒绝/音频无假成功/淘汰各失败点不丢不重/
summarize 多回合+计费幂等/满 mpsc 超时退出/shutdown abort 释放锁/管理员并发互禁不失锁/OpenAPI $ref 全解析/
限流清理+容量/client_ip 可信代理正负例/旧 sidecar 兼容）。架构/status/wiki/部署文档同步至单一事实。
**待验收闭环**：CI 全绿 + R4 same-tree/main 重测 + 人工残留 sign-off + 二审复核无新回归 → 恢复"M1c 收尾完成无阻塞" → M1d。
A5（outbox 消费幂等）留 M2（无消费者无法有意义测试）；A6（ensure_user 收窄）未做（仅测试用未接生产，非缺陷）。