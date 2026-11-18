# auth-local 文档

> 契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M1c 已实现）

实现 [`kernel_api::AuthProvider`]（验身 seam）+ 注册/邀请/会话令牌/管理员重置 crate 函数（组合根 REST 调用）。
依赖 kernel-api only（适配器纪律）；注册模式由组合根传入 `require_invite`（不依赖 kernel config）。
cookie/CSRF/登录限流属 HTTP 层（组合根 godoword）。数据挂稳定 user_id，与认证方式解耦（history 0600）。

- `LocalAuth{store}`：`id()`="auth-local"、`kind()`=Local、`authenticate(Password{username,password})`
  → `verify_login` → `AuthOutcome::Authenticated(UserId)`；OIDC 请求 → Err（M4 auth-oidc）。
- `hash_password`/`verify_password`：argon2id（OWASP 默认参数，`argon2` crate）。
- `register(store, username, password, display_name, invite_code, require_invite)`：
  **首用户事务锁成管理员**（SQLite 单写者串行 / PG `pg_advisory_xact_lock`，防竞态，history 0600）；
  username 唯一；require_invite 且非首用户 → 校验邀请码（max_uses/expires）并 used_count+1。
- `verify_login`：JOIN users/local_credentials by username，argon2 校验，status!=active → Disabled。
- 会话令牌：`create_session`（opaque 随机令牌，存 sha256 hash + expires_at）/`validate_session`（未过期→user_id）
  /`delete_session`（登出）。`auth_sessions` 表（kernel 0002 迁移）与单次 `tokens` 表分离（可撤销）。
- 邀请码：`create_invite`（明文码返回，存 hash + max_uses + 可选 expires）。
- 管理：`list_users`/`set_user_status`/`is_admin`/`admin_reset_password`（管理员权限由组合根校验）。

## schema（kernel 0002 迁移）

- `users` 增 `username TEXT`（本地登录标识，可空——OIDC 用户无）+ UNIQUE 索引（可空列两后端允许多 NULL）。
- `auth_sessions(id, user_id→users CASCADE, token_hash, expires_at, created_at)` + hash/user 索引。
- users/local_credentials/invites/tokens 表 M1b 0001 已建。

## 双后端测试（tests/auth.rs，纪律➀）

`backends()` 夹具（SQLite 恒跑 + TEST_PG_URL 驱动 PG，独立 schema 隔离）。验证 argon2 round-trip/错密码、
注册首用户成管理员、username 唯一、invite 注册（用尽/过期拒绝）、登录（禁用拒绝）、会话令牌创建/校验/过期/登出、
管理员重置、用户状态。