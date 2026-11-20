import { describe, it, expect } from 'vitest';
import {
  SAMPLE_RATE,
  base64ToBytes,
  pcm16ToFloat32,
  decodeAudioChunk,
  float32ToPcm16,
} from '../../src/audio/pcm';

function bytesFromInt16(values: number[]): Uint8Array {
  const out = new Uint8Array(values.length * 2);
  const view = new DataView(out.buffer);
  values.forEach((v, i) => view.setInt16(i * 2, v, true));
  return out;
}

function b64FromBytes(bytes: Uint8Array): string {
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

describe('pcm', () => {
  it('采样率常量为内核固定 24000', () => {
    expect(SAMPLE_RATE).toBe(24000);
  });

  it('base64 解码 round-trip', () => {
    const bytes = new Uint8Array([0, 1, 2, 255, 128, 64]);
    expect(base64ToBytes(b64FromBytes(bytes))).toEqual(bytes);
  });

  it('Int16 小端 → Float32 归一化', () => {
    const bytes = bytesFromInt16([0, 16384, -16384, 32767, -32768]);
    const f = pcm16ToFloat32(bytes);
    expect(f[0]).toBe(0);
    expect(f[1]).toBeCloseTo(0.5, 5);
    expect(f[2]).toBeCloseTo(-0.5, 5);
    expect(f[3]).toBeCloseTo(32767 / 32768, 5);
    expect(f[4]).toBe(-1);
  });

  it('奇数字节截尾（半样本丢弃）', () => {
    const bytes = new Uint8Array([0, 0, 0]); // 1.5 样本 → 1 帧
    expect(pcm16ToFloat32(bytes).length).toBe(1);
  });

  it('decodeAudioChunk 组合 b64→Float32', () => {
    const bytes = bytesFromInt16([16384, -16384]);
    const f = decodeAudioChunk(b64FromBytes(bytes));
    expect(f[0]).toBeCloseTo(0.5, 5);
    expect(f[1]).toBeCloseTo(-0.5, 5);
  });

  it('Float32 → Int16 round-trip + clamp', () => {
    const samples = new Float32Array([0, 0.5, -0.5, 2, -2]); // 2/-2 越界须 clamp
    const bytes = float32ToPcm16(samples);
    const back = pcm16ToFloat32(bytes);
    expect(back[0]).toBe(0);
    expect(back[1]).toBeCloseTo(0.5, 3);
    expect(back[2]).toBeCloseTo(-0.5, 3);
    expect(back[3]).toBeCloseTo(1, 3);
    expect(back[4]).toBeCloseTo(-1, 3);
  });
});