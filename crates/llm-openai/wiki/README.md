# llm-openai（OpenAI 兼容 LLM 适配器）

godoword 的 LLM 适配器之一，对接 OpenAI 兼容接口（如 qwen 网关 `/compatible-mode/v1`），
把上游流式响应归一化为内核统一事件（正文/思考/工具调用/用量）。用户自填 key（BYOK），
适配器只代理调用、不持久化 key。

当前已实现 OpenAI 兼容 LLM 适配（M1a）：流式聊天归一化（正文/思考/工具调用/用量）、模型列表、错误归一化，
经 fixture 黄金测试 + 本地真 key 活体验证。面向用户的使用说明待设置 UI（M1d）落地后补充。