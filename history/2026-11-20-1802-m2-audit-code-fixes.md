---
date: 2026-11-20T18:02+08:00
keywords: [audit, code-fixes, folders, conflict, metering, cancelled, speak, validation, frontend, ux]
scope: [kernel, godoword, auth-local]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M2 审计代码修复轮（F1 + O 系列，设计定案后独立 PR）

## 背景

PR #21 设计审计修订合并 main（b1a19b8，R4 冒烟重测通过），主开发者裁决「设计修订在 PR #21、
代码问题另开修复 PR」节奏（history 1721）。本 PR 落全量代码审计发现的可前置项；
契约/语义级变更（B2 删除事件归 M2 P2、O1 EOF 对称化归 M2 P6 一致性检查）不在本 PR 动 M1c 既有锚定。

## 决策

- **F1 delete_folder 上移冲突预检**（实验确证缺陷）：与 rename 对称——上移后路径撞子树外既有文件夹 →
  Conflict(400) 非 Storage(500)；**write_tx 删除先于改写**（目标路径同事务释放，/a/a 可上移复用 /a）；
  数据完整性不变（冲突整体回滚）。双后端负例（冲突拒绝+数据完整+目标路径复用）。
- **O2 计量写失败 warn 留痕**：record_call 单点收口（fail-open 不阻塞回合不变，静默丢弃改 warn；
  M2 用量看板以 call_log 为事实源）。
- **O3 TTS 流中途 cancel 记 cancelled**（原 ok）：call_log 状态语义准确；SlowTts 中途 cancel 负例锚定。
- **O4 models/voices 端点 adapter 匹配校验**：provider.adapter_id 与端点能力不符 → 400
  （防拿用户 key 打错误上游路径）；归属校验先于匹配（越权仍 404）；负例同配。
- **O5 binding 输入枚举校验**：capability ∈ llm/tts/stt、role ∈ default/summarizer/extractor/assessor、
  model 非空（create/update）→ 400；module_id 注册表校验留 M2（注册表未立）。
- **O6 遗留无守卫公共面收敛**：auth-local set_user_status/admin_reset_password + kernel repo::ensure_user
  加 #[doc(hidden)] + 文档警示（仅测试夹具用，生产走 guarded/token/register 流程）；集成测试依赖 pub 故不降 visibility。
- **O7 SessionRepo::delete 标注**：仓储层原语不发事件，生产 REST 走 journal::end_session 原子认领（doc 警示）。
- **O8 move_session UPDATE rows 校验**：预检后并发删除 → Ok(false) 不假成功。
- **O9 speak 钳位**：rate clamp [0.5,2.0]（上游契约区间，kernel 不依赖上游拒绝）+ 文本 ≤2000 字符
  （超限工具错误回喂 LLM）；单测锚定钳位/上限/音色 fallback。
- **O10 前端 UX 四项**：ChatView 播放中 ⏹ 按钮真停止（原重播，图标语义反）+ 近底自动滚动
  （不与手动上滚打架）+ 错误横幅语义变量底（原硬编码浅色暗主题刺眼）；SessionSidebar 删隐式文件夹
  先确认再登记（原序取消留空文件夹副作用）。svelte-check 0 错 + core vitest 54 绿；视觉项入人工残留。
- **不动项（时序/归属已定）**：O1 主回合/TTS EOF 对称化 → M2 P6；B2 删除事件语义 → M2 P2；
  O11 列表分页 → M2 仪表盘；O12 /config 公开+禁用枚举 → M4 公网重审（私有邀请制可接受）。

## 后果

kernel/godoword/auth-local 测试全绿（turn 19 含新 cancelled 负例、m1d 双后端含 F1 负例、settings handler 级
adapter 匹配/枚举负例、speak 单测 3）+ clippy -D warnings + fmt + svelte-check + vitest 54。
docs/wiki 同步（kernel docs+storage+wiki / godoword docs+wiki / auth-local docs+wiki / architecture 前端节）。
安全面无变更（授权/失败关闭/CSRF 教义不动）。人工残留：前端视觉四项（播放停止/自动滚动/暗色横幅/删除确认序）。
详见 [history 1721 审计修订](2026-11-20-1721-m2-audit-revision.md)、[status](../docs/project/status.md)。