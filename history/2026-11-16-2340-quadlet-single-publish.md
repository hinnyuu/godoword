---
date: 2026-11-16T23:40+08:00
keywords: [quadlet, publishport, pasta, dual-stack, fedora, empirical]
scope: [project]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "1"
---

# quadlet 双栈发布定论：单条不带 IP（实测）

## 背景

两条 PublishPort（0.0.0.0:8080:8080 + [::]:8080:8080）在 Fedora 44 quadlet 启动失败
（restart counter 5，start request repeated too quickly）。需实测定论而非推断。

## 决策

Fedora 44 / podman 5.8.7 实测：单条 `podman run -p 8080:8080`（不带 IP）→ 127.0.0.1 与
localhost 皆返回 {"status":"ok"}。pasta 默认对不带 IP 的发布端口同时绑 IPv4+IPv6。
定论：quadlet 用单条 `PublishPort=8080:8080`。两条同端口 PublishPort 致 pasta 转发冲突启动失败；
显式 [::] 在 v6only 系统仅 IPv6。godoword.container 与部署文档据此修正。

## 后果

宿主双栈绑定靠 pasta 默认（不带 IP），非显式两族。app 内部双栈监听不变。
smoke 脚本 -p 不带 IP 写法与此一致。见 [deploy/README](../deploy/README.md)。