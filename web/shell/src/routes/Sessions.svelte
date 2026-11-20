<script lang="ts">
  import { _ } from 'svelte-i18n';
  import SessionSidebar from '../components/SessionSidebar.svelte';
</script>

<div class="sessions-page">
  <SessionSidebar />
  <div class="main">
    <h1>{$_('nav.sessions')}</h1>
    <p>{$_('session.sessionsHint')}</p>
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