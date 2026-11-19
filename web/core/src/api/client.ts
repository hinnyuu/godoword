// 类型化 API 客户端（openapi-fetch + 生成类型，单一事实源）。
// 同源 cookie 认证（fetch 默认 same-origin 携 cookie）；变更请求自动注入 CSRF 头 X-Requested-With。
// 401 经 onUnauthorized 钩子（shell 跳登录带返回路径）。

import createClient from 'openapi-fetch';
import type { paths } from './schema';

/** CSRF 自定义头（与后端 csrf_guard 一致）。 */
export const CSRF_HEADER = 'X-Requested-With';
export const CSRF_VALUE = 'godoword';

const MUTATING = new Set(['POST', 'PUT', 'DELETE', 'PATCH']);

/** 401 回调（shell 注入：跳登录）。 */
export type UnauthorizedHandler = () => void;

let onUnauthorized: UnauthorizedHandler | null = null;

/** 注册 401 处理器（shell 启动时调用）。 */
export function setUnauthorizedHandler(handler: UnauthorizedHandler): void {
  onUnauthorized = handler;
}

/** 类型化 API 客户端（同源，baseUrl 空）。 */
export const api = createClient<paths>({ baseUrl: '' });

api.use({
  onRequest({ request }) {
    if (MUTATING.has(request.method)) {
      request.headers.set(CSRF_HEADER, CSRF_VALUE);
    }
    return request;
  },
  onResponse({ response }) {
    if (response.status === 401 && onUnauthorized) {
      onUnauthorized();
    }
    return response;
  },
});

export type { paths, components, operations } from './schema';