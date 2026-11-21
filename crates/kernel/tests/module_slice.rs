//! M2 P1 模块切片集成测试：回合接线（persona/工具分发/绑定 module_id/方案钉住）+
//! ModuleContext 实现（llm 计量归因/complete_session/白名单缓存合成/进度事件）。
//! mock 适配器不触网；存储走内存 SQLite（ctx/资产测试）+ DbSessions。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::{self, StreamExt};
use kernel::{
    run_migrations, AssetStore, DbSessions, KernelCtxFactory, MemAudio, ModuleRegistry,
    ResolvedBinding, Runtime, SessionMeta, SessionRepo, KERNEL_MIGRATIONS,
};
use kernel_api::{
    AudioEvent, AudioFormat, AudioStream, ChatChunk, ChatRequest, ChatStream, FinishReason,
    KernelError, LearningPlan, LlmFeatures, LlmProvider, ModelInfo, ModuleContext,
    ModuleContextFactory, ModuleError, ModuleId, ModuleManifest, PlanError, PlanProvider,
    PlanStructured, ProgressInput, ProviderConn, ProviderError, Role, SessionEvent, SessionId,
    Store, ToolCall, ToolResult, ToolSpec, TtsProvider, TtsRequest, TtsUsage, Usage, UserId,
    VoiceInfo, WordTimestamp,
};
use secrecy::SecretString;
use serde_json::json;
use uuid::Uuid;

const MOCK_YAML: &str =
    "id: teach-mock\nname: mock\nversion: 0.0.0\nkind: teaching\ndescription: mock 教学模块\n";
const MOCK_PERSONA: &str = "MODULE-PERSONA-标记";

/// 记录请求的 mock LLM：首轮发 mock_tool 工具调用，次轮 stop（tool_dispatch 模式）；
/// simple 模式直接 stop + usage。
struct CapturingLlm {
    requests: Arc<Mutex<Vec<ChatRequest>>>,
    tool_mode: bool,
}

#[async_trait]
impl LlmProvider for CapturingLlm {
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
        req: ChatRequest,
    ) -> Result<ChatStream, ProviderError> {
        let n = {
            let mut r = self.requests.lock().unwrap();
            r.push(req);
            r.len()
        };
        let chunks = if self.tool_mode && n == 1 {
            vec![
                Ok(ChatChunk::ToolCalls(vec![ToolCall {
                    id: "c1".to_string(),
                    name: "mock_tool".to_string(),
                    arguments: json!({ "x": 1 }),
                }])),
                Ok(ChatChunk::Finish(FinishReason::ToolCalls)),
            ]
        } else {
            vec![
                Ok(ChatChunk::TextDelta("好".to_string())),
                Ok(ChatChunk::Usage(Usage {
                    prompt_tokens: Some(7),
                    completion_tokens: Some(3),
                    reasoning_tokens: None,
                    total_tokens: Some(10),
                })),
                Ok(ChatChunk::Finish(FinishReason::Stop)),
            ]
        };
        Ok(Box::pin(stream::iter(chunks)))
    }
}

/// 记录 module 参数的 mock 绑定解析器。
struct RecordingBindings {
    modules: Arc<Mutex<Vec<Option<String>>>>,
}

#[async_trait]
impl kernel::BindingResolver for RecordingBindings {
    async fn resolve(
        &self,
        _: UserId,
        module: Option<&ModuleId>,
        _: kernel_api::Capability,
        _: Role,
    ) -> Result<ResolvedBinding, KernelError> {
        self.modules
            .lock()
            .unwrap()
            .push(module.map(|m| m.0.clone()));
        Ok(ResolvedBinding {
            conn: ProviderConn {
                base_url: "https://mock.example".to_string(),
                api_key: SecretString::from("k".to_string()),
                extra: json!({}),
            },
            model: "mock-model".to_string(),
            params: json!({ "voice": "mock-voice" }),
        })
    }
}

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
        req: TtsRequest,
    ) -> Result<AudioStream, ProviderError> {
        let pcm = Bytes::from(vec![7u8; 480]);
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
                text: req.text.clone(),
                begin_index: 0,
                end_index: 1,
                begin_time: 0,
                end_time: 100,
            }])),
            Ok(AudioEvent::Usage(TtsUsage {
                characters: Some(1),
                input_tokens: None,
                output_tokens: None,
            })),
            Ok(AudioEvent::End {
                complete: Some(pcm),
            }),
        ];
        Ok(Box::pin(stream::iter(evs)))
    }
}

/// mock 教学模块（persona + 专属工具）。
struct MockModule {
    manifest: ModuleManifest,
    tool_calls: Arc<AtomicUsize>,
}

impl MockModule {
    fn new(tool_calls: Arc<AtomicUsize>) -> Self {
        Self {
            manifest: kernel::parse_manifest(MOCK_YAML).unwrap(),
            tool_calls,
        }
    }
}

#[async_trait]
impl kernel_api::TeachingModule for MockModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn persona(&self) -> String {
        MOCK_PERSONA.to_string()
    }
    fn tools(&self) -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "mock_tool".to_string(),
            description: "mock".to_string(),
            parameters: json!({ "type": "object" }),
        }]
    }
    async fn execute_tool(
        &self,
        ctx: &dyn ModuleContext,
        call: ToolCall,
    ) -> Result<ToolResult, ModuleError> {
        self.tool_calls.fetch_add(1, Ordering::SeqCst);
        // ctx 携带会话与用户（工厂注入正确性）。
        assert!(ctx.session_id().is_some(), "工具分发 ctx 须会话级");
        Ok(ToolResult {
            tool_call_id: call.id,
            ok: true,
            content: json!({ "echo": call.name }),
        })
    }
}

struct MockPlan;

#[async_trait]
impl PlanProvider for MockPlan {
    async fn current_plan(&self, _: UserId) -> Result<Option<LearningPlan>, PlanError> {
        Ok(Some(LearningPlan {
            version: 3,
            structured: PlanStructured {
                level: "N5".to_string(),
                focus: vec![],
                weaknesses: vec![],
                extra: json!({}),
            },
            teacher_prompt: "PLAN-PROMPT-标记".to_string(),
            updated_at: chrono::Utc::now(),
        }))
    }
}

/// 构建测试运行时（注册表 + 记录型 mock）。
async fn setup(
    tool_mode: bool,
) -> (
    Arc<Runtime>,
    Arc<Store>,
    Arc<Mutex<Vec<ChatRequest>>>,
    Arc<Mutex<Vec<Option<String>>>>,
    Arc<AtomicUsize>,
    UserId,
) {
    let store = Arc::new(kernel::open_memory_sqlite().await.unwrap());
    run_migrations(&store, KERNEL_MIGRATIONS, None)
        .await
        .unwrap();
    let user = UserId::new();
    kernel::repo::ensure_user(&store, user.0).await.unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let modules_seen = Arc::new(Mutex::new(Vec::new()));
    let tool_calls = Arc::new(AtomicUsize::new(0));
    let llm: Arc<dyn LlmProvider> = Arc::new(CapturingLlm {
        requests: requests.clone(),
        tool_mode,
    });
    let tts: Arc<dyn TtsProvider> = Arc::new(MockTts);
    let bindings = Arc::new(RecordingBindings {
        modules: modules_seen.clone(),
    });
    let assets_dir = std::env::temp_dir().join(format!("gw-p1-slice-{}", Uuid::new_v4()));
    let assets = Arc::new(AssetStore::new(assets_dir, store.clone()));
    let factory = Arc::new(KernelCtxFactory::new(
        ModuleId("teach-mock".to_string()),
        store.clone(),
        bindings.clone(),
        llm.clone(),
        tts.clone(),
        assets,
        None,
    ));
    let mut registry = ModuleRegistry::new();
    registry
        .register_yaml(
            MOCK_YAML,
            Arc::new(MockModule::new(tool_calls.clone())),
            factory,
        )
        .unwrap();
    let sessions: Arc<dyn SessionRepo> = Arc::new(DbSessions::new(store.clone()));
    let rt = Arc::new(
        Runtime::new(
            llm,
            tts,
            bindings,
            sessions,
            Arc::new(MemAudio::new()),
            store.clone(),
        )
        .with_modules(Arc::new(registry)),
    );
    (rt, store, requests, modules_seen, tool_calls, user)
}

async fn create_module_session(store: &Arc<Store>, user: UserId, module: &str) -> SessionId {
    let sid = SessionId::new();
    let sessions = DbSessions::new(store.clone());
    sessions
        .create(
            user,
            sid,
            SessionMeta {
                module_id: Some(ModuleId(module.to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    sid
}

/// 回合接线：模块 persona 入系统提示词 + 绑定带 module_id + 模块工具分发。
#[tokio::test]
async fn turn_wiring_persona_binding_tools() {
    let (rt, store, requests, modules_seen, tool_calls, user) = setup(true).await;
    let sid = create_module_session(&store, user, "teach-mock").await;
    let events: Vec<SessionEvent> = rt.run_turn(user, sid, "你好".to_string()).collect().await;
    // 绑定解析带 module_id（Llm Default + Tts Default 皆然）。
    {
        let seen = modules_seen.lock().unwrap();
        assert!(
            seen.iter().all(|m| m.as_deref() == Some("teach-mock")),
            "绑定解析须带模块：{seen:?}"
        );
    }
    // 系统提示词含模块 persona。
    {
        let reqs = requests.lock().unwrap();
        assert!(
            reqs[0].messages[0].content.contains(MOCK_PERSONA),
            "persona 入系统提示词"
        );
        assert!(
            reqs[0].tools.iter().any(|t| t.name == "mock_tool"),
            "模块工具入 tools"
        );
        assert!(
            reqs[0].tools.iter().any(|t| t.name == "speak"),
            "speak kernel 统一提供"
        );
    }
    // 模块工具分发执行。
    assert_eq!(
        tool_calls.load(Ordering::SeqCst),
        1,
        "execute_tool 调用一次"
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::ToolCall { name, .. } if name == "mock_tool")));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::ToolResult { ok: true, .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        SessionEvent::RunEnd {
            finish_reason: FinishReason::Stop,
            ..
        }
    )));
    // 计量模块归因（主回合 LLM）。
    let rows: Vec<(Option<String>,)> = kernel_api::fetch_all_both!(
        &*store,
        "SELECT module_id FROM call_log WHERE user_id = $1",
        (Option<String>,),
        user.0
    )
    .unwrap();
    assert!(!rows.is_empty());
    assert!(
        rows.iter().all(|(m,)| m.as_deref() == Some("teach-mock")),
        "call_log 模块归因"
    );
}

/// 方案钉住：teacher_prompt 入系统提示词 + RunStart.plan_version + sessions.plan_version 落库。
#[tokio::test]
async fn plan_pinning() {
    let (rt, store, requests, _modules, _tools, user) = setup(false).await;
    let rt = Arc::new((*rt).clone().with_plan_provider(Arc::new(MockPlan)));
    let sid = create_module_session(&store, user, "teach-mock").await;
    let events: Vec<SessionEvent> = rt.run_turn(user, sid, "你好".to_string()).collect().await;
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::RunStart {
                plan_version: Some(3),
                ..
            }
        )),
        "RunStart 带方案版本"
    );
    {
        let reqs = requests.lock().unwrap();
        assert!(
            reqs[0].messages[0].content.contains("PLAN-PROMPT-标记"),
            "teacher_prompt 钉住"
        );
    }
    let meta = DbSessions::new(store.clone())
        .meta(user, sid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(meta.plan_version, Some(3), "sessions.plan_version 落库");
}

/// 未注册模块 fail-closed（不静默回退默认教师）。
#[tokio::test]
async fn unknown_module_fail_closed() {
    let (rt, store, _reqs, _modules, _tools, user) = setup(false).await;
    let sid = create_module_session(&store, user, "absent-module").await;
    let events: Vec<SessionEvent> = rt.run_turn(user, sid, "你好".to_string()).collect().await;
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::Error { code, .. } if code == "module_not_found")));
}

/// ctx 工厂：for_user 无会话（B3）+ complete_session fail-closed + llm 计量归因。
#[tokio::test]
async fn ctx_user_scope_and_llm_metering() {
    let (_rt, store, _reqs, _modules, _tools, user) = setup(false).await;
    let llm: Arc<dyn LlmProvider> = Arc::new(CapturingLlm {
        requests: Arc::new(Mutex::new(Vec::new())),
        tool_mode: false,
    });
    let tts: Arc<dyn TtsProvider> = Arc::new(MockTts);
    let bindings = Arc::new(RecordingBindings {
        modules: Arc::new(Mutex::new(Vec::new())),
    });
    let assets_dir = std::env::temp_dir().join(format!("gw-p1-ctx-{}", Uuid::new_v4()));
    let factory = KernelCtxFactory::new(
        ModuleId("teach-mock".to_string()),
        store.clone(),
        bindings,
        llm,
        tts,
        Arc::new(AssetStore::new(assets_dir, store.clone())),
        None,
    );
    let ctx = factory.for_user(user);
    assert_eq!(ctx.session_id(), None, "B3：for_user 无会话");
    assert!(
        ctx.complete_session().await.is_err(),
        "无会话 complete fail-closed"
    );
    assert!(
        ctx.current_plan().await.unwrap().is_none(),
        "无 PlanProvider → None"
    );
    // llm 中介：model 由绑定填 + 计量 module 归因。
    let req = ChatRequest {
        model: "module-不应生效".to_string(),
        messages: vec![],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        extra: json!({}),
    };
    let stream = ctx.llm(Role::Extractor, req).await.unwrap();
    let chunks: Vec<_> = stream.collect().await;
    assert!(chunks.iter().all(|c| c.is_ok()));
    let rows: Vec<(Option<String>, String, Option<i32>)> = kernel_api::fetch_all_both!(
        &*store,
        "SELECT module_id, capability, total_tokens FROM call_log WHERE user_id = $1",
        (Option<String>, String, Option<i32>),
        user.0
    )
    .unwrap();
    assert_eq!(rows.len(), 1, "ctx.llm 计量一条");
    assert_eq!(rows[0].0.as_deref(), Some("teach-mock"), "module 归因");
    assert_eq!(rows[0].1, "llm");
    assert_eq!(rows[0].2, Some(10), "usage 计量");
    // 会话级 ctx：complete_session 幂等 + 事件。
    let sid = SessionId::new();
    DbSessions::new(store.clone())
        .create(
            user,
            sid,
            SessionMeta {
                module_id: Some(ModuleId("teach-mock".to_string())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let sctx = factory.for_session(user, sid);
    assert_eq!(sctx.session_id(), Some(sid));
    assert!(sctx.complete_session().await.unwrap(), "完成 true");
    assert!(
        !sctx.complete_session().await.unwrap(),
        "重复完成 false（幂等）"
    );
    // emit_progress 经 ctx（module + session 归因 + 结构化列）。
    sctx.emit_progress(ProgressInput {
        kind: Some(kernel_api::EventKind::ExerciseOutcome),
        skill_tags: vec!["t".to_string()],
        outcome: Some(kernel_api::Outcome::Incorrect),
        item_ref: Some("い".to_string()),
        difficulty: Some(0.5),
        detail: json!({}),
    })
    .await
    .unwrap();
    let ev: (String, Option<String>, Option<String>, Option<f32>) = kernel_api::fetch_one_both!(
        &*store,
        "SELECT module_id, outcome, item_ref, difficulty FROM progress_events WHERE user_id = $1 AND kind = 'exercise-outcome'",
        (String, Option<String>, Option<String>, Option<f32>),
        user.0
    )
    .unwrap();
    assert_eq!(ev.0, "teach-mock");
    assert_eq!(ev.1.as_deref(), Some("incorrect"));
    assert_eq!(ev.2.as_deref(), Some("い"));
    assert_eq!(ev.3, Some(0.5));
}

/// 白名单缓存合成：miss 合成→持久化→计量 / hit 直供零上游 / 换音色懒再生 / 跨用户隔离。
#[tokio::test]
async fn cached_synthesis_hit_miss_scope() {
    let (_rt, store, _reqs, _modules, _tools, user) = setup(false).await;
    let llm: Arc<dyn LlmProvider> = Arc::new(CapturingLlm {
        requests: Arc::new(Mutex::new(Vec::new())),
        tool_mode: false,
    });
    let tts: Arc<dyn TtsProvider> = Arc::new(MockTts);
    let bindings = Arc::new(RecordingBindings {
        modules: Arc::new(Mutex::new(Vec::new())),
    });
    let assets_dir = std::env::temp_dir().join(format!("gw-p1-asset-{}", Uuid::new_v4()));
    let assets = Arc::new(AssetStore::new(assets_dir.clone(), store.clone()));
    let factory = KernelCtxFactory::new(
        ModuleId("teach-mock".to_string()),
        store.clone(),
        bindings,
        llm,
        tts,
        assets.clone(),
        None,
    );
    let ctx = factory.for_user(user);
    let req = |voice: &str| TtsRequest {
        model: "m".to_string(),
        text: "あ".to_string(),
        voice: voice.to_string(),
        format: AudioFormat::Pcm,
        sample_rate: 24000,
        rate: None,
        pitch: None,
        instruction: None,
        language_hints: vec![],
        word_timestamps: true,
        extra: json!({}),
    };
    // miss → 合成 + 持久化 + 计量。
    let a1 = ctx.get_or_synthesize("kana:あ", req("v1")).await.unwrap();
    assert!(!a1.cache_hit, "首次 miss");
    assert_eq!(a1.pcm.len(), 480);
    assert!(
        assets_dir.join(format!("{}.pcm", a1.audio_id.0)).exists(),
        "资产文件持久化"
    );
    let tts_calls = |store: Arc<Store>, user: UserId| async move {
        let r: (i64,) = kernel_api::fetch_one_both!(
            &*store,
            "SELECT COUNT(*) FROM call_log WHERE user_id = $1 AND capability = 'tts'",
            (i64,),
            user.0
        )
        .unwrap();
        r.0
    };
    assert_eq!(tts_calls(store.clone(), user).await, 1, "miss 计量一条");
    // hit → 直供零上游。
    let a2 = ctx.get_or_synthesize("kana:あ", req("v1")).await.unwrap();
    assert!(a2.cache_hit, "二次 hit");
    assert_eq!(a2.audio_id, a1.audio_id, "hit 同资产");
    assert_eq!(a2.pcm, a1.pcm);
    assert_eq!(tts_calls(store.clone(), user).await, 1, "hit 零上游零计量");
    // 换音色 → 作用域不匹配 → 懒再生新版（旧版保留）。
    let a3 = ctx.get_or_synthesize("kana:あ", req("v2")).await.unwrap();
    assert!(!a3.cache_hit, "换音色 miss");
    assert_ne!(a3.audio_id, a1.audio_id, "新版新资产");
    assert_eq!(tts_calls(store.clone(), user).await, 2);
    assert!(
        assets_dir.join(format!("{}.pcm", a1.audio_id.0)).exists(),
        "旧版保留"
    );
    // 跨用户隔离（越权负例）：另一用户同 key 不命中。
    let other = UserId::new();
    kernel::repo::ensure_user(&store, other.0).await.unwrap();
    let ctx2 = factory.for_user(other);
    let a4 = ctx2.get_or_synthesize("kana:あ", req("v1")).await.unwrap();
    assert!(!a4.cache_hit, "跨用户不命中");
    assert_ne!(a4.audio_id, a1.audio_id);
    std::fs::remove_dir_all(&assets_dir).ok();
}
