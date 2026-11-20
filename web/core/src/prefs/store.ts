// 用户偏好 store（跨设备随账户，经 API GET/PUT）。类型由 OpenAPI 生成（单一事实源）。

import { api, CSRF_VALUE } from '../api/client';
import type { components } from '../api/schema';

/** 用户偏好（生成类型）。 */
export type Preferences = components['schemas']['Preferences'];

/** 加载偏好（无则后端返回默认）。 */
export async function loadPreferences(): Promise<Preferences> {
  const { data, error } = await api.GET('/api/v1/preferences');
  if (error || !data) throw new Error('加载偏好失败');
  return data;
}

/** 保存偏好（last-write-wins）。CSRF header 经 middleware 注入，此处显式传满足生成类型 required。 */
export async function savePreferences(prefs: Preferences): Promise<Preferences> {
  const { data, error } = await api.PUT('/api/v1/preferences', {
    body: prefs,
    params: { header: { 'X-Requested-With': CSRF_VALUE } },
  });
  if (error || !data) throw new Error('保存偏好失败');
  return data;
}