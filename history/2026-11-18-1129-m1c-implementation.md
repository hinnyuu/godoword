---
date: 2026-11-18T11:29+08:00
keywords: [m1c, session, audio, auth, argon2, cookie, csrf, continue, detach, eviction, summarize, outbox, call_log, dev-debt]
scope: [project, kernel, godoword, auth-local]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1c 实现：会话运行时接基底 + 认证 + 管理

## 背景

M1b 存储基底合并后，按定案设计（history 0329 三条目 + 触发规则）实现 M1c。回归 dev-debt T1（删 dev-bypass）
+ T2 余部（会话/音频内存→DB/文件系统）。6 phase 单 PR 多 commit。主旨：长期稳定 + 防漂移，准确性优先于省钱。

## 决策

**实现**（按 phase）：
- **P1 持久化基底**：SessionRepo 内部接缝扩展（SessionMeta/OnDisconnect + create 携带 meta + meta()/history_with_ids，非契约）；
  DbSessions（sessions/messages 双兼容 SQL，seq 单活跃锁内 COALESCE(MAX)+1 i32，外键级联删）；
  FsAudio（/data/cache/<id>.pcm + sidecar，tmp+rename 原子写，TTL/容量上限周期清理防撑满磁盘）；
  Config cache_ttl_secs/cache_max_bytes；main/AppState/REST 转 trait 对象，内存 impl 降测试替身。
- **P2 上下文/计量**：context 字符启发式 token 估算（CJK≈1/字 ASCII≈len/4）+ truncate 装配；runtime 每会话 meta.params
  + summarize 淘汰（Summarizer role 绑定调 LLM 压缩 evicted span，失败回退 truncate）+ 淘汰 span 入 outbox
  （SessionEvicted，module_id 有则用之无则哨兵 kernel）+ call_log 计量所有上游调用（adapter_id 取 trait id()）；
  journal（record_call/emit_progress/pending/mark_processed/dispatch_once + OutboxHandler）+ main dispatcher 骨架（空注册表 M2 消费）。
- **P3 continue 重构**：run_turn 执行/投递分离——执行体 spawn tokio task 经有界 mpsc 投递，SSE 订阅 receiver；
  断开（receiver drop）读 on_disconnect 决策一次：cancel 置标志 / continue 取 detach permit 排空跑完（permit 满降级 cancel）；
  活跃锁归执行体（task 结束释放）；detach semaphore 默认 4（Config max_detached_turns 可配）；cancel_all + 优雅退出接线。
- **P4 auth-local**：新 crate（kind=adapter provides=None，依赖 kernel-api only）——argon2id；register 首用户事务锁成管理员
  （SQLite 单写者 / PG pg_advisory_xact_lock）+ username 唯一 + invite 校验；verify_login（禁用拒绝）；会话令牌
  create/validate/delete（auth_sessions opaque token sha256）；create_invite；list_users/set_user_status/is_admin/admin_reset_password；
  LocalAuth impl AuthProvider。kernel 0002 迁移（users.username + UNIQUE 索引 + auth_sessions）。
- **P5 认证接线**：godoword auth 模块（per-request AuthUser/AdminUser extractor，axum 0.8 原生 async fn trait；
  cookie HttpOnly/SameSite=Lax/Secure 可配；CSRF 自定义头守卫中间件；登录限流内存 10 次/5 分钟；auth/admin REST）；
  AppState 转 per-request（runtime/适配器非 optional，移除固定 user）；**删 dev-bypass（T1）**，Runtime 无条件构建，用户级 REST release 开放（未认证 401）。
- **P6 验收**：冒烟 503→401 + auth 流（注册首用户/cookie/me/会话/providers/CSRF/限流）+ cookie_secure=false（CI HTTP）；文档/history。

**实现教训**：①SQLite :memory: 池连接隔离——跨连接共享须文件库或 max_connections(1)（计量测试用 open_sqlite 文件库）；
②open_sqlite 启用外键——计量测试须 ensure_user 建 users 行避 call_log/outbox user_id FK 违约静默失败；
③ Store 宏需 &Store（&Arc<Store> 须 &*store 解引用），函数调用自动解引用（clippy explicit_auto_deref 区分）；
④axum 0.8 FromRequestParts 用原生 async fn in trait（RPITIT），非 async_trait 属性；⑤doc 注释行首 "+" 触发 clippy doc_lazy_continuation（markdown 列表项）。
⑥新增 history 须 gate.py --index 再生派生索引（CI --check-index）。

**验证**：双后端测试全绿（kernel session/journal/storage/turn/audio/binding/crypto + auth-local，独立 PG schema 隔离避并行竞态）；
clippy/fmt 干净；本地 auth 流端到端实证（首用户成管理员/邀请注册/非管理员 403/登出失效/CSRF 403/限流 429）；
本地真 key 完整对话（走真认证 register→provider/binding→回合+speak）。

## 后果

M1c 成立：会话/音频持久化（卷误删重建零丢失）、认证/管理/计量/淘汰/outbox 落地、continue detach。
dev-debt T1 回归（dev-bypass 删除）+ T2 余部回归（DB/文件系统）。T4（默认教师模式）留 M2。
不改 kernel-api 契约（内部接缝扩展免全量审查）。角色扮演/白名单缓存 M2 方向，M1c 机制无关其形态、零返工。
详见 [kernel/docs](../crates/kernel/docs/README.md)、[godoword/docs](../crates/godoword/docs/README.md)、
[auth-local/docs](../crates/auth-local/docs/README.md)、[architecture](../docs/project/architecture.md)。