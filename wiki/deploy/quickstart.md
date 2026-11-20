# 部署快速开始

> 面向部署者。M1d 镜像含完整初始化（自适应降权、卷初始化、主密钥、自动迁移备份）+ 会话 REST/SSE
> + provider/binding/config REST + 本地账户认证（auth-local）+ **Svelte 图形界面**（浏览器访问即用）。
> 用户级 REST 经 cookie 会话认证（未认证 401）。架构见 [架构](../../docs/project/architecture.md)。

## 前置

- podman（推荐，rootless 即可）或 docker
- 一个持久卷：named volume（推荐，省心）或 bind mount 目录

## podman 运行（named volume）

```bash
podman run -d --name godoword \
  -p 8080:8080 \
  -v godoword-data:/data:Z \
  ghcr.io/hinnyuu/godoword:latest
```

验证（用 `127.0.0.1` 最稳妥）：

```bash
curl http://127.0.0.1:8080/healthz    # 期望 {"status":"ok"}
curl http://127.0.0.1:8080/api/v1/meta
```

服务默认双栈监听 `[::]:8080`（兼容 IPv4），`localhost` 的 IPv6/IPv4 皆可达；
IPv6 不可用的主机自动回退 `0.0.0.0`。若你的环境 `localhost` 解析异常，直接用 `127.0.0.1`。

## bind mount（须先自建源目录）

**podman 与 docker 不同：podman 不会自动创建缺失的 bind 源目录**（docker 会以 root 自动建），
而是直接报错 `statfs ...: no such file or directory`。这是 podman 的刻意设计，避免意外建目录与属主错乱。
因此 bind mount 前必须自建目录并保证属主正确：

```bash
mkdir -p ~/godoword-data
# rootless 下目录属主须为调用者自己；SELinux 主机加 :Z 标签
podman run -d --name godoword -p 8080:8080 \
  -v ~/godoword-data:/data:Z \
  ghcr.io/hinnyuu/godoword:latest
curl http://127.0.0.1:8080/healthz
```

named volume 无此问题（podman 自动创建并处理属主），故推荐新手用 named volume。

### rootless 非登录用户（如测试用户）

以 `sudo -u <user> -i` 运行 rootless podman 时，若无 systemd 用户会话会告警并回退 cgroupfs。
启用 lingering 可消除告警并获得完整 systemd 管理：

```bash
sudo loginctl enable-linger <user>
sudo -u <user> -i mkdir -p /home/<user>/godoword-data   # 先建目录
sudo -u <user> -i podman run -d --name godoword -p 8080:8080 \
  -v /home/<user>/godoword-data:/data:Z ghcr.io/hinnyuu/godoword:latest
```

## docker 运行

```bash
docker run -d --name godoword -p 8080:8080 -v godoword-data:/data ghcr.io/hinnyuu/godoword:latest
```

或使用 [docker-compose](../../docker/docker-compose.yml)：`docker compose up -d`。
docker 会自动创建缺失的 bind 源目录（与 podman 不同）。

## quadlet（生产推荐，podman ≥ 4.4）

以 systemd 用户单元管理，开机自启。podman ≥ 5.8 用便捷命令：

```bash
podman quadlet install deploy/quadlet/godoword.container
systemctl --user start godoword.service
curl http://127.0.0.1:8080/healthz
```

移除：`systemctl --user stop godoword.service && podman quadlet rm godoword.container`。
quadlet 双栈发布用单条不带 IP 的 `PublishPort=8080:8080`（pasta 默认双栈）；显式 `[::]` 仅 IPv6，
两条同端口会冲突。详见 [deploy/README](../../deploy/README.md)。

## 数据持久化与主密钥（M1b）

一切持久状态入 `/data`，镜像无状态，容器误删重建零丢失：

```
/data
├── db/godoword.sqlite        # 默认 SQLite（WAL）；外接 PostgreSQL 时不用此文件
├── keys/master.key           # 主密钥（raw 32 字节，0600），加密用户 API key
├── cache/                    # 音频缓存（可再生可丢弃）
└── backups/                  # 迁移前自动备份（SQLite VACUUM INTO 一致快照）
```

- **主密钥**：首次启动自动生成 `/data/keys/master.key`（0600）；可用 env `GODOWORD_MASTER_KEY`
  （64 位 hex = 32 字节）覆盖。**备份卷=备份一切含密钥；丢卷=用户 key 永久不可解**（请妥善备份 /data）。
- **自适应降权**：rootless 保持容器 uid 0（文件归宿主调用者，无 subuid 残留）；rootful 以 /data 属主运行
  （属主 root 时 chown 到 1000，`PUID`/`PGID` 可覆盖）。bind 目录文件永远归宿主用户。
- **迁移备份**：每次有待应用迁移前，SQLite 自动 `VACUUM INTO /data/backups/`；PostgreSQL 镜像无 pg_dump，
  跳过自动备份（请自备外部备份，如 `pg_dump` 定时任务）。

### 备份与恢复

- SQLite：备份 `/data` 整卷即可（含 db + keys）。恢复=还原卷后重启容器（主密钥须与加密数据匹配）。
  迁移前快照在 `/data/backups/godoword-<时间戳>.sqlite`，可用于回滚到迁移前状态。
- PostgreSQL：用外部 `pg_dump`/`pg_restore` 备份数据库；主密钥仍在 `/data/keys`（或 env），须一并保管。

## 配置（M1b）

来源轴 env > 配置文件 > 默认。配置文件 `/data/config.toml`（`GODOWORD_CONFIG` 可改路径）：

```toml
listen = "[::]:8080"
registration_mode = "invite"   # invite/open/disabled
# database_url = "postgres://user:pass@host:5432/godoword"  # 设则外接 PG

[[provider_presets]]            # 设置页一键模板（不含 key，key 仍用户自填）
id = "qwen-llm"
adapter_id = "llm-openai"
label = "Qwen LLM"
base_url = "https://token-plan.maas.qianwenaiapi.com/compatible-mode/v1"
suggested_models = ["qwen4-max-preview"]
```

env 覆盖：`GODOWORD_LISTEN`/`GODOWORD_DATA_DIR`/`GODOWORD_DATABASE_URL`/`GODOWORD_REGISTRATION_MODE`/
`GODOWORD_MASTER_KEY`/`GODOWORD_COOKIE_SECURE`/`GODOWORD_TRUSTED_PROXIES`。`GET /api/v1/config` 只读展示生效值（脱敏，不含连接串凭据）。file/env 变更需重启。

## 说明

- M1d 镜像含健康/就绪检查、会话 REST/SSE、provider/binding/config REST、本地账户认证（auth/admin REST）、用户偏好/会话文件夹/音色试听/自助改密、OpenAPI（`/api/v1/openapi.json`）与 **Svelte 图形界面**（Dockerfile node 阶段构建，浏览器访问即用；认证/设置/对话/管理）。
- 回合对话与用户级 REST 经 cookie 会话认证（未认证返回 401）；注册首个用户自动成管理员，邀请码注册后续用户；配置界面 M1d。
  本地/HTTP 测试设 `GODOWORD_COOKIE_SECURE=false`（生产反代 TLS 下保持默认 true）。
- 生产部署 quadlet systemd 单元 + 反向代理 TLS 见 [deploy/README](../../deploy/README.md)。
- **反向代理后的登录限流**：默认按直连对端 IP 限流；经反代时所有用户共享反代 IP 会退化为全局限流。
  设 `GODOWORD_TRUSTED_PROXIES`（逗号分隔 IP/CIDR，如 `10.0.0.1,10.0.0.0/8`）声明可信代理，
  服务从 `X-Forwarded-For` 取最右不可信跳作为客户端 IP。仅可信代理的 XFF 被采信（防伪造）；反代须正确设置 XFF。
- SELinux 主机 bind mount 需 `:Z` 标签（容器内无法自动修复，权限拒绝时日志会提示）；quadlet 需 podman≥4.4。