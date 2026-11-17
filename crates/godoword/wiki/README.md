# godoword 服务

这是应用的入口程序。M1a 阶段它已能驱动 LLM 教师对话与 speak 语音合成（经 SSE 实时推送文本与音频），
并提供会话管理接口。当前为开发态切片：需 dev-bypass 开发模式与自填 key 才能对话，
账户、provider 配置界面、持久化随 M1b–M1d 加入。