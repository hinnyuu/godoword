// SessionEvent 线缆 fixture round-trip（锚定 kernel-api/tests/serde.rs 黄金值）。
// SSE 线缆形：event:<kebab 标签> + data:<纯载荷 JSON>（无信封）；parseSessionEvent 扁平化。
import { describe, it, expect } from 'vitest';
import { parseSessionEvent } from '../../src/sse/client';

// 与 kernel-api serde 黄金测试同源的载荷 fixture（脱敏，无密钥）。
const GOLDEN: Array<[string, Record<string, unknown>]> = [
  ['run-start', { run_id: 'r1', plan_version: 3, model: 'qwen4-max-preview' }],
  ['text-delta', { text: 'こんにちは' }],
  ['reasoning-delta', { text: 'thinking' }],
  ['tool-call', { id: 'call_1', name: 'speak', arguments: { text: 'こんにちは', rate: 0.5 } }],
  ['tool-result', { id: 'call_1', ok: true }],
  ['audio-start', { audio_id: 'a1', format: 'pcm', sample_rate: 24000 }],
  ['audio-chunk', { audio_id: 'a1', seq: 7, data: 'AAEC' }],
  ['audio-end', { audio_id: 'a1', url: '/api/v1/audio/a1' }],
  [
    'word-timestamps',
    {
      audio_id: 'a1',
      words: [{ text: 'こんにちは', begin_index: 0, end_index: 1, begin_time: 160, end_time: 640 }],
    },
  ],
  ['usage', { capability: 'llm', model: 'qwen4-max-preview', usage: { total_tokens: 55 } }],
  ['error', { code: 'auth', message: 'key 无效', retryable: false, action: 'settings' }],
  ['run-end', { run_id: 'r1', finish_reason: 'stop' }],
];

describe('sse/SessionEvent fixture round-trip（serde 黄金同源）', () => {
  for (const [tag, data] of GOLDEN) {
    it(`${tag} 扁平化保持载荷`, () => {
      const ev = parseSessionEvent(tag, JSON.stringify(data));
      expect(ev).toEqual({ event: tag, ...data });
    });
  }
  it('usage 载荷数字可解析（Q9 脚注）', () => {
    const ev = parseSessionEvent('usage', JSON.stringify(GOLDEN[9]![1]));
    expect(ev?.event).toBe('usage');
  });
});