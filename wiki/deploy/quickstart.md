# 部署快速开始

> 面向部署者。M0 阶段为最小 hello 服务。完整部署形态（自适应降权、卷初始化、quadlet、
> 反代 TLS、备份）随 M1 落地，见 [架构](../../docs/project/architecture.md)。

## 前置

- podman（推荐，rootless 即可）或 docker
- 一个持久卷目录或 named volume

## podman 运行（M0 hello）

```bash
podman run -d --name godoword \
  -p 8080:8080 \
  -v godoword-data:/data:Z \
  ghcr.io/hinnyuu/godoword:latest
```

验证：

```bash
curl http://localhost:8080/healthz    # 期望 {"status":"ok"}
```

## docker 运行

```bash
docker run -d --name godoword -p 8080:8080 -v godoword-data:/data ghcr.io/hinnyuu/godoword:latest
```

或使用 [docker-compose](../../docker/docker-compose.yml)：`docker compose up -d`。

## 说明

- M0 镜像仅含 hello 服务与 `/healthz`，用于验收部署链路。
- 数据持久化、主密钥自动生成、数据库迁移等完整初始化逻辑 M1 落地。
- 生产部署（quadlet systemd 单元、反向代理 + TLS、备份恢复）文档 M1 补全。
- SELinux 主机 bind mount 需 `:Z` 标签；quadlet 需 podman≥4.4。