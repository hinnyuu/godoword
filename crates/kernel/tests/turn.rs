//! 回合循环集成测试（mock 适配器，无真实上游）：验证 SSE 事件全序 + speak 执行 + 持久化 + 守护。

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::{self, StreamExt};
use kernel::{
    run_migrations, AudioCache, BindingResolver, DbSessions, MemAudio, MemSessions, OnDisconnect,
    ResolvedBinding, Runtime, SessionMeta, SessionRepo, KERNEL_MIGRATIONS,
};
use kernel_api::{
    AudioEvent, AudioFormat, AudioStream, ChatChunk, ChatRequest, ChatStream, Eviction,
    FinishReason, KernelError, LlmFeatures, LlmProvider, ModelInfo, ProviderConn, ProviderError,
    SessionEvent, SessionId, SessionParams, Store, ToolCall, TtsProvider, TtsRequest, TtsUsage,
    Usage, UserId, VoiceInfo, WordTimestamp,
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

/// 故障注入音频缓存（审计 R2）：put 恒失败，验证运行时错误语义（无假成功）。
struct FailAudio;

#[async_trait]
impl AudioCache for FailAudio {
    async fn put(&self, _: kernel_api::AudioId, _: kernel::CachedAudio) -> Result<(), KernelError> {
        Err(KernelError::Storage("注入：音频缓存写失败".to_string()))
    }
    async fn get(
        &self,
        _: kernel_api::AudioId,
        _: UserId,
    ) -> Result<Option<kernel::CachedAudio>, KernelError> {
        Ok(None)
    }
}

/// 注入自定义音频缓存的运行时（mock LLM/TTS/绑定 + 内存会话）。
async fn runtime_with_audio(audio: Arc<dyn AudioCache>) -> (Runtime, Arc<MemSessions>) {
    let sessions = Arc::new(MemSessions::new());
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
            always_tool: false,
        }),
        Arc::new(MockTts),
        Arc::new(MockBindings),
        sessions.clone(),
        audio,
        store,
    );
    (rt, sessions)
}

#[tokio::test]
async fn audio_put_failure_no_fake_success() {
    let (rt, sessions) = runtime_with_audio(Arc::new(FailAudio)).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(user, session, SessionMeta::default())
        .await
        .unwrap();
    let events: Vec<SessionEvent> = rt
        .run_turn(user, session, "朗读".to_string())
        .collect()
        .await;
    // put 失败：无正常 AudioEnd（无假回放 URL）。
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SessionEvent::AudioEnd { .. })),
        "R2：put 失败不发 AudioEnd"
    );
    // 错误语义：storage Error。
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::Error { code, .. } if code == "storage")),
        "R2：put 失败发 storage 错误"
    );
    // ToolResult ok=false（LLM 见失败），无成功 ToolResult。
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::ToolResult { ok: false, .. })),
        "R2：put 失败 ToolResult ok=false"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SessionEvent::ToolResult { ok: true, .. })),
        "R2：无成功 ToolResult"
    );
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
    // DbSessions（生产路径）：会话状态与 outbox 同 Store，evict_session 原子落库。
    let sessions = Arc::new(DbSessions::new(store.clone()));
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
    // 第二回合：水位推进后不重复淘汰已处理 span（审计 C7：消息 ID 不重复入 outbox）。
    let _events2: Vec<SessionEvent> = rt
        .run_turn(user, session, "再一条".to_string())
        .collect()
        .await;
    let details: Vec<(serde_json::Value,)> = kernel_api::fetch_all_both!(
        &*store,
        "SELECT detail FROM progress_events WHERE kind = 'session-evicted'",
        (serde_json::Value,)
    )
    .unwrap();
    let mut seen = std::collections::HashSet::new();
    for (d,) in details {
        if let Some(ids) = d["message_ids"].as_array() {
            for id in ids {
                assert!(
                    seen.insert(id.as_str().unwrap().to_string()),
                    "C7：同一消息 ID 不重复入 outbox"
                );
            }
        }
    }
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

/// 洪泛 LLM（审计 R3）：单轮发 300 个 TextDelta（>256 mpsc 容量）+ Stop，撑满投递通道制造背压。
struct FloodLlm;

#[async_trait]
impl LlmProvider for FloodLlm {
    fn id(&self) -> &'static str {
        "flood-llm"
    }
    fn features(&self) -> LlmFeatures {
        LlmFeatures {
            tool_calling: false,
            reasoning: false,
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
        let mut chunks: Vec<Result<ChatChunk, ProviderError>> = Vec::new();
        for i in 0..300 {
            chunks.push(Ok(ChatChunk::TextDelta(format!("t{i}"))));
        }
        chunks.push(Ok(ChatChunk::Finish(FinishReason::Stop)));
        Ok(Box::pin(stream::iter(chunks)))
    }
}

async fn runtime_flood(turn_timeout: std::time::Duration) -> (Runtime, Arc<MemSessions>) {
    let sessions = Arc::new(MemSessions::new());
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
        Arc::new(FloodLlm),
        Arc::new(MockTts),
        Arc::new(MockBindings),
        sessions.clone(),
        Arc::new(MemAudio::new()),
        store,
    )
    .with_turn_timeout(turn_timeout);
    (rt, sessions)
}

/// R3/A1：receiver 存活但不消费 → 满 mpsc → 硬超时 try_send 不永久阻塞 → 任务结束释放活跃锁。
#[tokio::test(flavor = "current_thread")]
async fn full_mpsc_timeout_releases_lock() {
    let (rt, sessions) = runtime_flood(std::time::Duration::from_millis(100)).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(user, session, SessionMeta::default())
        .await
        .unwrap();
    // 持流不消费（rx 存活）：300 delta 填满 256 mpsc → 任务阻塞 send → 硬超时 → 任务结束。
    let _stream1 = rt.run_turn(user, session, "x".to_string());
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    // 活跃锁已释放：新回合不 busy（首事件 RunStart，正常完成）。
    let events2: Vec<SessionEvent> = rt.run_turn(user, session, "y".to_string()).collect().await;
    assert!(
        matches!(events2.first(), Some(SessionEvent::RunStart { .. })),
        "R3/A1：满 mpsc 超时后活跃锁释放（新回合不 busy）"
    );
    assert!(
        matches!(
            events2.last(),
            Some(SessionEvent::RunEnd {
                finish_reason: FinishReason::Stop,
                ..
            })
        ),
        "新回合正常完成"
    );
}

/// R3：shutdown 超时任务真 abort+join（卡在满 mpsc 无法 cancel）→ 活跃锁释放，checkpoint 前无并发写。
#[tokio::test(flavor = "current_thread")]
async fn shutdown_aborts_stuck_tasks() {
    let (rt, sessions) = runtime_flood(std::time::Duration::from_secs(120)).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(user, session, SessionMeta::default())
        .await
        .unwrap();
    let stream1 = rt.run_turn(user, session, "x".to_string());
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    // 任务卡在满 mpsc（cancel 标志无法被处理）；短 grace shutdown → abort+join → 任务终止。
    rt.shutdown(std::time::Duration::from_millis(50)).await;
    // 卡死任务已被 abort+join：第一条流必然关闭（tx drop，不永久挂起）→ exec drop → ActiveGuard 释放。
    // （shutdown 后拒绝新回合 N6，故以流关闭验证任务终止，而非再发新回合。）
    let events: Vec<SessionEvent> = stream1.collect().await;
    assert!(
        events.len() <= 258,
        "R3：shutdown abort+join 后卡死流关闭（任务终止，实得 {}）",
        events.len()
    );
}

/// summarize 计数 LLM（审计 R4）：系统提示含"压缩"者计为 summarize 调用（教师 persona 不含此词）。
struct SummarizeLlm {
    summarize_calls: AtomicU32,
}

#[async_trait]
impl LlmProvider for SummarizeLlm {
    fn id(&self) -> &'static str {
        "summarize-llm"
    }
    fn features(&self) -> LlmFeatures {
        LlmFeatures {
            tool_calling: false,
            reasoning: false,
            streaming: true,
        }
    }
    async fn list_models(&self, _: &ProviderConn) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![])
    }
    async fn chat_stream(
        &self,
        _: &ProviderConn,
        req: ChatRequest,
    ) -> Result<ChatStream, ProviderError> {
        let is_summary = req
            .messages
            .first()
            .map(|m| m.content.contains("压缩"))
            .unwrap_or(false);
        let text = if is_summary {
            self.summarize_calls.fetch_add(1, Ordering::SeqCst);
            "摘要片段"
        } else {
            "回复"
        };
        Ok(Box::pin(stream::iter(vec![
            Ok(ChatChunk::TextDelta(text.to_string())),
            Ok(ChatChunk::Finish(FinishReason::Stop)),
        ])))
    }
}

async fn runtime_summarize() -> (
    Runtime,
    Arc<DbSessions>,
    Arc<Store>,
    Arc<SummarizeLlm>,
    std::path::PathBuf,
) {
    let db_dir = std::env::temp_dir().join(format!("gw-summarize-{}", Uuid::new_v4()));
    let store = Arc::new(
        kernel::open_sqlite(&db_dir.join("db").join("t.sqlite"))
            .await
            .unwrap(),
    );
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let sessions = Arc::new(DbSessions::new(store.clone()));
    let llm = Arc::new(SummarizeLlm {
        summarize_calls: AtomicU32::new(0),
    });
    let rt = Runtime::new(
        llm.clone(),
        Arc::new(MockTts),
        Arc::new(MockBindings),
        sessions.clone(),
        Arc::new(MemAudio::new()),
        store.clone(),
    );
    (rt, sessions, store, llm, db_dir)
}

fn hist_msg(content: String) -> kernel_api::ChatMessage {
    kernel_api::ChatMessage {
        role: "user".to_string(),
        content,
        reasoning: None,
        tool_calls: None,
        tool_call_id: None,
    }
}

/// R4：summarize 多回合行为——各回合新 span 各摘要一次，水位推进，滚动摘要累积，outbox 消息 ID 不重复。
#[tokio::test]
async fn summarize_multi_turn_no_dup() {
    let (rt, sessions, store, llm, db_dir) = runtime_summarize().await;
    let user = UserId::new();
    let session = SessionId::new();
    kernel::repo::ensure_user(&store, user.0).await.unwrap();
    sessions
        .create(
            user,
            session,
            SessionMeta {
                params: SessionParams {
                    context_budget_tokens: 5,
                    eviction: Eviction::Summarize,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    for i in 0..3 {
        sessions
            .append(user, session, hist_msg(format!("历史{i}啊啊啊啊")))
            .await
            .unwrap();
    }
    let _e1: Vec<SessionEvent> = rt
        .run_turn(user, session, "新消息".to_string())
        .collect()
        .await;
    assert_eq!(
        llm.summarize_calls.load(Ordering::SeqCst),
        1,
        "R4：首回合 summarize 一次"
    );
    let m1 = sessions.meta(user, session).await.unwrap().unwrap();
    assert!(m1.evicted_up_to > 0, "R4：水位推进");
    assert!(
        m1.summary.as_deref().unwrap().contains("摘要片段"),
        "R4：滚动摘要含片段"
    );
    // 第二回合：新增历史 → 新 span 淘汰 → summarize 再一次。
    for i in 0..3 {
        sessions
            .append(user, session, hist_msg(format!("新历史{i}呃呃呃呃")))
            .await
            .unwrap();
    }
    let _e2: Vec<SessionEvent> = rt
        .run_turn(user, session, "再一条".to_string())
        .collect()
        .await;
    assert_eq!(
        llm.summarize_calls.load(Ordering::SeqCst),
        2,
        "R4：第二回合新 span summarize 一次（累计 2）"
    );
    let details: Vec<(serde_json::Value,)> = kernel_api::fetch_all_both!(
        &*store,
        "SELECT detail FROM progress_events WHERE kind = 'session-evicted'",
        (serde_json::Value,)
    )
    .unwrap();
    assert!(details.len() >= 2, "R4：两回合各发淘汰事件");
    let mut seen = std::collections::HashSet::new();
    for (d,) in details {
        for id in d["message_ids"].as_array().unwrap() {
            assert!(
                seen.insert(id.as_str().unwrap().to_string()),
                "C7/R4：消息 ID 不重复入 outbox"
            );
        }
    }
    drop(store);
    std::fs::remove_dir_all(&db_dir).unwrap();
}

/// R4：summarize 计费幂等——span_hash 命中已摘要 fragment 则复用，不重复调 LLM 计费。
#[tokio::test]
async fn summarize_billing_idempotent_reuse() {
    let (rt, sessions, store, llm, db_dir) = runtime_summarize().await;
    let user = UserId::new();
    let session = SessionId::new();
    kernel::repo::ensure_user(&store, user.0).await.unwrap();
    sessions
        .create(
            user,
            session,
            SessionMeta {
                params: SessionParams {
                    context_budget_tokens: 5,
                    eviction: Eviction::Summarize,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    for i in 0..3 {
        sessions
            .append(user, session, hist_msg(format!("历史{i}啊啊啊啊")))
            .await
            .unwrap();
    }
    // 预置该 span 的摘要 fragment（模拟上回合已摘要落库，evict 曾失败水位未推进）。
    let hist = sessions.history_with_ids(user, session).await.unwrap();
    let span_hash = kernel::summary_span_hash(&hist);
    kernel::store_summary_span(&store, &span_hash, session.0, "预置摘要片段")
        .await
        .unwrap();
    // 本回合淘汰同一 span → lookup 命中 → 不调 LLM summarize（免重复计费）。
    let _e: Vec<SessionEvent> = rt
        .run_turn(user, session, "新消息".to_string())
        .collect()
        .await;
    assert_eq!(
        llm.summarize_calls.load(Ordering::SeqCst),
        0,
        "R4：span_hash 命中复用，不重复计费"
    );
    let m = sessions.meta(user, session).await.unwrap().unwrap();
    assert!(
        m.summary.as_deref().unwrap().contains("预置摘要片段"),
        "R4：复用预置 fragment"
    );
    assert!(m.evicted_up_to > 0, "R4：水位推进");
    drop(store);
    std::fs::remove_dir_all(&db_dir).unwrap();
}

/// 中途失败 summarize LLM（审计 N3）：summarize 调用先发部分文本再报错；教师调用正常。
struct FailSummarizeLlm {
    summarize_calls: AtomicU32,
}

#[async_trait]
impl LlmProvider for FailSummarizeLlm {
    fn id(&self) -> &'static str {
        "fail-summarize-llm"
    }
    fn features(&self) -> LlmFeatures {
        LlmFeatures {
            tool_calling: false,
            reasoning: false,
            streaming: true,
        }
    }
    async fn list_models(&self, _: &ProviderConn) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![])
    }
    async fn chat_stream(
        &self,
        _: &ProviderConn,
        req: ChatRequest,
    ) -> Result<ChatStream, ProviderError> {
        let is_summary = req
            .messages
            .first()
            .map(|m| m.content.contains("压缩"))
            .unwrap_or(false);
        if is_summary {
            self.summarize_calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::pin(stream::iter(vec![
                Ok(ChatChunk::TextDelta("部分摘要".to_string())),
                Err(ProviderError::Network {
                    message: "上游中断".to_string(),
                    retryable: false,
                }),
            ])))
        } else {
            Ok(Box::pin(stream::iter(vec![
                Ok(ChatChunk::TextDelta("回复".to_string())),
                Ok(ChatChunk::Finish(FinishReason::Stop)),
            ])))
        }
    }
}

/// N3：summarize 流中途失败 → 不固化部分 fragment、不复用、按 truncate 回退（水位仍推进、事件仍发、回合正常完成）。
#[tokio::test]
async fn summarize_midstream_failure_no_partial_persist() {
    let db_dir = std::env::temp_dir().join(format!("gw-sumfail-{}", Uuid::new_v4()));
    let store = Arc::new(
        kernel::open_sqlite(&db_dir.join("db").join("t.sqlite"))
            .await
            .unwrap(),
    );
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let sessions = Arc::new(DbSessions::new(store.clone()));
    let llm = Arc::new(FailSummarizeLlm {
        summarize_calls: AtomicU32::new(0),
    });
    let rt = Runtime::new(
        llm.clone(),
        Arc::new(MockTts),
        Arc::new(MockBindings),
        sessions.clone(),
        Arc::new(MemAudio::new()),
        store.clone(),
    );
    let user = UserId::new();
    let session = SessionId::new();
    kernel::repo::ensure_user(&store, user.0).await.unwrap();
    sessions
        .create(
            user,
            session,
            SessionMeta {
                params: SessionParams {
                    context_budget_tokens: 5,
                    eviction: Eviction::Summarize,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    for i in 0..3 {
        sessions
            .append(user, session, hist_msg(format!("历史{i}啊啊啊啊")))
            .await
            .unwrap();
    }
    let events: Vec<SessionEvent> = rt
        .run_turn(user, session, "新消息".to_string())
        .collect()
        .await;
    assert_eq!(
        llm.summarize_calls.load(Ordering::SeqCst),
        1,
        "N3：summarize 调用一次"
    );
    let spans: (i64,) =
        kernel_api::fetch_one_both!(&*store, "SELECT COUNT(*) FROM summary_spans", (i64,)).unwrap();
    assert_eq!(spans.0, 0, "N3：中途失败不固化部分摘要（summary_spans 空）");
    let m = sessions.meta(user, session).await.unwrap().unwrap();
    assert!(m.summary.is_none(), "N3：失败回退 truncate（无滚动摘要）");
    assert!(m.evicted_up_to > 0, "N3：水位仍推进");
    let ev: (i64,) = kernel_api::fetch_one_both!(
        &*store,
        "SELECT COUNT(*) FROM progress_events WHERE kind = 'session-evicted'",
        (i64,)
    )
    .unwrap();
    assert!(ev.0 >= 1, "N3：淘汰事件仍发");
    assert!(
        matches!(
            events.last(),
            Some(SessionEvent::RunEnd {
                finish_reason: FinishReason::Stop,
                ..
            })
        ),
        "N3：失败回退后回合正常完成"
    );
    drop(store);
    std::fs::remove_dir_all(&db_dir).unwrap();
}

/// N6：shutdown 置退出标志后 run_turn 拒绝新回合（不 spawn/登记），发 shutting_down 终态。
#[tokio::test(flavor = "current_thread")]
async fn shutdown_rejects_new_turn() {
    let (rt, sessions) = runtime_flood(std::time::Duration::from_secs(120)).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(user, session, SessionMeta::default())
        .await
        .unwrap();
    rt.shutdown(std::time::Duration::from_millis(50)).await;
    let events: Vec<SessionEvent> = rt.run_turn(user, session, "x".to_string()).collect().await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::Error { code, .. } if code == "shutting_down")),
        "N6：退出期间拒绝新回合"
    );
    assert!(
        matches!(
            events.last(),
            Some(SessionEvent::RunEnd {
                finish_reason: FinishReason::Error,
                ..
            })
        ),
        "N6：拒绝终态 RunEnd"
    );
}

/// N6：满 mpsc 超时后第一条流排空必然关闭（终态或丢弃但流终止，不永久挂起），活跃锁释放。
#[tokio::test(flavor = "current_thread")]
async fn full_mpsc_first_stream_closes_after_timeout() {
    let (rt, sessions) = runtime_flood(std::time::Duration::from_millis(100)).await;
    let user = UserId::new();
    let session = SessionId::new();
    sessions
        .create(user, session, SessionMeta::default())
        .await
        .unwrap();
    let stream1 = rt.run_turn(user, session, "x".to_string());
    // 不消费 → mpsc 填满 + 硬超时（终态 try_send 满队列丢弃，记日志）。
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    // 排空第一条流：必然终止（collect 返回有限事件，不永久挂起）。
    let events: Vec<SessionEvent> = stream1.collect().await;
    assert!(
        events.len() <= 258,
        "N6：第一条流排空关闭（事件有限，实得 {}）",
        events.len()
    );
    // 活跃锁释放：新回合不 busy。
    let events2: Vec<SessionEvent> = rt.run_turn(user, session, "y".to_string()).collect().await;
    assert!(
        matches!(events2.first(), Some(SessionEvent::RunStart { .. })),
        "N6：满 mpsc 超时后活跃锁释放"
    );
}
