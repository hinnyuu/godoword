# godoword 组合根

bin crate，以 cargo feature 注册适配器与教学模块到 kernel，启动 axum 服务。

## 当前状态（M1c）

幂等 entrypoint + 存储基底 + DB 绑定解析 + 会话 REST/SSE + 认证/管理 REST + 配置只读：

- 启动序（`src/main.rs`）：`Config::load` → `init::init_data_dir`（自适应降权 + 建 /data 子目录）
  → `MasterKey::load` → `open_sqlite`/`open_pg` → `run_migrations`（0001+0002，迁移前备份）
  → 适配器 + DbBindingResolver + Runtime（**无条件构建**，release 含完整回合能力）→ 监听。
  后台 task：音频缓存周期清理（TTL/容量）+ outbox dispatcher 骨架（M2 模块消费）。
- `/healthz`、`/readyz`（存储探活）、`/api/v1/meta`（milestone=M1c）、`/api/v1/config`（脱敏）、`/api/v1/openapi.json`、静态占位页。
- **认证**（`src/auth.rs`）：per-request `AuthUser`/`AdminUser` extractor（cookie → `auth_local::validate_session`）；
  cookie HttpOnly/SameSite=Lax/Secure 可配（`GODOWORD_COOKIE_SECURE`）；CSRF 自定义头守卫中间件（`/api/v1` 变更请求须 `X-Requested-With`）；
  登录限流内存（10 次/5 分钟）。`POST /api/v1/auth/{register,login,logout}`、`GET /auth/me`；
  `POST /api/v1/admin/{reset,invites}`、`GET /admin/users`（管理员）。
- 会话：`POST/GET /api/v1/sessions`、`GET/DELETE /sessions/{id}`、`POST /sessions/{id}/messages`（→SSE）、
  `POST /sessions/{id}/cancel`、`GET /api/v1/audio/{id}`（均经 AuthUser）。
- provider/binding CRUD + models/voices 测试端点（`src/settings.rs`，经 AuthUser，key 加密不回显）。
- 状态 `AppState`：DbSessions/FsAudio（trait 对象）+ Runtime + `Arc<Store>` + Config + MasterKey + 适配器 + RateLimiter；
  **用户经 per-request AuthUser**（无固定 user）。

## 部署初始化（`src/init.rs`，DEV-DEBT T3 已回归）

自适应降权（Rust 进程内，distroless 兼容）：读 `/proc/self/uid_map` 判 rootless/rootful。rootless 保持 uid 0
（文件归宿主调用者）；rootful euid=0 以 /data 属主运行（属主 root 则 chown 到 1000，PUID/PGID 覆盖）。
幂等建 db/keys/cache/backups。详见 [architecture](../../../docs/project/architecture.md)、[wiki/deploy](../../../wiki/deploy/quickstart.md)。

## roadmap 差异

- M1c 已落地：会话/音频持久化（DB/文件系统）+ 认证/管理 + 上下文预算/淘汰 + outbox/call_log + continue detach。
  **dev-bypass 已删除（DEV-DEBT T1 回归）**，适配器非 optional，用户级 REST release 开放（未认证 401）。
- 白名单音频复用 + 静态资产 + 角色音色档案实体 M2（音频默认新鲜合成，M1c 机制无关 M2 形态）。
- Svelte 前端 M1d；教学模块 M2+。models/voices OpenAPI 暂通用 object 标注（契约类型精确 schema 待契约演进）。