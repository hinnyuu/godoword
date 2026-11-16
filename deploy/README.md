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

一切持久状态入 `/data`。named volume 由 podman 自动处理属主；bind mount 需 SELinux `:Z`
标签与属主匹配（自适应降权 M1 落地后自动适应宿主 uid）。