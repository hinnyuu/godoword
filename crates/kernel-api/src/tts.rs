//! TTS 适配器契约。每款 TTS 一个 crate（协议各异，不走 OpenAI 兼容）。

use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;

use crate::error::ProviderError;
use crate::types::{
    AudioFormat, ModelInfo, ProviderConn, TtsRequest, TtsUsage, VoiceInfo, WordTimestamp,
};

/// 归一化音频事件。URL 返回模式的 TTS 由适配器内部下载并归一化为流。
#[derive(Debug, Clone)]
pub enum AudioEvent {
    Start {
        format: AudioFormat,
        sample_rate: u32,
    },
    /// 裸 PCM 块（流式播放），seq 单调。
    Chunk {
        seq: u32,
        pcm: Bytes,
    },
    /// 词级时间戳，增量追加语义（适配器对累积数组去重）。
    Words(Vec<WordTimestamp>),
    /// 完成，complete 为完整音频（供缓存）。
    End {
        complete: Option<Bytes>,
    },
    Usage(TtsUsage),
}

/// 归一化音频流。
pub type AudioStream = Pin<Box<dyn Stream<Item = Result<AudioEvent, ProviderError>> + Send>>;

/// TTS 适配器（tts-qwen-audio / 未来每款 TTS）。
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// 适配器标识，如 "tts-qwen-audio"。
    fn id(&self) -> &'static str;
    /// 静态模型目录。
    fn models(&self) -> Vec<ModelInfo>;
    /// 拉取音色列表（设置 UI 用）。
    async fn list_voices(
        &self,
        conn: &ProviderConn,
        model: &str,
    ) -> Result<Vec<VoiceInfo>, ProviderError>;
    /// 发起流式合成。
    async fn synthesize_stream(
        &self,
        conn: &ProviderConn,
        req: TtsRequest,
    ) -> Result<AudioStream, ProviderError>;
}
