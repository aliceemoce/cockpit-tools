export const VALID_ACCOUNTS_FILTER_VALUE = '__valid_accounts__' as const;

type Translate = (key: string, options?: Record<string, unknown>) => string;

export function buildValidAccountsFilterOption(
  t: Translate,
  count: number,
): { value: typeof VALID_ACCOUNTS_FILTER_VALUE; label: string } {
  return {
    value: VALID_ACCOUNTS_FILTER_VALUE,
    label: t('common.shared.filter.validAccounts', {
      count,
      defaultValue: '有效账号 ({{count}})',
    }),
  };
}

export function splitValidityFilterValues(values: Iterable<string>): {
  requireValidAccounts: boolean;
  selectedTypes: Set<string>;
} {
  const selectedTypes = new Set<string>();
  let requireValidAccounts = false;
  for (const value of values) {
    if (value === VALID_ACCOUNTS_FILTER_VALUE) {
      requireValidAccounts = true;
      continue;
    }
    selectedTypes.add(value);
  }
  return { requireValidAccounts, selectedTypes };
}

export function isAccountSessionExpired(errorText: string | null | undefined): boolean {
  if (!errorText) return false;
  const lower = errorText.toLowerCase();
  return (
    lower.includes('会话已过期') ||
    lower.includes('未认证') ||
    lower.includes('请重新导入') ||
    lower.includes('请重新登录') ||
    lower.includes('登录会话已过期') ||
    lower.includes('授权已失效') ||
    lower.includes('凭证无效') ||
    lower.includes('凭证失效') ||
    lower.includes('token已失效') ||
    lower.includes('token 已失效') ||
    lower.includes('登录授权已过期') ||
    lower.includes('session expired') ||
    lower.includes('invalid credentials') ||
    lower.includes('unauthenticated') ||
    lower.includes('re-import') ||
    lower.includes('re-login')
  );
}
