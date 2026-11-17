---
date: 2026-11-18T03:29+08:00
keywords: [audio, cache, whitelist, tts, fresh, replay, static-asset, role-play, byok, voice-cloning, eviction]
scope: [project, kernel, kernel-api]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 音频复用：显式白名单取代指纹 hash 去重

## 背景

蓝图（1720）与 architecture/roadmap 原假设"TTS 缓存 key = hash(model+voice+text+params)，回放不重复付费"——
靠指纹**推断**两请求是否相同以自动去重。M1c 设计讨论从需求出发审视：不同模块/用途复用标准完全不同
（五十音标准发音稳定可共享；对话同句不同场景语调情感不同不可复用；角色扮演老者/小孩即使同音节也须重新生成）。
指纹推断有整类正确性风险：hash 字段集合须覆盖全部影响音频的字段，漏字段即碰撞→错音频。讨论中暴露具体命门：
voice id 是 provider 作用域的（BYOK 克隆音色尤甚），TtsRequest 不含 base_url，纯 TtsRequest hash 会致
跨服务商同名 voice 碰撞→跨用户错复用/泄露。主开发者提出：不靠指纹推断，改由模块**显式声明**白名单。

## 决策

**显式白名单取代指纹推断**：默认一律新鲜合成（无标记）；要缓存的由模块明确声明"这条音频可缓存，稳定 key=X"。
同一性由权威（模块）**声明**而非指纹**推断**——消除整类碰撞 bug（provider 身份/缺失字段/语境/克隆/角色扮演），
且抗未来需求演进（新音频默认 fresh，要缓存须显式声明）。准确性优先于省钱。

**三层显式模型**（无指纹）：
- **静态资产**（M2）：模块入仓库 canonical 音频（如五十音标准发音），稳定资产 id，模块路由直供，零 TTS。
- **白名单缓存**（M2）：模块声明稳定 key=X → 内核 cached-synthesis 服务（get_or_synthesize），首次合成后缓存。
  内核自动按 module_id + provider 身份（adapter_id+base_url）作用域隔离防跨模块/跨服务商碰撞；模块只管模块内 key 语义。
  经 ModuleContext 暴露（M2 契约新增→届时全量审查）。
- **新鲜（默认，M1c）**：无标记，per-synthesis 随机 AudioId，每次新合成，存供 `GET /audio/{id}` 回放。

**M1c 范围**：仅文件系统回放存储（`FsAudio` impl AudioCache，`/data/cache/<id>.pcm` + sidecar JSON，写 tmp+rename 原子）
+ 新鲜默认（即 M1a 现有行为换文件系统，T2 回归）。**删除**指纹去重/Reuse 策略经 binding params/确定性 AudioId v5/
hash 字段集合（provider 身份修补不再需要）/uuid v5 feature。白名单 cached-synthesis 服务 M2 增量（内部接缝可加
key-based 方法免审查，零返工）。

**缓存有界**（M1c，稳定性必需）：新鲜音频回放存储也增长（PCM≈48KB/秒），无界撑满磁盘→崩溃。
TTL（默认 30 天可配）+ 容量上限（默认 1GB 可配，超限淘汰最旧）+ 周期清理 tokio task。缓存可再生，
淘汰只损失旧音频回放可用性（需要时重新合成），不损失正确性。

**回放≠复用**：回放是"某次合成的音频再放一遍"（per-synthesis，永远正确，M1c）；复用是"跨请求省合成"
（白名单声明，M2）。静态资产与白名单皆模块侧正交机制，不塞进 AudioCache 指纹逻辑。

## 后果

本条目**精化 1720 的音频缓存假设**（hash→白名单），1720 其余决策（拆分/speak/接缝/dev-debt）仍有效，
故不整体 supersede（不动 1720 frontmatter）。architecture/roadmap/status 的 hash-cache 旧表述同 PR 修正。
M1c 音频更简（内存→文件系统 + fresh 默认）、更安全（无推断）、更防漂移（权威声明）。
角色扮演经 SpeakArgs.voice/instruction（契约内）+ 新鲜默认完备支持，M2 零返工（见同日角色扮演条目）。
详见 [architecture](../docs/project/architecture.md)、[kernel/docs](../crates/kernel/docs/README.md)。