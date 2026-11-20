//! godoword 模块契约 crate。
//!
//! 本 crate 只含 trait 与共享类型，无实现。适配器与教学模块只依赖本 crate，
//! 互不 import；kernel 实现 [`ModuleContext`] 等服务 trait（依赖倒置）。
//! 设计依据见 `docs/project/architecture.md` 与 `history/` 相关决策。

pub mod auth;
pub mod error;
pub mod llm;
pub mod plan;
pub mod progress;
pub mod session;
pub mod speak;
pub mod store;
pub mod teaching;
pub mod tts;
pub mod types;

pub use auth::{AuthKind, AuthOutcome, AuthProvider, AuthRequest, OidcIdentity};
pub use error::{AuthError, KernelError, ModuleError, PlanError, ProviderError};
pub use llm::{ChatChunk, ChatStream, FinishReason, LlmFeatures, LlmProvider};
pub use plan::{LearningPlan, PlanProvider, PlanStructured};
pub use progress::{EventKind, Outcome, PersistedEvent, ProgressEvent, ProgressInput};
pub use session::SessionEvent;
pub use speak::{speak_tool_spec, SpeakArgs, SPEAK_TOOL_NAME};
pub use store::Store;
pub use teaching::{ModuleContext, ModuleContextFactory, ModuleStore, TeachingModule};
pub use tts::{AudioEvent, AudioStream, TtsProvider};
pub use types::{
    AssetAudio, AudioFormat, AudioId, Capability, ChatMessage, ChatRequest, EventId, Eviction,
    FrontendManifest, ModelInfo, ModuleId, ModuleKind, ModuleManifest, ProviderConn, ProviderId,
    Provides, Requires, Role, RouteSpec, RunId, SessionId, SessionParams, ToolCall, ToolResult,
    ToolSpec, TtsRequest, TtsUsage, Usage, UserId, VoiceInfo, WordTimestamp,
};
