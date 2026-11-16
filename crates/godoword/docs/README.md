# godoword 组合根

bin crate，以 cargo feature 注册适配器与教学模块到 kernel 注册表，启动 axum 服务。

## 当前状态（M0）

最小 hello 服务：`/healthz`、`/readyz`、`/api/v1/meta`，静态占位页。
用于验证 CI 构建与 podman/docker 部署链路。

## roadmap 差异

模块注册、/data 卷初始化、主密钥、迁移、自适应降权、认证会话层 M1 落地。
见 [architecture](../../../docs/project/architecture.md)。