// SSE 客户端（POST /messages → 该回合事件流）。POST→SSE 故用 fetch ReadableStream（EventSource 仅 GET）。
// 帧解析 eventsource-parser；线缆形 event:<kebab 名> + data:<纯载荷 JSON>（无信封）。
// 重连=重附持久态（GET session，shell 处理），无 mid-run 回放缓冲（已定案）。

import { createParser } from 'eventsource-parser';
import type { WordTimestamp } from '../audio/highlight';
import { CSRF_HEADER, CSRF_VALUE } from '../api/client';

/** 回合事件（判别联合，event 标签 + 载荷扁平化）。镜像 kernel-api SessionEvent serde 形状。 */
export type SessionEvent =
  | { event: 'run-start'; run_id: string; plan_version: number | null; model: string }
  | { event: 'text-delta'; text: string }
  | { event: 'reasoning-delta'; text: string }
  | { event: 'tool-call'; id: string; name: string; arguments: unknown }
  | { event: 'tool-result'; id: string; ok: boolean }
  | { event: 'audio-start'; audio_id: string; format: string; sample_rate: number }
  | { event: 'audio-chunk'; audio_id: string; seq: number; data: string }
  | { event: 'audio-end'; audio_id: string; url: string }
  | { event: 'word-timestamps'; audio_id: string; words: WordTimestamp[] }
  | { event: 'usage'; capability: string; model: string; usage: unknown }
  | { event: 'error'; code: string; message: string; retryable: boolean; action: string | null }
  | { event: 'run-end'; run_id: string; finish_reason: string };

/** 解析单帧（event 名 + data 载荷 → 扁平 SessionEvent）。未知事件返回 null。 */
export function parseSessionEvent(event: string, data: string): SessionEvent | null {
  let payload: Record<string, unknown>;
  try {
    payload = JSON.parse(data) as Record<string, unknown>;
  } catch {
    return null;
  }
  return { event, ...payload } as SessionEvent;
}

/** 流式一个回合（POST text → SSE），逐事件回调。signal 中止（显式取消/断开）。 */
export async function streamTurn(
  sessionId: string,
  text: string,
  onEvent: (ev: SessionEvent) => void,
  signal?: AbortSignal,
): Promise<void> {
  const res = await fetch(`/api/v1/sessions/${sessionId}/messages`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', [CSRF_HEADER]: CSRF_VALUE },
    body: JSON.stringify({ text }),
    signal: signal ?? null,
  });
  if (!res.ok || !res.body) {
    throw new Error(`回合请求失败：${res.status}`);
  }
  const parser = createParser({
    onEvent(msg) {
      const ev = parseSessionEvent(msg.event || 'message', msg.data);
      if (ev) onEvent(ev);
    },
  });
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    parser.feed(decoder.decode(value, { stream: true }));
  }
}