import {
  WindsurfAccount,
  getWindsurfAccountDisplayEmail,
  getWindsurfPlanBadge,
  getWindsurfUsage,
} from '../types/windsurf';
import * as windsurfService from '../services/windsurfService';
import { getProviderCurrentAccountId } from '../services/providerCurrentAccountService';
import { createProviderAccountStore } from './createProviderAccountStore';

const WINDSURF_ACCOUNTS_CACHE_KEY = 'agtools.windsurf.accounts.cache';
const WINDSURF_CURRENT_ACCOUNT_ID_KEY = 'agtools.windsurf.current_account_id';

function projectWindsurfAccountsForCache(accounts: WindsurfAccount[]): WindsurfAccount[] {
  return accounts.map((account) => ({
    id: account.id,
    github_login: account.github_login,
    github_id: account.github_id,
    github_name: account.github_name ?? null,
    github_email: account.github_email ?? null,
    tags: account.tags ?? null,
    github_access_token: '',
    github_token_type: null,
    github_scope: null,
    copilot_token: '',
    copilot_plan: account.copilot_plan ?? null,
    copilot_chat_enabled: account.copilot_chat_enabled ?? null,
    copilot_expires_at: account.copilot_expires_at ?? null,
    copilot_refresh_in: account.copilot_refresh_in ?? null,
    copilot_quota_snapshots: null,
    copilot_quota_reset_date: account.copilot_quota_reset_date ?? null,
    copilot_limited_user_quotas: null,
    copilot_limited_user_reset_date: account.copilot_limited_user_reset_date ?? null,
    windsurf_api_key: null,
    windsurf_api_server_url: account.windsurf_api_server_url ?? null,
    windsurf_auth_token: null,
    windsurf_user_status: null,
    windsurf_plan_status: null,
    windsurf_auth_status_raw: null,
    quota_query_last_error: account.quota_query_last_error ?? null,
    quota_query_last_error_at: account.quota_query_last_error_at ?? null,
    created_at: account.created_at,
    last_used: account.last_used,
    email: account.email,
    plan_type: account.plan_type,
    quota: account.quota,
  }));
}

export const useWindsurfAccountStore = createProviderAccountStore<WindsurfAccount>(
  WINDSURF_ACCOUNTS_CACHE_KEY,
  {
    listAccounts: windsurfService.listWindsurfAccounts,
    deleteAccount: windsurfService.deleteWindsurfAccount,
    deleteAccounts: windsurfService.deleteWindsurfAccounts,
    injectAccount: windsurfService.injectWindsurfToVSCode,
    refreshToken: windsurfService.refreshWindsurfToken,
    refreshAllTokens: windsurfService.refreshAllWindsurfTokens,
    importFromJson: windsurfService.importWindsurfFromJson,
    exportAccounts: windsurfService.exportWindsurfAccounts,
    updateAccountTags: windsurfService.updateWindsurfAccountTags,
  },
  {
    getDisplayEmail: getWindsurfAccountDisplayEmail,
    getPlanBadge: getWindsurfPlanBadge,
    getUsage: getWindsurfUsage,
  },
  {
    platformId: 'windsurf',
    currentAccountIdKey: WINDSURF_CURRENT_ACCOUNT_ID_KEY,
    resolveCurrentAccountId: () => getProviderCurrentAccountId('windsurf'),
    enableAccountsCache: true,
    projectAccountsForCache: projectWindsurfAccountsForCache,
  },
);
