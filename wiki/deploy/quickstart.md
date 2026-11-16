# 部署快速开始

> 面向部署者。M0 阶段为最小 hello 服务。完整部署形态（自适应降权、卷初始化、quadlet、
> 反代 TLS、备份）随 M1 落地，见 [架构](../../docs/project/architecture.md)。

## 前置

- podman（推荐，rootless 即可）或 docker
- 一个持久卷：named volume（推荐，省心）或 bind mount 目录

## podman 运行（M0 hello，named volume）

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
quadlet 双栈发布用两条 PublishPort（IPv4 `0.0.0.0:8080:8080` + IPv6 `[::]:8080:8080`），
单条 `[::]` 在 Fedora 等 v6only 系统仅 IPv6。详见 [deploy/README](../../deploy/README.md)。

## 说明

- M0 镜像仅含 hello 服务与 `/healthz`，用于验收部署链路。
- 数据持久化、主密钥自动生成、数据库迁移等完整初始化逻辑 M1 落地。
- 生产部署（quadlet systemd 单元、反向代理 + TLS、备份恢复）文档 M1 补全。
- SELinux 主机 bind mount 需 `:Z` 标签；quadlet 需 podman≥4.4。