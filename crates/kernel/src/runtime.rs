//! 内核回合循环：解析绑定 → 装配提示词 → 流式 LLM → 发 delta → 执行 speak → 回喂 → 循环至 stop/守护上限。
//! M1a 无教学模块：用默认教师 persona，无 plan.teacher_prompt。依赖内部接缝（零返工）。
//! speak 音频迭代内联于循环，live 发 audio 事件（断开即取消，drop 流中止上游）。

use base64::Engine;
use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;
use kernel_api::{
    speak_tool_spec, AudioEvent, Capability, ChatChunk, ChatMessage, ChatRequest, FinishReason,
    LlmProvider, ProviderError, Role, RunId, SessionEvent, SessionId, Store, ToolCall, ToolResult,
    TtsProvider, TtsUsage, Usage, UserId, SPEAK_TOOL_NAME,
};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::context::assemble_truncate;
use crate::journal::{emit_progress, record_call, CallRecord};
use crate::persona::DEFAULT_TEACHER_PERSONA;
use crate::seam::{AudioCache, BindingResolver, CachedAudio, SessionRepo, StoredMessage};
use crate::speak::prepare_speak;

/// 单活跃回合锁守卫（stream 被丢弃时自动释放活跃锁与取消标志，支持断开即取消）。
struct ActiveGuard {
    active: Arc<Mutex<HashSet<SessionId>>>,
    cancels: Arc<Mutex<HashMap<SessionId, Arc<AtomicBool>>>>,
    session: SessionId,
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.active.lock().unwrap().remove(&self.session);
        self.cancels.lock().unwrap().remove(&self.session);
    }
}

/// 同步获取单活跃回合锁（MutexGuard 限于此同步 fn，不跨 await，保证 stream Send）。
/// 返回 false 表示已有活跃回合。
fn try_acquire(active: &Mutex<HashSet<SessionId>>, session: SessionId) -> bool {
    let mut g = active.lock().unwrap();
    if g.contains(&session) {
        false
    } else {
        g.insert(session);
        true
    }
}

fn provider_error_code(e: &ProviderError) -> &'static str {
    match e {
        ProviderError::Auth { .. } => "auth",
        ProviderError::Quota { .. } => "quota",
        ProviderError::RateLimit { .. } => "rate_limit",
        ProviderError::ModelUnavailable { .. } => "model_unavailable",
        ProviderError::Network { .. } => "network",
        ProviderError::ContextLength => "context_length",
        ProviderError::ContentFilter => "content_filter",
        ProviderError::Internal { .. } => "internal",
    }
}

fn provider_error_event(e: ProviderError) -> SessionEvent {
    let (code, retryable, action) = match &e {
        ProviderError::Auth { .. } => ("auth", false, Some("settings")),
        ProviderError::Quota { .. } => ("quota", false, Some("settings")),
        ProviderError::RateLimit { .. } => ("rate_limit", true, None),
        ProviderError::ModelUnavailable { .. } => ("model_unavailable", false, Some("settings")),
        ProviderError::Network { retryable, .. } => ("network", *retryable, None),
        ProviderError::ContextLength => ("context_length", false, None),
        ProviderError::ContentFilter => ("content_filter", false, None),
        ProviderError::Internal { .. } => ("internal", false, None),
    };
    SessionEvent::Error {
        code: code.to_string(),
        message: e.to_string(),
        retryable,
        action: action.map(|s| s.to_string()),
    }
}

/// LLM 调用计量记录（usage 可空；latency 自 start）。
#[allow(clippy::too_many_arguments)]
fn llm_call_record(
    user: UserId,
    session: SessionId,
    run_id: RunId,
    model: &str,
    adapter_id: &str,
    start: Instant,
    usage: Option<Usage>,
    status: &str,
    error_code: Option<String>,
) -> CallRecord {
    CallRecord {
        user_id: user.0,
        session_id: Some(session.0),
        run_id: Some(run_id.0),
        capability: "llm".to_string(),
        adapter_id: adapter_id.to_string(),
        model: model.to_string(),
        prompt_tokens: usage
            .as_ref()
            .and_then(|u| u.prompt_tokens)
            .map(|t| t as i32),
        completion_tokens: usage
            .as_ref()
            .and_then(|u| u.completion_tokens)
            .map(|t| t as i32),
        reasoning_tokens: usage
            .as_ref()
            .and_then(|u| u.reasoning_tokens)
            .map(|t| t as i32),
        total_tokens: usage
            .as_ref()
            .and_then(|u| u.total_tokens)
            .map(|t| t as i32),
        characters: None,
        latency_ms: start.elapsed().as_millis() as i32,
        status: status.to_string(),
        error_code,
    }
}

/// TTS 调用计量记录（characters + tokens）。
#[allow(clippy::too_many_arguments)]
fn tts_call_record(
    user: UserId,
    session: SessionId,
    run_id: RunId,
    model: &str,
    adapter_id: &str,
    start: Instant,
    usage: Option<TtsUsage>,
    status: &str,
    error_code: Option<String>,
) -> CallRecord {
    CallRecord {
        user_id: user.0,
        session_id: Some(session.0),
        run_id: Some(run_id.0),
        capability: "tts".to_string(),
        adapter_id: adapter_id.to_string(),
        model: model.to_string(),
        prompt_tokens: usage
            .as_ref()
            .and_then(|u| u.input_tokens)
            .map(|t| t as i32),
        completion_tokens: usage
            .as_ref()
            .and_then(|u| u.output_tokens)
            .map(|t| t as i32),
        reasoning_tokens: None,
        total_tokens: usage
            .as_ref()
            .and_then(|u| u.input_tokens.zip(u.output_tokens))
            .map(|(i, o)| (i + o) as i32),
        characters: usage.as_ref().and_then(|u| u.characters).map(|t| t as i32),
        latency_ms: start.elapsed().as_millis() as i32,
        status: status.to_string(),
        error_code,
    }
}

/// summarize 淘汰：解析 Summarizer role 绑定 → LLM 压缩 evicted span → 摘要消息（记 call_log）。
/// 失败（无绑定/上游错）返回 None → 调用方回退 truncate。
async fn summarize_span(
    llm: &Arc<dyn LlmProvider>,
    bindings: &Arc<dyn BindingResolver>,
    store: &Arc<Store>,
    user: UserId,
    session: SessionId,
    run_id: RunId,
    evicted: &[StoredMessage],
) -> Option<ChatMessage> {
    let binding = bindings
        .resolve(user, None, Capability::Llm, Role::Summarizer)
        .await
        .ok()?;
    let span: String = evicted
        .iter()
        .map(|m| format!("{}: {}", m.message.role, m.message.content))
        .collect::<Vec<_>>()
        .join("\n");
    let req = ChatRequest {
        model: binding.model.clone(),
        messages: vec![
            system_message(
                "把以下对话历史压缩成简洁摘要，保留关键事实与学习进度，供后续对话参考。",
            ),
            ChatMessage {
                role: "user".to_string(),
                content: span,
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ],
        tools: vec![],
        temperature: None,
        max_tokens: None,
        extra: json!({}),
    };
    let start = Instant::now();
    let mut stream = match llm.chat_stream(&binding.conn, req).await {
        Ok(s) => s,
        Err(e) => {
            let _ = record_call(
                store,
                llm_call_record(
                    user,
                    session,
                    run_id,
                    &binding.model,
                    llm.id(),
                    start,
                    None,
                    "error",
                    Some(provider_error_code(&e).to_string()),
                ),
            )
            .await;
            return None;
        }
    };
    let mut summary = String::new();
    let mut usage: Option<Usage> = None;
    let mut err: Option<String> = None;
    while let Some(item) = stream.next().await {
        match item {
            Ok(ChatChunk::TextDelta(t)) => summary.push_str(&t),
            Ok(ChatChunk::Usage(u)) => usage = Some(u),
            Ok(_) => {}
            Err(e) => {
                err = Some(provider_error_code(&e).to_string());
                break;
            }
        }
    }
    let status = if err.is_some() { "error" } else { "ok" };
    let _ = record_call(
        store,
        llm_call_record(
            user,
            session,
            run_id,
            &binding.model,
            llm.id(),
            start,
            usage,
            status,
            err,
        ),
    )
    .await;
    if summary.is_empty() {
        return None;
    }
    Some(system_message(&format!(
        " earlier conversation summary: {summary}"
    )))
}

fn system_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: "system".to_string(),
        content: content.to_string(),
        reasoning: None,
        tool_calls: None,
        tool_call_id: None,
    }
}

/// 内核运行时（回合循环 + 接缝注入）。
pub struct Runtime {
    llm: Arc<dyn LlmProvider>,
    tts: Arc<dyn TtsProvider>,
    bindings: Arc<dyn BindingResolver>,
    sessions: Arc<dyn SessionRepo>,
    audio: Arc<dyn AudioCache>,
    store: Arc<Store>,
    turn_timeout: Duration,
    active: Arc<Mutex<HashSet<SessionId>>>,
    cancels: Arc<Mutex<HashMap<SessionId, Arc<AtomicBool>>>>,
    detach: Arc<tokio::sync::Semaphore>,
}

impl Runtime {
    /// 新建运行时（默认守护：回合超时 120s；工具上限取每会话 meta.params）。store 供 call_log/outbox 计量。
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tts: Arc<dyn TtsProvider>,
        bindings: Arc<dyn BindingResolver>,
        sessions: Arc<dyn SessionRepo>,
        audio: Arc<dyn AudioCache>,
        store: Arc<Store>,
    ) -> Self {
        Self {
            llm,
            tts,
            bindings,
            sessions,
            audio,
            store,
            turn_timeout: Duration::from_secs(120),
            active: Arc::new(Mutex::new(HashSet::new())),
            cancels: Arc::new(Mutex::new(HashMap::new())),
            detach: Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }

    /// 设置 detach 并发上限（on_disconnect=continue 断开后服务端跑完的回合数上限）。
    pub fn with_detach_cap(self, cap: usize) -> Self {
        Self {
            detach: Arc::new(tokio::sync::Semaphore::new(cap)),
            ..self
        }
    }

    /// 取消所有活跃回合（优雅退出用）。
    pub fn cancel_all(&self) {
        for flag in self.cancels.lock().unwrap().values() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// 会话仓储句柄（REST 用）。
    pub fn sessions(&self) -> Arc<dyn SessionRepo> {
        self.sessions.clone()
    }

    /// 音频缓存句柄（回放用）。
    pub fn audio(&self) -> Arc<dyn AudioCache> {
        self.audio.clone()
    }

    /// 显式取消会话活跃回合（置取消标志，循环检查后中止）。
    pub fn cancel(&self, session: SessionId) {
        if let Some(flag) = self.cancels.lock().unwrap().get(&session) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// 跑一个回合，返回 SessionEvent 流（SSE）。执行/投递分离：回合 spawn 为 tokio task 经有界 mpsc
    /// 投递，SSE 订阅 receiver。客户端断开（receiver drop）→ 读 on_disconnect：cancel 置取消标志 /
    /// continue 取 detach permit 后排空跑完（permit 满则降级 cancel）。活跃锁归执行体（task 结束释放）。
    pub fn run_turn(
        &self,
        user: UserId,
        session: SessionId,
        user_text: String,
    ) -> impl Stream<Item = SessionEvent> + Send + 'static {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<SessionEvent>(256);
        let exec = self.run_turn_stream(user, session, user_text);
        let sessions = self.sessions.clone();
        let cancels = self.cancels.clone();
        let detach = self.detach.clone();

        tokio::spawn(async move {
            let mut exec = Box::pin(exec);
            let mut decided = false;
            let mut _detach_permit: Option<tokio::sync::OwnedSemaphorePermit> = None;
            while let Some(ev) = exec.next().await {
                if tx.send(ev).await.is_err() && !decided {
                    // 客户端断开：决策一次（cancel / continue）。
                    decided = true;
                    let on_disc = sessions
                        .meta(user, session)
                        .await
                        .ok()
                        .flatten()
                        .map(|m| m.on_disconnect)
                        .unwrap_or_default();
                    let mut cancel = true;
                    if on_disc == crate::seam::OnDisconnect::Continue {
                        if let Ok(p) = detach.clone().try_acquire_owned() {
                            _detach_permit = Some(p);
                            cancel = false; // continue：排空跑完
                        }
                    }
                    if cancel {
                        if let Some(flag) = cancels.lock().unwrap().get(&session) {
                            flag.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }
            // exec 结束 → ActiveGuard 释放活跃锁；_detach_permit drop 释放 detach 名额。
        });

        async_stream::stream! {
            while let Some(ev) = rx.recv().await {
                yield ev;
            }
        }
    }

    /// 回合执行体（async_stream，yield SessionEvent）。由 run_turn spawn 为 task 经 mpsc 投递。
    fn run_turn_stream(
        &self,
        user: UserId,
        session: SessionId,
        user_text: String,
    ) -> impl Stream<Item = SessionEvent> + Send + 'static {
        let llm = self.llm.clone();
        let tts = self.tts.clone();
        let bindings = self.bindings.clone();
        let sessions = self.sessions.clone();
        let audio = self.audio.clone();
        let store = self.store.clone();
        let turn_timeout = self.turn_timeout;
        let active = self.active.clone();
        let cancels = self.cancels.clone();

        async_stream::stream! {
            let run_id = RunId::new();
            if !try_acquire(&active, session) {
                yield SessionEvent::Error {
                    code: "busy".to_string(),
                    message: "会话已有活跃回合".to_string(),
                    retryable: true,
                    action: None,
                };
                yield SessionEvent::RunEnd { run_id, finish_reason: FinishReason::Error };
                return;
            }
            let cancel_flag = Arc::new(AtomicBool::new(false));
            cancels.lock().unwrap().insert(session, cancel_flag.clone());
            let _guard = ActiveGuard { active: active.clone(), cancels: cancels.clone(), session };
            let deadline = Instant::now() + turn_timeout;

            let binding = match bindings.resolve(user, None, Capability::Llm, Role::Default).await {
                Ok(b) => b,
                Err(e) => {
                    yield SessionEvent::Error {
                        code: "unconfigured".to_string(),
                        message: e.to_string(),
                        retryable: false,
                        action: Some("settings".to_string()),
                    };
                    yield SessionEvent::RunEnd { run_id, finish_reason: FinishReason::Error };
                    return;
                }
            };
            yield SessionEvent::RunStart { run_id, plan_version: None, model: binding.model.clone() };

            // 归属 + 失败关闭（审计 C1/C4）：会话不存在/非本用户/DB 错误 → 中止，不调计费上游。
            let meta = match sessions.meta(user, session).await {
                Ok(Some(m)) => m,
                Ok(None) => {
                    yield SessionEvent::Error {
                        code: "session_not_found".to_string(),
                        message: "会话不存在或无权访问".to_string(),
                        retryable: false,
                        action: None,
                    };
                    yield SessionEvent::RunEnd { run_id, finish_reason: FinishReason::Error };
                    return;
                }
                Err(e) => {
                    yield SessionEvent::Error {
                        code: "storage".to_string(),
                        message: e.to_string(),
                        retryable: false,
                        action: None,
                    };
                    yield SessionEvent::RunEnd { run_id, finish_reason: FinishReason::Error };
                    return;
                }
            };
            let params = meta.params.clone();
            let tool_cap = meta.params.tool_iteration_cap;
            let history = match sessions.history_with_ids(user, session).await {
                Ok(h) => h,
                Err(e) => {
                    yield SessionEvent::Error {
                        code: "storage".to_string(),
                        message: e.to_string(),
                        retryable: false,
                        action: None,
                    };
                    yield SessionEvent::RunEnd { run_id, finish_reason: FinishReason::Error };
                    return;
                }
            };
            let user_msg = ChatMessage {
                role: "user".to_string(),
                content: user_text,
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
            };
            if let Err(e) = sessions.append(user, session, user_msg.clone()).await {
                yield SessionEvent::Error {
                    code: "storage".to_string(),
                    message: e.to_string(),
                    retryable: false,
                    action: None,
                };
                yield SessionEvent::RunEnd { run_id, finish_reason: FinishReason::Error };
                return;
            }
            let system = system_message(DEFAULT_TEACHER_PERSONA);
            let mut assembled = assemble_truncate(
                system,
                history,
                user_msg,
                params.context_budget_tokens,
            );
            // summarize 淘汰（opt-in）：压缩 evicted span 为摘要置历史前端，失败回退 truncate。
            if params.eviction == kernel_api::Eviction::Summarize && !assembled.evicted.is_empty() {
                if let Some(summary_msg) =
                    summarize_span(&llm, &bindings, &store, user, session, run_id, &assembled.evicted)
                        .await
                {
                    assembled.messages.insert(1, summary_msg);
                }
            }
            // 淘汰 span 入 outbox（SessionEvicted；module_id 有则用之、无则哨兵 kernel）。
            if !assembled.evicted.is_empty() {
                let module_id = meta
                    .module_id
                    .as_ref()
                    .map(|m| m.0.clone())
                    .unwrap_or_else(|| "kernel".to_string());
                let ids: Vec<String> =
                    assembled.evicted.iter().map(|m| m.id.to_string()).collect();
                let _ = emit_progress(
                    &store,
                    user.0,
                    Some(session.0),
                    &module_id,
                    "session-evicted",
                    json!([]),
                    json!({ "message_ids": ids }),
                )
                .await;
            }
            let mut convo = assembled.messages;

            let tools = vec![speak_tool_spec()];
            let mut iterations: u32 = 0;

            let finish = 'turn: loop {
                if cancel_flag.load(Ordering::Relaxed) {
                    break FinishReason::Cancelled;
                }
                if Instant::now() > deadline {
                    yield SessionEvent::Error {
                        code: "timeout".to_string(),
                        message: "回合超时".to_string(),
                        retryable: false,
                        action: None,
                    };
                    break FinishReason::Error;
                }
                if iterations >= tool_cap {
                    yield SessionEvent::Error {
                        code: "tool_cap".to_string(),
                        message: "工具迭代达上限".to_string(),
                        retryable: false,
                        action: None,
                    };
                    break FinishReason::Error;
                }
                iterations += 1;

                let req = ChatRequest {
                    model: binding.model.clone(),
                    messages: convo.clone(),
                    tools: tools.clone(),
                    temperature: None,
                    max_tokens: None,
                    extra: json!({}),
                };
                let llm_start = Instant::now();
                let mut stream = match llm.chat_stream(&binding.conn, req).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = record_call(
                            &store,
                            llm_call_record(user, session, run_id, &binding.model, llm.id(), llm_start, None, "error", Some(provider_error_code(&e).to_string())),
                        )
                        .await;
                        yield provider_error_event(e);
                        break FinishReason::Error;
                    }
                };

                let mut content = String::new();
                let mut reasoning = String::new();
                let mut pending: Vec<ToolCall> = Vec::new();
                let mut chunk_finish = FinishReason::Stop;
                let mut llm_usage: Option<Usage> = None;
                let mut llm_err: Option<String> = None;
                while let Some(item) = stream.next().await {
                    if cancel_flag.load(Ordering::Relaxed) {
                        chunk_finish = FinishReason::Cancelled;
                        break;
                    }
                    let chunk = match item {
                        Ok(c) => c,
                        Err(e) => {
                            llm_err = Some(provider_error_code(&e).to_string());
                            yield provider_error_event(e);
                            chunk_finish = FinishReason::Error;
                            break;
                        }
                    };
                    match chunk {
                        ChatChunk::TextDelta(t) => {
                            content.push_str(&t);
                            yield SessionEvent::TextDelta { text: t };
                        }
                        ChatChunk::ReasoningDelta(t) => {
                            reasoning.push_str(&t);
                            yield SessionEvent::ReasoningDelta { text: t };
                        }
                        ChatChunk::ToolCalls(tcs) => {
                            for tc in &tcs {
                                yield SessionEvent::ToolCall {
                                    id: tc.id.clone(),
                                    name: tc.name.clone(),
                                    arguments: tc.arguments.clone(),
                                };
                            }
                            pending = tcs;
                        }
                        ChatChunk::Usage(u) => {
                            llm_usage = Some(u.clone());
                            yield SessionEvent::Usage {
                                capability: Capability::Llm,
                                model: binding.model.clone(),
                                usage: serde_json::to_value(u).unwrap_or(json!({})),
                            };
                        }
                        ChatChunk::Finish(fr) => chunk_finish = fr,
                    }
                }

                let llm_status = if llm_err.is_some() { "error" } else { "ok" };
                let _ = record_call(
                    &store,
                    llm_call_record(user, session, run_id, &binding.model, llm.id(), llm_start, llm_usage, llm_status, llm_err),
                )
                .await;

                let assistant_msg = ChatMessage {
                    role: "assistant".to_string(),
                    content: content.clone(),
                    reasoning: if reasoning.is_empty() { None } else { Some(reasoning.clone()) },
                    tool_calls: if pending.is_empty() {
                        None
                    } else {
                        Some(serde_json::to_value(&pending).unwrap_or(json!([])))
                    },
                    tool_call_id: None,
                };
                convo.push(assistant_msg.clone());
                if let Err(e) = sessions.append(user, session, assistant_msg).await {
                    yield SessionEvent::Error {
                        code: "storage".to_string(),
                        message: e.to_string(),
                        retryable: false,
                        action: None,
                    };
                    break 'turn FinishReason::Error;
                }

                if chunk_finish == FinishReason::ToolCalls && !pending.is_empty() {
                    for tc in pending {
                        let result = if tc.name == SPEAK_TOOL_NAME {
                            // 内联 speak：live 发 audio 事件（prepare → 合成流迭代 → 缓存）。
                            match prepare_speak(bindings.as_ref(), user, &tc).await {
                                Err(e) => ToolResult {
                                    tool_call_id: tc.id.clone(),
                                    ok: false,
                                    content: json!({ "error": e.to_string() }),
                                },
                                Ok((tts_binding, tts_req, audio_id)) => {
                                    let format = tts_req.format;
                                    let sample_rate = tts_req.sample_rate;
                                    let text = tts_req.text.clone();
                                    yield SessionEvent::AudioStart { audio_id, format, sample_rate };
                                    let tts_start = Instant::now();
                                    match tts.synthesize_stream(&tts_binding.conn, tts_req).await {
                                        Err(e) => {
                                            let _ = record_call(
                                                &store,
                                                tts_call_record(user, session, run_id, &tts_binding.model, tts.id(), tts_start, None, "error", Some(provider_error_code(&e).to_string())),
                                            )
                                            .await;
                                            let msg = e.to_string();
                                            yield provider_error_event(e);
                                            ToolResult {
                                                tool_call_id: tc.id.clone(),
                                                ok: false,
                                                content: json!({ "error": msg }),
                                            }
                                        }
                                        Ok(mut astream) => {
                                            let mut complete: Vec<u8> = Vec::new();
                                            let mut words_acc = Vec::new();
                                            let mut ok = true;
                                            let mut err = String::new();
                                            let mut tts_usage: Option<TtsUsage> = None;
                                            let mut tts_err: Option<String> = None;
                                            while let Some(item) = astream.next().await {
                                                let ev = match item {
                                                    Ok(e) => e,
                                                    Err(e) => {
                                                        ok = false;
                                                        err = e.to_string();
                                                        tts_err = Some(provider_error_code(&e).to_string());
                                                        yield provider_error_event(e);
                                                        break;
                                                    }
                                                };
                                                match ev {
                                                    AudioEvent::Start { .. } => {}
                                                    AudioEvent::Chunk { seq, pcm } => {
                                                        complete.extend_from_slice(&pcm);
                                                        yield SessionEvent::AudioChunk {
                                                            audio_id,
                                                            seq,
                                                            data: base64::engine::general_purpose::STANDARD.encode(&pcm),
                                                        };
                                                    }
                                                    AudioEvent::Words(w) => {
                                                        words_acc.extend(w.clone());
                                                        yield SessionEvent::WordTimestamps { audio_id, words: w };
                                                    }
                                                    AudioEvent::Usage(u) => {
                                                        tts_usage = Some(u.clone());
                                                        yield SessionEvent::Usage {
                                                            capability: Capability::Tts,
                                                            model: tts_binding.model.clone(),
                                                            usage: serde_json::to_value(u).unwrap_or(json!({})),
                                                        };
                                                    }
                                                    AudioEvent::End { complete: end } => {
                                                        let pcm = end.unwrap_or_else(|| Bytes::from(complete.clone()));
                                                        let _ = audio.put(audio_id, CachedAudio {
                                                            pcm,
                                                            words: words_acc.clone(),
                                                            format,
                                                            sample_rate,
                                                            owner: user,
                                                        }).await;
                                                        yield SessionEvent::AudioEnd {
                                                            audio_id,
                                                            url: format!("/api/v1/audio/{audio_id}"),
                                                        };
                                                    }
                                                }
                                            }
                                            let tts_status = if tts_err.is_some() { "error" } else { "ok" };
                                            let _ = record_call(
                                                &store,
                                                tts_call_record(user, session, run_id, &tts_binding.model, tts.id(), tts_start, tts_usage, tts_status, tts_err),
                                            )
                                            .await;
                                            ToolResult {
                                                tool_call_id: tc.id.clone(),
                                                ok,
                                                content: if ok {
                                                    json!({ "audio_id": audio_id.to_string(), "text": text })
                                                } else {
                                                    json!({ "error": err })
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            ToolResult {
                                tool_call_id: tc.id.clone(),
                                ok: false,
                                content: json!({ "error": format!("未知工具：{}", tc.name) }),
                            }
                        };
                        yield SessionEvent::ToolResult { id: tc.id.clone(), ok: result.ok };
                        let tool_msg = ChatMessage {
                            role: "tool".to_string(),
                            content: serde_json::to_string(&result.content).unwrap_or_default(),
                            reasoning: None,
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                        };
                        convo.push(tool_msg.clone());
                        if let Err(e) = sessions.append(user, session, tool_msg).await {
                            yield SessionEvent::Error {
                                code: "storage".to_string(),
                                message: e.to_string(),
                                retryable: false,
                                action: None,
                            };
                            break 'turn FinishReason::Error;
                        }
                    }
                    continue;
                }
                break chunk_finish;
            };

            yield SessionEvent::RunEnd { run_id, finish_reason: finish };
        }
    }
}
