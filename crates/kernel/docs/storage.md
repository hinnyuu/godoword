# kernel 存储

> 存储分发契约（Store enum + 薄分发宏）见 [kernel-api contract](../../kernel-api/docs/contract.md)；
> 双兼容 SQL 子集纪律见 [conventions](../../../docs/project/conventions.md)。本文记 kernel 存储基底的当前事实（M1b）。

## pool 初始化（`src/storage.rs`）

- `open_sqlite(db_path)`：SQLite 文件库，**WAL 日志模式 + 外键约束启用**（`foreign_keys(true)`），
  文件缺失自动建（`create_if_missing`），父目录自动创建，池上限 5 连接。默认路径 `/data/db/godoword.sqlite`。
- `open_pg(url)`：PostgreSQL 外接连接串，`PgPool::connect`。
- 两者皆包进 `kernel_api::Store` 供薄分发宏（`execute_both!` 等）跑双兼容子集。

## 迁移编排（`src/migrate.rs`）

sqlx `migrate!` 宏单驱动 pin，无法服务 Store 双驱动，故**自定义 runner**：

- 迁移 SQL 编译期 `include_str!` 内嵌（`migrations/0001_init.sql`），单二进制、distroless 友好。
  `KERNEL_MIGRATIONS` 为有序 `Migration{version,name,sql}` 列表；M2 教学模块迁移按注册序追加于其后。
- 版本表 `_schema_migrations(version TEXT PK, name, checksum, applied_at TIMESTAMPTZ)`。
- 流程：建版本表 → 读已应用 → **checksum 校验**（已应用迁移 SQL 被改 → 报错拒绝，防漂移；
  演进须新增迁移而非改旧）→ 有待应用则**先备份** → **每条迁移同事务原子**（全部语句 + 版本记录一起提交，
  中途失败整体回滚，防多语句半应用，审计 A2）→ 重跑无待应用则零操作（幂等）。
- `run_migrations(store, migrations, backup_dir: Option<&Path>) -> 本次应用数`。

### 迁移前备份（后端真方言点，收口备份 helper）

- **SQLite**：`VACUUM INTO '/data/backups/godoword-<时间戳>.sqlite'`——WAL 下一致快照，单文件。
  VACUUM 属管理语句，须 `sqlx::raw_sql`（exec 式）而非 `query().execute()`；文件名须字面量
  （不接受绑定参数），路径完全受控（时间戳目录）单引号转义后内联。
- **PostgreSQL**：运行时镜像（debian-slim）无 pg_dump，**跳过 + 告警日志**，PG 部署自备外部备份
  （见 [wiki/deploy](../../../wiki/deploy/quickstart.md)）。

## schema（`migrations/0001_init.sql` + `0002_auth.sql` + `0003_eviction_watermark.sql` + `0004_summary_spans.sql` + `0005_preferences_folders.sql`）

双兼容子集：uuid 主键应用生成、时间戳应用侧（`Utc::now()`）、JSON 整取回 Rust、`$1` 占位、
单一 PG 风格类型名（UUID/TIMESTAMPTZ/JSONB/BOOLEAN/BYTEA/TEXT/INTEGER）靠 SQLite 类型亲和性双后端通用。
外键 `REFERENCES … ON DELETE CASCADE`（SQLite 须 pool 启用 foreign_keys，PG 默认强制）。
0001（M1b）建 11 表；0002（M1c）增 users.username + auth_sessions；0003（M1c-hardening C7）增 sessions.summary + evicted_up_to；
0004（二审 R4）建 summary_spans（summarize 计费幂等）；0005（M1d）建 user_preferences + session_folders + sessions.path。M4 oidc 前瞻。

| 表 | 列 | 约束/说明 |
|---|---|---|
| users | id UUID PK, email TEXT?, display_name TEXT?, **username TEXT?（0002）**, is_admin BOOL, status TEXT, created_at/updated_at TS | username 本地登录标识（UNIQUE 索引，可空 OIDC 无）；email 可空前瞻 M4；status active/disabled |
| local_credentials | user_id UUID PK→users, password_hash TEXT, created_at/updated_at TS | M1c argon2；一用户一凭据 |
| oidc_identities | id UUID PK, user_id→users, issuer TEXT, subject TEXT, created_at TS | M4 前瞻；UNIQUE(issuer,subject) |
| providers | id UUID PK, user_id→users, adapter_id TEXT, label TEXT, base_url TEXT, **key_enc BYTEA**, extra_json JSONB?, created_at/updated_at TS | 一实例一 key；key_enc=nonce+密文+tag |
| bindings | id UUID PK, user_id→users, **module_id TEXT（哨兵 `*`）**, capability TEXT, role TEXT, provider_id→providers, model TEXT, params_json JSONB, created_at/updated_at TS | UNIQUE(user_id,module_id,capability,role)；`*`=全模块默认 |
| sessions | id UUID PK, user_id→users, module_id TEXT?, plan_version TEXT?, on_disconnect TEXT, params_json JSONB?, **summary TEXT?（0003）**, **evicted_up_to INTEGER?（0003）**, **path TEXT?（0005）**, created_at/updated_at TS | module_id/plan_version 可空；on_disconnect cancel/continue；**summary 滚动摘要 + evicted_up_to 淘汰水位（C7）**；**path 虚拟路径文件夹链（M1d，NULL=未分类）** |
| messages | id UUID PK, session_id→sessions, seq INTEGER, role TEXT, content TEXT, reasoning TEXT?, tool_calls JSONB?, tool_call_id TEXT?, audio_refs JSONB?, created_at TS | UNIQUE(session_id,seq)；JSON 整取 |
| call_log | id UUID PK, user_id→users, session_id UUID?, run_id UUID?, capability TEXT, adapter_id TEXT, model TEXT, prompt/completion/reasoning/total_tokens INTEGER?, characters INTEGER?, latency_ms INTEGER?, status TEXT, error_code TEXT?, created_at TS | M1c 计量落地 |
| progress_events | id UUID PK, user_id→users, session_id UUID?, module_id TEXT, kind TEXT, skill_tags JSONB?, detail JSONB?, created_at TS, processed_at TS? | outbox；processed_at 空=未处理 |
| tokens | id UUID PK, user_id UUID?→users, purpose TEXT, token_hash TEXT, expires_at TS, used_at TS?, created_at TS | 单次（used_at）；**M1c-hardening C8 密码重置 purpose=password_reset** |
| invites | id UUID PK, code_hash TEXT, max_uses INTEGER, used_count INTEGER, created_by UUID?→users(SET NULL), expires_at TS?, created_at TS | 邀请码注册 |
| auth_sessions | id UUID PK, user_id→users CASCADE, token_hash TEXT, expires_at TS, created_at TS | **0002（M1c）** cookie 会话令牌（opaque token sha256，可撤销），与单次 tokens 分离 |
| summary_spans | span_hash TEXT PK, session_id→sessions CASCADE, fragment TEXT, created_at TS | **0004（二审 R4）** summarize 计费幂等：span 消息 ID 集哈希键，命中复用 fragment 免重复 LLM 计费 |
| user_preferences | user_id UUID PK→users CASCADE, prefs_json JSONB, updated_at TS | **0005（M1d）** 用户偏好跨设备随账户（theme/auto_play/on_disconnect_default/密度/字号/custom_css/theme_vars）；UPSERT last-write-wins |
| session_folders | id UUID PK, user_id→users CASCADE, module_id TEXT, path TEXT, created_at/updated_at TS | **0005（M1d）** 虚拟路径文件夹登记表（持久空文件夹）；UNIQUE(user_id,module_id,path)（path NOT NULL 避 NULL 陷阱）；module_id 哨兵 `default`=无模块命名空间（chroot） |

索引：providers(user_id)、bindings(user_id)、sessions(user_id)、call_log(user_id,created_at)、
progress_events(user_id,processed_at)、tokens(token_hash)、invites(code_hash)、users(username) UNIQUE、
auth_sessions(token_hash)、auth_sessions(user_id)、summary_spans(session_id)、session_folders(user_id)。

**会话虚拟路径（M1d，`src/path.rs`）**：路径即文件夹身份（`/日语/五十音`），前端 trie 渲染。
服务端权威 **NFC 归一化**（`unicode-normalization`，日语浊音 `が` 单码位 vs `か`+组合符 NFD 同形异码→裂文件夹陷阱）
+ 校验（禁 `/`/控制字符/空段/`.`/`..`/段首尾空白）；**上限固定常量**（深度≤8/段≤64 字符/全路径≤1024，**不进 config**，
校验仅写入时已存数据永不回溯失效）。前缀匹配/子树改写在 Rust 侧（`is_within`/`rewrite_prefix`/`parent`，不用 SQL LIKE 免转义）。
文件夹改名/移动=子树（文件夹+会话）前缀批量改写单事务原子（防环：目标不可在源子树内；**冲突预检：改写后路径不得与子树外既有文件夹冲突→Conflict；改写后深度/全路径上限重校验→Invalid**，审计 B/D）；删非空文件夹=子树上移一层
（`parent`，根级→NULL，**绝不级联删会话**；**上移冲突预检→Conflict 400，与改名对称；目标先删释放路径，子文件夹可上移复用目标路径**，审计 F1）；移动会话自动注册目标路径链（`chain` 前缀集 ON CONFLICT DO NOTHING；**UPDATE rows 校验防并发删除假成功**，审计 O8）。
**祖先链登记与改写同事务**（create/rename/delete/move 皆然，审计 A：上移/改名目标祖先必在登记表，前端树=登记表∪会话路径前缀不隐没会话）。
模块 chroot：module_id 隐式根命名空间（哨兵 `default`↔sessions.module_id NULL），创建时固定杜绝跨模块移动。

**bindings 哨兵 `*`**：取代 NULL 表全模块默认——两后端 NULL 皆互不相等致 UNIQUE 失效，
哨兵使 UNIQUE(user,module,cap,role) 正常约束、5 级 fallback 解析一致（见 [architecture](../../../docs/project/architecture.md)）。

**progress_events kernel 哨兵**（M1c）：kernel 生命周期事件（SessionEvicted/SessionEnded）的 module_id NOT NULL，
而 M1 默认教师无模块（sessions.module_id 可空）→ 填 session.module_id 有则用之、无则哨兵 `kernel`
（区别 bindings 的 `*` 全模块默认语义，表内核生命周期事件非模块归属）。见 history M1c 设计决策条目。

**双兼容类型映射**（双后端测试实证）：`INTEGER` ↔ Rust **i32**（PG INT4；SQLite INTEGER 亲和性可解码 i32，
勿用 i64——PG 报 INT4/INT8 不匹配）；`COUNT(*)` ↔ i64（PG BIGINT）；`DOUBLE PRECISION` ↔ f64；
`BOOLEAN` ↔ bool；`UUID` ↔ uuid::Uuid；`TIMESTAMPTZ` ↔ chrono::DateTime<Utc>（跨后端精度差毫秒级比较）；
`JSONB` ↔ serde_json::Value（整取）；`BYTEA` ↔ Vec<u8>；可空列 ↔ Option<T>。

## 双后端测试（`tests/storage.rs`，纪律➀）

`backends()` 夹具（SQLite 恒跑 + `TEST_PG_URL` 驱动 PG，未设显式 skip）。schema 表名固定，
故 CRUD 集中于单测试函数内顺序遍历两后端避并行竞态：

- `migrations_and_schema_crud`：迁移幂等（首次 5/重跑 0）+ 版本表 + users/providers/bindings/sessions/messages
  CRUD round-trip（uuid/bool/json/bytea/可空/timestamp）+ bindings 哨兵 `*` UNIQUE 拒绝重复。
- `sqlite_backup_creates_file`：生产路径 `open_sqlite`（文件库）触发 VACUUM INTO 备份，断言备份文件含版本表。
- `checksum_drift_rejected`：已应用迁移改 SQL → 拒绝（runner 逻辑，独立内存库）。