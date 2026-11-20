// PCM 解码（16-bit 单声道小端 → Float32）。音频引擎纯逻辑，浏览器/node 皆可测。
// 上游 TTS 固定 format=pcm / sample_rate=24000（kernel prepare_speak 定，不暴露给 LLM）。

/** 内核固定采样率（与 kernel-api TtsRequest 默认一致）。 */
export const SAMPLE_RATE = 24000;

/** base64 → 字节（atob 浏览器与 node 22 皆有全局）。 */
export function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

/** 16-bit 小端 PCM → Float32（[-1, 1)）。奇数字节截尾（半样本丢弃，防越界）。 */
export function pcm16ToFloat32(bytes: Uint8Array): Float32Array {
  const frames = bytes.length >> 1;
  const view = new DataView(bytes.buffer, bytes.byteOffset, frames * 2);
  const out = new Float32Array(frames);
  for (let i = 0; i < frames; i++) {
    out[i] = view.getInt16(i * 2, true) / 32768;
  }
  return out;
}

/** SSE audio-chunk 的 b64 PCM → Float32 样本。 */
export function decodeAudioChunk(b64: string): Float32Array {
  return pcm16ToFloat32(base64ToBytes(b64));
}

/** Float32 → 16-bit 小端 PCM（回放/测试用，clamp 防溢出）。 */
export function float32ToPcm16(samples: Float32Array): Uint8Array {
  const out = new Uint8Array(samples.length * 2);
  const view = new DataView(out.buffer);
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i] ?? 0));
    view.setInt16(i * 2, Math.round(s * 32767), true);
  }
  return out;
}