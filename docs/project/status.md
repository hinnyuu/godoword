# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-17T14:47+08:00
- 当前里程碑: M1 前置小 PR（CI 缺口闭合 + agent 交接基础设施）
- 主线: M0 完成（51d16c3）→ M1 前置存储契约 C 完成（bf952a3）→ 本小 PR 进行中

## done

- M0：脚手架 + 四类文档体系 + 关系图谱门禁 + CI（含容器冒烟）+ kernel-api 契约 + hello 服务；
  人工验收全过（Fedora 44 SELinux/quadlet、真实 Debian 13 docker 含 compose）。
- M1 前置（bf952a3）：存储契约 C 方案（Store enum 双具体驱动 + 薄分发宏 + 双兼容 SQL 子集）；
  纪律➀ CI 真测双后端（storage required check）+ 纪律➁ 方言点设计掉；serde 黄金测试；
  axum/sqlx 耦合声明；R4 same-tree+main 重测；R5 gate 三修；契约级全量审查制度化（首次执行）；
  SQLite CVE RUSTSEC-2026-0459 书面风险接受。

## in-flight

- 本小 PR：§1 双后端兜底扩 workspace 级 + §2 gate 纳入 tests/ + §3 gitleaks 校验和
  + agent 交接基础设施（AGENTS.md + status.md + gate 新鲜度）。

## blocked / 待拍板决策

- **M1 拆分（§4）**：拆子里程碑 vs 不拆——待用户定案。
  agent 倾向：拆子里程碑 + 垂直切片先行 + 无 UI 阶段仅自动验收（完整人工验收留到有 UI 的子里程碑）。

## 下一动作

1. 完成本小 PR → CI 绿（含 storage 双后端 workspace 级）→ 用户 sign-off → merge。
2. M1 拆分定案 → M1 规划（垂直切片优先：llm-openai + 回合循环 + SSE 出声）。

## 近期关键上下文

- 存储：共享 SQL 守双兼容 SQL 子集；`query!` 仅驱动专属，共享用运行时 query_as + 薄分发宏。
- 契约级变更（trait/Store/协议类型）强制全量 docs+wiki 语义审查 + 核对矩阵。
- SQLite CVE 风险接受：本地向量需打开不可信 DB+wal，godoword 自建自有 DB 前提不成立；
  sqlx 0.8/0.9 均 pin libsqlite3-sys <0.38 无上游修复，roadmap 跟踪。
- 治理克制：当前最稀缺的是第一个真实 kernel 实现，非更完善治理（外部评审共识）。