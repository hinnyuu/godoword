<script lang="ts">
  import Router, { push, location } from 'svelte-spa-router';
  import { _ } from 'svelte-i18n';
  import { setUnauthorizedHandler } from '@godoword/core';
  import './lib/i18n';
  import { auth } from './lib/auth.svelte';
  import { prefs } from './lib/prefs.svelte';
  import Login from './routes/Login.svelte';
  import Register from './routes/Register.svelte';
  import Reset from './routes/Reset.svelte';
  import Home from './routes/Home.svelte';
  import Sessions from './routes/Sessions.svelte';
  import Settings from './routes/Settings.svelte';
  import Admin from './routes/Admin.svelte';
  import Onboarding from './routes/Onboarding.svelte';

  setUnauthorizedHandler(() => push('/login'));

  const routes = {
    '/login': Login,
    '/register': Register,
    '/reset': Reset,
    '/': Home,
    '/chat/:id': Home,
    '/sessions': Sessions,
    '/settings': Settings,
    '/admin': Admin,
    '/onboarding': Onboarding,
    '*': Login,
  };

  const publicRoutes = ['/login', '/register', '/reset'];

  // 启动加载用户（一次）。
  $effect(() => {
    void auth.load();
  });

  // 偏好随登录态加载（登录后重载服务端偏好；登出由 logout 重置）。
  // 修复：仅在挂载加载会在未登录时 401→默认，登录后不重载→主题丢失。
  $effect(() => {
    if (auth.user) void prefs.load();
  });

  // 认证门：未登录访问受保护路由 → 跳登录。
  $effect(() => {
    if (!auth.loading && !auth.user && !publicRoutes.includes($location ?? '')) {
      push('/login');
    }
  });

  async function logout() {
    await auth.logout();
    prefs.reset();
    push('/login');
  }

  let isPublic = $derived(publicRoutes.includes($location ?? ''));
</script>

{#if auth.loading}
  <p class="loading">加载中…</p>
{:else}
  {#if auth.user && !isPublic}
    <nav>
      <a href="#/">{$_('nav.chat')}</a>
      <a href="#/sessions">{$_('nav.sessions')}</a>
      <a href="#/settings">{$_('nav.settings')}</a>
      {#if auth.isAdmin}<a href="#/admin">{$_('nav.admin')}</a>{/if}
      <span class="spacer"></span>
      <span class="user">{auth.user.display_name ?? auth.user.username}</span>
      <button onclick={logout}>{$_('auth.logout')}</button>
    </nav>
  {/if}
  <Router {routes} />
  <footer>
    {$_('app.license')} ·
    <a href="https://github.com/hinnyuu/godoword" target="_blank" rel="noopener">{$_('app.source')}</a>
  </footer>
{/if}

<style>
  nav {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--gw-border);
    background: var(--gw-surface);
    flex-wrap: wrap;
  }
  nav a {
    text-decoration: none;
    color: var(--gw-fg);
  }
  nav a:hover {
    color: var(--gw-accent);
  }
  .spacer {
    flex: 1;
  }
  .user {
    color: var(--gw-muted);
    font-size: 0.9rem;
  }
  nav button {
    padding: 0.3rem 0.7rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: transparent;
    color: var(--gw-fg);
    cursor: pointer;
  }
  .loading {
    padding: 2rem;
    color: var(--gw-muted);
  }
  footer {
    padding: 1rem;
    font-size: 0.8rem;
    color: var(--gw-muted);
    border-top: 1px solid var(--gw-border);
    margin-top: 2rem;
  }
</style>