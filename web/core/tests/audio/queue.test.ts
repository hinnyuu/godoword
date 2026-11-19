import { describe, it, expect } from 'vitest';
import { PcmQueue } from '../../src/audio/queue';

describe('PcmQueue', () => {
  it('push/pull 基本', () => {
    const q = new PcmQueue();
    q.push(new Float32Array([1, 2, 3]));
    expect(q.bufferedFrames).toBe(3);
    const out = new Float32Array(3);
    expect(q.pull(out)).toBe(3);
    expect([...out]).toEqual([1, 2, 3]);
    expect(q.empty).toBe(true);
  });

  it('跨 chunk 边界拼接', () => {
    const q = new PcmQueue();
    q.push(new Float32Array([1, 2]));
    q.push(new Float32Array([3, 4, 5]));
    const out = new Float32Array(4);
    expect(q.pull(out)).toBe(4);
    expect([...out]).toEqual([1, 2, 3, 4]);
    expect(q.bufferedFrames).toBe(1);
    const rest = new Float32Array(1);
    q.pull(rest);
    expect([...rest]).toEqual([5]);
  });

  it('欠载补静音（into 大于缓冲）', () => {
    const q = new PcmQueue();
    q.push(new Float32Array([1, 2]));
    const out = new Float32Array(5);
    expect(q.pull(out)).toBe(2); // 仅 2 帧取自队列
    expect([...out]).toEqual([1, 2, 0, 0, 0]); // 余补 0
    expect(q.empty).toBe(true);
  });

  it('空队列 pull 全静音', () => {
    const q = new PcmQueue();
    const out = new Float32Array(3);
    expect(q.pull(out)).toBe(0);
    expect([...out]).toEqual([0, 0, 0]);
  });

  it('空段 push 忽略', () => {
    const q = new PcmQueue();
    q.push(new Float32Array(0));
    expect(q.bufferedFrames).toBe(0);
    expect(q.empty).toBe(true);
  });

  it('clear 重置', () => {
    const q = new PcmQueue();
    q.push(new Float32Array([1, 2, 3]));
    q.clear();
    expect(q.empty).toBe(true);
    expect(q.bufferedFrames).toBe(0);
  });

  it('多次 pull 耗尽（worklet render quantum 模拟）', () => {
    const q = new PcmQueue();
    q.push(new Float32Array(300).fill(0.5));
    let total = 0;
    for (let i = 0; i < 3; i++) {
      const out = new Float32Array(128);
      total += q.pull(out);
    }
    expect(total).toBe(300); // 3×128=384 容量，实际 300 帧
    expect(q.empty).toBe(true);
  });
});