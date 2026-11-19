//! 设置 REST（M1c）：providers/bindings CRUD + models/voices 测试端点（utoipa 标注，M1d 生成 TS）。
//!
//! 用户级 REST 经 AuthUser 认证（未认证 401）；变更请求须带 CSRF 自定义头 X-Requested-With（缺则 403）。
//! binding 强制 provider 同主（审计 C2）。provider key 经 MasterKey 加密存储，响应绝不返回明文/密文 key。
//! models/voices 测试端点解密 key 调适配器 list_models/list_voices（设置页“测试连接”），并经 call_log 计量（审计 C9）。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use bytes::Bytes;
use futures_util::StreamExt;
use kernel::{repo, Binding, CachedAudio, Provider};
use kernel_api::{
    AudioEvent, AudioFormat, AudioId, ModelInfo, ProviderError, TtsRequest, VoiceInfo,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::rest::AppState;

/// 新建 provider 请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateProviderReq {
    pub adapter_id: String,
    pub label: String,
    pub base_url: String,
    /// 用户 API key（加密存储，不回显）。
    pub key: String,
    #[serde(default)]
    pub extra: Option<Value>,
}

/// 更新 provider 请求（key 为 None 时保留原 key）。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateProviderReq {
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub extra: Option<Value>,
}

/// 新建 binding 请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateBindingReq {
    /// 模块 id；省略或 `*` 表全模块默认。
    #[serde(default = "default_module")]
    pub module_id: String,
    pub capability: String,
    pub role: String,
    pub provider_id: Uuid,
    pub model: String,
    #[serde(default)]
    pub params: Option<Value>,
}

fn default_module() -> String {
    "*".to_string()
}

/// 更新 binding 请求（provider/model/params）。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateBindingReq {
    pub provider_id: Uuid,
    pub model: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// voices 查询参数。
#[derive(Deserialize)]
pub struct VoicesQuery {
    model: Option<String>,
}

fn extra_val(o: Option<Value>) -> Value {
    o.unwrap_or_else(|| serde_json::json!({}))
}

fn map_storage_err(e: kernel_api::KernelError) -> StatusCode {
    match e {
        kernel_api::KernelError::Storage(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// 新建 provider。
#[utoipa::path(
    post, path = "/api/v1/providers", request_body = CreateProviderReq, tag = "providers",
    responses(
        (status = 201, description = "已创建", body = Provider),
        (status = 400, description = "请求无效"),
        (status = 401, description = "未认证")
    )
)]
pub async fn create_provider(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Json(req): Json<CreateProviderReq>,
) -> Result<(StatusCode, Json<Provider>), StatusCode> {
    let p = repo::create_provider(
        &st.store,
        &st.master,
        user.0,
        &req.adapter_id,
        &req.label,
        &req.base_url,
        &req.key,
        extra_val(req.extra),
    )
    .await
    .map_err(map_storage_err)?;
    Ok((StatusCode::CREATED, Json(p)))
}

/// 列出用户 providers。
#[utoipa::path(
    get, path = "/api/v1/providers", tag = "providers",
    responses((status = 200, description = "provider 列表", body = Vec<Provider>), (status = 401))
)]
pub async fn list_providers(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<Provider>>, StatusCode> {
    let v = repo::list_providers(&st.store, user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(v))
}

/// 取单个 provider。
#[utoipa::path(
    get, path = "/api/v1/providers/{id}", tag = "providers",
    params(("id" = Uuid, Path, description = "provider id")),
    responses((status = 200, body = Provider), (status = 404), (status = 401))
)]
pub async fn get_provider(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Provider>, StatusCode> {
    repo::get_provider(&st.store, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// 更新 provider。
#[utoipa::path(
    put, path = "/api/v1/providers/{id}", request_body = UpdateProviderReq, tag = "providers",
    params(("id" = Uuid, Path, description = "provider id")),
    responses((status = 200, body = Provider), (status = 404), (status = 401))
)]
pub async fn update_provider(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProviderReq>,
) -> Result<Json<Provider>, StatusCode> {
    repo::update_provider(
        &st.store,
        &st.master,
        user.0,
        id,
        &req.label,
        &req.base_url,
        req.key.as_deref(),
        extra_val(req.extra),
    )
    .await
    .map_err(map_storage_err)?
    .map(Json)
    .ok_or(StatusCode::NOT_FOUND)
}

/// 删除 provider（级联删其 bindings）。
#[utoipa::path(
    delete, path = "/api/v1/providers/{id}", tag = "providers",
    params(("id" = Uuid, Path, description = "provider id")),
    responses((status = 204, description = "已删除"), (status = 404), (status = 401))
)]
pub async fn delete_provider(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let deleted = repo::delete_provider(&st.store, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// 测试端点：拉取 provider 可用模型（设置页“测试连接”）。
#[utoipa::path(
    get, path = "/api/v1/providers/{id}/models", tag = "providers",
    params(("id" = Uuid, Path, description = "provider id")),
    responses((status = 200, description = "模型列表", body = Vec<ModelInfo>), (status = 404), (status = 502, description = "上游错误"), (status = 401))
)]
pub async fn provider_models(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ModelInfo>>, StatusCode> {
    let llm = &st.llm;
    let conn = repo::provider_conn(&st.store, &st.master, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let start = std::time::Instant::now();
    let result = llm.list_models(&conn).await;
    meter_test(&st.store, user, "llm", llm.id(), "", start, &result).await;
    let models = result.map_err(|e| {
        tracing::warn!("list_models 上游错误：{e}");
        upstream_status(&e)
    })?;
    Ok(Json(models))
}

/// 测试端点：拉取 provider 音色（设置页“测试连接”）。
#[utoipa::path(
    get, path = "/api/v1/providers/{id}/voices", tag = "providers",
    params(("id" = Uuid, Path, description = "provider id"), ("model" = Option<String>, Query, description = "模型 id（省略取适配器首个）")),
    responses((status = 200, description = "音色列表", body = Vec<VoiceInfo>), (status = 404), (status = 502), (status = 401))
)]
pub async fn provider_voices(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<VoicesQuery>,
) -> Result<Json<Vec<VoiceInfo>>, StatusCode> {
    let tts = &st.tts;
    let conn = repo::provider_conn(&st.store, &st.master, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let model = q
        .model
        .or_else(|| tts.models().first().map(|m| m.id.clone()))
        .ok_or(StatusCode::BAD_REQUEST)?;
    let start = std::time::Instant::now();
    let result = tts.list_voices(&conn, &model).await;
    meter_test(&st.store, user, "tts", tts.id(), &model, start, &result).await;
    let voices = result.map_err(|e| {
        tracing::warn!("list_voices 上游错误：{e}");
        upstream_status(&e)
    })?;
    Ok(Json(voices))
}

/// 测试端点上游调用计量（审计 C9：所有上游调用经 call_log）。
async fn meter_test<T>(
    store: &kernel_api::Store,
    user: kernel_api::UserId,
    capability: &str,
    adapter_id: &str,
    model: &str,
    start: std::time::Instant,
    result: &Result<T, ProviderError>,
) {
    let err = result
        .as_ref()
        .err()
        .map(|e| upstream_status(e).to_string());
    let _ = kernel::record_call(
        store,
        kernel::CallRecord {
            user_id: user.0,
            session_id: None,
            run_id: None,
            capability: capability.to_string(),
            adapter_id: adapter_id.to_string(),
            model: model.to_string(),
            prompt_tokens: None,
            completion_tokens: None,
            reasoning_tokens: None,
            total_tokens: None,
            characters: None,
            latency_ms: start.elapsed().as_millis() as i32,
            status: if result.is_ok() { "ok" } else { "error" }.to_string(),
            error_code: err,
        },
    )
    .await;
}

fn upstream_status(e: &ProviderError) -> StatusCode {
    match e {
        ProviderError::Auth { .. } => StatusCode::UNAUTHORIZED,
        ProviderError::Quota { .. } => StatusCode::PAYMENT_REQUIRED,
        ProviderError::RateLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::BAD_GATEWAY,
    }
}

/// 新建 binding。
#[utoipa::path(
    post, path = "/api/v1/bindings", request_body = CreateBindingReq, tag = "bindings",
    responses((status = 201, body = Binding), (status = 400, description = "请求无效或 UNIQUE 冲突"), (status = 401))
)]
pub async fn create_binding(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Json(req): Json<CreateBindingReq>,
) -> Result<(StatusCode, Json<Binding>), StatusCode> {
    let b = repo::create_binding(
        &st.store,
        user.0,
        &req.module_id,
        &req.capability,
        &req.role,
        req.provider_id,
        &req.model,
        extra_val(req.params),
    )
    .await
    .map_err(map_storage_err)?;
    Ok((StatusCode::CREATED, Json(b)))
}

/// 列出用户 bindings。
#[utoipa::path(
    get, path = "/api/v1/bindings", tag = "bindings",
    responses((status = 200, body = Vec<Binding>), (status = 401))
)]
pub async fn list_bindings(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<Binding>>, StatusCode> {
    let v = repo::list_bindings(&st.store, user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(v))
}

/// 取单个 binding。
#[utoipa::path(
    get, path = "/api/v1/bindings/{id}", tag = "bindings",
    params(("id" = Uuid, Path, description = "binding id")),
    responses((status = 200, body = Binding), (status = 404), (status = 401))
)]
pub async fn get_binding(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Binding>, StatusCode> {
    repo::get_binding(&st.store, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// 更新 binding。
#[utoipa::path(
    put, path = "/api/v1/bindings/{id}", request_body = UpdateBindingReq, tag = "bindings",
    params(("id" = Uuid, Path, description = "binding id")),
    responses((status = 200, body = Binding), (status = 404), (status = 401))
)]
pub async fn update_binding(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateBindingReq>,
) -> Result<Json<Binding>, StatusCode> {
    repo::update_binding(
        &st.store,
        user.0,
        id,
        req.provider_id,
        &req.model,
        extra_val(req.params),
    )
    .await
    .map_err(map_storage_err)?
    .map(Json)
    .ok_or(StatusCode::NOT_FOUND)
}

/// 删除 binding。
#[utoipa::path(
    delete, path = "/api/v1/bindings/{id}", tag = "bindings",
    params(("id" = Uuid, Path, description = "binding id")),
    responses((status = 204), (status = 404), (status = 401))
)]
pub async fn delete_binding(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let deleted = repo::delete_binding(&st.store, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// 音色试听默认短文本。
const DEFAULT_PREVIEW_TEXT: &str = "こんにちは、始めましょう。";

/// 音色试听请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct PreviewReq {
    pub model: String,
    pub voice: String,
    /// 可选自定义文本（缺省固定短文本）。
    #[serde(default)]
    pub text: Option<String>,
}

/// 音色试听响应（合成音频 id，经 GET /audio/{id} + /meta 播放/高亮）。
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct PreviewResp {
    pub audio_id: Uuid,
}

/// 音色试听（设置页选音色听一句）：走用户 key 合成 + 缓存 + call_log 计量（审计 C9）。
#[utoipa::path(
    post, path = "/api/v1/providers/{id}/preview", tag = "providers",
    params(("id" = Uuid, Path, description = "provider id")), request_body = PreviewReq,
    responses((status = 200, body = PreviewResp), (status = 400), (status = 401), (status = 404), (status = 502))
)]
pub async fn provider_preview(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<PreviewReq>,
) -> Result<Json<PreviewResp>, StatusCode> {
    let conn = repo::provider_conn(&st.store, &st.master, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let model = req.model.clone();
    let tts_req = TtsRequest {
        model: req.model,
        text: req.text.unwrap_or_else(|| DEFAULT_PREVIEW_TEXT.to_string()),
        voice: req.voice,
        format: AudioFormat::Pcm,
        sample_rate: 24000,
        rate: None,
        pitch: None,
        instruction: None,
        language_hints: vec!["ja".to_string()],
        word_timestamps: true,
        extra: serde_json::json!({}),
    };
    let start = std::time::Instant::now();
    let init = st.tts.synthesize_stream(&conn, tts_req).await;
    let mut stream = match init {
        Ok(s) => s,
        Err(e) => {
            let status = upstream_status(&e);
            let r: Result<(), ProviderError> = Err(e);
            meter_test(&st.store, user, "tts", st.tts.id(), &model, start, &r).await;
            return Err(status);
        }
    };
    let audio_id = AudioId::new();
    let mut complete: Vec<u8> = Vec::new();
    let mut words = Vec::new();
    let mut stream_err: Option<ProviderError> = None;
    while let Some(item) = stream.next().await {
        match item {
            Ok(AudioEvent::Chunk { pcm, .. }) => complete.extend_from_slice(&pcm),
            Ok(AudioEvent::Words(w)) => words.extend(w),
            Ok(AudioEvent::End { complete: end }) => {
                if let Some(b) = end {
                    complete = b.to_vec();
                }
            }
            Ok(_) => {}
            Err(e) => {
                stream_err = Some(e);
                break;
            }
        }
    }
    let final_result: Result<(), ProviderError> = match stream_err {
        Some(e) => Err(e),
        None => Ok(()),
    };
    meter_test(
        &st.store,
        user,
        "tts",
        st.tts.id(),
        &model,
        start,
        &final_result,
    )
    .await;
    if let Err(e) = final_result {
        return Err(upstream_status(&e));
    }
    // 缓存写失败关闭（审计 R2）：put 失败不假成功。
    st.audio
        .put(
            audio_id,
            CachedAudio {
                pcm: Bytes::from(complete),
                words,
                format: AudioFormat::Pcm,
                sample_rate: 24000,
                owner: user,
            },
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(PreviewResp {
        audio_id: audio_id.0,
    }))
}
