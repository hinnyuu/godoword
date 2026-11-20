//! 进度事件信封（持久 outbox）。结构化事件记练习对错，提取报告记对话涌现证据。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{EventId, ModuleId, SessionId, UserId};

/// 事件类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    ExerciseOutcome,
    SessionEvicted,
    SessionEnded,
    Custom,
}

/// 练习结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Correct,
    Incorrect,
    Partial,
}

/// 模块发出的进度输入（kernel 包裹信封后持久化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressInput {
    #[serde(default)]
    pub kind: Option<EventKind>,
    #[serde(default)]
    pub skill_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<f32>,
    #[serde(default)]
    pub detail: Value,
}

/// 持久进度事件信封。SessionEvicted 的 span 消息 ID 存于 detail。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub id: EventId,
    pub user_id: UserId,
    pub module_id: ModuleId,
    pub session_id: Option<SessionId>,
    pub ts: DateTime<Utc>,
    pub kind: EventKind,
    pub skill_tags: Vec<String>,
    pub outcome: Option<Outcome>,
    pub item_ref: Option<String>,
    /// 难度（契约形状新增，审计 B1 诚实标注；旧信封 serde default 兼容）。
    #[serde(default)]
    pub difficulty: Option<f32>,
    pub detail: Value,
}

/// 消费侧事件别名（handle_event 参数）。
pub type PersistedEvent = ProgressEvent;
