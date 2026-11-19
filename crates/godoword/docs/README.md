# godoword 组合根

bin crate，以 cargo feature 注册适配器与教学模块到 kernel，启动 axum 服务。

## 当前状态（M1c + M1c-hardening 加固）

幂等 entrypoint + 存储基底 + DB 绑定解析 + 会话 REST/SSE + 认证/管理 REST + 配置只读：

- 启动序（`src/main.rs`）：`Config::load` → `init::init_data_dir`（自适应降权 + 建 /data 子目录）
  → `MasterKey::load` → `open_sqlite`/`open_pg` → `run_migrations`（0001–0004，迁移前备份 + 每条迁移同事务原子 A2）
  → 适配器 + DbBindingResolver + Runtime（**无条件构建**）→ 监听（`into_make_service_with_connect_info` 供 IP 限流）。
  后台 task：音频缓存周期清理 + outbox dispatcher 骨架。退出屏障：`runtime.shutdown`（cancel + 有界等待 + 超时 abort+join，R3）+ WAL checkpoint（真屏障）。
- `/healthz`、`/readyz`、`/api/v1/meta`（milestone=M1d）、`/api/v1/config`（脱敏）、`/api/v1/openapi.json`、SPA 伺服（web/shell/public，M1d 构建产物）。
- **认证**（`src/auth.rs`）：per-request `AuthUser`/`AdminUser` extractor（cookie → `validate_session` 联查 active）；
  cookie HttpOnly/SameSite=Lax/Secure 可配；CSRF 自定义头守卫（`/api/v1` 变更须 `X-Requested-With`，缺则 403）；
  登录限流内存（username + **IP** 双维度，10 次/5 分钟；**过期窗口清理 + 容量满新键 fail-closed（绝不淘汰活跃封禁，R9.2/N5）**；
  **IP 经可信代理 XFF 最右不可信跳解析，审计 R9.4**，`client_ip`/`parse_trusted`，trusted_proxies 启动解析；
  **reset-password 公开端点 IP 限流防 Argon2 放大，审计 N4**）。
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

## M1d REST 扩展（用户级，均 AuthUser + 对象级归属）

- **偏好**（`src/prefs.rs`）：`GET/PUT /api/v1/preferences`（kernel::Preferences，跨设备随账户，UPSERT）。
- **文件夹**（`src/folders.rs`）：`GET/POST /folders`、`PUT/DELETE /folders/{id}`（虚拟路径目录树；module 查询默认 default；
  FolderError→400/500；改名子树批量改写防环、删非空上移一层绝不删会话）。
- **会话**（`src/rest.rs`）：`GET /sessions?folder=&module=`（元数据列表 SessionListItem：title/created_at/last_active/message_count/path）、
  `PUT /sessions/{id}/path`（移动，自动注册路径链）、`GET /sessions/{id}`（+path，SessionDetailResp）。
- **音频回放高亮出口**：`GET /audio/{id}/meta`（AudioMetaResp：words/format/sample_rate，归属校验，补 P25 回放高亮缺口）。
- **音色试听**（`src/settings.rs`）：`POST /providers/{id}/preview`（PreviewReq{model,voice,text?}→PreviewResp{audio_id}，
  走用户 key 合成+缓存+call_log 计量 C9，缓存写失败关闭 R2）。**上游错误映射**：`upstream_status` Auth/其他→**502**（401 专属用户会话 AuthUser，
  防前端 401 钩子误登出，P8 验收修复）、Quota→402、RateLimit→429。测试连接按 adapter 分流（llm→models / tts→voices）。
- **账户自助**（`src/auth.rs`）：`POST /auth/change-password`（验旧密→撤销其他会话保留当前）、`PUT /auth/me`（显示名）；
  me/admin_users/register/login 响应类型化 UserInfo。
- **OpenAPI schema 补全（决策 B）**：kernel-api 5 类型（ModelInfo/VoiceInfo/ChatMessage/WordTimestamp/AudioFormat）+
  auth-local UserInfo + godoword 新 DTO 皆 ToSchema 注册 ApiDoc；models/voices 响应 Vec<ModelInfo>/Vec<VoiceInfo>；
  $ref 全解析 + 操作级 security 矩阵测试覆盖新端点。前端 openapi-typescript 生成类型单一事实源。

## roadmap 差异

- M1c 已落地：会话/音频持久化（DB/文件系统）+ 认证/管理 + 上下文预算/淘汰 + outbox/call_log + continue detach。
  **dev-bypass 已删除（DEV-DEBT T1 回归）**，适配器非 optional，用户级 REST release 开放（未认证 401）。
- 白名单音频复用 + 静态资产 + 角色音色档案实体 M2（音频默认新鲜合成，M1c 机制无关 M2 形态）。
- Svelte 前端 M1d；教学模块 M2+。models/voices OpenAPI 暂通用 object 标注（契约类型精确 schema 待契约演进）。
  **OpenAPI 完整性（R5/N1）**：CreateSessionReq 注册 + cookieAuth securityScheme + **操作级 security 矩阵**（公开端点 security:[]、受保护 cookieAuth，不用根级全局以免误伤 register/login/healthz，N1）+ 变更端点 CSRF header/403 + 404 标注 + openapi.json 自登记 + `$ref` 全解析与 security 矩阵单元测试，M1d 可生成 TS。