//! 审计修复测试夹具（cfg(test)）：内存 SQLite + mock 适配器 + 真实 AppState，
//! handler 级失败/越权/计量负例锚定（不触网、不触 /data）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_api::{
    AudioEvent, AudioFormat, ChatStream, LlmFeatures, LlmProvider, ModelInfo, ProviderConn,
    ProviderError, TtsProvider, TtsRequest, TtsUsage, VoiceInfo, WordTimestamp,
};

use crate::rest::AppState;

/// mock LLM（测试端点/绑定解析用；chat_stream 不涉及）。
pub struct MockLlm;

#[async_trait]
impl LlmProvider for MockLlm {
    fn id(&self) -> &'static str {
        "llm-openai"
    }
    fn features(&self) -> LlmFeatures {
        LlmFeatures {
            tool_calling: true,
            reasoning: true,
            streaming: true,
        }
    }
    async fn list_models(&self, _conn: &ProviderConn) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![ModelInfo {
            id: "mock-model".to_string(),
            display_name: None,
        }])
    }
    async fn chat_stream(
        &self,
        _conn: &ProviderConn,
        _req: kernel_api::ChatRequest,
    ) -> Result<ChatStream, ProviderError> {
        Err(ProviderError::Internal {
            message: "mock 不支持 chat".to_string(),
        })
    }
}

/// mock TTS 行为模式。
#[derive(Clone, Copy, PartialEq)]
pub enum TtsMode {
    /// 正常合成（Start/Chunk/Usage/Words/End，characters=5）。
    Ok,
    /// 流永久挂起（超时负例）。
    Hang,
    /// 上游 key 无效（502 语义负例）。
    AuthErr,
}

pub struct MockTts(pub TtsMode);

#[async_trait]
impl TtsProvider for MockTts {
    fn id(&self) -> &'static str {
        "tts-qwen-audio"
    }
    fn models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "mock-tts".to_string(),
            display_name: None,
        }]
    }
    async fn list_voices(
        &self,
        _conn: &ProviderConn,
        _model: &str,
    ) -> Result<Vec<VoiceInfo>, ProviderError> {
        Ok(vec![])
    }
    async fn synthesize_stream(
        &self,
        _conn: &ProviderConn,
        _req: TtsRequest,
    ) -> Result<kernel_api::AudioStream, ProviderError> {
        use futures_util::StreamExt;
        match self.0 {
            TtsMode::Ok => {
                let events: Vec<Result<AudioEvent, ProviderError>> = vec![
                    Ok(AudioEvent::Start {
                        format: AudioFormat::Pcm,
                        sample_rate: 24000,
                    }),
                    Ok(AudioEvent::Chunk {
                        seq: 0,
                        pcm: bytes::Bytes::from(vec![0u8; 4800]),
                    }),
                    Ok(AudioEvent::Usage(TtsUsage {
                        characters: Some(5),
                        input_tokens: Some(2),
                        output_tokens: Some(11),
                    })),
                    Ok(AudioEvent::Words(vec![WordTimestamp {
                        text: "こんにちは".to_string(),
                        begin_index: 0,
                        end_index: 5,
                        begin_time: 0,
                        end_time: 800,
                    }])),
                    Ok(AudioEvent::End {
                        complete: Some(bytes::Bytes::from(vec![0u8; 4800])),
                    }),
                ];
                Ok(futures_util::stream::iter(events).boxed())
            }
            TtsMode::Hang => Ok(futures_util::stream::pending().boxed()),
            TtsMode::AuthErr => Err(ProviderError::Auth {
                message: "mock key 无效".to_string(),
            }),
        }
    }
}

/// 构建隔离 AppState（内存 SQLite + 迁移 + 临时主密钥 + MemAudio + mock 适配器）。
pub async fn state(tts_mode: TtsMode) -> Arc<AppState> {
    let store = Arc::new(kernel::open_memory_sqlite().await.unwrap());
    kernel::run_migrations(&store, kernel::KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let key_dir = std::env::temp_dir().join(format!("gw-test-key-{}", uuid::Uuid::new_v4()));
    let master = Arc::new(kernel::MasterKey::load(&key_dir).unwrap());
    std::fs::remove_dir_all(&key_dir).ok();
    let sessions: Arc<dyn kernel::SessionRepo> = Arc::new(kernel::DbSessions::new(store.clone()));
    let audio: Arc<dyn kernel::AudioCache> = Arc::new(kernel::MemAudio::new());
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm);
    let tts: Arc<dyn TtsProvider> = Arc::new(MockTts(tts_mode));
    let resolver = Arc::new(kernel::DbBindingResolver::new(
        store.clone(),
        master.clone(),
    ));
    // 模块注册表（M2 P1）：teach-free 注册（与生产同构）+ 临时资产目录。
    let assets_dir = std::env::temp_dir().join(format!("gw-test-assets-{}", uuid::Uuid::new_v4()));
    let assets = Arc::new(kernel::AssetStore::new(assets_dir, store.clone()));
    let mut registry = kernel::ModuleRegistry::new();
    #[cfg(feature = "teach-free")]
    {
        let factory = Arc::new(kernel::KernelCtxFactory::new(
            kernel_api::ModuleId(teach_free::TEACH_FREE_ID.to_string()),
            store.clone(),
            resolver.clone(),
            llm.clone(),
            tts.clone(),
            assets.clone(),
            None,
        ));
        registry
            .register_yaml(teach_free::module_yaml(), teach_free::module(), factory)
            .unwrap();
    }
    let registry = Arc::new(registry);
    let runtime = Arc::new(
        kernel::Runtime::new(
            llm.clone(),
            tts.clone(),
            resolver,
            sessions.clone(),
            audio.clone(),
            store.clone(),
        )
        .with_modules(registry.clone()),
    );
    Arc::new(AppState {
        sessions,
        audio,
        runtime,
        store,
        cfg: kernel::Config::default(),
        master,
        llm,
        tts,
        registry,
        rate_limiter: Arc::new(crate::auth::RateLimiter::new()),
        trusted_proxies: vec![],
    })
}

/// 注册首用户（管理员）+ 建会话令牌（cookie 用）。
pub async fn user_with_token(st: &AppState, username: &str) -> (kernel_api::UserId, String) {
    let user = auth_local::register(&st.store, username, "test-pass-123", None, None, false)
        .await
        .unwrap();
    let token = auth_local::create_session(&st.store, user, chrono::Duration::hours(1))
        .await
        .unwrap();
    (user, token)
}

/// 建 TTS provider（mock key 加密存储）。
pub async fn tts_provider(st: &AppState, user: kernel_api::UserId) -> uuid::Uuid {
    kernel::repo::create_provider(
        &st.store,
        &st.master,
        user.0,
        "tts-qwen-audio",
        "mock-tts",
        "https://mock.example",
        "mock-key",
        serde_json::json!({}),
    )
    .await
    .unwrap()
    .id
}
