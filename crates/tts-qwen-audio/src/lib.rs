//! qwen-audio TTS 适配器。实现 `kernel_api::TtsProvider`：
//! DashScope 原生 `/api/v1/services/audio/tts/SpeechSynthesizer` 流式，
//! 归一化 sentence 事件为裸 PCM 流与词级时间戳。协议事实与脱敏 fixtures 见 `docs/README.md`。

mod sse;
mod wire;

use async_trait::async_trait;
use futures_util::StreamExt;
use kernel_api::{
    AudioFormat, AudioStream, ModelInfo, ProviderConn, ProviderError, TtsProvider, TtsRequest,
    TtsUsage, VoiceInfo,
};
use secrecy::ExposeSecret;
use serde_json::{json, Value};
use std::time::Duration;

/// DashScope 路径（base_url 为 host 根，如 `https://host`）。
const TTS_PATH: &str = "api/v1/services/audio/tts/SpeechSynthesizer";

/// URL 模式下载大小上限（审计 A1：防异常/恶意响应撑爆内存；正常 TTS 输出远小于此）。
const MAX_AUDIO_BYTES: usize = 32 * 1024 * 1024;

/// qwen-audio TTS 适配器（DashScope 原生协议，不走 OpenAI 兼容）。
pub struct QwenTtsProvider {
    client: reqwest::Client,
}

impl QwenTtsProvider {
    /// 新建适配器（共享 reqwest 连接池）。
    /// 超时（审计 F1）：connect 10s + read 30s（块间隔，流式安全；URL 模式下载亦受 read 超时约束）。
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client 构建"),
        }
    }
}

impl Default for QwenTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn endpoint(base_url: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), TTS_PATH)
}

fn bearer(conn: &ProviderConn) -> String {
    format!("Bearer {}", conn.api_key.expose_secret())
}

fn format_str(f: AudioFormat) -> &'static str {
    match f {
        AudioFormat::Pcm => "pcm",
        AudioFormat::Wav => "wav",
        AudioFormat::Mp3 => "mp3",
        AudioFormat::Opus => "opus",
    }
}

/// 组装 DashScope 请求体（input.text + parameters）。
fn build_request(req: &TtsRequest) -> Value {
    let mut params = json!({
        "voice": req.voice,
        "format": format_str(req.format),
        "sample_rate": req.sample_rate,
        "word_timestamp_enabled": req.word_timestamps,
    });
    if !req.language_hints.is_empty() {
        params["language_hints"] = json!(req.language_hints);
    }
    if let Some(rate) = req.rate {
        params["rate"] = json!(rate);
    }
    if let Some(pitch) = req.pitch {
        params["pitch"] = json!(pitch);
    }
    if let Some(instr) = &req.instruction {
        params["instruction"] = json!(instr);
    }
    if let Value::Object(extra) = &req.extra {
        for (k, v) in extra {
            params[k] = v.clone();
        }
    }
    json!({ "model": req.model, "input": { "text": req.text }, "parameters": params })
}

/// HTTP 错误 → 归一化 ProviderError（DashScope {code, message}）。
async fn map_error(resp: reqwest::Response, model: &str) -> ProviderError {
    let status = resp.status().as_u16();
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs);
    let text = resp.text().await.unwrap_or_default();
    let body = serde_json::from_str::<wire::ErrorBody>(&text).ok();
    let msg = body
        .as_ref()
        .and_then(|b| b.message.clone())
        .unwrap_or_else(|| text.clone());
    let code = body
        .as_ref()
        .and_then(|b| b.code.clone())
        .unwrap_or_default();
    let lower = format!("{msg} {code}").to_lowercase();
    match status {
        401 | 403 => ProviderError::Auth { message: msg },
        429 => ProviderError::RateLimit { retry_after },
        404 => ProviderError::ModelUnavailable {
            model: model.to_string(),
        },
        _ if lower.contains("invalidapikey") || lower.contains("accessdenied") => {
            ProviderError::Auth { message: msg }
        }
        _ if lower.contains("throttling") || lower.contains("quota") => {
            ProviderError::Quota { message: msg }
        }
        500..=599 => ProviderError::Network {
            message: msg,
            retryable: true,
        },
        _ => ProviderError::Internal { message: msg },
    }
}

fn map_usage(u: &wire::TtsUsageWire) -> TtsUsage {
    TtsUsage {
        characters: u.characters,
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
    }
}

/// URL 模式音频下载（审计 A1）：**查 HTTP 状态**（403/404 错误体不得当音频缓存播放）
/// + 流式累积**大小上限**（防异常响应撑爆内存）。错误归一化复用 map_error。
async fn download_audio(
    client: &reqwest::Client,
    url: &str,
    model: &str,
) -> Result<Vec<u8>, ProviderError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ProviderError::Network {
            message: e.to_string(),
            retryable: true,
        })?;
    if !resp.status().is_success() {
        return Err(map_error(resp, model).await);
    }
    let mut pcm: Vec<u8> = Vec::new();
    let mut chunks = resp.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|e| ProviderError::Network {
            message: e.to_string(),
            retryable: true,
        })?;
        if pcm.len() + chunk.len() > MAX_AUDIO_BYTES {
            return Err(ProviderError::Internal {
                message: format!("音频 URL 下载超上限 {MAX_AUDIO_BYTES} 字节"),
            });
        }
        pcm.extend_from_slice(&chunk);
    }
    Ok(pcm)
}

#[async_trait]
impl TtsProvider for QwenTtsProvider {
    fn id(&self) -> &'static str {
        "tts-qwen-audio"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "qwen-audio-3.0-tts-plus".to_string(),
            display_name: Some("qwen-audio 3.0 TTS Plus".to_string()),
        }]
    }

    async fn list_voices(
        &self,
        _conn: &ProviderConn,
        _model: &str,
    ) -> Result<Vec<VoiceInfo>, ProviderError> {
        // 本网关无音色列表 API，返回实测可用系统音色静态目录。
        Ok(vec![
            VoiceInfo {
                id: "longanlingxin".to_string(),
                name: "龙安灵心".to_string(),
                gender: Some("female".to_string()),
                languages: vec!["zh".to_string(), "en".to_string(), "ja".to_string()],
                instruction: true,
                ssml: true,
                timestamps: true,
            },
            VoiceInfo {
                id: "longanlufeng".to_string(),
                name: "龙安露峰".to_string(),
                gender: Some("male".to_string()),
                languages: vec!["zh".to_string(), "en".to_string(), "ja".to_string()],
                instruction: true,
                ssml: true,
                timestamps: true,
            },
        ])
    }

    async fn synthesize_stream(
        &self,
        conn: &ProviderConn,
        req: TtsRequest,
    ) -> Result<AudioStream, ProviderError> {
        let body = build_request(&req);
        let resp = self
            .client
            .post(endpoint(&conn.base_url))
            .header("Authorization", bearer(conn))
            .header("X-DashScope-SSE", "enable")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network {
                message: e.to_string(),
                retryable: true,
            })?;
        if !resp.status().is_success() {
            return Err(map_error(resp, &req.model).await);
        }
        let is_sse = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|ct| ct.contains("text/event-stream"))
            .unwrap_or(false);
        if is_sse {
            return Ok(sse::normalize_stream(
                resp.bytes_stream(),
                req.format,
                req.sample_rate,
            ));
        }
        // URL 模式：单响应含 audio.url，内部下载归一化为流。
        let frame: wire::TtsFrame = resp.json().await.map_err(|e| ProviderError::Internal {
            message: format!("TTS 响应解析失败：{e}"),
        })?;
        let url = frame
            .output
            .audio
            .url
            .ok_or_else(|| ProviderError::Internal {
                message: "URL 模式响应缺 audio.url".to_string(),
            })?;
        let usage = frame.usage.as_ref().map(map_usage);
        let pcm = download_audio(&self.client, &url, &req.model).await?;
        Ok(sse::normalize_url(
            bytes::Bytes::from(pcm),
            usage,
            req.format,
            req.sample_rate,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel_api::AudioEvent;
    use secrecy::SecretString;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// 本地顺序 HTTP 服务（每连接一响应，读至请求头结束即回）；返回 root URL。
    async fn serve_seq(make: impl FnOnce(String) -> Vec<(String, Vec<u8>)>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let responses = make(root.clone());
        tokio::spawn(async move {
            for (head, body) in responses {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    let n = sock.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.write_all(&body).await;
                let _ = sock.shutdown().await;
            }
        });
        root
    }

    fn conn(root: &str) -> ProviderConn {
        ProviderConn {
            base_url: root.to_string(),
            api_key: SecretString::from("test-key".to_string()),
            extra: json!({}),
        }
    }

    fn req() -> TtsRequest {
        TtsRequest {
            model: "m".to_string(),
            text: "あ".to_string(),
            voice: "v".to_string(),
            format: AudioFormat::Pcm,
            sample_rate: 24000,
            rate: None,
            pitch: None,
            instruction: None,
            language_hints: vec![],
            word_timestamps: false,
            extra: json!({}),
        }
    }

    fn json_head() -> String {
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n".to_string()
    }

    /// 审计 A1 负例：URL 下载 404 → 归一化错误，**错误体不得当音频**。
    #[tokio::test]
    async fn url_download_http_error_is_error() {
        let root = serve_seq(|root| {
            let frame = json!({ "output": { "audio": { "url": format!("{root}/audio") } } });
            vec![
                (json_head(), serde_json::to_vec(&frame).unwrap()),
                (
                    "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n".to_string(),
                    br#"{"code":"NotFound","message":"audio expired"}"#.to_vec(),
                ),
            ]
        })
        .await;
        let p = QwenTtsProvider::new();
        let r = p.synthesize_stream(&conn(&root), req()).await;
        assert!(
            matches!(r, Err(ProviderError::ModelUnavailable { .. })),
            "404 须归一化错误，错误体不得当音频"
        );
    }

    /// 审计 A1 负例：URL 下载超大小上限 → 错误，不得无界累积。
    #[tokio::test]
    async fn url_download_oversize_is_error() {
        let root = serve_seq(|root| {
            let frame = json!({ "output": { "audio": { "url": format!("{root}/audio") } } });
            vec![
                (json_head(), serde_json::to_vec(&frame).unwrap()),
                (
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n".to_string(),
                    vec![0u8; MAX_AUDIO_BYTES + 1],
                ),
            ]
        })
        .await;
        let p = QwenTtsProvider::new();
        let r = p.synthesize_stream(&conn(&root), req()).await;
        assert!(
            matches!(r, Err(ProviderError::Internal { ref message }) if message.contains("超上限")),
            "超限须报错，不得无界累积"
        );
    }

    /// URL 模式正例：小音频下载归一化为 Start/Chunk/Usage/End。
    #[tokio::test]
    async fn url_download_success_normalizes() {
        let root = serve_seq(|root| {
            let frame = json!({
                "output": { "audio": { "url": format!("{root}/audio") } },
                "usage": { "characters": 1 }
            });
            vec![
                (json_head(), serde_json::to_vec(&frame).unwrap()),
                (
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n".to_string(),
                    vec![1u8, 2, 3, 4],
                ),
            ]
        })
        .await;
        let p = QwenTtsProvider::new();
        let stream = p.synthesize_stream(&conn(&root), req()).await.unwrap();
        let events: Vec<_> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], AudioEvent::Start { .. }));
        assert!(matches!(&events[1], AudioEvent::Chunk { seq: 0, pcm } if pcm.len() == 4));
        assert!(matches!(events[2], AudioEvent::Usage(_)));
        assert!(matches!(&events[3], AudioEvent::End { complete: Some(b) } if b.len() == 4));
    }
}
