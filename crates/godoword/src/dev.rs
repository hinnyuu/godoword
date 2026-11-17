//! DEV-DEBT T1：开发态旁路（env key + 固定 dev user）。
//! 仅 `dev-bypass` feature 编译，release Dockerfile 永不启用；M1c 真认证/绑定/加密落地后**删除**本模块。
//! key 仅经 env 传入，绝不落盘/入日志/入仓库。

use async_trait::async_trait;
use kernel::{BindingResolver, ResolvedBinding};
use kernel_api::{Capability, KernelError, ModuleId, ProviderConn, Role, UserId};
use secrecy::SecretString;
use serde_json::json;

/// 固定 dev 用户（M1c 真认证后删除）。
pub fn dev_user() -> UserId {
    UserId(uuid::Uuid::from_u128(1))
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|s| !s.is_empty())
}

/// env 绑定解析器（dev 旁路）：从 GODOWORD_DEV_* 读 key/base_url/model。
pub struct EnvResolver;

#[async_trait]
impl BindingResolver for EnvResolver {
    async fn resolve(
        &self,
        _user: UserId,
        _module: Option<&ModuleId>,
        capability: Capability,
        _role: Role,
    ) -> Result<ResolvedBinding, KernelError> {
        match capability {
            Capability::Llm => {
                let key = env("GODOWORD_DEV_LLM_KEY")
                    .or_else(|| env("QWEN_TOKEN_PLAN_KEY"))
                    .ok_or_else(|| KernelError::Unconfigured("GODOWORD_DEV_LLM_KEY".to_string()))?;
                let base_url = env("GODOWORD_DEV_LLM_BASE_URL").unwrap_or_else(|| {
                    "https://token-plan.maas.qianwenaiapi.com/compatible-mode/v1".to_string()
                });
                let model = env("GODOWORD_DEV_LLM_MODEL")
                    .unwrap_or_else(|| "qwen4-max-preview".to_string());
                Ok(ResolvedBinding {
                    conn: ProviderConn {
                        base_url,
                        api_key: SecretString::from(key),
                        extra: json!({}),
                    },
                    model,
                    params: json!({}),
                })
            }
            Capability::Tts => {
                let key = env("GODOWORD_DEV_TTS_KEY")
                    .or_else(|| env("GODOWORD_DEV_LLM_KEY"))
                    .or_else(|| env("QWEN_TOKEN_PLAN_KEY"))
                    .ok_or_else(|| KernelError::Unconfigured("GODOWORD_DEV_TTS_KEY".to_string()))?;
                let base_url = env("GODOWORD_DEV_TTS_BASE_URL")
                    .unwrap_or_else(|| "https://token-plan.maas.qianwenaiapi.com".to_string());
                let model = env("GODOWORD_DEV_TTS_MODEL")
                    .unwrap_or_else(|| "qwen-audio-3.0-tts-plus".to_string());
                Ok(ResolvedBinding {
                    conn: ProviderConn {
                        base_url,
                        api_key: SecretString::from(key),
                        extra: json!({}),
                    },
                    model,
                    params: json!({ "voice": "longanlingxin" }),
                })
            }
            Capability::Stt => Err(KernelError::Unconfigured("stt".to_string())),
        }
    }
}
