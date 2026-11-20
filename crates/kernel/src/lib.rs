//! godoword 内核实现。回合循环、会话、绑定解析、speak 执行。
//! 适配器与教学模块只依赖 kernel-api；本 crate 实现内核服务（依赖倒置）。
//! M1a：最小回合循环 + 内部接缝 + 默认教师 persona。M1b：存储基底（pool + 双后端迁移）。
//! M1c：会话/音频持久化 + 上下文淘汰 + outbox/call_log + continue。M1c-hardening：对象级授权 + 失败关闭 + 硬边界 + 水位制。

mod audio;
mod binding;
mod config;
mod context;
mod crypto;
mod folders;
mod journal;
mod memory;
mod migrate;
mod path;
mod persona;
mod preferences;
pub mod repo;
mod runtime;
mod seam;
mod session;
mod session_meta;
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
pub use folders::{
    create_folder, delete_folder, get_folder, list_folders, move_session, rename_folder, Folder,
    FolderError,
};
pub use journal::{
    call_log_count, call_log_rows, dispatch_once, emit_progress, end_session, evict_session,
    lookup_summary_span, mark_processed, pending_events, record_call, store_summary_span,
    summary_span_hash, CallRecord, OutboxEvent, OutboxHandler,
};
pub use memory::{MemAudio, MemSessions};
pub use migrate::{run_migrations, Migration, KERNEL_MIGRATIONS};
pub use path::{
    depth, is_within, normalize_path, parent, rewrite_prefix, PathError, DEFAULT_NAMESPACE,
    MAX_DEPTH, MAX_PATH_CHARS, MAX_SEGMENT_CHARS,
};
pub use persona::DEFAULT_TEACHER_PERSONA;
pub use preferences::{get_preferences, put_preferences, Preferences};
pub use repo::{Binding, Provider};
pub use runtime::Runtime;
pub use seam::{
    AudioCache, BindingResolver, CachedAudio, OnDisconnect, ResolvedBinding, SessionMeta,
    SessionRepo, StoredMessage,
};
pub use session::DbSessions;
pub use session_meta::{get_session_path, list_sessions_meta, SessionListItem};
pub use storage::{checkpoint, open_memory_sqlite, open_pg, open_sqlite, ping};
