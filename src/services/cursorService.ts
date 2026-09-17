import { invoke } from '@tauri-apps/api/core';
import { CursorAccount } from '../types/cursor';

export interface CursorOAuthLoginStartResponse {
  loginId: string;
  verificationUri: string;
  expiresIn: number;
  intervalSeconds: number;
}

/** Cursor OAuth: 开始登录（生成 PKCE，返回浏览器 URL） */
export async function startCursorOAuthLogin(): Promise<CursorOAuthLoginStartResponse> {
  return await invoke('cursor_oauth_login_start');
}

/** Cursor OAuth: 等待轮询完成（用户在浏览器完成登录后返回账号） */
export async function completeCursorOAuthLogin(loginId: string): Promise<CursorAccount> {
  return await invoke('cursor_oauth_login_complete', { loginId });
}

/** Cursor OAuth: 取消登录 */
export async function cancelCursorOAuthLogin(loginId?: string): Promise<void> {
  return await invoke('cursor_oauth_login_cancel', { loginId: loginId ?? null });
}

export async function listCursorAccounts(): Promise<CursorAccount[]> {
  return await invoke('list_cursor_accounts');
}

export type CursorAccountListPage = {
  accounts: CursorAccount[];
  total: number;
  offset: number;
  nextOffset: number;
  hasMore: boolean;
};

export async function listCursorAccountsPage(
  offset: number,
  limit = 200,
): Promise<CursorAccountListPage> {
  const page = await invoke<{
    accounts: CursorAccount[];
    total: number;
    offset: number;
    next_offset: number;
    has_more: boolean;
  }>('list_cursor_accounts_page', { offset, limit });
  return {
    accounts: page.accounts,
    total: page.total,
    offset: page.offset,
    nextOffset: page.next_offset,
    hasMore: page.has_more,
  };
}

export async function deleteCursorAccount(accountId: string): Promise<void> {
  return await invoke('delete_cursor_account', { accountId });
}

export async function deleteCursorAccounts(accountIds: string[]): Promise<void> {
  return await invoke('delete_cursor_accounts', { accountIds });
}

export async function importCursorFromJson(jsonContent: string): Promise<CursorAccount[]> {
  return await invoke('import_cursor_from_json', { jsonContent });
}

export async function importCursorFromLocal(): Promise<CursorAccount[]> {
  return await invoke('import_cursor_from_local');
}

export async function exportCursorAccounts(accountIds: string[]): Promise<string> {
  return await invoke('export_cursor_accounts', { accountIds });
}

export async function refreshCursorToken(accountId: string): Promise<CursorAccount> {
  return await invoke('refresh_cursor_token', { accountId });
}

/** 0012：maxDurationSecs 为墙钟预算（不按条数截断）时可滚动覆盖全池。 */
export async function refreshAllCursorTokens(
  maxCount?: number | null,
  maxDurationSecs?: number | null,
): Promise<number> {
  return await invoke('refresh_all_cursor_tokens', {
    maxCount: maxCount ?? null,
    maxDurationSecs: maxDurationSecs ?? null,
  });
}

/** 0012：自动刷新每轮墙钟预算（秒）。不按 120 条截断，最旧优先滚动覆盖全池。 */
export const CURSOR_AUTO_REFRESH_WINDOW_SECS = 240;
/** @deprecated 仅作兼容保留：不再作为每轮条数硬上限，见 CURSOR_AUTO_REFRESH_WINDOW_SECS */
export const CURSOR_AUTO_REFRESH_BATCH_SIZE = 120;

export async function addCursorAccountWithToken(accessToken: string): Promise<CursorAccount> {
  return await invoke('add_cursor_account_with_token', { accessToken });
}

/** 续杯管家云端拉号：只进账号池，不写入 Cursor */
export async function pullCursorAccountFromXubei(): Promise<CursorAccount> {
  return await invoke('pull_cursor_account_from_xubei');
}

/** 续杯管家无感换号：云端拉号 + wuxian 热替换 + 写入当前 Cursor */
export async function switchCursorAccountFromXubeiSeamless(): Promise<CursorAccount> {
  return await invoke('switch_cursor_account_from_xubei_seamless');
}

/** 续杯管家换号：云端 switch + 写入当前 Cursor（与原版「换号」一致；GUI「换号」按钮走此命令） */
export async function switchCursorAccountFromXubeiPool(): Promise<CursorAccount> {
  return await invoke('switch_cursor_account_from_xubei_pool');
}

/** 只读：续杯 get-token 当前邮箱 */
export async function readWuxianGetTokenEmail(): Promise<string | null> {
  return await invoke('read_wuxian_get_token_email');
}

export type RenewalProgramStatus = {
  program: 'assistant' | 'xubei' | 'wuyou';
  materialReady: boolean;
  backendReady: boolean;
  processRunning: boolean;
  loggedIn: boolean;
  loginUsername: string | null;
  seamlessEmail: string | null;
  getTokenEmail: string | null;
  seamlessSource: string | null;
  seamlessEnabled: boolean;
  autoSwitch: boolean;
  autoResetMachine: boolean;
  autoSendContinue: boolean;
  injectReady: boolean;
  cursorVersion: string | null;
  expiryHint: string | null;
  activeTokenHint: string | null;
  note: string | null;
  /** 续杯自动换号驱动相位（idle/checking/switching/success/failed/cooldown/disabled） */
  autoSwitchPhase?: string | null;
  autoSwitchLastResult?: string | null;
  autoSwitchLastError?: string | null;
  autoSwitchCooldownRemainingSecs?: number | null;
  autoSwitchDriverActive?: boolean | null;
  usagePercent?: number | null;
  /** WP-8 / G3：仅 true 时才允许显示「无限额度」；null/undefined/false 均不得显示 */
  quotaUnlimited?: boolean | null;
  /** 与默认页自动换号分账说明 */
  autoSwitchScopeNote?: string | null;
  /** WP-5 / D2：三开关各自的取值来源（磁盘层 / 默认值），用于区分「用户偏好」与「缺省」 */
  autoSwitchSource?: string | null;
  autoResetMachineSource?: string | null;
  autoSendContinueSource?: string | null;
  /** WP-5 / D3：指定 Cursor 路径 + 来源；active 表示后续解析是否真用它 */
  cursorPathConfigured?: string | null;
  cursorPathSource?: string | null;
  cursorPathActive?: boolean | null;
  /** WP-5 / D4：推荐 Cursor 版本 + 可追溯来源 */
  recommendedVersion?: string | null;
  recommendedVersionSource?: string | null;
  /** WP-5 / D2：偏好是否真的从磁盘读到了。false 时三开关值无意义，UI 必须显示「未知」。 */
  prefsReadable?: boolean;
};

export type RenewalConsoleStatus = {
  programs: RenewalProgramStatus[];
  defaultCursorCachedEmail: string | null;
};

export async function getRenewalConsoleStatus(): Promise<RenewalConsoleStatus> {
  return await invoke('get_renewal_console_status');
}

export type XubeiRenewalPrefs = {
  seamlessEnabled: boolean;
  autoSwitch: boolean;
  autoResetMachine: boolean;
  autoSendContinue: boolean;
};

export type XubeiRenewalPrefKey =
  | 'seamlessEnabled'
  | 'autoSwitch'
  | 'autoResetMachine'
  | 'autoSendContinue';

export async function getXubeiRenewalPrefs(): Promise<XubeiRenewalPrefs> {
  return await invoke('get_xubei_renewal_prefs');
}

export async function setXubeiRenewalPref(
  key: XubeiRenewalPrefKey,
  value: boolean,
): Promise<XubeiRenewalPrefs> {
  return await invoke('set_xubei_renewal_pref', { key, value });
}

export type RenewalAppsSyncResult = {
  ok: boolean;
  reportPath: string;
  report: Record<string, unknown> | null;
  error: string | null;
};

export async function syncRenewalAppsAutoUpdate(
  launchXubei = false,
): Promise<RenewalAppsSyncResult> {
  return await invoke('sync_renewal_apps_auto_update', { launchXubei });
}

export async function updateCursorAccountTags(accountId: string, tags: string[]): Promise<CursorAccount> {
  return await invoke('update_cursor_account_tags', { accountId, tags });
}

export async function getCursorAccountsIndexPath(): Promise<string> {
  return await invoke('get_cursor_accounts_index_path');
}

export async function injectCursorAccount(accountId: string): Promise<string> {
  return await invoke('inject_cursor_account', { accountId });
}

export type WuyouNativeStatus = {
  materialReady: boolean;
  installed: boolean;
  fingerprintVerified: boolean;
  traditionalSwitchReady: boolean;
  cloudPullAvailable: boolean;
  seamlessAvailable: boolean;
  backendReady: boolean;
  referenceAsarSha256?: string | null;
  installAsarSha256?: string | null;
  installAsarPath?: string | null;
  referenceAsarPath?: string | null;
  accountSourceTag: string;
  note: string;
};

export async function getWuyouNativeStatus(): Promise<WuyouNativeStatus> {
  return await invoke('get_wuyou_native_status');
}

export async function wuyouTraditionalSwitch(accountId: string): Promise<string> {
  return await invoke('wuyou_traditional_switch', { accountId });
}

/** 默认自动换号：与多开「启动」同构（自动选号），不指定账号 */
export async function injectCursorAccountAuto(): Promise<string> {
  return await invoke('inject_cursor_account_auto');
}

/** 续杯管家 Basic 注入（workbench i0/i1/i2 + exthost + main）；alone 不依赖管家进程 */
export async function injectXubeiBasic(allowLiveWrite = false): Promise<string> {
  return await invoke('inject_xubei_basic', { allowLiveWrite });
}

/** 0012：当前账号实时额度快照（强制拉 usage；查不到 queried=false，禁止当满额度） */
export type CursorCurrentQuotaSnapshot = {
  accountId: string | null;
  queried: boolean;
  remainingPercent: number | null;
  error?: string | null;
  account?: CursorAccount | null;
};

export async function getCursorCurrentQuotaRealtime(): Promise<CursorCurrentQuotaSnapshot> {
  const raw = await invoke<{
    accountId: string | null;
    queried: boolean;
    remainingPercent: number | null;
    error?: string | null;
    account?: CursorAccount | null;
  }>('refresh_cursor_current_account_realtime');
  return {
    accountId: raw.accountId ?? null,
    queried: raw.queried === true,
    remainingPercent: raw.remainingPercent ?? null,
    error: raw.error ?? null,
    account: raw.account ?? null,
  };
}

export async function probeCursorAccountChat(accountId: string): Promise<CursorAccount> {
  return await invoke('probe_cursor_account_chat', { accountId });
}

export async function probeCursorAccountsChat(accountIds: string[]): Promise<CursorAccount[]> {
  return await invoke('probe_cursor_accounts_chat', { accountIds });
}

/** 指定 Cursor 路径：打开文件对话框选择 Cursor.exe */
export async function pickCursorPath(): Promise<string> {
  return await invoke('pick_cursor_path');
}

/** 手动发送继续：向 wuxian 本地服务发送 resume 信号 */
export async function manualSendContinue(): Promise<string> {
  return await invoke('manual_send_continue');
}

/** 重置 Cursor 机器码 */
export async function resetCursorMachineId(): Promise<string> {
  return await invoke('reset_cursor_machine_id');
}

/** 还原续杯注入（从 .bak 备份恢复被注入的 Cursor 文件） */
export async function restoreCursorInjection(): Promise<string> {
  return await invoke('restore_cursor_injection');
}

/** 启动 Cockpit 内置 wuxian 无感 HTTP 服务 */
export async function startWuxianSeamlessServer(): Promise<Record<string, unknown>> {
  return await invoke('start_wuxian_seamless_server');
}

/** 停止 Cockpit 内置 wuxian 无感 HTTP 服务 */
export async function stopWuxianSeamlessServer(): Promise<Record<string, unknown>> {
  return await invoke('stop_wuxian_seamless_server');
}

/** 查询 wuxian 无感服务状态 */
export async function getWuxianSeamlessServerStatus(): Promise<Record<string, unknown>> {
  return await invoke('get_wuxian_seamless_server_status');
}

/** 激活卡密 */
export async function activateCardKey(cardKey: string): Promise<Record<string, unknown>> {
  return await invoke('activate_card_key', { cardKey });
}

/** 查授权：对齐原版 GET /api/v1/license */
export async function verifyCardKey(): Promise<Record<string, unknown>> {
  return await invoke('verify_card_key');
}

/** 修改续杯管家密码 */
export async function changeXubeiPassword(
  oldPassword: string,
  newPassword: string,
): Promise<Record<string, unknown>> {
  return await invoke('change_xubei_password', { oldPassword, newPassword });
}

/** 退出续杯管家登录 */
export async function logoutXubei(): Promise<Record<string, unknown>> {
  return await invoke('logout_xubei');
}
