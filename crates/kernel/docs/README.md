# kernel 文档

> 契约见 [kernel-api](../../kernel-api/docs/contract.md)；架构见 [architecture](../../../docs/project/architecture.md)。

## 当前事实（M1c + M1c-hardening 加固）

内核实现 crate。适配器/教学模块只依赖 kernel-api，本 crate 实现内核服务（依赖倒置）。

- `Runtime`（`src/runtime.rs`）：回合循环。解析绑定 → 读会话 meta.params → 上下文装配（token 预算 + truncate/summarize 淘汰，
  **水位制**：仅处理 seq>evicted_up_to 新历史，滚动摘要前置，淘汰 span 入 outbox 一次）→ 流式 LLM → 发 delta → 内联执行 speak
  → 回喂 → 循环至 stop/守护上限 → 持久化消息（**失败关闭**）+ call_log 计量 → run-end。
  **执行/投递分离**：执行体 spawn tokio task 经有界 mpsc 投递 SessionEvent，SSE 订阅 receiver；断开读 on_disconnect
  （cancel 置标志 / continue 取 detach permit 排空跑完，permit 满降级 cancel）。**对象级授权**：sessions 全路径携带 user_id（审计 C1）。
- 守护：工具迭代上限（每会话 `SessionParams.tool_iteration_cap`，默认 8）、**硬超时**（timeout 包裹执行体，默认 120s，审计 C6）、
  每会话单活跃回合锁（`ActiveGuard` 归执行体）；detach semaphore 默认 4；**上游退避重试**（LLM/TTS 可重试错误 bounded 2，审计 C8）；
  TTS 流响应 cancel；`shutdown` 退出屏障（cancel + 有界等待 detached 落盘）+ `checkpoint`（SQLite WAL，审计 C6）。
- speak 执行（`src/speak.rs` `prepare_speak`）：反序列化 `SpeakArgs` → 解析 TTS 绑定 → 组装 `TtsRequest`
  （model/format=pcm/sample_rate=24000/word_timestamps=true 由 kernel 定）；音频流迭代内联于循环，
  live 发 AudioStart/AudioChunk(b64 PCM)/WordTimestamps/Usage/AudioEnd，complete 入缓存（默认新鲜合成，**owner 归属**）。
- 上下文/计量（`src/context.rs`/`src/journal.rs`）：字符启发式 token 估算 + truncate 装配；summarize 淘汰（Summarizer role + 失败回退）；
  call_log 计量所有上游调用（adapter_id 取 trait id()）；outbox（emit_progress/pending/dispatch_once + OutboxHandler，M1c 无消费者骨架）。
- 错误归一化：`ProviderError` → `SessionEvent::Error`（auth/quota→action:settings、rate_limit/network 可重试等）。

## 内部接缝（非契约，零返工设计）

`src/seam.rs` 三 trait，回合循环依赖之，M1a 内存/env impl → M1b/c DB/加密 impl 不重写循环：

- `BindingResolver`：解析 (user,module,cap,role) → `ResolvedBinding{conn,model,params}`。M1a env impl 在
  godoword（dev-bypass feature）；M1b/c DB 5 级 fallback + 解密。
- `SessionRepo`：create(携带 SessionMeta)/meta/exists/history/history_with_ids/append/list/delete/update_eviction，
  **全路径携带 user_id 对象级授权**（审计 C1）+ append 失败关闭（审计 C4）。`DbSessions` 生产默认（双兼容 SQL，seq 单活跃锁内分配，级联删）；
  `MemSessions` 测试替身。`SessionMeta{module_id,on_disconnect,params,summary,evicted_up_to}`（**C7 淘汰水位**）、
  `OnDisconnect{Cancel,Continue}`、`StoredMessage{id,seq,message}`。
- `AudioCache`：put/get(id,**user**) `CachedAudio{pcm,words,format,sample_rate,owner}`（**get 校验归属**，审计 C1）。
  `FsAudio` 文件系统 `/data/cache/<id>.pcm` + sidecar（原子写，TTL+容量有界）；`MemAudio` 测试替身。**默认新鲜合成**；
  复用由模块**显式白名单**声明稳定 key（M2，取代指纹 hash）+ 静态资产（M2）。见 history 音频白名单条目。

## 存储基底（M1b）

`src/storage.rs`（pool 初始化：SQLite WAL+外键 / PG 外接 + `checkpoint` 退出 WAL 屏障）+ `src/migrate.rs`（自定义双后端迁移 runner：
版本表 + checksum 防漂移 + 迁移前备份）+ `migrations/0001_init.sql`（11 表）+ `0002_auth.sql`（users.username + auth_sessions）
+ `0003_eviction_watermark.sql`（sessions.summary + evicted_up_to，审计 C7）。
schema 列细节、迁移编排与备份策略见 [storage.md](storage.md)。双后端测试 `tests/storage.rs`。

## 加密（M1b）

`src/crypto.rs` `MasterKey`：主密钥静态加密用户 API key（BYOK）。威胁模型防静态泄露（DB 备份/卷快照外流），
不防运行中服务器被攻破（history 0500）。

- 加载：env `GODOWORD_MASTER_KEY`（hex 64 字符，长度/格式错误 fail-fast）覆盖优先；否则读
  `/data/keys/master.key`（raw 32 字节），缺失则 `rand` OsRng 自动生成（unix `create_new` + mode 0600，
  避免世界可读窗口）。幂等：再加载读同一文件。Debug 脱敏。
- AES-256-GCM（`aes-gcm`），**每记录独立随机 nonce**。密文 blob = `nonce(12) + 密文 + tag(16)`。
  解密 GCM 认证失败（密钥错/篡改）→ 错误。providers.key_enc BYTEA 存此 blob。
- 丢卷=密钥永久不可解；备份卷=备份一切含密钥（wiki/deploy 须明示）。信封加密/轮换留远期。
- 测试：`src/crypto.rs` 单元（round-trip/nonce 唯一/篡改/错密钥/hex 解析）+ `tests/crypto.rs`（文件生成 0600/幂等/坏长度）。

## 绑定解析与仓储（M1b）

`src/binding.rs` `DbBindingResolver`（实现 `BindingResolver` 接缝，**M1b 生产路径**，替换 M1a env 旁路）：
5 级 fallback `(user,module,cap,role) → (user,module,cap,default) → (user,*,cap,role) → (user,*,cap,default)
→ 未配置`；module 为 None（M1 无模块）时直接从 `*` 级起；哨兵 `*` 表全模块默认。命中后读 provider 密文
→ `MasterKey` 解密 → `ProviderConn{base_url, api_key:SecretString, extra}` + model + params。注入 `Arc<Store>` + `Arc<MasterKey>`。

`src/repo.rs` providers/bindings 仓储 CRUD（供 REST）：`Provider`/`Binding` 域类型（sqlx::FromRow + Serialize，
**不含 key**——key 经 MasterKey 加密存 key_enc BYTEA，绝不以明文返回）。所有查询按 user_id scoped（多用户隔离）。
**binding 强制 provider 同主**（create/update 入口校验 + get_provider_secret 带 user_id 解密条件，审计 C2）。
create/list/get/update/delete provider（update 可重加密 key）+ binding；delete provider 级联删 bindings（外键 CASCADE）。
capability/role 为 lowercase 字符串。双后端测试 `tests/binding.rs`（5 级 fallback + 解密 + 未配置 + CRUD + 用户隔离 + 级联 + 跨用户 provider 负例）。

## 配置加载（M1b）

`src/config.rs` `Config`（来源轴：env > 配置文件 > 内置默认，history 0530）。配置文件 `/data/config.toml`
（`GODOWORD_CONFIG` 可改路径），TOML + serde。字段：listen（默认 `[::]:8080`）、data_dir（默认 `/data`）、
database_url（设则 PG，否则 SQLite `data_dir/db/godoword.sqlite`）、registration_mode（invite/open/disabled，默认 invite）、
provider_presets（预设：端点+适配器+建议模型，**不含 key**）。env 逐项覆盖：`GODOWORD_LISTEN`/`GODOWORD_DATA_DIR`/
`GODOWORD_DATABASE_URL`/`GODOWORD_REGISTRATION_MODE`。`EnvSource` trait 抽象 env 访问（无全局竞态单元测试）。
`view()` 脱敏只读视图（GET /config）：仅暴露 database_backend 类型，**不暴露连接串凭据**。
用户级配置（providers/bindings）仅 DB，与本轴正交；webui 不回写部署配置。单元测试 5（默认/env 覆盖/坏模式/TOML 预设/env>file）。

## M1 简化与 roadmap 差异

- 无教学模块：用 `DEFAULT_TEACHER_PERSONA`（`src/persona.rs`），无模块 persona/无 plan.teacher_prompt；
  默认教师模式是否作永久产品态留 M2（dev-debt T4）。
- M1b 存储基底/加密/绑定 DB/配置/降权；**M1c 会话/音频持久化（DbSessions/FsAudio，T2 回归）+ 上下文预算/淘汰
  + outbox/call_log + continue detach 已落地**。认证在 auth-local crate + godoword（M1c）。
- 音频默认新鲜合成；白名单复用 + 静态资产 + 角色音色档案实体 M2（M1c 机制无关其形态、零返工）。
- LLM 提取报告（extractor role）归 teach-assess M2；M1c outbox dispatcher 骨架无消费者。
- token 估算字符启发式（roadmap 远期换 tokenizer）。