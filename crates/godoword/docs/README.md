# godoword 组合根

bin crate，以 cargo feature 注册适配器与教学模块到 kernel，启动 axum 服务。

## 当前状态（M1b）

幂等 entrypoint + 存储基底 + DB 绑定解析 + 会话 REST/SSE + 配置只读：

- 启动序（`src/main.rs`）：`Config::load` → `init::init_data_dir`（自适应降权 + 建 /data 子目录）
  → `MasterKey::load`（生成/读主密钥）→ `open_sqlite`/`open_pg` → `run_migrations`（迁移前备份）→ 建状态 → 监听。
- `/healthz`（存活）、`/readyz`（存储探活 SELECT 1）、`/api/v1/meta`（milestone=M1b，dev_bypass 标志）、
  `/api/v1/config`（脱敏只读配置视图）、静态占位页。
- `POST/GET /api/v1/sessions`、`GET/DELETE /api/v1/sessions/{id}`、`POST /sessions/{id}/messages`（→SSE）、
  `POST /sessions/{id}/cancel`、`GET /api/v1/audio/{id}`。
- 状态 `AppState`：MemSessions/MemAudio（内存，M1c 换 DB/文件系统）+ Runtime（仅 dev-bypass）+ 当前用户
  + `Arc<Store>`（迁移已应用）+ `Config`。
- 监听地址取自配置 `listen`（默认双栈 `[::]:8080`，IPv6 不可用回退 `0.0.0.0`）。优雅退出 SIGTERM/Ctrl-C。

## 部署初始化（`src/init.rs`，DEV-DEBT T3 回归）

自适应降权（Rust 进程内，无 shell，distroless 兼容）：读 `/proc/self/uid_map` 判 rootless/rootful。
**rootless 保持 uid 0**（容器 0 映射宿主调用者，文件无 subuid 残留）；**rootful euid=0** 时 stat /data 属主
→ 降到属主 uid/gid，属主 0 则 chown -R /data 到 1000（PUID/PGID 覆盖）再降权（setgroups→setgid→setuid）。
chown 在降权前（含 M1a 旧 root 文件升级）。/data 写 EACCES → SELinux :Z 提示。幂等建 db/keys/cache/backups。
详见 [architecture](../../../docs/project/architecture.md) 与 [wiki/deploy](../../../wiki/deploy/quickstart.md)。

## dev-bypass feature（DEV-DEBT T1）

`--features dev-bypass` 启用 `src/dev.rs` 固定 dev user + 适配器（llm-openai/tts-qwen-audio）+ Runtime
（DbBindingResolver 生产绑定路径）。**release Dockerfile 永不启用**（release M1b 无回合能力，messages 返回 503；
用户级 REST 待 M1c 认证开放）。M1b dev provider/binding 经 env 幂等种入 DB（phase 6）；M1c 真认证后删除本模块。
key 仅经 env，绝不落盘/入日志/入仓库。

## roadmap 差异

- 存储基底/主密钥/迁移/自适应降权 M1b 已落地；持久化会话/音频（DB/文件系统）+ 认证 M1c。
- 适配器随 dev-bypass 编译（optional）；M1c 真认证后转非 optional + 删除 dev-bypass。
- provider/binding CRUD REST + models/voices 测试端点 M1b phase 6；见 [architecture](../../../docs/project/architecture.md)。