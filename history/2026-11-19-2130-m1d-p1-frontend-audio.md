---
date: 2026-11-19T21:30+08:00
keywords: [m1d, frontend, svelte, vite, audio, audioworklet, vitest, workspace, ci, spike]
scope: [project, godoword]
type: change
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# M1d P1 实现：前端工作区骨架 + 音频引擎（风险前置 spike）

## 背景

M1d 设计落档（PR #17，bf3a413）后启动 P1。承 M1a 风险前置原则：先立前端工具链 + 音频引擎 spike
（M1d 最高技术风险），跑通出声 + 词级高亮再铺 UI。设计依据 [history 2049](2026-11-19-2049-m1d-design.md)。

## 决策（实现要点）

- **npm workspaces 两层**：根 package.json（workspaces web/core + web/shell）+ tsconfig.base.json（strict +
  noUncheckedIndexedAccess + exactOptionalPropertyTypes + verbatimModuleSyntax）。
- **web/core**（框架无关 TS）：音频纯逻辑 pcm.ts（16-bit LE→Float32，SAMPLE_RATE=24000）/ highlight.ts
  （activeWordIndex 二分 + appendWords）/ queue.ts（PcmQueue FIFO，欠载补静音）；浏览器侧 worklet.ts
  （AudioWorkletProcessor + registerProcessor('pcm-player')，共享 PcmQueue 打包内联防漂移）/ engine.ts
  （AudioContext({sampleRate:24000}) + AudioWorkletNode + 高亮播放时钟 ctx.currentTime，workletUrl 注入保持 core 框架无关）。
  vitest 18 单测纯逻辑（node 环境）。
- **web/shell**（Svelte 5 runes + Vite 6）：svelte.config（runes:true）+ vite.config（publicDir=static 避与 axum
  占位 public/index.html 冲突，dev proxy /api→8080）+ AudioSpike.svelte harness（① 合成音无后端验证引擎/高亮
  ② 真 key live SSE ③ 真 key 回放）。主题引擎地基 app.css（语义 CSS 变量 :root + data-theme 明暗）。
- **worklet 打包**：Vite `?worker&url` 产出自包含 IIFE（PcmQueue 内联 + registerProcessor，无 ES import），
  AudioWorklet.addModule 可加载——实测构建产物验证。
- **前端 CI job**（提前于 P7 引入验证骨架）：npm ci + core tsc + vitest + shell svelte-check + vite build。
- **镜像兼容**：保留 web/shell/public/index.html 占位（axum 伺服，冒烟静态断言至 P7 切 SPA）；Vite 源 index.html
  独立（title=godoword），build→dist（P7 Dockerfile node 阶段拷入 public）。

## 后果

工具链全验证：core tsc 绿 + vitest 18 绿 + svelte-check 0 错误 + vite build 绿（worklet 独立打包自包含）。
**浏览器出声 + 高亮感官确认 = 人工残留**（agent 无浏览器感官访问权；harness ① 合成音可无 key 验证引擎，
② live 需真 key + 已配 provider，③ 回放待 P2 /audio/{id}/meta 端点）。移动端 AudioWorklet 兼容真机实测列人工残留。
P1 代码层就绪，待人工浏览器 spike 确认 + CI 绿 → P2 后端扩展。详见 [status](../docs/project/status.md)、
[architecture 前端节](../docs/project/architecture.md#前端m1d-起)。