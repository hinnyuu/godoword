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

fn map_storage_err(_e: kernel_api::KernelError) -> StatusCode {
    // 审计 F6：真实存储故障 → 500（用户输入冲突/归属由 handler 预检 → 400，不混淆）。
    StatusCode::INTERNAL_SERVER_ERROR
}

/// binding 身份键冲突预检（UNIQUE(user,module,cap,role)）：已存在 → 400（区别存储故障 500）。
async fn binding_conflict(
    st: &AppState,
    user: kernel_api::UserId,
    module: &str,
    cap: &str,
    role: &str,
) -> bool {
    match repo::list_bindings(&st.store, user.0).await {
        Ok(bs) => bs
            .iter()
            .any(|b| b.module_id == module && b.capability == cap && b.role == role),
        Err(_) => false,
    }
}

/// provider 归属预检（审计 C2 的 REST 语义）：不存在/非本用户 → 400。
async fn provider_owned(st: &AppState, user: kernel_api::UserId, id: Uuid) -> bool {
    matches!(repo::get_provider(&st.store, user.0, id).await, Ok(Some(_)))
}

/// provider 输入边界校验（二审 R2-4）：adapter 须已注册、label/base_url 非空、base_url 须 http(s)、key 非空。
fn provider_input_valid(
    st: &AppState,
    adapter_id: &str,
    label: &str,
    base_url: &str,
    key: Option<&str>,
) -> bool {
    let adapter_known = adapter_id == st.llm.id() || adapter_id == st.tts.id();
    let url_ok = base_url.starts_with("http://") || base_url.starts_with("https://");
    let key_ok = key.map(|k| !k.is_empty()).unwrap_or(true);
    adapter_known && !label.trim().is_empty() && !base_url.trim().is_empty() && url_ok && key_ok
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
    if !provider_input_valid(
        &st,
        &req.adapter_id,
        &req.label,
        &req.base_url,
        Some(&req.key),
    ) {
        return Err(StatusCode::BAD_REQUEST);
    }
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
    responses((status = 200, body = Provider), (status = 400), (status = 404), (status = 401))
)]
pub async fn update_provider(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProviderReq>,
) -> Result<Json<Provider>, StatusCode> {
    // adapter_id 不可改（身份键）；沿用既有行 adapter 校验其余字段。
    let Some(existing) = repo::get_provider(&st.store, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        return Err(StatusCode::NOT_FOUND);
    };
    if !provider_input_valid(
        &st,
        &existing.adapter_id,
        &req.label,
        &req.base_url,
        req.key.as_deref(),
    ) {
        return Err(StatusCode::BAD_REQUEST);
    }
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
    // 审计 O4：adapter 与端点能力匹配校验（TTS provider 调 /models 会拿用户 key 打错误上游路径）。
    let p = repo::get_provider(&st.store, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if p.adapter_id != llm.id() {
        return Err(StatusCode::BAD_REQUEST);
    }
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
    // 审计 O4：adapter 与端点能力匹配校验。
    let p = repo::get_provider(&st.store, user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if p.adapter_id != tts.id() {
        return Err(StatusCode::BAD_REQUEST);
    }
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
        // 上游 key 无效 ≠ 用户未认证：401 专属用户会话（AuthUser），上游错误用 502（防前端 401 钩子误登出）。
        ProviderError::Auth { .. } => StatusCode::BAD_GATEWAY,
        ProviderError::Quota { .. } => StatusCode::PAYMENT_REQUIRED,
        ProviderError::RateLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::BAD_GATEWAY,
    }
}

/// binding 输入边界校验（审计 O5）：capability/role 须属契约枚举，model 非空。
/// module_id 注册表校验留 M2（注册表落地后）。
fn binding_input_valid(capability: &str, role: &str, model: &str) -> bool {
    let cap_ok = matches!(capability, "llm" | "tts" | "stt");
    let role_ok = matches!(role, "default" | "summarizer" | "extractor" | "assessor");
    cap_ok && role_ok && !model.trim().is_empty()
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
    if !binding_input_valid(&req.capability, &req.role, &req.model) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !provider_owned(&st, user, req.provider_id).await {
        return Err(StatusCode::BAD_REQUEST);
    }
    if binding_conflict(&st, user, &req.module_id, &req.capability, &req.role).await {
        return Err(StatusCode::BAD_REQUEST);
    }
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
    if req.model.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !provider_owned(&st, user, req.provider_id).await {
        return Err(StatusCode::BAD_REQUEST);
    }
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
/// 试听自定义文本上限（审计 F2：防巨量文本→内存 PCM 缓冲放大；500 字符远超试听需求）。
const PREVIEW_TEXT_CAP: usize = 500;
/// 试听合成总超时（审计 F2：独立于回合硬超时，测试端点不得无限挂起）。
const PREVIEW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// 音色试听请求。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct PreviewReq {
    pub model: String,
    pub voice: String,
    /// 可选自定义文本（缺省固定短文本，≤500 字符）。
    #[serde(default)]
    pub text: Option<String>,
}

/// 音色试听响应（合成音频 id，经 GET /audio/{id} + /meta 播放/高亮）。
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PreviewResp {
    pub audio_id: Uuid,
}

/// 试听合成排空（可注入超时供测试）：收集完整 PCM + 词时间戳 + Usage（计量完整，审计 F2）。
async fn drain_preview(
    tts: &Arc<dyn kernel_api::TtsProvider>,
    conn: &kernel_api::ProviderConn,
    req: TtsRequest,
) -> Result<
    (
        Vec<u8>,
        Vec<kernel_api::WordTimestamp>,
        Option<kernel_api::TtsUsage>,
    ),
    ProviderError,
> {
    let mut stream = tts.synthesize_stream(conn, req).await?;
    let mut complete: Vec<u8> = Vec::new();
    let mut words = Vec::new();
    let mut usage = None;
    while let Some(item) = stream.next().await {
        match item? {
            AudioEvent::Chunk { pcm, .. } => complete.extend_from_slice(&pcm),
            AudioEvent::Words(w) => words.extend(w),
            AudioEvent::Usage(u) => usage = Some(u),
            AudioEvent::End { complete: end } => {
                if let Some(b) = end {
                    complete = b.to_vec();
                }
            }
            AudioEvent::Start { .. } => {}
        }
    }
    Ok((complete, words, usage))
}

/// 试听计量（call_log 含 characters/tokens，与 speak 计量对称；审计 C9/F2）。
#[allow(clippy::too_many_arguments)]
async fn meter_preview(
    store: &kernel_api::Store,
    user: kernel_api::UserId,
    adapter_id: &str,
    model: &str,
    start: std::time::Instant,
    usage: Option<kernel_api::TtsUsage>,
    status: &str,
    error_code: Option<String>,
) {
    let _ = kernel::record_call(
        store,
        kernel::CallRecord {
            user_id: user.0,
            session_id: None,
            run_id: None,
            capability: "tts".to_string(),
            adapter_id: adapter_id.to_string(),
            model: model.to_string(),
            prompt_tokens: usage
                .as_ref()
                .and_then(|u| u.input_tokens)
                .map(|t| t as i32),
            completion_tokens: usage
                .as_ref()
                .and_then(|u| u.output_tokens)
                .map(|t| t as i32),
            reasoning_tokens: None,
            total_tokens: usage
                .as_ref()
                .and_then(|u| u.input_tokens.zip(u.output_tokens))
                .map(|(i, o)| (i + o) as i32),
            characters: usage.as_ref().and_then(|u| u.characters).map(|t| t as i32),
            latency_ms: start.elapsed().as_millis() as i32,
            status: status.to_string(),
            error_code,
        },
    )
    .await;
}

/// 音色试听（设置页选音色听一句）：走用户 key 合成 + 缓存 + call_log 计量（审计 C9）。
/// 文本上限 400 / 总超时 504 / 缓存写失败关闭 R2（审计 F2）。
#[utoipa::path(
    post, path = "/api/v1/providers/{id}/preview", tag = "providers",
    params(("id" = Uuid, Path, description = "provider id")), request_body = PreviewReq,
    responses((status = 200, body = PreviewResp), (status = 400), (status = 401), (status = 404), (status = 502), (status = 504))
)]
pub async fn provider_preview(
    State(st): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<PreviewReq>,
) -> Result<Json<PreviewResp>, StatusCode> {
    if let Some(t) = &req.text {
        if t.chars().count() > PREVIEW_TEXT_CAP {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
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
    let drained =
        tokio::time::timeout(PREVIEW_TIMEOUT, drain_preview(&st.tts, &conn, tts_req)).await;
    let (pcm, words, usage) = match drained {
        Err(_) => {
            meter_preview(
                &st.store,
                user,
                st.tts.id(),
                &model,
                start,
                None,
                "error",
                Some("timeout".to_string()),
            )
            .await;
            return Err(StatusCode::GATEWAY_TIMEOUT);
        }
        Ok(Err(e)) => {
            let status = upstream_status(&e);
            meter_preview(
                &st.store,
                user,
                st.tts.id(),
                &model,
                start,
                None,
                "error",
                Some(status.to_string()),
            )
            .await;
            return Err(status);
        }
        Ok(Ok(v)) => v,
    };
    meter_preview(
        &st.store,
        user,
        st.tts.id(),
        &model,
        start,
        usage,
        "ok",
        None,
    )
    .await;
    // 缓存写失败关闭（审计 R2）：put 失败不假成功。
    let audio_id = AudioId::new();
    st.audio
        .put(
            audio_id,
            CachedAudio {
                pcm: Bytes::from(pcm),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthUser;
    use crate::testutil::{state, tts_provider, user_with_token, TtsMode};

    fn preview_req(text: Option<String>) -> PreviewReq {
        PreviewReq {
            model: "mock-tts".to_string(),
            voice: "longanlingxin".to_string(),
            text,
        }
    }

    #[tokio::test]
    async fn preview_ok_meters_usage_and_caches() {
        let st = state(TtsMode::Ok).await;
        let (user, _) = user_with_token(&st, "pv1").await;
        let pid = tts_provider(&st, user).await;
        let resp = provider_preview(
            State(st.clone()),
            AuthUser(user),
            Path(pid),
            Json(preview_req(None)),
        )
        .await
        .expect("试听成功");
        // 缓存归属 + 回放元数据出口。
        let cached = st
            .audio
            .get(kernel_api::AudioId(resp.audio_id), user)
            .await
            .unwrap();
        assert!(cached.is_some(), "试听音频入缓存（owner）");
        assert_eq!(cached.unwrap().words.len(), 1, "词时间戳入 sidecar");
        // 计量含 characters/tokens（F2：与 speak 对称）。
        let rows = kernel::call_log_rows(&st.store, user.0).await.unwrap();
        assert_eq!(rows.len(), 1, "试听计量一条");
        assert_eq!(rows[0].0, "ok");
        assert_eq!(rows[0].1, Some(5), "characters 计量");
        assert_eq!(rows[0].2, Some(13), "total_tokens=2+11");
    }

    #[tokio::test]
    async fn preview_text_cap_400() {
        let st = state(TtsMode::Ok).await;
        let (user, _) = user_with_token(&st, "pv2").await;
        let pid = tts_provider(&st, user).await;
        let long = "あ".repeat(PREVIEW_TEXT_CAP + 1);
        let r = provider_preview(
            State(st.clone()),
            AuthUser(user),
            Path(pid),
            Json(preview_req(Some(long))),
        )
        .await;
        assert_eq!(r.unwrap_err(), StatusCode::BAD_REQUEST, "F2 文本上限");
        let n = kernel::call_log_count(&st.store, user.0).await.unwrap();
        assert_eq!(n, 0, "上限拒绝不调上游不计量");
    }

    #[tokio::test]
    async fn preview_cross_user_404() {
        let st = state(TtsMode::Ok).await;
        let (u1, _) = user_with_token(&st, "pv3a").await;
        let (u2, _) = user_with_token(&st, "pv3b").await;
        let pid = tts_provider(&st, u1).await;
        let r = provider_preview(
            State(st.clone()),
            AuthUser(u2),
            Path(pid),
            Json(preview_req(None)),
        )
        .await;
        assert_eq!(r.unwrap_err(), StatusCode::NOT_FOUND, "越权试听 404");
    }

    #[tokio::test]
    async fn preview_upstream_auth_502_not_401() {
        let st = state(TtsMode::AuthErr).await;
        let (user, _) = user_with_token(&st, "pv4").await;
        let pid = tts_provider(&st, user).await;
        let r = provider_preview(
            State(st.clone()),
            AuthUser(user),
            Path(pid),
            Json(preview_req(None)),
        )
        .await;
        assert_eq!(
            r.unwrap_err(),
            StatusCode::BAD_GATEWAY,
            "上游 key 无效 → 502（401 专属用户会话）"
        );
    }

    #[tokio::test]
    async fn preview_drain_timeout() {
        let st = state(TtsMode::Hang).await;
        let (user, _) = user_with_token(&st, "pv5").await;
        let pid = tts_provider(&st, user).await;
        let conn = repo::provider_conn(&st.store, &st.master, user.0, pid)
            .await
            .unwrap()
            .unwrap();
        let req = TtsRequest {
            model: "mock-tts".to_string(),
            text: "hi".to_string(),
            voice: "v".to_string(),
            format: AudioFormat::Pcm,
            sample_rate: 24000,
            rate: None,
            pitch: None,
            instruction: None,
            language_hints: vec![],
            word_timestamps: true,
            extra: serde_json::json!({}),
        };
        // F2：挂起上游在总超时内被切断（handler 用 60s 常量，此处注入短超时验证机制）。
        let r = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            drain_preview(&st.tts, &conn, req),
        )
        .await;
        assert!(r.is_err(), "挂起流被超时切断");
    }

    #[tokio::test]
    async fn provider_input_validation_400() {
        let st = state(TtsMode::Ok).await;
        let (user, _) = user_with_token(&st, "pvv").await;
        let mk = |adapter: &str, key: &str, url: &str| CreateProviderReq {
            adapter_id: adapter.to_string(),
            label: "l".to_string(),
            base_url: url.to_string(),
            key: key.to_string(),
            extra: None,
        };
        // 二审 R2-4：未知 adapter / 空 key / 非 http(s) / 空端点 → 400。
        for req in [
            mk("bogus-adapter", "k", "https://x.example"),
            mk("tts-qwen-audio", "", "https://x.example"),
            mk("tts-qwen-audio", "k", "ftp://x.example"),
            mk("tts-qwen-audio", "k", ""),
        ] {
            let r = create_provider(State(st.clone()), AuthUser(user), Json(req)).await;
            assert_eq!(r.unwrap_err(), StatusCode::BAD_REQUEST, "R2-4 输入边界");
        }
        // 合法创建 201；更新留空 key 保留原 key（仍可解密）。
        let ok = create_provider(
            State(st.clone()),
            AuthUser(user),
            Json(mk("tts-qwen-audio", "k", "https://x.example")),
        )
        .await
        .unwrap();
        let pid = ok.1.id;
        let upd = update_provider(
            State(st.clone()),
            AuthUser(user),
            Path(pid),
            Json(UpdateProviderReq {
                label: "l2".to_string(),
                base_url: "https://y.example".to_string(),
                key: None,
                extra: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(upd.label, "l2", "更新生效");
        assert!(
            repo::provider_conn(&st.store, &st.master, user.0, pid)
                .await
                .unwrap()
                .is_some(),
            "留空 key 保留原 key 可解密"
        );
    }

    #[tokio::test]
    async fn binding_conflict_and_ownership_400() {
        let st = state(TtsMode::Ok).await;
        let (u1, _) = user_with_token(&st, "bd1").await;
        let (u2, _) = user_with_token(&st, "bd2").await;
        let p1 = tts_provider(&st, u1).await;
        let mk = |pid: Uuid| CreateBindingReq {
            module_id: "*".to_string(),
            capability: "tts".to_string(),
            role: "default".to_string(),
            provider_id: pid,
            model: "m".to_string(),
            params: None,
        };
        let _ = create_binding(State(st.clone()), AuthUser(u1), Json(mk(p1)))
            .await
            .unwrap();
        // UNIQUE 冲突预检 → 400（非 500）。
        let dup = create_binding(State(st.clone()), AuthUser(u1), Json(mk(p1))).await;
        assert_eq!(dup.unwrap_err(), StatusCode::BAD_REQUEST, "F6 冲突 400");
        // 跨用户 provider → 400（C2 REST 语义）。
        let cross = create_binding(State(st.clone()), AuthUser(u2), Json(mk(p1))).await;
        assert_eq!(
            cross.unwrap_err(),
            StatusCode::BAD_REQUEST,
            "跨用户绑定 400"
        );
    }

    /// 审计 O4：测试端点 adapter 匹配校验（TTS provider 调 /models、LLM provider 调 /voices → 400）。
    #[tokio::test]
    async fn test_endpoints_adapter_mismatch_400() {
        let st = state(TtsMode::Ok).await;
        let (user, _) = user_with_token(&st, "am-match").await;
        let tts_pid = tts_provider(&st, user).await;
        let llm_pid = kernel::repo::create_provider(
            &st.store,
            &st.master,
            user.0,
            "llm-openai",
            "mock-llm",
            "https://mock.example",
            "mock-key",
            serde_json::json!({}),
        )
        .await
        .unwrap()
        .id;
        let r = provider_models(State(st.clone()), AuthUser(user), Path(tts_pid)).await;
        assert_eq!(
            r.unwrap_err(),
            StatusCode::BAD_REQUEST,
            "TTS provider 调 /models 400"
        );
        let r = provider_voices(
            State(st.clone()),
            AuthUser(user),
            Path(llm_pid),
            Query(VoicesQuery { model: None }),
        )
        .await;
        assert_eq!(
            r.unwrap_err(),
            StatusCode::BAD_REQUEST,
            "LLM provider 调 /voices 400"
        );
        // 匹配路径正常（mock 适配器，不触网）。
        assert!(
            provider_models(State(st.clone()), AuthUser(user), Path(llm_pid))
                .await
                .is_ok()
        );
        assert!(provider_voices(
            State(st.clone()),
            AuthUser(user),
            Path(tts_pid),
            Query(VoicesQuery { model: None })
        )
        .await
        .is_ok());
        // 越权仍 404（归属先于匹配）。
        let (other, _) = user_with_token(&st, "am-match2").await;
        let r = provider_models(State(st.clone()), AuthUser(other), Path(tts_pid)).await;
        assert_eq!(r.unwrap_err(), StatusCode::NOT_FOUND, "越权 404");
    }

    /// 审计 O5：binding capability/role 枚举 + model 非空校验。
    #[tokio::test]
    async fn binding_input_validation_400() {
        let st = state(TtsMode::Ok).await;
        let (user, _) = user_with_token(&st, "bv1").await;
        let pid = tts_provider(&st, user).await;
        let mk = |cap: &str, role: &str, model: &str| CreateBindingReq {
            module_id: "*".to_string(),
            capability: cap.to_string(),
            role: role.to_string(),
            provider_id: pid,
            model: model.to_string(),
            params: None,
        };
        for req in [
            mk("bogus", "default", "m"),
            mk("tts", "bogus", "m"),
            mk("tts", "default", "  "),
        ] {
            let r = create_binding(State(st.clone()), AuthUser(user), Json(req)).await;
            assert_eq!(r.unwrap_err(), StatusCode::BAD_REQUEST, "O5 枚举/非空校验");
        }
        // 合法创建后 update 空 model → 400。
        let b = create_binding(
            State(st.clone()),
            AuthUser(user),
            Json(mk("tts", "default", "m")),
        )
        .await
        .unwrap()
        .1;
        let r = update_binding(
            State(st.clone()),
            AuthUser(user),
            Path(b.id),
            Json(UpdateBindingReq {
                provider_id: pid,
                model: "".to_string(),
                params: None,
            }),
        )
        .await;
        assert_eq!(
            r.unwrap_err(),
            StatusCode::BAD_REQUEST,
            "O5 update 空 model 400"
        );
    }
}
