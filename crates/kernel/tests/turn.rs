//! 回合循环集成测试（mock 适配器，无真实上游）：验证 SSE 事件全序 + speak 执行 + 持久化 + 守护。

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::{self, StreamExt};
use kernel::{
    AudioCache, BindingResolver, MemAudio, MemSessions, ResolvedBinding, Runtime, SessionRepo,
};
use kernel_api::{
    AudioEvent, AudioFormat, AudioStream, ChatChunk, ChatRequest, ChatStream, FinishReason,
    KernelError, LlmFeatures, LlmProvider, ModelInfo, ProviderConn, ProviderError, SessionEvent,
    SessionId, SessionParams, ToolCall, TtsProvider, TtsRequest, TtsUsage, Usage, UserId,
    VoiceInfo, WordTimestamp,
};
use secrecy::SecretString;
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// mock LLM：首轮发 text + speak 工具调用 + usage + finish(tool_calls)；次轮发 text + finish(stop)。
struct MockLlm {
    calls: AtomicU32,
    always_tool: bool,
}

#[async_trait]
impl LlmProvider for MockLlm {
    fn id(&self) -> &'static str {
        "mock-llm"
    }
    fn features(&self) -> LlmFeatures {
        LlmFeatures {
            tool_calling: true,
            reasoning: true,
            streaming: true,
        }
    }
    async fn list_models(&self, _: &ProviderConn) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![])
    }
    async fn chat_stream(
        &self,
        _: &ProviderConn,
        _: ChatRequest,
    ) -> Result<ChatStream, ProviderError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let speak = || {
            ChatChunk::ToolCalls(vec![ToolCall {
                id: format!("call_{n}"),
                name: "speak".to_string(),
                arguments: json!({ "text": "こんにちは", "rate": 0.8, "language_hints": ["ja"] }),
            }])
        };
        let chunks = if self.always_tool {
            vec![Ok(speak()), Ok(ChatChunk::Finish(FinishReason::ToolCalls))]
        } else if n == 0 {
            vec![
                Ok(ChatChunk::TextDelta("让我朗读。".to_string())),
                Ok(speak()),
                Ok(ChatChunk::Usage(Usage {
                    prompt_tokens: Some(10),
                    completion_tokens: Some(5),
                    reasoning_tokens: None,
                    total_tokens: Some(15),
                })),
                Ok(ChatChunk::Finish(FinishReason::ToolCalls)),
            ]
        } else {
            vec![
                Ok(ChatChunk::TextDelta("读完了。".to_string())),
                Ok(ChatChunk::Finish(FinishReason::Stop)),
            ]
        };
        Ok(Box::pin(stream::iter(chunks)))
    }
}

/// mock TTS：发 Start/Chunk/Words/Usage/End。
struct MockTts;

#[async_trait]
impl TtsProvider for MockTts {
    fn id(&self) -> &'static str {
        "mock-tts"
    }
    fn models(&self) -> Vec<ModelInfo> {
        vec![]
    }
    async fn list_voices(
        &self,
        _: &ProviderConn,
        _: &str,
    ) -> Result<Vec<VoiceInfo>, ProviderError> {
        Ok(vec![])
    }
    async fn synthesize_stream(
        &self,
        _: &ProviderConn,
        _: TtsRequest,
    ) -> Result<AudioStream, ProviderError> {
        let pcm = Bytes::from(vec![0u8; 4800]);
        let evs = vec![
            Ok(AudioEvent::Start {
                format: AudioFormat::Pcm,
                sample_rate: 24000,
            }),
            Ok(AudioEvent::Chunk {
                seq: 0,
                pcm: pcm.clone(),
            }),
            Ok(AudioEvent::Words(vec![WordTimestamp {
                text: "こんにちは".to_string(),
                begin_index: 0,
                end_index: 1,
                begin_time: 400,
                end_time: 800,
            }])),
            Ok(AudioEvent::Usage(TtsUsage {
                characters: Some(5),
                input_tokens: Some(2),
                output_tokens: Some(11),
            })),
            Ok(AudioEvent::End {
                complete: Some(pcm),
            }),
        ];
        Ok(Box::pin(stream::iter(evs)))
    }
}

struct MockBindings;

#[async_trait]
impl BindingResolver for MockBindings {
    async fn resolve(
        &self,
        _: UserId,
        _: Option<&kernel_api::ModuleId>,
        _: kernel_api::Capability,
        _: kernel_api::Role,
    ) -> Result<ResolvedBinding, KernelError> {
        Ok(ResolvedBinding {
            conn: ProviderConn {
                base_url: "mock://".to_string(),
                api_key: SecretString::from("test"),
                extra: json!({}),
            },
            model: "mock-model".to_string(),
            params: json!({}),
        })
    }
}

fn runtime(always_tool: bool, cap: u32) -> (Runtime, Arc<MemSessions>, Arc<MemAudio>) {
    let sessions = Arc::new(MemSessions::new());
    let audio = Arc::new(MemAudio::new());
    let rt = Runtime::new(
        Arc::new(MockLlm {
            calls: AtomicU32::new(0),
            always_tool,
        }),
        Arc::new(MockTts),
        Arc::new(MockBindings),
        sessions.clone(),
        audio.clone(),
    )
    .with_params(SessionParams {
        tool_iteration_cap: cap,
        ..Default::default()
    });
    (rt, sessions, audio)
}

#[tokio::test]
async fn turn_speak_full_sequence() {
    let (rt, sessions, audio) = runtime(false, 8);
    let user = UserId::new();
    let session = SessionId::new();
    sessions.create(user, session).await.unwrap();

    let events: Vec<SessionEvent> = rt
        .run_turn(user, session, "教我こんにちは".to_string())
        .collect()
        .await;

    assert!(matches!(
        events.first(),
        Some(SessionEvent::RunStart { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(SessionEvent::RunEnd {
            finish_reason: FinishReason::Stop,
            ..
        })
    ));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::ToolCall { name, .. } if name == "speak")));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::AudioStart { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::AudioChunk { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::WordTimestamps { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::AudioEnd { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::ToolResult { ok: true, .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        SessionEvent::Usage {
            capability: kernel_api::Capability::Tts,
            ..
        }
    )));

    // 音频入缓存
    let audio_id = events
        .iter()
        .find_map(|e| match e {
            SessionEvent::AudioStart { audio_id, .. } => Some(*audio_id),
            _ => None,
        })
        .expect("audio_id");
    let cached = audio.get(audio_id).await.unwrap().expect("缓存音频");
    assert_eq!(cached.pcm.len(), 4800);
    assert_eq!(cached.words.len(), 1);

    // 转录持久化：user + assistant(tool_calls) + tool + assistant = 4
    let hist = sessions.history(session).await.unwrap();
    assert_eq!(hist.len(), 4);
    assert_eq!(hist[0].role, "user");
    assert_eq!(hist[1].role, "assistant");
    assert!(hist[1].tool_calls.is_some());
    assert_eq!(hist[2].role, "tool");
    assert_eq!(hist[3].role, "assistant");
}

#[tokio::test]
async fn tool_cap_guard() {
    let (rt, sessions, _audio) = runtime(true, 2);
    let user = UserId::new();
    let session = SessionId::new();
    sessions.create(user, session).await.unwrap();

    let events: Vec<SessionEvent> = rt
        .run_turn(user, session, "朗读".to_string())
        .collect()
        .await;

    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::Error { code, .. } if code == "tool_cap")));
    assert!(matches!(
        events.last(),
        Some(SessionEvent::RunEnd {
            finish_reason: FinishReason::Error,
            ..
        })
    ));
}

#[tokio::test]
async fn unconfigured_binding_errors() {
    struct NoBinding;
    #[async_trait]
    impl BindingResolver for NoBinding {
        async fn resolve(
            &self,
            _: UserId,
            _: Option<&kernel_api::ModuleId>,
            _: kernel_api::Capability,
            _: kernel_api::Role,
        ) -> Result<ResolvedBinding, KernelError> {
            Err(KernelError::Unconfigured("llm".to_string()))
        }
    }
    let sessions = Arc::new(MemSessions::new());
    let rt = Runtime::new(
        Arc::new(MockLlm {
            calls: AtomicU32::new(0),
            always_tool: false,
        }),
        Arc::new(MockTts),
        Arc::new(NoBinding),
        sessions.clone(),
        Arc::new(MemAudio::new()),
    );
    let user = UserId::new();
    let session = SessionId::new();
    sessions.create(user, session).await.unwrap();
    let events: Vec<SessionEvent> = rt.run_turn(user, session, "hi".to_string()).collect().await;
    assert!(events.iter().any(|e| matches!(e, SessionEvent::Error { code, action: Some(a), .. } if code == "unconfigured" && a == "settings")));
}
