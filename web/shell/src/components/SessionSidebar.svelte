<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { push } from 'svelte-spa-router';
  import {
    listFolders,
    listSessions,
    createSession,
    deleteSession,
    moveSession,
    createFolder,
    deleteFolder,
    buildTree,
    type Folder,
    type SessionListItem,
    type TreeNode,
  } from '@godoword/core';
  import { prefs } from '../lib/prefs.svelte';

  let folders = $state<Folder[]>([]);
  let sessions = $state<SessionListItem[]>([]);
  let error = $state('');
  let expanded = $state<Record<string, boolean>>({});

  async function refresh() {
    try {
      [folders, sessions] = await Promise.all([listFolders(), listSessions()]);
    } catch (err) {
      error = err instanceof Error ? err.message : '加载失败';
    }
  }

  $effect(() => {
    void refresh();
  });

  let tree = $derived(buildTree(folders.map((f) => f.path)));

  function sessionsAt(path: string | null): SessionListItem[] {
    return sessions.filter((s) => (s.path ?? null) === path);
  }

  async function newSession() {
    error = '';
    try {
      const id = await createSession(prefs.prefs.on_disconnect_default ?? undefined);
      await refresh();
      push(`/chat/${id}`);
    } catch (err) {
      error = err instanceof Error ? err.message : '创建失败';
    }
  }

  async function open(id: string) {
    push(`/chat/${id}`);
  }

  async function removeSession(id: string) {
    if (!confirm($_('session.deleteConfirm'))) return;
    error = '';
    try {
      await deleteSession(id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : '删除失败';
    }
  }

  async function move(id: string) {
    const target = prompt($_('session.moveTo'), '/');
    if (target === null) return;
    error = '';
    try {
      await moveSession(id, target.trim() === '' ? null : target.trim());
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : '移动失败';
    }
  }

  async function newFolder() {
    const path = prompt($_('session.newFolder'), '/');
    if (!path) return;
    error = '';
    try {
      await createFolder(path);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : '创建文件夹失败';
    }
  }

  async function removeFolder(path: string) {
    const f = folders.find((x) => x.path === path);
    if (!f) return;
    if (!confirm(`删除文件夹 ${path}？会话上移一层，不会删除会话。`)) return;
    error = '';
    try {
      await deleteFolder(f.id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : '删除文件夹失败';
    }
  }

  function toggle(path: string) {
    expanded[path] = !expanded[path];
  }
</script>

{#snippet sessionRow(s: SessionListItem)}
  <div class="session-row">
    <button class="open" onclick={() => open(s.id)} title={s.title}>{s.title}</button>
    <span class="meta">{s.message_count}</span>
    <button class="icon" onclick={() => move(s.id)} title={$_('session.moveTo')}>↪</button>
    <button class="icon" onclick={() => removeSession(s.id)} title={$_('session.delete')}>✕</button>
  </div>
{/snippet}

{#snippet folderNode(nodes: TreeNode[])}
  {#each nodes as node (node.path)}
    <div class="folder">
      <div class="folder-head">
        <button class="toggle" onclick={() => toggle(node.path)}>{expanded[node.path] ? '▼' : '▶'}</button>
        <button class="folder-name" onclick={() => toggle(node.path)}>{node.name}</button>
        <button class="icon" onclick={() => removeFolder(node.path)} title={$_('session.delete')}>✕</button>
      </div>
      {#if expanded[node.path]}
        <div class="folder-body">
          {@render folderNode(node.children)}
          {#each sessionsAt(node.path) as s (s.id)}
            {@render sessionRow(s)}
          {/each}
        </div>
      {/if}
    </div>
  {/each}
{/snippet}

<aside class="sidebar">
  <div class="sidebar-head">
    <button onclick={newSession}>+ {$_('session.new')}</button>
    <button onclick={newFolder} title={$_('session.newFolder')}>📁</button>
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  <div class="tree">
    {@render folderNode(tree)}
    <div class="folder">
      <div class="folder-head"><span class="folder-name">{$_('session.uncategorized')}</span></div>
      <div class="folder-body">
        {#each sessionsAt(null) as s (s.id)}
          {@render sessionRow(s)}
        {/each}
      </div>
    </div>
  </div>
</aside>

<style>
  .sidebar {
    width: 16rem;
    border-right: 1px solid var(--gw-border);
    background: var(--gw-surface);
    display: flex;
    flex-direction: column;
    height: calc(100vh - 8rem);
    overflow-y: auto;
  }
  .sidebar-head {
    display: flex;
    gap: 0.5rem;
    padding: 0.5rem;
    border-bottom: 1px solid var(--gw-border);
  }
  .sidebar-head button {
    flex: 1;
    padding: 0.4rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-accent);
    color: #fff;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .tree {
    padding: 0.5rem;
  }
  .folder-head {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.2rem 0;
  }
  .folder-name {
    flex: 1;
    cursor: pointer;
    font-size: 0.85rem;
    font-weight: 600;
    background: transparent;
    border: none;
    text-align: left;
    color: var(--gw-fg);
    padding: 0;
  }
  .folder-body {
    padding-left: 0.75rem;
  }
  .toggle,
  .icon {
    background: transparent;
    border: none;
    color: var(--gw-muted);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.1rem 0.2rem;
  }
  .session-row {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.15rem 0;
  }
  .session-row .open {
    flex: 1;
    text-align: left;
    background: transparent;
    border: none;
    color: var(--gw-fg);
    cursor: pointer;
    font-size: 0.8rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .session-row .open:hover {
    color: var(--gw-accent);
  }
  .meta {
    color: var(--gw-muted);
    font-size: 0.7rem;
  }
  .error {
    color: #dc2626;
    font-size: 0.75rem;
    padding: 0 0.5rem;
  }
</style>