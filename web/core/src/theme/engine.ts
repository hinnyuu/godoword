// 主题引擎（CSS 自定义属性 + 预设 + 用户变量合并）。偏好跨设备经 prefs 持久化。
// 语义变量与 web/shell app.css 一致；用户自定义变量/高级 CSS 经 prefs 注入（shell 应用）。

/** 主题预设。 */
export interface ThemePreset {
  id: string;
  name: string;
  dark: boolean;
  vars: Record<string, string>;
}

/** 内置预设（明/暗 + 示例自定义）。 */
export const PRESETS: ThemePreset[] = [
  {
    id: 'light',
    name: '明亮',
    dark: false,
    vars: {
      '--gw-bg': '#ffffff',
      '--gw-fg': '#1a1a1a',
      '--gw-muted': '#666666',
      '--gw-accent': '#2563eb',
      '--gw-surface': '#f4f4f5',
      '--gw-border': '#e4e4e7',
    },
  },
  {
    id: 'dark',
    name: '暗色',
    dark: true,
    vars: {
      '--gw-bg': '#18181b',
      '--gw-fg': '#f4f4f5',
      '--gw-muted': '#a1a1aa',
      '--gw-accent': '#60a5fa',
      '--gw-surface': '#27272a',
      '--gw-border': '#3f3f46',
    },
  },
];

/** 合并主题变量（用户覆盖预设）。纯函数可测。 */
export function mergeVars(
  base: Record<string, string>,
  user: Record<string, string> | undefined,
): Record<string, string> {
  return { ...base, ...(user ?? {}) };
}

/** 应用主题到 DOM（data-theme + CSS 变量）。浏览器侧。 */
export function applyTheme(vars: Record<string, string>, dark: boolean): void {
  const root = document.documentElement;
  root.dataset.theme = dark ? 'dark' : 'light';
  for (const [k, v] of Object.entries(vars)) {
    root.style.setProperty(k, v);
  }
}

/** 清除内联自定义变量（一键重置）。浏览器侧。 */
export function clearCustomVars(): void {
  const root = document.documentElement;
  root.removeAttribute('style');
}