# godoword

借助 LLM 与语音合成 API 学习外语的私有部署 Web 应用，日语优先。完全 BYOK（用户自带 API key），
LLM 担任核心语言教师，结合 TTS 完成听说教学。单容器 all-in-one，podman 优先、兼容 docker。

- 许可证：AGPL-3.0（见 [LICENSE](LICENSE)）
- 状态：M1a 垂直切片完成（适配器 + 回合循环 + SSE 出声），M1b–M1d 进行中（详见 [roadmap](docs/project/roadmap.md)）

## 文档入口

| 读者 | 入口 |
|---|---|
| 用户 / 部署者 | [wiki/user](wiki/user/) · [wiki/deploy](wiki/deploy/) |
| 协作者 / 开发者 | [wiki/contributors](wiki/contributors/) |
| Agent / 主开发者 | [docs/project](docs/project/)（架构、规范、门禁、术语表） |
| 决策历史 | [history](history/) |

## 核心设计

高度模块化：内核（kernel）+ 适配器（adapter）+ 教学模块（teaching module）三层，
模块间仅通过 [kernel-api](crates/kernel-api/) 契约 trait 交流，可整模块重写替换。
文档、wiki、历史严格分离，关系图谱门禁防止文档漂移。

详见 [架构文档](docs/project/architecture.md)。

## 快速开始

M1a 已能驱动 LLM 教师对话与语音合成（开发态切片），部署方式见 [wiki/deploy/quickstart](wiki/deploy/quickstart.md)。
完整教学功能与配置界面随里程碑 M1b–M3 落地。