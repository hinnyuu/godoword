<script lang="ts">
  import { _ } from 'svelte-i18n';
  import SessionSidebar from '../components/SessionSidebar.svelte';
</script>

<div class="sessions-page">
  <SessionSidebar />
  <div class="main">
    <h1>{$_('nav.sessions')}</h1>
    <p>从左侧目录树选择会话继续对话，或新建会话/文件夹整理。删除会话需确认（转录不可恢复）；删除文件夹会话上移一层。</p>
  </div>
</div>

<style>
  .sessions-page {
    display: flex;
  }
  .main {
    flex: 1;
    padding: 2rem;
    color: var(--gw-muted);
  }
  .main h1 {
    color: var(--gw-fg);
  }
</style>