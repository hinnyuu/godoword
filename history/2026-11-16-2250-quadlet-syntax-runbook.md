---
date: 2026-11-16T22:50+08:00
keywords: [quadlet, podman, selinux, runbook, dual-stack, acceptance]
scope: [project]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "1"
---

# quadlet 语法修正与人工残留 runbook 补全

## 背景

人工残留验收清单原仅部署命令，缺验证与清理命令。测试机为 Fedora 44 + podman 5.8.7，
支持 podman quadlet install/list/print/rm 便捷命令（podman 5.8+）。

## 决策

修正 deploy/quadlet/godoword.container 严格语法：PublishPort=[::]:8080:8080 双栈绑定
（原 8080:8080 仅 IPv4，localhost IPv6 不可达）；移除 Environment=GODOWORD_LISTEN=0.0.0.0
（强制 IPv4，与双栈修正矛盾，用 app 默认 [::]:8080）；加 ContainerName=godoword、
TimeoutStartSec=300（容纳首次拉镜像）；移除冗余 After=network-online.target（quadlet 自动处理）。
部署文档改用 podman quadlet install（5.8+，自动 reload）+ 4.4–5.7 手动安装回退；
移除用 podman quadlet rm。PR 残留 runbook 补全 SELinux/quadlet/docker 三项的部署+验证+清理命令。

## 后果

quadlet 语法以 podman-systemd.unit(5) 为准。见 [deploy/README](../deploy/README.md)。