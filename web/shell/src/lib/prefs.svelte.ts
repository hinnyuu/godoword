// 用户偏好状态（runes store）：加载跨设备偏好 + 应用主题/自定义 CSS + 更新保存。
// 审计 M5：update 串行队列防并发 last-write-wins 乱序；apply 先清残留内联变量再应用。
import {
  loadPreferences,
  savePreferences,
  mergeVars,
  applyTheme,
  clearCustomVars,
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
  private queue: Promise<void> = Promise.resolve();

  async load(): Promise<void> {
    try {
      this.prefs = await loadPreferences();
    } catch {
      this.prefs = {};
    }
    this.apply();
  }

  /** 应用主题（预设变量 + 用户覆盖）+ 自定义 CSS + 密度/字号。先清残留内联变量（防删除项滞留）。 */
  apply(): void {
    clearCustomVars();
    const preset = PRESETS.find((p) => p.id === (this.prefs.theme ?? 'light')) ?? PRESETS[0]!;
    const userVars = (this.prefs.theme_vars ?? undefined) as Record<string, string> | undefined;
    applyTheme(mergeVars(preset.vars, userVars), preset.dark);
    applyCustomCss(this.prefs.custom_css ?? undefined);
    const root = document.documentElement;
    if (this.prefs.font_size) {
      root.style.setProperty('--gw-font-size', `${this.prefs.font_size}px`);
    }
    if (this.prefs.density === 'compact') {
      root.style.setProperty('--gw-space', '0.6rem');
    }
  }

  /** 更新偏好（串行队列：并发 toggle 按序落库，响应不被旧请求覆盖）。
   *  二审 R2-1：快照须在队列**执行时**取（调用时取会让连续 patch 基于旧态丢前序变更）。 */
  update(patch: Partial<Preferences>): Promise<void> {
    const run = this.queue.then(async () => {
      const next = { ...this.prefs, ...patch };
      this.prefs = await savePreferences(next);
      this.apply();
    });
    this.queue = run.catch(() => {});
    return run;
  }

  /** 登出重置（清用户偏好，回默认主题）。 */
  reset(): void {
    this.prefs = {};
    this.apply();
  }
}

export const prefs = new PrefsStore();