//! 内核回合循环：解析绑定 → 装配提示词 → 流式 LLM → 发 delta → 执行 speak → 回喂 → 循环至 stop/守护上限。
//! M1a 无教学模块：用默认教师 persona，无 plan.teacher_prompt。依赖内部接缝（零返工）。
//! speak 音频迭代内联于循环，live 发 audio 事件（断开即取消，drop 流中止上游）。

use base64::Engine;
use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;
use kernel_api::{
    speak_tool_spec, AudioEvent, Capability, ChatChunk, ChatMessage, ChatRequest, FinishReason,
    LlmProvider, ProviderError, Role, RunId, SessionEvent, SessionId, SessionParams, ToolCall,
    ToolResult, TtsProvider, UserId, SPEAK_TOOL_NAME,
};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::persona::DEFAULT_TEACHER_PERSONA;
use crate::seam::{AudioCache, BindingResolver, CachedAudio, SessionRepo};
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
    tool_cap: u32,
    turn_timeout: Duration,
    active: Arc<Mutex<HashSet<SessionId>>>,
    cancels: Arc<Mutex<HashMap<SessionId, Arc<AtomicBool>>>>,
}

impl Runtime {
    /// 新建运行时（默认守护：工具上限 8、回合超时 120s）。
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tts: Arc<dyn TtsProvider>,
        bindings: Arc<dyn BindingResolver>,
        sessions: Arc<dyn SessionRepo>,
        audio: Arc<dyn AudioCache>,
    ) -> Self {
        Self {
            llm,
            tts,
            bindings,
            sessions,
            audio,
            tool_cap: 8,
            turn_timeout: Duration::from_secs(120),
            active: Arc::new(Mutex::new(HashSet::new())),
            cancels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 应用会话参数（工具上限）。
    pub fn with_params(mut self, params: SessionParams) -> Self {
        self.tool_cap = params.tool_iteration_cap;
        self
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

    /// 跑一个回合，返回 SessionEvent 流（SSE）。断开即取消（drop 流中止上游，释放活跃锁）。
    pub fn run_turn(
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
        let tool_cap = self.tool_cap;
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

            let history = sessions.history(session).await.unwrap_or_default();
            let user_msg = ChatMessage {
                role: "user".to_string(),
                content: user_text,
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
            };
            let _ = sessions.append(session, user_msg.clone()).await;
            let mut convo = vec![system_message(DEFAULT_TEACHER_PERSONA)];
            convo.extend(history);
            convo.push(user_msg);

            let tools = vec![speak_tool_spec()];
            let mut iterations: u32 = 0;

            let finish = loop {
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
                let mut stream = match llm.chat_stream(&binding.conn, req).await {
                    Ok(s) => s,
                    Err(e) => {
                        yield provider_error_event(e);
                        break FinishReason::Error;
                    }
                };

                let mut content = String::new();
                let mut reasoning = String::new();
                let mut pending: Vec<ToolCall> = Vec::new();
                let mut chunk_finish = FinishReason::Stop;
                while let Some(item) = stream.next().await {
                    if cancel_flag.load(Ordering::Relaxed) {
                        chunk_finish = FinishReason::Cancelled;
                        break;
                    }
                    let chunk = match item {
                        Ok(c) => c,
                        Err(e) => {
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
                            yield SessionEvent::Usage {
                                capability: Capability::Llm,
                                model: binding.model.clone(),
                                usage: serde_json::to_value(u).unwrap_or(json!({})),
                            };
                        }
                        ChatChunk::Finish(fr) => chunk_finish = fr,
                    }
                }

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
                let _ = sessions.append(session, assistant_msg).await;

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
                                    match tts.synthesize_stream(&tts_binding.conn, tts_req).await {
                                        Err(e) => {
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
                                            while let Some(item) = astream.next().await {
                                                let ev = match item {
                                                    Ok(e) => e,
                                                    Err(e) => {
                                                        ok = false;
                                                        err = e.to_string();
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
                                                        }).await;
                                                        yield SessionEvent::AudioEnd {
                                                            audio_id,
                                                            url: format!("/api/v1/audio/{audio_id}"),
                                                        };
                                                    }
                                                }
                                            }
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
                        let _ = sessions.append(session, tool_msg).await;
                    }
                    continue;
                }
                break chunk_finish;
            };

            yield SessionEvent::RunEnd { run_id, finish_reason: finish };
        }
    }
}
