// PCM 采样队列（AudioWorklet 环形缓冲的纯逻辑核心，浏览器/node 皆可测）。
// 主线程 push 解码样本，worklet render quantum pull 填输出；欠载补静音（不阻塞渲染）。
// 播放时钟由引擎用 AudioContext.currentTime 计（不依赖队列计数，欠载静音亦走时）。

/** FIFO 采样队列（Float32 单声道）。 */
export class PcmQueue {
  private chunks: Float32Array[] = [];
  private offset = 0;
  private buffered = 0;

  /** 入队一段样本（空段忽略）。 */
  push(chunk: Float32Array): void {
    if (chunk.length === 0) return;
    this.chunks.push(chunk);
    this.buffered += chunk.length;
  }

  /**
   * 取样本填入 into（worklet 输出缓冲）。返回实际取自队列的帧数；
   * 不足部分补 0（静音），保证 into 填满（render quantum 不可欠帧）。
   */
  pull(into: Float32Array): number {
    let written = 0;
    while (written < into.length && this.buffered > 0) {
      const head = this.chunks[0]!;
      const avail = head.length - this.offset;
      const need = into.length - written;
      const n = Math.min(avail, need);
      into.set(head.subarray(this.offset, this.offset + n), written);
      written += n;
      this.offset += n;
      this.buffered -= n;
      if (this.offset >= head.length) {
        this.chunks.shift();
        this.offset = 0;
      }
    }
    for (let i = written; i < into.length; i++) into[i] = 0;
    return written;
  }

  /** 当前缓冲帧数。 */
  get bufferedFrames(): number {
    return this.buffered;
  }

  /** 是否空。 */
  get empty(): boolean {
    return this.buffered === 0;
  }

  /** 清空（停止/切换音频用）。 */
  clear(): void {
    this.chunks = [];
    this.offset = 0;
    this.buffered = 0;
  }
}