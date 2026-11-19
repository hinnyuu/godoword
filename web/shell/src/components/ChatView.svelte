<script lang="ts">
  import { _ } from 'svelte-i18n';
  import {
    streamTurn,
    cancelSession,
    getSession,
    createSession,
    AudioEngine,
    appendWords,
    type SessionEvent,
    type WordTimestamp,
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
  let error = $state('');
  let errorAction = $state<string | null>(null);
  let abort: AbortController | null = null;
  let engine: AudioEngine | null = null;
  let activeAudio = $state<string | null>(null);
  let highlight = $state(-1);
  let usageNote = $state('');

  function toMsg(role: string, content: string, reasoning: string | null, toolCalls: unknown): Msg {
    const segments: Segment[] = [];
    // 从 tool 消息 content 提取 audio_id（回放）。
    if (role === 'tool') {
      try {
        const c = JSON.parse(content) as { audio_id?: string };
        if (c.audio_id) segments.push({ audioId: c.audio_id, words: [], chunks: [], done: true });
      } catch {
        /* 非 JSON 忽略 */
      }
    }
    void toolCalls;
    return { role, text: content, reasoning: reasoning ?? '', segments, showReasoning: false };
  }

  async function loadHistory(id: string) {
    try {
      const detail = await getSession(id);
      messages = detail.messages
        .filter((m) => m.role !== 'tool')
        .map((m) => toMsg(m.role, m.content, m.reasoning ?? null, m.tool_calls));
      // 回放：从 tool 消息补音频段到 preceding assistant。
      for (const m of detail.messages) {
        if (m.role === 'tool') {
          try {
            const c = JSON.parse(m.content) as { audio_id?: string };
            if (c.audio_id) {
              const lastAssistant = [...messages].reverse().find((x) => x.role === 'assistant');
              lastAssistant?.segments.push({ audioId: c.audio_id, words: [], chunks: [], done: true });
            }
          } catch {
            /* ignore */
          }
        }
      }
    } catch (err) {
      error = err instanceof Error ? err.message : '加载会话失败';
    }
  }

  $effect(() => {
    currentId = sessionId;
    messages = [];
    error = '';
    if (sessionId) void loadHistory(sessionId);
  });

  function findSeg(assistant: Msg, audioId: string): Segment {
    let seg = assistant.segments.find((s) => s.audioId === audioId);
    if (!seg) {
      seg = { audioId, words: [], chunks: [], done: false };
      assistant.segments.push(seg);
    }
    return seg;
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
      case 'audio-end':
        findSeg(assistant, ev.audio_id).done = true;
        if (prefs.prefs.auto_play) void playSeg(findSeg(assistant, ev.audio_id));
        break;
      case 'usage':
        usageNote = `${ev.capability} · ${ev.model}`;
        break;
      case 'error':
        error = ev.message;
        errorAction = ev.action;
        break;
      case 'run-end':
        streaming = false;
        break;
    }
  }

  async function send(e?: SubmitEvent) {
    e?.preventDefault();
    const text = input.trim();
    if (!text || streaming) return;
    input = '';
    error = '';
    errorAction = null;
    messages.push({ role: 'user', text, reasoning: '', segments: [], showReasoning: false });
    const assistant: Msg = { role: 'assistant', text: '', reasoning: '', segments: [], showReasoning: false };
    messages.push(assistant);
    streaming = true;
    abort = new AbortController();
    try {
      let id = currentId;
      if (!id) {
        id = await createSession(prefs.prefs.on_disconnect_default ?? undefined);
        currentId = id;
      }
      await streamTurn(id, text, (ev) => handleEvent(ev, assistant), abort.signal);
    } catch (err) {
      if (!abort.signal.aborted) error = err instanceof Error ? err.message : '回合失败';
    } finally {
      streaming = false;
    }
  }

  async function cancel() {
    abort?.abort();
    if (currentId) await cancelSession(currentId);
    streaming = false;
  }

  async function playSeg(seg: Segment) {
    if (!engine) {
      engine = new AudioEngine({ workletUrl, onHighlight: (i) => (highlight = i) });
    }
    await engine.init();
    engine.stop();
    activeAudio = seg.audioId;
    highlight = -1;
    // 回放无 chunks（历史音频）→ 拉 PCM。
    if (seg.chunks.length === 0) {
      const { getAudioPcm, getAudioMeta } = await import('@godoword/core');
      try {
        const meta = await getAudioMeta(seg.audioId);
        seg.words = meta.words;
        const { bytes } = await getAudioPcm(seg.audioId);
        engine.pushPcm(bytes);
      } catch {
        error = '音频回放失败（可能已过期）';
        return;
      }
    } else {
      for (const c of seg.chunks) engine.pushChunk(c);
    }
    engine.setWords(seg.words);
    await engine.play();
  }

  function stopAudio() {
    engine?.stop();
    activeAudio = null;
    highlight = -1;
  }
</script>

<div class="chat">
  <div class="messages">
    {#each messages as m (m)}
      <div class="msg {m.role}">
        {#if m.role === 'assistant' && m.reasoning}
          <button class="reasoning-toggle" onclick={() => (m.showReasoning = !m.showReasoning)}>
            {$_('session.reasoning')} {m.showReasoning ? '▲' : '▼'}
          </button>
          {#if m.showReasoning}<pre class="reasoning">{m.reasoning}</pre>{/if}
        {/if}
        {#if m.role === 'tool'}
          <div class="tool">{$_('session.speaking')}</div>
        {:else}
          <div class="text">{m.text}</div>
        {/if}
        {#each m.segments as seg (seg.audioId)}
          <div class="audio-card">
            <button onclick={() => playSeg(seg)}>{activeAudio === seg.audioId ? '⏹' : '▶'}</button>
            {#if activeAudio === seg.audioId}
              <button onclick={stopAudio}>停</button>
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

  {#if error}
    <div class="error-banner">
      {error}
      {#if errorAction === 'settings'}<a href="#/settings">{$_('error.goSettings')}</a>{/if}
    </div>
  {/if}
  {#if usageNote}<div class="usage">{usageNote}</div>{/if}

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
    background: #fef2f2;
    color: #dc2626;
    padding: 0.5rem 1rem;
    font-size: 0.85rem;
  }
  .error-banner a {
    margin-left: 0.5rem;
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