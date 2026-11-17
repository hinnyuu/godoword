# AGENTS.md — agent 入口指南

> 无上下文 agent 的切入口。本文件是**地图与接续协议**，不复述其他文档（保持短小，省上下文）。
> 稳定指南（极少变）；**live 工作状态见 [docs/project/status.md](docs/project/status.md)**。

## 这是什么项目

godoword：私有部署外语学习 Web 应用（日语优先），LLM 担任语言教师 + TTS 听说教学，
完全 BYOK（用户自带 key），AGPL-3.0，单容器 all-in-one（podman 优先、兼容 docker）。
**单人开发者 + 旋转 agent 长期维护**——每个里程碑可能耗尽一个 agent 上下文，由新 agent 接续。

## 阅读顺序（建立心智模型）

1. [docs/project/status.md](docs/project/status.md) — 我们在哪、下一步、待拍板决策（**先读这个**）
2. [docs/project/roadmap.md](docs/project/roadmap.md) — 里程碑与要去哪
3. [docs/project/architecture.md](docs/project/architecture.md) — 三层架构与依赖纪律
4. [docs/project/conventions.md](docs/project/conventions.md) + [gates.md](docs/project/gates.md) — 怎么干活
5. 要动的模块 `crates/<name>/docs/` — 具体契约
6. [history/INDEX.md](history/INDEX.md) — 近期决策（**已定的不要重议**）
7. `devenv.md`（仓库根，gitignored）— 本机环境事实。**新机器 clone 后此文件不存在**：
   探测本机环境（硬件/OS/容器运行时/工具链/网络/资源约束）并创建它；已存在则读取并持续维护。

## 接续协议（新 agent 必做）

- 读 status.md 的「待拍板决策」——这些需用户输入，**别自作主张**。
- 查 `git log` + 开放 PR（`gh pr list`）——什么在飞。
- 查 history INDEX 近期条目——已定决策不重新讨论（本项目旧伤：混淆历史与当前方案）。
- status 可能滞后：用 git log / PR / history **交叉验证**当前状态。
- 遵守 conventions/gates：讨论先于大改；契约级变更走全量语义审查；conventional commits（subject 中文）。

## 工作纪律

- **保持 status.md 当前**：每完成有意义步骤更新 done/in-flight/下一动作；收工前确认反映现实。
  上下文可能突然耗尽，持续维护比临终遗言健壮。
- 改 `src/` 或 `tests/` 须同 PR 更新 status.md（gate 强制新鲜度）+ 命中的模块 docs/wiki + history。
- 提交前跑 `python3 scripts/docs-gate/gate.py --local` 预检（覆盖率/漏改/链接/术语/中文/禁入/status）。
- 重构建走 CI；本机 `cargo check` 为主、`CARGO_BUILD_JOBS≤2`（2H2G 防 OOM）。
- 双后端：含共享 SQL 的测试走 `backends()` 夹具（SQLite 恒跑 + `TEST_PG_URL` 驱动 PG）；
  共享 SQL 守双兼容 SQL 子集（uuid 主键 / 应用侧时间戳 / JSON 整取回 Rust）。

## 文档体系

| 类别 | 受众 | 职责 | 入仓库 |
|---|---|---|---|
| docs | agent | 当前代码事实 + roadmap 差异 | 是 |
| wiki | 人类 | 用户/协作者向解释 | 是 |
| history | agent/人类 | 为什么变（不可变） | 是 |
| status | agent | live 交接状态 | 是（随仓库走） |
| devenv | 本机 agent | 环境本地事实 | 否（gitignored） |