---
date: 2026-11-19T13:24+08:00
keywords: [m1c, third-audit, openapi-security, end-session, summarize-failure, reset-amplification, rate-limiter, shutdown, sidecar, migration-rollback]
scope: [project, kernel, godoword, auth-local]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "15"
---

# M1c 第三次审计收尾（N1–N7）+ 测试缺口补齐

## 背景

PR #15（二审收尾 R1–R9）合并前，第三方异地复核（/tmp/godoword_PR15_第三次审计意见.md，head e903888）
肯定主体加固真实有效，但发现 N1–N8：多为前轮修复的边缘/并发缺口或本轮新引入回归。逐项对照代码核实**全部属实**。
主开发者定案整体采纳（N6 终态投递权衡取"硬边界优先 + 明确关闭语义 + 日志"），全部本期修。

## 决策（实现要点）

- **N1（我引入的回归）**：OpenAPI 根级全局 security:[{cookieAuth}] 被所有 operation 继承，误伤公开端点
  （healthz/register/login/reset-password 等，M1d TS 客户端契约回归）。改**操作级 security 矩阵**：is_public 判定，
  公开 security:[]、受保护 cookieAuth；移除根级全局；登记 openapi.json 自身。测试遍历所有 path/method 断言矩阵 +
  所有变更方法 CSRF required/403。
- **N2**：end_session 原 SELECT→INSERT→DELETE 不查 rows，PG 并发重复事件都 true。改 **DELETE...RETURNING module_id
  唯一认领**（取得一行才同事务插事件），并发恰一个 true 一个事件。双后端并发负例。
- **N3**：summarize_span 流中途失败（部分文本+Err）仍返回 Some(部分)→固化复用。改 failed||empty→None，
  按 truncate 回退（不存储/复用部分 fragment）。真实失败链负例（FailSummarizeLlm）。
- **N4**：reset_with_token 原 hash 先于认领（无效令牌触发 Argon2，公开端点放大）→ 廉价预查询令牌有效性后再 hash；
  改密 UPDATE 查 rows_affected==1 否则回滚令牌认领（无凭据不假成功）；create_reset_token 无 local_credentials 拒签发；
  reset-password 端点 IP 限流。负例：无凭据拒签发 + 改密 0 行失败令牌未消费可重试。
- **N5（我引入的回归）**：限流器容量淘汰按起点逐出最旧（含活跃封禁）→ 洪泛绕过。改 prune 仅清过期、
  容量满新键 fail-closed（绝不淘汰活跃项）。负例：小容量洪泛后目标仍 429。
- **N6（权衡定案）**：shutdown 快照竞态 → shutting_down 原子标志拒新回合；满 mpsc 终态 try_send 不可投递记日志、
  流关闭（硬边界优先，不重新引入阻塞）。负例：shutdown 期间 run_turn 拒绝 + 满队列第一条流排空必然关闭。
- **N7**：get sidecar NotFound（PCM-only 崩溃孤儿）→ Ok(None) 404 可再生（原 Err→500）。负例。
- **N8**：迁移失败回滚探针（双后端，首 DDL+版本记录回滚、修正可重跑）；OpenAPI 矩阵测试；真实摘要失败链测试；
  **覆盖表述修正**——runtime 回合行为测试实为 SQLite 单后端，auth/journal/storage 并发与迁移回滚才是双后端，
  architecture/status/wiki 不再泛称"各配负例（双后端）"。

## 后果

N1–N7 修复 + N8 测试缺口补齐，全 workspace 双后端测试绿、fmt/clippy -D warnings 净。
**history 不回写**：二审实现条目（0003）保留为当时声明，本条目记第三次审计修复（审计第五.3 正确）。
待验收闭环：PR #15 CI 重跑绿 + 定向复核 + 合并 + R4 main 重测 + 人工 sign-off → 恢复"M1c 收尾完成无阻塞" → M1d。
教训延续：失败路径/并发唯一性/公开操作语义须作一等公民首轮覆盖，避免边缘缺口留待下轮审计。