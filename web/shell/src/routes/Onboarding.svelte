<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { push } from 'svelte-spa-router';
  import { listProviders, listBindings } from '@godoword/core';

  let hasLlm = $state(false);
  let hasTts = $state(false);

  $effect(() => {
    void (async () => {
      try {
        const [providers, bindings] = await Promise.all([listProviders(), listBindings()]);
        hasLlm =
          providers.some((p) => p.adapter_id === 'llm-openai') &&
          bindings.some((b) => b.capability === 'llm');
        hasTts =
          providers.some((p) => p.adapter_id === 'tts-qwen-audio') &&
          bindings.some((b) => b.capability === 'tts');
      } catch {
        // 未配置/加载失败 → 步骤未勾选。
      }
    })();
  });
</script>

<section class="onboarding">
  <h1>{$_('onboarding.title')}</h1>
  <ol>
    <li class:done={hasLlm}>
      <span class="mark">{hasLlm ? '✓' : '1'}</span>
      {$_('onboarding.step1')}
      <a href="#/settings">{$_('onboarding.goConfigure')}</a>
    </li>
    <li class:done={hasTts}>
      <span class="mark">{hasTts ? '✓' : '2'}</span>
      {$_('onboarding.step2')}
      <a href="#/settings">{$_('onboarding.goConfigure')}</a>
    </li>
    <li class:done={hasLlm && hasTts}>
      <span class="mark">{hasLlm && hasTts ? '✓' : '3'}</span>
      {$_('onboarding.step3')}
      <button onclick={() => push('/')} disabled={!hasLlm}>{$_('nav.chat')}</button>
    </li>
  </ol>
  <button class="skip" onclick={() => push('/')}>{$_('onboarding.skip')}</button>
</section>

<style>
  .onboarding {
    max-width: 32rem;
    margin: 3rem auto;
    padding: 2rem;
  }
  ol {
    list-style: none;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 1rem;
    border: 1px solid var(--gw-border);
    border-radius: var(--gw-radius);
    background: var(--gw-surface);
  }
  li.done {
    opacity: 0.7;
  }
  .mark {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 50%;
    background: var(--gw-accent);
    color: #fff;
    font-size: 0.85rem;
  }
  li a,
  li button {
    margin-left: auto;
  }
  button {
    padding: 0.3rem 0.7rem;
    border: none;
    border-radius: 4px;
    background: var(--gw-accent);
    color: #fff;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
  }
  .skip {
    margin-top: 1.5rem;
    background: transparent;
    color: var(--gw-muted);
    border: 1px solid var(--gw-border);
  }
</style>