//! OpenAI 兼容 LLM 适配器。实现 `kernel_api::LlmProvider`：
//! `/compatible-mode/v1/chat/completions` 流式，归一化 reasoning_content 与 tool_calls。
//! 上游协议事实与脱敏 fixtures 见 `docs/README.md` 与 `tests/fixtures/`。

mod sse;
mod wire;

use async_trait::async_trait;
use kernel_api::{
    ChatRequest, ChatStream, LlmFeatures, LlmProvider, ModelInfo, ProviderConn, ProviderError,
};
use secrecy::ExposeSecret;
use serde_json::{json, Value};
use std::time::Duration;

/// OpenAI 兼容 LLM 适配器。`base_url` 为 v1 根（如 `https://host/compatible-mode/v1`）。
pub struct OpenAiProvider {
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// 新建适配器（共享 reqwest 连接池）。
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}

fn bearer(conn: &ProviderConn) -> String {
    format!("Bearer {}", conn.api_key.expose_secret())
}

/// 组装 OpenAI 请求体（stream=true，tools 转 function 形，extra 顶层合并）。
fn build_request(req: &ChatRequest) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            let mut obj = json!({ "role": m.role, "content": m.content });
            if let Some(tc) = &m.tool_calls {
                obj["tool_calls"] = tc.clone();
            }
            if let Some(id) = &m.tool_call_id {
                obj["tool_call_id"] = json!(id);
            }
            obj
        })
        .collect();
    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    if !req.tools.is_empty() {
        body["tools"] = json!(req
            .tools
            .iter()
            .map(|t| json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.parameters }
            }))
            .collect::<Vec<_>>());
    }
    if let Some(temp) = req.temperature {
        body["temperature"] = json!(temp);
    }
    if let Some(max) = req.max_tokens {
        body["max_tokens"] = json!(max);
    }
    if let Value::Object(extra) = &req.extra {
        for (k, v) in extra {
            body[k] = v.clone();
        }
    }
    body
}

/// HTTP 错误响应 → 归一化 ProviderError。
async fn map_error(resp: reqwest::Response, model: &str) -> ProviderError {
    let status = resp.status().as_u16();
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs);
    let text = resp.text().await.unwrap_or_default();
    let envelope = serde_json::from_str::<wire::ErrorEnvelope>(&text).ok();
    let body = envelope.and_then(|e| e.error);
    let msg = body
        .as_ref()
        .and_then(|b| b.message.clone())
        .unwrap_or_else(|| text.clone());
    let code = body
        .as_ref()
        .and_then(|b| b.code.as_ref())
        .map(|c| c.to_string())
        .unwrap_or_default();
    let lower = format!("{msg} {code}").to_lowercase();
    match status {
        401 | 403 => ProviderError::Auth { message: msg },
        402 => ProviderError::Quota { message: msg },
        429 => ProviderError::RateLimit { retry_after },
        404 => ProviderError::ModelUnavailable {
            model: model.to_string(),
        },
        400 if lower.contains("context_length")
            || lower.contains("maximum context")
            || lower.contains("too long") =>
        {
            ProviderError::ContextLength
        }
        500..=599 => ProviderError::Network {
            message: msg,
            retryable: true,
        },
        _ => ProviderError::Internal { message: msg },
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn id(&self) -> &'static str {
        "llm-openai"
    }

    fn features(&self) -> LlmFeatures {
        LlmFeatures {
            tool_calling: true,
            reasoning: true,
            streaming: true,
        }
    }

    async fn list_models(&self, conn: &ProviderConn) -> Result<Vec<ModelInfo>, ProviderError> {
        let resp = self
            .client
            .get(endpoint(&conn.base_url, "models"))
            .header("Authorization", bearer(conn))
            .send()
            .await
            .map_err(|e| ProviderError::Network {
                message: e.to_string(),
                retryable: true,
            })?;
        if !resp.status().is_success() {
            return Err(map_error(resp, "").await);
        }
        let parsed: wire::ModelsResponse =
            resp.json().await.map_err(|e| ProviderError::Internal {
                message: format!("模型目录解析失败：{e}"),
            })?;
        Ok(parsed
            .data
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id,
                display_name: m.display_name,
            })
            .collect())
    }

    async fn chat_stream(
        &self,
        conn: &ProviderConn,
        req: ChatRequest,
    ) -> Result<ChatStream, ProviderError> {
        let body = build_request(&req);
        let resp = self
            .client
            .post(endpoint(&conn.base_url, "chat/completions"))
            .header("Authorization", bearer(conn))
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
        Ok(sse::normalize(resp.bytes_stream()))
    }
}
