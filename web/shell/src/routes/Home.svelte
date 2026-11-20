<script lang="ts">
  import SessionSidebar from '../components/SessionSidebar.svelte';
  import ChatView from '../components/ChatView.svelte';

  let { params }: { params?: { id?: string } } = $props();
  // '/' → null（ChatView 首次发送懒创建会话，避免空会话泛滥）；'/chat/:id' → 现有会话。
  let sessionId = $derived(params?.id ?? null);
</script>

<div class="home">
  <SessionSidebar />
  <div class="chat-area">
    <ChatView {sessionId} />
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
</style>