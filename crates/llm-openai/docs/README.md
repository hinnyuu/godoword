# llm-openai 文档

> 适配器契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M1a 骨架）

实现 [`kernel_api::LlmProvider`]，目标上游为 OpenAI 兼容 `/compatible-mode/v1`。
当前为骨架 + 脱敏 fixtures，适配器实现 M1a-2 落地。

## 脱敏 fixtures（tests/fixtures/）

本地真 key 录制、去密钥/签名 URL 后的真实响应，作为归一化黄金测试锚点（CI 永不用真 key）：

- `models.json`：`GET /models` 响应（模型目录，list_models 用）。
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

- list_models / chat_stream / features 实现 M1a-2。
- token 估算当前字符启发式（roadmap 换 tokenizer，适配器未来提供 count_tokens）。