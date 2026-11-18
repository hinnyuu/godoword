//! OpenAPI 文档聚合（utoipa）。M1c 落 REST 标注，M1d 由此生成 TS 客户端。
//! SSE 精确表述（审计 D4）：`/messages` **path 入** OpenAPI，`SessionEvent` **事件 schema 不入**（单独协议文档维护）。
//! models/voices 响应暂以通用 object 标注（ModelInfo/VoiceInfo 为 kernel-api 契约类型，精确 schema 待契约演进补）。
//! **完整性（审计 R5）**：CreateSessionReq 注册；`build_openapi` 集中注入 cookieAuth securityScheme + 全局 security
//! + 所有 /api/v1 变更操作 CSRF `X-Requested-With` header + 403；单元测试锚定所有本地 `$ref` 可解析。

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
        crate::rest::CreateSessionReq,
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

/// 构建完整 OpenAPI 文档（审计 R5）：derive 基础 + 机器可读 cookieAuth/security + 所有 /api/v1 变更操作
/// 集中补 CSRF `X-Requested-With` header 参数 + 403 响应（DRY，与 csrf_guard 中间件一致）。
pub fn build_openapi() -> Value {
    let mut doc = serde_json::to_value(ApiDoc::openapi()).unwrap_or_else(|_| json!({}));
    if !doc
        .get("components")
        .map(|c| c.is_object())
        .unwrap_or(false)
    {
        doc["components"] = json!({});
    }
    // cookie 会话认证（apiKey in cookie）+ 全局 security 要求。
    doc["components"]["securitySchemes"] = json!({
        "cookieAuth": { "type": "apiKey", "in": "cookie", "name": "godoword_session" }
    });
    doc["security"] = json!([{ "cookieAuth": [] }]);
    // CSRF：所有 /api/v1 变更操作补 header 参数 + 403（缺头或非管理员）。
    if let Some(paths) = doc.get_mut("paths").and_then(|p| p.as_object_mut()) {
        for (path, item) in paths.iter_mut() {
            if !path.starts_with("/api/v1") {
                continue;
            }
            for method in ["post", "put", "delete", "patch"] {
                let Some(op) = item.get_mut(method).and_then(|o| o.as_object_mut()) else {
                    continue;
                };
                let params = op.entry("parameters").or_insert_with(|| json!([]));
                if let Some(arr) = params.as_array_mut() {
                    let has = arr.iter().any(|p| {
                        p.get("name").and_then(|n| n.as_str()) == Some("X-Requested-With")
                    });
                    if !has {
                        arr.push(json!({
                            "name": "X-Requested-With",
                            "in": "header",
                            "required": true,
                            "description": "CSRF 自定义头（变更请求必带，缺则 403）",
                            "schema": { "type": "string" }
                        }));
                    }
                }
                let responses = op.entry("responses").or_insert_with(|| json!({}));
                if let Some(rmap) = responses.as_object_mut() {
                    rmap.entry("403")
                        .or_insert_with(|| json!({ "description": "CSRF 头缺失或非管理员" }));
                }
            }
        }
    }
    doc
}

/// 伺服 OpenAPI JSON。
pub async fn openapi_json() -> Json<Value> {
    Json(build_openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 递归收集所有 `$ref` 字符串。
    fn collect_refs(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, val) in m {
                    if k == "$ref" {
                        if let Some(s) = val.as_str() {
                            out.push(s.to_string());
                        }
                    } else {
                        collect_refs(val, out);
                    }
                }
            }
            Value::Array(a) => {
                for x in a {
                    collect_refs(x, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn openapi_refs_resolve_and_auth_complete() {
        let doc = build_openapi();
        let schemas = doc
            .get("components")
            .and_then(|c| c.get("schemas"))
            .and_then(|s| s.as_object())
            .cloned()
            .unwrap_or_default();
        let mut refs = Vec::new();
        collect_refs(&doc, &mut refs);
        assert!(!refs.is_empty(), "R5：文档应含 $ref");
        for r in &refs {
            let name = r
                .strip_prefix("#/components/schemas/")
                .unwrap_or_else(|| panic!("R5：非本地 schema $ref：{r}"));
            assert!(
                schemas.contains_key(name),
                "R5：悬挂 $ref {r}（components.schemas 无 {name}）"
            );
        }
        // CreateSessionReq 注册（原悬挂引用修复）。
        assert!(
            schemas.contains_key("CreateSessionReq"),
            "R5：CreateSessionReq 注册"
        );
        // cookieAuth securityScheme + 全局 security。
        assert_eq!(
            doc["components"]["securitySchemes"]["cookieAuth"]["type"].as_str(),
            Some("apiKey"),
            "R5：cookieAuth apiKey"
        );
        assert_eq!(
            doc["components"]["securitySchemes"]["cookieAuth"]["in"].as_str(),
            Some("cookie"),
            "R5：cookieAuth in cookie"
        );
        assert!(
            doc["security"]
                .as_array()
                .map(|a| !a.is_empty())
                .unwrap_or(false),
            "R5：全局 security 要求"
        );
        // 变更端点 CSRF header + 403（集中注入）。
        let post_sessions = &doc["paths"]["/api/v1/sessions"]["post"];
        let params = post_sessions["parameters"].as_array().expect("parameters");
        assert!(
            params
                .iter()
                .any(|p| p["name"].as_str() == Some("X-Requested-With")),
            "R5：变更端点 CSRF header"
        );
        assert!(
            post_sessions["responses"]["403"].is_object(),
            "R5：变更端点 403"
        );
        // delete 会话 404（越权/不存在）。
        assert!(
            doc["paths"]["/api/v1/sessions/{id}"]["delete"]["responses"]["404"].is_object(),
            "R5：delete 会话 404"
        );
    }
}
