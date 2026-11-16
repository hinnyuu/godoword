//! 共享类型与标识符。

use bytes::Bytes;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! uuid_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub uuid::Uuid);
        impl $name {
            /// 生成新标识符。
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

uuid_id!(
    UserId,
    "稳定内部用户标识，业务数据皆挂于此，与认证方式解耦。"
);
uuid_id!(SessionId, "会话标识。");
uuid_id!(EventId, "持久事件标识。");
uuid_id!(ProviderId, "provider 实例标识。");
uuid_id!(AudioId, "合成音频标识（缓存键关联）。");
uuid_id!(RunId, "回合标识。");

/// 模块标识，等于 crate 名（如 teach-gojuon）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleId(pub String);

/// 能力类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Capability {
    Llm,
    Tts,
    Stt,
}

/// 绑定 role：教师默认 / 摘要 / 进度提取 / 评估，支持差异化模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Default,
    Summarizer,
    Extractor,
    Assessor,
}

/// kernel 解析绑定并解密后传入适配器；适配器绝不持久化 key。
#[derive(Clone)]
pub struct ProviderConn {
    pub base_url: String,
    pub api_key: SecretString,
    pub extra: Value,
}

impl std::fmt::Debug for ProviderConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderConn")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .field("extra", &self.extra)
            .finish()
    }
}

/// 模型信息（list_models 返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: Option<String>,
}

/// 音色信息（list_voices 返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub gender: Option<String>,
    pub languages: Vec<String>,
    pub instruction: bool,
    pub ssml: bool,
    pub timestamps: bool,
}

/// LLM 用量。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

/// TTS 用量（3.0 按字符，3.1 按 token）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TtsUsage {
    pub characters: Option<u32>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

/// 词级时间戳（毫秒）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordTimestamp {
    pub text: String,
    pub begin_index: u32,
    pub end_index: u32,
    pub begin_time: u32,
    pub end_time: u32,
}

/// 音频编码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Pcm,
    Wav,
    Mp3,
    Opus,
}

/// 工具规格（JSON Schema 参数）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// LLM 发起的工具调用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// 工具执行结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub ok: bool,
    pub content: Value,
}

/// 聊天消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// LLM 请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub extra: Value,
}

/// TTS 请求（承载教学参数：慢速/语调/指令/语言提示/词级时间戳）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsRequest {
    pub model: String,
    pub text: String,
    pub voice: String,
    #[serde(default = "default_format")]
    pub format: AudioFormat,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    #[serde(default)]
    pub language_hints: Vec<String>,
    #[serde(default)]
    pub word_timestamps: bool,
    #[serde(default)]
    pub extra: Value,
}

fn default_format() -> AudioFormat {
    AudioFormat::Pcm
}
fn default_sample_rate() -> u32 {
    24000
}

/// 会话运行时参数（模块可配）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionParams {
    #[serde(default = "default_budget")]
    pub context_budget_tokens: u32,
    #[serde(default)]
    pub eviction: Eviction,
    #[serde(default = "default_tool_cap")]
    pub tool_iteration_cap: u32,
}

impl Default for SessionParams {
    fn default() -> Self {
        Self {
            context_budget_tokens: default_budget(),
            eviction: Eviction::default(),
            tool_iteration_cap: default_tool_cap(),
        }
    }
}

fn default_budget() -> u32 {
    8000
}
fn default_tool_cap() -> u32 {
    8
}

/// 上下文淘汰策略。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Eviction {
    #[default]
    Truncate,
    Summarize,
}

/// 模块类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleKind {
    KernelApi,
    Adapter,
    Teaching,
    Root,
}

/// 适配器提供的能力。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provides {
    pub capability: Capability,
    pub adapter_id: String,
}

/// 教学模块所需能力。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requires {
    pub capabilities: Vec<Capability>,
}

/// 前端清单（lazy chunk 挂载）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendManifest {
    pub entry: String,
    pub routes: Vec<String>,
    #[serde(default)]
    pub nav: Value,
}

/// 路由规格（AuthProvider extra_routes 用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSpec {
    pub method: String,
    pub path: String,
}

/// 模块清单（module.yaml 反序列化）。impact/cross_module 由 docs-gate 读取，kernel 忽略。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: ModuleKind,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides: Option<Provides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Requires>,
    #[serde(default)]
    pub session_params: SessionParams,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontend: Option<FrontendManifest>,
    #[serde(default)]
    pub consumes_events: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrations: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tables_prefix: Option<String>,
}

/// 音频字节块别名。
pub type AudioBytes = Bytes;
