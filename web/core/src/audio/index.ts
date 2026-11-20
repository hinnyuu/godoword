// 音频引擎公共出口（worklet 经 ?worker&url 单独加载，不在此导出——其副作用 registerProcessor）。
export { SAMPLE_RATE, base64ToBytes, pcm16ToFloat32, decodeAudioChunk, float32ToPcm16 } from './pcm';
export { activeWordIndex, appendWords, type WordTimestamp } from './highlight';
export { PcmQueue } from './queue';
export { AudioEngine, type AudioEngineOptions } from './engine';