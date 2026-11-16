# 部署

> M0 阶段为最小 hello 服务。完整部署形态（自适应降权、卷初始化、主密钥、备份恢复、
> 反向代理 + TLS）随 M1 落地。快速开始见 [wiki/deploy/quickstart](../wiki/deploy/quickstart.md)。

## quadlet（rootless 推荐）

`quadlet/godoword.container` 为 systemd 用户单元。语法要点：**双栈发布用单条不带 IP 的
`PublishPort=8080:8080`**——pasta 默认同时绑 IPv4 与 IPv6（Fedora 44 / podman 5.8.7 实测
`127.0.0.1` 与 `localhost` 皆可达）。两个陷阱：显式 `PublishPort=[::]:8080:8080` 在
`IPV6_V6ONLY=1` 的系统仅 IPv6（`127.0.0.1` 被拒绝）；两条同端口 PublishPort（IPv4+IPv6）
会致 pasta 端口转发冲突、服务启动失败。`Volume=godoword-data:/data:Z`（SELinux relabel）；
`TimeoutStartSec=300` 容纳首次镜像拉取；`[Install] WantedBy=default.target` 开机自启。

**podman ≥ 5.8** 用便捷命令安装（自动 reload systemd）：

```bash
podman quadlet install quadlet/godoword.container
systemctl --user start godoword.service
podman quadlet list                 # 查看已安装
```

更新用 `podman quadlet install --replace quadlet/godoword.container`；移除用 `podman quadlet rm godoword.container`。

**podman 4.4–5.7**（无 quadlet 子命令）手动安装：

```bash
mkdir -p ~/.config/containers/systemd
cp quadlet/godoword.container ~/.config/containers/systemd/
systemctl --user daemon-reload
systemctl --user start godoword.service
```

注意：quadlet 需 podman ≥ 4.4 与 cgroup v2。开发机 podman 4.3.1 不支持，须在生产主机使用。
语法详见 podman-systemd.unit(5)；调试用 `systemd-analyze --user --generators=true verify godoword.service`。

## 卷与持久化

一切持久状态入 `/data`。named volume 由 podman 自动创建并处理属主（推荐）。

**bind mount 须先自建源目录**：podman 与 docker 不同，不会自动创建缺失的 bind 源目录
（刻意设计，避免意外建目录与属主错乱），缺失时直接报 `statfs ...: no such file or directory`。
故 bind mount 前先 `mkdir -p` 并保证属主为调用者；SELinux 主机加 `:Z` 标签。
rootless 非登录用户需 `loginctl enable-linger <user>`。自适应降权 M1 落地后自动适应宿主 uid。

## 监听

服务默认双栈监听 `[::]:8080`（兼容 IPv4），`localhost` 的 IPv6/IPv4 皆可达；IPv6 不可用回退
`0.0.0.0`。`GODOWORD_LISTEN` 可覆盖。验证用 `127.0.0.1` 最稳妥。