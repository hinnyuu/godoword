<!-- PR 标题须符合 conventional commits（type/scope 英文 + subject 中文） -->

## 变更说明

<!-- 做了什么、为什么。关联 history 条目（如有） -->

## 文档同步

<!-- 改动的源文件命中了哪些 docs/wiki？已更新还是豁免（docs-nochange 标签 + 理由）？ -->

## 测试

<!-- 单测/fixture/本地预检结果 -->

## 人工验收 runbook（R13 必填）

> 验收对象须与合并对象为同一镜像 digest。命令环境无关，不假设开发机事实。

**镜像**：`ghcr.io/hinnyuu/godoword:pr-<N>`（CI 构建，digest 见 CI 输出）

**部署步骤**：

```bash
podman pull ghcr.io/hinnyuu/godoword:pr-<N>
podman run -d --name godoword-accept -p 8080:8080 -v godoword-accept-data:/data:Z ghcr.io/hinnyuu/godoword:pr-<N>
```

**初始化预期**：<!-- 首次启动应观察到什么 -->

**测试清单与通过标准**：

- [ ] `curl http://localhost:8080/healthz` 返回 `{"status":"ok"}`
- [ ] <!-- 功能项 -->
- [ ] 删容器重建数据存活（如适用）

**局限声明**：<!-- 本环境无法覆盖的验收项，如 SELinux/docker/quadlet -->

## 自查清单

- [ ] 已过 `python3 scripts/docs-gate/gate.py --local`
- [ ] 无密钥、无禁入文件（devenv.md/*.key/.env）
- [ ] 术语合规，链接有效
- [ ] 行为/接口/决策变更已记 history