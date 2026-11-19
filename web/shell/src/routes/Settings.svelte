<script lang="ts">
  import { _ } from 'svelte-i18n';
</script>

<section class="placeholder">
  <h1>{$_('nav.settings')}</h1>
  <p>设置 UI（provider 三卡片 + 高级绑定矩阵 + 音色试听 + 主题面板 + 偏好）随 P5 落地。</p>
</section>

<style>
  .placeholder {
    padding: 2rem;
    color: var(--gw-muted);
  }
</style>