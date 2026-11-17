//! qwen-audio TTS 适配器。实现 `kernel_api::TtsProvider`：
//! DashScope 原生 `/api/v1/services/audio/tts/SpeechSynthesizer` 流式，
//! 归一化 sentence 事件为裸 PCM 流与词级时间戳。M1a 实现中（骨架 + 脱敏 fixtures 已录）。
