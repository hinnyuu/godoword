//! DashScope 原生 TTS 线缆类型（SpeechSynthesizer 流帧 / URL 模式响应）。

use serde::Deserialize;

/// 流帧 / URL 模式响应顶层。
#[derive(Debug, Deserialize)]
pub struct TtsFrame {
    pub output: TtsOutput,
    #[serde(default)]
    pub usage: Option<TtsUsageWire>,
}

/// 输出体。
#[derive(Debug, Deserialize)]
pub struct TtsOutput {
    #[serde(default)]
    pub audio: TtsAudio,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub sentence: Option<TtsSentence>,
}

/// 音频载荷（流式 data base64 / 末帧 url）。
#[derive(Debug, Default, Deserialize)]
pub struct TtsAudio {
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// 句子（词级时间戳）。
#[derive(Debug, Deserialize)]
pub struct TtsSentence {
    #[serde(default)]
    pub words: Vec<WordWire>,
}

/// 词级时间戳线缆形（毫秒）。
#[derive(Debug, Deserialize)]
pub struct WordWire {
    pub text: String,
    pub begin_index: u32,
    pub end_index: u32,
    pub begin_time: u32,
    pub end_time: u32,
}

/// 用量线缆形（3.0 按字符，亦含 token）。
#[derive(Debug, Deserialize)]
pub struct TtsUsageWire {
    #[serde(default)]
    pub characters: Option<u32>,
    #[serde(default)]
    pub input_tokens: Option<u32>,
    #[serde(default)]
    pub output_tokens: Option<u32>,
}

/// 上游错误体（DashScope {code, message, request_id}）。
#[derive(Debug, Deserialize)]
pub struct ErrorBody {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}
