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
    renameFolder,
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
      error = err instanceof Error ? err.message : $_('session.loadFailed');
    }
  }

  $effect(() => {
    void refresh();
  });

  // 树 = 登记表 ∪ 会话路径前缀（设计 Q11：会话路径亦渲染文件夹节点，防孤儿路径隐没）。
  let tree = $derived(
    buildTree([
      ...folders.map((f) => f.path),
      ...sessions.map((s) => s.path).filter((p): p is string => p != null),
    ]),
  );

  let collapsed = $derived(prefs.prefs.sidebar_collapsed ?? false);

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
      error = err instanceof Error ? err.message : $_('session.createFailed');
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
      error = err instanceof Error ? err.message : $_('session.deleteFailed');
    }
  }

  async function move(id: string) {
    const target = prompt($_('session.movePrompt'), '');
    if (target === null) return;
    error = '';
    try {
      await moveSession(id, target.trim() === '' ? null : target.trim());
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('session.moveFailed');
    }
  }

  async function newFolder() {
    const path = prompt($_('session.newFolderPrompt'), '');
    if (!path || path.trim() === '') return;
    error = '';
    try {
      await createFolder(path.trim());
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('session.folderCreateFailed');
    }
  }

  async function rename(node: TreeNode) {
    const target = prompt($_('session.renamePrompt'), node.path);
    if (target === null || target.trim() === '' || target.trim() === node.path) return;
    error = '';
    try {
      // 隐式祖先节点（旧数据可能未登记）：先登记再改名。
      let f = folders.find((x) => x.path === node.path);
      if (!f) f = await createFolder(node.path);
      await renameFolder(f.id, target.trim());
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('session.renameFailed');
    }
  }

  async function removeFolder(path: string) {
    const f = folders.find((x) => x.path === path);
    if (!f) return;
    if (!confirm($_('session.folderDeleteConfirm', { values: { path } }))) return;
    error = '';
    try {
      await deleteFolder(f.id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('session.folderDeleteFailed');
    }
  }

  function toggle(path: string) {
    expanded[path] = !expanded[path];
  }

  function toggleCollapse() {
    void prefs.update({ sidebar_collapsed: !collapsed });
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
        <button class="icon" onclick={() => rename(node)} title={$_('session.rename')}>✎</button>
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

<aside class="sidebar" class:collapsed>
  <div class="sidebar-head">
    <button class="collapse" onclick={toggleCollapse} title={collapsed ? $_('session.expand') : $_('session.collapse')}>
      {collapsed ? '»' : '«'}
    </button>
    {#if !collapsed}
      <button onclick={newSession}>+ {$_('session.new')}</button>
      <button onclick={newFolder} title={$_('session.newFolder')}>📁</button>
    {/if}
  </div>
  {#if !collapsed}
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
  {/if}
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
    flex-shrink: 0;
  }
  .sidebar.collapsed {
    width: 2.5rem;
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
  .sidebar-head .collapse {
    flex: 0 0 auto;
    background: transparent;
    color: var(--gw-muted);
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