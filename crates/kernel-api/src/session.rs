//! 会话事件协议（后端 → 前端 SSE）。回合内全序。见 architecture.md。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::llm::FinishReason;
use crate::types::{AudioFormat, AudioId, Capability, RunId, WordTimestamp};

/// SSE 事件：`event:` 为标签，`data:` 为载荷。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "kebab-case")]
pub enum SessionEvent {
    RunStart {
        run_id: RunId,
        plan_version: Option<u32>,
        model: String,
    },
    TextDelta {
        text: String,
    },
    /// 思考内容，UI 默认折叠可展开。
    ReasoningDelta {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Value,
    },
    ToolResult {
        id: String,
        ok: bool,
    },
    AudioStart {
        audio_id: AudioId,
        format: AudioFormat,
        sample_rate: u32,
    },
    /// 裸 PCM base64，seq 单调。
    AudioChunk {
        audio_id: AudioId,
        seq: u32,
        data: String,
    },
    AudioEnd {
        audio_id: AudioId,
        url: String,
    },
    /// 增量追加语义。
    WordTimestamps {
        audio_id: AudioId,
        words: Vec<WordTimestamp>,
    },
    Usage {
        capability: Capability,
        model: String,
        usage: Value,
    },
    Error {
        code: String,
        message: String,
        retryable: bool,
        action: Option<String>,
    },
    RunEnd {
        run_id: RunId,
        finish_reason: FinishReason,
    },
}
