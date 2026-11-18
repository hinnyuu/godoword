//! 认证 HTTP 层（M1c phase 5）：per-request AuthUser/AdminUser extractor（cookie → 会话令牌校验）、
//! cookie 读写（HttpOnly/SameSite=Lax/Secure 可配）、CSRF 自定义头守卫、登录限流（内存）、
//! auth/admin REST。验身/注册/令牌逻辑在 auth-local crate；本模块只做 HTTP 接线。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, FromRequestParts, Path, State};
use axum::http::{header, request::Parts, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use kernel_api::{Store, UserId};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::rest::AppState;

/// 会话 cookie 名。
pub const SESSION_COOKIE: &str = "godoword_session";
/// 会话 TTL（7 天）。
const SESSION_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// CSRF 自定义头（变更请求须带，挡跨站 simple form；SameSite=Lax 为主防御）。
const CSRF_HEADER: &str = "x-requested-with";

/// 已认证用户 extractor（cookie → validate_session → UserId；无效 → 401）。
pub struct AuthUser(pub UserId);

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = StatusCode;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = parse_cookie(&parts.headers, SESSION_COOKIE).ok_or(StatusCode::UNAUTHORIZED)?;
        let user = auth_local::validate_session(&state.store, &token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;
        Ok(AuthUser(user))
    }
}

/// 管理员 extractor（AuthUser + is_admin；非管理员 → 403）。
pub struct AdminUser(pub UserId);

impl FromRequestParts<Arc<AppState>> for AdminUser {
    type Rejection = StatusCode;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        let admin = auth_local::is_admin(&state.store, user)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if !admin {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(AdminUser(user))
    }
}

/// 从 Cookie 头解析指定 cookie 值。
pub fn parse_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in cookie.split(';') {
        let mut kv = pair.trim().splitn(2, '=');
        if kv.next() == Some(name) {
            return kv.next().map(|v| v.to_string());
        }
    }
    None
}

/// Set-Cookie 头（HttpOnly/SameSite=Lax/Path=/; Secure 可配）。
fn set_cookie(token: &str, secure: bool) -> String {
    let mut c = format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
        SESSION_TTL.as_secs()
    );
    if secure {
        c.push_str("; Secure");
    }
    c
}

/// 清除 cookie（登出）。
fn clear_cookie(secure: bool) -> String {
    let mut c = format!("{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0");
    if secure {
        c.push_str("; Secure");
    }
    c
}

/// CSRF 守卫中间件：/api/v1 变更请求（POST/PUT/DELETE/PATCH）须带自定义头。
pub async fn csrf_guard(req: axum::extract::Request, next: Next) -> Response {
    let method = req.method();
    let path = req.uri().path();
    let mutating = matches!(
        *method,
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    );
    if mutating && path.starts_with("/api/v1") && req.headers().get(CSRF_HEADER).is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(req).await
}

/// 登录限流（内存，username 键，窗口 5 分钟最多 10 次；单容器重启重置可接受）。
#[derive(Default)]
pub struct RateLimiter {
    inner: Mutex<HashMap<String, (u32, Instant)>>,
}

impl RateLimiter {
    /// 新建限流器。
    pub fn new() -> Self {
        Self::default()
    }
    /// 尝试一次（true 允许 / false 超限）。
    pub fn check(&self, key: &str) -> bool {
        let mut g = self.inner.lock().unwrap();
        let now = Instant::now();
        let window = Duration::from_secs(300);
        let entry = g.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(entry.1) > window {
            *entry = (0, now);
        }
        if entry.0 >= 10 {
            return false;
        }
        entry.0 += 1;
        true
    }
}

/// 注册请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct RegisterReq {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub invite_code: Option<String>,
}

/// 登录请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

/// 管理员发起密码重置请求（生成重置令牌，审计 C8 token 化）。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ResetReq {
    pub user_id: Uuid,
}

/// 用户持令牌重置密码请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ResetPasswordReq {
    pub token: String,
    pub new_password: String,
}

/// 创建邀请码请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct InviteReq {
    #[serde(default = "default_max_uses")]
    pub max_uses: i32,
}

fn default_max_uses() -> i32 {
    1
}

fn auth_err_status(e: kernel_api::AuthError) -> StatusCode {
    match e {
        kernel_api::AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
        kernel_api::AuthError::Disabled => StatusCode::FORBIDDEN,
        kernel_api::AuthError::RegistrationRestricted(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn user_info(store: &Store, user: UserId) -> Result<Value, StatusCode> {
    let info = auth_local::get_user(store, user)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(serde_json::to_value(info).unwrap_or(json!({})))
}

/// 注册（首用户成管理员；注册模式由配置；成功发会话 cookie）。
#[utoipa::path(post, path = "/api/v1/auth/register", request_body = RegisterReq, tag = "auth",
    responses((status = 200, body = Value), (status = 400), (status = 403), (status = 429)))]
pub async fn register(
    State(st): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<RegisterReq>,
) -> Result<Response, StatusCode> {
    if !st.rate_limiter.check(&format!("reg:{}", req.username))
        || !st.rate_limiter.check(&format!("reg-ip:{}", addr.ip()))
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let require_invite = match st.cfg.registration_mode {
        kernel::RegistrationMode::Disabled => return Err(StatusCode::FORBIDDEN),
        kernel::RegistrationMode::Open => false,
        kernel::RegistrationMode::Invite => true,
    };
    let user = auth_local::register(
        &st.store,
        &req.username,
        &req.password,
        req.display_name.as_deref(),
        req.invite_code.as_deref(),
        require_invite,
    )
    .await
    .map_err(auth_err_status)?;
    let token = auth_local::create_session(
        &st.store,
        user,
        chrono::Duration::seconds(SESSION_TTL.as_secs() as i64),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let info = user_info(&st.store, user).await?;
    Ok((
        [(header::SET_COOKIE, set_cookie(&token, st.cfg.cookie_secure))],
        Json(info),
    )
        .into_response())
}

/// 登录（验身成功发会话 cookie）。
#[utoipa::path(post, path = "/api/v1/auth/login", request_body = LoginReq, tag = "auth",
    responses((status = 200, body = Value), (status = 401), (status = 403), (status = 429)))]
pub async fn login(
    State(st): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<LoginReq>,
) -> Result<Response, StatusCode> {
    if !st.rate_limiter.check(&format!("login:{}", req.username))
        || !st.rate_limiter.check(&format!("login-ip:{}", addr.ip()))
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let user = auth_local::verify_login(&st.store, &req.username, &req.password)
        .await
        .map_err(auth_err_status)?;
    let token = auth_local::create_session(
        &st.store,
        user,
        chrono::Duration::seconds(SESSION_TTL.as_secs() as i64),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let info = user_info(&st.store, user).await?;
    Ok((
        [(header::SET_COOKIE, set_cookie(&token, st.cfg.cookie_secure))],
        Json(info),
    )
        .into_response())
}

/// 登出（删会话令牌 + 清 cookie）。
#[utoipa::path(post, path = "/api/v1/auth/logout", tag = "auth", responses((status = 200, body = Value)))]
pub async fn logout(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(token) = parse_cookie(&headers, SESSION_COOKIE) {
        let _ = auth_local::delete_session(&st.store, &token).await;
    }
    Ok((
        [(header::SET_COOKIE, clear_cookie(st.cfg.cookie_secure))],
        Json(json!({ "ok": true })),
    )
        .into_response())
}

/// 当前用户信息。
#[utoipa::path(get, path = "/api/v1/auth/me", tag = "auth", responses((status = 200, body = Value), (status = 401)))]
pub async fn me(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(user_info(&st.store, user).await?))
}

/// 管理员重置用户密码。
#[utoipa::path(post, path = "/api/v1/admin/reset", request_body = ResetReq, tag = "admin",
    responses((status = 200, body = Value), (status = 401), (status = 403), (status = 404)))]
pub async fn admin_reset(
    State(st): State<Arc<AppState>>,
    AdminUser(_admin): AdminUser,
    Json(req): Json<ResetReq>,
) -> Result<Json<Value>, StatusCode> {
    let target = UserId(req.user_id);
    if auth_local::get_user(&st.store, target)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .is_none()
    {
        return Err(StatusCode::NOT_FOUND);
    }
    // token 化（审计 C8）：管理员生成重置令牌，用户持令牌自行设新密码。
    let token = auth_local::create_reset_token(&st.store, target)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "reset_token": token })))
}

/// 用户持重置令牌设新密码（单次/过期校验；无需登录，令牌即凭据）。
#[utoipa::path(post, path = "/api/v1/auth/reset-password", request_body = ResetPasswordReq, tag = "auth",
    responses((status = 200, body = Value), (status = 400)))]
pub async fn reset_password(
    State(st): State<Arc<AppState>>,
    Json(req): Json<ResetPasswordReq>,
) -> Result<Json<Value>, StatusCode> {
    let ok = auth_local::reset_with_token(&st.store, &req.token, &req.new_password)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ok {
        Ok(Json(json!({ "ok": true })))
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

/// 管理员创建邀请码。
#[utoipa::path(post, path = "/api/v1/admin/invites", request_body = InviteReq, tag = "admin",
    responses((status = 200, body = Value), (status = 401), (status = 403)))]
pub async fn admin_invite(
    State(st): State<Arc<AppState>>,
    AdminUser(admin): AdminUser,
    Json(req): Json<InviteReq>,
) -> Result<Json<Value>, StatusCode> {
    let code = auth_local::create_invite(&st.store, admin, req.max_uses, None)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "code": code, "max_uses": req.max_uses })))
}

/// 管理员列出用户。
#[utoipa::path(get, path = "/api/v1/admin/users", tag = "admin",
    responses((status = 200, body = Value), (status = 401), (status = 403)))]
pub async fn admin_users(
    State(st): State<Arc<AppState>>,
    AdminUser(_admin): AdminUser,
) -> Result<Json<Value>, StatusCode> {
    let users = auth_local::list_users(&st.store)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::to_value(users).unwrap_or(json!([]))))
}

/// 设用户状态请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct StatusReq {
    /// active / disabled。
    pub status: String,
}

/// 管理员设用户状态（禁用撤销会话；防自禁/防禁最后一个管理员，审计 C3）。
#[utoipa::path(post, path = "/api/v1/admin/users/{id}/status", request_body = StatusReq, tag = "admin",
    params(("id" = Uuid, Path, description = "用户 id")),
    responses((status = 200, body = Value), (status = 400), (status = 401), (status = 403), (status = 404)))]
pub async fn admin_set_status(
    State(st): State<Arc<AppState>>,
    AdminUser(admin): AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<StatusReq>,
) -> Result<Json<Value>, StatusCode> {
    if req.status != "active" && req.status != "disabled" {
        return Err(StatusCode::BAD_REQUEST);
    }
    let target = UserId(id);
    if req.status == "disabled" {
        if id == admin.0 {
            return Err(StatusCode::BAD_REQUEST); // 不可禁用自己
        }
        if auth_local::is_admin(&st.store, target)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            let n = auth_local::active_admin_count(&st.store)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if n <= 1 {
                return Err(StatusCode::BAD_REQUEST); // 不可禁用最后一个活跃管理员
            }
        }
    }
    let ok = auth_local::set_user_status(&st.store, target, &req.status)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ok {
        Ok(Json(json!({ "ok": true })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
