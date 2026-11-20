// 音频引擎（浏览器侧）：AudioContext + AudioWorklet 排队播放 + 词级高亮同步。
// 框架无关：workletUrl 由 shell 经 Vite ?worker&url 注入（core 不依赖 bundler 语法）。
// live 模式逐 chunk push；回放模式一次性 push 完整 PCM。高亮时钟用 AudioContext.currentTime。

import { SAMPLE_RATE, decodeAudioChunk } from './pcm';
import { activeWordIndex, type WordTimestamp } from './highlight';

/** 引擎配置。 */
export interface AudioEngineOptions {
  /** AudioWorklet 脚本 URL（shell 经 Vite ?worker&url 提供）。 */
  workletUrl: string;
  /** 采样率（默认内核固定 24000）。 */
  sampleRate?: number;
  /** 高亮回调（当前词索引，-1=无）。 */
  onHighlight?: (index: number) => void;
  /** 缓冲水位回调（帧数，UI 缓冲指示用）。 */
  onBuffer?: (frames: number) => void;
  /** 播放结束回调（已推入样本播完且 markComplete；自动播放队列用）。 */
  onEnded?: () => void;
}

/** 流式 PCM 播放引擎。 */
export class AudioEngine {
  private ctx: AudioContext | null = null;
  private node: AudioWorkletNode | null = null;
  private words: readonly WordTimestamp[] = [];
  private startAt = 0;
  private playing = false;
  private raf = 0;
  private lastHighlight = -2;
  private pushedMs = 0;
  private complete = false;
  private endedFired = false;
  private readonly sampleRate: number;

  constructor(private readonly opts: AudioEngineOptions) {
    this.sampleRate = opts.sampleRate ?? SAMPLE_RATE;
  }

  /** 初始化 AudioContext + 加载 worklet + 建节点（须用户手势后调用以满足自动播放策略）。 */
  async init(): Promise<void> {
    if (this.ctx) return;
    const ctx = new AudioContext({ sampleRate: this.sampleRate });
    await ctx.audioWorklet.addModule(this.opts.workletUrl);
    const node = new AudioWorkletNode(ctx, 'pcm-player');
    node.port.onmessage = (e: MessageEvent<{ type: string; buffered?: number }>) => {
      if (e.data.type === 'progress' && typeof e.data.buffered === 'number') {
        this.opts.onBuffer?.(e.data.buffered);
      }
    };
    node.connect(ctx.destination);
    this.ctx = ctx;
    this.node = node;
  }

  /** 设置词级时间戳（live 增量更新 / 回放一次性）。 */
  setWords(words: readonly WordTimestamp[]): void {
    this.words = words;
  }

  /** 推入一段 b64 PCM（live audio-chunk / 回放完整音频）。 */
  pushChunk(b64: string): void {
    const samples = decodeAudioChunk(b64);
    this.pushedMs += (samples.length / this.sampleRate) * 1000;
    this.node?.port.postMessage({ type: 'push', samples });
  }

  /** 推入完整 PCM 字节（回放 GET /audio/{id}）。 */
  pushPcm(bytes: Uint8Array): void {
    const frames = bytes.length >> 1;
    const view = new DataView(bytes.buffer, bytes.byteOffset, frames * 2);
    const samples = new Float32Array(frames);
    for (let i = 0; i < frames; i++) samples[i] = view.getInt16(i * 2, true) / 32768;
    this.pushedMs += (frames / this.sampleRate) * 1000;
    this.node?.port.postMessage({ type: 'push', samples });
  }

  /** 标记样本已全部推入（live audio-end / 回放 push 后）；播完触发 onEnded。 */
  markComplete(): void {
    this.complete = true;
  }

  /** 开始/恢复播放（启动高亮时钟）。 */
  async play(): Promise<void> {
    if (!this.ctx) await this.init();
    const ctx = this.ctx!;
    await ctx.resume();
    if (!this.playing) {
      this.startAt = ctx.currentTime - this.pausedAt();
      this.playing = true;
      this.tick();
    }
  }

  private pauseOffset = 0;
  private pausedAt(): number {
    return this.pauseOffset;
  }

  /** 暂停（保留进度）。 */
  async pause(): Promise<void> {
    if (!this.ctx || !this.playing) return;
    this.pauseOffset = this.ctx.currentTime - this.startAt;
    this.playing = false;
    cancelAnimationFrame(this.raf);
    await this.ctx.suspend();
  }

  /** 停止并清空缓冲（重置进度）。 */
  stop(): void {
    this.playing = false;
    this.pauseOffset = 0;
    this.pushedMs = 0;
    this.complete = false;
    this.endedFired = false;
    cancelAnimationFrame(this.raf);
    this.node?.port.postMessage({ type: 'clear' });
    this.lastHighlight = -2;
    this.opts.onHighlight?.(-1);
  }

  /** 当前播放时刻（毫秒）。 */
  get currentTimeMs(): number {
    if (!this.ctx) return 0;
    return (this.playing ? this.ctx.currentTime - this.startAt : this.pauseOffset) * 1000;
  }

  /** 是否播放中。 */
  get isPlaying(): boolean {
    return this.playing;
  }

  private tick = (): void => {
    if (!this.playing || !this.ctx) return;
    const t = (this.ctx.currentTime - this.startAt) * 1000;
    const idx = activeWordIndex(this.words, t);
    if (idx !== this.lastHighlight) {
      this.lastHighlight = idx;
      this.opts.onHighlight?.(idx);
    }
    // 播完检测：样本全部推入且时钟越过总时长 → 结束回调（自动播放队列驱动）。
    if (this.complete && !this.endedFired && this.pushedMs > 0 && t >= this.pushedMs) {
      this.endedFired = true;
      this.playing = false;
      this.pauseOffset = this.pushedMs / 1000;
      this.opts.onHighlight?.(-1);
      this.opts.onEnded?.();
      return;
    }
    this.raf = requestAnimationFrame(this.tick);
  };

  /** 释放资源。 */
  dispose(): void {
    this.stop();
    this.node?.disconnect();
    void this.ctx?.close();
    this.node = null;
    this.ctx = null;
  }
}