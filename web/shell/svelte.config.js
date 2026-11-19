import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  preprocess: vitePreprocess(),
  // runes 不强制：Svelte 5 自动检测（本 shell 组件用 runes，svelte-spa-router 等 legacy lib 自动 legacy 模式）。
};