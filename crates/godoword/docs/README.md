# godoword 组合根

bin crate，以 cargo feature 注册适配器与教学模块到 kernel，启动 axum 服务。

## 当前状态（M1c + M1c-hardening 加固）

幂等 entrypoint + 存储基底 + DB 绑定解析 + 会话 REST/SSE + 认证/管理 REST + 配置只读：

- 启动序（`src/main.rs`）：`Config::load` → `init::init_data_dir`（自适应降权 + 建 /data 子目录）
  → `MasterKey::load` → `open_sqlite`/`open_pg` → `run_migrations`（0001–0004，迁移前备份 + 每条迁移同事务原子 A2）
  → 适配器 + DbBindingResolver + Runtime（**无条件构建**）→ 监听（`into_make_service_with_connect_info` 供 IP 限流）。
  后台 task：音频缓存周期清理 + outbox dispatcher 骨架。退出屏障：`runtime.shutdown`（cancel + 有界等待 + 超时 abort+join，R3）+ WAL checkpoint（真屏障）。
- `/healthz`、`/readyz`、`/api/v1/meta`（milestone=M1c）、`/api/v1/config`（脱敏）、`/api/v1/openapi.json`、静态占位页。
- **认证**（`src/auth.rs`）：per-request `AuthUser`/`AdminUser` extractor（cookie → `validate_session` 联查 active）；
  cookie HttpOnly/SameSite=Lax/Secure 可配；CSRF 自定义头守卫（`/api/v1` 变更须 `X-Requested-With`，缺则 403）；
  登录限流内存（username + **IP** 双维度，10 次/5 分钟；**过期窗口清理 + 容量上限，审计 R9.2**；
  **IP 经可信代理 XFF 最右不可信跳解析，审计 R9.4**，`client_ip`/`parse_trusted`，trusted_proxies 启动解析）。
  `POST /auth/{register,login,logout,reset-password}`、`GET /auth/me`；
  `POST /admin/{reset,invites}`、`GET /admin/users`、`POST /admin/users/{id}/status`（**防自禁/防禁最后管理员 +
  并发互禁不失锁，R7 串行化守卫收口 auth-local `set_user_status_guarded`**）。
  **重置 token 化（C8）+ 事务原子单次（R1）**：admin/reset 生成令牌 → 用户 POST auth/reset-password 持令牌设密码
  （密码策略经 auth-local `validate_password` 统一，弱密码 → 400；重置条件认领事务，并发仅一次成功）。
- 会话：`POST/GET /sessions`（**create 接受 on_disconnect/params，C5**）、`GET/DELETE /sessions/{id}`（DELETE SessionEnded outbox + 删除同事务原子，R6）、
  `POST /sessions/{id}/messages`（→SSE）、`POST /sessions/{id}/cancel`、`GET /audio/{id}`（均经 AuthUser + **对象级归属校验，越权 404，C1**）。
- provider/binding CRUD + models/voices 测试端点（`src/settings.rs`，经 AuthUser，**binding 强制 provider 同主 C2**，key 加密不回显，**测试端点计量 C9**）。
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
  **OpenAPI 完整性（R5/N1）**：CreateSessionReq 注册 + cookieAuth securityScheme + **操作级 security 矩阵**（公开端点 security:[]、受保护 cookieAuth，不用根级全局以免误伤 register/login/healthz，N1）+ 变更端点 CSRF header/403 + 404 标注 + openapi.json 自登记 + `$ref` 全解析与 security 矩阵单元测试，M1d 可生成 TS。