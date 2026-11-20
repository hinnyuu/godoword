import { describe, it, expect } from 'vitest';
import { routeError, parseUsage } from '../../src/session/ux';

describe('session/ux routeError（Q13 错误分流）', () => {
  it('shutting_down → 退出提示', () => {
    expect(routeError('shutting_down', '退出中', true, null).kind).toBe('shutdown');
  });
  it('action=settings 优先', () => {
    expect(routeError('auth', 'key 无效', false, 'settings').kind).toBe('settings');
    expect(routeError('quota', '配额', false, 'settings').kind).toBe('settings');
  });
  it('code 兜底：unconfigured/model_unavailable → settings', () => {
    expect(routeError('unconfigured', '未配置', false, null).kind).toBe('settings');
    expect(routeError('model_unavailable', '模型', false, null).kind).toBe('settings');
  });
  it('busy → toast', () => {
    expect(routeError('busy', '活跃回合', true, null).kind).toBe('toast');
  });
  it('retryable → retry', () => {
    expect(routeError('rate_limit', '限流', true, null).kind).toBe('retry');
    expect(routeError('network', '网络', true, null).kind).toBe('retry');
  });
  it('不可重试其余 → info', () => {
    expect(routeError('storage', '存储', false, null).kind).toBe('info');
    expect(routeError('timeout', '超时', false, null).kind).toBe('info');
  });
});

describe('session/ux parseUsage（Q9 回合级数字）', () => {
  it('LLM tokens', () => {
    const n = parseUsage('llm', 'qwen', { prompt_tokens: 234, completion_tokens: 252 });
    expect(n.promptTokens).toBe(234);
    expect(n.completionTokens).toBe(252);
  });
  it('TTS characters', () => {
    const n = parseUsage('tts', 'qwen-tts', { characters: 5 });
    expect(n.characters).toBe(5);
  });
  it('未知形状降级仅 model', () => {
    const n = parseUsage('llm', 'm', null);
    expect(n.model).toBe('m');
    expect(n.promptTokens).toBeUndefined();
  });
});