<script lang="ts">
  import { _ } from 'svelte-i18n';
</script>

<section class="placeholder">
  <h1>{$_('nav.chat')}</h1>
  <p>会话 UI（对话流 + 音频卡片 + 词级高亮 + 文件夹树）随 P6 落地。</p>
</section>

<style>
  .placeholder {
    padding: 2rem;
    color: var(--gw-muted);
  }
</style>