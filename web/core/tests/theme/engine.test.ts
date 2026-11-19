import { describe, it, expect } from 'vitest';
import { mergeVars, PRESETS } from '../../src/theme/engine';

describe('theme/engine', () => {
  it('预设含明暗', () => {
    expect(PRESETS.find((p) => p.id === 'light')?.dark).toBe(false);
    expect(PRESETS.find((p) => p.id === 'dark')?.dark).toBe(true);
  });

  it('mergeVars 用户覆盖预设', () => {
    const base = { '--gw-accent': '#2563eb', '--gw-bg': '#fff' };
    const merged = mergeVars(base, { '--gw-accent': '#ff0000' });
    expect(merged['--gw-accent']).toBe('#ff0000');
    expect(merged['--gw-bg']).toBe('#fff');
  });

  it('mergeVars 无用户变量返回预设', () => {
    const base = { '--gw-bg': '#fff' };
    expect(mergeVars(base, undefined)).toEqual(base);
  });
});