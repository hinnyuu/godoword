// 用户偏好状态（runes store）：加载跨设备偏好 + 应用主题/自定义 CSS + 更新保存。
import {
  loadPreferences,
  savePreferences,
  mergeVars,
  applyTheme,
  PRESETS,
  type Preferences,
} from '@godoword/core';

const CUSTOM_STYLE_ID = 'gw-custom-css';

function applyCustomCss(css: string | undefined): void {
  let el = document.getElementById(CUSTOM_STYLE_ID) as HTMLStyleElement | null;
  if (!css) {
    el?.remove();
    return;
  }
  if (!el) {
    el = document.createElement('style');
    el.id = CUSTOM_STYLE_ID;
    document.head.appendChild(el);
  }
  el.textContent = css;
}

class PrefsStore {
  prefs = $state<Preferences>({});

  async load(): Promise<void> {
    try {
      this.prefs = await loadPreferences();
    } catch {
      this.prefs = {};
    }
    this.apply();
  }

  /** 应用主题（预设变量 + 用户覆盖）+ 自定义 CSS。 */
  apply(): void {
    const preset = PRESETS.find((p) => p.id === (this.prefs.theme ?? 'light')) ?? PRESETS[0]!;
    const userVars = (this.prefs.theme_vars ?? undefined) as Record<string, string> | undefined;
    applyTheme(mergeVars(preset.vars, userVars), preset.dark);
    applyCustomCss(this.prefs.custom_css ?? undefined);
    if (this.prefs.font_size) {
      document.documentElement.style.setProperty('--gw-font-size', `${this.prefs.font_size}px`);
    }
  }

  async update(patch: Partial<Preferences>): Promise<void> {
    const next = { ...this.prefs, ...patch };
    this.prefs = await savePreferences(next);
    this.apply();
  }

  /** 登出重置（清用户偏好，回默认主题）。 */
  reset(): void {
    this.prefs = {};
    this.apply();
  }
}

export const prefs = new PrefsStore();