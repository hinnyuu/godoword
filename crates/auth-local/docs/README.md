# auth-local 文档

> 契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M1c + M1c-hardening 加固）

实现 [`kernel_api::AuthProvider`]（验身 seam）+ 注册/邀请/会话令牌/管理员重置 crate 函数（组合根 REST 调用）。
依赖 kernel-api only（适配器纪律）；注册模式由组合根传入 `require_invite`（不依赖 kernel config）。
cookie/CSRF/登录限流属 HTTP 层（组合根 godoword）。数据挂稳定 user_id，与认证方式解耦（history 0600）。

- `LocalAuth{store}`：`id()`="auth-local"、`kind()`=Local、`authenticate(Password{username,password})`
  → `verify_login` → `AuthOutcome::Authenticated(UserId)`；OIDC 请求 → Err（M4 auth-oidc）。
- `hash_password`/`verify_password`：argon2id（OWASP 默认参数，`argon2` crate）。
- `validate_password`（**统一密码策略，审计 R1**，register/reset 共用）：8–128 字符 + 必含字母与数字
  （`is_alphabetic`/`is_numeric`）；区分大小写（绝不折叠）；建议加特殊字符（不强制，提示文案 surface UI）。
  失败复用 `AuthError::RegistrationRestricted`（→400），不改 kernel-api。
- `register(store, username, password, display_name, invite_code, require_invite)`：
  **首用户事务锁成管理员**（SQLite 单写者串行 / PG `pg_advisory_xact_lock`，防竞态，history 0600）；
  username 唯一；require_invite 且非首用户 → 校验邀请码（max_uses/expires）并 used_count+1。
- `verify_login`：JOIN users/local_credentials by username，argon2 校验，status!=active → Disabled。
- 会话令牌：`create_session`（opaque 随机令牌，存 sha256 hash + expires_at）/`validate_session`（**联查 users.status=active**，
  禁用账户旧 cookie 失效，审计 C3）/`delete_session`（登出）/`revoke_user_sessions`（禁用/重置撤销）。
  `auth_sessions` 表（kernel 0002）与单次 `tokens` 分离（可撤销）。
- 邀请码：`create_invite`（明文码返回，存 hash + max_uses + 可选 expires）。
- 管理：`list_users`/`set_user_status`（**禁用撤销会话**）/`is_admin`（**须 active**）/`active_admin_count`（防禁最后管理员）/
  `set_user_status_guarded`（**串行化守卫 + 事务原子，审计 R7**：防自禁/防禁最后活跃管理员/禁用撤销会话；
  PG `pg_advisory_xact_lock` / SQLite 条件 UPDATE 子查询写锁内求值，防并发互禁→零管理员）/
  `admin_reset_password`（设密码 + 撤销会话，**同事务原子 + validate_password**，审计 R1）。
  **重置 token 化（审计 C8）+ 事务原子单次性（审计 R1）**：`create_reset_token`（tokens 表 purpose=password_reset，单次 1h）
  + `reset_with_token`（**先 validate_password**（弱密码不消费令牌）→ 驱动专属事务内**条件 UPDATE 认领**
  （used_at IS NULL 查 rows_affected，仅一个并发请求成功）→ 改密 → 撤销会话 → 提交；任一步失败回滚令牌可重试。
  PG 行锁 / SQLite 单写者保并发安全；管理员不知晓新密码）。

## schema（kernel 0002 迁移）

- `users` 增 `username TEXT`（本地登录标识，可空——OIDC 用户无）+ UNIQUE 索引（可空列两后端允许多 NULL）。
- `auth_sessions(id, user_id→users CASCADE, token_hash, expires_at, created_at)` + hash/user 索引。
- users/local_credentials/invites/tokens 表 M1b 0001 已建。

## 双后端测试（tests/auth.rs，纪律➀）

`backends()` 夹具（SQLite 恒跑 + TEST_PG_URL 驱动 PG，独立 schema 隔离）。验证 argon2 round-trip/错密码、
**密码策略（弱密码/超长拒绝 + 大小写敏感，审计 R1）**、注册首用户成管理员、username 唯一、invite 注册（用尽/过期拒绝）、
登录（禁用拒绝）、会话令牌创建/校验/过期/登出、管理员重置、用户状态、**重置令牌并发仅一次成功 + 弱密码不消费令牌
（审计 R1，独立并发测试 SQLite 文件库 + PG）**、**管理员守卫（自禁/最后管理员拒绝 + 并发互禁后仍≥1 活跃管理员，
审计 R7，独立并发测试）**。