---
date: 2026-11-16T23:20+08:00
keywords: [quadlet, ipv6, dual-stack, publishport, bindv6only, fedora, regression]
scope: [project]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "1"
---

# quadlet 双栈发布回归修正（两条 PublishPort）

## 背景

Fedora 44 quadlet 验收：PublishPort=[::]:8080:8080 时 localhost（::1）通但 127.0.0.1 连接被拒绝。
上一版（2250 条目）误以为单条 [::] 即双栈。

## 决策

根因：[::] 在 IPV6_V6ONLY=1 的系统（Fedora/pasta）仅绑 IPv6，宿主无 IPv4 socket → 127.0.0.1 拒绝。
双栈发布标准写法是两条 PublishPort：0.0.0.0:8080:8080（IPv4）+ [::]:8080:8080（IPv6），
两条并存让两族皆可达（同 Docker/podman -p 80:80 -p [::]:80:80 模式）。
godoword.container 改用两条；部署文档纠正"单条 [::] 双栈"的错误说明。

## 后果

quadlet 与 podman run 的双栈宿主绑定须显式两族。app 内部双栈监听（[::]:8080）不变。
见 [deploy/README](../deploy/README.md)。