import { describe, it, expect } from 'vitest';
import { parseSessionEvent } from '../../src/sse/client';

describe('sse/parseSessionEvent', () => {
  it('text-delta 扁平化', () => {
    const ev = parseSessionEvent('text-delta', '{"text":"こんにちは"}');
    expect(ev).toEqual({ event: 'text-delta', text: 'こんにちは' });
  });

  it('error 含 action', () => {
    const ev = parseSessionEvent('error', '{"code":"auth","message":"key 无效","retryable":false,"action":"settings"}');
    expect(ev).toMatchObject({ event: 'error', code: 'auth', retryable: false, action: 'settings' });
  });

  it('audio-chunk 含 seq/data', () => {
    const ev = parseSessionEvent('audio-chunk', '{"audio_id":"x","seq":3,"data":"AAEC"}');
    expect(ev).toMatchObject({ event: 'audio-chunk', seq: 3, data: 'AAEC' });
  });

  it('run-end finish_reason', () => {
    const ev = parseSessionEvent('run-end', '{"run_id":"r","finish_reason":"stop"}');
    expect(ev).toMatchObject({ event: 'run-end', finish_reason: 'stop' });
  });

  it('非法 JSON 返回 null', () => {
    expect(parseSessionEvent('text-delta', '{not json')).toBeNull();
  });
});