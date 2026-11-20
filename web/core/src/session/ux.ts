// 会话 UX 域逻辑（Q13 错误分流 + Q9 usage 脚注）：纯函数，shell 以 i18n 渲染。
// 错误分流定案（history 2049 Q13）：auth/quota/unconfigured→去设置；retryable→重试；
// busy→toast；shutting_down→退出提示；其余→信息横幅。失败不丢输入文本（转录保留 + 重试）。

/** 错误路由类别。 */
export type ErrorKind = 'settings' | 'retry' | 'toast' | 'shutdown' | 'info';

/** 错误路由结果。 */
export interface ErrorRoute {
  kind: ErrorKind;
  message: string;
}

/** 按 code/action/retryable 分流（服务端 action 优先，code 兜底）。 */
export function routeError(
  code: string,
  message: string,
  retryable: boolean,
  action: string | null,
): ErrorRoute {
  if (code === 'shutting_down') return { kind: 'shutdown', message };
  if (action === 'settings' || ['auth', 'quota', 'unconfigured', 'model_unavailable'].includes(code)) {
    return { kind: 'settings', message };
  }
  if (code === 'busy') return { kind: 'toast', message };
  if (retryable) return { kind: 'retry', message };
  return { kind: 'info', message };
}

/** 回合用量脚注（Q9：回合级数字，看板 M2）。 */
export interface UsageNote {
  capability: string;
  model: string;
  promptTokens?: number;
  completionTokens?: number;
  characters?: number;
}

/** 解析 SSE Usage 事件载荷（LLM tokens / TTS characters；未知形状降级仅 model）。 */
export function parseUsage(capability: string, model: string, usage: unknown): UsageNote {
  const note: UsageNote = { capability, model };
  if (usage && typeof usage === 'object') {
    const u = usage as Record<string, unknown>;
    if (typeof u.prompt_tokens === 'number') note.promptTokens = u.prompt_tokens;
    if (typeof u.completion_tokens === 'number') note.completionTokens = u.completion_tokens;
    if (typeof u.characters === 'number') note.characters = u.characters;
  }
  return note;
}