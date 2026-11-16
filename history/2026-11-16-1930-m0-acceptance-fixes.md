---
date: 2026-11-16T19:30+08:00
keywords: [acceptance, ipv6, dual-stack, bind-mount, podman, runbook, lingering]
scope: [project, godoword]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "1"
---

# M0 人工验收发现与修正

## 背景

开发者人工验收（Fedora 44 + Debian 12）发现两处问题，其余全部通过。

## 决策

问题①（Fedora 44）：`curl http://localhost:8080` 解析到 IPv6 ::1 失败，因 app 仅绑 0.0.0.0（IPv4）。
根治：godoword 默认双栈监听 [::]:8080（Linux 默认 IPV6_V6ONLY=0 兼容 IPv4），IPv6 不可用回退
0.0.0.0；显式 GODOWORD_LISTEN 优先。文档/runbook 同时改用 127.0.0.1 最稳妥。

问题②（Debian 12）：podman bind mount 不自动创建缺失源目录（与 docker 不同，刻意设计），
报 statfs no such file。runbook 补 mkdir + chown 前置；部署文档明确"podman bind 源目录须自建"。
rootless 非登录用户需 loginctl enable-linger 消除 cgroup 告警。named volume 无此问题（自动创建处理属主）。

## 后果

部署文档（wiki/deploy、deploy/README）与 PR runbook 修订；godoword 监听逻辑变更需重新构建镜像再验收。
Fedora 44 可用于补测本机缺的 SELinux/quadlet 项。见 [quickstart](../wiki/deploy/quickstart.md)。