# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-17T20:20+08:00
- 当前里程碑: **M1a 已合并（20efa24）→ M1b 待启动（交接新 agent）**
- 主线: M0（51d16c3）→ M1 前置 C（bf952a3）→ 前置小 PR（f8ff64c）→ 蓝图（bde1ba8）→ **M1a（20efa24）**

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

- 本交接 docs PR（分支 docs/m1a-accepted-m1b-handoff）：status 更新到 M1a 已合并 + M1b 启动规划。
  合并后新 agent 从 main 接续 M1b。

## blocked / 待拍板决策

- 无当前阻塞。M1 全部设计决策已定案。
- 远期留 M2 拍板：无模块默认教师模式是否作永久"自由聊天"产品态（dev-debt T4）。

## 下一动作（新 agent 接续 M1b）

1. 读 AGENTS.md → 本文件 → roadmap M1 蓝图 → architecture → kernel-api 契约 → kernel/docs（M1a 事实）。
   交叉验证 git log（M1a=20efa24）+ history INDEX（1720 蓝图/1828 M1a/1844 合规）。
2. 建分支 feat/m1b-storage-substrate，按下方 phase 推进，单 PR 多 commit，持续维护本文件。
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

**待定点（实现决策，重大者交用户）**：
- 迁移 runner：sqlx `migrate!` 宏单驱动 pin，双后端需**自定义迁移 runner**（Store 薄分发跑 DDL + 版本表 + 备份）。
- 加密 crate：`aes-gcm` + `rand`（nonce）；主密钥 32 字节编码（env hex/base64 / 文件 raw）待定。
- 降权 crate：`nix`（setuid/setgid/setgroups）+ uid_map 解析；Rust 进程内无 shell（distroless 兼容）。
- 配置 crate：`toml` + serde。
- schema 列细节入 `kernel/docs/storage.md`；key_enc BYTEA；JSON 整取回 Rust；uuid 主键应用生成、时间戳应用侧。
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