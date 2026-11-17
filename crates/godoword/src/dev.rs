//! DEV-DEBT T1：开发态旁路（固定 dev user + env 幂等种入 DB）。
//! 仅 `dev-bypass` feature 编译，release Dockerfile 永不启用；M1c 真认证落地后**删除**本模块。
//! M1b 绑定解析已转 DB（DbBindingResolver）；本模块把 GODOWORD_DEV_* env 的 provider/binding
//! 幂等种入 DB（经 MasterKey 加密），使本地真 key 测试走完整 DB 加密路径。key 仅经 env，绝不入日志/仓库。

use kernel::{repo, MasterKey};
use kernel_api::{Store, UserId};
use serde_json::json;

/// 固定 dev 用户（M1c 真认证后删除）。
pub fn dev_user() -> UserId {
    UserId(uuid::Uuid::from_u128(1))
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|s| !s.is_empty())
}

const DEV_LLM_LABEL: &str = "dev-llm";
const DEV_TTS_LABEL: &str = "dev-tts";

/// 幂等种入 dev provider/binding（已存在同标签 provider 则跳过）。
pub async fn seed(store: &Store, master: &MasterKey, user: UserId) {
    if let Err(e) = repo::ensure_user(store, user.0).await {
        tracing::warn!("dev 用户种入失败：{e}");
        return;
    }
    let providers = repo::list_providers(store, user.0)
        .await
        .unwrap_or_default();
    let has = |label: &str| providers.iter().any(|p| p.label == label);

    // LLM
    if !has(DEV_LLM_LABEL) {
        if let Some(key) = env("GODOWORD_DEV_LLM_KEY").or_else(|| env("QWEN_TOKEN_PLAN_KEY")) {
            let base_url = env("GODOWORD_DEV_LLM_BASE_URL").unwrap_or_else(|| {
                "https://token-plan.maas.qianwenaiapi.com/compatible-mode/v1".to_string()
            });
            let model =
                env("GODOWORD_DEV_LLM_MODEL").unwrap_or_else(|| "qwen4-max-preview".to_string());
            match repo::create_provider(
                store,
                master,
                user.0,
                "llm-openai",
                DEV_LLM_LABEL,
                &base_url,
                &key,
                json!({}),
            )
            .await
            {
                Ok(p) => {
                    let _ = repo::create_binding(
                        store,
                        user.0,
                        "*",
                        "llm",
                        "default",
                        p.id,
                        &model,
                        json!({}),
                    )
                    .await;
                    tracing::info!("dev LLM provider/binding 已种入 DB（加密）");
                }
                Err(e) => tracing::warn!("dev LLM provider 种入失败：{e}"),
            }
        }
    }

    // TTS
    if !has(DEV_TTS_LABEL) {
        if let Some(key) = env("GODOWORD_DEV_TTS_KEY")
            .or_else(|| env("GODOWORD_DEV_LLM_KEY"))
            .or_else(|| env("QWEN_TOKEN_PLAN_KEY"))
        {
            let base_url = env("GODOWORD_DEV_TTS_BASE_URL")
                .unwrap_or_else(|| "https://token-plan.maas.qianwenaiapi.com".to_string());
            let model = env("GODOWORD_DEV_TTS_MODEL")
                .unwrap_or_else(|| "qwen-audio-3.0-tts-plus".to_string());
            let voice =
                env("GODOWORD_DEV_TTS_VOICE").unwrap_or_else(|| "longanlingxin".to_string());
            match repo::create_provider(
                store,
                master,
                user.0,
                "tts-qwen-audio",
                DEV_TTS_LABEL,
                &base_url,
                &key,
                json!({}),
            )
            .await
            {
                Ok(p) => {
                    let _ = repo::create_binding(
                        store,
                        user.0,
                        "*",
                        "tts",
                        "default",
                        p.id,
                        &model,
                        json!({ "voice": voice }),
                    )
                    .await;
                    tracing::info!("dev TTS provider/binding 已种入 DB（加密）");
                }
                Err(e) => tracing::warn!("dev TTS provider 种入失败：{e}"),
            }
        }
    }
}
