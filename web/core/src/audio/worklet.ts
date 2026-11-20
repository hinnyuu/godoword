// AudioWorklet 处理器（运行于 AudioWorkletGlobalScope，独立realm）。
// 经 Vite ?worker&url 打包为独立脚本 URL，由引擎 audioWorklet.addModule 加载。
// 共享 PcmQueue 纯逻辑（打包内联，防重复漂移）。render quantum  pull 填输出，欠载补静音。

import { PcmQueue } from './queue';

// AudioWorkletGlobalScope 全局（标准 DOM lib 未含 worklet scope 类型，此处 ambient 声明）。
declare abstract class AudioWorkletProcessor {
  readonly port: MessagePort;
  constructor();
  abstract process(
    inputs: Float32Array[][],
    outputs: Float32Array[][],
    parameters: Record<string, Float32Array>,
  ): boolean;
}
declare function registerProcessor(name: string, ctor: typeof AudioWorkletProcessor): void;

interface PushMessage {
  type: 'push';
  samples: Float32Array;
}
interface ClearMessage {
  type: 'clear';
}
type InMessage = PushMessage | ClearMessage;

class PcmPlayerProcessor extends AudioWorkletProcessor {
  private queue = new PcmQueue();

  constructor() {
    super();
    this.port.onmessage = (e: MessageEvent<InMessage>) => {
      const msg = e.data;
      if (msg.type === 'push') this.queue.push(msg.samples);
      else if (msg.type === 'clear') this.queue.clear();
    };
  }

  process(_inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
    const out = outputs[0]?.[0];
    if (out) {
      const pulled = this.queue.pull(out);
      this.port.postMessage({ type: 'progress', pulled, buffered: this.queue.bufferedFrames });
    }
    return true; // 保持存活（引擎 stop 时 disconnect）
  }
}

registerProcessor('pcm-player', PcmPlayerProcessor);