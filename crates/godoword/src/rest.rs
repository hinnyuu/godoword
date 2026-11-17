//! 会话 REST + SSE 传输（M1a）。POST /messages 响应即该回合 SessionEvent 流。
//! SSE 事件标签 = SessionEvent 的 event 字段，data = 其 data 载荷（tag=event/content=data 协议）。

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use kernel::{AudioCache, Config, ConfigView, MemAudio, MemSessions, Runtime, SessionRepo};
use kernel_api::{AudioId, SessionEvent, SessionId, Store, UserId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

/// 应用状态（M1b：内存会话/音频 + DB 存储基底 + 配置；runtime 仅 dev-bypass 模式存在）。
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
}

/// 就绪探针：存储连通（SELECT 1）才算就绪。
pub async fn readyz(State(st): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    kernel::ping(&st.store)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(json!({ "status": "ok" })))
}

/// 配置脱敏只读视图。
pub async fn config(State(st): State<Arc<AppState>>) -> Json<ConfigView> {
    Json(st.cfg.view())
}

fn to_sse(ev: SessionEvent) -> Event {
    let v = serde_json::to_value(&ev).unwrap_or(json!({}));
    let name = v["event"].as_str().unwrap_or("message").to_string();
    Event::default().event(name).data(v["data"].to_string())
}

/// 创建会话。
pub async fn create_session(State(st): State<Arc<AppState>>) -> Json<Value> {
    let id = SessionId::new();
    let _ = st.sessions.create(st.user, id).await;
    Json(json!({ "id": id.to_string() }))
}

/// 列出用户会话。
pub async fn list_sessions(State(st): State<Arc<AppState>>) -> Json<Value> {
    let ids = st.sessions.list(st.user).await.unwrap_or_default();
    Json(json!({
        "sessions": ids.iter().map(|id| json!({ "id": id.to_string() })).collect::<Vec<_>>()
    }))
}

/// 读会话转录。
pub async fn get_session(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let sid = SessionId(id);
    if !st.sessions.exists(sid).await.unwrap_or(false) {
        return Err(StatusCode::NOT_FOUND);
    }
    let msgs = st.sessions.history(sid).await.unwrap_or_default();
    Ok(Json(json!({ "id": id.to_string(), "messages": msgs })))
}

/// 删除会话（含转录）。
pub async fn delete_session(State(st): State<Arc<AppState>>, Path(id): Path<Uuid>) -> StatusCode {
    let _ = st.sessions.delete(SessionId(id)).await;
    StatusCode::NO_CONTENT
}

/// 回合消息请求体。
#[derive(Deserialize)]
pub struct MessageReq {
    /// 用户消息文本。
    pub text: String,
}

/// 发消息 → 该回合 SSE 事件流。
pub async fn messages(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<MessageReq>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let rt = st.runtime.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let stream = rt.run_turn(st.user, SessionId(id), req.text);
    Ok(Sse::new(stream.map(|ev| Ok::<_, Infallible>(to_sse(ev)))))
}

/// 显式取消活跃回合。
pub async fn cancel(State(st): State<Arc<AppState>>, Path(id): Path<Uuid>) -> Json<Value> {
    if let Some(rt) = &st.runtime {
        rt.cancel(SessionId(id));
    }
    Json(json!({ "ok": true }))
}

/// 回放缓存音频（裸 PCM）。
pub async fn get_audio(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Response, StatusCode> {
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
