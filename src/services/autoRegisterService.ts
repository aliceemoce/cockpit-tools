import { invoke } from '@tauri-apps/api/core';

export interface AutoRegisterParams {
  email: string;
  emailPassword: string;
  firstName: string;
  lastName: string;
  proxyUrl?: string;
}

export interface AutoRegisterResult {
  success: boolean;
  ssoToken?: string;
  name?: string;
  error?: string;
}

export interface SsoTokenImportParams {
  ssoToken: string;
  region?: string;
}

export interface SsoTokenImportResult {
  success: boolean;
  data?: {
    email: string;
    userId: string;
    accessToken: string;
    refreshToken: string;
    clientId: string;
    clientSecret: string;
    region: string;
    expiresIn: number;
    idp: string;
    subscriptionType: string;
    subscriptionTitle: string;
    usage: { current: number; limit: number };
  };
  error?: string;
}

/**
 * Kiro 自动注册
 * 使用 Playwright 自动完成 AWS Builder ID 注册流程
 */
export async function autoRegisterKiro(params: AutoRegisterParams): Promise<AutoRegisterResult> {
  return await invoke('auto_register_kiro', {
    params: {
      email: params.email,
      emailPassword: params.emailPassword,
      firstName: params.firstName,
      lastName: params.lastName,
      proxyUrl: params.proxyUrl,
    }
  });
}

/**
 * 从 SSO Token 导入账号
 * 将注册成功后获取的 SSO Token 转换为账号凭证
 */
export async function importFromSsoToken(
  ssoToken: string,
  region: string = 'us-east-1',
  email?: string,
  name?: string
): Promise<SsoTokenImportResult> {
  return await invoke('import_from_sso_token', {
    params: {
      bearerToken: ssoToken,
      region,
      email,
      name,
    }
  });
}

/**
 * Windsurf 自动注册
 * 使用 Playwright 自动完成 Windsurf OAuth 授权流程
 */
export async function autoRegisterWindsurf(params: {
  email?: string;
  name?: string;
  proxyUrl?: string;
  browserPath?: string;
}): Promise<{
  success: boolean;
  data?: {
    accessToken: string;
    tokenType: string;
    expiresIn: number;
    email: string;
    name: string;
    loginProvider: string;
  };
  error?: string;
}> {
  return await invoke('auto_register_windsurf', {
    params: {
      email: params.email,
      name: params.name,
      proxyUrl: params.proxyUrl,
      browserPath: params.browserPath,
    }
  });
}

/**
 * 监听自动注册日志
 * 通过 Tauri 的 event 系统接收实时日志
 */
export function onAutoRegisterLog(callback: (email: string, message: string) => void): () => void {
  // 使用 window.__TAURI__.event 监听事件
  const handler = (event: { payload: { email: string; message: string } }) => {
    callback(event.payload.email, event.payload.message);
  };

  // 动态导入 Tauri event API
  import('@tauri-apps/api/event').then(({ listen }) => {
    listen('auto-register-log', handler);
  });

  // 返回取消监听函数（简化版，实际需要保存 unlisten 函数）
  return () => {
    // 实际取消逻辑需要在 listen 回调中处理
  };
}
