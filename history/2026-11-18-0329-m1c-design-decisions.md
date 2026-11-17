---
date: 2026-11-18T03:29+08:00
keywords: [m1c, session, auth, runtime, continue, detach, eviction, summarize, outbox, call_log, cookie, csrf, argon2, dev-debt, dual-backend]
scope: [project, kernel, godoword, auth-local]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1c 设计决策（会话运行时接基底 + 认证 + 管理）

## 背景

M1b 存储基底合并（9d9c907）后交接 M1c。新 agent 完整探索项目（契约/接缝/回合循环/存储/CI/冒烟）
后提六项设计决策，交 M1b agent 审计（认可无漂移 + 三技术点校正），并交叉验证早期 history
0600-auth-transition / 0800-audit-refinements（auth 姿态、首用户事务锁、持久 outbox/call_log/预算淘汰已定案）。
主旨：长期稳定 + 防漂移优先，准确性优先于省钱。音频复用与角色扮演分层另见同日两条目。

## 决策

**①契约姿态**：不改 kernel-api 契约。auth-local 实现 `AuthProvider::authenticate`（验身 seam），
注册/邀请/管理员重置为 auth-local crate 函数 + 组合根标准端点；`extra_routes` 已被 trait 预留专指 M4 OIDC
（local 用标准端点）。内部接缝 `SessionRepo`（增读 on_disconnect/params/module_id）与 `AudioCache` 扩展非契约，免全量审查。

**②on_disconnect continue 运行时重构**：执行/投递分离——回合 spawn 为 tokio task，经有界 mpsc 发
`SessionEvent`，SSE 订阅；活跃锁归 task（task 结束释放，非 stream drop）。断开 receiver drop → 读
on_disconnect：cancel 置取消标志 / continue 排空事件跑完。detach semaphore 默认 4 可配，try_acquire
拿不到 permit 降级 cancel（硬上限防资源耗尽）。每会话单活跃回合锁 + 显式 cancel 非回归须测试锚定；
优雅退出 cancel 全部活跃回合（含 detached）+ SQLite WAL checkpoint。循环体逻辑保留（零返工）。

**③音频**：默认新鲜合成 + 模块显式白名单（M2）+ 静态资产（M2），取代蓝图指纹 hash 去重假设。
M1c 仅文件系统回放存储（随机 AudioId，每次合成存供回放）+ 缓存有界。详见同日音频白名单条目。

**④认证令牌（方案 A）**：新增 `auth_sessions` 表（0002 迁移，与单次 tokens 表分离——后者留密码重置/邀请/邮箱验证）；
cookie 携 opaque token，服务端 sha256 查表 + expires_at TTL + 登出即删（可撤销）。cookie Secure 可配
（默认 true，冒烟/本地 HTTP 设 false，如 GODOWORD_COOKIE_SECURE）；SameSite=Lax 主 CSRF + 变更请求自定义头纵深防御；
登录限流内存计数（username 主 + IP，单容器重启重置可接受）；首用户成管理员事务锁防竞态（0600 已定案）。
argon2id（argon2 + password-hash，OWASP 参数）。管理员重置 token 化（tokens 表 purpose=password_reset，与 M4 SMTP 自助同机制）。

**⑤全量 summarize 淘汰**：truncate 默认 + summarize opt-in 两者都做。淘汰先把被淘汰 span 发 SessionEvicted
入 outbox；summarize 解析 Summarizer role 绑定调 LLM 压缩成摘要消息置历史前端 + 记 call_log，失败回退 truncate；
mock 测。token 字符启发式（CJK≈1/字、ASCII≈len/4，roadmap 远期换 tokenizer）。默认教师恒 truncate。

**⑥outbox + call_log 计量**：kernel 生命周期事件 `progress_events.module_id` = session.module_id 有则用之、
无则哨兵 `kernel`（新约定，区别 bindings 的 `*` 全模块默认语义）。call_log helper 包裹每次上游调用，
adapter_id 取 `LlmProvider::id()`/`TtsProvider::id()`，测 latency 写全字段。outbox dispatcher 骨架
（tokio task 轮询 processed_at IS NULL → 路由注册模块 handle_event），M1c 无消费者；LLM 提取报告归 teach-assess M2（边界守住）。

**配套**：schema 0001 不动 + 0002 增 auth_sessions；冒烟用户级 REST 503→401 + auth 流；本地真 key 改走真认证
（删 dev-bypass 后 register→provider/binding→回合）；auth-local module.yaml kind=adapter provides=None（auth 不经 bindings）；
messages.seq 单活跃锁内 COALESCE(MAX(seq),0)+1（i32）；ChatMessage↔messages 映射 audio_refs 独立列；
call_log 保留策略留 M2 用量看板；SessionRepo.list M1c 最小元数据（完整列表 M1d）。

## 后果

M1c 设计完备：回归 dev-debt T1（删 dev-bypass）+ T2 余部（会话/音频 DB/文件系统）；认证/管理/计量/淘汰/outbox
落地。不改 kernel-api 契约（免契约级全量审查）；但因音频方向 supersede 蓝图假设，触发设计方向级全量审查
（见同日触发规则条目）。角色扮演/音频白名单为 M2 方向，M1c 机制无关其形态、零返工。
详见 [roadmap](../docs/project/roadmap.md)、[architecture](../docs/project/architecture.md)、[status](../docs/project/status.md)。