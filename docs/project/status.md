# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-17T21:01+08:00
- 当前里程碑: **M1b 进行中（分支 feat/m1b-storage-substrate，phase 1 schema/迁移）**
- 主线: M0（51d16c3）→ M1 前置 C（bf952a3）→ 前置小 PR（f8ff64c）→ 蓝图（bde1ba8）→ M1a（20efa24）→ 交接（8b5c1a4）→ **M1b（进行中）**

## done

- M0：脚手架 + 四类文档体系 + 关系图谱门禁 + CI（含容器冒烟）+ kernel-api 契约 + hello 服务；
  人工验收全过（Fedora 44 SELinux/quadlet、真实 Debian 13 docker 含 compose）。
- M1 前置（bf952a3）：存储契约 C 方案 + 纪律➀CI 真测双后端（storage required check）+ 纪律➁方言点设计掉；
  serde 黄金测试；axum/sqlx 耦合声明；R4 same-tree+main 重测；契约级全量审查制度化；SQLite CVE 风险接受。
- M1 前置小 PR（f8ff64c）：双后端 workspace 级 + gate 纳入 tests/ + gitleaks 校验和 + agent 交接基础设施。
- M1 拆分蓝图（bde1ba8）：M1a–M1d 四子里程碑，垂直切片先行；speak 落契约（决策 A）；内部接缝；
  bindings 哨兵 `*`；dev-bypass feature；开发态回归机制。详见 [roadmap](roadmap.md#m1-拆分蓝图)。
- **M1a 垂直切片（20efa24，PR #5 已合并，release 重测冒烟 success）**：llm-openai + tts-qwen-audio 适配器全实现
  （fixture 黄金 + 本地真 key 活体）；kernel 最小回合循环（内部接缝 + 内存 impl + 默认 persona + 守护 + cancel）；
  speak 契约入 kernel-api（全量审查矩阵 history 1828）；godoword 会话 REST + SSE + dev-bypass。
  本地真 key 全链路实证：回合 stop 零 error + 2 次 speak 出声 + 词级时间戳 + 回放 + 转录。修 tool_calls 回喂归一化 bug。
- M1a 依赖合规（history 1844）：CDLA-Permissive-2.0 白名单（webpki-roots CA 数据）+ chrono unmaintained
  （RUSTSEC-2026-0543）书面风险接受（用户认可，roadmap 跟踪）。

## in-flight

- M1b（分支 feat/m1b-storage-substrate，单 PR 多 commit）：kernel 存储基底 + 部署初始化。
  本会话已完整探索项目 + 与用户对齐全部 M1b 设计决策（见下「M1b 决策定案」），按 6 phase 推进。
  已完成 phase 1（schema + 双后端迁移 runner + 备份）+ phase 2（主密钥 + AES-256-GCM）
  + phase 3（DbBindingResolver 5 级 fallback + 解密 + providers/bindings 仓储 CRUD）
  + phase 4（配置 env>file>默认 + 预设 + 脱敏视图）+ phase 5（自适应降权 + 幂等 entrypoint，
  本地实证 rootful 降权 uid 1000 + 幂等重启密钥复用 + 迁移前备份 + readyz/config），均测试绿。
  当前 phase 6：provider/binding CRUD REST + models/voices + dev 种入 + 冒烟 uid/卷矩阵。

## blocked / 待拍板决策

- 无当前阻塞。M1b 全部设计决策已与用户定案（见下）。
- 远期留 M2 拍板：无模块默认教师模式是否作永久"自由聊天"产品态（dev-debt T4）。

## M1b 决策定案（2026-11-17 与用户对齐，不重议）

1. **生产接线/dev-bypass 边界**：DbBindingResolver 为唯一生产绑定路径（dev/release 共用）；适配器转非 optional。
   dev-bypass feature 保留至 M1c，职责收窄为固定 dev user + 启动时 GODOWORD_DEV_* 幂等种入 DB（加密），
   替代 M1a EnvResolver 直读 env。release M1b 跑完整 entrypoint + healthz/meta/config 只读/静态；
   用户级 REST（sessions/providers/bindings/回合）由 dev-bypass 把守至 M1c 认证。T1 完整保留到 M1c。
2. **迁移备份（方案 a）**：SQLite `VACUUM INTO '/data/backups/…'` 自动备份；PG 无 pg_dump 跳过+告警日志，
   文档说明 PG 自备外部备份。版本表 `_schema_migrations(version PK,name,checksum,applied_at)`；
   迁移 SQL `include_str!` 编译期内嵌；runner 接受有序 (module,migrations) 供 M2 扩展；checksum 校验防漂移。
3. **自适应降权算法**：读 /proc/self/uid_map 判 rootless（容器 0→宿主非 0）/rootful（0→0）。
   rootless 保持 uid 0（文件归宿主调用者，无 subuid 残留）；rootful euid=0 时 stat /data 属主→降到属主 uid/gid，
   属主 0 则先 chown -R /data 到 1000（PUID/PGID 覆盖）再降权；setgroups([])→setgid→setuid（nix）；
   M1a→M1b 升级 chown -R 旧 root 文件；/data EACCES→SELinux :Z 提示。冒烟实证 uid/卷矩阵。
4. **实现选型**：主密钥文件 raw 32 字节 0600 / env hex 64 字符 fail-fast（rand OsRng 生成）；
   crate aes-gcm+rand（加密）/ nix features user,fs（降权）/ toml+serde（配置）/ utoipa（OpenAPI，axum 0.8 兼容）；
   M1b 一次性建全部 11 表（M1c/M4 逻辑后续填）。
5. **架构姿态**：M1b 不改 kernel-api 契约（Store 已存在直接复用；DTO/config/加密/迁移服务放 kernel 内部 + godoword），
   不触发契约级全量审查，仅常规 docs/wiki/history 同步。kernel 内新增 storage/migrate/crypto/config/binding 模块，
   godoword 新增 init 模块，不新增 crate。SessionRepo/AudioCache DB/文件系统 impl 属 M1c（M1b 不动回合循环）。

## 下一动作（M1b 执行中）

1. 按 6 phase 推进（单 PR 多 commit），每 phase 本机 cargo check/test（jobs≤2）+ gate --local 预检，持续维护本文件。
2. phase 1 schema/迁移 → 2 加密 → 3 绑定 DB → 4 配置 → 5 降权/entrypoint → 6 REST/冒烟 → 7 文档/history。
3. M1b 完成 → CI 绿（含 storage 双后端 + 完整 uid/卷冒烟）→ SELinux 人工残留 sign-off → merge。

## M1b 启动规划（交接棒）

**范围**（蓝图 M1b）：kernel 存储基底 + 部署初始化。回归 dev-debt **T2**（内存生产默认→DB/文件系统）、
**T3**（root 运行→自适应降权）。无 UI → 无人工视觉验收。

**执行 phase**：
1. 完整 kernel schema + 迁移编排（kernel→模块序、迁移前自动备份）+ 双后端测试（backends() 夹具）。
2. 加密：主密钥（env GODOWORD_MASTER_KEY 覆盖 / 自动生成 /data/keys/master.key 0600）+ AES-256-GCM（每记录独立 nonce）。
3. 绑定解析 DB impl（5 级 fallback，哨兵 `*`）+ 解密 → ProviderConn（替换 M1a env 旁路的生产路径）。
4. 配置加载：env > /data/config.toml（GODOWORD_CONFIG）> 默认；provider 预设（不含 key）；registration_mode 等。
5. 自适应降权（stat /data 属主 + /proc/self/uid_map → setgid/setgroups/setuid）+ 幂等 entrypoint（建目录→主密钥→迁移→启动）。
6. provider/binding CRUD REST + 配置只读（脱敏）+ models/voices 测试端点（utoipa）；冒烟扩完整 uid/卷矩阵 + /data 断言；文档/history。

**待定点**：已全部定案，见上「M1b 决策定案」（迁移 runner / 加密 crate / 降权 crate / 配置 crate / 主密钥编码 / schema）。
schema 列细节入 `kernel/docs/storage.md`；key_enc BYTEA；JSON 整取回 Rust；uuid 主键应用生成、时间戳应用侧。
- 完整表：users/local_credentials/oidc_identities/providers/bindings/sessions/messages/call_log/progress_events/tokens/invites。

**验收**：CI 双后端迁移幂等/CRUD/加密/绑定/配置（workspace storage required check）+ 冒烟完整 uid/卷矩阵
（降权后有意义）+ /data/db/master.key/备份断言 + 删容器重建数据存活。**人工残留：SELinux :Z（Fedora 主机）**。

## 开发态临时取舍登记（dev-debt，须回归）

| # | 临时取舍 | 引入 | 回归于 | 回归断言 |
|---|---|---|---|---|
| T1 | env dev key 旁路 + 固定 dev user | M1a | M1c | dev-bypass feature 及 GODOWORD_DEV_* 从非测试代码删除 |
| T2 | 内存 impl 作生产默认 | M1a | **M1b**/M1c | 生产默认换 DB/文件系统；内存 impl 仅留测试 |
| T3 | 容器 root 运行（无降权/初始化） | M1a | **M1b** | 自适应降权 + 幂等 entrypoint 上线 |
| T4 | 无模块默认教师模式 | M1a | M2 决议 | M2 定永久产品态或退役 |

机制：dev-bypass feature release 永不启用 + 代码 `// DEV-DEBT: T<n>` 标记 + 每里程碑验收核对逾期阻塞。
永久架构（接缝 trait + 内存测试替身）非债务。详见 [roadmap](roadmap.md#开发态临时取舍回归机制)。

## 近期关键上下文

- 存储：共享 SQL 守双兼容 SQL 子集；`query!` 仅驱动专属，共享用运行时 query_as + 薄分发宏（execute_both! 等）。
- 契约级变更（trait/Store/协议类型/speak 规格）强制全量 docs+wiki 语义审查 + 核对矩阵。
- kernel 内部接缝（SessionRepo/BindingResolver/AudioCache）已立（M1a），M1b 加 DB impl 替换内存生产默认，回合循环不重写。
- speak 契约（SPEAK_TOOL_NAME/SpeakArgs/speak_tool_spec）已在 kernel-api；kernel 运行时 prepare_speak 执行。
- API 实测事实：LLM OpenAI 兼容 /compatible-mode/v1（reasoning_content/tool_calls 按 index 组装，回喂须 function 形 + arguments 字符串）；
  TTS DashScope 原生 SpeechSynthesizer（X-DashScope-SSE: enable，sentence 事件，base64 PCM，词级时间戳跨帧去重，音色 longanlingxin/longanlufeng）。
- 风险接受：SQLite CVE RUSTSEC-2026-0459 + chrono unmaintained RUSTSEC-2026-0543（均用户认可，roadmap 跟踪，deny.toml 注释）。
- 本机 2H2G：重构建走 CI，本机 cargo check/test jobs≤2；cargo-deny 本机未装（以 CI 为准）；GitHub 网络抖动容忍重试。
- 治理克制：当前最稀缺的是真实 kernel 实现，非更完善治理（外部评审共识）。