//! 本地真 key 活体验证（`#[ignore]`，CI 永不用真 key）。
//! 运行：`QWEN_TOKEN_PLAN_KEY=<key> cargo test -p tts-qwen-audio --test live -- --ignored`
//! base_url 为公开网关 host 根（非密钥）；key 仅经 env 传入，不落盘不入仓库。

use futures_util::StreamExt;
use kernel_api::{AudioEvent, AudioFormat, ProviderConn, TtsProvider, TtsRequest};
use secrecy::SecretString;
use serde_json::json;
use tts_qwen_audio::QwenTtsProvider;

const BASE_URL: &str = "https://token-plan.maas.qianwenaiapi.com";

fn conn() -> ProviderConn {
    let key = std::env::var("QWEN_TOKEN_PLAN_KEY").expect("须设 QWEN_TOKEN_PLAN_KEY");
    ProviderConn {
        base_url: BASE_URL.to_string(),
        api_key: SecretString::from(key),
        extra: json!({}),
    }
}

#[tokio::test]
#[ignore]
async fn live_list_voices() {
    let p = QwenTtsProvider::new();
    let voices = p
        .list_voices(&conn(), "qwen-audio-3.0-tts-plus")
        .await
        .expect("list_voices");
    assert!(voices.iter().any(|v| v.id == "longanlingxin"));
}

#[tokio::test]
#[ignore]
async fn live_synthesize_stream() {
    let p = QwenTtsProvider::new();
    let req = TtsRequest {
        model: "qwen-audio-3.0-tts-plus".to_string(),
        text: "こんにちは".to_string(),
        voice: "longanlingxin".to_string(),
        format: AudioFormat::Pcm,
        sample_rate: 24000,
        rate: Some(0.8),
        pitch: None,
        instruction: None,
        language_hints: vec!["ja".to_string()],
        word_timestamps: true,
        extra: json!({}),
    };
    let mut stream = p
        .synthesize_stream(&conn(), req)
        .await
        .expect("synthesize_stream 应成功");
    let mut got_start = false;
    let mut got_words = false;
    let mut pcm_bytes = 0usize;
    let mut got_end = false;
    let mut got_usage = false;
    while let Some(e) = stream.next().await {
        match e.expect("事件应成功") {
            AudioEvent::Start {
                format,
                sample_rate,
            } => {
                assert_eq!(format, AudioFormat::Pcm);
                assert_eq!(sample_rate, 24000);
                got_start = true;
            }
            AudioEvent::Words(w) => {
                if !w.is_empty() {
                    got_words = true;
                }
            }
            AudioEvent::Chunk { pcm, .. } => pcm_bytes += pcm.len(),
            AudioEvent::Usage(_) => got_usage = true,
            AudioEvent::End { complete } => {
                assert!(
                    complete.map(|b| b.len()).unwrap_or(0) > 0,
                    "complete 应非空"
                );
                got_end = true;
            }
        }
    }
    assert!(got_start && got_words && got_end && got_usage);
    assert!(pcm_bytes > 0, "应收到 PCM 块");
    assert_eq!(pcm_bytes % 2, 0, "16-bit PCM 字节数应为偶数");
}
