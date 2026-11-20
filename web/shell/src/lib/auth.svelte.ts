// 认证状态（Svelte 5 runes 模块 store）。shell 组件共享。
import {
  getMe,
  login as apiLogin,
  register as apiRegister,
  logout as apiLogout,
  type UserInfo,
} from '@godoword/core';

class AuthStore {
  user = $state<UserInfo | null>(null);
  loading = $state(true);

  /** 启动加载当前用户（cookie 有效则已登录）。 */
  async load(): Promise<void> {
    try {
      this.user = await getMe();
    } catch {
      this.user = null;
    } finally {
      this.loading = false;
    }
  }

  async login(username: string, password: string): Promise<void> {
    this.user = await apiLogin(username, password);
  }

  async register(username: string, password: string, displayName?: string, inviteCode?: string): Promise<void> {
    this.user = await apiRegister(username, password, displayName, inviteCode);
  }

  async logout(): Promise<void> {
    await apiLogout();
    this.user = null;
  }

  get isAdmin(): boolean {
    return this.user?.is_admin ?? false;
  }
}

export const auth = new AuthStore();