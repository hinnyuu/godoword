// 领域服务层（shell 组件调这些函数，不裸调 api）。封装 openapi-fetch + CSRF header param + 错误归一。
// 类型由 OpenAPI 生成（单一事实源）。变更请求须 params.header.X-Requested-With（生成类型 required）。

import { api, CSRF_VALUE } from './client';
import type { components } from './schema';

type Schemas = components['schemas'];
export type UserInfo = Schemas['UserInfo'];
export type Provider = Schemas['Provider'];
export type Binding = Schemas['Binding'];
export type Folder = Schemas['Folder'];
export type SessionListItem = Schemas['SessionListItem'];
export type SessionDetail = Schemas['SessionDetailResp'];
export type AudioMeta = Schemas['AudioMetaResp'];
export type ModelInfo = Schemas['ModelInfo'];
export type VoiceInfo = Schemas['VoiceInfo'];
export type ConfigView = Schemas['ConfigView'];

/** CSRF header params（变更请求复用）。 */
const csrf = { header: { 'X-Requested-With': CSRF_VALUE } } as const;

/** 统一错误（携带 HTTP 状态供 UI 分流）。 */
export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

function fail(response: { status: number } | undefined, fallback: string): never {
  throw new ApiError(response?.status ?? 0, fallback);
}

// ── 系统 ──
export async function getConfig(): Promise<ConfigView> {
  const { data, error, response } = await api.GET('/api/v1/config');
  if (error || !data) fail(response, '加载配置失败');
  return data;
}

// ── 认证 ──
export async function login(username: string, password: string): Promise<UserInfo> {
  const { data, error, response } = await api.POST('/api/v1/auth/login', {
    body: { username, password },
    params: csrf,
  });
  if (error || !data) fail(response, '登录失败');
  return data;
}

export async function register(
  username: string,
  password: string,
  displayName?: string,
  inviteCode?: string,
): Promise<UserInfo> {
  const { data, error, response } = await api.POST('/api/v1/auth/register', {
    body: {
      username,
      password,
      ...(displayName ? { display_name: displayName } : {}),
      ...(inviteCode ? { invite_code: inviteCode } : {}),
    },
    params: csrf,
  });
  if (error || !data) fail(response, '注册失败');
  return data;
}

export async function logout(): Promise<void> {
  await api.POST('/api/v1/auth/logout', { params: csrf });
}

export async function getMe(): Promise<UserInfo> {
  const { data, error, response } = await api.GET('/api/v1/auth/me');
  if (error || !data) fail(response, '加载用户失败');
  return data;
}

export async function changePassword(oldPassword: string, newPassword: string): Promise<void> {
  const { error, response } = await api.POST('/api/v1/auth/change-password', {
    body: { old_password: oldPassword, new_password: newPassword },
    params: csrf,
  });
  if (error) fail(response, '改密失败');
}

export async function updateMe(displayName: string): Promise<UserInfo> {
  const { data, error, response } = await api.PUT('/api/v1/auth/me', {
    body: { display_name: displayName },
    params: csrf,
  });
  if (error || !data) fail(response, '更新显示名失败');
  return data;
}

export async function resetPassword(token: string, newPassword: string): Promise<void> {
  const { error, response } = await api.POST('/api/v1/auth/reset-password', {
    body: { token, new_password: newPassword },
    params: csrf,
  });
  if (error) fail(response, '重置密码失败');
}

// ── 管理 ──
export async function adminUsers(): Promise<UserInfo[]> {
  const { data, error, response } = await api.GET('/api/v1/admin/users');
  if (error || !data) fail(response, '加载用户列表失败');
  return data;
}

export async function adminSetStatus(userId: string, status: string): Promise<void> {
  const { error, response } = await api.POST('/api/v1/admin/users/{id}/status', {
    params: { path: { id: userId }, ...csrf },
    body: { status },
  });
  if (error) fail(response, '设置状态失败');
}

export async function adminCreateInvite(maxUses: number): Promise<{ code: string; max_uses: number }> {
  const { data, error, response } = await api.POST('/api/v1/admin/invites', {
    body: { max_uses: maxUses },
    params: csrf,
  });
  if (error || !data) fail(response, '生成邀请码失败');
  return data as { code: string; max_uses: number };
}

export async function adminResetToken(userId: string): Promise<string> {
  const { data, error, response } = await api.POST('/api/v1/admin/reset', {
    body: { user_id: userId },
    params: csrf,
  });
  if (error || !data) fail(response, '生成重置令牌失败');
  return (data as { reset_token: string }).reset_token;
}

// ── 会话 ──
export async function listSessions(folder?: string, module?: string): Promise<SessionListItem[]> {
  const { data, error, response } = await api.GET('/api/v1/sessions', {
    params: { query: { ...(folder ? { folder } : {}), ...(module ? { module } : {}) } },
  });
  if (error || !data) fail(response, '加载会话列表失败');
  return data.sessions;
}

export async function createSession(onDisconnect?: string): Promise<string> {
  const { data, error, response } = await api.POST('/api/v1/sessions', {
    body: onDisconnect ? { on_disconnect: onDisconnect } : {},
    params: csrf,
  });
  if (error || !data) fail(response, '创建会话失败');
  return (data as { id: string }).id;
}

export async function getSession(id: string): Promise<SessionDetail> {
  const { data, error, response } = await api.GET('/api/v1/sessions/{id}', {
    params: { path: { id } },
  });
  if (error || !data) fail(response, '加载会话失败');
  return data;
}

export async function deleteSession(id: string): Promise<void> {
  const { error, response } = await api.DELETE('/api/v1/sessions/{id}', {
    params: { path: { id }, ...csrf },
  });
  if (error) fail(response, '删除会话失败');
}

export async function moveSession(id: string, path: string | null): Promise<void> {
  const { error, response } = await api.PUT('/api/v1/sessions/{id}/path', {
    params: { path: { id }, ...csrf },
    body: { path },
  });
  if (error) fail(response, '移动会话失败');
}

export async function cancelSession(id: string): Promise<void> {
  await api.POST('/api/v1/sessions/{id}/cancel', { params: { path: { id }, ...csrf } });
}

export async function getAudioMeta(id: string): Promise<AudioMeta> {
  const { data, error, response } = await api.GET('/api/v1/audio/{id}/meta', {
    params: { path: { id } },
  });
  if (error || !data) fail(response, '加载音频元数据失败');
  return data;
}

/** 回放音频 PCM 字节（裸 PCM，x-sample-rate 头）。 */
export async function getAudioPcm(id: string): Promise<{ bytes: Uint8Array; sampleRate: number }> {
  const res = await fetch(`/api/v1/audio/${id}`);
  if (!res.ok) throw new ApiError(res.status, '加载音频失败');
  const sampleRate = Number(res.headers.get('x-sample-rate') ?? '24000');
  return { bytes: new Uint8Array(await res.arrayBuffer()), sampleRate };
}

// ── 文件夹 ──
export async function listFolders(module?: string): Promise<Folder[]> {
  const { data, error, response } = await api.GET('/api/v1/folders', {
    params: { query: module ? { module } : {} },
  });
  if (error || !data) fail(response, '加载文件夹失败');
  return data.folders;
}

export async function createFolder(path: string, module?: string): Promise<Folder> {
  const { data, error, response } = await api.POST('/api/v1/folders', {
    params: { query: module ? { module } : {}, ...csrf },
    body: { path },
  });
  if (error || !data) fail(response, '创建文件夹失败');
  return data;
}

export async function renameFolder(id: string, path: string): Promise<Folder> {
  const { data, error, response } = await api.PUT('/api/v1/folders/{id}', {
    params: { path: { id }, ...csrf },
    body: { path },
  });
  if (error || !data) fail(response, '重命名文件夹失败');
  return data;
}

export async function deleteFolder(id: string): Promise<void> {
  const { error, response } = await api.DELETE('/api/v1/folders/{id}', {
    params: { path: { id }, ...csrf },
  });
  if (error) fail(response, '删除文件夹失败');
}

// ── provider ──
export async function listProviders(): Promise<Provider[]> {
  const { data, error, response } = await api.GET('/api/v1/providers');
  if (error || !data) fail(response, '加载服务商失败');
  return data;
}

export async function createProvider(
  adapterId: string,
  label: string,
  baseUrl: string,
  key: string,
): Promise<Provider> {
  const { data, error, response } = await api.POST('/api/v1/providers', {
    body: { adapter_id: adapterId, label, base_url: baseUrl, key },
    params: csrf,
  });
  if (error || !data) fail(response, '创建服务商失败');
  return data;
}

export async function updateProvider(
  id: string,
  label: string,
  baseUrl: string,
  key?: string,
): Promise<Provider> {
  const { data, error, response } = await api.PUT('/api/v1/providers/{id}', {
    params: { path: { id }, ...csrf },
    body: { label, base_url: baseUrl, ...(key ? { key } : {}) },
  });
  if (error || !data) fail(response, '更新服务商失败');
  return data;
}

export async function deleteProvider(id: string): Promise<void> {
  const { error, response } = await api.DELETE('/api/v1/providers/{id}', {
    params: { path: { id }, ...csrf },
  });
  if (error) fail(response, '删除服务商失败');
}

export async function providerModels(id: string): Promise<ModelInfo[]> {
  const { data, error, response } = await api.GET('/api/v1/providers/{id}/models', {
    params: { path: { id } },
  });
  if (error || !data) fail(response, '拉取模型失败');
  return data;
}

export async function providerVoices(id: string, model?: string): Promise<VoiceInfo[]> {
  const { data, error, response } = await api.GET('/api/v1/providers/{id}/voices', {
    params: { path: { id }, query: model ? { model } : {} },
  });
  if (error || !data) fail(response, '拉取音色失败');
  return data;
}

export async function previewVoice(id: string, model: string, voice: string, text?: string): Promise<string> {
  const { data, error, response } = await api.POST('/api/v1/providers/{id}/preview', {
    params: { path: { id }, ...csrf },
    body: { model, voice, ...(text ? { text } : {}) },
  });
  if (error || !data) fail(response, '试听失败');
  return (data as { audio_id: string }).audio_id;
}

// ── binding ──
export async function listBindings(): Promise<Binding[]> {
  const { data, error, response } = await api.GET('/api/v1/bindings');
  if (error || !data) fail(response, '加载绑定失败');
  return data;
}

export async function createBinding(
  capability: string,
  role: string,
  providerId: string,
  model: string,
  params?: Record<string, unknown>,
  moduleId = '*',
): Promise<Binding> {
  const { data, error, response } = await api.POST('/api/v1/bindings', {
    body: {
      module_id: moduleId,
      capability,
      role,
      provider_id: providerId,
      model,
      ...(params ? { params } : {}),
    },
    params: csrf,
  });
  if (error || !data) fail(response, '创建绑定失败');
  return data;
}

export async function updateBinding(
  id: string,
  providerId: string,
  model: string,
  params?: Record<string, unknown>,
): Promise<Binding> {
  const { data, error, response } = await api.PUT('/api/v1/bindings/{id}', {
    params: { path: { id }, ...csrf },
    body: { provider_id: providerId, model, ...(params ? { params } : {}) },
  });
  if (error || !data) fail(response, '更新绑定失败');
  return data;
}

export async function deleteBinding(id: string): Promise<void> {
  const { error, response } = await api.DELETE('/api/v1/bindings/{id}', {
    params: { path: { id }, ...csrf },
  });
  if (error) fail(response, '删除绑定失败');
}