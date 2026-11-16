//! LLM 适配器契约。

use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;
use serde::{Deserialize, Serialize};

use crate::error::ProviderError;
use crate::types::{ChatRequest, ModelInfo, ProviderConn, ToolCall, Usage};

/// 适配器能力声明。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LlmFeatures {
    pub tool_calling: bool,
    pub reasoning: bool,
    pub streaming: bool,
}

/// 回合结束原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Cancelled,
    Error,
}

/// 归一化 LLM 流块。qwen 的 reasoning_content 与 tool_calls 增量由适配器组装。
#[derive(Debug, Clone)]
pub enum ChatChunk {
    TextDelta(String),
    ReasoningDelta(String),
    /// 适配器组装完整 tool_calls 后一次性发出。
    ToolCalls(Vec<ToolCall>),
    Usage(Usage),
    Finish(FinishReason),
}

/// 归一化 LLM 流。
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>;

/// LLM 适配器（llm-openai / 未来 llm-anthropic）。
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 适配器标识，如 "llm-openai"。
    fn id(&self) -> &'static str;
    /// 能力声明。
    fn features(&self) -> LlmFeatures;
    /// 拉取可用模型列表（设置 UI 用）。
    async fn list_models(&self, conn: &ProviderConn) -> Result<Vec<ModelInfo>, ProviderError>;
    /// 发起流式聊天。
    async fn chat_stream(
        &self,
        conn: &ProviderConn,
        req: ChatRequest,
    ) -> Result<ChatStream, ProviderError>;
}
