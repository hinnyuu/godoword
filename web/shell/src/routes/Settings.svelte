<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onDestroy } from 'svelte';
  import {
    getConfig,
    listProviders,
    createProvider,
    updateProvider,
    deleteProvider,
    providerModels,
    providerVoices,
    previewVoice,
    listBindings,
    createBinding,
    updateBinding,
    deleteBinding,
    getAudioPcm,
    getAudioMeta,
    updateMe,
    changePassword,
    AudioEngine,
    PRESETS,
    type Provider,
    type Binding,
    type ConfigView,
    type VoiceInfo,
  } from '@godoword/core';
  import workletUrl from '@godoword/core/audio/worklet?worker&url';
  import { prefs } from '../lib/prefs.svelte';
  import { auth } from '../lib/auth.svelte';

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

  // provider 编辑（换 key 不删建，保绑定）。
  let editId = $state('');
  let editLabel = $state('');
  let editBaseUrl = $state('');
  let editKey = $state('');

  // 三卡片（Q3：默认 LLM / 默认 TTS+音色 / 可选摘要模型）。
  let llmProviderId = $state('');
  let llmModel = $state('');
  let ttsProviderId = $state('');
  let ttsModel = $state('');
  let ttsVoice = $state('');
  let cardVoices = $state<VoiceInfo[]>([]);
  let sumProviderId = $state('');
  let sumModel = $state('');

  // 音色试听。
  let voices = $state<VoiceInfo[]>([]);
  let previewProviderId = $state('');
  let previewModel = $state('qwen-audio-3.0-tts-plus');
  let previewVoiceId = $state('longanlingxin');
  let previewText = $state('');
  let previewStatus = $state('');
  let testStatus = $state('');
  let engine: AudioEngine | null = null;

  // 二审 R2-5：卸载释放试听音频资源（防 AudioContext 泄漏）。
  onDestroy(() => {
    engine?.dispose();
  });

  // 主题自定义。
  let accent = $state('#2563eb');
  let customCss = $state('');

  // 账户自助。
  let displayName = $state('');
  let oldPassword = $state('');
  let newPassword = $state('');
  let accountMsg = $state('');

  // 高级绑定矩阵。
  let advModule = $state('*');
  let advCap = $state('llm');
  let advRole = $state('default');
  let advProvider = $state('');
  let advModel = $state('');
  let advVoice = $state('');
  let rowModels = $state<Record<string, string>>({});
  let rowProviders = $state<Record<string, string>>({});

  function bindingFor(cap: string, role = 'default'): Binding | undefined {
    return bindings.find((b) => b.module_id === '*' && b.capability === cap && b.role === role);
  }

  async function refresh() {
    try {
      [cfg, providers, bindings] = await Promise.all([getConfig(), listProviders(), listBindings()]);
      customCss = prefs.prefs.custom_css ?? '';
      displayName = auth.user?.display_name ?? '';
      const llm = bindingFor('llm');
      llmProviderId = llm?.provider_id ?? '';
      const llmPreset = cfg?.provider_presets.find((p) => p.capability === 'llm');
      llmModel = llm?.model ?? llmPreset?.suggested_models?.at(0) ?? '';
      const tts = bindingFor('tts');
      ttsProviderId = tts?.provider_id ?? '';
      const ttsPreset = cfg?.provider_presets.find((p) => p.capability === 'tts');
      ttsModel = tts?.model ?? ttsPreset?.suggested_models?.at(0) ?? '';
      ttsVoice = (tts?.params_json as { voice?: string } | undefined)?.voice ?? '';
      const sum = bindingFor('llm', 'summarizer');
      sumProviderId = sum?.provider_id ?? '';
      sumModel = sum?.model ?? llmModel;
      rowModels = Object.fromEntries(bindings.map((b) => [b.id, b.model]));
      rowProviders = Object.fromEntries(bindings.map((b) => [b.id, b.provider_id]));
      if (ttsProviderId) await loadCardVoices();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.loadFailed');
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
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  function startEdit(p: Provider) {
    editId = p.id;
    editLabel = p.label;
    editBaseUrl = p.base_url;
    editKey = '';
  }

  async function saveEdit(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    try {
      await updateProvider(editId, editLabel, editBaseUrl, editKey || undefined);
      editId = '';
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function removeProvider(id: string) {
    if (!confirm($_('settings.providerDeleteConfirm'))) return;
    error = '';
    try {
      await deleteProvider(id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function testConnection(p: Provider) {
    error = '';
    testStatus = '';
    try {
      if (p.adapter_id === 'tts-qwen-audio') {
        const vs = await providerVoices(p.id);
        testStatus = `${p.label} ${$_('settings.testVoices')}：${vs.map((v) => v.name).join(', ') || $_('settings.empty')}`;
      } else {
        const ms = await providerModels(p.id);
        testStatus = `${p.label} ${$_('settings.testModels')}：${ms.map((m) => m.id).join(', ') || $_('settings.empty')}`;
      }
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function loadCardVoices() {
    try {
      cardVoices = await providerVoices(ttsProviderId, ttsModel || undefined);
      if (!ttsVoice && cardVoices.length > 0) ttsVoice = cardVoices[0]!.id;
    } catch {
      cardVoices = [];
    }
  }

  async function loadVoices() {
    error = '';
    try {
      voices = await providerVoices(previewProviderId, previewModel);
      if (voices.length > 0) previewVoiceId = voices[0]!.id;
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function doPreview() {
    error = '';
    previewStatus = $_('settings.previewSynthesizing');
    try {
      if (!engine) engine = new AudioEngine({ workletUrl });
      await engine.init();
      engine.stop();
      const audioId = await previewVoice(previewProviderId, previewModel, previewVoiceId, previewText || undefined);
      const meta = await getAudioMeta(audioId);
      const { bytes } = await getAudioPcm(audioId);
      engine.setWords(meta.words);
      engine.pushPcm(bytes);
      engine.markComplete();
      await engine.play();
      previewStatus = $_('settings.previewPlaying');
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
      previewStatus = '';
    }
  }

  // 简化卡片绑定：存在则 update（不删建，防中途失败丢绑定），否则 create。
  async function setDefaultBinding(cap: string, role: string, providerId: string, model: string, params?: Record<string, unknown>) {
    error = '';
    try {
      const existing = bindingFor(cap, role);
      if (existing) {
        await updateBinding(existing.id, providerId, model, params ?? (existing.params_json as Record<string, unknown>));
      } else {
        await createBinding(cap, role, providerId, model, params);
      }
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function bindLlm() {
    if (llmProviderId && llmModel) await setDefaultBinding('llm', 'default', llmProviderId, llmModel);
  }

  async function bindTts() {
    if (ttsProviderId && ttsModel) {
      await setDefaultBinding('tts', 'default', ttsProviderId, ttsModel, ttsVoice ? { voice: ttsVoice } : {});
    }
  }

  async function bindSummarizer() {
    if (sumProviderId && sumModel) await setDefaultBinding('llm', 'summarizer', sumProviderId, sumModel);
  }

  async function unbindSummarizer() {
    const sum = bindingFor('llm', 'summarizer');
    if (!sum) return;
    error = '';
    try {
      await deleteBinding(sum.id);
      sumProviderId = '';
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
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
      accountMsg = $_('auth.displayNameSaved');
    } catch (err) {
      accountMsg = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function doChangePassword(e: SubmitEvent) {
    e.preventDefault();
    accountMsg = '';
    try {
      await changePassword(oldPassword, newPassword);
      oldPassword = '';
      newPassword = '';
      accountMsg = $_('auth.passwordChanged');
    } catch (err) {
      accountMsg = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function createAdvanced(e: SubmitEvent) {
    e.preventDefault();
    if (!advProvider || !advModel) return;
    error = '';
    try {
      const params = advCap === 'tts' && advVoice ? { voice: advVoice } : undefined;
      await createBinding(advCap, advRole, advProvider, advModel, params, advModule || '*');
      advModel = '';
      advVoice = '';
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function saveRow(b: Binding) {
    error = '';
    try {
      await updateBinding(b.id, rowProviders[b.id] ?? b.provider_id, rowModels[b.id] ?? b.model, b.params_json as Record<string, unknown>);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  async function removeRow(b: Binding) {
    error = '';
    try {
      await deleteBinding(b.id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : $_('common.opFailed');
    }
  }

  let llmProviders = $derived(providers.filter((p) => p.adapter_id === 'llm-openai'));
  let ttsProviders = $derived(providers.filter((p) => p.adapter_id === 'tts-qwen-audio'));
</script>

<section class="settings">
  <h1>{$_('nav.settings')}</h1>
  {#if error}<p class="error">{error}</p>{/if}

  <!-- 简化三卡片（Q3） -->
  <div class="cards">
    <div class="card">
      <h2>{$_('settings.defaultLlm')}</h2>
      {#if bindingFor('llm')}
        <p class="bound">{bindingFor('llm')!.model}</p>
      {:else}
        <p class="unbound">{$_('settings.unbound')}</p>
      {/if}
      <select bind:value={llmProviderId}>
        <option value="">{$_('settings.selectProvider')}</option>
        {#each llmProviders as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
      <input placeholder={$_('settings.model')} bind:value={llmModel} />
      <button onclick={bindLlm} disabled={!llmProviderId || !llmModel}>{$_('common.bind')}</button>
    </div>

    <div class="card">
      <h2>{$_('settings.defaultTts')}</h2>
      {#if bindingFor('tts')}
        <p class="bound">{bindingFor('tts')!.model} · {ttsVoice}</p>
      {:else}
        <p class="unbound">{$_('settings.unbound')}</p>
      {/if}
      <select bind:value={ttsProviderId} onchange={loadCardVoices}>
        <option value="">{$_('settings.selectProvider')}</option>
        {#each ttsProviders as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
      <select bind:value={ttsVoice}>
        <option value="">{$_('settings.voice')}</option>
        {#each cardVoices as v (v.id)}
          <option value={v.id}>{v.name}（{v.id}）</option>
        {/each}
      </select>
      <input placeholder={$_('settings.model')} bind:value={ttsModel} />
      <button onclick={bindTts} disabled={!ttsProviderId || !ttsModel}>{$_('common.bind')}</button>
    </div>

    <div class="card">
      <h2>{$_('settings.summarizer')}</h2>
      <p class="hint">{$_('settings.summarizerHint')}</p>
      {#if bindingFor('llm', 'summarizer')}
        <p class="bound">{bindingFor('llm', 'summarizer')!.model}</p>
      {:else}
        <p class="unbound">{$_('settings.unbound')}</p>
      {/if}
      <select bind:value={sumProviderId}>
        <option value="">{$_('settings.selectProvider')}</option>
        {#each llmProviders as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
      <input placeholder={$_('settings.model')} bind:value={sumModel} />
      <button onclick={bindSummarizer} disabled={!sumProviderId || !sumModel}>{$_('common.bind')}</button>
      <button class="ghost" onclick={unbindSummarizer} disabled={!bindingFor('llm', 'summarizer')}>{$_('common.unbind')}</button>
    </div>
  </div>

  <!-- 音色试听 -->
  <div class="panel">
    <h2>{$_('settings.preview')}</h2>
    <div class="row">
      <select bind:value={previewProviderId} onchange={loadVoices}>
        <option value="">{$_('settings.selectProvider')}</option>
        {#each ttsProviders as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
      <select bind:value={previewVoiceId}>
        {#each voices as v (v.id)}
          <option value={v.id}>{v.name}（{v.id}）</option>
        {/each}
      </select>
      <input placeholder={$_('settings.previewCustomText')} bind:value={previewText} />
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
      <label>{$_('settings.accent')} <input type="color" bind:value={accent} onchange={applyAccent} /></label>
      <button onclick={resetTheme}>{$_('settings.resetTheme')}</button>
    </div>
    <label class="css-label">
      {$_('settings.customCss')}
      <textarea bind:value={customCss} onblur={applyCustomCss} rows="3" placeholder={$_('settings.cssExample')}></textarea>
    </label>
  </div>

  <!-- 偏好开关（Q4/Q12：自动播放/断开行为/密度/字号） -->
  <div class="panel">
    <h2>{$_('settings.prefs')}</h2>
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
    <div class="row">
      <label>{$_('settings.density')}
        <select
          value={prefs.prefs.density ?? 'comfortable'}
          onchange={(e) => prefs.update({ density: e.currentTarget.value })}
        >
          <option value="comfortable">{$_('settings.comfortable')}</option>
          <option value="compact">{$_('settings.compact')}</option>
        </select>
      </label>
      <label>{$_('settings.fontSize')}
        <input
          type="number"
          min="12"
          max="24"
          value={prefs.prefs.font_size ?? 16}
          onchange={(e) => prefs.update({ font_size: Number(e.currentTarget.value) || null })}
        />
      </label>
      <label>{$_('settings.turnTimeout')}
        <input
          type="number"
          min="30"
          max="3600"
          title={$_('settings.turnTimeoutHint')}
          value={prefs.prefs.turn_timeout_secs ?? 180}
          onchange={(e) => prefs.update({ turn_timeout_secs: Number(e.currentTarget.value) || null })}
        />
      </label>
    </div>
    <p class="hint">{$_('settings.turnTimeoutHint')}</p>
  </div>

  <!-- 账户自助 -->
  <div class="panel">
    <h2>{$_('auth.account')}</h2>
    <div class="row">
      <label>{$_('auth.displayName')} <input bind:value={displayName} /></label>
      <button onclick={saveDisplayName}>{$_('common.save')}</button>
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
        <option value="">{$_('settings.preset')}</option>
        {#each cfg?.provider_presets ?? [] as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
      <select bind:value={adapterId}>
        <option value="llm-openai">llm-openai</option>
        <option value="tts-qwen-audio">tts-qwen-audio</option>
      </select>
      <input placeholder={$_('settings.label')} bind:value={label} required />
      <input placeholder={$_('settings.baseUrl')} bind:value={baseUrl} required />
      <input type="password" placeholder={$_('settings.apiKey')} bind:value={key} required />
      <button type="submit">{$_('common.add')}</button>
    </form>
    <p class="hint">{$_('settings.apiKeyHint')}</p>
    <p class="hint">{$_('settings.endpointHint')}</p>
    <table>
      <tbody>
        {#each providers as p (p.id)}
          <tr>
            <td>{p.label}</td>
            <td>{p.adapter_id}</td>
            <td class="actions">
              <button onclick={() => testConnection(p)}>{$_('settings.testConnection')}</button>
              <button onclick={() => startEdit(p)}>{$_('common.edit')}</button>
              <button onclick={() => removeProvider(p.id)}>{$_('common.delete')}</button>
            </td>
          </tr>
          {#if editId === p.id}
            <tr>
              <td colspan="3">
                <form onsubmit={saveEdit} class="provider-form">
                  <input placeholder={$_('settings.label')} bind:value={editLabel} required />
                  <input placeholder={$_('settings.baseUrl')} bind:value={editBaseUrl} required />
                  <input type="password" placeholder={$_('settings.keyOptional')} bind:value={editKey} />
                  <button type="submit">{$_('common.save')}</button>
                  <button type="button" class="ghost" onclick={() => (editId = '')}>{$_('common.cancel')}</button>
                </form>
              </td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>
    {#if testStatus}<p class="status">{testStatus}</p>{/if}
  </div>

  <!-- 高级绑定矩阵（完整 CRUD） -->
  <div class="panel">
    <button class="advanced-toggle" onclick={() => (advanced = !advanced)}>
      {$_('settings.advanced')} · {$_('settings.bindingMatrix')} {advanced ? '▲' : '▼'}
    </button>
    {#if advanced}
      <table>
        <thead>
          <tr>
            <th>{$_('settings.module')}</th>
            <th>{$_('settings.capability')}</th>
            <th>{$_('settings.role')}</th>
            <th>{$_('settings.providers')}</th>
            <th>{$_('settings.model')}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each bindings as b (b.id)}
            <tr>
              <td>{b.module_id}</td>
              <td>{b.capability}</td>
              <td>{b.role}</td>
              <td>
                <select bind:value={rowProviders[b.id]}>
                  {#each providers as p (p.id)}
                    <option value={p.id}>{p.label}</option>
                  {/each}
                </select>
              </td>
              <td><input bind:value={rowModels[b.id]} /></td>
              <td class="actions">
                <button onclick={() => saveRow(b)}>{$_('common.save')}</button>
                <button onclick={() => removeRow(b)}>{$_('common.delete')}</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <form onsubmit={createAdvanced} class="provider-form">
        <input placeholder={$_('settings.module')} bind:value={advModule} size={6} />
        <select bind:value={advCap}>
          <option value="llm">llm</option>
          <option value="tts">tts</option>
          <option value="stt">stt</option>
        </select>
        <select bind:value={advRole}>
          <option value="default">default</option>
          <option value="summarizer">summarizer</option>
          <option value="extractor">extractor</option>
          <option value="assessor">assessor</option>
        </select>
        <select bind:value={advProvider}>
          <option value="">{$_('settings.selectProvider')}</option>
          {#each providers as p (p.id)}
            <option value={p.id}>{p.label}</option>
          {/each}
        </select>
        <input placeholder={$_('settings.model')} bind:value={advModel} required />
        <input placeholder={$_('settings.paramsVoice')} bind:value={advVoice} />
        <button type="submit">{$_('settings.createBinding')}</button>
      </form>
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
  .card select,
  .card input {
    display: block;
    width: 100%;
    margin: 0.3rem 0;
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
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  button.active {
    outline: 2px solid var(--gw-fg);
  }
  button.ghost {
    background: transparent;
    color: var(--gw-fg);
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
  td input,
  td select {
    width: 100%;
    min-width: 6rem;
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