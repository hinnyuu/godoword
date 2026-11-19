// 词级高亮同步（播放时钟 vs 词时间戳）。纯逻辑，可 node 单测。
// 时间戳毫秒，增量追加语义（适配器跨帧去重，前端按到达累积）。

/** 词级时间戳（镜像 kernel-api WordTimestamp 线缆形状；P3 由 OpenAPI 生成类型取代）。 */
export interface WordTimestamp {
  text: string;
  begin_index: number;
  end_index: number;
  begin_time: number;
  end_time: number;
}

/**
 * 给定播放时刻（毫秒）求当前高亮词索引。词按 begin_time 升序（适配器产出有序）。
 * 命中 begin_time <= t < end_time 的词；落在间隙/越界返回 -1。二分查找。
 */
export function activeWordIndex(words: readonly WordTimestamp[], timeMs: number): number {
  let lo = 0;
  let hi = words.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const w = words[mid]!;
    if (timeMs < w.begin_time) hi = mid - 1;
    else if (timeMs >= w.end_time) lo = mid + 1;
    else return mid;
  }
  return -1;
}

/** 增量追加词时间戳（word-timestamps 事件累积；适配器已去重，直接拼接）。 */
export function appendWords(
  existing: readonly WordTimestamp[],
  incoming: readonly WordTimestamp[],
): WordTimestamp[] {
  return [...existing, ...incoming];
}