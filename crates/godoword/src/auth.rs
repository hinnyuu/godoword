//! 认证 HTTP 层（M1c phase 5）：per-request AuthUser/AdminUser extractor（cookie → 会话令牌校验）、
//! cookie 读写（HttpOnly/SameSite=Lax/Secure 可配）、CSRF 自定义头守卫、登录限流（内存）、
//! auth/admin REST。验身/注册/令牌逻辑在 auth-local crate；本模块只做 HTTP 接线。

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, FromRequestParts, Path, State};
use axum::http::{header, request::Parts, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use ipnet::IpNet;
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

/// 登录限流（内存，username + IP 双维度，窗口 5 分钟最多 10 次；单容器重启重置可接受）。
/// **过期窗口清理 + 容量上限（审计 R9.2）**：防 username 键无限增长致内存膨胀。
pub struct RateLimiter {
    inner: Mutex<HashMap<String, (u32, Instant)>>,
    window: Duration,
    max_entries: usize,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            window: Duration::from_secs(300),
            max_entries: 8192,
        }
    }
}

impl RateLimiter {
    /// 新建限流器。
    pub fn new() -> Self {
        Self::default()
    }
    /// 尝试一次（true 允许 / false 超限）。超容量先清过期窗口；仍满则新键 fail-closed（保密禁，审计 N5）。
    pub fn check(&self, key: &str) -> bool {
        let mut g = self.inner.lock().unwrap();
        let now = Instant::now();
        if g.len() >= self.max_entries {
            self.prune_locked(&mut g, now);
        }
        // 容量满且为新键 → fail-closed 拒绝（绝不淘汰窗口内活跃封禁，审计 N5：防洪泛逐出目标封禁绕过）。
        if g.len() >= self.max_entries && !g.contains_key(key) {
            return false;
        }
        let entry = g.entry(key.to_string()).or_insert((0, now));
        if now.duration_since(entry.1) > self.window {
            *entry = (0, now);
        }
        if entry.0 >= 10 {
            return false;
        }
        entry.0 += 1;
        true
    }
    /// 仅清理过期窗口；**窗口内活跃封禁绝不淘汰**（审计 N5：容量满时新键 fail-closed，而非逐出活跃项）。
    fn prune_locked(&self, g: &mut HashMap<String, (u32, Instant)>, now: Instant) {
        g.retain(|_, (_, start)| now.duration_since(*start) <= self.window);
    }
    /// 当前键数（测试/观测用）。
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
    /// 是否为空。
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 解析可信代理配置项为 IpNet（支持裸 IP 与 CIDR；非法项跳过，审计 R9.4）。
pub fn parse_trusted(s: &str) -> Option<IpNet> {
    if let Ok(net) = s.parse::<IpNet>() {
        return Some(net);
    }
    s.parse::<IpAddr>().ok().map(IpNet::from)
}

/// 解析客户端 IP（审计 R9.4）：对端非可信代理 → 直接用对端 IP（不采 XFF，防伪造）；
/// 对端是可信代理 → 从 X-Forwarded-For **右往左取第一个不在可信集的 IP**（最右不可信跳）。
/// 无可信 XFF 跳则回退对端。防 IP 限流在反代后退化为全局限流。
pub fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted: &[IpNet]) -> IpAddr {
    let is_trusted = |ip: &IpAddr| trusted.iter().any(|net| net.contains(ip));
    if !is_trusted(&peer) {
        return peer;
    }
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        for part in xff.split(',').rev() {
            if let Ok(ip) = part.trim().parse::<IpAddr>() {
                if !is_trusted(&ip) {
                    return ip;
                }
            }
        }
    }
    peer
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
    headers: HeaderMap,
    Json(req): Json<RegisterReq>,
) -> Result<Response, StatusCode> {
    let ip = client_ip(addr.ip(), &headers, &st.trusted_proxies);
    if !st.rate_limiter.check(&format!("reg:{}", req.username))
        || !st.rate_limiter.check(&format!("reg-ip:{ip}"))
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
    headers: HeaderMap,
    Json(req): Json<LoginReq>,
) -> Result<Response, StatusCode> {
    let ip = client_ip(addr.ip(), &headers, &st.trusted_proxies);
    if !st.rate_limiter.check(&format!("login:{}", req.username))
        || !st.rate_limiter.check(&format!("login-ip:{ip}"))
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
    // token 化（审计 C8）：管理员生成重置令牌，用户持令牌自行设新密码。无本地凭据账户拒绝（N4 → 400）。
    let token = auth_local::create_reset_token(&st.store, target)
        .await
        .map_err(auth_err_status)?;
    Ok(Json(json!({ "reset_token": token })))
}

/// 用户持重置令牌设新密码（单次/过期校验；无需登录，令牌即凭据；IP 限流防放大，审计 N4）。
#[utoipa::path(post, path = "/api/v1/auth/reset-password", request_body = ResetPasswordReq, tag = "auth",
    responses((status = 200, body = Value), (status = 400), (status = 429)))]
pub async fn reset_password(
    State(st): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<ResetPasswordReq>,
) -> Result<Json<Value>, StatusCode> {
    let ip = client_ip(addr.ip(), &headers, &st.trusted_proxies);
    if !st.rate_limiter.check(&format!("reset-ip:{ip}")) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let ok = auth_local::reset_with_token(&st.store, &req.token, &req.new_password)
        .await
        .map_err(auth_err_status)?;
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
    // 守卫 + 禁用串行化原子（审计 R7）：防自禁/防禁最后管理员/并发互禁不失锁，收口 auth-local 事务。
    match auth_local::set_user_status_guarded(&st.store, admin, target, &req.status).await {
        Ok(true) => Ok(Json(json!({ "ok": true }))),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(e) => Err(auth_err_status(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_blocks_after_10() {
        let rl = RateLimiter::new();
        for _ in 0..10 {
            assert!(rl.check("k"));
        }
        assert!(!rl.check("k"), "第 11 次超限");
    }

    #[test]
    fn rate_limiter_prunes_expired() {
        let rl = RateLimiter {
            inner: Mutex::new(HashMap::new()),
            window: Duration::from_millis(1),
            max_entries: 100,
        };
        rl.check("a");
        assert_eq!(rl.len(), 1);
        std::thread::sleep(Duration::from_millis(5));
        let now = Instant::now();
        let mut g = rl.inner.lock().unwrap();
        rl.prune_locked(&mut g, now);
        assert_eq!(g.len(), 0, "R9.2：过期窗口清理");
        drop(g);
        assert!(rl.is_empty(), "R9.2：清理后为空");
    }

    #[test]
    fn rate_limiter_capacity_cap() {
        let rl = RateLimiter {
            inner: Mutex::new(HashMap::new()),
            window: Duration::from_secs(300),
            max_entries: 16,
        };
        for i in 0..16 {
            rl.check(&format!("k{i}"));
        }
        assert_eq!(rl.len(), 16);
        // 容量满 → 新键 fail-closed（不插入，N5）。
        assert!(!rl.check("overflow"), "N5：容量满新键拒绝");
        assert!(rl.len() <= 16, "N5：容量上限（实得 {}）", rl.len());
    }

    #[test]
    fn rate_limiter_flood_preserves_active_ban() {
        let rl = RateLimiter {
            inner: Mutex::new(HashMap::new()),
            window: Duration::from_secs(300),
            max_entries: 8,
        };
        // 封禁 target（10 次允许，第 11 次拒绝）。
        for _ in 0..10 {
            assert!(rl.check("target"));
        }
        assert!(!rl.check("target"), "target 被封禁");
        // 洪泛大量不同新键填满容量（新键 fail-closed，不逐出活跃项）。
        for i in 0..100 {
            rl.check(&format!("flood{i}"));
        }
        assert!(rl.len() <= 8, "N5：容量上限");
        // target 活跃封禁仍在（洪泛未重置其计数）。
        assert!(
            !rl.check("target"),
            "N5：洪泛后 target 仍封禁（活跃计数不被逐出重置）"
        );
    }

    #[test]
    fn client_ip_untrusted_peer_ignores_xff() {
        let trusted: Vec<IpNet> = vec![parse_trusted("10.0.0.1").unwrap()];
        let peer: IpAddr = "203.0.113.9".parse().unwrap();
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        assert_eq!(
            client_ip(peer, &h, &trusted),
            peer,
            "R9.4：非可信对端不采 XFF"
        );
    }

    #[test]
    fn client_ip_trusted_proxy_rightmost_untrusted() {
        let trusted: Vec<IpNet> = vec!["10.0.0.0/8".parse().unwrap()];
        let peer: IpAddr = "10.0.0.1".parse().unwrap();
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.7, 10.0.0.2".parse().unwrap());
        assert_eq!(
            client_ip(peer, &h, &trusted),
            "203.0.113.7".parse::<IpAddr>().unwrap(),
            "R9.4：最右不可信跳"
        );
    }

    #[test]
    fn client_ip_no_xff_fallback_peer() {
        let trusted: Vec<IpNet> = vec![parse_trusted("10.0.0.1").unwrap()];
        let peer: IpAddr = "10.0.0.1".parse().unwrap();
        let h = HeaderMap::new();
        assert_eq!(client_ip(peer, &h, &trusted), peer, "R9.4：无 XFF 回退对端");
    }

    #[test]
    fn parse_trusted_ip_and_cidr() {
        assert!(parse_trusted("10.0.0.1").is_some());
        assert!(parse_trusted("10.0.0.0/8").is_some());
        assert!(parse_trusted("::1").is_some());
        assert!(parse_trusted("not-an-ip").is_none());
    }
}
