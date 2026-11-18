-- M1c 认证 schema（0002，增量不动 0001）。双兼容 SQL 子集。
-- users 增 username（本地登录标识，可空——OIDC 用户无 username）；UNIQUE 索引可空列两后端皆允许多 NULL。
-- auth_sessions：cookie 会话令牌（opaque token 的 sha256 hash + 过期），与单次 tokens 表分离（可撤销）。

ALTER TABLE users ADD COLUMN username TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users (username);

CREATE TABLE IF NOT EXISTS auth_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_hash ON auth_sessions (token_hash);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_user ON auth_sessions (user_id);