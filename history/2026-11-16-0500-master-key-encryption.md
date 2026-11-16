---
date: 2026-11-16T05:00+08:00
keywords: [master-key, encryption, byok, security, oidc]
scope: [project, kernel]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 主密钥静态加密（反对凭据派生）

## 背景

用户质疑 master key 必要性，提议用"用户名+密码"或 authentik 凭据替代。

## 决策

保留主密钥。认证凭据回答"你是谁"，加密密钥回答"数据被偷怎么办"，不可互替。
反对凭据派生：OIDC 给的是断言（subject 公开标识符）非稳定秘密，无从派生（决定性反例）；
密码重置=全部 key 永久不可解；密码变更需全量重加密；DB 单点泄露=离线暴破密码解 key；
后台异步任务需无登录态访问 key。方案：env GODOWORD_MASTER_KEY 覆盖，否则自动生成
/data/keys/master.key（0600）；AES-256-GCM 每记录独立 nonce。威胁模型：防静态泄露
（DB 备份/卷快照外流），不防运行中服务器被攻破。Bitwarden 密码派生是其零知识产品定位，
godoword 的 key 是可重填手段，不值得重置丢数据代价。

## 后果

备份卷=备份一切含密钥，丢卷=key 永久不可解（文档须明示）。信封加密留作后续可选。
见 [architecture](../docs/project/architecture.md)。