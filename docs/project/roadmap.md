# 路线图

> 本文档记录项目目标序列与当前差异。里程碑决策见 [history](../../history/)。

## 里程碑

| 里程碑 | 内容 | 版本 | 状态 |
|---|---|---|---|
| M0 | 脚手架 + CI + 门禁 + 首批文档 + kernel-api 契约 + 最小 hello 镜像 + 容器冒烟测试（CI 集成） | — | 进行中 |
| M1 | kernel 完整实现 + auth-local + llm-openai + tts-qwen-audio + 设置 UI（BYOK 可配 + 测试按钮）+ 管理员重置 + schema 前瞻（tokens/users.email）+ Svelte 前端骨架 | — | 未开始 |
| M2 | teach-assess + teach-gojuon（placement → plan → 自适应课程，黄金路径全通） | — | 未开始 |
| M3 | teach-dialogue | v1.0 | 未开始 |
| M4 | SMTP 自助重置/邮箱验证 + auth-oidc（authentik）+ 账户关联 | v1.1 | 未开始 |

每里程碑结束须人工部署验收（见 [conventions](conventions.md)）。

## 黄金路径

注册（首用户成管理员）→ 配置 provider（预设模板 + 自填 key）→ placement 评估 → 初始教学方案
→ 五十音图自适应课程 → 对话模拟。评估模块订阅进度事件与提取报告，持续重生成教学方案，
为其他教学模块的 LLM 老师制订系统提示词片段。

## 远期项（未排期）

- tokenizer 计数替代字符启发式估算（适配器按模型族提供 count_tokens）
- anthropic 兼容适配器（llm-anthropic）
- STT 适配器与听说互动（网关已有 qwen-audio-3.0-asr-flash / realtime-plus）
- MySQL 存储驱动（当前 SQLite + PostgreSQL）
- 更多教学模块（卡片闯关、LLM 考官考试等）与失败模块退役
- PWA / 移动端增强
- E2E 自动化测试（当前单测 + fixture + 人工验收）
- mid-run 回放缓冲（若需求出现）
- 会话自动归档、LLM 生成会话标题
- 信封加密（per-user data key）、主密钥轮换工具
- RP-initiated logout（OIDC 会话联动）

## 当前差异（M0）

- 架构契约（kernel-api trait）已定义，但 kernel/适配器/教学模块均未实现。
- 前端为静态占位，Svelte 骨架 M1 起。
- token 估算为字符启发式（roadmap 换 tokenizer）。
- 部署仅 hello 服务，自适应降权/卷初始化完整逻辑 M1 落地。
- 验收矩阵中 SELinux / docker / quadlet 项需其他主机补测（本开发机 Debian 无 SELinux、
  无 docker、podman 4.3.1 无 quadlet）。