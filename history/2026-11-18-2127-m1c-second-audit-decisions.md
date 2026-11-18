---
date: 2026-11-18T21:27+08:00
keywords: [m1c, second-audit, hardening, concurrency, atomicity, reset-token, backpressure, eviction, openapi, doc-lean, discipline]
scope: [project, kernel, godoword, auth-local]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1c 二审收尾决策 + 文档精炼原则

## 背景

M1c-hardening（fa59c4e）合并后，第三方 agent 二审（/tmp/godoword_M1c-hardening_二次审计意见.md，HEAD dd0211a）
肯定主体加固有效（C1/C2/C3 授权闭合、禁用语义修复、continue 公开、淘汰正常路径不重复、门禁修复），
但指出**首轮加固是 happy-path 导向**，未系统覆盖失败/并发/原子性，且有新瑕疵。核实属实的关键遗留：
R1 重置令牌并发双用 + 空密码绕过；R2 音频写失败仍发成功；R3 shutdown 超时不 abort + 硬超时后满 mpsc 背压可再卡住；
R4 淘汰 outbox 与水位非原子（丢事件/重复）+ 无 summarize 行为测试；R5 OpenAPI CreateSessionReq 悬挂 $ref + 认证/CSRF 非机器可读；
R6 SessionEnded 与 delete 非原子；R7 管理员并发互禁→零管理员；R8 test_gate.py 未接 CI；R9 旧 sidecar 升级/限流器清理/反代 IP。
另有文档漂移（status 自指、roadmap SSE 旧口径、gates 第 14 项、PR 模板"越权 401"应 404、storage 迁移数陈旧、绝对化保证过宽）。

主开发者定案：**M1d 稍延后，下一任 agent 专门做二审收尾加固**（多轮审计/多 agent 加固确保长期稳定，工时不吝）；
并提出关键关切——**安全加固文字在文档占比过高会淹没产品本质、影响 agent 理解需求**。本条目定清原则与决策。

## 决策

**修复设计取舍（主开发者拍板，本期全做）**：
- **R3 退出/背压**：取**真 abort+join**——停新回合 → cancel → 有界等待 → abort 剩余任务 → join → WAL checkpoint
  （sqlx 单语句原子，abort 在 await 点 drop future 不留半事务）；硬超时后事件投递用 try_send/关闭通道，杜绝满 mpsc 永久阻塞。
- **R4 淘汰一致性**：emit_progress + update_eviction **同一 DB 事务**；**summarize 计费去重本期做**——持久化幂等键
  （span 消息 ID 集哈希）防重复摘要计费；补 emit/update 各失败点 + summarize 多回合行为测试。
- **R9 反代 IP**：本期做**可信代理 XFF 解析**（Config trusted_proxies + X-Forwarded-For），否则 IP 限流退化为全局限流。
- R1 重置：条件 UPDATE 认领（used_at IS NULL 查 rows_affected）+ 事务包裹认领→改密→撤销；统一密码入口非空 + 长度校验；双后端并发负例。
- R2 音频：put 失败 → 错误语义（不发正常 AudioEnd/成功 ToolResult）+ 故障注入。R5 OpenAPI：注册 CreateSessionReq + cookieAuth
  + CSRF header param + 403/404 + $ref 全解析测试。R6 SessionEnded 与 delete 原子。R7 管理员禁用事务/锁串行化 + 并发负例。
- R8 test_gate.py 接入 docs-gate CI。R9 旧 sidecar 升级处理（清理/兼容/404 可再生）+ 限流器过期清理。

**文档精炼原则（核心，防加固文字淹没产品本质）**：
加固沉淀到**测试 + 代码 + history + 精炼规则**，而非 live 文档叙事——
①可靠性靠**失败/并发负例测试**机械强制（永久防漂移），不靠散文；②**history** 承载审计发现 + 修复决策完整叙事（不可变、为什么）；
③**docs/wiki** 只记一句话当前事实；④**status** 只留当前未完成项（完成即移除，不累积审计叙事）；⑤**AGENTS/gates** 只加高杠杆规则；
⑥审计报告留外部 + history 摘要，不复制进 docs；⑦产品本质（语言学习/BYOK/LLM 教师 + TTS/教学模块）在 README/architecture/roadmap 保持显著。
多轮审计须**收敛**：每轮 → 修复 + 测试 + history 决策，live 文档不累积审计散文；可靠性增长在机械层（测试）不在文字层。

**元教训（入纪律）**：凡声称"失败关闭/原子/单次/硬边界"，须有**失败路径 + 并发负例**测试锚定，不能只测 happy path
（首轮加固缺此，致二审发现 R1-R4/R7）。此规则补入 AGENTS/gates（一行高杠杆，非清单膨胀）。

## 后果

M1c 状态修正为"主体加固完成，二审遗留待修复复核"（撤回过早的"闭环/无阻塞"）。M1d 延后至二审收尾加固复核完成。
下一任专门加固 agent 凭 status 紧凑待办 + 本条目方案 + 精炼纪律执行 R1-R9 + 并发/失败测试 + CI 接入。
详见 [status](../docs/project/status.md)、[gates](../docs/project/gates.md)、[AGENTS](../AGENTS.md)。