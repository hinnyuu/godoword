import { describe, it, expect } from 'vitest';
import { activeWordIndex, appendWords, type WordTimestamp } from '../../src/audio/highlight';

function word(begin: number, end: number, text = 'w'): WordTimestamp {
  return { text, begin_index: 0, end_index: 1, begin_time: begin, end_time: end };
}

describe('highlight', () => {
  const words = [word(0, 100, 'こ'), word(100, 200, 'ん'), word(300, 400, 'に')];

  it('命中含时刻的词', () => {
    expect(activeWordIndex(words, 0)).toBe(0);
    expect(activeWordIndex(words, 50)).toBe(0);
    expect(activeWordIndex(words, 150)).toBe(1);
    expect(activeWordIndex(words, 350)).toBe(2);
  });

  it('边界：begin 含、end 不含', () => {
    expect(activeWordIndex(words, 100)).toBe(1); // 100 属第二词
    expect(activeWordIndex(words, 200)).toBe(-1); // 200–300 间隙
  });

  it('间隙/越界返回 -1', () => {
    expect(activeWordIndex(words, 250)).toBe(-1);
    expect(activeWordIndex(words, -5)).toBe(-1);
    expect(activeWordIndex(words, 500)).toBe(-1);
  });

  it('空数组返回 -1', () => {
    expect(activeWordIndex([], 10)).toBe(-1);
  });

  it('appendWords 增量拼接', () => {
    const a = [word(0, 100)];
    const b = [word(100, 200)];
    const merged = appendWords(a, b);
    expect(merged.length).toBe(2);
    expect(merged[1]?.begin_time).toBe(100);
    expect(a.length).toBe(1); // 不改原数组
  });
});