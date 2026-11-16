# 部署

> M0 阶段为最小 hello 服务。完整部署形态（自适应降权、卷初始化、主密钥、备份恢复、
> 反向代理 + TLS）随 M1 落地。快速开始见 [wiki/deploy/quickstart](../wiki/deploy/quickstart.md)。

## quadlet（podman ≥ 4.4，rootless 推荐）

`quadlet/godoword.container` 为 systemd 用户单元示例：

```bash
mkdir -p ~/.config/containers/systemd
cp quadlet/godoword.container ~/.config/containers/systemd/
systemctl --user daemon-reload
systemctl --user start godoword.service
```

注意：quadlet 需 podman ≥ 4.4。开发机 podman 4.3.1 不支持，须在生产主机（较新 podman）使用。

## 卷与持久化

一切持久状态入 `/data`。named volume 由 podman 自动创建并处理属主（推荐）。

**bind mount 须先自建源目录**：podman 与 docker 不同，不会自动创建缺失的 bind 源目录
（刻意设计，避免意外建目录与属主错乱），缺失时直接报 `statfs ...: no such file or directory`。
故 bind mount 前先 `mkdir -p` 并保证属主为调用者；SELinux 主机加 `:Z` 标签。
rootless 非登录用户需 `loginctl enable-linger <user>`。自适应降权 M1 落地后自动适应宿主 uid。

## 监听

服务默认双栈监听 `[::]:8080`（兼容 IPv4），`localhost` 的 IPv6/IPv4 皆可达；IPv6 不可用回退
`0.0.0.0`。`GODOWORD_LISTEN` 可覆盖。验证用 `127.0.0.1` 最稳妥。