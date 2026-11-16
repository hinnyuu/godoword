<!-- PR 标题须符合 conventional commits（type/scope 英文 + subject 中文） -->

## 变更说明

<!-- 做了什么、为什么。关联 history 条目（如有） -->

## 文档同步

<!-- 改动的源文件命中了哪些 docs/wiki？已更新还是豁免（docs-nochange 标签 + 理由）？ -->

## 测试

<!-- 单测/fixture/本地预检结果 -->

## 自动化冒烟（CI）

> CI `smoke` job 在镜像构建后自动执行 `scripts/acceptance/smoke.sh`（podman，资源隔离 + 自动清理）。
> 覆盖 healthz/meta/静态/双栈/优雅退出/uid 卷矩阵/删容器重建/bind 属主。本地可跑
> `bash scripts/acceptance/smoke.sh <镜像>`。

- [ ] CI smoke 绿（贴 checks 链接）
- 镜像 digest：<!-- 见 CI image job -->

## 人工残留验收清单（仅自动化覆盖不到的项）

> 需人类感官/判断（webui 视觉交互、TTS 音质）或 agent 缺访问权（SELinux/quadlet/docker 主机）的项。
> 给出具体步骤与通过标准。**无残留则写"无"**。merge 前置 = 冒烟绿 + 人工对残留 sign-off。

| 项 | 主机/条件 | 步骤 | 通过标准 |
|---|---|---|---|
| <!-- 例：SELinux :Z --> | <!-- Fedora --> | <!-- 命令 --> | <!-- 期望 --> |

- [ ] 人工 sign-off（残留项全部通过 / 无残留）

## 自查清单

- [ ] 已过 `python3 scripts/docs-gate/gate.py --local`
- [ ] 无密钥、无禁入文件（devenv.md/*.key/.env）
- [ ] 术语合规，链接有效
- [ ] 行为/接口/决策变更已记 history

## 合并后动作（人工 sign-off 后）

squash merge → 启用 main 分支保护（PR + 检查全绿）→ ghcr 包设为公开。