# godoword 组合根

bin crate，以 cargo feature 注册适配器与教学模块到 kernel，启动 axum 服务。

## 当前状态（M1b）

幂等 entrypoint + 存储基底 + DB 绑定解析 + 会话 REST/SSE + 配置只读：

- 启动序（`src/main.rs`）：`Config::load` → `init::init_data_dir`（自适应降权 + 建 /data 子目录）
  → `MasterKey::load`（生成/读主密钥）→ `open_sqlite`/`open_pg` → `run_migrations`（迁移前备份）→ 建状态 → 监听。
- `/healthz`（存活）、`/readyz`（存储探活 SELECT 1）、`/api/v1/meta`（milestone=M1b，dev_bypass 标志）、
  `/api/v1/config`（脱敏只读配置视图）、`/api/v1/openapi.json`（utoipa OpenAPI，M1d 生成 TS）、静态占位页。
- `POST/GET /api/v1/sessions`、`GET/DELETE /api/v1/sessions/{id}`、`POST /sessions/{id}/messages`（→SSE）、
  `POST /sessions/{id}/cancel`、`GET /api/v1/audio/{id}`。
- `POST/GET /api/v1/providers`、`GET/PUT/DELETE /api/v1/providers/{id}`、`GET /providers/{id}/models`、
  `GET /providers/{id}/voices`（测试连接）；`POST/GET /api/v1/bindings`、`GET/PUT/DELETE /api/v1/bindings/{id}`。
  provider key 加密存储不回显；models/voices 解密 key 调适配器（`src/settings.rs`）。
- 状态 `AppState`：MemSessions/MemAudio（内存，M1c 换 DB/文件系统）+ Runtime（仅 dev-bypass）+ 当前用户
  + `Arc<Store>` + `Config` + `Arc<MasterKey>` + 适配器 Option（models/voices）。
- **用户级 REST release 把守**：`user_access()` 门（无 dev-bypass → 503），M1c 真认证中间件替换。
  healthz/readyz/meta/config/openapi/静态 release 开放。
- 监听地址取自配置 `listen`（默认双栈 `[::]:8080`，IPv6 不可用回退 `0.0.0.0`）。优雅退出 SIGTERM/Ctrl-C。

## 部署初始化（`src/init.rs`，DEV-DEBT T3 回归）

自适应降权（Rust 进程内，无 shell，distroless 兼容）：读 `/proc/self/uid_map` 判 rootless/rootful。
**rootless 保持 uid 0**（容器 0 映射宿主调用者，文件无 subuid 残留）；**rootful euid=0** 时 stat /data 属主
→ 降到属主 uid/gid，属主 0 则 chown -R /data 到 1000（PUID/PGID 覆盖）再降权（setgroups→setgid→setuid）。
chown 在降权前（含 M1a 旧 root 文件升级）。/data 写 EACCES → SELinux :Z 提示。幂等建 db/keys/cache/backups。
详见 [architecture](../../../docs/project/architecture.md) 与 [wiki/deploy](../../../wiki/deploy/quickstart.md)。

## dev-bypass feature（DEV-DEBT T1）

`--features dev-bypass` 启用 `src/dev.rs` 固定 dev user + env 幂等种入 DB（`seed`：GODOWORD_DEV_* →
加密 provider/binding）+ 适配器（llm-openai/tts-qwen-audio）+ Runtime（DbBindingResolver 生产绑定路径）。
**release Dockerfile 永不启用**（release M1b 用户级 REST 返回 503，待 M1c 认证开放）。M1c 真认证后删除本模块。
key 仅经 env，绝不落盘/入日志/入仓库。

## roadmap 差异

- 存储基底/主密钥/迁移/自适应降权/配置/provider-binding CRUD REST M1b 已落地；
  持久化会话/音频（DB/文件系统）+ 认证 M1c。
- 适配器随 dev-bypass 编译（optional）；M1c 真认证后转非 optional + 删除 dev-bypass。
- models/voices 响应 OpenAPI 暂以通用 object 标注（ModelInfo/VoiceInfo 为 kernel-api 契约类型，
  精确 schema 待契约演进补，避免 M1b 契约变更）；见 [architecture](../../../docs/project/architecture.md)。