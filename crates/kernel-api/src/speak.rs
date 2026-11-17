//! speak 工具契约（kernel 统一提供，教学模块不实现）。
//!
//! speak 是众多教学模块共用的 LLM 工具面，故规格落契约层：演进触发契约级全量审查防漂移，
//! 与 [`crate::types::TtsRequest`] 代码锚定防分裂。LLM 可控子集为 [`SpeakArgs`]；
//! model/format/sample_rate/word_timestamps 由 kernel 从绑定/配置填，不暴露给 LLM。

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::types::ToolSpec;

/// speak 工具名（kernel 运行时注入 ChatRequest.tools，教学模块不实现）。
pub const SPEAK_TOOL_NAME: &str = "speak";

/// speak 工具参数（LLM 可控子集）。kernel 反序列化 ToolCall.arguments 为本类型，
/// 合并绑定配置后转 [`crate::types::TtsRequest`] 经 TtsProvider 执行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakArgs {
    /// 要朗读的文本。
    pub text: String,
    /// 音色 id（可选，缺省用绑定默认音色）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    /// 语速 0.5–2.0（可选，慢速教学用 <1）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f32>,
    /// 语调（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f32>,
    /// 自然语言风格指令（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    /// 语言提示（如 `["ja"]`，可选）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub language_hints: Vec<String>,
}

/// speak 工具规格（JSON Schema 参数）。与 [`SpeakArgs`] 由黄金测试锚定一致。
pub fn speak_tool_spec() -> ToolSpec {
    ToolSpec {
        name: SPEAK_TOOL_NAME.to_string(),
        description: "朗读文本（TTS 语音合成）。需要发音、领读或听力示范时调用。".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "要朗读的文本" },
                "voice": { "type": "string", "description": "音色 id（可选，缺省用绑定默认音色）" },
                "rate": { "type": "number", "description": "语速 0.5-2.0（可选，慢速教学用小于 1）" },
                "pitch": { "type": "number", "description": "语调（可选）" },
                "instruction": { "type": "string", "description": "自然语言风格指令（可选）" },
                "language_hints": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "语言提示（如 [\"ja\"]，可选）"
                }
            },
            "required": ["text"]
        }),
    }
}
