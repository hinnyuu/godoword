<script lang="ts">
  import { push } from 'svelte-spa-router';
  import { createSession } from '@godoword/core';
  import SessionSidebar from '../components/SessionSidebar.svelte';
  import ChatView from '../components/ChatView.svelte';
  import { prefs } from '../lib/prefs.svelte';

  let { params }: { params?: { id?: string } } = $props();

  let sessionId = $state<string | null>(null);
  let creating = $state(false);

  $effect(() => {
    if (params?.id) {
      sessionId = params.id;
      return;
    }
    if (!creating && !sessionId) {
      creating = true;
      void createSession(prefs.prefs.on_disconnect_default ?? undefined)
        .then((id) => push(`/chat/${id}`))
        .finally(() => (creating = false));
    }
  });
</script>

<div class="home">
  <SessionSidebar />
  <div class="chat-area">
    {#if sessionId}
      <ChatView {sessionId} />
    {:else}
      <p class="loading">创建会话中…</p>
    {/if}
  </div>
</div>

<style>
  .home {
    display: flex;
  }
  .chat-area {
    flex: 1;
    min-width: 0;
  }
  .loading {
    padding: 2rem;
    color: var(--gw-muted);
  }
</style>