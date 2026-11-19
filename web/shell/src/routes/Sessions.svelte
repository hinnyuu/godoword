<script lang="ts">
  import { _ } from 'svelte-i18n';
</script>

<section class="placeholder">
  <h1>{$_('nav.sessions')}</h1>
  <p>历史会话 UI（元数据列表 + 文件夹目录树 + 继续/删除）随 P6 落地。</p>
</section>

<style>
  .placeholder {
    padding: 2rem;
    color: var(--gw-muted);
  }
</style>