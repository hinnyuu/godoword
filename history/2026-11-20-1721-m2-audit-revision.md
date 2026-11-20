---
date: 2026-11-20T17:21+08:00
keywords: [m2, audit, static-asset, user-side-synthesis, whitelist, persistent-asset, backup, export-import, placement, mobile, drift, contract]
scope: [project, kernel, kernel-api, godoword, tts-qwen-audio]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: "21"
---

# M2 设计第三方审计 + 修订定案（静态音频→用户侧生成 + 白名单回归 + 用户级备份设计）

## 背景

主开发者委托第三方审计（全量阅读 docs/wiki/history/仓库外需求记录/全部源码测试/CI/门禁重建心智模型，
基线门禁全绿），两轮：①全量代码审计；②PR #21 侧重审计（实际需求体验/架构/长期可维护性）。
审计发现设计文档与契约/代码事实矛盾数处 + 漂移 5+1 处 + 代码缺陷 1（实验确证）与观察项若干。
主开发者裁决三项产品决策 + 备份四问，并定修复节奏：**设计修订就在 PR #21（修完才算审计通过），
代码问题另开修复 PR**（职责清晰，符「设计定案→实施」纪律）。

## 决策

### 产品裁决（主开发者）

- **UX-A1 gojuon 无方案不强制定级**：`current_plan=None` 允许**默认节奏**直接学（固定顺序、不跳过），
  placement 随时可补。与 teach-free persona-only 同姿态（零摩擦开学）。
- **UX-A2 移动适配入 M2**：M2 新 UI（仪表盘/操练卡/热力图）同步做好手机适配，M1d 遗留侧栏固定宽挤压一并处理。
- **UX-A3 静态音频入仓库作废（法律前提坍塌）**：主开发者实际调查确认 DashScope 输出权利**不满足 AGPL 再分发要求**。
  改为**用户侧生成**：每用户用自己 TTS key/音色生成 gojuon 标准音频，**一次生成永久使用**（持久卷损毁除外）。
  私人使用无再分发→AGPL 前提问题规避，DashScope 条款阻塞项关闭。
- **用户级资产备份设计定案（实施 M2 后）**：四问裁决——音频按用户各自生成；懒生成+可选一键全量；
  导出包**不含 API key**（导入重填，「key 不下发浏览器」教义不破）；备份设计现在定、实施 M2 后（M3 前或独立小里程碑）。

### 用户侧标准音频机制（UX-A3 工程化）

- **白名单 cached-synthesis 回归 M2**：原推迟理由「静态资产已覆盖 gojuon，无消费者」坍塌——gojuon 标准音频
  正是 history 0329 白名单机制的消费者。模块声明稳定 key（如 `gojuon:あ`）→ kernel `get_or_synthesize`：
  命中直供，未命中按用户 TTS 绑定合成→持久化→call_log 计量（module 归因）。
- **持久资产区 `/data/assets/`**（与 `/data/cache` 分离）：白名单音频**豁免 TTL/容量淘汰**（「一次生成永久使用」
  与可丢弃缓存教义冲突的解法）；kernel 资产登记表（user/module/稳定 key/provider 身份/voice）；
  卷损毁→按需懒再生兜底。回放缓存（新鲜合成）仍走 `/data/cache` 有界淘汰，两区正交。
- **作用域**：承 history 0329 provider 身份（adapter_id+base_url）隔离，**加 voice+model**——换音色/服务商后
  旧音频不匹配→按需懒再生新版本，旧版保留（可回退，~5MB/套有界）。
- **生成时机**：懒生成（学到哪生成到哪，单项 ~1-2s，可后台预取下一课）+ 可选「一键生成全部」按钮（~104 项 ~2-4 分钟）。
  成本 ~520 字符/用户（极小），存储 ~5MB/用户。
- **降级不阻塞课时**：无 TTS 绑定/合成失败→识别+认读照常，听音题暂缺并提示配置/重试（与 UX-A1 零摩擦一致）。
- **契约影响**：ModuleContext 暴露音频合成服务（模块永不触 key/上游）+ 资产登记——P1 有界契约新增纳入，
  契约级全量审查范围略增。构建期合成脚本/资产入仓库/P4 静态资产生产 phase 作废重写。

### 用户级备份设计（实施 M2 后，本轮定案）

- **范围**：用户级 kernel 数据（providers 不含 key/bindings/sessions+转录/folders/preferences/progress/call_log）
  + 模块私有数据（assess plans/reports、gojuon 进度——**模块经契约钩子贡献导出/导入，契约新增归备份里程碑非 M2 P1**）
  + 音频（持久资产必含；回放缓存可选勾选，包体大）。
- **格式**：单 ZIP（manifest.json 版本/用户/时间 + data.json + audio/*.pcm+sidecar）。
- **安全硬要求**：zip-slip 路径穿越防护 + 解压炸弹上限（条目数+解压总量）+ 上传体积上限 +
  导入单事务原子 + 音频暂存失败清理（失败关闭教义）；仅用户自助（AuthUser），管理员不可代操作。
- **导入语义**：恢复向——仅空账户（无会话/provider）或显式「覆盖我的数据」确认（先删当前用户数据，事务原子）；
  **不做合并**（ID 冲突/用户名唯一/remap 复杂度）；schema 版本向前兼容（新包旧应用拒绝）。
- **密钥**：v1 不含 key（key_enc 主密钥加密跨部署不可解，重填最简最安全）；passphrase 加密含 key 留可选增强。
- **价值**：私有部署用户可能无卷访问权（寄部署）→用户级导出=数据可携带权，与整卷备份（部署级）正交互补。

### 审计设计修正（B 系列，PR #21 修订落档）

- **B1 difficulty 契约事实纠正**：`ProgressEvent` 有 outcome/item_ref 但**无 difficulty**（仅 ProgressInput 有）——
  原预告「契约形状已有，非契约变更」不成立。定案：**承认契约形状新增**（ProgressEvent 补 difficulty，
  P1 契约级全量审查已触发，诚实标注）；kernel `emit_progress` 签名同步扩 outcome/item_ref/difficulty。
- **B2 删除事件语义**：现 `end_session`（DELETE）发 session-ended（R6/N2 锚定）与 M2「删除是隐私操作不提取」矛盾
  （assess 订阅 session-ended 会对已消失转录提取）。定案：**DELETE 不再发 session-ended**（隐私删除无事件），
  **complete_session 发 session-ended**（完成≠删除，触发提取）；**实施归 M2 P2**（与 complete_session 同落，
  前置代码修复 PR 不动 M1c 既有语义），R6/N2 测试随 P2 更新。
- **B3 ModuleContext.session_id 张力**：现 trait `session_id()->SessionId` 非可选 vs `for_user` 工厂无会话
  （placement/plan 路由/outbox 消费；删除型事件 session 行已不存在）——列 **P1 契约审查必决项**
  （Option/独立 UserContext/哨兵三选一）。
- **B4 call_log.module_id 入 0006**：用量看板「按模块聚合」+ ctx.llm 全计量需模块归因——现 call_log 无 module_id 列，
  for_user 调用无 session 可 join，会话删除后 join 丢归因。0006 补列，回合循环与 ctx.llm 写入时填。
  另 sessions.plan_version TEXT vs LearningPlan.version u32 vs RunStart.plan_version Option<u32> 类型对齐 P1 定。
- **B5 outbox 消费并发/毒丸**：现 dispatch_once 串行——M2 提取每事件调 LLM 在循环内会阻塞全用户 outbox；
  「失败留待」无重试上限→毒丸饿死后续。设计补：**有界并发/per-event spawn + 超时 + 重试上限/死信**。
- **B6 重规划锁约束**：**不可在 SQLite 写事务内跨 LLM 调用持锁**（单写者阻塞全应用写入）。
  短事务认领行（claim）+ 进程内 per-user 异步锁，失败释放。P3 落地约束。
- **B7 YAML crate 选型**：serde_yaml 废弃（history 1231 留 M1），M2 注册表反序列化选维护中 crate 过 cargo-deny。
- **B8 teach-free 迁移哨兵涟漪**：NULL→teach-free 后 DEFAULT_NAMESPACE/FolderQuery 默认/列表过滤/前端 module 参数
  全须核对（bindings `*` 不动）；P1/P5 核对项。
- **B9 mount 契约单路径**：模块 import 共享 core 单例，`mount(target, services)` 的 services 仅传路由/nav 上下文，
  防双实例版本偏斜；P1/P5 spike 验证。

### 代码审计发现（另开修复 PR，设计定案后启动）

- **F1（实验确证）**：delete_folder 上移冲突→Storage(500)，rename 有 Conflict 预检(400) 而 delete 漏——
  补同款子树外冲突预检；数据回滚完整无损坏。
- **O 系列观察项**：主回合/TTS 静默 EOF 当成功（与 summarize 第五次复核不对称，TTS 部分音频入缓存 ok=true）；
  record_call 失败静默无日志（M2 看板事实源宜 warn 留痕）；TTS cancel 记 ok 宜 cancelled；
  models/voices 端点不校验 adapter 匹配；binding capability/role/module_id 无枚举校验；
  无守卫 set_user_status/admin_reset_password/ensure_user pub 死面（误用绕过守卫隐患）；SessionRepo::delete 死面；
  move_session rows 未校验；speak rate/text 无钳位上限；前端 ⏹ 图标语义反/无自动滚动/错误横幅硬编码浅色/
  删隐式文件夹先登记后 confirm；列表无分页；/config 公开+禁用账户 403 枚举（私有可接受，M4 公网重审）。
- 安全面复核通过（对象级授权/失败关闭/CSRF/限流/XFF/argon2/重置单次/管理员守卫/key 加密/越权 404/OpenAPI 矩阵），
  无新严重漏洞。

### 漂移清扫（C 系列，本 PR 修）

白名单/音色档案「M2→M2+ 推迟」旧表述漏改 5 处：wiki/user「即将可用」、kernel wiki、kernel docs ×2、
architecture 正文（与同文件已改对处自相矛盾）。kernel/src/audio.rs 注释「白名单（M2）」因白名单回归 M2 自然恢复准确，无需改。
glossary「静态资产」术语退役→「持久资产」；「角色扮演」定义音色档案 M2→M2+。
1614 不可变（正文与 frontmatter 均不动，承精化先例），关联以本条目正文引用 + pr:"21" 承载。

## 后果

- M2 蓝图重写：P4 静态资产生产→**用户侧音频机制**（白名单合成服务+持久资产区+gojuon 集成）；
  P2 0006 扩 call_log.module_id + 删除事件语义；P3 锁约束；P5 +移动适配；验收补负例
  （F1 冲突 400/毒丸重试/重规划锁不阻塞写/模块归因/删除 vs 完成事件）。
- 契约新增略增（cached-synthesis + 资产登记 + difficulty 诚实标注）仍**有界**→P1 契约级全量审查 + 二审。
- 本条目**精化 1614**（静态音频/白名单推迟两决策翻转 + 审计修正），1614 其余决策仍有效，
  不整体 supersede（不动 1614 frontmatter，承 history 0329 精化 1720 先例）。
- **全量语义审查矩阵**（设计方向级，行=docs/wiki 全文件×列=本轮变更事实）：roadmap/status/architecture/
  contract/kernel docs+wiki/godoword docs/tts docs/auth-local docs/llm docs/wiki user+deploy+contributors/
  README/glossary/conventions/gates/AGENTS 逐格核对——需改已改（本 PR），一致项不动；
  残留搜索（静态资产/白名单 M2+/音色档案 M2/difficulty 已有）干净。**二审复核**：修订后全表面再读一遍，
  交叉核对 history 1614/0329/0630/0900 与仓库外需求记录 Q18/Q26/Q29 无新矛盾；
  备份「不含 key」与 P17 主密钥威胁模型一致（导出包无密文无明文，跨部署重填）。
- 修复节奏：PR #21 修订合并（设计审计通过）→ 代码修复 PR（F1 + O 系列可前置项；O1 EOF 对称化归 M2 P6 一致性检查，
  B2 删除语义归 M2 P2）→ P1 启动。
- 详见 [roadmap M2 蓝图](../docs/project/roadmap.md#m2-实施蓝图)、[status](../docs/project/status.md)、
  [contract 预告](../crates/kernel-api/docs/contract.md)。