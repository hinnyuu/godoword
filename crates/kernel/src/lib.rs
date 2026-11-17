//! godoword 内核实现。回合循环、会话、绑定解析、speak 执行。
//! 适配器与教学模块只依赖 kernel-api；本 crate 实现内核服务（依赖倒置）。
//! M1a：最小回合循环 + 内部接缝内存 impl + 默认教师 persona（无教学模块）。
//! M1b：存储基底（pool 初始化 + 双后端迁移编排）。

mod memory;
mod migrate;
mod persona;
mod runtime;
mod seam;
mod speak;
mod storage;

pub use memory::{MemAudio, MemSessions};
pub use migrate::{run_migrations, Migration, KERNEL_MIGRATIONS};
pub use persona::DEFAULT_TEACHER_PERSONA;
pub use runtime::Runtime;
pub use seam::{AudioCache, BindingResolver, CachedAudio, ResolvedBinding, SessionRepo};
pub use storage::{open_pg, open_sqlite};
