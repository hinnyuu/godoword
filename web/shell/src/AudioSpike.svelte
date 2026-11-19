<script lang="ts">
  // P1 音频 spike harness（风险前置验证，非生产 UI）：
  // ① 合成音（无后端）验证 AudioWorklet 引擎 + 高亮；② 真 key live（SSE）；③ 真 key 回放（PCM+meta）。
  // 生产 SSE 客户端/会话 UI 在 P3/P6 重做（此 harness 仅验证音频管线跑通）。
  import { AudioEngine, appendWords, float32ToPcm16, type WordTimestamp } from '@godoword/core';
  import workletUrl from '@godoword/core/audio/worklet?worker&url';

  let engine: AudioEngine | null = null;
  let words = $state<WordTimestamp[]>([]);
  let highlight = $state(-1);
  let status = $state('就绪');
  let buffered = $state(0);
  let username = $state('');
  let password = $state('');
  let loggedIn = $state(false);
  let sessionId = $state('');
  let liveText = $state('请用 speak 工具慢速朗读日语「こんにちは」。');
  let replayId = $state('');

  function ensureEngine(): AudioEngine {
    if (!engine) {
      engine = new AudioEngine({
        workletUrl,
        onHighlight: (i) => (highlight = i),
        onBuffer: (f) => (buffered = f),
      });
    }
    return engine;
  }

  // ① 合成 440Hz 音 1 秒 + 3 个假词（验证引擎/高亮，无需后端/key）。
  async function syntheticTest() {
    const eng = ensureEngine();
    await eng.init();
    eng.stop();
    const sr = 24000;
    const samples = new Float32Array(sr);
    for (let i = 0; i < sr; i++) samples[i] = 0.3 * Math.sin((2 * Math.PI * 440 * i) / sr);
    const fakeWords: WordTimestamp[] = [
      { text: 'こん', begin_index: 0, end_index: 2, begin_time: 0, end_time: 333 },
      { text: 'にち', begin_index: 2, end_index: 4, begin_time: 333, end_time: 666 },
      { text: 'は', begin_index: 4, end_index: 5, begin_time: 666, end_time: 1000 },
    ];
    words = fakeWords;
    eng.setWords(fakeWords);
    eng.pushPcm(float32ToPcm16(samples));
    await eng.play();
    status = '合成音播放中（高亮应随播放移动）';
  }

  async function login() {
    const res = await fetch('/api/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Requested-With': 'godoword' },
      body: JSON.stringify({ username, password }),
    });
    if (res.ok) {
      loggedIn = true;
      status = '登录成功';
    } else {
      status = `登录失败 ${res.status}`;
    }
  }

  // ② 真 key live：创建会话 → POST messages SSE → 音频 chunk + 词时间戳。
  async function liveTest() {
    const eng = ensureEngine();
    await eng.init();
    eng.stop();
    words = [];
    if (!sessionId) {
      const r = await fetch('/api/v1/sessions', {
        method: 'POST',
        headers: { 'X-Requested-With': 'godoword' },
      });
      sessionId = (await r.json()).id;
    }
    status = 'live 回合进行中…';
    const res = await fetch(`/api/v1/sessions/${sessionId}/messages`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Requested-With': 'godoword' },
      body: JSON.stringify({ text: liveText }),
    });
    const reader = res.body!.getReader();
    const decoder = new TextDecoder();
    let buf = '';
    let acc: WordTimestamp[] = [];
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      let idx: number;
      while ((idx = buf.indexOf('\n\n')) >= 0) {
        const frame = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        let ev = '';
        let data = '';
        for (const line of frame.split('\n')) {
          if (line.startsWith('event:')) ev = line.slice(6).trim();
          else if (line.startsWith('data:')) data = line.slice(5).trim();
        }
        if (!data) continue;
        const payload = JSON.parse(data);
        if (ev === 'audio-chunk') {
          await eng.play();
          eng.pushChunk(payload.data);
        } else if (ev === 'word-timestamps') {
          acc = appendWords(acc, payload.words);
          words = acc;
          eng.setWords(acc);
        } else if (ev === 'error') {
          status = `错误 ${payload.code}: ${payload.message}`;
        } else if (ev === 'run-end') {
          status = `回合结束 ${payload.finish_reason}`;
        }
      }
    }
  }

  // ③ 真 key 回放：GET audio PCM + meta（words）。
  async function replayTest() {
    const eng = ensureEngine();
    await eng.init();
    eng.stop();
    const metaRes = await fetch(`/api/v1/audio/${replayId}/meta`);
    if (!metaRes.ok) {
      status = `meta 失败 ${metaRes.status}`;
      return;
    }
    const meta = await metaRes.json();
    words = meta.words;
    eng.setWords(meta.words);
    const pcmRes = await fetch(`/api/v1/audio/${replayId}`);
    const bytes = new Uint8Array(await pcmRes.arrayBuffer());
    eng.pushPcm(bytes);
    await eng.play();
    status = '回放播放中（高亮重放）';
  }

  function stop() {
    engine?.stop();
    status = '已停止';
  }
</script>

<section class="spike">
  <h2>音频 spike（P1）</h2>
  <p class="status">{status} · 缓冲 {buffered} 帧</p>

  <div class="row">
    <button onclick={syntheticTest}>① 合成音测试（无后端）</button>
    <button onclick={stop}>停止</button>
  </div>

  <div class="words">
    {#each words as w, i}
      <span class:active={i === highlight}>{w.text}</span>
    {/each}
  </div>

  <hr />
  <div class="row">
    <input placeholder="用户名" bind:value={username} />
    <input type="password" placeholder="密码" bind:value={password} />
    <button onclick={login}>登录</button>
    <span>{loggedIn ? '已登录' : '未登录'}</span>
  </div>

  <div class="row">
    <input class="wide" bind:value={liveText} />
    <button onclick={liveTest} disabled={!loggedIn}>② 真 key live</button>
  </div>

  <div class="row">
    <input placeholder="audio id" bind:value={replayId} />
    <button onclick={replayTest} disabled={!loggedIn}>③ 真 key 回放</button>
  </div>
</section>

<style>
  .spike {
    margin-top: 2rem;
    padding: 1rem;
    border: 1px solid var(--gw-border);
    border-radius: var(--gw-radius);
    background: var(--gw-surface);
  }
  .row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
    margin: 0.5rem 0;
  }
  .wide {
    flex: 1;
    min-width: 16rem;
  }
  .words {
    font-size: 1.5rem;
    margin: 1rem 0;
    min-height: 2rem;
  }
  .words span {
    padding: 0 0.1rem;
    border-radius: 3px;
  }
  .words span.active {
    background: var(--gw-accent);
    color: var(--gw-bg);
  }
  .status {
    color: var(--gw-muted);
    font-size: 0.9rem;
  }
  input {
    padding: 0.3rem 0.5rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-bg);
    color: var(--gw-fg);
  }
  button {
    padding: 0.3rem 0.8rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-accent);
    color: #fff;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>