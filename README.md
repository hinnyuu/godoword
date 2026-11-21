# godoword

借助 LLM 与语音合成 API 学习外语的私有部署 Web 应用，日语优先。完全 BYOK（用户自带 API key），
LLM 担任核心语言教师，结合 TTS 完成听说教学。单容器 all-in-one，podman 优先、兼容 docker。

- 许可证：AGPL-3.0（见 [LICENSE](LICENSE)）
- 状态：M1d 完成（Svelte 图形界面：认证/管理/引导/设置/对话/会话文件夹 + 音频引擎词级高亮 + 主题自定义 + 偏好跨设备），后端经多轮审计加固；M2 实施中（P1 模块切片已落：自由对话 teach-free + 模块机制，详见 [roadmap](docs/project/roadmap.md)）

## 产品愿景与核心需求

godoword 的核心是**让 LLM 担任语言教师、结合 TTS 完成听说教学**（日语优先，未来 STT 听说互动）。
以下核心需求贯穿全部设计，**新 agent 接手须始终明确**（当前实现事实见 [architecture](docs/project/architecture.md)，
里程碑序列见 [roadmap](docs/project/roadmap.md)，决策为什么见 [history](history/)）：

- **完全 BYOK**：用户自选 LLM/TTS 服务商并自填 API key，服务端仅代理调用；key 静态加密、不下发浏览器、管理员不可读。
- **多角色声音与角色扮演**：TTS 音色目录/克隆归服务商；跨模块老师声音经绑定配置，对话角色声音经 speak 参数，
  角色音色档案库（M2+）跨模块复用——定位为横切声音基础设施（地基原语 + 模块编排，见 history 角色扮演条目）。
- **自适应教学与集中评估**：多教学模块（五十音/对话等）输出进度事件，评估模块（teach-assess）集中提取、重生成教学方案，
  为各模块 LLM 老师制订难度与侧重点（placement → 教学方案 → 自适应课程，评估为教学核心）。
- **双认证平滑过渡**：本地用户名密码（argon2id）+ 公网 authentik OIDC（M4）；业务数据挂稳定 user_id，切换认证零业务数据迁移。
- **高自定义 WebUI**：Svelte SPA + i18n（M1d 起），界面可定制美化，设置/会话/认证/管理图形化，词级高亮跟读。
- **私有部署单容器**：podman 优先兼容 docker，双栈 IPv4/IPv6，自适应降权，一切持久状态入卷，容器误删重建零丢失。
- **高度模块化**：内核 + 适配器 + 教学模块三层，仅经 kernel-api 契约交流，可整模块重写替换（见下「核心设计」）。

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

M1d 提供完整图形界面：注册登录、BYOK 服务商配置（预设一键 + 音色试听）、与 LLM 教师对话（流式文本 + 语音 + 词级高亮跟读）、
会话文件夹整理、主题自定义、偏好跨设备。数据持久化到卷、key 静态加密、启动自动迁移备份与自适应降权，账户认证与对象级授权就绪。
部署方式见 [wiki/deploy/quickstart](wiki/deploy/quickstart.md)。自适应教学评估与对话模拟随里程碑 M2–M3 落地。