//! DashScope TTS 流归一化：sentence 事件 → [`kernel_api::AudioEvent`]。
//! 裸 PCM 块按序拼接，词级时间戳跨帧去重增量追加，末帧 complete 由块拼接。

use base64::Engine;
use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;
use kernel_api::{AudioEvent, AudioFormat, AudioStream, ProviderError, TtsUsage, WordTimestamp};
use std::collections::HashSet;

use crate::wire::{TtsFrame, TtsUsageWire, WordWire};

fn map_usage(u: &TtsUsageWire) -> TtsUsage {
    TtsUsage {
        characters: u.characters,
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
    }
}

fn word_key(w: &WordWire) -> (String, u32, u32, u32, u32) {
    (
        w.text.clone(),
        w.begin_index,
        w.end_index,
        w.begin_time,
        w.end_time,
    )
}

fn to_word(w: &WordWire) -> WordTimestamp {
    WordTimestamp {
        text: w.text.clone(),
        begin_index: w.begin_index,
        end_index: w.end_index,
        begin_time: w.begin_time,
        end_time: w.end_time,
    }
}

fn decode_pcm(data: &str) -> Result<Bytes, ProviderError> {
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map(Bytes::from)
        .map_err(|e| ProviderError::Internal {
            message: format!("PCM base64 解码失败：{e}"),
        })
}

/// 流式 SSE 归一化。先发 Start，逐帧发 Words（去重）/Chunk/Usage，finish=stop 发 End（complete 拼接）。
pub fn normalize_stream<S>(bytes: S, format: AudioFormat, sample_rate: u32) -> AudioStream
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
{
    Box::pin(async_stream::stream! {
        yield Ok(AudioEvent::Start { format, sample_rate });
        let mut buf: Vec<u8> = Vec::new();
        let mut seq: u32 = 0;
        let mut complete: Vec<u8> = Vec::new();
        let mut seen: HashSet<(String, u32, u32, u32, u32)> = HashSet::new();
        let mut usage_emitted = false;
        let mut stream = Box::pin(bytes);
        while let Some(item) = stream.next().await {
            let chunk = match item {
                Ok(b) => b,
                Err(e) => {
                    yield Err(ProviderError::Network { message: e.to_string(), retryable: true });
                    return;
                }
            };
            buf.extend_from_slice(&chunk);
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line);
                let payload = match line.trim().strip_prefix("data:") {
                    Some(p) => p.trim(),
                    None => continue,
                };
                let frame: TtsFrame = match serde_json::from_str(payload) {
                    Ok(f) => f,
                    Err(e) => {
                        yield Err(ProviderError::Internal { message: format!("TTS 帧解析失败：{e}") });
                        return;
                    }
                };
                if let Some(sentence) = &frame.output.sentence {
                    let new: Vec<WordTimestamp> = sentence
                        .words
                        .iter()
                        .filter(|w| seen.insert(word_key(w)))
                        .map(to_word)
                        .collect();
                    if !new.is_empty() {
                        yield Ok(AudioEvent::Words(new));
                    }
                }
                if let Some(data) = &frame.output.audio.data {
                    if !data.is_empty() {
                        let pcm = match decode_pcm(data) {
                            Ok(p) => p,
                            Err(e) => { yield Err(e); return; }
                        };
                        complete.extend_from_slice(&pcm);
                        yield Ok(AudioEvent::Chunk { seq, pcm });
                        seq += 1;
                    }
                }
                if let Some(u) = &frame.usage {
                    if !usage_emitted {
                        yield Ok(AudioEvent::Usage(map_usage(u)));
                        usage_emitted = true;
                    }
                }
                if frame.output.finish_reason.as_deref() == Some("stop") {
                    yield Ok(AudioEvent::End { complete: Some(Bytes::from(complete)) });
                    return;
                }
            }
        }
        yield Ok(AudioEvent::End { complete: Some(Bytes::from(complete)) });
    })
}

/// URL 模式归一化：完整音频字节 → Start/Chunk/Usage/End 合成流（适配器内部下载后调用）。
pub fn normalize_url(
    pcm: Bytes,
    usage: Option<TtsUsage>,
    format: AudioFormat,
    sample_rate: u32,
) -> AudioStream {
    Box::pin(async_stream::stream! {
        yield Ok(AudioEvent::Start { format, sample_rate });
        yield Ok(AudioEvent::Chunk { seq: 0, pcm: pcm.clone() });
        if let Some(u) = usage {
            yield Ok(AudioEvent::Usage(u));
        }
        yield Ok(AudioEvent::End { complete: Some(pcm) });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream::{self, StreamExt};

    const STREAM_FIXTURE: &str = include_str!("../tests/fixtures/tts-stream.sse");

    async fn collect(pieces: Vec<Bytes>) -> Vec<AudioEvent> {
        let s = stream::iter(pieces.into_iter().map(Ok::<_, reqwest::Error>));
        normalize_stream(s, AudioFormat::Pcm, 24000)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.expect("事件应成功"))
            .collect()
    }

    fn assert_golden(events: &[AudioEvent]) {
        assert!(matches!(
            events[0],
            AudioEvent::Start {
                format: AudioFormat::Pcm,
                sample_rate: 24000
            }
        ));
        let words: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                AudioEvent::Words(w) => Some(w),
                _ => None,
            })
            .collect();
        assert_eq!(words.len(), 1, "词级时间戳跨帧去重后仅发一次");
        assert_eq!(words[0].len(), 1);
        assert_eq!(words[0][0].text, "こんにちは.");
        assert_eq!(words[0][0].begin_time, 400);
        assert_eq!(words[0][0].end_time, 800);

        let chunks: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                AudioEvent::Chunk { seq, pcm } => Some((*seq, pcm.len())),
                _ => None,
            })
            .collect();
        assert_eq!(chunks.len(), 3, "三个 PCM 块");
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[1].0, 1);
        assert_eq!(chunks[2].0, 2);

        let usage = events
            .iter()
            .find_map(|e| match e {
                AudioEvent::Usage(u) => Some(u),
                _ => None,
            })
            .expect("usage");
        assert_eq!(usage.characters, Some(5));
        assert_eq!(usage.input_tokens, Some(2));
        assert_eq!(usage.output_tokens, Some(11));

        let end = events
            .iter()
            .find_map(|e| match e {
                AudioEvent::End { complete } => Some(complete),
                _ => None,
            })
            .expect("end");
        assert_eq!(
            end.as_ref().map(|b| b.len()),
            Some(52800),
            "complete PCM 拼接字节数"
        );

        assert_eq!(events.len(), 7, "Start+Words+3Chunk+Usage+End");
    }

    #[tokio::test]
    async fn golden_stream_single_chunk() {
        let events = collect(vec![Bytes::from_static(STREAM_FIXTURE.as_bytes())]).await;
        assert_golden(&events);
    }

    #[tokio::test]
    async fn golden_stream_split_chunks() {
        let pieces = STREAM_FIXTURE
            .as_bytes()
            .chunks(13)
            .map(Bytes::copy_from_slice)
            .collect();
        let events = collect(pieces).await;
        assert_golden(&events);
    }

    #[tokio::test]
    async fn url_mode_normalization() {
        let pcm = Bytes::from(vec![0u8; 4800]);
        let usage = TtsUsage {
            characters: Some(5),
            input_tokens: Some(2),
            output_tokens: Some(11),
        };
        let events: Vec<_> = normalize_url(pcm.clone(), Some(usage), AudioFormat::Pcm, 24000)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], AudioEvent::Start { .. }));
        assert!(matches!(&events[1], AudioEvent::Chunk { seq: 0, pcm } if pcm.len() == 4800));
        assert!(matches!(events[2], AudioEvent::Usage(_)));
        assert!(matches!(&events[3], AudioEvent::End { complete: Some(b) } if b.len() == 4800));
    }
}
