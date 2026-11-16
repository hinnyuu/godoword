# 开发规范

> 本规范是 agent 与主开发者的强制约定。门禁机制见 [gates.md](gates.md)，术语见 [glossary.yaml](glossary.yaml)。

## 语言规则

- 文档、wiki、历史的**所有解释型文字用中文**。例外：英文文件名、变量名、代码标识符，
  以及计算机专业常用英文词（token、socket、api、trait、crate、commit 等）。
- 公共接口的 rustdoc 注释用中文（它本质是文档）。代码标识符用英文。
- commit message：type/scope 用英文（机器解析部分），subject 用中文。
  例：`feat(tts): 新增词级时间戳支持`。
- UI 字符串 zh-CN 首发，第一天外部化（i18n），en 后置。

## 用词统一

写作前查 [glossary.yaml](glossary.yaml)，使用规范词，避免禁用别名。
新术语须在同一 PR 入表。CI 机械扫描禁用别名（glossary.yaml 自身除外）。

## 文档体系（四类，严格分离）

| 类别 | 读者 | 职责 | 位置 |
|---|---|---|---|
| docs | agent / 主开发者 | 当前提交代码事实 + 与 roadmap 的差异/待办 | 项目级 `docs/project/`，模块级 `crates/<name>/docs/` |
| wiki | 人类（用户/协作者） | 充足解释，人类逻辑 | `wiki/{user,deploy,contributors}`，模块级 `crates/<name>/wiki/` |
| history | agent / 人类 | 为什么变（决策/变更/事故/退役），不复述当前事实 | `history/` |
| devenv | 本机 agent | 环境本地操作上下文 | 仓库根 `devenv.md`，**严禁提交** |

铁律：
- docs/wiki **只记当前事实**，严禁堆积历史变更（防膨胀与注意力稀释）。
- history 只记"为什么变"，当前事实永远只在 docs。
- 项目文档可引用 devenv.md，但绝不依赖其内容承载项目事实；验收 runbook 写环境无关通用命令。

## 历史格式

每条一个 markdown 文件，文件名 `YYYY-MM-DD-HHMM-<slug>.md`（避免冒号）。frontmatter：

```yaml
date: 2026-11-16T14:30+08:00   # 决策达成时刻，精确到分钟，显式时区
keywords: [tts, qwen, protocol]
scope: [tts-qwen-audio]        # 模块 id 或 project/kernel
type: decision                 # decision | change | incident | retirement
status: accepted               # accepted | superseded
supersedes: []                 # 取代的旧条目文件名
superseded_by: []
pr: ""                         # 关联 PR（可选）
```

正文三段：背景 / 决策 / 后果，简短，链接到当前 docs。
**正文不可变**（append-only）；唯一例外：被取代时更新 status/superseded_by（CI 可自动）。
CI 从 frontmatter 生成 `history/INDEX.md` + `INDEX.json`（派生物，可再生）。

## 提交与验收流程

- 严格 conventional commits；squash merge，PR 标题 conventional。
- 分支开发 → PR → CI 全绿（含 docs-gate）→ 构建推送 `ghcr.io/hinnyuu/godoword:pr-N`/`sha-xxxx`
  → **人工 podman 部署验收** → squash merge 进 main → main 构建 `:latest`，release 打 semver。
- main 分支保护：必须 PR + 检查全绿。
- **每个进入验收的 PR，agent 必须产出验收 runbook**（具体命令、初始化预期、测试清单、通过标准），
  写入 PR 描述的固定章节。验收对象与合并对象须为同一镜像 digest。

## agent 提交前自查

见 [gates.md](gates.md) 的自查清单。核心：改动的源文件命中的 docs/wiki 是否同 PR 更新？
是否需要 history 条目？术语是否合规？链接是否有效？有无密钥/禁入文件？

## 开发环境

重构建一律走 CI（GitHub runner）；本机 `cargo check` 为主、`CARGO_BUILD_JOBS≤2` 防 OOM。
本机环境事实见 `devenv.md`（不入仓库）；agent 会话开始读取，环境变化时维护。