# llm-openai 文档

> 适配器契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M1a-2 已实现）

实现 [`kernel_api::LlmProvider`]，目标上游为 OpenAI 兼容 `/compatible-mode/v1`。
`OpenAiProvider`（共享 reqwest 连接池，rustls-tls）：

- `id()` = "llm-openai"；`features()` = tool_calling/reasoning/streaming 皆 true。
- `list_models`：`GET {base_url}/models` → `ModelInfo`。
- `chat_stream`：`POST {base_url}/chat/completions`（stream=true）→ SSE 归一化为 `ChatChunk` 流
  （`src/sse.rs`：reasoning_content/content 增量、tool_calls 按 index 组装完整后一次性发、usage、finish）。
- 错误归一化（`map_error`）：401/403→Auth、402→Quota、429→RateLimit(retry_after)、404→ModelUnavailable、
  400 context_length→ContextLength、5xx→Network(retryable)、其余→Internal。
- `base_url` 为 v1 根（如 `https://host/compatible-mode/v1`），适配器追加 `/models`、`/chat/completions`。
- **超时（审计 F1）**：reqwest connect 10s + read 30s（块间隔，流式安全；上游停滞→Network(retryable)，
  kernel 重试/硬超时兜底；测试端点不无限挂起）。

测试：`src/sse.rs` 单元测试用 fixture 黄金锚定归一化（单块 + 7 字节切片跨块缓冲，83 块精确断言）；
`tests/live.rs` 本地真 key 活体验证（`#[ignore]`，CI 不跑）。

## 脱敏 fixtures（tests/fixtures/）

本地真 key 录制、去密钥/签名 URL 后的真实响应，作为归一化黄金测试锚点（CI 永不用真 key）：

- `models.json`：`GET /models` 响应脱敏样本（协议参考；list_models 由 live 测试覆盖，当前无单测引用此 fixture）。
- `chat-stream-toolcall.sse`：`POST /chat/completions`（stream=true，定义 speak 工具）完整 SSE 流，
  覆盖 reasoning_content 增量 + text 增量 + tool_calls 增量组装 + finish_reason + usage + `[DONE]`。

## 上游协议事实（实测，实现依据）

- SSE 标准 `data: {chunk}` 格式，末帧 `data: [DONE]`。
- chunk 形如 `{id, object:"chat.completion.chunk", created, model, choices:[{index, delta, finish_reason}], usage}`。
- `delta.reasoning_content` 增量思考文本 → 归一化 `ChatChunk::ReasoningDelta`。
- `delta.content` 增量正文 → `ChatChunk::TextDelta`。
- `delta.tool_calls[]` 按 `index` 增量：首帧含 `id`/`function.name`/`arguments:""`，
  后续帧仅 `index`/`function.arguments` 片段 → 适配器按 index 组装完整后一次性发 `ChatChunk::ToolCalls`。
- `finish_reason:"tool_calls"|"stop"|...` → `ChatChunk::Finish`。
- usage 帧 `choices:[]`，`usage.{prompt_tokens,completion_tokens,total_tokens}`，
  `completion_tokens_details.reasoning_tokens` → `Usage.reasoning_tokens`。
- 请求模型 `qwen4-max-preview`，响应 model 字段可能为 `qwen4.0-max-preview`（别名）。

## roadmap 差异

- token 估算当前字符启发式（roadmap 换 tokenizer，适配器未来提供 count_tokens）。
- anthropic 兼容另立 llm-anthropic 适配器（roadmap 远期）。
- kernel-api M1d 新增 utoipa 有意耦合：本适配器返回的 `ModelInfo` 派生 ToSchema（OpenAPI schema 补全→前端类型生成），适配器实现不变。