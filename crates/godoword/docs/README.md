# godoword 组合根

bin crate，以 cargo feature 注册适配器与教学模块到 kernel，启动 axum 服务。

## 当前状态（M1a）

注册 llm-openai + tts-qwen-audio 适配器 + kernel 回合循环，提供会话 REST + SSE：

- `/healthz`、`/readyz`、`/api/v1/meta`（milestone=M1a，dev_bypass 标志），静态占位页。
- `POST/GET /api/v1/sessions`（创建/列表）、`GET/DELETE /api/v1/sessions/{id}`（转录/删除）、
  `POST /api/v1/sessions/{id}/messages`（→ 该回合 SessionEvent SSE 流）、
  `POST /api/v1/sessions/{id}/cancel`（显式取消）、`GET /api/v1/audio/{id}`（缓存 PCM 回放）。
- 状态 `AppState`：MemSessions/MemAudio（内存）+ Runtime（仅 dev-bypass）+ 当前用户。

默认双栈监听 `[::]:8080`（兼容 IPv4），IPv6 不可用回退 `0.0.0.0`；`GODOWORD_LISTEN` 可覆盖。
优雅退出处理 SIGTERM/Ctrl-C。

## dev-bypass feature（DEV-DEBT T1）

`--features dev-bypass` 启用 `src/dev.rs` env key 旁路（GODOWORD_DEV_LLM_KEY/TTS_KEY + base_url/model，
QWEN_TOKEN_PLAN_KEY 兜底）+ 固定 dev user，构造 Runtime 使回合可用。**release Dockerfile 永不启用**
（适配器为 optional dep 随 dev-bypass 编译，release M1a 无回合能力，messages 返回 503）。
M1c 真认证/绑定/加密落地后删除本模块。key 仅经 env，绝不落盘/入日志/入仓库。

## roadmap 差异

- /data 卷初始化、主密钥、迁移、自适应降权 M1b；持久化会话/认证会话层 M1c。
- 适配器 M1b/c 转正式绑定后改为非 optional dep。
- 见 [architecture](../../../docs/project/architecture.md)。