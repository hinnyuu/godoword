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
    execute_both, fetch_all_both, fetch_one_both, fetch_optional_both, AuthError, AuthKind,
    AuthOutcome, AuthProvider, AuthRequest, Store, UserId,
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

/// 统一密码策略（注册/重置共用，审计 R1）：8–128 字符 + 必含字母与数字；区分大小写（绝不折叠）。
/// 建议（不强制）加特殊字符——提示文案 surface 到 UI/docs。长度按字符计，上限足以防 argon2 超长 DoS。
pub fn validate_password(password: &str) -> Result<(), AuthError> {
    let n = password.chars().count();
    if !(8..=128).contains(&n) {
        return Err(AuthError::RegistrationRestricted(
            "密码长度须 8–128 字符（区分大小写）".to_string(),
        ));
    }
    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_digit = password.chars().any(|c| c.is_numeric());
    if !has_letter || !has_digit {
        return Err(AuthError::RegistrationRestricted(
            "密码须同时含字母与数字，建议加特殊字符（区分大小写）".to_string(),
        ));
    }
    Ok(())
}

/// 用户名策略（审计 F4 + 二审 R2-3）：trim 后非空、≤64 字符、无控制字符、**首尾无空白**
/// （存储不 trim，首尾空白会造成视觉同形异码登录陷阱，同路径 NFC 教训）。
pub fn validate_username(username: &str) -> Result<(), AuthError> {
    let t = username.trim();
    if t.is_empty() || t.chars().count() > 64 || t.chars().any(|c| c.is_control()) || username != t
    {
        return Err(AuthError::RegistrationRestricted(
            "用户名须 1–64 字符（无控制字符/首尾空白）".to_string(),
        ));
    }
    Ok(())
}

/// 显示名策略（审计 F4）：≤64 字符、无控制字符（可空串=清除显示名）。
pub fn validate_display_name(name: &str) -> Result<(), AuthError> {
    if name.chars().count() > 64 || name.chars().any(|c| c.is_control()) {
        return Err(AuthError::RegistrationRestricted(
            "显示名须 ≤64 字符（无控制字符）".to_string(),
        ));
    }
    Ok(())
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
    validate_username(username)?;
    if let Some(d) = display_name {
        validate_display_name(d)?;
    }
    validate_password(password)?;
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

/// 校验会话令牌（未过期 **且账户 active** → user_id；否则 None。审计 C3：禁用账户旧 cookie 失效）。
pub async fn validate_session(store: &Store, token: &str) -> Result<Option<UserId>, AuthError> {
    let now = Utc::now();
    let row: Option<(Uuid,)> = fetch_optional_both!(
        store,
        "SELECT s.user_id FROM auth_sessions s JOIN users u ON s.user_id = u.id
         WHERE s.token_hash = $1 AND s.expires_at > $2 AND u.status = 'active'",
        (Uuid,),
        token_hash(token),
        now
    )
    .map_err(internal)?;
    Ok(row.map(|(id,)| UserId(id)))
}

/// 撤销用户全部会话（禁用/重置密码时用，审计 C3）。
pub async fn revoke_user_sessions(store: &Store, user: UserId) -> Result<(), AuthError> {
    execute_both!(
        store,
        "DELETE FROM auth_sessions WHERE user_id = $1",
        user.0
    )
    .map_err(internal)?;
    Ok(())
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
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
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

/// 取单个用户信息（不存在 → None）。
pub async fn get_user(store: &Store, user: UserId) -> Result<Option<UserInfo>, AuthError> {
    fetch_optional_both!(
        store,
        "SELECT id, username, display_name, is_admin, status, created_at FROM users WHERE id = $1",
        UserInfo,
        user.0
    )
    .map_err(internal)
}

/// 更新显示名（M1d 账户自助；不存在 → None）。
pub async fn update_display_name(
    store: &Store,
    user: UserId,
    display_name: &str,
) -> Result<Option<UserInfo>, AuthError> {
    validate_display_name(display_name)?;
    let n = execute_both!(
        store,
        "UPDATE users SET display_name = $1, updated_at = $2 WHERE id = $3",
        display_name,
        Utc::now(),
        user.0
    )
    .map_err(internal)?;
    if n == 0 {
        return Ok(None);
    }
    get_user(store, user).await
}

/// 设用户状态（active/disabled）。禁用时撤销全部会话（审计 C3）。
///
/// **遗留无守卫面（审计 O6）**：生产 REST 一律走 [`set_user_status_guarded`]（防自禁/防禁最后管理员/串行化）；
/// 本函数无守卫，仅测试夹具用，新代码不得用于请求路径。
#[doc(hidden)]
pub async fn set_user_status(store: &Store, user: UserId, status: &str) -> Result<bool, AuthError> {
    let n = execute_both!(
        store,
        "UPDATE users SET status = $1, updated_at = $2 WHERE id = $3",
        status,
        Utc::now(),
        user.0
    )
    .map_err(internal)?;
    if n > 0 && status != "active" {
        revoke_user_sessions(store, user).await?;
    }
    Ok(n > 0)
}

/// 是否管理员（须 active；禁用管理员旧 cookie 不通过 AdminUser，审计 C3）。
pub async fn is_admin(store: &Store, user: UserId) -> Result<bool, AuthError> {
    let row: Option<(bool,)> = fetch_optional_both!(
        store,
        "SELECT is_admin FROM users WHERE id = $1 AND status = 'active'",
        (bool,),
        user.0
    )
    .map_err(internal)?;
    Ok(row.map(|(a,)| a).unwrap_or(false))
}

/// 活跃管理员数量（防禁用最后一个管理员致系统失管，审计 C3）。
pub async fn active_admin_count(store: &Store) -> Result<i64, AuthError> {
    let row: (i64,) = fetch_one_both!(
        store,
        "SELECT COUNT(*) FROM users WHERE is_admin = $1 AND status = 'active'",
        (i64,),
        true
    )
    .map_err(internal)?;
    Ok(row.0)
}

/// SQLite 管理员状态守卫体（Transaction Executor lifetime 泛型足够，同 register helper）。
/// 守卫嵌入条件 UPDATE 子查询：首语句即写（获写锁），子查询于写时见最新已提交态 → 并发互禁串行化。
/// 禁用条件 `NOT(目标是活跃管理员 AND 活跃管理员数<=1)`：非最后管理员才禁；rows=0 区分不存在/最后管理员。
async fn set_status_sqlite(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    actor: UserId,
    target: UserId,
    disable: bool,
    now: DateTime<Utc>,
) -> Result<bool, AuthError> {
    if disable && target == actor {
        return Err(AuthError::RegistrationRestricted(
            "不可禁用自己".to_string(),
        ));
    }
    let n = if disable {
        sqlx::query(
            "UPDATE users SET status = 'disabled', updated_at = ? WHERE id = ?
             AND NOT (is_admin AND status = 'active'
                      AND (SELECT COUNT(*) FROM users WHERE is_admin AND status = 'active') <= 1)",
        )
        .bind(now)
        .bind(target.0)
        .execute(&mut **tx)
        .await
        .map_err(internal)?
        .rows_affected()
    } else {
        sqlx::query("UPDATE users SET status = 'active', updated_at = ? WHERE id = ?")
            .bind(now)
            .bind(target.0)
            .execute(&mut **tx)
            .await
            .map_err(internal)?
            .rows_affected()
    };
    if n == 0 {
        if disable {
            let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE id = ?")
                .bind(target.0)
                .fetch_optional(&mut **tx)
                .await
                .map_err(internal)?;
            if exists.is_none() {
                return Ok(false);
            }
            return Err(AuthError::RegistrationRestricted(
                "不可禁用最后一个活跃管理员".to_string(),
            ));
        }
        return Ok(false);
    }
    if disable {
        sqlx::query("DELETE FROM auth_sessions WHERE user_id = ?")
            .bind(target.0)
            .execute(&mut **tx)
            .await
            .map_err(internal)?;
    }
    Ok(true)
}

/// 管理员设用户状态（**串行化守卫 + 事务原子**，审计 R7）：防自禁 / 防禁最后一个活跃管理员 / 禁用撤销会话。
/// 检查与禁用须串行化防并发互禁→零管理员：PG `pg_advisory_xact_lock` / SQLite 条件 UPDATE 子查询（写锁内求值）。
/// Ok(true)=已更新 / Ok(false)=用户不存在 / Err=守卫拒绝（→400）。
pub async fn set_user_status_guarded(
    store: &Store,
    actor: UserId,
    target: UserId,
    status: &str,
) -> Result<bool, AuthError> {
    let disable = status != "active";
    let now = Utc::now();
    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            let r = set_status_sqlite(&mut tx, actor, target, disable, now).await;
            match r {
                Ok(v) => {
                    tx.commit().await.map_err(internal)?;
                    Ok(v)
                }
                Err(e) => Err(e),
            }
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(0x6777_6164_i64) // "gwad"：管理员状态串行化锁（区别注册锁）
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            let row: Option<(bool,)> = sqlx::query_as("SELECT is_admin FROM users WHERE id = $1")
                .bind(target.0)
                .fetch_optional(&mut *tx)
                .await
                .map_err(internal)?;
            let Some((target_admin,)) = row else {
                return Ok(false);
            };
            if disable {
                if target == actor {
                    return Err(AuthError::RegistrationRestricted(
                        "不可禁用自己".to_string(),
                    ));
                }
                if target_admin {
                    let (n,): (i64,) = sqlx::query_as(
                        "SELECT COUNT(*) FROM users WHERE is_admin = $1 AND status = 'active'",
                    )
                    .bind(true)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(internal)?;
                    if n <= 1 {
                        return Err(AuthError::RegistrationRestricted(
                            "不可禁用最后一个活跃管理员".to_string(),
                        ));
                    }
                }
            }
            sqlx::query("UPDATE users SET status = $1, updated_at = $2 WHERE id = $3")
                .bind(status)
                .bind(now)
                .bind(target.0)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            if disable {
                sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
                    .bind(target.0)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
            tx.commit().await.map_err(internal)?;
            Ok(true)
        }
    }
}

/// 管理员重置用户密码（设新密码哈希 + 撤销旧会话，**同事务原子**，审计 R1）。
///
/// **遗留面（审计 O6）**：生产 REST 走 token 化流程（[`create_reset_token`] + [`reset_with_token`]，
/// 管理员不知晓新密码）；本函数仅测试夹具用，新代码不得用于请求路径。
#[doc(hidden)]
pub async fn admin_reset_password(
    store: &Store,
    target: UserId,
    new_password: &str,
) -> Result<bool, AuthError> {
    validate_password(new_password)?;
    let hash = hash_password(new_password)?;
    let now = Utc::now();
    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            let n = sqlx::query(
                "UPDATE local_credentials SET password_hash = ?, updated_at = ? WHERE user_id = ?",
            )
            .bind(&hash)
            .bind(now)
            .bind(target.0)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
            if n > 0 {
                sqlx::query("DELETE FROM auth_sessions WHERE user_id = ?")
                    .bind(target.0)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
            tx.commit().await.map_err(internal)?;
            Ok(n > 0)
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            let n = sqlx::query(
                "UPDATE local_credentials SET password_hash = $1, updated_at = $2 WHERE user_id = $3",
            )
            .bind(&hash)
            .bind(now)
            .bind(target.0)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
            if n > 0 {
                sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
                    .bind(target.0)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
            tx.commit().await.map_err(internal)?;
            Ok(n > 0)
        }
    }
}

/// 管理员生成密码重置令牌（tokens 表 purpose=password_reset，单次，1h 过期；审计 C8 token 化）。
/// 管理员不直接知晓用户新密码——用户持令牌自行重置。**无本地凭据账户拒绝签发**（审计 N4）。
pub async fn create_reset_token(store: &Store, target: UserId) -> Result<String, AuthError> {
    let cred: Option<(Uuid,)> = fetch_optional_both!(
        store,
        "SELECT user_id FROM local_credentials WHERE user_id = $1",
        (Uuid,),
        target.0
    )
    .map_err(internal)?;
    if cred.is_none() {
        return Err(AuthError::RegistrationRestricted(
            "该账户无本地密码，不可重置".to_string(),
        ));
    }
    let token = generate_token();
    let now = Utc::now();
    let expires = now + Duration::hours(1);
    execute_both!(
        store,
        "INSERT INTO tokens (id, user_id, purpose, token_hash, expires_at, used_at, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        Uuid::new_v4(),
        target.0,
        "password_reset",
        token_hash(&token),
        expires,
        Option::<DateTime<Utc>>::None,
        now
    )
    .map_err(internal)?;
    Ok(token)
}

/// 用户持重置令牌设新密码（**事务原子 + 条件认领单次性**，审计 R1）。
/// 先校验密码（弱密码不消费令牌）；事务内条件 UPDATE 认领（used_at IS NULL 查 rows_affected，仅一个并发请求成功）
/// → 改密 → 撤销旧会话 → 提交。任一步失败回滚，令牌未消费可重试。双后端并发安全（PG 行锁 / SQLite 单写者）。
pub async fn reset_with_token(
    store: &Store,
    token: &str,
    new_password: &str,
) -> Result<bool, AuthError> {
    validate_password(new_password)?;
    let now = Utc::now();
    let th = token_hash(token);
    // 廉价预查询（审计 N4.1）：无效令牌直接拒绝，**不触发 Argon2**（公开端点防 CPU/内存放大）。
    // 预查询不替代原子认领——并发单次性仍由事务内条件 UPDATE 保证。
    let valid: Option<(Uuid,)> = fetch_optional_both!(
        store,
        "SELECT id FROM tokens WHERE token_hash = $1 AND purpose = 'password_reset'
         AND used_at IS NULL AND expires_at > $2",
        (Uuid,),
        &th,
        now
    )
    .map_err(internal)?;
    if valid.is_none() {
        return Ok(false);
    }
    let hash = hash_password(new_password)?;
    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            let claimed = sqlx::query(
                "UPDATE tokens SET used_at = ? WHERE token_hash = ? AND purpose = 'password_reset'
                 AND used_at IS NULL AND expires_at > ?",
            )
            .bind(now)
            .bind(&th)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
            if claimed == 0 {
                return Ok(false);
            }
            let (user_id,): (Uuid,) =
                sqlx::query_as("SELECT user_id FROM tokens WHERE token_hash = ?")
                    .bind(&th)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(internal)?;
            let n = sqlx::query(
                "UPDATE local_credentials SET password_hash = ?, updated_at = ? WHERE user_id = ?",
            )
            .bind(&hash)
            .bind(now)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
            if n != 1 {
                // 无本地凭据（审计 N4.2）：回滚令牌认领，重置失败（不消费令牌、不假成功）。
                return Err(AuthError::Internal(
                    "账户无本地凭据，密码重置失败".to_string(),
                ));
            }
            sqlx::query("DELETE FROM auth_sessions WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
            Ok(true)
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            let claimed = sqlx::query(
                "UPDATE tokens SET used_at = $1 WHERE token_hash = $2 AND purpose = 'password_reset'
                 AND used_at IS NULL AND expires_at > $3",
            )
            .bind(now)
            .bind(&th)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
            if claimed == 0 {
                return Ok(false);
            }
            let (user_id,): (Uuid,) =
                sqlx::query_as("SELECT user_id FROM tokens WHERE token_hash = $1")
                    .bind(&th)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(internal)?;
            let n = sqlx::query(
                "UPDATE local_credentials SET password_hash = $1, updated_at = $2 WHERE user_id = $3",
            )
            .bind(&hash)
            .bind(now)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
            if n != 1 {
                return Err(AuthError::Internal(
                    "账户无本地凭据，密码重置失败".to_string(),
                ));
            }
            sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
                .bind(user_id)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
            tx.commit().await.map_err(internal)?;
            Ok(true)
        }
    }
}

/// 自助改密（M1d：验旧密→统一策略→设新密→撤销**其他**会话保留当前，事务原子）。
/// current_token 为当前会话令牌（保留不撤销）；None 则撤销全部。无凭据/旧密错→失败。
pub async fn change_password(
    store: &Store,
    user: UserId,
    old_password: &str,
    new_password: &str,
    current_token: Option<&str>,
) -> Result<bool, AuthError> {
    let row: Option<(String, String)> = fetch_optional_both!(
        store,
        "SELECT c.password_hash, u.status FROM local_credentials c JOIN users u ON c.user_id = u.id
         WHERE c.user_id = $1",
        (String, String),
        user.0
    )
    .map_err(internal)?;
    let Some((hash, status)) = row else {
        return Ok(false);
    };
    if status != "active" {
        return Err(AuthError::Disabled);
    }
    if !verify_password(old_password, &hash) {
        return Err(AuthError::InvalidCredentials);
    }
    validate_password(new_password)?;
    let new_hash = hash_password(new_password)?;
    let now = Utc::now();
    let keep_hash = current_token.map(token_hash);
    match store {
        Store::Sqlite(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            let n = sqlx::query(
                "UPDATE local_credentials SET password_hash = ?, updated_at = ? WHERE user_id = ?",
            )
            .bind(&new_hash)
            .bind(now)
            .bind(user.0)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
            if n != 1 {
                return Err(AuthError::Internal("无本地凭据，改密失败".to_string()));
            }
            match &keep_hash {
                Some(kh) => {
                    sqlx::query("DELETE FROM auth_sessions WHERE user_id = ? AND token_hash != ?")
                        .bind(user.0)
                        .bind(kh)
                        .execute(&mut *tx)
                        .await
                        .map_err(internal)?;
                }
                None => {
                    sqlx::query("DELETE FROM auth_sessions WHERE user_id = ?")
                        .bind(user.0)
                        .execute(&mut *tx)
                        .await
                        .map_err(internal)?;
                }
            }
            tx.commit().await.map_err(internal)?;
        }
        Store::Pg(pool) => {
            let mut tx = pool.begin().await.map_err(internal)?;
            let n = sqlx::query(
                "UPDATE local_credentials SET password_hash = $1, updated_at = $2 WHERE user_id = $3",
            )
            .bind(&new_hash)
            .bind(now)
            .bind(user.0)
            .execute(&mut *tx)
            .await
            .map_err(internal)?
            .rows_affected();
            if n != 1 {
                return Err(AuthError::Internal("无本地凭据，改密失败".to_string()));
            }
            match &keep_hash {
                Some(kh) => {
                    sqlx::query(
                        "DELETE FROM auth_sessions WHERE user_id = $1 AND token_hash != $2",
                    )
                    .bind(user.0)
                    .bind(kh)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
                }
                None => {
                    sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
                        .bind(user.0)
                        .execute(&mut *tx)
                        .await
                        .map_err(internal)?;
                }
            }
            tx.commit().await.map_err(internal)?;
        }
    }
    Ok(true)
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
