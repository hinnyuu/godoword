<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { push } from 'svelte-spa-router';
  import { untrack, onDestroy } from 'svelte';
  import {
    streamTurn,
    cancelSession,
    getSession,
    createSession,
    AudioEngine,
    appendWords,
    routeError,
    parseUsage,
    type SessionEvent,
    type WordTimestamp,
    type UsageNote,
    type ErrorRoute,
  } from '@godoword/core';
  import workletUrl from '@godoword/core/audio/worklet?worker&url';
  import { prefs } from '../lib/prefs.svelte';

  let { sessionId }: { sessionId: string | null } = $props();
  let currentId = $state<string | null>(null);

  interface Segment {
    audioId: string;
    words: WordTimestamp[];
    chunks: string[];
    done: boolean;
  }
  interface Msg {
    role: string;
    text: string;
    reasoning: string;
    segments: Segment[];
    showReasoning: boolean;
  }

  let messages = $state<Msg[]>([]);
  let input = $state('');
  let streaming = $state(false);
  let errorRoute = $state<ErrorRoute | null>(null);
  let usageLlm = $state<UsageNote | null>(null);
  let usageTts = $state<UsageNote | null>(null);
  let abort: AbortController | null = null;
  let engine: AudioEngine | null = null;
  let activeAudio = $state<string | null>(null);
  let highlight = $state(-1);
  let lastText = '';
  let autoQueue: Segment[] = [];
  let playPending = false;
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  let messagesEl: HTMLDivElement | undefined = $state();

  // 审计 O10：新消息/流式增长自动滚动（仅在近底时，不与手动上滚阅读打架）。
  // sign-off 轮补全：信号须覆盖气泡内全部流式内容——正文 + 思考 + 音频词 + 折叠展开（原仅跟踪正文）。
  $effect(() => {
    const last = messages[messages.length - 1];
    const words = last?.segments.reduce((n, s) => n + s.words.length, 0) ?? 0;
    const sig = `${messages.length}:${(last?.text ?? '').length}:${(last?.reasoning ?? '').length}:${words}:${last?.showReasoning ? 1 : 0}`;
    if (!messagesEl || sig === '0:0:0:0:0') return;
    const nearBottom =
      messagesEl.scrollHeight - messagesEl.scrollTop - messagesEl.clientHeight < 80;
    if (nearBottom) messagesEl.scrollTop = messagesEl.scrollHeight;
  });

  // 二审 R2-5：卸载清定时器 + 中止在途回合 + 释放音频资源（防 AudioContext 泄漏）。
  onDestroy(() => {
    if (toastTimer) clearTimeout(toastTimer);
    abort?.abort();
    engine?.dispose();
  });

  function toMsg(role: string, content: string, reasoning: string | null): Msg {
    return { role, text: content, reasoning: reasoning ?? '', segments: [], showReasoning: false };
  }

  async function loadHistory(id: string) {
    try {
      const detail = await getSession(id);
      // 单遍构建：tool 消息的音频段挂到**其前驱** assistant（M1：多轮 speak 回放不错位）。
      const msgs: Msg[] = [];
      for (const m of detail.messages) {
        if (m.role === 'tool') {
          try {
            const c = JSON.parse(m.content) as { audio_id?: string };
            if (c.audio_id) {
              const prev = [...msgs].reverse().find((x) => x.role === 'assistant');
              prev?.segments.push({ audioId: c.audio_id, words: [], chunks: [], done: true });
            }
          } catch {
            /* 非 JSON 忽略 */
          }
          continue;
        }
        msgs.push(toMsg(m.role, m.content, m.reasoning ?? null));
      }
      messages = msgs;
    } catch (err) {
      errorRoute = { kind: 'info', message: err instanceof Error ? err.message : $_('session.loadFailed') };
    }
  }

  // 会话切换：中止在途回合 + 重置（M4）；懒创建后的路由同步（sessionId===currentId）不重置。
  // untrack currentId：避免 send 内设置 currentId 触发本 effect 误重置。
  $effect(() => {
    const sid = sessionId;
    if (sid === untrack(() => currentId)) return;
    abort?.abort();
    streaming = false;
    currentId = sid;
    messages = [];
    errorRoute = null;
    usageLlm = null;
    usageTts = null;
    autoQueue = [];
    engine?.stop();
    activeAudio = null;
    highlight = -1;
    if (sid) void loadHistory(sid);
  });

  function ensureEngine(): AudioEngine {
    if (!engine) {
      engine = new AudioEngine({
        workletUrl,
        onHighlight: (i) => (highlight = i),
        onEnded: () => {
          // 自动播放队列：当前段播完接续下一段（M3：多 speak 段不互相打断）。
          const next = autoQueue.shift();
          if (next) {
            void playSeg(next, true);
          } else {
            activeAudio = null;
            highlight = -1;
          }
        },
      });
    }
    return engine;
  }

  function findSeg(assistant: Msg, audioId: string): Segment {
    let seg = assistant.segments.find((s) => s.audioId === audioId);
    if (!seg) {
      assistant.segments.push({ audioId, words: [], chunks: [], done: false });
      // 取 $state 代理引用（M2 加固：新建段亦经代理 mutate）。
      seg = assistant.segments[assistant.segments.length - 1]!;
    }
    return seg;
  }

  function showError(route: ErrorRoute) {
    if (toastTimer) clearTimeout(toastTimer);
    errorRoute = route;
    if (route.kind === 'toast') {
      toastTimer = setTimeout(() => (errorRoute = null), 3000);
    }
  }

  function handleEvent(ev: SessionEvent, assistant: Msg) {
    switch (ev.event) {
      case 'text-delta':
        assistant.text += ev.text;
        break;
      case 'reasoning-delta':
        assistant.reasoning += ev.text;
        break;
      case 'audio-start':
        findSeg(assistant, ev.audio_id);
        break;
      case 'audio-chunk':
        findSeg(assistant, ev.audio_id).chunks.push(ev.data);
        break;
      case 'word-timestamps': {
        const seg = findSeg(assistant, ev.audio_id);
        seg.words = appendWords(seg.words, ev.words);
        break;
      }
      case 'audio-end': {
        const seg = findSeg(assistant, ev.audio_id);
        seg.done = true;
        if (prefs.prefs.auto_play) {
          // 二审 R2-6：pending 守卫——init/play 异步窗口内亦视为占用，防两段并发 playSeg 交错。
          if (engine?.isPlaying || playPending) {
            autoQueue.push(seg);
          } else {
            void playSeg(seg, true);
          }
        }
        break;
      }
      case 'usage': {
        const note = parseUsage(ev.capability, ev.model, ev.usage);
        if (ev.capability === 'tts') usageTts = note;
        else usageLlm = note;
        break;
      }
      case 'error':
        showError(routeError(ev.code, ev.message, ev.retryable, ev.action));
        break;
      case 'run-end':
        streaming = false;
        break;
    }
  }

  async function doSend(text: string) {
    errorRoute = null;
    usageLlm = null;
    usageTts = null;
    messages.push({ role: 'user', text, reasoning: '', segments: [], showReasoning: false });
    messages.push({ role: 'assistant', text: '', reasoning: '', segments: [], showReasoning: false });
    // 取 $state 代理引用（mutate 原始对象不触发响应式，Svelte 5 runes 陷阱）。
    const assistant = messages[messages.length - 1]!;
    streaming = true;
    lastText = text;
    abort = new AbortController();
    try {
      let id = currentId;
      if (!id) {
        id = await createSession(prefs.prefs.on_disconnect_default ?? undefined);
        currentId = id;
        // 懒创建后同步 URL（M4：刷新/分享不丢会话；effect 见 currentId 相等不重置）。
        push(`/chat/${id}`);
      }
      await streamTurn(id, text, (ev) => handleEvent(ev, assistant), abort.signal);
    } catch (err) {
      if (!abort.signal.aborted) {
        showError({ kind: 'info', message: err instanceof Error ? err.message : $_('session.turnFailed') });
      }
    } finally {
      streaming = false;
    }
  }

  async function send(e?: SubmitEvent) {
    e?.preventDefault();
    const text = input.trim();
    if (!text || streaming) return;
    input = '';
    await doSend(text);
  }

  // Q13 retryable 重试：重发上一文本（服务端已持久失败回合的用户消息，重试=新回合提交）。
  async function retry() {
    if (streaming || !lastText) return;
    await doSend(lastText);
  }

  async function cancel() {
    abort?.abort();
    if (currentId) await cancelSession(currentId);
    streaming = false;
  }

  async function playSeg(seg: Segment, fromQueue = false) {
    if (!fromQueue) autoQueue = [];
    playPending = true;
    try {
      const eng = ensureEngine();
      await eng.init();
      eng.stop();
      activeAudio = seg.audioId;
      highlight = -1;
      // 回放无 chunks（历史音频）→ 拉 PCM + meta 词时间戳。
      if (seg.chunks.length === 0) {
        const { getAudioPcm, getAudioMeta } = await import('@godoword/core');
        try {
          const meta = await getAudioMeta(seg.audioId);
          seg.words = meta.words;
          const { bytes } = await getAudioPcm(seg.audioId);
          eng.pushPcm(bytes);
        } catch {
          showError({ kind: 'info', message: $_('session.replayFailed') });
          return;
        }
      } else {
        for (const c of seg.chunks) eng.pushChunk(c);
      }
      eng.setWords(seg.words);
      eng.markComplete();
      await eng.play();
    } finally {
      playPending = false;
    }
  }

  function stopAudio() {
    autoQueue = [];
    engine?.stop();
    activeAudio = null;
    highlight = -1;
  }
</script>

<div class="chat">
  <div class="messages" bind:this={messagesEl}>
    {#each messages as m (m)}
      <div class="msg {m.role}">
        {#if m.role === 'assistant' && m.reasoning}
          <button class="reasoning-toggle" onclick={() => (m.showReasoning = !m.showReasoning)}>
            {$_('session.reasoning')} {m.showReasoning ? '▲' : '▼'}
          </button>
          {#if m.showReasoning}<pre class="reasoning">{m.reasoning}</pre>{/if}
        {/if}
        <div class="text">{m.text}</div>
        {#each m.segments as seg (seg.audioId)}
          <div class="audio-card">
            <button
              onclick={() => (activeAudio === seg.audioId ? stopAudio() : playSeg(seg))}
              disabled={!seg.done}
              title={seg.done ? (activeAudio === seg.audioId ? $_('session.stop') : $_('session.play')) : $_('session.synthesizing')}
            >{activeAudio === seg.audioId ? '⏹' : '▶'}</button>
            {#if activeAudio === seg.audioId}
              <button onclick={stopAudio}>{$_('session.stop')}</button>
            {/if}
            <span class="words">
              {#each seg.words as w, i (i)}
                <span class:active={activeAudio === seg.audioId && i === highlight}>{w.text}</span>
              {/each}
            </span>
          </div>
        {/each}
      </div>
    {/each}
    {#if streaming}<div class="streaming">…</div>{/if}
  </div>

  {#if errorRoute && errorRoute.kind === 'toast'}
    <div class="toast">{errorRoute.message}</div>
  {:else if errorRoute}
    <div class="error-banner">
      {errorRoute.message}
      {#if errorRoute.kind === 'settings'}<a href="#/settings">{$_('error.goSettings')}</a>{/if}
      {#if errorRoute.kind === 'retry'}<button class="retry" onclick={retry}>{$_('error.retry')}</button>{/if}
      {#if errorRoute.kind === 'shutdown'}<span class="hint">{$_('error.shuttingDown')}</span>{/if}
    </div>
  {/if}
  {#if usageLlm || usageTts}
    <div class="usage">
      {#if usageLlm}
        LLM · {usageLlm.model}
        {#if usageLlm.promptTokens != null || usageLlm.completionTokens != null}
          · {usageLlm.promptTokens ?? 0}+{usageLlm.completionTokens ?? 0} {$_('usage.tokens')}
        {/if}
      {/if}
      {#if usageTts}
        · TTS · {usageTts.model}
        {#if usageTts.characters != null}· {usageTts.characters} {$_('usage.characters')}{/if}
      {/if}
    </div>
  {/if}

  <form class="composer" onsubmit={send}>
    <input bind:value={input} placeholder={$_('session.placeholder')} disabled={streaming} />
    {#if streaming}
      <button type="button" onclick={cancel}>{$_('session.cancel')}</button>
    {:else}
      <button type="submit">{$_('session.send')}</button>
    {/if}
  </form>
</div>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 8rem);
  }
  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .msg {
    max-width: 80%;
    padding: 0.6rem 0.9rem;
    border-radius: var(--gw-radius);
    white-space: pre-wrap;
  }
  .msg.user {
    align-self: flex-end;
    background: var(--gw-accent);
    color: #fff;
  }
  .msg.assistant {
    align-self: flex-start;
    background: var(--gw-surface);
    border: 1px solid var(--gw-border);
  }
  .reasoning-toggle {
    background: transparent;
    border: none;
    color: var(--gw-muted);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0;
  }
  .reasoning {
    color: var(--gw-muted);
    font-size: 0.8rem;
    white-space: pre-wrap;
    margin: 0.3rem 0;
  }
  .audio-card {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.5rem;
    flex-wrap: wrap;
  }
  .audio-card button {
    padding: 0.2rem 0.5rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-bg);
    color: var(--gw-fg);
    cursor: pointer;
  }
  .audio-card button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .words span {
    padding: 0 0.05rem;
    border-radius: 2px;
  }
  .words span.active {
    background: var(--gw-accent);
    color: #fff;
  }
  .streaming {
    color: var(--gw-muted);
    padding: 0 1rem;
  }
  .error-banner {
    /* 审计 O10：语义变量底 + 红边框，暗色/自定义主题不刺眼（原硬编码浅底）。 */
    background: var(--gw-surface);
    border: 1px solid #dc2626;
    color: #dc2626;
    padding: 0.5rem 1rem;
    font-size: 0.85rem;
  }
  .error-banner a {
    margin-left: 0.5rem;
  }
  .error-banner .hint {
    margin-left: 0.5rem;
    color: var(--gw-muted);
  }
  .retry {
    margin-left: 0.5rem;
    padding: 0.1rem 0.5rem;
    border: 1px solid #dc2626;
    border-radius: 4px;
    background: transparent;
    color: #dc2626;
    cursor: pointer;
  }
  .toast {
    position: fixed;
    bottom: 5rem;
    left: 50%;
    transform: translateX(-50%);
    background: var(--gw-fg);
    color: var(--gw-bg);
    padding: 0.4rem 1rem;
    border-radius: var(--gw-radius);
    font-size: 0.85rem;
    z-index: 10;
  }
  .usage {
    color: var(--gw-muted);
    font-size: 0.75rem;
    padding: 0 1rem;
  }
  .composer {
    display: flex;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    border-top: 1px solid var(--gw-border);
  }
  .composer input {
    flex: 1;
    padding: 0.5rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-bg);
    color: var(--gw-fg);
  }
  .composer button {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    background: var(--gw-accent);
    color: #fff;
    cursor: pointer;
  }
</style>