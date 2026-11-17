//! 本地真 key 活体验证（`#[ignore]`，CI 永不用真 key）。
//! 运行：`QWEN_TOKEN_PLAN_KEY=<key> cargo test -p llm-openai --test live -- --ignored`
//! base_url 为公开网关端点（非密钥）；key 仅经 env 传入，不落盘不入仓库。

use futures_util::StreamExt;
use kernel_api::{ChatChunk, ChatMessage, ChatRequest, LlmProvider, ProviderConn, ToolSpec};
use llm_openai::OpenAiProvider;
use secrecy::SecretString;
use serde_json::json;

const BASE_URL: &str = "https://token-plan.maas.qianwenaiapi.com/compatible-mode/v1";

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
async fn live_list_models() {
    let p = OpenAiProvider::new();
    let models = p.list_models(&conn()).await.expect("list_models 应成功");
    assert!(
        models.iter().any(|m| m.id.contains("qwen4-max")),
        "模型目录应含 qwen4-max"
    );
}

#[tokio::test]
#[ignore]
async fn live_chat_speak() {
    let p = OpenAiProvider::new();
    let req = ChatRequest {
        model: "qwen4-max-preview".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "用 speak 工具慢速朗读日语「こんにちは」。".to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
        }],
        tools: vec![ToolSpec {
            name: "speak".to_string(),
            description: "朗读文本（TTS）".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "text": { "type": "string" }, "rate": { "type": "number" } },
                "required": ["text"]
            }),
        }],
        temperature: None,
        max_tokens: None,
        extra: json!({}),
    };
    let mut stream = p
        .chat_stream(&conn(), req)
        .await
        .expect("chat_stream 应成功");
    let mut got_tool = false;
    let mut got_finish = false;
    let mut got_usage = false;
    while let Some(c) = stream.next().await {
        match c.expect("chunk 应成功") {
            ChatChunk::ToolCalls(t) => {
                assert_eq!(t[0].name, "speak");
                assert!(t[0].arguments["text"].is_string());
                got_tool = true;
            }
            ChatChunk::Finish(_) => got_finish = true,
            ChatChunk::Usage(_) => got_usage = true,
            _ => {}
        }
    }
    assert!(got_tool, "应组装出 speak 工具调用");
    assert!(got_finish, "应有 finish");
    assert!(got_usage, "应有 usage");
}
