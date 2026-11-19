// svelte-i18n 初始化（zh-CN 首发，catalog 来自 core 单一事实源）。
import { register, init } from 'svelte-i18n';
import { zhCN } from '@godoword/core';

register('zh-CN', () => Promise.resolve(zhCN));

init({
  fallbackLocale: 'zh-CN',
  initialLocale: 'zh-CN',
});