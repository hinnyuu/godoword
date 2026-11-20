---
date: 2026-11-20T21:30+08:00
keywords: [m2, p1, audit, pre-fix, tts-url, tool-arguments, reattach, play-guard, scroll, b3, b7, noyalib]
scope: [project, llm-openai, tts-qwen-audio, godoword]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M2 P1 前置全量审计 + 前置修复（A1–A5）+ B3/B7 定案

## 背景

M2 审计全闭环（history 2031）后新 agent 接续 P1。按开工指引全量重建心智模型
（docs/wiki/history + kernel/kernel-api/godoword 核心源码全读；适配器/认证/前端审计 agent 全量覆盖 + 主 agent 逐项复核），
交叉验证 git log（main d10ce8b ← 234a95f ← b1a19b8）与开放 PR（gh 显式核验 0）。
全量代码审计侧重 PR #22/#23：**无 P0**，#22 五项前端改动与 auth-local doc(hidden) 复核零行为漂移；
发现 5 项 P1 应修（A1–A5，皆实验/源码确证）+ P2 观察组。主开发者裁决：**前置小修复 PR**（承 #22 先例）+
B3a Option<SessionId> + B7 noyalib。

## 决策

### 前置修复（本 PR，A1–A5 + 廉价加固）

- **A1 tts-qwen-audio URL 模式下载**：原不查 HTTP 状态——403/404 错误体被当完整 PCM 缓存播放；亦无大小上限。
  修：`download_audio` 查状态（错误归一化复用 map_error）+ 流式累积 32MiB 上限。
  负例同配：本地 HTTP 服务 404→ModelUnavailable / 超限→Internal + 正例归一化（src/lib.rs 测试）。
- **A2 llm-openai tool arguments**：原解析失败 `unwrap_or(Value::Null)` 静默吞错，截断/非法参数以"成功 ToolCall"下发。
  修：assemble 返回 Result，解析失败 yield Internal 错误中止流；空 arguments→`{}`（无参工具惯例）；
  **廉价加固**：tool index 上限 255（防异常上游无界 resize OOM）。负例同配 3（非法/空参/巨 index）。
- **A3 ChatView 断线重附**：history 2049 定案「断线重连=重附持久态（GET session，shell 处理）」未落地——
  流异常仅弹横幅，continue 回合余下内容丢至手动刷新。修：doSend catch（非主动 abort）重拉 loadHistory 重附持久态。
- **A4 播放失效守卫**：playSeg await meta/PCM 窗口内 stop/切会话/重复点击不作废在途请求→幽灵播放/双音叠加；
  回放失败不重置 activeAudio→⏹ 残留。修：playSeq 失效令牌（stopAudio/切会话/新播放递增，await 后校验）+
  失败重置播放态 + stopAudio/切会话重置 playPending（防自动播放队列永堵）。Settings doPreview 同族令牌 + 卸载失效。
  loadHistory 补陈旧响应守卫（在途切会话丢弃）。
- **A5 历史加载滚到底**：$effect 近底信号不覆盖初始加载（scrollTop=0/scrollHeight 已增→nearBottom=false长会话停顶）。
  修：loadHistory 后 await tick 无条件滚底（带会话守卫）。
- A3–A5 为 UI 交互正确性，组件轻测姿态（M1d 分层）→ **人工残留 sign-off**（PR 清单逐项命令+标准+清理）。

### P2 观察组登记（不阻塞，归属明示）

- LLM 侧：id/name 覆盖非追加（分片 name 网关丢片段）+ 仅 finish=="tool_calls" 发组装（stop 带 tool_calls 上游丢调用）
  ——本网关实测协议兼容，**未来接新网关时核对**；TTS usage 首帧 vs LLM last-wins 不对称——history 1455 真 key 探针
  已排除本网关多句场景，**归 P6 一致性检查**（O1 同接缝）；LLM 静默 EOF 部分回答记 ok——**O1 同族归 P6**。
- `audio.url` 上游响应控制的任意 URL fetch：base_url 之外的 SSRF 面，**私有部署邀请制风险接受延伸**（同 base_url 裁决），M4 公网重审。
- auth 未知用户即回/Disabled 先于验密→时延+状态码枚举：与已裁决枚举面同类（私有可接受），M4 重审。
- 前端 P2 组（裸 fetch 绕 401 钩子/prefs 队列不含 load/reset 跨账户残留/Settings min-max 无 form 约束【后端钳位兜底】/
  侧栏懒创建不刷新/i18n 漏网【App 加载中/core 错误兜底/PRESETS 名】/worklet 空转/sampleRate 丢弃/client 注释不符）
  ——**归 M2 P5 前端 phase**（移动适配顺手）或远期。

### B3/B7 定案（主开发者拍板，P1 实施依据）

- **B3a**：`ModuleContext::session_id() -> Option<SessionId>`（for_user=None/for_session=Some；
  与 ProgressEvent.session_id Option 一致；需会话方法对 None fail-closed）。独立 UserContext trait 面翻倍违有界新增姿态，
  哨兵 Uuid::nil 隐藏缺席易误用，皆否决。
- **B3b**：plan_version 类型对齐 **u32/INTEGER**（LearningPlan.version/RunStart 已 u32）。核实 sessions.plan_version TEXT
  从未被写入（生产/测试皆 NULL）→ P1 kernel 迁移 DROP+ADD INTEGER 零数据风险；SessionMeta 补 plan_version: Option<u32>。
- **B7**：YAML crate 选 **noyalib 0.0.63**（实测调查：2026-11-19 仍活跃发布、MIT/Apache-2.0 过 cargo-deny、
  forbid(unsafe_code) 纯 Rust、serde 原生 from_str、YAML 1.2 官方套件 406/406、资源预算防炸弹）。
  serde_yaml 废弃/serde_yml 废弃转 noyalib/serde_yaml_ng（2024-05 停更）/serde_norway（2024-12 停更）皆否。
  0.0.x Cargo 语义精确 pin；MSRV 1.89 → workspace rust-version 1.85→1.89。
- **P1 phase 边界提案**（roadmap 微调，实施 PR 落档）：P1 自带 kernel 0006（plan_version INTEGER + call_log.module_id +
  progress_events outcome/item_ref/difficulty 列 + sessions.completed_at），P2 其余基底顺延 0007；
  complete_session P1 实现（completed_at + session-ended），**B2 DELETE 语义翻转仍 P2**（P3 前无消费者无实际危害，守原裁决）；
  cached-synthesis 机制 P1 落（资产登记表 + /data/assets + get_or_synthesize 计量），P4 gojuon 消费者 + 全量/预取/降级 UX；
  teach-free 迁移走 kernel 0006（sessions/session_folders 皆 kernel 表），模块迁移 runner 仍 P2/P3（首个消费者 assess）；
  契约 trait emit_progress(ProgressInput) 签名实际不动（ProgressInput 早含三字段，变的是 journal 内部函数 + DB 列）。

## 后果

P1 起点稳固（无 P0；A1–A5 修复 + 负例锚定；P2 观察归属明示不淹没）。前置修复合并后启动 P1 实施
（注册表 + ModuleContext + 回合接线 + teach-free 端到端，契约新增触发契约级全量审查 + 二审）。
B3/B7 已定案，P1 不再议。详见 [status](../docs/project/status.md)、[roadmap M2 蓝图](../docs/project/roadmap.md#m2-实施蓝图)。