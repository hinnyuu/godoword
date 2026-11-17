---
date: 2026-11-17T22:06+08:00
keywords: [m1b, storage, migration, encryption, master-key, binding, config, privilege-drop, entrypoint, dual-backend, rest, utoipa, dev-debt]
scope: [project, kernel, godoword]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1b 存储基底 + 部署初始化实现（schema/迁移/加密/绑定/配置/降权/REST）

## 背景

M1a 垂直切片（20efa24）消解最高集成风险后，M1b 铺存储基底 + 部署初始化（蓝图 1720）。
本会话完整探索项目后与主开发者对齐全部 M1b 设计决策（四项，见下），按 6 phase 单 PR 多 commit 推进。
主旨：长期稳定 + 防漂移优先。回归 dev-debt T2 绑定部分（内存→DB）与 T3（root→自适应降权）。

## 决策

**主开发者拍板四项**：
①生产接线/dev-bypass 边界——DbBindingResolver 为唯一生产绑定路径，适配器非 optional（随 dev-bypass 编译至 M1c）；
dev-bypass 保留至 M1c，收窄为固定 dev user + GODOWORD_DEV_* 幂等种入 DB（加密，替代 M1a EnvResolver 直读）；
release M1b 跑完整 entrypoint + healthz/readyz/meta/config/openapi/静态，用户级 REST 由 user_access 门把守（503）至 M1c 认证。
②迁移备份方案 a——SQLite `VACUUM INTO` 自动备份；PG 镜像无 pg_dump 跳过+告警（PG 自备外部备份）；
`_schema_migrations` 版本表 + `include_str!` 内嵌 + checksum 防漂移 + 有序 runner（M2 模块迁移追加）。
③自适应降权算法——读 /proc/self/uid_map 判 rootless/rootful；rootless 保持 uid 0（文件归宿主调用者无 subuid 残留）；
rootful euid=0 stat /data 属主→降到属主或属主 0 时 chown -R 到 1000（PUID/PGID 覆盖）再 setgroups→setgid→setuid；
M1a 升级 chown -R 旧 root 文件；EACCES→SELinux :Z 提示。
④选型——主密钥文件 raw 32 字节 0600 / env hex 64 字符 fail-fast；aes-gcm+rand / nix(user,fs) / toml+serde / utoipa(chrono,uuid)；
M1b 一次性建全部 11 表。**架构姿态：不改 kernel-api 契约**（Store 复用，DTO/config/加密/迁移服务放 kernel 内部 + godoword），
不触发契约级全量审查。

**实现**（kernel）：storage（SQLite WAL+外键 / PG pool + ping）、migrate（自定义双后端 runner）、
crypto（MasterKey + AES-256-GCM 每记录 nonce）、repo（providers/bindings CRUD + provider_conn + ensure_user，
域类型不含 key、按 user_id scoped）、binding（DbBindingResolver 5 级 fallback + 解密）、config（env>file>默认 + 预设 + 脱敏视图）。
（godoword）：init（降权 + 幂等 /data）、main 启动序、settings（provider/binding CRUD + models/voices 测试端点）、
rest（user_access 门 + utoipa 标注）、openapi（ApiDoc + /openapi.json）、dev（env 幂等种入）。冒烟扩完整 uid/卷矩阵 + /data 断言。

**双后端实证教训**（纪律➀捕获）：①PG `INTEGER`=INT4↔Rust i32（非 i64，否则 ColumnDecode）；
②SQLite `VACUUM INTO` 须 `sqlx::raw_sql`（query().execute 静默不执行）+ 字面量文件名（不接受绑定参数）；
③`:memory:` 池连接隔离不适用于 VACUUM/跨连接共享，备份测试用生产 open_sqlite 文件库；
④共享 PG 库 schema 测试须重置 public schema（测试专属后端操作）+ 集中单函数避并行竞态。

**验证**：kernel 双后端测试全绿（迁移幂等/CRUD round-trip/UNIQUE/加密 round-trip/篡改/5 级 fallback/用户隔离/级联/配置）。
本地真 key 全链路：dev 种入加密→DB 绑定解析解密→回合 finish=stop 零 error + speak 出声 + 词级时间戳 + 音频回放（24kHz PCM）
+ 转录 + models 测试端点（真实上游）+ provider/binding CRUD（201/200/204）。entrypoint 本地实证 rootful 降权 uid 1000
+ 幂等重启（0 迁移/主密钥复用 mtime 不变/无新备份）+ master.key 0600 + WAL + 备份生成。key 全程不入日志/仓库/REST 响应。

## 后果

M1b 存储基底 + 部署初始化成立：数据持久化（卷误删重建零丢失）、key 静态加密、自动迁移备份、自适应降权 bind 友好、
BYOK provider/binding 可配（REST，M1d 接 UI）。dev-debt T3 回归（降权上线）；T2 绑定部分回归（DB），余会话/音频 M1c。
T1（dev-bypass）保留至 M1c。会话/音频仍内存（M1c DB/文件系统 impl）。SELinux :Z 人工残留待 Fedora 主机补测。
详见 [kernel/docs/storage.md](../crates/kernel/docs/storage.md)、[architecture](../docs/project/architecture.md)、[roadmap](../docs/project/roadmap.md)。