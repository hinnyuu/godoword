//! ModuleContext 实现 + ctx 工厂（M2 P1，契约新增落地）。
//! kernel 中介 LLM/TTS（模块永不触 key/上游）：绑定解析带 module_id + 全计量 call_log（module 归因，B4）。
//! 白名单缓存合成（get_or_synthesize）：持久资产区命中直供 / 未命中合成→持久化→计量（UX-A3）。

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures_util::StreamExt;
use kernel_api::{
    AssetAudio, AudioFormat, ChatChunk, ChatRequest, ChatStream, KernelError, LearningPlan,
    ModuleContext, ModuleContextFactory, ModuleId, ModuleStore, PlanProvider, ProgressInput, Role,
    SessionId, TtsRequest, TtsUsage, UserId, WordTimestamp,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::assets::AssetStore;
use crate::journal::{complete_session, emit_progress, record_call, CallRecord};
use crate::seam::BindingResolver;
use crate::speak::DEFAULT_VOICE;

/// ctx 工厂（每模块一实例，module_id 构造期绑定；组合根注入模块）。
pub struct KernelCtxFactory {
    module_id: ModuleId,
    store: Arc<ModuleStore>,
    bindings: Arc<dyn BindingResolver>,
    llm: Arc<dyn kernel_api::LlmProvider>,
    tts: Arc<dyn kernel_api::TtsProvider>,
    assets: Arc<AssetStore>,
    plan: Option<Arc<dyn PlanProvider>>,
}

impl KernelCtxFactory {
    /// 新建工厂（plan 为 None 时 current_plan 恒 None——P1 无 assess，placement 引导）。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        module_id: ModuleId,
        store: Arc<ModuleStore>,
        bindings: Arc<dyn BindingResolver>,
        llm: Arc<dyn kernel_api::LlmProvider>,
        tts: Arc<dyn kernel_api::TtsProvider>,
        assets: Arc<AssetStore>,
        plan: Option<Arc<dyn PlanProvider>>,
    ) -> Self {
        Self {
            module_id,
            store,
            bindings,
            llm,
            tts,
            assets,
            plan,
        }
    }
}

impl ModuleContextFactory for KernelCtxFactory {
    fn for_session(&self, user: UserId, session: SessionId) -> Arc<dyn ModuleContext> {
        Arc::new(KernelCtx {
            module_id: self.module_id.clone(),
            store: self.store.clone(),
            bindings: self.bindings.clone(),
            llm: self.llm.clone(),
            tts: self.tts.clone(),
            assets: self.assets.clone(),
            plan: self.plan.clone(),
            user,
            session: Some(session),
        })
    }

    fn for_user(&self, user: UserId) -> Arc<dyn ModuleContext> {
        Arc::new(KernelCtx {
            module_id: self.module_id.clone(),
            store: self.store.clone(),
            bindings: self.bindings.clone(),
            llm: self.llm.clone(),
            tts: self.tts.clone(),
            assets: self.assets.clone(),
            plan: self.plan.clone(),
            user,
            session: None,
        })
    }
}

/// 模块受控 ctx（user + 可选 session + module 绑定）。
struct KernelCtx {
    module_id: ModuleId,
    store: Arc<ModuleStore>,
    bindings: Arc<dyn BindingResolver>,
    llm: Arc<dyn kernel_api::LlmProvider>,
    tts: Arc<dyn kernel_api::TtsProvider>,
    assets: Arc<AssetStore>,
    plan: Option<Arc<dyn PlanProvider>>,
    user: UserId,
    session: Option<SessionId>,
}

fn provider_err(e: impl std::fmt::Display) -> KernelError {
    KernelError::Internal(e.to_string())
}

/// LLM 计量记录（ctx.llm 用；module 归因）。
#[allow(clippy::too_many_arguments)]
fn llm_record(
    user: UserId,
    session: Option<SessionId>,
    module: &ModuleId,
    model: &str,
    adapter_id: &str,
    start: Instant,
    usage: Option<kernel_api::Usage>,
    status: &str,
    error_code: Option<String>,
) -> CallRecord {
    CallRecord {
        user_id: user.0,
        session_id: session.map(|s| s.0),
        run_id: None,
        module_id: Some(module.0.clone()),
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

/// TTS 计量记录（get_or_synthesize 用；module 归因）。
#[allow(clippy::too_many_arguments)]
fn tts_record(
    user: UserId,
    session: Option<SessionId>,
    module: &ModuleId,
    model: &str,
    adapter_id: &str,
    start: Instant,
    usage: Option<TtsUsage>,
    status: &str,
    error_code: Option<String>,
) -> CallRecord {
    CallRecord {
        user_id: user.0,
        session_id: session.map(|s| s.0),
        run_id: None,
        module_id: Some(module.0.clone()),
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

#[async_trait]
impl ModuleContext for KernelCtx {
    fn user_id(&self) -> UserId {
        self.user
    }

    fn session_id(&self) -> Option<SessionId> {
        self.session
    }

    fn store(&self) -> &ModuleStore {
        &self.store
    }

    async fn emit_progress(&self, input: ProgressInput) -> Result<(), KernelError> {
        emit_progress(
            &self.store,
            self.user.0,
            self.session.map(|s| s.0),
            &self.module_id.0,
            &input,
        )
        .await?;
        Ok(())
    }

    async fn current_plan(&self) -> Result<Option<LearningPlan>, KernelError> {
        match &self.plan {
            Some(p) => p.current_plan(self.user).await.map_err(provider_err),
            None => Ok(None),
        }
    }

    async fn llm(&self, role: Role, mut req: ChatRequest) -> Result<ChatStream, KernelError> {
        let binding = self
            .bindings
            .resolve(
                self.user,
                Some(&self.module_id),
                kernel_api::Capability::Llm,
                role,
            )
            .await?;
        // model 由绑定填（模块不指定上游模型）。
        req.model = binding.model.clone();
        let start = Instant::now();
        let stream = match self.llm.chat_stream(&binding.conn, req).await {
            Ok(s) => s,
            Err(e) => {
                let _ = record_call(
                    &self.store,
                    llm_record(
                        self.user,
                        self.session,
                        &self.module_id,
                        &binding.model,
                        self.llm.id(),
                        start,
                        None,
                        "error",
                        Some(format!("{e:?}")),
                    ),
                )
                .await;
                return Err(provider_err(e));
            }
        };
        // 计量包裹：流终止（含错误/EOF）记 call_log。模块中途弃流不记（与主回合循环同语义，
        // 模块消费者读至终止；P6 一致性检查覆盖 EOF 对称化）。
        let store = self.store.clone();
        let user = self.user;
        let session = self.session;
        let module = self.module_id.clone();
        let model = binding.model.clone();
        let adapter_id = self.llm.id().to_string();
        let wrapped = async_stream::stream! {
            let mut inner = stream;
            let mut usage: Option<kernel_api::Usage> = None;
            let mut err: Option<String> = None;
            while let Some(item) = inner.next().await {
                match &item {
                    Ok(ChatChunk::Usage(u)) => usage = Some(u.clone()),
                    Err(e) => err = Some(format!("{e:?}")),
                    _ => {}
                }
                yield item;
            }
            let status = if err.is_some() { "error" } else { "ok" };
            let _ = record_call(
                &store,
                llm_record(user, session, &module, &model, &adapter_id, start, usage, status, err),
            )
            .await;
        };
        Ok(Box::pin(wrapped))
    }

    async fn complete_session(&self) -> Result<bool, KernelError> {
        let session = self.session.ok_or_else(|| {
            KernelError::Internal("complete_session 需会话级 ctx（for_user 无会话）".to_string())
        })?;
        complete_session(&self.store, self.user.0, session.0).await
    }

    async fn get_or_synthesize(
        &self,
        key: &str,
        req: TtsRequest,
    ) -> Result<AssetAudio, KernelError> {
        let binding = self
            .bindings
            .resolve(
                self.user,
                Some(&self.module_id),
                kernel_api::Capability::Tts,
                Role::Default,
            )
            .await?;
        let voice = if req.voice.trim().is_empty() {
            binding
                .params
                .get("voice")
                .and_then(|v| v.as_str())
                .unwrap_or(DEFAULT_VOICE)
                .to_string()
        } else {
            req.voice.clone()
        };
        let adapter_id = self.tts.id();
        let base_url = binding.conn.base_url.clone();
        let model = binding.model.clone();

        // 命中持久资产 → 直供（零上游零费用）。文件缺失（卷损毁）→ 懒再生。
        if let Some(row) = self
            .assets
            .lookup(
                self.user.0,
                &self.module_id.0,
                key,
                adapter_id,
                &base_url,
                &voice,
                &model,
            )
            .await?
        {
            if let Some(pcm) = self.assets.read_pcm(row.audio_id).await? {
                let words: Vec<WordTimestamp> = row
                    .words
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                let format: AudioFormat = serde_json::from_value(Value::String(row.format.clone()))
                    .unwrap_or(AudioFormat::Pcm);
                return Ok(AssetAudio {
                    audio_id: kernel_api::AudioId(row.audio_id),
                    pcm,
                    words,
                    format,
                    sample_rate: row.sample_rate as u32,
                    cache_hit: true,
                });
            }
        }

        // 未命中 → 合成 → 持久化 → 计量。
        let mut synth_req = req.clone();
        synth_req.voice = voice.clone();
        synth_req.model = model.clone();
        let start = Instant::now();
        let mut stream = match self.tts.synthesize_stream(&binding.conn, synth_req).await {
            Ok(s) => s,
            Err(e) => {
                let _ = record_call(
                    &self.store,
                    tts_record(
                        self.user,
                        self.session,
                        &self.module_id,
                        &model,
                        adapter_id,
                        start,
                        None,
                        "error",
                        Some(format!("{e:?}")),
                    ),
                )
                .await;
                return Err(provider_err(e));
            }
        };
        let mut complete: Vec<u8> = Vec::new();
        let mut words: Vec<WordTimestamp> = Vec::new();
        let mut usage: Option<TtsUsage> = None;
        let mut err: Option<String> = None;
        let mut end_pcm: Option<bytes::Bytes> = None;
        while let Some(item) = stream.next().await {
            match item {
                Ok(kernel_api::AudioEvent::Chunk { pcm, .. }) => complete.extend_from_slice(&pcm),
                Ok(kernel_api::AudioEvent::Words(w)) => words.extend(w),
                Ok(kernel_api::AudioEvent::Usage(u)) => usage = Some(u),
                Ok(kernel_api::AudioEvent::End { complete: end }) => end_pcm = end,
                Ok(_) => {}
                Err(e) => {
                    err = Some(format!("{e:?}"));
                    break;
                }
            }
        }
        let status = if err.is_some() { "error" } else { "ok" };
        let _ = record_call(
            &self.store,
            tts_record(
                self.user,
                self.session,
                &self.module_id,
                &model,
                adapter_id,
                start,
                usage,
                status,
                err.clone(),
            ),
        )
        .await;
        if let Some(e) = err {
            return Err(KernelError::Internal(format!("资产合成失败：{e}")));
        }
        let pcm = end_pcm.unwrap_or_else(|| bytes::Bytes::from(complete));
        if pcm.is_empty() {
            return Err(KernelError::Internal(
                "资产合成空音频（不持久化）".to_string(),
            ));
        }
        let audio_id = Uuid::new_v4();
        // 失败关闭：文件写失败不入登记表（无假成功）。
        self.assets.write_pcm(audio_id, &pcm).await?;
        let format_str = serde_json::to_value(req.format)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "pcm".to_string());
        let words_json = serde_json::to_value(&words).unwrap_or(json!([]));
        let winner = self
            .assets
            .upsert(
                self.user.0,
                &self.module_id.0,
                key,
                adapter_id,
                &base_url,
                &voice,
                &model,
                audio_id,
                &format_str,
                req.sample_rate as i32,
                &words_json,
            )
            .await?;
        // 并发合成败者：清理孤儿文件（登记表胜者为准）。
        if winner != audio_id {
            self.assets.remove_file(audio_id).await;
        }
        Ok(AssetAudio {
            audio_id: kernel_api::AudioId(winner),
            pcm,
            words,
            format: req.format,
            sample_rate: req.sample_rate,
            cache_hit: false,
        })
    }
}
