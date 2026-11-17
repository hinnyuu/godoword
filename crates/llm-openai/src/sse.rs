//! SSE 流归一化：OpenAI 兼容流块 → [`kernel_api::ChatChunk`]。
//! reasoning_content 增量、tool_calls 按 index 组装完整后一次性发出。

use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;
use kernel_api::{ChatChunk, ChatStream, FinishReason, ProviderError, ToolCall, Usage};
use serde_json::Value;

use crate::wire::{ChatCompletionChunk, UsageWire};

/// 工具调用组装器（按 index 累积 id/name/arguments 片段）。
#[derive(Default)]
struct ToolBuilder {
    id: String,
    name: String,
    arguments: String,
}

fn map_finish(reason: &str) -> FinishReason {
    match reason {
        "stop" => FinishReason::Stop,
        "tool_calls" => FinishReason::ToolCalls,
        "length" => FinishReason::Length,
        "content_filter" => FinishReason::ContentFilter,
        _ => FinishReason::Error,
    }
}

fn map_usage(u: UsageWire) -> Usage {
    Usage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        reasoning_tokens: u.completion_tokens_details.and_then(|d| d.reasoning_tokens),
        total_tokens: u.total_tokens,
    }
}

fn assemble(builders: &[ToolBuilder]) -> Vec<ToolCall> {
    builders
        .iter()
        .map(|b| ToolCall {
            id: b.id.clone(),
            name: b.name.clone(),
            arguments: serde_json::from_str(&b.arguments).unwrap_or(Value::Null),
        })
        .collect()
}

/// 把上游字节流归一化为 ChatChunk 流。按行解析 SSE `data:` 帧，容忍 `id:`/`event:`/空行。
pub fn normalize<S>(bytes: S) -> ChatStream
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
{
    Box::pin(async_stream::stream! {
        let mut buf: Vec<u8> = Vec::new();
        let mut builders: Vec<ToolBuilder> = Vec::new();
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
                if payload == "[DONE]" {
                    return;
                }
                let parsed: ChatCompletionChunk = match serde_json::from_str(payload) {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(ProviderError::Internal { message: format!("SSE 解析失败：{e}") });
                        return;
                    }
                };
                if let Some(u) = parsed.usage {
                    yield Ok(ChatChunk::Usage(map_usage(u)));
                }
                for choice in parsed.choices {
                    if let Some(r) = choice.delta.reasoning_content {
                        if !r.is_empty() {
                            yield Ok(ChatChunk::ReasoningDelta(r));
                        }
                    }
                    if let Some(c) = choice.delta.content {
                        if !c.is_empty() {
                            yield Ok(ChatChunk::TextDelta(c));
                        }
                    }
                    if let Some(tcs) = choice.delta.tool_calls {
                        for tc in tcs {
                            let idx = tc.index as usize;
                            if builders.len() <= idx {
                                builders.resize_with(idx + 1, ToolBuilder::default);
                            }
                            let b = &mut builders[idx];
                            if let Some(id) = tc.id {
                                b.id = id;
                            }
                            if let Some(f) = tc.function {
                                if let Some(n) = f.name {
                                    b.name = n;
                                }
                                if let Some(a) = f.arguments {
                                    b.arguments.push_str(&a);
                                }
                            }
                        }
                    }
                    if let Some(fr) = choice.finish_reason {
                        if fr == "tool_calls" && !builders.is_empty() {
                            yield Ok(ChatChunk::ToolCalls(assemble(&builders)));
                        }
                        yield Ok(ChatChunk::Finish(map_finish(&fr)));
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream::{self, StreamExt};

    const FIXTURE: &str = include_str!("../tests/fixtures/chat-stream-toolcall.sse");

    async fn collect(pieces: Vec<Bytes>) -> Vec<ChatChunk> {
        let s = stream::iter(pieces.into_iter().map(Ok::<_, reqwest::Error>));
        normalize(s)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.expect("chunk 应成功"))
            .collect()
    }

    fn assert_golden(chunks: &[ChatChunk]) {
        let reasoning = chunks
            .iter()
            .filter(|c| matches!(c, ChatChunk::ReasoningDelta(_)))
            .count();
        let text = chunks
            .iter()
            .filter(|c| matches!(c, ChatChunk::TextDelta(_)))
            .count();
        assert_eq!(reasoning, 12, "reasoning 增量数");
        assert_eq!(text, 68, "正文增量数");

        let tool_calls: Vec<_> = chunks
            .iter()
            .filter_map(|c| match c {
                ChatChunk::ToolCalls(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(tool_calls.len(), 1, "tool_calls 组装后仅发一次");
        assert_eq!(tool_calls[0].len(), 1);
        let tc = &tool_calls[0][0];
        assert_eq!(tc.name, "speak");
        assert_eq!(tc.arguments["text"], "こんにちは");
        assert_eq!(tc.arguments["rate"], 0.5);

        assert!(
            chunks
                .iter()
                .any(|c| matches!(c, ChatChunk::Finish(FinishReason::ToolCalls))),
            "finish=tool_calls"
        );

        let usage = chunks
            .iter()
            .find_map(|c| match c {
                ChatChunk::Usage(u) => Some(u),
                _ => None,
            })
            .expect("usage 帧");
        assert_eq!(usage.prompt_tokens, Some(234));
        assert_eq!(usage.completion_tokens, Some(252));
        assert_eq!(usage.reasoning_tokens, Some(36));
        assert_eq!(usage.total_tokens, Some(486));

        assert_eq!(chunks.len(), 83, "总块数 12+68+1+1+1");
    }

    #[tokio::test]
    async fn golden_single_chunk() {
        let chunks = collect(vec![Bytes::from_static(FIXTURE.as_bytes())]).await;
        assert_golden(&chunks);
    }

    #[tokio::test]
    async fn golden_split_chunks() {
        let pieces = FIXTURE
            .as_bytes()
            .chunks(7)
            .map(Bytes::copy_from_slice)
            .collect();
        let chunks = collect(pieces).await;
        assert_golden(&chunks);
    }
}
