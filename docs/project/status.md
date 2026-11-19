# 工作状态（live 交接）

> 老 agent 持续维护，新 agent **先读此文件**（入口见 [AGENTS.md](../../AGENTS.md)）。
> committed（随仓库走，跨机器交接）。可能滞后——用 git log + 开放 PR + history INDEX 交叉验证。

- verified: 2026-11-19T23:45+08:00
- 当前里程碑: **M1d 实施中——P0 合并；P1 音频听音确认；P2 后端 CI 全绿；P3 web/core（vitest 32）；P4 shell 认证/管理/引导 UI 代码就绪 → 待 CI + 人工认证流 → P5 设置 UI**
- 主线: M0（51d16c3）→ M1 前置 C（bf952a3）→ 前置小 PR（f8ff64c）→ 蓝图（bde1ba8）→ M1a（20efa24）→ M1b（9d9c907）→ M1c（3c4dc0b）→ M1c-hardening（fa59c4e）→ 二审交接（7fe8bc3）→ 多轮审计收尾（a2bdf17）→ 验收+产品愿景（9d15342）→ **M1d 设计落档（本 PR）→ M1d 实施**

## done

- M0 / M1 前置 / M1a / M1b / M1c / M1c-hardening / M1c 多轮审计收尾 / 验收+产品愿景复核：
  全部合并验收完成（详见 history INDEX 与 [roadmap](roadmap.md)；加固叙事沉 history，live 文档只记当前事实）。
  后端基础就绪无阻塞：REST/SSE/cookie 认证/对象级授权/音频回放/词级时间戳/上下文淘汰/outbox/call_log。
- **M1d 设计闭环（history 2049，本 PR 落档）**：需求驱动多轮讨论（agent 审计级全量阅读 + 隔离实例实验确证线缆 →
  Round 1 核心体验 → Round 2 旅程细节 → Q11 虚拟路径专轮 → Round 3 技术设计），Q1–Q17 全部主开发者裁决。
  蓝图入 [roadmap M1d 实施蓝图](roadmap.md#m1d-实施蓝图)。基线实测全绿（gate/test_gate 6-6/fmt/clippy/96 测试）。

## in-flight

- **M1d 实施 PR #18**（分支 feat/m1d-frontend）：P1 音频（听音确认）+ P2 后端（CI 全绿）+ P3 web/core（vitest 32）+
  **P4 shell 认证/管理/引导 UI + core 服务层**（svelte-check/build 绿；IntoParams query 契约修复）已落。待 P4 CI + 人工认证流确认 → P5。
- P0 设计落档已合并（PR #17，bf3a413，R4 main 重测 success）。

## M1d 实施跟踪（phase 级，详细交接）

> 蓝图见 [roadmap](roadmap.md#m1d-实施蓝图)；决策 Why 见 [history 2049](../../history/2026-11-19-2049-m1d-design.md)。
> 单实施 PR 多 phase（P1–P8），音频 spike 前置；每 phase 边界可交接，status 持续维护。

| Phase | 内容 | 状态 |
|---|---|---|
| P0 | 设计落档（roadmap/history/glossary/conventions/status） | **完成（PR #17 bf3a413）** |
| P1 | 音频 spike：web/core audio 引擎 + AudioWorklet 出声 + 词级高亮同步，本地真 key + 移动浏览器实测 | **代码就绪（工具链全绿）；待人工浏览器出声+高亮确认** |
| P2 | 后端扩展：kernel-api 5 类型 ToSchema（**utoipa 耦合→契约级全量审查**）+ 0005 迁移 + path/preferences/folders/sessions-meta + auth-local change_password/UserInfo + godoword REST + schema 补全 + 双后端测试 | **代码+测试 SQLite 绿+契约审查矩阵完成；待 CI PG 双后端/cargo-deny** |
| P3 | web/core：api（openapi-fetch+生成类型）/sse/session/theme/i18n/prefs + vitest + 类型 freshness | **完成（vitest 32/tsc/svelte-check/build 绿；CI freshness 锚已加）** |
| P4 | web/shell 认证/管理/引导 UI | **代码就绪（core 服务层 + i18n/auth/prefs store + hash 路由 + Login/Register/Reset/Admin/Onboarding；svelte-check/build 绿）；待人工浏览器认证流确认** |
| P5 | web/shell 设置 UI（provider 三卡片+高级绑定矩阵+音色试听+主题面板+偏好） | 待启动 |
| P6 | web/shell 会话 UI（对话+文件夹树+音频卡片+高亮 live/回放+usage 脚注） | 待启动 |
| P7 | Dockerfile node 阶段 + axum SPA fallback + 前端 CI job + 冒烟扩展 | 待启动 |
| P8 | 验收：CI 全绿 + R4 main 重测 + 人工残留 sign-off + 全状态面 reconcile + history 实现条目 | 待启动 |

### M1d 已定案决策（不重议，详见 history 2049）

- **产品**：音频默认手动+自动播放开关；音色试听计量端点；设置三卡片+高级矩阵；主题引擎+高级 CSS（**布局拖拽永久放弃**）；
  偏好跨设备随账户；引导清单；on_disconnect 偏好级默认+逐会话覆盖（明示消耗额度）；自助改密+显示名编辑；
  回合级 usage（看板 M2）；响应式含移动真机验收；错误按 code 分流。
- **会话文件夹=虚拟路径**：sessions.path 文件夹链（路径即身份，trie 渲染，会话名不进 path）；**嵌套 M1d 即做**；
  空文件夹登记表 session_folders（UNIQUE(user,module,path)，path NOT NULL）；**模块 chroot**（哨兵 default）；
  删非空→子会话上移一层（绝不级联删会话）；NFC 归一化（日语浊音陷阱）+ 服务端校验；
  **上限固定常量**（深度≤8/段≤64/全路径≤1024，不进 config，校验仅写入时已存数据永不回溯失效）；前缀匹配 Rust 侧（不用 LIKE）。
- **技术**：openapi-typescript+openapi-fetch，生成类型提交+CI freshness；**后端补全 schema（B，单一事实源）**；
  音频风险前置 spike；web/core 框架无关+shell Svelte 5 runes（领域逻辑不进组件，不引状态库）；
  SSE fetch+eventsource-parser；i18n svelte-i18n zh-CN；主题 CSS 变量；路由 history+ServeDir index.html fallback；
  core 重测+组件轻测+视觉人工。

### M1d 后端扩展清单（P2，均不动 trait 签名）

- 0005 迁移：`user_preferences(user_id PK,prefs_json,updated_at)` + `session_folders(id,user_id,module_id,path,UNIQUE(user_id,module_id,path))` + `sessions.path TEXT`。
- REST：`GET/PUT /preferences`；`GET/POST /folders`、`PUT/DELETE /folders/{id}`；`GET /sessions?folder=&page=`（元数据 title/created_at/last_active/message_count/path）+ `PUT /sessions/{id}/path` + `GET /sessions/{id}`（+path）；`GET /audio/{id}/meta`（回放时间戳出口）；`POST /providers/{id}/preview`（试听）；`POST /auth/change-password` + `PUT /auth/me`。
- **契约变更（B）**：kernel-api 加 utoipa 依赖 + ModelInfo/VoiceInfo/ChatMessage/WordTimestamp/AudioFormat ToSchema derive；
  contract.md 耦合声明 axum/sqlx **+utoipa**；**契约级全量语义审查矩阵**（P2/P8）。SSE SessionEvent/Usage 不入 OpenAPI。
- auth-local：UserInfo ToSchema + change_password（验旧密→validate_password→撤销其他会话保留当前，事务）。
- 新依赖：unicode-normalization（NFC，cargo-deny 许可证核验）；前端 svelte5/vite/openapi-typescript/openapi-fetch/eventsource-parser/svelte-i18n/vitest 等。

## blocked / 待拍板决策

- **无阻塞**。M1d 设计全部定案（Q1–Q17），实施待 P0 合并后启动。
- 远期留 M2：默认教师模式产品态（T4）；角色音色档案实体 + 白名单音频复用；六轮复核非阻断观察
  （summarize usage Finish 后或漏记/主循环与 summarize Finish 处理不对称/断言助手增强）+ outbox 消费幂等（A5）一致性检查。

## 下一动作

1. **P4 收尾**：PR #18 CI 绿 + 人工浏览器认证流确认（注册首用户→登录→管理→邀请→重置→引导）。
2. **P5 设置 UI**：provider 三卡片（预设一键 + 测试连接）+ 高级绑定矩阵 + 音色试听 + 主题面板（预设/自定义变量/高级 CSS）+ 偏好开关。
3. P6 会话 UI（对话流 + 文件夹目录树 + 音频卡片 + 高亮 live/回放 + usage 脚注 + 错误分流）→ P7 Dockerfile node 阶段/SPA 伺服/冒烟 →
   P8 验收（人工感官 + 部署矩阵重 sign-off）。

**文档精炼原则**（history 2127）：加固沉淀测试/code/history，live 文档只记当前事实；产品本质保持显著。
M1d 因内容较多，status 按主开发者要求做 phase 级详细跟踪（决策指针指 history，不复制叙事）。

## 开发态临时取舍登记（dev-debt，须回归）

| # | 临时取舍 | 引入 | 回归于 | 状态 |
|---|---|---|---|---|
| T1 | env dev key 旁路 + 固定 dev user | M1a | M1c | **已回归** |
| T2 | 内存 impl 作生产默认 | M1a | M1b/M1c | **已回归** |
| T3 | 容器 root 运行 | M1a | M1b | **已回归** |
| T4 | 无模块默认教师模式 | M1a | M2 决议 | 待 M2 |

## 近期关键上下文（M1d 交接用）

- **审计发现已纳入 M1d**：回放时间戳出口（GET /audio/{id} 仅 PCM，words 在 sidecar 未暴露→P25 回放高亮缺口，M1d 补 /audio/{id}/meta）；
  会话列表仅 id（M1c 设计已延期 M1d，补元数据）；SPA 深链 404（ServeDir 无 fallback，P7 修）；
  OpenAPI 弱类型响应（B 补全 schema）；Dockerfile:20 陈旧注释（P7 顺手修）；错误消息嵌套重复"未配置绑定：未配置绑定："（UI 文案打磨）。
- **SSE 线缆实测**：`event: <kebab 名>` + `data: <纯载荷 JSON>`（无信封）；POST→SSE 故前端用 fetch 流式解析（非 EventSource）。
  认证 cookie godoword_session（HttpOnly/Lax）+ 变更须 X-Requested-With 头；未认证 401 越权 404。
- **存储**：共享 SQL 守双兼容 SQL 子集；双兼容类型映射 PG INTEGER↔i32 / COUNT(*)↔i64；VACUUM INTO 须 raw_sql+字面量。
  路径前缀匹配在 Rust 侧（不用 LIKE，免转义，守 JSON 整取纪律）；文件夹改名/移动=单事务批量改写。
- **契约审查触发**：M1d 的 kernel-api utoipa 耦合新增属契约级→全量 docs+wiki 审查 + 核对矩阵（P2/P8）；
  升级 axum/sqlx/utoipa 视为契约变更（contract.md 声明）。
- 本机 2H2G：重构建走 CI，本机 cargo check/test jobs≤2；npm 慢前端构建优先 CI；cargo-deny 本机未装以 CI 为准；
  GitHub 网络抖动容忍重试；本机 PG 测试用内网镜像 uhub.service.ucloud.cn/hinnyuu/postgres:16（devenv.md，不入仓库）。
- 测试 key 仅经 env（/root/.config/agents/shared/env，QWEN_TOKEN_PLAN_KEY），严禁入仓库/文档/日志；CI 永不用真 key。
- 治理：人工残留清单纪律（history 2302，逐项命令+具体 tag+标准+清理）；治理克制（最稀缺的是真实实现，非更完善治理）。