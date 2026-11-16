---
date: 2026-11-16T06:00+08:00
keywords: [auth, oidc, authentik, user-id, transition, local]
scope: [project, auth-local]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 认证平滑过渡

## 背景

初期本地认证，后续支持公网 authentik，需平滑过渡不迁移业务数据。

## 决策

所有业务数据挂稳定内部 user_id，与认证方式解耦。users / local_credentials(argon2) /
oidc_identities(issuer,subject) 分表。AuthProvider trait 只管验身返回 user_id，
kernel 会话层（DB sessions + cookie）共用。local 与 OIDC 可并存。关联流程：已登录本地用户
设置页发起 OIDC 绑定（默认手动；verified email 自动关联为 opt-in，需 email_verified=true）。
切换认证零业务数据迁移。注册默认 invite，首用户例外成为管理员（事务锁防竞态）。
管理员不可读用户 key。本地密码重置早期管理员重置，M4 起 SMTP 自助为主、管理员永久备选。

## 后果

AuthProvider 统一 enum 形状（AuthRequest/AuthOutcome）。OIDC 细节 M4。
见 [architecture](../docs/project/architecture.md)。