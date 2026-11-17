# 用户指南

> 面向使用者的人类向文档。当前为 M1b 开发态切片，完整功能随里程碑落地。

godoword 帮助你借助 LLM 老师学习外语（日语优先）。你自行填写 API key（BYOK），
LLM 担任教师，结合语音合成完成听说练习。

## 当前可用（M1b）

M1b 为开发态切片：内核已能驱动 LLM 教师对话并调用 speak 工具实时合成语音（经 SSE 推送）。
数据持久化到卷（默认 SQLite，可外接 PostgreSQL），你的 API key 静态加密存储，启动时自动建表迁移
（迁移前自动备份）并自适应降权（bind 目录文件归宿主用户）。对话仍需开发模式（dev-bypass）与自填 key；
provider/key 配置界面、账户、图形界面随 M1c–M1d 加入。

## 即将可用

- M1c–M1d：本地账户、provider 与 key 配置界面、会话持久化、Svelte 前端
- M2：五十音图自适应课程、学习进度评估
- M3：对话模拟

## 相关

- [部署指南](../deploy/quickstart.md)
- [项目路线图](../../docs/project/roadmap.md)