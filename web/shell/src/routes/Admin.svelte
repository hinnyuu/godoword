<script lang="ts">
  import { _ } from 'svelte-i18n';
  import {
    adminUsers,
    adminSetStatus,
    adminCreateInvite,
    adminResetToken,
    type UserInfo,
  } from '@godoword/core';
  import { auth } from '../lib/auth.svelte';

  let users = $state<UserInfo[]>([]);
  let error = $state('');
  let maxUses = $state(1);
  let inviteCode = $state('');
  let resetToken = $state('');

  async function refresh() {
    try {
      users = await adminUsers();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.loadFailed');
    }
  }

  $effect(() => {
    if (auth.isAdmin) void refresh();
  });

  async function toggleStatus(u: UserInfo) {
    error = '';
    try {
      await adminSetStatus(u.id, u.status === 'active' ? 'disabled' : 'active');
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function genInvite() {
    error = '';
    try {
      const r = await adminCreateInvite(maxUses);
      inviteCode = r.code;
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function genReset(u: UserInfo) {
    error = '';
    resetToken = '';
    try {
      resetToken = await adminResetToken(u.id);
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }
</script>

<section>
  <h1>{$_('nav.admin')}</h1>
  {#if !auth.isAdmin}
    <p class="error">{$_('admin.needAdmin')}</p>
  {:else}
    {#if error}<p class="error">{error}</p>{/if}

    <h2>{$_('admin.invites')}</h2>
    <div class="row">
      <label>{$_('admin.maxUses')} <input type="number" min="1" bind:value={maxUses} /></label>
      <button onclick={genInvite}>{$_('admin.createInvite')}</button>
    </div>
    {#if inviteCode}<p class="token">{$_('auth.inviteCode')}: <code>{inviteCode}</code></p>{/if}
    {#if resetToken}<p class="token">{$_('auth.resetToken')}: <code>{resetToken}</code></p>{/if}

    <h2>{$_('admin.users')}</h2>
    <table>
      <thead>
        <tr>
          <th>{$_('auth.username')}</th>
          <th>{$_('auth.displayName')}</th>
          <th>{$_('admin.status')}</th>
          <th>{$_('admin.admin')}</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each users as u (u.id)}
          <tr>
            <td>{u.username ?? '—'}</td>
            <td>{u.display_name ?? '—'}</td>
            <td>{u.status}</td>
            <td>{u.is_admin ? '✓' : ''}</td>
            <td class="actions">
              <button onclick={() => toggleStatus(u)}>
                {u.status === 'active' ? $_('admin.disable') : $_('admin.enable')}
              </button>
              <button onclick={() => genReset(u)}>{$_('admin.resetToken')}</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  section {
    padding: 1rem;
  }
  .row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin: 0.5rem 0;
  }
  input {
    padding: 0.3rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-bg);
    color: var(--gw-fg);
    width: 5rem;
  }
  button {
    padding: 0.3rem 0.7rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-accent);
    color: #fff;
    cursor: pointer;
    font-size: 0.85rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    margin-top: 0.5rem;
  }
  th,
  td {
    text-align: left;
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid var(--gw-border);
  }
  .actions {
    display: flex;
    gap: 0.3rem;
  }
  .error {
    color: #dc2626;
  }
  .token code {
    background: var(--gw-bg);
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
    word-break: break-all;
  }
</style>