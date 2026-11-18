//! 回合循环集成测试（mock 适配器，无真实上游）：验证 SSE 事件全序 + speak 执行 + 持久化 + 守护。

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::{self, StreamExt};
use kernel::{
    run_migrations, AudioCache, BindingResolver, MemAudio, MemSessions, OnDisconnect,
    ResolvedBinding, Runtime, SessionMeta, SessionRepo, KERNEL_MIGRATIONS,
};
use kernel_api::{
    AudioEvent, AudioFormat, AudioStream, ChatChunk, ChatRequest, ChatStream, FinishReason,
    KernelError, LlmFeatures, LlmProvider, ModelInfo, ProviderConn, ProviderError, SessionEvent,
    SessionId, SessionParams, Store, ToolCall, TtsProvider, TtsRequest, TtsUsage, Usage, UserId,
    VoiceInfo, WordTimestamp,
};
use secrecy::SecretString;
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use uuid::Uuid;

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

async fn runtime(always_tool: bool) -> (Runtime, Arc<MemSessions>, Arc<MemAudio>) {
    let sessions = Arc::new(MemSessions::new());
    let audio = Arc::new(MemAudio::new());
    let store = Arc::new(Store::Sqlite(
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap(),
    ));
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let rt = Runtime::new(
        Arc::new(MockLlm {
            calls: AtomicU32::new(0),
            always_tool,
        }),
        Arc::new(MockTts),
        Arc::new(MockBindings),
        sessions.clone(),
        audio.clone(),
        store,
    );
    (rt, sessions, audio)
}

#[tokio::test]
async fn turn_speak_full_sequence() {
    let (rt, sessions, audio) = runtime(false).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(user, session, SessionMeta::default())
        .await
        .unwrap();

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
    let cached = audio.get(audio_id, user).await.unwrap().expect("缓存音频");
    assert_eq!(cached.pcm.len(), 4800);
    assert_eq!(cached.words.len(), 1);

    // 转录持久化：user + assistant(tool_calls) + tool + assistant = 4
    let hist = sessions.history(user, session).await.unwrap();
    assert_eq!(hist.len(), 4);
    assert_eq!(hist[0].role, "user");
    assert_eq!(hist[1].role, "assistant");
    assert!(hist[1].tool_calls.is_some());
    assert_eq!(hist[2].role, "tool");
    assert_eq!(hist[3].role, "assistant");
}

#[tokio::test]
async fn tool_cap_guard() {
    let (rt, sessions, _audio) = runtime(true).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(
            user,
            session,
            SessionMeta {
                params: SessionParams {
                    tool_iteration_cap: 2,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();

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
    let store = Arc::new(Store::Sqlite(
        SqlitePool::connect("sqlite::memory:").await.unwrap(),
    ));
    let rt = Runtime::new(
        Arc::new(MockLlm {
            calls: AtomicU32::new(0),
            always_tool: false,
        }),
        Arc::new(MockTts),
        Arc::new(NoBinding),
        sessions.clone(),
        Arc::new(MemAudio::new()),
        store,
    );
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(user, session, SessionMeta::default())
        .await
        .unwrap();
    let events: Vec<SessionEvent> = rt.run_turn(user, session, "hi".to_string()).collect().await;
    assert!(events.iter().any(|e| matches!(e, SessionEvent::Error { code, action: Some(a), .. } if code == "unconfigured" && a == "settings")));
}

#[tokio::test]
async fn turn_metering_and_eviction() {
    let sessions = Arc::new(MemSessions::new());
    let audio = Arc::new(MemAudio::new());
    let db_dir = std::env::temp_dir().join(format!("gw-turn-meter-{}", Uuid::new_v4()));
    let store = Arc::new(
        kernel::open_sqlite(&db_dir.join("db").join("t.sqlite"))
            .await
            .unwrap(),
    );
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let rt = Runtime::new(
        Arc::new(MockLlm {
            calls: AtomicU32::new(0),
            always_tool: false,
        }),
        Arc::new(MockTts),
        Arc::new(MockBindings),
        sessions.clone(),
        audio.clone(),
        store.clone(),
    );
    let user = UserId::new();
    let session = SessionId::new();
    kernel::repo::ensure_user(&store, user.0).await.unwrap();
    // 低预算（5 token）+ 预置历史 → 装配时淘汰历史 span 入 outbox。
    sessions
        .create(
            user,
            session,
            SessionMeta {
                params: SessionParams {
                    context_budget_tokens: 5,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    for i in 0..3 {
        sessions
            .append(
                user,
                session,
                kernel_api::ChatMessage {
                    role: "user".to_string(),
                    content: format!("历史消息{i}啊啊啊啊"),
                    reasoning: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            )
            .await
            .unwrap();
    }
    let _events: Vec<SessionEvent> = rt
        .run_turn(user, session, "新消息".to_string())
        .collect()
        .await;

    // call_log 计量：至少 LLM 调用（MockLlm 两轮 → 2 条 llm）。
    let llm_calls: (i64,) = kernel_api::fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM call_log WHERE capability = 'llm'",
        (i64,)
    )
    .unwrap();
    assert!(llm_calls.0 >= 1, "call_log 记录 LLM 调用");
    // outbox：淘汰 span 发 session-evicted（module_id 哨兵 kernel）。
    let evicted: (i64,) = kernel_api::fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM progress_events WHERE kind = 'session-evicted' AND module_id = 'kernel'",
        (i64,)
    )
    .unwrap();
    assert!(evicted.0 >= 1, "淘汰 span 入 outbox（kernel 哨兵）");
    drop(store);
    std::fs::remove_dir_all(&db_dir).unwrap();
}

// current_thread：spawn 的 task 须待 await 才运行，保证 drop(stream) 先于 task 首次 send（确定断开时序）。
#[tokio::test(flavor = "current_thread")]
async fn disconnect_continue_completes() {
    let (rt, sessions, _audio) = runtime(false).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(
            user,
            session,
            SessionMeta {
                on_disconnect: OnDisconnect::Continue,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    // 立即 drop 流（模拟客户端断开），不消费任何事件。
    drop(rt.run_turn(user, session, "hi".to_string()));
    // continue：服务端 detach 跑完 → 转录含 assistant。
    let mut len = 0;
    for _ in 0..100 {
        len = sessions.history(user, session).await.unwrap().len();
        if len >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        len >= 2,
        "continue 断开后服务端跑完（user+assistant），实得 {len}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn disconnect_cancel_stops() {
    let (rt, sessions, _audio) = runtime(false).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(
            user,
            session,
            SessionMeta {
                on_disconnect: OnDisconnect::Cancel,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    drop(rt.run_turn(user, session, "hi".to_string()));
    // cancel：置取消标志 → 回合中止，仅 user 消息（无 assistant）。
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let hist = sessions.history(user, session).await.unwrap();
    assert_eq!(
        hist.len(),
        1,
        "cancel 断开后回合中止（仅 user），实得 {}",
        hist.len()
    );
    assert_eq!(hist[0].role, "user");
}
