//! 认证适配器契约。本地与 OIDC 统一接口，kernel 会话层共用。

use std::collections::HashMap;

use async_trait::async_trait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use crate::error::AuthError;
use crate::types::{RouteSpec, UserId};

/// 认证类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthKind {
    Local,
    Oidc,
}

/// OIDC 身份断言（subject 为公开标识符，非秘密）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcIdentity {
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: bool,
}

/// 认证请求。
#[derive(Debug)]
pub enum AuthRequest {
    Password {
        username: String,
        password: SecretString,
    },
    OidcStart,
    OidcCallback {
        params: HashMap<String, String>,
    },
}

/// 认证结果。
#[derive(Debug)]
pub enum AuthOutcome {
    /// 验身成功，kernel 会话层发 cookie。
    Authenticated(UserId),
    /// OIDC 跳转。
    Redirect(String),
    /// 待账户关联（P13）。
    PendingLink(OidcIdentity),
}

/// 认证适配器（auth-local / auth-oidc）。
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// 适配器标识，如 "auth-local"。
    fn id(&self) -> &'static str;
    fn kind(&self) -> AuthKind;
    async fn authenticate(&self, req: AuthRequest) -> Result<AuthOutcome, AuthError>;
    /// OIDC 的 login/callback 路由；local 用 kernel 标准端点。
    fn extra_routes(&self) -> Vec<RouteSpec> {
        vec![]
    }
}
