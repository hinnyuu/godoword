//! 会话 REST + SSE 传输 + 系统端点（healthz/readyz/meta/config）。
//! POST /messages 响应即该回合 SessionEvent 流（SSE 不入 OpenAPI，单独协议文档）。
//! 用户级端点 release M1b 由 dev-bypass 把守（user_access，M1c 认证后开放）。

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use kernel::{
    AudioCache, Config, ConfigView, MasterKey, MemAudio, MemSessions, Runtime, SessionRepo,
};
use kernel_api::{AudioId, LlmProvider, SessionEvent, SessionId, Store, TtsProvider, UserId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

/// 应用状态（M1b：内存会话/音频 + DB 存储基底 + 配置；runtime/适配器仅 dev-bypass 模式存在）。
pub struct AppState {
    /// 会话仓储。
    pub sessions: Arc<MemSessions>,
    /// 音频缓存。
    pub audio: Arc<MemAudio>,
    /// 运行时（dev-bypass 模式 Some；release M1b None，回合返回 503）。
    pub runtime: Option<Arc<Runtime>>,
    /// 当前用户（M1b dev 固定用户；M1c 真认证）。
    pub user: UserId,
    /// 存储基底（迁移已应用；readyz 探活 + 绑定解析）。
    pub store: Arc<Store>,
    /// 部署级配置（/config 脱敏只读）。
    pub cfg: Config,
    /// 主密钥（provider key 加解密）。
    pub master: Arc<MasterKey>,
    /// LLM 适配器（models 测试端点；release M1b None）。
    pub llm: Option<Arc<dyn LlmProvider>>,
    /// TTS 适配器（voices 测试端点；release M1b None）。
    pub tts: Option<Arc<dyn TtsProvider>>,
}

/// 用户级 REST 访问门：release M1b 无认证，需 dev-bypass（M1c 真认证中间件替换）。
pub(crate) fn user_access() -> Result<(), StatusCode> {
    if cfg!(feature = "dev-bypass") {
        Ok(())
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
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
        "milestone": "M1b",
        "dev_bypass": cfg!(feature = "dev-bypass"),
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

/// 创建会话。
#[utoipa::path(post, path = "/api/v1/sessions", tag = "sessions", responses((status = 200, body = Value), (status = 503)))]
pub async fn create_session(State(st): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    user_access()?;
    let id = SessionId::new();
    let _ = st.sessions.create(st.user, id).await;
    Ok(Json(json!({ "id": id.to_string() })))
}

/// 列出用户会话。
#[utoipa::path(get, path = "/api/v1/sessions", tag = "sessions", responses((status = 200, body = Value), (status = 503)))]
pub async fn list_sessions(State(st): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    user_access()?;
    let ids = st.sessions.list(st.user).await.unwrap_or_default();
    Ok(Json(json!({
        "sessions": ids.iter().map(|id| json!({ "id": id.to_string() })).collect::<Vec<_>>()
    })))
}

/// 读会话转录。
#[utoipa::path(get, path = "/api/v1/sessions/{id}", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 200, body = Value), (status = 404), (status = 503)))]
pub async fn get_session(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    user_access()?;
    let sid = SessionId(id);
    if !st.sessions.exists(sid).await.unwrap_or(false) {
        return Err(StatusCode::NOT_FOUND);
    }
    let msgs = st.sessions.history(sid).await.unwrap_or_default();
    Ok(Json(json!({ "id": id.to_string(), "messages": msgs })))
}

/// 删除会话（含转录）。
#[utoipa::path(delete, path = "/api/v1/sessions/{id}", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 204), (status = 503)))]
pub async fn delete_session(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    user_access()?;
    let _ = st.sessions.delete(SessionId(id)).await;
    Ok(StatusCode::NO_CONTENT)
}

/// 回合消息请求体。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct MessageReq {
    /// 用户消息文本。
    pub text: String,
}

/// 发消息 → 该回合 SSE 事件流（SSE 不入 OpenAPI schema）。
#[utoipa::path(post, path = "/api/v1/sessions/{id}/messages", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), request_body = MessageReq, responses((status = 200, description = "SessionEvent SSE 流", content_type = "text/event-stream"), (status = 503)))]
pub async fn messages(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<MessageReq>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    user_access()?;
    let rt = st.runtime.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let stream = rt.run_turn(st.user, SessionId(id), req.text);
    Ok(Sse::new(stream.map(|ev| Ok::<_, Infallible>(to_sse(ev)))))
}

/// 显式取消活跃回合。
#[utoipa::path(post, path = "/api/v1/sessions/{id}/cancel", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 200, body = Value), (status = 503)))]
pub async fn cancel(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    user_access()?;
    if let Some(rt) = &st.runtime {
        rt.cancel(SessionId(id));
    }
    Ok(Json(json!({ "ok": true })))
}

/// 回放缓存音频（裸 PCM）。
#[utoipa::path(get, path = "/api/v1/audio/{id}", tag = "sessions", params(("id" = Uuid, Path, description = "音频 id")), responses((status = 200, description = "裸 PCM", content_type = "application/octet-stream"), (status = 404), (status = 503)))]
pub async fn get_audio(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Response, StatusCode> {
    user_access()?;
    let cached = st
        .audio
        .get(AudioId(id))
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
