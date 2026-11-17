---
date: 2026-11-18T03:29+08:00
keywords: [role-play, persona, voice, character, byok, voice-cloning, foundation, module, adapter]
scope: [project, kernel, kernel-api]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 角色扮演分层精化（横切声音基础设施）

## 背景

0100-modular-architecture 早期分类："角色扮演归内核 persona 能力，非独立模块"。M1c 讨论中主开发者提新需求：
TTS 音色克隆强大 + BYOK 自选 TTS → 角色扮演玩法多；不同模块 LLM 老师声音可为标准播音员/明星/用户自训 TTS 声音；
未来可能有覆盖全部模块音频的角色扮演系统。须重审角色扮演在哪一层：独立模块（集中管理供他模块调用）vs
项目地基（内核）vs 每模块自搞。

## 决策

**角色扮演不是单一层问题，须分解**（五关注点各归其层）：
- 音色克隆 + 音色目录 → **适配器**（TtsProvider.list_voices，克隆是 provider 能力）。
- 声音绑定（跨模块老师声音）→ **内核 binding**（现有 5 级 fallback，全局 TTS 绑定默认 voice 即跨模块老师声音，已支持）。
- 对话中角色声音 → **SpeakArgs.voice/instruction**（契约内）+ 模块 persona 引导（新鲜默认 + 白名单模型完备支持）。
- 角色/音色档案库（命名角色 voice+instruction+persona 片段跨模块复用）→ **内核实体**（M2，与 binding 内聚的用户数据）。
- 角色扮演编排（对话切换角色/角色行为）→ **教学模块**（TeachingModule.persona/工具）。

**定位：横切声音基础设施**（主开发者拍板）。地基提供共享原语（内核 persona/binding/voice + 适配器音色目录/克隆），
模块消费原语编排角色（可选专门角色扮演特性模块做角色管理 UX/复杂编排，M2+，消费地基不垄断声音管理）。
承继 0100"内核 persona 能力"（原语在地基），满足横切共享 + 防上帝对象（特性逻辑在模块）+ 依赖纪律
（模块经 ModuleContext/契约消费地基，不互 import）。否决纯独立模块（声音管理是基础设施不宜离散模块化，
且朴素"他模块调用"违反依赖纪律）与每模块自搞（重复 + 无共享）。

**角色/音色档案需内核实体**（主开发者拍板）：M2 新增迁移（characters/voice_profiles 表）+ kernel-api 契约扩展
（经 ModuleContext 暴露）走届时全量审查。M1c 不建此实体（治理克制），音频机制对实体形态无关、零返工。

**音色克隆完全归服务商**（或用户自建 TTS API）管，内核不管克隆流程（适配器 list_voices 暴露克隆所得 voice id）。
未来若有开发者非要做项目级克隆，做成**独立模块产出 TTS API**，内核当普通服务商 TTS API 正常使用
（即成为适配器/provider，非内核克隆逻辑）。

## 后果

本条目**精化 0100 的角色扮演分类**（横切基础设施 + 内核实体 M2 + 克隆 provider 侧/独立模块产 TTS API），
0100 三层架构其余仍有效，故不整体 supersede（不动 0100 frontmatter）。角色扮演为 M2+ 方向，M1c 仅确保
音频原语就绪（新鲜默认 + SpeakArgs.voice/instruction + 白名单 M2），不实现任何角色扮演逻辑。
详见 [architecture](../docs/project/architecture.md)、[roadmap](../docs/project/roadmap.md)、同日音频白名单条目。