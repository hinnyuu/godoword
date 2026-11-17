# godoword 服务

这是应用的入口程序。M1a 阶段它已能驱动 LLM 教师对话与 speak 语音合成（经 SSE 实时推送文本与音频），
并提供会话管理接口。M1b 起加入存储基底：数据持久化到卷（默认 SQLite，可外接 PostgreSQL）、
用户 API key 静态加密、启动时自动建表迁移（迁移前自动备份）、自适应降权（bind 目录文件归宿主用户）。
当前对话仍需 dev-bypass 开发模式与自填 key；账户、provider 配置界面随 M1c–M1d 加入。