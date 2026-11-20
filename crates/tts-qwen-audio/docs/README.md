# tts-qwen-audio 文档

> 适配器契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M1a-3 已实现）

实现 [`kernel_api::TtsProvider`]，目标上游为 DashScope 原生
`/api/v1/services/audio/tts/SpeechSynthesizer`（**不走 OpenAI 兼容**）。
`QwenTtsProvider`（共享 reqwest 连接池，rustls-tls）：

- `id()` = "tts-qwen-audio"；`models()` 静态目录（qwen-audio-3.0-tts-plus）。
- `list_voices`：本网关无音色 API，返回实测可用系统音色静态目录（longanlingxin 女 / longanlufeng 男，
  instruction/ssml/timestamps 皆支持，languages zh/en/ja）。
- `synthesize_stream`：`POST {base_url}/api/v1/services/audio/tts/SpeechSynthesizer`
  （`X-DashScope-SSE: enable`）→ SSE 归一化为 `AudioEvent` 流（`src/sse.rs`：Start → Words 跨帧去重 →
  Chunk 裸 PCM → Usage → End complete 拼接）。content-type 非 event-stream 时走 URL 模式
  （单响应 audio.url 内部下载 → `normalize_url` 合成流）。**URL 下载查 HTTP 状态 + 32MiB 大小上限
  （审计 A1：错误体不得当音频缓存播放，异常响应不得无界累积；错误归一化复用 map_error）**。
- 错误归一化（`map_error`）：401/403→Auth、429→RateLimit、404→ModelUnavailable、
  InvalidApiKey/AccessDenied→Auth、Throttling/quota→Quota、5xx→Network(retryable)、其余→Internal。
- `base_url` 为 host 根（如 `https://host`），适配器追加 DashScope 路径。
- **超时（审计 F1）**：reqwest connect 10s + read 30s（块间隔，流式安全；URL 模式下载亦受约束）。

测试：`src/sse.rs` 单元测试用 fixture 黄金锚定（流式单块 + 13 字节切片，7 事件精确断言含 complete=52800 字节；
URL 模式合成流）；`src/lib.rs` URL 下载本地 HTTP 服务测试（**审计 A1 负例：404 归一化错误/超限报错** + 正例归一化）；
`tests/live.rs` 本地真 key 活体验证（`#[ignore]`，CI 不跑）。

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

- 3.1 系列日语专用音色本网关不存在（roadmap 未来网关扩展）。
- 音色目录当前静态（本网关无音色 API）；未来网关若提供 API 可改动态拉取。
- kernel-api M1d 新增 utoipa 有意耦合：本适配器返回的 `VoiceInfo`/`ModelInfo` 派生 ToSchema（OpenAPI schema 补全），适配器实现不变。
- **M2**（history 1614/1721/0010）：gojuon 标准音频**用户侧运行时**经本适配器合成（用户 key/音色，白名单稳定 key + 持久资产区，一次生成永久使用；**P1 白名单机制已落**，gojuon 消费者 P4）；领读经 speak 新鲜合成；适配器实现不变。