//! OpenAPI 文档聚合（utoipa）。M1c 落 REST 标注，M1d 由此生成 TS 客户端。
//! SSE 精确表述（审计 D4）：`/messages` **path 入** OpenAPI，`SessionEvent` **事件 schema 不入**（单独协议文档维护）。
//! models/voices 响应暂以通用 object 标注（ModelInfo/VoiceInfo 为 kernel-api 契约类型，精确 schema 待契约演进补）。

use axum::Json;
use serde_json::{json, Value};
use utoipa::OpenApi;

/// godoword REST OpenAPI 文档。
#[derive(OpenApi)]
#[openapi(
    info(
        title = "godoword API",
        version = "0.0.0",
        description = "私有部署外语学习服务 REST API（M1c）。用户级端点经 cookie 会话认证（auth-local，未认证 401）；\
        变更请求须带 CSRF 自定义头 X-Requested-With（缺则 403）。SSE /messages path 入 OpenAPI，SessionEvent 事件 schema 单独维护。"
    ),
    paths(
        crate::rest::healthz,
        crate::rest::readyz,
        crate::rest::meta,
        crate::rest::config,
        crate::auth::register,
        crate::auth::login,
        crate::auth::logout,
        crate::auth::reset_password,
        crate::auth::me,
        crate::auth::admin_reset,
        crate::auth::admin_invite,
        crate::auth::admin_users,
        crate::auth::admin_set_status,
        crate::rest::create_session,
        crate::rest::list_sessions,
        crate::rest::get_session,
        crate::rest::delete_session,
        crate::rest::messages,
        crate::rest::cancel,
        crate::rest::get_audio,
        crate::settings::create_provider,
        crate::settings::list_providers,
        crate::settings::get_provider,
        crate::settings::update_provider,
        crate::settings::delete_provider,
        crate::settings::provider_models,
        crate::settings::provider_voices,
        crate::settings::create_binding,
        crate::settings::list_bindings,
        crate::settings::get_binding,
        crate::settings::update_binding,
        crate::settings::delete_binding,
    ),
    components(schemas(
        kernel::Provider,
        kernel::Binding,
        kernel::ConfigView,
        kernel::ProviderPreset,
        kernel::RegistrationMode,
        crate::settings::CreateProviderReq,
        crate::settings::UpdateProviderReq,
        crate::settings::CreateBindingReq,
        crate::settings::UpdateBindingReq,
        crate::auth::RegisterReq,
        crate::auth::LoginReq,
        crate::auth::ResetReq,
        crate::auth::ResetPasswordReq,
        crate::auth::InviteReq,
        crate::auth::StatusReq,
        crate::rest::MessageReq,
    )),
    tags(
        (name = "system", description = "存活/就绪/元信息/配置"),
        (name = "auth", description = "注册/登录/登出/当前用户"),
        (name = "admin", description = "管理员重置/邀请码/用户管理"),
        (name = "sessions", description = "会话与回合（SSE）"),
        (name = "providers", description = "BYOK provider 管理 + 测试连接"),
        (name = "bindings", description = "模块→provider 绑定"),
    )
)]
pub struct ApiDoc;

/// 伺服 OpenAPI JSON。
pub async fn openapi_json() -> Json<Value> {
    Json(serde_json::to_value(ApiDoc::openapi()).unwrap_or_else(|_| json!({})))
}
