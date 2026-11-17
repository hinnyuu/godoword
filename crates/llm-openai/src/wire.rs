//! OpenAI 兼容上游线缆类型（反序列化流块 / 模型目录）。

use serde::Deserialize;
use serde_json::Value;

/// 流式聊天块。
#[derive(Debug, Deserialize)]
pub struct ChatCompletionChunk {
    #[serde(default)]
    pub choices: Vec<ChunkChoice>,
    #[serde(default)]
    pub usage: Option<UsageWire>,
}

/// 块内选项。
#[derive(Debug, Deserialize)]
pub struct ChunkChoice {
    #[serde(default)]
    pub delta: Delta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// 增量内容。
#[derive(Debug, Default, Deserialize)]
pub struct Delta {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// 工具调用增量（按 index 组装）。
#[derive(Debug, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub function: Option<FunctionDelta>,
}

/// 函数增量。
#[derive(Debug, Default, Deserialize)]
pub struct FunctionDelta {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

/// 用量线缆形（含 reasoning_tokens 细节）。
#[derive(Debug, Deserialize)]
pub struct UsageWire {
    #[serde(default)]
    pub prompt_tokens: Option<u32>,
    #[serde(default)]
    pub completion_tokens: Option<u32>,
    #[serde(default)]
    pub total_tokens: Option<u32>,
    #[serde(default)]
    pub completion_tokens_details: Option<CompletionDetails>,
}

/// completion 细节。
#[derive(Debug, Deserialize)]
pub struct CompletionDetails {
    #[serde(default)]
    pub reasoning_tokens: Option<u32>,
}

/// 模型目录响应。
#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
    #[serde(default)]
    pub data: Vec<ModelWire>,
}

/// 模型线缆形。
#[derive(Debug, Deserialize)]
pub struct ModelWire {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// 上游错误体（OpenAI 形 {error:{message,type,code}}）。
#[derive(Debug, Deserialize)]
pub struct ErrorEnvelope {
    #[serde(default)]
    pub error: Option<ErrorBody>,
}

/// 错误体。
#[derive(Debug, Deserialize)]
pub struct ErrorBody {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub code: Option<Value>,
}
