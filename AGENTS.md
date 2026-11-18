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
- 查 `git log` + 开放 PR——什么在飞。优先 `gh pr list --state open`；**无 gh 时查公开 PR 页/REST API；
  都不可用须在 status/交付显式写"开放 PR 未核验"，不得默认为零**（审计 D9）。
- 查 history INDEX 近期条目——已定决策不重新讨论（本项目旧伤：混淆历史与当前方案）。
  **但缺陷≠重议**：审计/测试发现的正确性、安全、授权缺陷必须登记并修复，不以"已定案"为由回避。
- status 可能滞后：用 git log / PR / history **交叉验证**当前状态。
- **冲突裁决**：git log/合并 PR 定主线；源码 + 可复现测试定实现事实；status 负责 live 交接；history 只解释为什么。
  冲突未解时标 blocked，**不自行挑选最有利的一份文档**（审计 P2）。
- 遵守 conventions/gates：讨论先于大改；契约级变更走全量语义审查；conventional commits（subject 中文）。

## 工作纪律

- **保持 status.md 当前**：每完成有意义步骤更新 done/in-flight/下一动作；收工前确认反映现实。
  上下文可能突然耗尽，持续维护比临终遗言健壮。**交接 PR 不把自身列为 merged HEAD 的 in-flight**（自指陈旧，审计 D2）。
- **子里程碑收尾定向全表面核对**（审计 D8）：交接 PR 须核对 status/roadmap/architecture/README/wiki/web/OpenAPI/PR 模板
  是否受本轮影响并同步至单一事实；设计阶段审查不能替代实现后的"代码—设计—测试—当前事实"验收。
- 改 `src/`、`tests/` 或 `migrations/`、`web/` 须同 PR 更新 status.md（gate 强制新鲜度）+ 命中的模块 docs/wiki + history。
- 提交前跑 `python3 scripts/docs-gate/gate.py --local` 预检；门禁逻辑改动须过 `scripts/docs-gate/test_gate.py`。
- 重构建走 CI；本机 `cargo check` 为主、`CARGO_BUILD_JOBS≤2`（2H2G 防 OOM）。
- 双后端：含共享 SQL 的测试走 `backends()` 夹具（SQLite 恒跑 + `TEST_PG_URL` 驱动 PG）；
  共享 SQL 守双兼容 SQL 子集（uuid 主键 / 应用侧时间戳 / JSON 整取回 Rust）。
- **多用户授权负例**：用户级资源（session/audio/provider/binding）须对象级归属校验 + 两用户越权负例（审计 C1/C2）。

## 文档体系

| 类别 | 受众 | 职责 | 入仓库 |
|---|---|---|---|
| docs | agent | 当前代码事实 + roadmap 差异 | 是 |
| wiki | 人类 | 用户/协作者向解释 | 是 |
| history | agent/人类 | 为什么变（不可变） | 是 |
| status | agent | live 交接状态 | 是（随仓库走） |
| devenv | 本机 agent | 环境本地事实 | 否（gitignored） |