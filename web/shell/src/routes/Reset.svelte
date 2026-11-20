<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { push } from 'svelte-spa-router';
  import { resetPassword } from '@godoword/core';

  let token = $state('');
  let newPassword = $state('');
  let error = $state('');
  let done = $state(false);
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      await resetPassword(token, newPassword);
      done = true;
      setTimeout(() => push('/login'), 1500);
    } catch (err) {
      error = err instanceof Error ? err.message : $_('auth.resetPassword');
    } finally {
      busy = false;
    }
  }
</script>

<div class="auth-card">
  <h1>{$_('auth.resetPassword')}</h1>
  {#if done}
    <p class="ok">{$_('auth.resetDone')}</p>
  {:else}
    <form onsubmit={submit}>
      <label>
        {$_('auth.resetToken')}
        <input bind:value={token} required />
      </label>
      <label>
        {$_('auth.newPassword')}
        <input type="password" bind:value={newPassword} autocomplete="new-password" required />
        <span class="hint">{$_('auth.passwordHint')}</span>
      </label>
      {#if error}<p class="error">{error}</p>{/if}
      <button type="submit" disabled={busy}>{$_('auth.resetPassword')}</button>
    </form>
  {/if}
  <p class="alt"><a href="#/login">{$_('auth.login')}</a></p>
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
  .hint {
    color: var(--gw-muted);
    font-size: 0.75rem;
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
  .ok {
    color: #16a34a;
  }
  .alt {
    font-size: 0.85rem;
    margin-top: 1rem;
  }
</style>