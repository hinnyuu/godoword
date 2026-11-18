//! 本地认证适配器（M1c）。argon2id 密码哈希、邀请码注册（首用户事务锁成管理员）、cookie 会话令牌、
//! 管理员重置。实现 [`AuthProvider`] 验身 seam；注册/邀请/会话/重置为 crate 函数（组合根 REST 调用）。
//! cookie/CSRF/登录限流属 HTTP 层（组合根 phase 5）。数据挂稳定 user_id，与认证方式解耦（history 0600）。
//! 依赖 kernel-api only（适配器纪律）；注册模式由组合根传入 require_invite（不依赖 kernel config）。

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use kernel_api::{
    execute_both, fetch_all_both, fetch_optional_both, AuthError, AuthKind, AuthOutcome,
    AuthProvider, AuthRequest, Store, UserId,
};
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// 本地认证适配器（持 Store 供 authenticate 验身）。
pub struct LocalAuth {
    store: std::sync::Arc<Store>,
}

impl LocalAuth {
    /// 新建适配器。
    pub fn new(store: std::sync::Arc<Store>) -> Self {
        Self { store }
    }
}

fn internal(e: impl std::fmt::Display) -> AuthError {
    AuthError::Internal(e.to_string())
}

/// argon2id 密码哈希（OWASP 默认参数）。
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(internal)
}

/// 校验密码（hash 解析失败/不匹配 → false）。
pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(h) => Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok(),
        Err(_) => false,
    }
}

/// 生成 opaque 随机会话令牌（两个 uuid 拼接，足够熵）。
fn generate_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// 令牌 sha256 hex（存储 hash，不存明文令牌）。
fn token_hash(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 注册（首用户事务锁成管理员；require_invite 时校验邀请码；username 唯一）。
/// 事务锁防首用户竞态：SQLite 单写者串行 / PG advisory xact lock（history 0600 定案）。
pub async fn register(
    store: &Store,
    username: &str,
    password: &str,
    display_name: Option<&str>,
    invite_code: Option<&str>,
    require_invite: bool,
) -> Result<UserId, AuthError> {
    if username.is_empty() || password.is_empty() {
        return Err(AuthError::RegistrationRestricted(
            "用户名/密码不可为空".to_string(),
        ));
    }
    let hash = hash_password(password)?;
    let id = Uuid::new_v4();
    let now = Utc::now();
    let display = display_name.unwrap_or(username);

    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            // 用户名唯一。
            let dup: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(&mut *tx)
                .await
                .map_err(internal)?;
            if dup.is_some() {
                return Err(AuthError::RegistrationRestricted(
                    "用户名已存在".to_string(),
                ));
            }
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                .fetch_one(&mut *tx)
                .await
                .map_err(internal)?;
            let is_admin = count.0 == 0;
            if require_invite && !is_admin {
                consume_invite_tx_sqlite(&mut tx, invite_code).await?;
            }
            sqlx::query(
                "INSERT INTO users (id, email, display_name, is_admin, status, created_at, updated_at, username)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(Option::<String>::None)
            .bind(display)
            .bind(is_admin)
            .bind("active")
            .bind(now)
            .bind(now)
            .bind(username)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            sqlx::query(
                "INSERT INTO local_credentials (user_id, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?)",
            )
            .bind(id)
            .bind(&hash)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            // advisory xact lock 串行化注册（防首用户竞态）。
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(0x676f_646f_i64) // "godo"
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            let dup: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE username = $1")
                .bind(username)
                .fetch_optional(&mut *tx)
                .await
                .map_err(internal)?;
            if dup.is_some() {
                return Err(AuthError::RegistrationRestricted(
                    "用户名已存在".to_string(),
                ));
            }
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                .fetch_one(&mut *tx)
                .await
                .map_err(internal)?;
            let is_admin = count.0 == 0;
            if require_invite && !is_admin {
                consume_invite_tx_pg(&mut tx, invite_code).await?;
            }
            sqlx::query(
                "INSERT INTO users (id, email, display_name, is_admin, status, created_at, updated_at, username)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(id)
            .bind(Option::<String>::None)
            .bind(display)
            .bind(is_admin)
            .bind("active")
            .bind(now)
            .bind(now)
            .bind(username)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            sqlx::query(
                "INSERT INTO local_credentials (user_id, password_hash, created_at, updated_at) VALUES ($1, $2, $3, $4)",
            )
            .bind(id)
            .bind(&hash)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
        }
    }
    Ok(UserId(id))
}

async fn consume_invite_tx_sqlite(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    code: Option<&str>,
) -> Result<(), AuthError> {
    let code = code.ok_or_else(|| AuthError::RegistrationRestricted("需邀请码".to_string()))?;
    let hash = token_hash(code);
    let row: Option<(Uuid, i32, i32, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, max_uses, used_count, expires_at FROM invites WHERE code_hash = ?",
    )
    .bind(&hash)
    .fetch_optional(&mut **tx)
    .await
    .map_err(internal)?;
    let (id, max_uses, used_count, expires_at) =
        row.ok_or_else(|| AuthError::RegistrationRestricted("邀请码无效".to_string()))?;
    check_invite_valid(max_uses, used_count, expires_at)?;
    sqlx::query("UPDATE invites SET used_count = used_count + 1 WHERE id = ?")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    Ok(())
}

async fn consume_invite_tx_pg(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    code: Option<&str>,
) -> Result<(), AuthError> {
    let code = code.ok_or_else(|| AuthError::RegistrationRestricted("需邀请码".to_string()))?;
    let hash = token_hash(code);
    let row: Option<(Uuid, i32, i32, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, max_uses, used_count, expires_at FROM invites WHERE code_hash = $1",
    )
    .bind(&hash)
    .fetch_optional(&mut **tx)
    .await
    .map_err(internal)?;
    let (id, max_uses, used_count, expires_at) =
        row.ok_or_else(|| AuthError::RegistrationRestricted("邀请码无效".to_string()))?;
    check_invite_valid(max_uses, used_count, expires_at)?;
    sqlx::query("UPDATE invites SET used_count = used_count + 1 WHERE id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    Ok(())
}

fn check_invite_valid(
    max_uses: i32,
    used_count: i32,
    expires_at: Option<DateTime<Utc>>,
) -> Result<(), AuthError> {
    if used_count >= max_uses {
        return Err(AuthError::RegistrationRestricted(
            "邀请码已用尽".to_string(),
        ));
    }
    if let Some(exp) = expires_at {
        if Utc::now() > exp {
            return Err(AuthError::RegistrationRestricted(
                "邀请码已过期".to_string(),
            ));
        }
    }
    Ok(())
}

/// 验身（username + password → user_id；禁用账户 → Disabled）。
pub async fn verify_login(
    store: &Store,
    username: &str,
    password: &str,
) -> Result<UserId, AuthError> {
    let row: Option<(Uuid, String, String)> = fetch_optional_both!(
        store,
        "SELECT u.id, c.password_hash, u.status FROM users u
         JOIN local_credentials c ON c.user_id = u.id WHERE u.username = $1",
        (Uuid, String, String),
        username
    )
    .map_err(internal)?;
    let (id, hash, status) = row.ok_or(AuthError::InvalidCredentials)?;
    if status != "active" {
        return Err(AuthError::Disabled);
    }
    if !verify_password(password, &hash) {
        return Err(AuthError::InvalidCredentials);
    }
    Ok(UserId(id))
}

/// 创建会话令牌（返回明文令牌，存储 hash；TTL 过期）。
pub async fn create_session(
    store: &Store,
    user: UserId,
    ttl: Duration,
) -> Result<String, AuthError> {
    let token = generate_token();
    let now = Utc::now();
    let expires = now + ttl;
    execute_both!(
        store,
        "INSERT INTO auth_sessions (id, user_id, token_hash, expires_at, created_at) VALUES ($1,$2,$3,$4,$5)",
        Uuid::new_v4(),
        user.0,
        token_hash(&token),
        expires,
        now
    )
    .map_err(internal)?;
    Ok(token)
}

/// 校验会话令牌（未过期 → user_id；否则 None）。
pub async fn validate_session(store: &Store, token: &str) -> Result<Option<UserId>, AuthError> {
    let now = Utc::now();
    let row: Option<(Uuid,)> = fetch_optional_both!(
        store,
        "SELECT user_id FROM auth_sessions WHERE token_hash = $1 AND expires_at > $2",
        (Uuid,),
        token_hash(token),
        now
    )
    .map_err(internal)?;
    Ok(row.map(|(id,)| UserId(id)))
}

/// 删除会话令牌（登出）。
pub async fn delete_session(store: &Store, token: &str) -> Result<(), AuthError> {
    execute_both!(
        store,
        "DELETE FROM auth_sessions WHERE token_hash = $1",
        token_hash(token)
    )
    .map_err(internal)?;
    Ok(())
}

/// 创建邀请码（返回明文码，存储 hash；max_uses + 可选过期）。
pub async fn create_invite(
    store: &Store,
    created_by: UserId,
    max_uses: i32,
    expires_at: Option<DateTime<Utc>>,
) -> Result<String, AuthError> {
    let code = generate_token();
    let now = Utc::now();
    execute_both!(
        store,
        "INSERT INTO invites (id, code_hash, max_uses, used_count, created_by, expires_at, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        Uuid::new_v4(),
        token_hash(&code),
        max_uses,
        0i32,
        created_by.0,
        expires_at,
        now
    )
    .map_err(internal)?;
    Ok(code)
}

/// 用户信息（管理列表用，不含密码）。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub is_admin: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// 列出用户（管理员用）。
pub async fn list_users(store: &Store) -> Result<Vec<UserInfo>, AuthError> {
    fetch_all_both!(
        store,
        "SELECT id, username, display_name, is_admin, status, created_at FROM users ORDER BY created_at",
        UserInfo
    )
    .map_err(internal)
}

/// 设用户状态（active/disabled，管理员用）。
pub async fn set_user_status(store: &Store, user: UserId, status: &str) -> Result<bool, AuthError> {
    let n = execute_both!(
        store,
        "UPDATE users SET status = $1, updated_at = $2 WHERE id = $3",
        status,
        Utc::now(),
        user.0
    )
    .map_err(internal)?;
    Ok(n > 0)
}

/// 是否管理员。
pub async fn is_admin(store: &Store, user: UserId) -> Result<bool, AuthError> {
    let row: Option<(bool,)> = fetch_optional_both!(
        store,
        "SELECT is_admin FROM users WHERE id = $1",
        (bool,),
        user.0
    )
    .map_err(internal)?;
    Ok(row.map(|(a,)| a).unwrap_or(false))
}

/// 管理员重置用户密码（直接设新密码哈希；管理员权限由组合根校验）。
pub async fn admin_reset_password(
    store: &Store,
    target: UserId,
    new_password: &str,
) -> Result<bool, AuthError> {
    let hash = hash_password(new_password)?;
    let now = Utc::now();
    let n = execute_both!(
        store,
        "UPDATE local_credentials SET password_hash = $1, updated_at = $2 WHERE user_id = $3",
        &hash,
        now,
        target.0
    )
    .map_err(internal)?;
    Ok(n > 0)
}

#[async_trait]
impl AuthProvider for LocalAuth {
    fn id(&self) -> &'static str {
        "auth-local"
    }
    fn kind(&self) -> AuthKind {
        AuthKind::Local
    }
    async fn authenticate(&self, req: AuthRequest) -> Result<AuthOutcome, AuthError> {
        match req {
            AuthRequest::Password { username, password } => {
                let user = verify_login(&self.store, &username, password.expose_secret()).await?;
                Ok(AuthOutcome::Authenticated(user))
            }
            _ => Err(AuthError::Oidc("auth-local 不支持 OIDC".to_string())),
        }
    }
}
