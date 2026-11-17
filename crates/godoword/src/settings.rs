//! 设置 REST（M1b）：providers/bindings CRUD + models/voices 测试端点（utoipa 标注，M1d 生成 TS）。
//!
//! 用户级 REST 在 release M1b 由 dev-bypass 把守（无认证，返回 503）；M1c 真认证中间件落地后开放。
//! provider key 经 MasterKey 加密存储，响应绝不返回明文/密文 key。models/voices 测试端点解密 key
//! 构造 ProviderConn 调适配器 list_models/list_voices（设置页“测试连接”按钮）。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use kernel::{repo, Binding, Provider};
use kernel_api::{ModelInfo, ProviderError, VoiceInfo};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::rest::{user_access, AppState};

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
        (status = 503, description = "需 dev-bypass（M1c 认证后开放）")
    )
)]
pub async fn create_provider(
    State(st): State<Arc<AppState>>,
    Json(req): Json<CreateProviderReq>,
) -> Result<(StatusCode, Json<Provider>), StatusCode> {
    user_access()?;
    let p = repo::create_provider(
        &st.store,
        &st.master,
        st.user.0,
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
    responses((status = 200, description = "provider 列表", body = Vec<Provider>), (status = 503))
)]
pub async fn list_providers(
    State(st): State<Arc<AppState>>,
) -> Result<Json<Vec<Provider>>, StatusCode> {
    user_access()?;
    let v = repo::list_providers(&st.store, st.user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(v))
}

/// 取单个 provider。
#[utoipa::path(
    get, path = "/api/v1/providers/{id}", tag = "providers",
    params(("id" = Uuid, Path, description = "provider id")),
    responses((status = 200, body = Provider), (status = 404), (status = 503))
)]
pub async fn get_provider(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Provider>, StatusCode> {
    user_access()?;
    repo::get_provider(&st.store, st.user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// 更新 provider。
#[utoipa::path(
    put, path = "/api/v1/providers/{id}", request_body = UpdateProviderReq, tag = "providers",
    params(("id" = Uuid, Path, description = "provider id")),
    responses((status = 200, body = Provider), (status = 404), (status = 503))
)]
pub async fn update_provider(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProviderReq>,
) -> Result<Json<Provider>, StatusCode> {
    user_access()?;
    repo::update_provider(
        &st.store,
        &st.master,
        st.user.0,
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
    responses((status = 204, description = "已删除"), (status = 404), (status = 503))
)]
pub async fn delete_provider(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    user_access()?;
    let deleted = repo::delete_provider(&st.store, st.user.0, id)
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
    responses((status = 200, description = "模型列表", body = Value), (status = 404), (status = 502, description = "上游错误"), (status = 503))
)]
pub async fn provider_models(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ModelInfo>>, StatusCode> {
    user_access()?;
    let llm = st.llm.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let conn = repo::provider_conn(&st.store, &st.master, st.user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let models = llm.list_models(&conn).await.map_err(|e| {
        tracing::warn!("list_models 上游错误：{e}");
        upstream_status(&e)
    })?;
    Ok(Json(models))
}

/// 测试端点：拉取 provider 音色（设置页“测试连接”）。
#[utoipa::path(
    get, path = "/api/v1/providers/{id}/voices", tag = "providers",
    params(("id" = Uuid, Path, description = "provider id"), ("model" = Option<String>, Query, description = "模型 id（省略取适配器首个）")),
    responses((status = 200, description = "音色列表", body = Value), (status = 404), (status = 502), (status = 503))
)]
pub async fn provider_voices(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(q): Query<VoicesQuery>,
) -> Result<Json<Vec<VoiceInfo>>, StatusCode> {
    user_access()?;
    let tts = st.tts.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let conn = repo::provider_conn(&st.store, &st.master, st.user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let model = q
        .model
        .or_else(|| tts.models().first().map(|m| m.id.clone()))
        .ok_or(StatusCode::BAD_REQUEST)?;
    let voices = tts.list_voices(&conn, &model).await.map_err(|e| {
        tracing::warn!("list_voices 上游错误：{e}");
        upstream_status(&e)
    })?;
    Ok(Json(voices))
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
    responses((status = 201, body = Binding), (status = 400, description = "请求无效或 UNIQUE 冲突"), (status = 503))
)]
pub async fn create_binding(
    State(st): State<Arc<AppState>>,
    Json(req): Json<CreateBindingReq>,
) -> Result<(StatusCode, Json<Binding>), StatusCode> {
    user_access()?;
    let b = repo::create_binding(
        &st.store,
        st.user.0,
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
    responses((status = 200, body = Vec<Binding>), (status = 503))
)]
pub async fn list_bindings(
    State(st): State<Arc<AppState>>,
) -> Result<Json<Vec<Binding>>, StatusCode> {
    user_access()?;
    let v = repo::list_bindings(&st.store, st.user.0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(v))
}

/// 取单个 binding。
#[utoipa::path(
    get, path = "/api/v1/bindings/{id}", tag = "bindings",
    params(("id" = Uuid, Path, description = "binding id")),
    responses((status = 200, body = Binding), (status = 404), (status = 503))
)]
pub async fn get_binding(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Binding>, StatusCode> {
    user_access()?;
    repo::get_binding(&st.store, st.user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// 更新 binding。
#[utoipa::path(
    put, path = "/api/v1/bindings/{id}", request_body = UpdateBindingReq, tag = "bindings",
    params(("id" = Uuid, Path, description = "binding id")),
    responses((status = 200, body = Binding), (status = 404), (status = 503))
)]
pub async fn update_binding(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateBindingReq>,
) -> Result<Json<Binding>, StatusCode> {
    user_access()?;
    repo::update_binding(
        &st.store,
        st.user.0,
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
    responses((status = 204), (status = 404), (status = 503))
)]
pub async fn delete_binding(
    State(st): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    user_access()?;
    let deleted = repo::delete_binding(&st.store, st.user.0, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
