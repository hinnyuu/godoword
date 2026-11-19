<script lang="ts">
  import { _ } from 'svelte-i18n';
  import {
    getConfig,
    listProviders,
    createProvider,
    deleteProvider,
    providerModels,
    providerVoices,
    previewVoice,
    listBindings,
    createBinding,
    deleteBinding,
    getAudioPcm,
    getAudioMeta,
    updateMe,
    changePassword,
    AudioEngine,
    type Provider,
    type Binding,
    type ConfigView,
    type VoiceInfo,
  } from '@godoword/core';
  import workletUrl from '@godoword/core/audio/worklet?worker&url';
  import { prefs } from '../lib/prefs.svelte';
  import { auth } from '../lib/auth.svelte';
  import { PRESETS } from '@godoword/core';

  let cfg = $state<ConfigView | null>(null);
  let providers = $state<Provider[]>([]);
  let bindings = $state<Binding[]>([]);
  let error = $state('');
  let advanced = $state(false);

  // 新建 provider 表单。
  let presetId = $state('');
  let adapterId = $state('llm-openai');
  let label = $state('');
  let baseUrl = $state('');
  let key = $state('');

  // 音色试听。
  let voices = $state<VoiceInfo[]>([]);
  let previewProviderId = $state('');
  let previewModel = $state('qwen-audio-3.0-tts-plus');
  let previewVoiceId = $state('longanlingxin');
  let previewText = $state('');
  let previewStatus = $state('');
  let engine: AudioEngine | null = null;

  // 主题自定义。
  let accent = $state('#2563eb');
  let customCss = $state('');

  // 账户自助。
  let displayName = $state('');
  let oldPassword = $state('');
  let newPassword = $state('');
  let accountMsg = $state('');

  async function refresh() {
    try {
      [cfg, providers, bindings] = await Promise.all([getConfig(), listProviders(), listBindings()]);
      customCss = prefs.prefs.custom_css ?? '';
      displayName = auth.user?.display_name ?? '';
    } catch (err) {
      error = err instanceof Error ? err.message : '加载失败';
    }
  }

  $effect(() => {
    void refresh();
  });

  function applyPreset() {
    const p = cfg?.provider_presets.find((x) => x.id === presetId);
    if (p) {
      adapterId = p.adapter_id;
      label = p.label;
      baseUrl = p.base_url;
    }
  }

  async function addProvider(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    try {
      await createProvider(adapterId, label, baseUrl, key);
      key = '';
      label = '';
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : '创建失败';
    }
  }

  async function removeProvider(id: string) {
    error = '';
    try {
      await deleteProvider(id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : '删除失败';
    }
  }

  async function testModels(id: string) {
    error = '';
    try {
      const models = await providerModels(id);
      previewStatus = `模型：${models.map((m) => m.id).join(', ') || '（空）'}`;
    } catch (err) {
      error = err instanceof Error ? err.message : '测试失败';
    }
  }

  async function loadVoices() {
    error = '';
    try {
      voices = await providerVoices(previewProviderId, previewModel);
      if (voices.length > 0) previewVoiceId = voices[0]!.id;
    } catch (err) {
      error = err instanceof Error ? err.message : '拉取音色失败';
    }
  }

  async function doPreview() {
    error = '';
    previewStatus = '合成中…';
    try {
      if (!engine) engine = new AudioEngine({ workletUrl });
      await engine.init();
      engine.stop();
      const audioId = await previewVoice(previewProviderId, previewModel, previewVoiceId, previewText || undefined);
      const meta = await getAudioMeta(audioId);
      const { bytes } = await getAudioPcm(audioId);
      engine.setWords(meta.words);
      engine.pushPcm(bytes);
      await engine.play();
      previewStatus = '试听播放中';
    } catch (err) {
      error = err instanceof Error ? err.message : '试听失败';
      previewStatus = '';
    }
  }

  // 简化绑定卡片：找 (*, cap, default) 绑定。
  function defaultBinding(cap: string): Binding | undefined {
    return bindings.find((b) => b.module_id === '*' && b.capability === cap && b.role === 'default');
  }

  function ttsVoice(): string {
    const params = defaultBinding('tts')?.params_json as { voice?: string } | undefined;
    return params?.voice ?? '';
  }

  async function setDefaultBinding(cap: string, providerId: string, model: string, voice?: string) {
    error = '';
    try {
      const existing = defaultBinding(cap);
      if (existing) await deleteBinding(existing.id);
      await createBinding(cap, 'default', providerId, model, voice ? { voice } : undefined);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : '绑定失败';
    }
  }

  async function applyThemePreset(id: string) {
    await prefs.update({ theme: id });
  }

  async function applyAccent() {
    await prefs.update({ theme_vars: { '--gw-accent': accent } });
  }

  async function applyCustomCss() {
    await prefs.update({ custom_css: customCss || null });
  }

  async function resetTheme() {
    customCss = '';
    accent = '#2563eb';
    await prefs.update({ theme: 'light', theme_vars: null, custom_css: null });
  }

  async function saveDisplayName() {
    accountMsg = '';
    try {
      auth.user = await updateMe(displayName);
      accountMsg = '显示名已保存';
    } catch (err) {
      accountMsg = err instanceof Error ? err.message : '保存失败';
    }
  }

  async function doChangePassword(e: SubmitEvent) {
    e.preventDefault();
    accountMsg = '';
    try {
      await changePassword(oldPassword, newPassword);
      oldPassword = '';
      newPassword = '';
      accountMsg = '密码已修改（其他设备会话已失效）';
    } catch (err) {
      accountMsg = err instanceof Error ? err.message : '改密失败';
    }
  }

  let llmProviders = $derived(providers.filter((p) => p.adapter_id === 'llm-openai'));
  let ttsProviders = $derived(providers.filter((p) => p.adapter_id === 'tts-qwen-audio'));
</script>

<section class="settings">
  <h1>{$_('nav.settings')}</h1>
  {#if error}<p class="error">{error}</p>{/if}

  <!-- 简化三卡片 -->
  <div class="cards">
    <div class="card">
      <h2>{$_('settings.defaultLlm')}</h2>
      {#if defaultBinding('llm')}
        <p class="bound">{defaultBinding('llm')!.model}</p>
      {:else}
        <p class="unbound">未配置</p>
      {/if}
      <select
        onchange={(e) => {
          const p = llmProviders.find((x) => x.id === e.currentTarget.value);
          if (p) void setDefaultBinding('llm', p.id, 'qwen4-max-preview');
        }}
      >
        <option value="">选择服务商…</option>
        {#each llmProviders as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
    </div>

    <div class="card">
      <h2>{$_('settings.defaultTts')}</h2>
      {#if defaultBinding('tts')}
        <p class="bound">{defaultBinding('tts')!.model} · {ttsVoice()}</p>
      {:else}
        <p class="unbound">未配置</p>
      {/if}
      <select
        onchange={(e) => {
          const p = ttsProviders.find((x) => x.id === e.currentTarget.value);
          if (p) void setDefaultBinding('tts', p.id, 'qwen-audio-3.0-tts-plus', 'longanlingxin');
        }}
      >
        <option value="">选择服务商…</option>
        {#each ttsProviders as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
    </div>
  </div>

  <!-- 音色试听 -->
  <div class="panel">
    <h2>{$_('settings.preview')}</h2>
    <div class="row">
      <select bind:value={previewProviderId} onchange={loadVoices}>
        <option value="">TTS 服务商…</option>
        {#each ttsProviders as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
      <select bind:value={previewVoiceId}>
        {#each voices as v (v.id)}
          <option value={v.id}>{v.name}（{v.id}）</option>
        {/each}
      </select>
      <input placeholder="自定义文本（可选）" bind:value={previewText} />
      <button onclick={doPreview} disabled={!previewProviderId}>{$_('settings.preview')}</button>
    </div>
    {#if previewStatus}<p class="status">{previewStatus}</p>{/if}
  </div>

  <!-- 主题面板 -->
  <div class="panel">
    <h2>{$_('settings.theme')}</h2>
    <div class="row">
      {#each PRESETS as p (p.id)}
        <button class:active={prefs.prefs.theme === p.id} onclick={() => applyThemePreset(p.id)}>{p.name}</button>
      {/each}
      <label>强调色 <input type="color" bind:value={accent} onchange={applyAccent} /></label>
      <button onclick={resetTheme}>{$_('settings.resetTheme')}</button>
    </div>
    <label class="css-label">
      {$_('settings.customCss')}
      <textarea bind:value={customCss} onblur={applyCustomCss} rows="3"></textarea>
    </label>
  </div>

  <!-- 偏好开关 -->
  <div class="panel">
    <h2>偏好</h2>
    <label class="toggle">
      <input
        type="checkbox"
        checked={prefs.prefs.auto_play ?? false}
        onchange={(e) => prefs.update({ auto_play: e.currentTarget.checked })}
      />
      {$_('settings.autoPlay')}
    </label>
    <label class="toggle">
      <input
        type="checkbox"
        checked={prefs.prefs.on_disconnect_default === 'continue'}
        onchange={(e) => prefs.update({ on_disconnect_default: e.currentTarget.checked ? 'continue' : 'cancel' })}
      />
      {$_('settings.onDisconnect')}
    </label>
  </div>

  <!-- 账户自助 -->
  <div class="panel">
    <h2>账户</h2>
    <div class="row">
      <label>{$_('auth.displayName')} <input bind:value={displayName} /></label>
      <button onclick={saveDisplayName}>保存</button>
    </div>
    <form onsubmit={doChangePassword} class="row">
      <input type="password" placeholder={$_('auth.oldPassword')} bind:value={oldPassword} autocomplete="current-password" required />
      <input type="password" placeholder={$_('auth.newPassword')} bind:value={newPassword} autocomplete="new-password" required />
      <button type="submit">{$_('auth.changePassword')}</button>
    </form>
    {#if accountMsg}<p class="status">{accountMsg}</p>{/if}
  </div>

  <!-- provider 管理 -->
  <div class="panel">
    <h2>{$_('settings.providers')}</h2>
    <form onsubmit={addProvider} class="provider-form">
      <select bind:value={presetId} onchange={applyPreset}>
        <option value="">预设模板…</option>
        {#each cfg?.provider_presets ?? [] as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
      <select bind:value={adapterId}>
        <option value="llm-openai">llm-openai</option>
        <option value="tts-qwen-audio">tts-qwen-audio</option>
      </select>
      <input placeholder="标签" bind:value={label} required />
      <input placeholder={$_('settings.baseUrl')} bind:value={baseUrl} required />
      <input type="password" placeholder={$_('settings.apiKey')} bind:value={key} required />
      <button type="submit">添加</button>
    </form>
    <p class="hint">{$_('settings.apiKeyHint')}</p>
    <table>
      <tbody>
        {#each providers as p (p.id)}
          <tr>
            <td>{p.label}</td>
            <td>{p.adapter_id}</td>
            <td class="actions">
              <button onclick={() => testModels(p.id)}>{$_('settings.testConnection')}</button>
              <button onclick={() => removeProvider(p.id)}>删除</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <!-- 高级绑定矩阵 -->
  <div class="panel">
    <button class="advanced-toggle" onclick={() => (advanced = !advanced)}>
      {$_('settings.advanced')} {advanced ? '▲' : '▼'}
    </button>
    {#if advanced}
      <table>
        <thead>
          <tr><th>module</th><th>cap</th><th>role</th><th>model</th><th></th></tr>
        </thead>
        <tbody>
          {#each bindings as b (b.id)}
            <tr>
              <td>{b.module_id}</td>
              <td>{b.capability}</td>
              <td>{b.role}</td>
              <td>{b.model}</td>
              <td><button onclick={() => deleteBinding(b.id).then(refresh)}>删除</button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</section>

<style>
  .settings {
    padding: 1rem;
    max-width: 56rem;
    margin: 0 auto;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
    gap: 1rem;
    margin: 1rem 0;
  }
  .card,
  .panel {
    border: 1px solid var(--gw-border);
    border-radius: var(--gw-radius);
    background: var(--gw-surface);
    padding: 1rem;
    margin: 1rem 0;
  }
  .card h2,
  .panel h2 {
    margin: 0 0 0.5rem;
    font-size: 1rem;
  }
  .bound {
    color: var(--gw-accent);
    font-size: 0.85rem;
  }
  .unbound {
    color: var(--gw-muted);
    font-size: 0.85rem;
  }
  .row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
    margin: 0.5rem 0;
  }
  select,
  input,
  textarea {
    padding: 0.4rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-bg);
    color: var(--gw-fg);
  }
  button {
    padding: 0.4rem 0.8rem;
    border: 1px solid var(--gw-border);
    border-radius: 4px;
    background: var(--gw-accent);
    color: #fff;
    cursor: pointer;
    font-size: 0.85rem;
  }
  button.active {
    outline: 2px solid var(--gw-fg);
  }
  .provider-form {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin: 0.5rem 0;
  }
  .hint {
    color: var(--gw-muted);
    font-size: 0.75rem;
  }
  .status {
    color: var(--gw-muted);
    font-size: 0.85rem;
  }
  .error {
    color: #dc2626;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    margin-top: 0.5rem;
  }
  td,
  th {
    text-align: left;
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid var(--gw-border);
    font-size: 0.85rem;
  }
  .actions {
    display: flex;
    gap: 0.3rem;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0.5rem 0;
    font-size: 0.9rem;
  }
  .css-label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.85rem;
    margin-top: 0.5rem;
  }
  textarea {
    font-family: monospace;
    width: 100%;
  }
  .advanced-toggle {
    background: transparent;
    color: var(--gw-fg);
  }
</style>