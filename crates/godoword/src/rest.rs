//! 会话 REST + SSE 传输 + 系统端点（healthz/readyz/meta/config）。
//! POST /messages 响应即该回合 SessionEvent 流（SSE path 入 OpenAPI，SessionEvent 事件 schema 单独协议文档）。
//! 用户级端点经 AuthUser extractor 认证（cookie → 会话令牌）+ 对象级归属校验（审计 C1，越权 404）；未认证 401。

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use kernel::{AudioCache, Config, ConfigView, MasterKey, Runtime, SessionMeta, SessionRepo};
use kernel_api::{
    AudioFormat, AudioId, LlmProvider, SessionEvent, SessionId, Store, TtsProvider, WordTimestamp,
};
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
    /// 模块注册表（M2 P1：会话模块校验/ctx 工厂/模块路由）。
    pub registry: Arc<kernel::ModuleRegistry>,
    /// 登录限流器。
    pub rate_limiter: Arc<RateLimiter>,
    /// 可信反向代理 IP/CIDR（审计 R9.4，启动时从配置解析）：XFF 客户端 IP 解析用。
    pub trusted_proxies: Vec<ipnet::IpNet>,
}

/// 缺省会话模块（T4 决议：自由对话为默认命名空间；feature 关闭时回退无模块 legacy）。
#[cfg(feature = "teach-free")]
pub(crate) const DEFAULT_MODULE: &str = teach_free::TEACH_FREE_ID;
#[cfg(not(feature = "teach-free"))]
pub(crate) const DEFAULT_MODULE: &str = "";

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
        "milestone": "M1d",
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

/// 创建会话请求（可选配置断开行为与运行时参数，审计 C5：continue 公开可配；M2 P1：module_id 可选）。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateSessionReq {
    /// 断开行为 cancel/continue（默认 cancel）。
    #[serde(default)]
    pub on_disconnect: Option<String>,
    /// 运行时参数（预算/淘汰/工具上限，默认值见 SessionParams；JSON 对象）。
    #[serde(default)]
    pub params: Option<Value>,
    /// 教学模块 id（缺省 teach-free；未注册 → 400）。
    #[serde(default)]
    pub module_id: Option<String>,
}

/// 创建会话。
#[utoipa::path(post, path = "/api/v1/sessions", request_body = CreateSessionReq, tag = "sessions", responses((status = 200, body = Value), (status = 400), (status = 401)))]
pub async fn create_session(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    body: Option<Json<CreateSessionReq>>,
) -> Result<Json<Value>, StatusCode> {
    let mut meta = SessionMeta::default();
    let mut req_module: Option<String> = None;
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
        req_module = req.module_id;
    }
    // 模块校验（M2 P1）：显式 module 须已注册；缺省 teach-free（T4 默认命名空间）。
    let module_id = match req_module {
        Some(id) => {
            if !st.registry.contains(&id) {
                return Err(StatusCode::BAD_REQUEST);
            }
            Some(kernel_api::ModuleId(id))
        }
        None => {
            if !DEFAULT_MODULE.is_empty() && st.registry.contains(DEFAULT_MODULE) {
                Some(kernel_api::ModuleId(DEFAULT_MODULE.to_string()))
            } else {
                None
            }
        }
    };
    meta.module_id = module_id.clone();
    let id = SessionId::new();
    st.sessions
        .create(user, id, meta)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // on_session_start 钩子（失败关闭：回滚会话，不留半初始化）。
    if let Some(mid) = &module_id {
        if let Some(r) = st.registry.get(&mid.0) {
            let ctx = r.factory.for_session(user, id);
            if let Err(e) = r.module.on_session_start(ctx.as_ref()).await {
                tracing::warn!("on_session_start 失败（会话回滚）：{e}");
                let _ = st.sessions.delete(user, id).await;
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
    Ok(Json(json!({ "id": id.to_string() })))
}

/// 模块清单项（GET /modules）。
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ModuleListItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
}

/// 模块清单响应。
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ModuleListResp {
    pub modules: Vec<ModuleListItem>,
}

/// 列出已注册教学模块（M2 P1）。
#[utoipa::path(get, path = "/api/v1/modules", tag = "modules", responses((status = 200, body = ModuleListResp), (status = 401)))]
pub async fn list_modules(
    State(st): State<Arc<AppState>>,
    AuthUser(_user): AuthUser,
) -> Json<ModuleListResp> {
    Json(ModuleListResp {
        modules: st
            .registry
            .list()
            .iter()
            .map(|r| ModuleListItem {
                id: r.manifest.id.clone(),
                name: r.manifest.name.clone(),
                version: r.manifest.version.clone(),
                description: r.manifest.description.clone(),
            })
            .collect(),
    })
}

/// 完成会话（M2 P1，完成≠删除）：completed_at + session-ended 事件，转录保留。
/// 幂等：已完成 → 409；不存在/越权 → 404。
#[utoipa::path(post, path = "/api/v1/sessions/{id}/complete", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 200, body = Value), (status = 401), (status = 404), (status = 409)))]
pub async fn complete_session(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let sid = SessionId(id);
    let meta = st
        .sessions
        .meta(user, sid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let module_id = meta
        .module_id
        .as_ref()
        .map(|m| m.0.clone())
        .unwrap_or_else(|| DEFAULT_MODULE.to_string());
    let r = st.registry.get(&module_id).ok_or(StatusCode::NOT_FOUND)?;
    let ctx = r.factory.for_session(user, sid);
    match ctx.complete_session().await {
        Ok(true) => {
            // 完成钩子（模块自清理；失败仅 warn——完成语义已原子落库）。
            if let Err(e) = r.module.on_session_end(ctx.as_ref()).await {
                tracing::warn!("on_session_end 失败（完成已落库）：{e}");
            }
            Ok(Json(json!({ "ok": true })))
        }
        Ok(false) => Err(StatusCode::CONFLICT),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// 会话列表查询（folder 子树过滤 / module 命名空间过滤；page 预留，M1d 全量返回私有规模有界）。
#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SessionListQuery {
    /// 文件夹路径子树过滤（如 /日语/五十音）。
    #[serde(default)]
    pub folder: Option<String>,
    /// 模块命名空间过滤（default=无模块）。
    #[serde(default)]
    pub module: Option<String>,
}

/// 会话列表响应。
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct SessionListResp {
    pub sessions: Vec<kernel::SessionListItem>,
}

/// 列出用户会话（元数据：title/created_at/last_active/message_count/path）。
#[utoipa::path(get, path = "/api/v1/sessions", tag = "sessions", params(SessionListQuery), responses((status = 200, body = SessionListResp), (status = 401)))]
pub async fn list_sessions(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Query(q): Query<SessionListQuery>,
) -> Result<Json<SessionListResp>, StatusCode> {
    // folder 过滤服务端 NFC 归一化（与存储路径同形；非法 → 400）。
    let folder = match q.folder.as_deref() {
        Some(f) => Some(kernel::normalize_path(f).map_err(|_| StatusCode::BAD_REQUEST)?),
        None => None,
    };
    let sessions =
        kernel::list_sessions_meta(&st.store, user.0, q.module.as_deref(), folder.as_deref())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(SessionListResp { sessions }))
}

/// 会话转录响应（含虚拟路径）。
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct SessionDetailResp {
    pub id: Uuid,
    pub path: Option<String>,
    pub messages: Vec<kernel_api::ChatMessage>,
}

/// 读会话转录（含 path）。
#[utoipa::path(get, path = "/api/v1/sessions/{id}", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 200, body = SessionDetailResp), (status = 401), (status = 404)))]
pub async fn get_session(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionDetailResp>, StatusCode> {
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
    let path = kernel::get_session_path(&st.store, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(SessionDetailResp {
        id,
        path,
        messages: msgs,
    }))
}

/// 移动会话请求（path=null 回未分类）。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct MoveSessionReq {
    #[serde(default)]
    pub path: Option<String>,
}

/// 移动会话到文件夹路径（自动注册目标链；归属校验）。
#[utoipa::path(put, path = "/api/v1/sessions/{id}/path", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), request_body = MoveSessionReq, responses((status = 200, body = Value), (status = 400), (status = 401), (status = 404)))]
pub async fn move_session_path(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<MoveSessionReq>,
) -> Result<Json<Value>, StatusCode> {
    match kernel::move_session(&st.store, user.0, id, req.path.as_deref()).await {
        Ok(true) => Ok(Json(json!({ "ok": true }))),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(e) => Err(folder_err_status(e)),
    }
}

/// 音频元数据响应（回放词级高亮出口）。
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct AudioMetaResp {
    pub words: Vec<WordTimestamp>,
    pub format: AudioFormat,
    pub sample_rate: u32,
}

/// 回放音频元数据（词级时间戳 + 编码 + 采样率；归属校验，越权/缺失 404）。
#[utoipa::path(get, path = "/api/v1/audio/{id}/meta", tag = "sessions", params(("id" = Uuid, Path, description = "音频 id")), responses((status = 200, body = AudioMetaResp), (status = 401), (status = 404)))]
pub async fn audio_meta(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<AudioMetaResp>, StatusCode> {
    let cached = st
        .audio
        .get(AudioId(id), user)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(AudioMetaResp {
        words: cached.words,
        format: cached.format,
        sample_rate: cached.sample_rate,
    }))
}

/// FolderError → HTTP（Invalid/Conflict/Cycle→400，Storage→500）。
pub fn folder_err_status(e: kernel::FolderError) -> StatusCode {
    match e {
        kernel::FolderError::Invalid(_)
        | kernel::FolderError::Conflict(_)
        | kernel::FolderError::Cycle(_) => StatusCode::BAD_REQUEST,
        kernel::FolderError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// 删除会话（含转录，SessionEnded outbox 同事务原子，审计 R6）。
#[utoipa::path(delete, path = "/api/v1/sessions/{id}", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 204), (status = 401), (status = 404)))]
pub async fn delete_session(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    // SessionEnded outbox + 删除同事务原子（审计 R6）：不吞错伪造结束，越权/不存在 404。
    match kernel::end_session(&st.store, user.0, id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// 回合消息请求体。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct MessageReq {
    /// 用户消息文本。
    pub text: String,
}

/// 发消息 → 该回合 SSE 事件流（SSE 不入 OpenAPI schema）。
#[utoipa::path(post, path = "/api/v1/sessions/{id}/messages", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), request_body = MessageReq, responses((status = 200, description = "SessionEvent SSE 流", content_type = "text/event-stream"), (status = 401), (status = 404)))]
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
#[utoipa::path(post, path = "/api/v1/sessions/{id}/cancel", tag = "sessions", params(("id" = Uuid, Path, description = "会话 id")), responses((status = 200, body = Value), (status = 401), (status = 404)))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthUser;
    use crate::testutil::{state, user_with_token, TtsMode};
    use kernel_api::{AudioId, WordTimestamp};

    #[tokio::test]
    async fn audio_meta_owner_only() {
        let st = state(TtsMode::Ok).await;
        let (u1, _) = user_with_token(&st, "am1").await;
        let (u2, _) = user_with_token(&st, "am2").await;
        let aid = AudioId::new();
        st.audio
            .put(
                aid,
                kernel::CachedAudio {
                    pcm: bytes::Bytes::from(vec![0u8; 48]),
                    words: vec![WordTimestamp {
                        text: "あ".to_string(),
                        begin_index: 0,
                        end_index: 1,
                        begin_time: 0,
                        end_time: 100,
                    }],
                    format: AudioFormat::Pcm,
                    sample_rate: 24000,
                    owner: u1,
                },
            )
            .await
            .unwrap();
        let ok = audio_meta(State(st.clone()), AuthUser(u1), Path(aid.0))
            .await
            .unwrap();
        assert_eq!(ok.words.len(), 1, "owner 可读 meta");
        let cross = audio_meta(State(st.clone()), AuthUser(u2), Path(aid.0)).await;
        assert_eq!(cross.unwrap_err(), StatusCode::NOT_FOUND, "越权 meta 404");
    }

    #[tokio::test]
    async fn list_sessions_folder_nfc_and_invalid() {
        let st = state(TtsMode::Ok).await;
        let (user, _) = user_with_token(&st, "ls1").await;
        let sid = SessionId::new();
        st.sessions
            .create(user, sid, SessionMeta::default())
            .await
            .unwrap();
        kernel::move_session(&st.store, user.0, sid.0, Some("/が"))
            .await
            .unwrap();
        // NFD 查询（か+组合浊音符）服务端 NFC 归一化后命中。
        let q = SessionListQuery {
            folder: Some("/か\u{3099}".to_string()),
            module: None,
        };
        let resp = list_sessions(State(st.clone()), AuthUser(user), Query(q))
            .await
            .unwrap();
        assert_eq!(resp.sessions.len(), 1, "NFC 归一化过滤命中");
        // 非法路径 → 400。
        let bad = SessionListQuery {
            folder: Some("/a//b".to_string()),
            module: None,
        };
        let r = list_sessions(State(st.clone()), AuthUser(user), Query(bad)).await;
        assert_eq!(r.unwrap_err(), StatusCode::BAD_REQUEST, "非法 folder 400");
    }

    /// M2 P1：会话创建模块校验（未注册 400）+ 缺省 teach-free（T4 默认命名空间）。
    #[tokio::test]
    async fn create_session_module_validation_and_default() {
        let st = state(TtsMode::Ok).await;
        let (user, _) = user_with_token(&st, "cs-mod").await;
        // 未注册模块 → 400。
        let r = create_session(
            State(st.clone()),
            AuthUser(user),
            Some(Json(CreateSessionReq {
                on_disconnect: None,
                params: None,
                module_id: Some("absent-module".to_string()),
            })),
        )
        .await;
        assert_eq!(r.unwrap_err(), StatusCode::BAD_REQUEST, "未注册模块 400");
        // 缺省 → teach-free。
        let resp = create_session(State(st.clone()), AuthUser(user), None)
            .await
            .unwrap();
        let sid = SessionId(resp.0["id"].as_str().unwrap().parse().unwrap());
        let meta = st.sessions.meta(user, sid).await.unwrap().unwrap();
        assert_eq!(
            meta.module_id.as_ref().map(|m| m.0.as_str()),
            Some("teach-free"),
            "缺省会话模块 teach-free"
        );
        // 显式 teach-free 亦可。
        let resp = create_session(
            State(st.clone()),
            AuthUser(user),
            Some(Json(CreateSessionReq {
                on_disconnect: None,
                params: None,
                module_id: Some("teach-free".to_string()),
            })),
        )
        .await
        .unwrap();
        assert!(resp.0["id"].as_str().is_some());
    }

    /// M2 P1：完成语义（完成≠删除）+ 幂等 409 + 越权 404 + 转录保留 + 事件。
    #[tokio::test]
    async fn complete_session_semantics_rest() {
        let st = state(TtsMode::Ok).await;
        let (u1, _) = user_with_token(&st, "comp1").await;
        let (u2, _) = user_with_token(&st, "comp2").await;
        let resp = create_session(State(st.clone()), AuthUser(u1), None)
            .await
            .unwrap();
        let sid: Uuid = resp.0["id"].as_str().unwrap().parse().unwrap();
        // 越权 → 404。
        let r = complete_session(State(st.clone()), AuthUser(u2), Path(sid)).await;
        assert_eq!(r.unwrap_err(), StatusCode::NOT_FOUND, "越权完成 404");
        // 完成 → 200，会话仍在（完成≠删除）。
        let ok = complete_session(State(st.clone()), AuthUser(u1), Path(sid))
            .await
            .unwrap();
        assert_eq!(ok.0["ok"], serde_json::json!(true));
        assert!(
            st.sessions.exists(u1, SessionId(sid)).await.unwrap(),
            "完成不删会话"
        );
        // session-ended 事件（module 归因 teach-free）。
        let pending = kernel::pending_events(&st.store, 100).await.unwrap();
        assert!(
            pending.iter().any(|e| e.kind == "session-ended"
                && e.session_id == Some(sid)
                && e.module_id == "teach-free"),
            "完成事件 + 归因"
        );
        // 重复完成 → 409（幂等）。
        let r = complete_session(State(st.clone()), AuthUser(u1), Path(sid)).await;
        assert_eq!(r.unwrap_err(), StatusCode::CONFLICT, "重复完成 409");
        // 不存在 → 404。
        let r = complete_session(State(st.clone()), AuthUser(u1), Path(Uuid::new_v4())).await;
        assert_eq!(r.unwrap_err(), StatusCode::NOT_FOUND);
    }

    /// M2 P1：模块清单端点。
    #[tokio::test]
    async fn list_modules_contains_teach_free() {
        let st = state(TtsMode::Ok).await;
        let (user, _) = user_with_token(&st, "lm1").await;
        let resp = list_modules(State(st.clone()), AuthUser(user)).await;
        assert!(
            resp.modules.iter().any(|m| m.id == "teach-free"),
            "清单含 teach-free"
        );
    }
}
