//! godoword 内核实现。回合循环、会话、绑定解析、speak 执行。
//! 适配器与教学模块只依赖 kernel-api；本 crate 实现内核服务（依赖倒置）。
//! M1a：最小回合循环 + 内部接缝内存 impl + 默认教师 persona（无教学模块）。
//! M1b：存储基底（pool 初始化 + 双后端迁移编排）。

mod audio;
mod binding;
mod config;
mod context;
mod crypto;
mod journal;
mod memory;
mod migrate;
mod persona;
pub mod repo;
mod runtime;
mod seam;
mod session;
mod speak;
mod storage;

pub use audio::FsAudio;
pub use binding::DbBindingResolver;
pub use config::{Config, ConfigView, ProviderPreset, RegistrationMode};
pub use context::{
    assemble_truncate, estimate_message_tokens, estimate_messages_tokens, estimate_tokens,
    AssembledContext,
};
pub use crypto::MasterKey;
pub use journal::{
    dispatch_once, emit_progress, mark_processed, pending_events, record_call, CallRecord,
    OutboxEvent, OutboxHandler,
};
pub use memory::{MemAudio, MemSessions};
pub use migrate::{run_migrations, Migration, KERNEL_MIGRATIONS};
pub use persona::DEFAULT_TEACHER_PERSONA;
pub use repo::{Binding, Provider};
pub use runtime::Runtime;
pub use seam::{
    AudioCache, BindingResolver, CachedAudio, OnDisconnect, ResolvedBinding, SessionMeta,
    SessionRepo, StoredMessage,
};
pub use session::DbSessions;
pub use storage::{open_pg, open_sqlite, ping};
