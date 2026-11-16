---
date: 2026-11-16T07:00+08:00
keywords: [podman, rootless, uid, volume, init, quadlet, selinux]
scope: [project]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# podman rootless 与自适应降权

## 背景

podman 优先，rootful/rootless 需讨论。用户要求 bind mount 友好、宿主 uid 任意（1001/1002）、
误删容器不丢数据、首次验收人工实测。

## 决策

rootless 优先（无特权需求）。镜像无状态，一切持久状态入 /data（db/keys/cache），
entrypoint 全幂等（建目录→主密钥→迁移→启动），无 marker，容器误删重建零丢失。
自适应降权（Rust 进程内，无 shell/gosu）：stat /data 属主 + 读 /proc/self/uid_map 判定
rootless/rootful → 以属主映射的容器 uid 运行（setgid→setgroups→setuid）。
bind 目录文件永远归宿主用户，无 subuid 残留；用户无需 uid 参数（可选 PUID/PGID 覆盖）。
rootful+root 属主卷 → chown 1000 再降权。SELinux :Z 容器内无法自动修复 → EACCES 明确提示。
端口 8080，/healthz /readyz，优雅退出。生产 quadlet（podman≥4.4）+ 反代 TLS。
验收矩阵：rootless+bind(非1000 uid)/rootless+named/rootful+bind/docker+bind/删容器重建/SELinux。

## 后果

quadlet 需 podman≥4.4（开发机 4.3.1 不支持，须其他主机补测）。
详见后续 audit 与 devenv 条目。见 [architecture](../docs/project/architecture.md)。