# tts-qwen-audio 文档

> 适配器契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M1a 骨架）

实现 [`kernel_api::TtsProvider`]，目标上游为 DashScope 原生
`/api/v1/services/audio/tts/SpeechSynthesizer`（**不走 OpenAI 兼容**）。
当前为骨架 + 脱敏 fixtures，适配器实现 M1a-3 落地。

## 脱敏 fixtures（tests/fixtures/）

本地真 key 录制、去签名 URL（含 OSSAccessKeyId/Signature）后的真实响应：

- `tts-stream.sse`：流式（`X-DashScope-SSE: enable`）完整 SSE，覆盖
  sentence-begin → sentence-synthesis（base64 PCM）→ sentence-end → 末帧（url+usage）。
- `tts-url.json`：非流式（URL 模式）单响应，`output.audio.url` + usage（适配器内部下载归一化为流）。

## 上游协议事实（实测，实现依据）

- SSE 格式 `id:N\nevent:result\ndata:{json}`（event 恒为 result，类型在 `output.type`）。
- data JSON：`{output:{type, original_text, normalized_text, audio:{data(b64), id, expires_at, url?},
  finish_reason, sentence:{index, words[]}}, request_id, usage?}`。
- `output.type` 序列：`sentence-begin`（words，无音频）→ `sentence-synthesis`（base64 PCM 块，words 增量）
  → `sentence-end`（words，usage）→ 末帧 `type:null`/`finish_reason:"stop"`/`audio.url`/usage。
- `audio.data` base64 解码为裸 PCM（请求 format=pcm/sample_rate=24000）；按序拼接 → `AudioEvent::Chunk`。
- `sentence.words[]`：`{text, begin_index, end_index, begin_time, end_time}`（毫秒）；
  跨帧增量追加，适配器对累积数组去重 → `AudioEvent::Words`。
- 末帧 `audio.url` 为完整音频（24h 有效）；流式模式 complete 可由 PCM 块拼接，URL 模式内部下载归一化。
- usage：`{input_tokens, output_tokens, total_tokens, characters}` → `TtsUsage`。
- 本网关音色 longanlingxin（女）/longanlufeng（男）；日语 `language_hints:["ja"]` 可用；
  `word_timestamp_enabled:true` 词级时间戳仅流式；rate [0.5,2.0]。

## roadmap 差异

- models / list_voices / synthesize_stream 实现 M1a-3。
- 3.1 系列日语专用音色本网关不存在（roadmap 未来网关扩展）。