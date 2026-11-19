<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { push } from 'svelte-spa-router';
  import { auth } from '../lib/auth.svelte';

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await auth.login(username, password);
      push('/');
    } catch (err) {
      error = err instanceof Error ? err.message : $_('auth.loginFailed');
    } finally {
      busy = false;
    }
  }
</script>

<div class="auth-card">
  <h1>{$_('app.name')}</h1>
  <p class="tagline">{$_('app.tagline')}</p>
  <form onsubmit={submit}>
    <label>
      {$_('auth.username')}
      <input bind:value={username} autocomplete="username" required />
    </label>
    <label>
      {$_('auth.password')}
      <input type="password" bind:value={password} autocomplete="current-password" required />
    </label>
    {#if error}<p class="error">{error}</p>{/if}
    <button type="submit" disabled={busy}>{$_('auth.login')}</button>
  </form>
  <p class="alt">
    <a href="#/register">{$_('auth.register')}</a> ·
    <a href="#/reset">{$_('auth.resetPassword')}</a>
  </p>
</div>

<style>
  .auth-card {
    max-width: 24rem;
    margin: 4rem auto;
    padding: 2rem;
    border: 1px solid var(--gw-border);
    border-radius: var(--gw-radius);
    background: var(--gw-surface);
  }
  .tagline {
    color: var(--gw-muted);
    font-size: 0.9rem;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 1rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.9rem;
  }
  input {
    padding: 0.5rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-bg);
    color: var(--gw-fg);
  }
  button {
    padding: 0.5rem;
    border: none;
    border-radius: 4px;
    background: var(--gw-accent);
    color: #fff;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.6;
  }
  .error {
    color: #dc2626;
    font-size: 0.85rem;
    margin: 0;
  }
  .alt {
    font-size: 0.85rem;
    margin-top: 1rem;
  }
</style>