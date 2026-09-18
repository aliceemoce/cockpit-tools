import { useTranslation } from 'react-i18next';
import { PlatformInstancesContent } from '../components/platform/PlatformInstancesContent';
import { useCursorInstanceStore } from '../stores/useCursorInstanceStore';
import { useCursorAccountStore } from '../stores/useCursorAccountStore';
import type { CursorAccount } from '../types/cursor';
import { usePlatformRuntimeSupport } from '../hooks/usePlatformRuntimeSupport';
import {
  buildCursorAccountPresentation,
  buildQuotaPreviewLines,
} from '../presentation/platformAccountPresentation';

const EMPTY_CURSOR_INSTANCE_ACCOUNTS: CursorAccount[] = [];
const CURSOR_INSTANCE_SELECT_LIMIT = 200;

interface CursorInstancesContentProps {
  accountsForSelect?: CursorAccount[];
}

export function CursorInstancesContent({
  accountsForSelect,
}: CursorInstancesContentProps = {}) {
  const { t } = useTranslation();
  const instanceStore = useCursorInstanceStore();
  const fetchAccounts = useCursorAccountStore((state) => state.fetchAccounts);
  // 父组件已传选择列表时不订 store 全表，避免四千账号更新拖死多开页
  const storeAccounts = useCursorAccountStore((state) =>
    accountsForSelect ? EMPTY_CURSOR_INSTANCE_ACCOUNTS : state.accounts,
  );
  const limitedStoreAccounts =
    !accountsForSelect && storeAccounts.length > CURSOR_INSTANCE_SELECT_LIMIT
      ? storeAccounts.slice(0, CURSOR_INSTANCE_SELECT_LIMIT)
      : storeAccounts;
  const sourceAccounts = accountsForSelect ?? limitedStoreAccounts;
  const isSupportedPlatform = usePlatformRuntimeSupport('desktop');

  const renderCursorQuotaPreview = (account: CursorAccount) => {
    const presentation = buildCursorAccountPresentation(account, t);
    const lines = buildQuotaPreviewLines(presentation.quotaItems, 3);
    if (lines.length === 0) {
      return <span className="account-quota-empty">{t('instances.quota.empty', '暂无配额缓存')}</span>;
    }
    return (
      <div className="account-quota-preview">
        {lines.map((line) => (
          <span className="account-quota-item" key={line.key}>
            <span className={`quota-dot ${line.quotaClass}`} />
            <span className={`quota-text ${line.quotaClass}`}>{line.text}</span>
          </span>
        ))}
      </div>
    );
  };

  return (
    <PlatformInstancesContent<CursorAccount>
      instanceStore={instanceStore}
      accounts={sourceAccounts}
      fetchAccounts={fetchAccounts}
      renderAccountQuotaPreview={renderCursorQuotaPreview}
      renderAccountBadge={(account) => {
        const presentation = buildCursorAccountPresentation(account, t);
        return (
          <span className={`instance-plan-badge cursor-plan-badge ${presentation.planClass}`}>
            {presentation.planLabel}
          </span>
        );
      }}
      getAccountSearchText={(account) => {
        const presentation = buildCursorAccountPresentation(account, t);
        return `${presentation.displayName} ${presentation.planLabel}`;
      }}
      appType="cursor"
      isSupported={isSupportedPlatform}
      unsupportedTitleKey="common.shared.instances.unsupported.title"
      unsupportedTitleDefault="暂不支持当前系统"
      unsupportedDescKey="cursor.instances.unsupported.descPlatform"
      unsupportedDescDefault="Cursor 多开实例仅支持 macOS、Windows 和 Linux。"
    />
  );
}
