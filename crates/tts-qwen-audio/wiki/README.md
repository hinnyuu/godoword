# tts-qwen-audio（qwen-audio TTS 适配器）

godoword 的 TTS 适配器之一，对接 qwen-audio DashScope 原生语音合成接口，
把上游流式音频归一化为内核统一事件（裸 PCM 块 + 词级时间戳），供前端排队播放与跟读高亮。
用户自填 key（BYOK），适配器只代理调用、不持久化 key。

当前为 M1a 骨架，适配器实现进行中。面向用户的使用说明待设置 UI（M1d）落地后补充。