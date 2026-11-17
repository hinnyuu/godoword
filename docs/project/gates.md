# 门禁

> 关系图谱门禁防止文档漂移。机制设计见 [history](../../history/) 相关决策。
> 诚实局限：门禁防"漏改"，不防"语义写错"；语义一致性靠 agent 自查 + 人工验收。

## 关系图谱

分布式源头：各模块 `module.yaml` 的 `impact` 规则（glob → docs/wiki 清单 + history 要求）
与 `cross_module` 契约边。项目级 [impact-map.yaml](impact-map.yaml) 承载跨模块与基础设施规则
（如 kernel-api 变更牵连全部模块文档）。CI 聚合二者校验。

## CI 检查清单

1. **commitlint**：PR 标题与 commits 符合 conventional（type/scope 英文，subject 中文）。
2. **覆盖率**：`crates/*/src/` 与 `crates/*/tests/` 每个文件须命中本 crate impact 规则
   （tests/ 纳入——serde 黄金测试等是契约锚点，改动触发文档要求）；
   `scripts/`、`.github/workflows/`、`docker/`、`deploy/`、`web/` 每个文件须命中项目规则。
   新文件未登记 → 红（防图谱腐烂）。Cargo.toml/module.yaml/docs 自身豁免覆盖率。
3. **漏改门禁**：改动文件命中的 docs/wiki 须在同 PR 更新，或显式豁免。
4. **链接检查**：docs/wiki/history 内部相对链接有效，**含锚点校验**（`#锚点` 须命中目标标题）。
5. **术语门禁**：扫描 [glossary.yaml](glossary.yaml) 禁用别名（glossary.yaml 自身除外；
   **history 豁免**——历史正文不可变，写入时用词合规由当时门禁保证）。
6. **中文门禁**：解释型 markdown 须含 CJK 字符（纯英文文档 → 红）。
7. **禁入路径**：`devenv.md`、`*.key`、`.env*`（除 .env.example）等不得出现在提交。
8. **gitleaks**：密钥扫描；**下载二进制经 sha256 校验和验证后执行**；豁免收窄为占位符与
   CI 测试凭据 regex，保留 docs/wiki/history 目录扫描。
9. **依赖许可证**：cargo-deny 白名单（MIT/Apache-2.0/BSD/ISC 等），其余个案审。
10. **Rust**：fmt / clippy / check / test。**前端**：lint / build（M1 起）。
11. **构建推送**：镜像推送 `ghcr.io/hinnyuu/godoword:pr-N` 与 `sha-xxxx`。
12. **容器冒烟测试**：`scripts/acceptance/smoke.sh`，CI 在镜像构建后以 podman 自动执行
    （healthz/readyz/meta/config/openapi/静态、双栈、优雅退出、用户级 REST release 把守 503、
    /data 初始化断言 db/master.key 0600/backups、完整 uid/卷矩阵 rootless named/bind 非 1000 uid +
    rootful bind/named 降权属主、删容器重建数据存活 + 主密钥复用），资源隔离 + 自动清理。
13. **双后端存储测试**（纪律➀）：`storage` job 以 postgres service + SQLite 跑 **workspace 级**
    `cargo test --workspace`（设 `TEST_PG_URL`），任何读此变量的测试自动获双后端；
    **main 分支保护 required check**，方言漂移双向捕获。含共享 SQL 的 crate 须走 `backends()` 夹具。
    release 亦对 main 镜像重跑冒烟（R4 same-tree）。
14. **status 新鲜度**：改 `src/` 或 `tests/` 须同 PR 更新 `docs/project/status.md`（live 交接），
    可 docs-nochange 豁免。见 [conventions](conventions.md#agent-交接与入门)。

本地预检：`python3 scripts/docs-gate/gate.py --local`（检查 2–7，无需网络）；
容器冒烟可本地跑 `bash scripts/acceptance/smoke.sh <镜像>`（需 podman，自动清理）；
双后端存储测试本地需 PG 容器（设 `TEST_PG_URL`），`cargo test -p kernel-api --test storage`。

## agent 提交前自查清单

1. `git diff --name-only` 列出改动文件。
2. 对每个改动源文件查 impact 规则，列出应更新的 docs/wiki。
3. 逐一确认已更新，或走豁免流程。
4. 行为/接口/决策变更 → 写 history 条目（[格式](conventions.md#历史格式)）。
5. 新术语 → glossary.yaml 入表。
6. 跑 `gate.py --local` 预检。
7. 确认无密钥、无禁入文件。
8. commit message 合规（type/scope 英文 + subject 中文）。
9. 有人工残留项时：PR 清单逐项含完整命令（具体镜像 tag）+ 通过标准 + 清理命令，覆盖全部残留项
   （[人工残留清单纪律](conventions.md#验收拆分自动化冒烟--人工残留)）。

## 豁免流程

确无需更新文档时：PR 加标签 `docs-nochange` + PR 评论说明理由。门禁记录豁免留痕，
人工验收审视豁免合理性。豁免不是常态，频繁豁免须反思图谱规则是否过宽。

## 图谱维护

新增模块/文件须登记 impact 规则（覆盖率检查强制）。模块退役须清理其规则与跨模块边。
图谱规则本身变更（gates/impact-map/glossary）须更新本文档并记 history。

## 契约级变更全量审查

机械门禁防漏改，不防语义漂移。**凡 kernel-api 契约级变更**（trait 签名/Store 形态/协议类型/
耦合版本），强制全量 docs+wiki 语义审查：附核对矩阵（doc 文件×变更事实），agent 全量读核对、
搜剿旧表述残留、修不一致，结论记 history。见 [conventions](conventions.md#契约级变更全量审查制度化)。