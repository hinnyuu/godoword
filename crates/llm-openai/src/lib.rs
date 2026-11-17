//! OpenAI 兼容 LLM 适配器。实现 `kernel_api::LlmProvider`：
//! `/compatible-mode/v1/chat/completions` 流式，归一化 reasoning_content 与 tool_calls。
//! M1a 实现中（骨架 + 脱敏 fixtures 已录）。
