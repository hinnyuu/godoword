//! 归一化错误分类。适配器将上游错误映射为 [`ProviderError`]，kernel 转为可操作 UI。

use thiserror::Error;

/// 上游 provider 错误（归一化）。
#[derive(Debug, Error)]
pub enum ProviderError {
    /// key 无效或过期，action：打开设置页。
    #[error("认证失败：{message}")]
    Auth { message: String },
    /// 余额或配额不足。
    #[error("配额不足：{message}")]
    Quota { message: String },
    /// 限流，可重试。
    #[error("限流")]
    RateLimit {
        retry_after: Option<std::time::Duration>,
    },
    /// 模型不可用。
    #[error("模型不可用：{model}")]
    ModelUnavailable { model: String },
    /// 网络错误。
    #[error("网络错误：{message}")]
    Network { message: String, retryable: bool },
    /// 上下文超长。
    #[error("上下文超出模型窗口")]
    ContextLength,
    /// 内容过滤。
    #[error("内容被过滤")]
    ContentFilter,
    /// 其他内部错误。
    #[error("内部错误：{message}")]
    Internal { message: String },
}

impl ProviderError {
    /// 是否可重试。
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimit { .. }
                | Self::Network {
                    retryable: true,
                    ..
                }
        )
    }
}

/// 认证错误。
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("凭据无效")]
    InvalidCredentials,
    #[error("账户被禁用")]
    Disabled,
    #[error("注册受限：{0}")]
    RegistrationRestricted(String),
    #[error("OIDC 流程错误：{0}")]
    Oidc(String),
    #[error("内部错误：{0}")]
    Internal(String),
}

/// 教学模块错误。
#[derive(Debug, Error)]
pub enum ModuleError {
    #[error("工具执行失败：{0}")]
    Tool(String),
    #[error("内部错误：{0}")]
    Internal(String),
}

/// 教学方案错误。
#[derive(Debug, Error)]
pub enum PlanError {
    #[error("方案生成失败：{0}")]
    Generation(String),
    #[error("内部错误：{0}")]
    Internal(String),
}

/// kernel 服务错误（经 ModuleContext 返回）。
#[derive(Debug, Error)]
pub enum KernelError {
    #[error("未配置绑定：{0}")]
    Unconfigured(String),
    #[error("存储错误：{0}")]
    Storage(String),
    #[error("内部错误：{0}")]
    Internal(String),
}
