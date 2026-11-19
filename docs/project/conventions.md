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
- 分支开发 → PR → CI 全绿（含 docs-gate【主门禁 + 索引 freshness + **门禁逻辑回归 test_gate.py**，审计 R8】与**容器冒烟测试**）→ 构建推送
  `ghcr.io/hinnyuu/godoword:pr-N`/`sha-xxxx` → CI 自动跑冒烟 → **人工对残留清单 sign-off** →
  squash merge 进 main → main 构建 `:latest`，release 打 semver。
- main 分支保护：必须 PR + 检查全绿。

### 验收拆分：自动化冒烟 + 人工残留

判据：**确定性 CLI 反馈 + agent 有访问权 + 可客观断言 → 自动化冒烟**；
**需人类感官/判断（webui 视觉交互、TTS 音质）或 agent 缺访问权（SELinux/quadlet/docker 主机）→ 人工**。

- **自动化冒烟**：`scripts/acceptance/smoke.sh`，CI 在镜像构建后自动执行（podman）。
  覆盖 healthz/meta/静态页、双栈 localhost+IPv4、优雅退出、uid/卷矩阵（rootless named/bind、
  非 1000 uid、rootful bind）、删容器重建、bind 属主断言。资源用唯一前缀隔离 + 专用端口段，
  `trap EXIT` 自动清理（含临时用户与镜像），**只清自有资源，绝不碰用户容器**。
- **人工残留验收**：仅自动化覆盖不到的项。agent 须在 PR 列出残留清单，人工 sign-off。
  webui 时代视觉交互、TTS 音质等必须人工。
- **人工残留清单纪律**（M1b 验收批评后制度化，防再犯）：PR 残留清单**逐项**须含三件——
  ①**完整可复制命令**（含**具体镜像 tag**，如 `pr-7`，禁笼统 `pr-N`；含前置准备如建目录/装单元）；
  ②**明确通过标准**（期望输出/状态）；③**测试后清理命令**（容器/卷/单元/临时目录，只清自有资源）。
  **覆盖全部残留项**（如 SELinux 须含 `:Z` 正例 + 不带 `:Z` 负例 + quadlet，不得只列部分），
  每项命令与清理一一对应。agent 撰写后逐项自检完整性；人工审视清单可执行性。
- **merge 前置 = 自动化冒烟绿 + 人工对残留 sign-off**。人工介入仍强制，但聚焦于真正需要人的部分。
- 验收与合并须为**同一 tree 内容**（squash merge 保持 tree 不变）；**merge 后 CI 对 main 镜像重跑冒烟**
  （release.yaml smoke job）。不要求同一镜像 digest（重建必生新 digest），以 same-tree + main 重测保证一致。

## agent 提交前自查

见 [gates.md](gates.md) 的自查清单。核心：改动的源文件命中的 docs/wiki 是否同 PR 更新？
是否需要 history 条目？术语是否合规？链接是否有效？有无密钥/禁入文件？

## 开发环境

重构建一律走 CI（GitHub runner）；本机 `cargo check` 为主、`CARGO_BUILD_JOBS≤2` 防 OOM。
本机环境事实见 `devenv.md`（不入仓库，gitignored）。**不存在时**（新机器 clone）agent 探测本机环境
（硬件/OS/安全模块/容器运行时/工具链/网络可达性/资源约束）并创建；**存在时**会话开始读取、环境变化时维护、
每次核验更新 verified 时间戳。严禁写入密钥值（只记路径），严禁提交。
本机双后端存储测试需 PG 容器（podman postgres:16，设 `TEST_PG_URL`）；CI storage job 自带 service。

## 存储双兼容 SQL 子集（纪律➁）

模块共享查询须落在 SQLite 与 PostgreSQL 双兼容 SQL 子集内，把方言点设计掉：

- **uuid 主键**，应用生成 id——不用 AUTOINCREMENT/IDENTITY（灭自增方言）。
- **时间戳应用侧生成**（`Utc::now()` 插入）——不用 DB 默认值 `now()`/`CURRENT_TIMESTAMP`（灭默认值方言）。
- **JSON 整取回 Rust 处理**——不在 SQL 里按子字段过滤/查询（灭 `->>` vs `json_extract` 分裂）。
- 占位符 `$1,$2…`（SQLite 由驱动翻译为 `?`）；不用 `::cast`（SQLite 把 `$1::uuid` 静默解析为命名参数）。
- 单一 PG 风格类型名迁移集（UUID/TIMESTAMPTZ/JSONB/BOOLEAN）靠 SQLite 类型亲和性双后端通用。

残余真方言点收口 kernel helper。共享代码用运行时 `query_as` + 薄分发宏；`query!` 仅驱动专属。
**CI 真测双后端**（纪律➀，storage required check）兜底：违反子集 → 某一后端测试红，双向捕获。

**双后端夹具约定**：凡含共享 SQL 的 crate，其存储测试须走 `backends()` 模式
（参照 `crates/kernel-api/tests/storage.rs`：SQLite 恒跑 + `TEST_PG_URL` 驱动 PG，未设时显式 skip 提示）。
CI `storage` job 跑 **workspace 级** `cargo test --workspace`（设 `TEST_PG_URL`），任何读此变量的测试
自动获双后端——无需维护 crate 清单。rust job 的 workspace 测试未设 `TEST_PG_URL`（验代码健康，PG 侧 skip）。

## 前端纪律（M1d 起）

承 [history 技术栈决策](../../history/2026-11-16-0330-rust-svelte-stack.md) 与
[M1d 设计](../../history/2026-11-19-2049-m1d-design.md)：

- **分层**：`web/core` 框架无关 TS（API 客户端/类型、SSE 客户端、音频引擎、会话/路径域逻辑、主题引擎、i18n、偏好），
  `web/shell` Svelte 5 SPA（runes）视图层，`web/modules/<id>` 教学模块前端（M2+）。换框架时 core 存活、视图按模块增量重写。
- **领域逻辑不进组件**：域逻辑（路径 trie/NFC/校验、SSE 帧解析、音频排队、主题变量合并）在 core 纯 TS，shell 以 runes 绑定。
- **契约类型单一事实源**：后端 OpenAPI（utoipa）→ openapi-typescript 生成类型，**生成文件提交入仓库 + CI freshness diff**
  （重生成比对，类比 history INDEX）；后端补全 schema（含 kernel-api REST-facing 类型 ToSchema），不手写重复类型。
- **i18n**：UI 字符串 zh-CN 首发，**第一天外部化**（svelte-i18n），en 后置。
- **测试分层**：core vitest 重测（域逻辑 + 脱敏 fixture round-trip 锚定线缆）+ 组件轻测 + **视觉/音质/交互人工感官验收**
  （M1d 强制）；E2E 自动化后置 roadmap。CI `frontend` job：OpenAPI 生成类型 freshness（gen + git diff --exit-code）+
  npm ci + core tsc + vitest + shell svelte-check + vite build；重构建走 CI（本机 npm 慢）。
- **音频风险前置**：AudioWorklet 引擎先 spike 跑通出声 + 词级高亮 + 移动浏览器，再铺 UI（承 M1a 风险前置原则）。

## 全量语义审查（制度化）

两类变更除机械门禁外**强制全量 docs+wiki 语义审查**：
①**kernel-api 契约级变更**（trait 签名、Store 形态、协议类型、耦合版本）；
②**supersede 蓝图/architecture 既有假设的设计方向变更**（非契约变更但跨多文档制造真实分歧，
如音频缓存指纹→显式白名单）。设计方向级审查须趁上下文仍在做二审复核（一审自查难免盲点）。

- 附**核对矩阵**（行=所有 docs/wiki 文件，列=本次变更事实），逐格核对"需改/已改/一致"。
- agent 全量读所有 docs/wiki，核对每个论断符合新现实，搜剿旧表述残留（如废弃契约名、旧规则、被 supersede 的旧假设），
  交叉核对文档间一致性，修不一致。非抽查，是全量。
- 审查结论记一条 history（含触发理由：契约级 or 设计方向级 supersede）。机械门禁防漏改，全量审查防**语义漂移**——两者互补。
- 部分精化既有 history 条目（非整体取代）时，新条目正文引用澄清，**不**动旧条目 frontmatter（整体取代才改 status/superseded_by）。

## agent 交接与入门

旋转 agent 模型的关键基础设施（详见根 [AGENTS.md](../../AGENTS.md)）：

- **AGENTS.md**（根，committed）：稳定入门地图 + 阅读顺序 + 接续协议。短小，指向文档不复述。
- **docs/project/status.md**（committed）：live 交接状态——当前里程碑、done/in-flight/blocked、
  下一动作、待拍板决策、近期上下文。随仓库走（跨机器交接），与 devenv（环境本地、gitignored）相反。
- **持续维护**：每完成有意义步骤更新 status；收工前确认反映现实。上下文可能突然耗尽，
  持续维护比临终遗言健壮。
- **新鲜度门禁**：改 `src/` 或 `tests/` 须同 PR 更新 status.md（gate 强制，可 docs-nochange 豁免）。
- 新 agent 接续：先读 status → 查 git log/开放 PR/history INDEX 交叉验证 → 待拍板决策交用户，
  已定决策不重议。