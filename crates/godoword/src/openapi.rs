//! OpenAPI 文档聚合（utoipa）。M1c 落 REST 标注，M1d 由此生成 TS 客户端。
//! SSE 精确表述（审计 D4）：`/messages` **path 入** OpenAPI，`SessionEvent` **事件 schema 不入**（单独协议文档维护）。
//! models/voices 响应暂以通用 object 标注（ModelInfo/VoiceInfo 为 kernel-api 契约类型，精确 schema 待契约演进补）。
//! **完整性（审计 R5/N1）**：CreateSessionReq 注册；`build_openapi` 注入 cookieAuth securityScheme + **操作级 security 矩阵**
//! （公开 operation security:[]、受保护 cookieAuth，不用根级全局以免误伤公开端点）+ 所有 /api/v1 变更操作 CSRF
//! `X-Requested-With` header + 403；单元测试锚定所有本地 `$ref` 可解析 + 独立公开操作矩阵。

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

/// 公开 operation（无需 cookie 认证，审计 N1）：健康/元信息/配置/注册/登录/重置/登出/OpenAPI 自身。
/// 运行时按 extractor 选择认证（非全局 cookie），文档须与之一致——公开操作 security:[]，受保护 cookieAuth。
fn is_public(path: &str, method: &str) -> bool {
    matches!(
        (path, method),
        ("/healthz", "get")
            | ("/readyz", "get")
            | ("/api/v1/meta", "get")
            | ("/api/v1/config", "get")
            | ("/api/v1/openapi.json", "get")
            | ("/api/v1/auth/register", "post")
            | ("/api/v1/auth/login", "post")
            | ("/api/v1/auth/reset-password", "post")
            | ("/api/v1/auth/logout", "post")
    )
}

/// 构建完整 OpenAPI 文档（审计 R5/N1）：derive 基础 + cookieAuth securityScheme + **操作级 security 矩阵**
/// （公开 security:[] / 受保护 cookieAuth，不用根级全局以免误伤公开端点）+ 所有 /api/v1 变更操作集中补
/// CSRF `X-Requested-With` header 参数 + 403 响应（DRY，与 csrf_guard 中间件一致）。
pub fn build_openapi() -> Value {
    let mut doc = serde_json::to_value(ApiDoc::openapi()).unwrap_or_else(|_| json!({}));
    if !doc
        .get("components")
        .map(|c| c.is_object())
        .unwrap_or(false)
    {
        doc["components"] = json!({});
    }
    doc["components"]["securitySchemes"] = json!({
        "cookieAuth": { "type": "apiKey", "in": "cookie", "name": "godoword_session" }
    });
    if !doc.get("paths").map(|p| p.is_object()).unwrap_or(false) {
        doc["paths"] = json!({});
    }
    // 登记 openapi.json 自身（公开 GET，文档自描述完整）。
    doc["paths"]["/api/v1/openapi.json"] = json!({
        "get": {
            "tags": ["system"],
            "operationId": "openapi_json",
            "responses": {
                "200": {
                    "description": "OpenAPI 文档",
                    "content": { "application/json": { "schema": { "type": "object" } } }
                }
            }
        }
    });
    if let Some(paths) = doc.get_mut("paths").and_then(|p| p.as_object_mut()) {
        for (path, item) in paths.iter_mut() {
            for method in ["get", "post", "put", "delete", "patch"] {
                let Some(op) = item.get_mut(method).and_then(|o| o.as_object_mut()) else {
                    continue;
                };
                // 操作级 security 矩阵（审计 N1）。
                let sec = if is_public(path, method) {
                    json!([])
                } else {
                    json!([{ "cookieAuth": [] }])
                };
                op.insert("security".to_string(), sec);
                // CSRF：/api/v1 变更操作补 header 参数 + 403（运行时 register/login/reset 也要求自定义头）。
                if path.starts_with("/api/v1")
                    && matches!(method, "post" | "put" | "delete" | "patch")
                {
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
        // cookieAuth securityScheme（机器可读）。
        assert_eq!(
            doc["components"]["securitySchemes"]["cookieAuth"]["type"].as_str(),
            Some("apiKey"),
            "cookieAuth apiKey"
        );
        assert_eq!(
            doc["components"]["securitySchemes"]["cookieAuth"]["in"].as_str(),
            Some("cookie"),
            "cookieAuth in cookie"
        );
        // N1：不用根级全局 security（会误伤公开端点）。
        assert!(
            doc.get("security")
                .map(|s| s.as_array().map(|a| a.is_empty()).unwrap_or(true))
                .unwrap_or(true),
            "N1：无根级全局 security"
        );
        // N1：操作级 security 矩阵——公开 security:[]，受保护 cookieAuth；N8：所有变更方法 CSRF required+403。
        let paths = doc["paths"].as_object().expect("paths");
        let mut public_count = 0;
        let mut protected_count = 0;
        for (path, item) in paths {
            for method in ["get", "post", "put", "delete", "patch"] {
                let Some(op) = item.get(method) else { continue };
                let sec = op
                    .get("security")
                    .and_then(|s| s.as_array())
                    .unwrap_or_else(|| panic!("N1：{method} {path} 缺 security"));
                if is_public(path, method) {
                    assert!(sec.is_empty(), "N1：公开 {method} {path} security 应为空");
                    public_count += 1;
                } else {
                    assert!(
                        sec.iter().any(|s| s.get("cookieAuth").is_some()),
                        "N1：受保护 {method} {path} 应要求 cookieAuth"
                    );
                    protected_count += 1;
                }
                if path.starts_with("/api/v1")
                    && matches!(method, "post" | "put" | "delete" | "patch")
                {
                    let params = op
                        .get("parameters")
                        .and_then(|p| p.as_array())
                        .unwrap_or_else(|| panic!("N8：{method} {path} 缺 parameters"));
                    let csrf = params
                        .iter()
                        .find(|p| p["name"].as_str() == Some("X-Requested-With"))
                        .unwrap_or_else(|| panic!("N8：{method} {path} 缺 CSRF header"));
                    assert_eq!(csrf["required"].as_bool(), Some(true), "N8：CSRF required");
                    assert!(
                        op["responses"]["403"].is_object(),
                        "N8：{method} {path} 缺 403"
                    );
                }
            }
        }
        assert!(
            public_count >= 8,
            "N1：公开操作数（healthz/register/login 等）"
        );
        assert!(protected_count >= 10, "N1：受保护操作数");
        // 独立精确公开操作表（第四次复核建议：避免与生产 is_public 同源偏差——若误把受保护端点加入
        // is_public，此独立表会断言失败）。
        let expected_public: std::collections::HashSet<(&str, &str)> = [
            ("/healthz", "get"),
            ("/readyz", "get"),
            ("/api/v1/meta", "get"),
            ("/api/v1/config", "get"),
            ("/api/v1/openapi.json", "get"),
            ("/api/v1/auth/register", "post"),
            ("/api/v1/auth/login", "post"),
            ("/api/v1/auth/reset-password", "post"),
            ("/api/v1/auth/logout", "post"),
        ]
        .into_iter()
        .collect();
        for (path, item) in paths {
            for method in ["get", "post", "put", "delete", "patch"] {
                let Some(op) = item.get(method) else { continue };
                let sec = op.get("security").and_then(|s| s.as_array()).unwrap();
                if expected_public.contains(&(path.as_str(), method)) {
                    assert!(sec.is_empty(), "独立表：{method} {path} 应公开 security:[]");
                } else {
                    assert!(
                        sec.iter().any(|s| s.get("cookieAuth").is_some()),
                        "独立表：{method} {path} 应受保护 cookieAuth"
                    );
                }
            }
        }
        for (p, m) in &expected_public {
            assert!(doc["paths"][p][m].is_object(), "独立表：{m} {p} 须登记");
        }
        // openapi.json 自身登记（公开）。
        assert!(
            doc["paths"]["/api/v1/openapi.json"]["get"].is_object(),
            "N1：openapi.json 登记"
        );
        // delete 会话 404（越权/不存在）。
        assert!(
            doc["paths"]["/api/v1/sessions/{id}"]["delete"]["responses"]["404"].is_object(),
            "R5：delete 会话 404"
        );
    }
}
