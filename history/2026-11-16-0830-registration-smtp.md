---
date: 2026-11-16T08:30+08:00
keywords: [registration, invite, smtp, reset, oidc, milestone-m4]
scope: [project, auth-local]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 注册邀请与 SMTP 重置（并入 M4）

## 背景

注册模式默认值与密码重置路径需定。用户主张 invite 默认（安全第一），SMTP 自助重置
（QQ/163 免费 SMTP 门槛低），并提议与公网 authentik 合并。

## 决策

registration_mode = invite(默认)|open|disabled，部署配置；首用户例外成管理员。
invites 表（code_hash/created_by/expires_at/max_uses/used_count），管理员 UI 生成。
SMTP 自助重置与 auth-oidc 合并为 M4"公网身份升级"（共享 public_url/HTTPS/反代，
邮箱验证↔OIDC 关联交互，外部前置一次准备）。M4 内部 SMTP 先行→OIDC 随后，独立配置门控。
重置安全：无用户枚举、token 哈希单次~30min、账号+IP 限流、重置失效全部会话、邮箱须先验证。
[smtp] 配置 host/port/security/username/password(授权码)/from + public_url。
管理员重置永久备选（兜底云主机封 SMTP 端口）。schema 前瞻（users.email/tokens）M1 落地。
OIDC 会话 v1 独立于 IdP，不做 RP-initiated logout。

## 后果

开发机实测 smtp.qq.com/smtp.163.com 465/587 全通。版本 v1.0=M3，v1.1=M4。
见 [roadmap](../docs/project/roadmap.md)。