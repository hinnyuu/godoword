# kernel 文档

> 契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M1a 最小回合循环）

内核实现 crate。适配器/教学模块只依赖 kernel-api，本 crate 实现内核服务（依赖倒置）。

- `Runtime`（`src/runtime.rs`）：回合循环。解析绑定 → 装配提示词（默认教师 persona + speak 工具规格）
  → 流式 LLM → 发 text/reasoning-delta → 收集 tool_calls → **内联执行 speak**（live 发 audio 事件）
  → 回喂工具结果 → 循环至 stop/守护上限 → 持久化消息 → run-end。返回 `SessionEvent` 流（SSE）。
- 守护：工具迭代上限（默认 8，`SessionParams.tool_iteration_cap`）、回合超时（默认 120s）、
  每会话单活跃回合锁（`ActiveGuard`，stream drop 自动释放 → 断开即取消）。
- speak 执行（`src/speak.rs` `prepare_speak`）：反序列化 `SpeakArgs` → 解析 TTS 绑定 → 组装 `TtsRequest`
  （model/format=pcm/sample_rate=24000/word_timestamps=true 由 kernel 定）；音频流迭代内联于循环，
  live 发 AudioStart/AudioChunk(b64 PCM)/WordTimestamps/Usage/AudioEnd，complete 入缓存。
- 错误归一化：`ProviderError` → `SessionEvent::Error`（auth/quota→action:settings、rate_limit/network 可重试等）。

## 内部接缝（非契约，零返工设计）

`src/seam.rs` 三 trait，回合循环依赖之，M1a 内存/env impl → M1b/c DB/加密 impl 不重写循环：

- `BindingResolver`：解析 (user,module,cap,role) → `ResolvedBinding{conn,model,params}`。M1a env impl 在
  godoword（dev-bypass feature）；M1b/c DB 5 级 fallback + 解密。
- `SessionRepo`：create/exists/history/append/list/delete。`MemSessions`（内存）M1a 生产默认（临时，dev-debt T2）
  + 永久测试替身；M1c DB impl。
- `AudioCache`：put/get `CachedAudio{pcm,words,format,sample_rate}`。`MemAudio`（内存）M1a 默认；M1c 文件系统 `/data/cache`。

## M1 简化与 roadmap 差异

- 无教学模块：用 `DEFAULT_TEACHER_PERSONA`（`src/persona.rs`），无模块 persona/无 plan.teacher_prompt；
  默认教师模式是否作永久产品态留 M2（dev-debt T4）。
- 无持久化/加密/认证：M1b 存储基底 + M1c 认证落地。
- 上下文装配当前全量历史（无预算/淘汰）；token 预算 + truncate/summarize 淘汰 + 淘汰前进度提取 M1c。
- on_disconnect 当前仅 cancel（drop 即取消）；continue + detach 上限 M1c。
- 用量计量当前仅发 Usage 事件；call_log 持久化 M1c。