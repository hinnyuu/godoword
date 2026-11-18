//! 会话 REST + SSE 传输 + 系统端点（healthz/readyz/meta/config）。
//! POST /messages 响应即该回合 SessionEvent 流（SSE path 入 OpenAPI，SessionEvent 事件 schema 单独协议文档）。
//! 用户级端点经 AuthUser extractor 认证（cookie → 会话令牌）+ 对象级归属校验（审计 C1，越权 404）；未认证 401。

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use kernel::{AudioCache, Config, ConfigView, MasterKey, Runtime, SessionMeta, SessionRepo};
use kernel_api::{AudioId, LlmProvider, SessionEvent, SessionId, Store, TtsProvider};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{AuthUser, RateLimiter};

/// 应用状态（M1c：DB 会话/文件系统音频 + 运行时 + 认证；用户经 per-request AuthUser）。
pub struct AppState {
    /// 会话仓储（DB impl）。
    pub sessions: Arc<dyn SessionRepo>,
    /// 音频缓存（文件系统 impl）。
    pub audio: Arc<dyn AudioCache>,
    /// 运行时（回合循环）。
    pub runtime: Arc<Runtime>,
    /// 存储基底（迁移已应用；readyz 探活 + 绑定解析 + 认证）。
    pub store: Arc<Store>,
    /// 部署级配置（/config 脱敏只读）。
    pub cfg: Config,
    /// 主密钥（provider key 加解密）。
    pub master: Arc<MasterKey>,
    /// LLM 适配器。
    pub llm: Arc<dyn LlmProvider>,
    /// TTS 适配器。
    pub tts: Arc<dyn TtsProvider>,
    /// 登录限流器。
    pub rate_limiter: Arc<RateLimiter>,
}

/// 存活探针。
#[utoipa::path(get, path = "/healthz", tag = "system", responses((status = 200, description = "存活", body = Value)))]
pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// 就绪探针：存储连通（SELECT 1）才算就绪。
#[utoipa::path(get, path = "/readyz", tag = "system", responses((status = 200, body = Value), (status = 503, description = "存储未就绪")))]
pub async fn readyz(State(st): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    kernel::ping(&st.store)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(json!({ "status": "ok" })))
}

/// 服务元信息。
#[utoipa::path(get, path = "/api/v1/meta", tag = "system", responses((status = 200, body = Value)))]
pub async fn meta() -> Json<Value> {
    Json(json!({
        "name": "godoword",
        "version": env!("CARGO_PKG_VERSION"),
        "milestone": "M1c",
    }))
}

/// 配置脱敏只读视图。
#[utoipa::path(get, path = "/api/v1/config", tag = "system", responses((status = 200, body = ConfigView)))]
pub async fn config(State(st): State<Arc<AppState>>) -> Json<ConfigView> {
    Json(st.cfg.view())
}

fn to_sse(ev: SessionEvent) -> Event {
    let v = serde_json::to_value(&ev).unwrap_or(json!({}));
    let name = v["event"].as_str().unwrap_or("message").to_string();
    Event::default().event(name).data(v["data"].to_string())
}

/// 创建会话请求（可选配置断开行为与运行时参数，审计 C5：continue 公开可配）。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateSessionReq {
    /// 断开行为 cancel/continue（默认 cancel）。
    #[serde(default)]
    pub on_disconnect: Option<String>,
    /// 运行时参数（预算/淘汰/工具上限，默认值见 SessionParams；JSON 对象）。
    #[serde(default)]
    pub params: Option<Value>,
}

/// 创建会话。
#[utoipa::path(post, path = "/api/v1/sessions", request_body = CreateSessionReq, tag = "sessions", responses((status = 200, body = Value), (status = 400), (status = 401)))]
pub async fn create_session(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    body: Option<Json<CreateSessionReq>>,
) -> Result<Json<Value>, StatusCode> {
    let mut meta = SessionMeta::default();
    if let Some(Json(req)) = body {
        if let Some(od) = req.on_disconnect {
            meta.on_disconnect = match od.as_str() {
                "cancel" => kernel::OnDisconnect::Cancel,
                "continue" => kernel::OnDisconnect::Continue,
                _ => return Err(StatusCode::BAD_REQUEST),
            };
        }
        if let Some(p) = req.params {
            meta.params = serde_json::from_value(p).map_err(|_| StatusCode::BAD_REQUEST)?;
        }
    }
    let id = SessionId::new();
    st.sessions
        .create(user, id, meta)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "id": id.to_string() })))
}

/// 列出用户会话。
#[utoipa::path(get, path = "/api/v1/sessions", tag = "sessions", responses((status = 200, body = Value), (status = 401)))]
pub async fn list_sessions(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> Result<Json<Value>, StatusCode> {
    let ids = st
        .sessions
        .list(user)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "sessions": ids.iter().map(|id| json!({ "id": id.to_string() })).collect::<Vec<_>>()
    })))
}

/// 读会话转录。
#[utoipa::path(get, path = "/api/v1/sessions/{id}", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 200, body = Value), (status = 401), (status = 404)))]
pub async fn get_session(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let sid = SessionId(id);
    if !st
        .sessions
        .exists(user, sid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let msgs = st
        .sessions
        .history(user, sid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "id": id.to_string(), "messages": msgs })))
}

/// 删除会话（含转录）。
#[utoipa::path(delete, path = "/api/v1/sessions/{id}", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 204), (status = 401)))]
pub async fn delete_session(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let sid = SessionId(id);
    if !st
        .sessions
        .exists(user, sid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    // 会话结束发 SessionEnded outbox（审计 C8；module_id 有则用之、无则哨兵 kernel）。
    let module_id = st
        .sessions
        .meta(user, sid)
        .await
        .ok()
        .flatten()
        .and_then(|m| m.module_id.map(|m| m.0))
        .unwrap_or_else(|| "kernel".to_string());
    let _ = kernel::emit_progress(
        &st.store,
        user.0,
        Some(sid.0),
        &module_id,
        "session-ended",
        json!([]),
        json!({}),
    )
    .await;
    st.sessions
        .delete(user, sid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 回合消息请求体。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct MessageReq {
    /// 用户消息文本。
    pub text: String,
}

/// 发消息 → 该回合 SSE 事件流（SSE 不入 OpenAPI schema）。
#[utoipa::path(post, path = "/api/v1/sessions/{id}/messages", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), request_body = MessageReq, responses((status = 200, description = "SessionEvent SSE 流", content_type = "text/event-stream"), (status = 401)))]
pub async fn messages(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<MessageReq>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let sid = SessionId(id);
    if !st
        .sessions
        .exists(user, sid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let stream = st.runtime.run_turn(user, sid, req.text);
    Ok(Sse::new(stream.map(|ev| Ok::<_, Infallible>(to_sse(ev)))))
}

/// 显式取消活跃回合。
#[utoipa::path(post, path = "/api/v1/sessions/{id}/cancel", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 200, body = Value), (status = 401)))]
pub async fn cancel(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let sid = SessionId(id);
    if !st
        .sessions
        .exists(user, sid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    st.runtime.cancel(sid);
    Ok(Json(json!({ "ok": true })))
}

/// 回放缓存音频（裸 PCM）。
#[utoipa::path(get, path = "/api/v1/audio/{id}", tag = "sessions", params(("id" = Uuid, Path, description = "音频 id")), responses((status = 200, description = "裸 PCM", content_type = "application/octet-stream"), (status = 401), (status = 404)))]
pub async fn get_audio(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let cached = st
        .audio
        .get(AudioId(id), user)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::HeaderName::from_static("x-sample-rate"),
                cached.sample_rate.to_string(),
            ),
        ],
        cached.pcm,
    )
        .into_response())
}
