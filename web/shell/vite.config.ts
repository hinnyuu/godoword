import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Vite 构建 SPA → dist（P7 Dockerfile node 阶段拷入运行镜像 web/shell/public）。
// publicDir 用 static（独立于 axum 伺服的 public/ 占位，避免 index.html 冲突，P7 切换）。
export default defineConfig({
  plugins: [svelte()],
  publicDir: 'static',
  server: {
    port: 5173,
    proxy: {
      '/api': { target: 'http://127.0.0.1:8080', changeOrigin: true },
    },
  },
  build: {
    outDir: 'dist',
    target: 'es2022',
    emptyOutDir: true,
  },
});