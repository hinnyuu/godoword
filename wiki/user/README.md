# 用户指南

> 面向使用者的人类向文档。当前为 M1a 开发态切片，完整功能随里程碑落地。

godoword 帮助你借助 LLM 老师学习外语（日语优先）。你自行填写 API key（BYOK），
LLM 担任教师，结合语音合成完成听说练习。

## 当前可用（M1a）

M1a 为开发态垂直切片：内核已能驱动 LLM 教师对话并调用 speak 工具实时合成语音（经 SSE 推送），
但需开发模式（dev-bypass）与自填 key，且无配置界面/账户/持久化。普通用户尚无可用的图形界面。

## 即将可用

- M1b–M1d：provider 与 key 配置界面、本地账户、持久化、Svelte 前端
- M2：五十音图自适应课程、学习进度评估
- M3：对话模拟

## 相关

- [部署指南](../deploy/quickstart.md)
- [项目路线图](../../docs/project/roadmap.md)