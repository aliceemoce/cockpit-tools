import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import type { PlatformOverviewHeaderId } from './PlatformOverviewTabsHeader';
import { getPlatformLabel } from '../../utils/platformMeta';

const DETECT_DEFER_MS = 250;

function resolveDetectAppId(platform: PlatformOverviewHeaderId): string | null {
  switch (platform) {
    case 'codex':
      return 'codex';
    case 'zed':
      return 'zed';
    case 'github-copilot':
      return 'vscode';
    case 'windsurf':
      return 'windsurf';
    case 'kiro':
      return 'kiro';
    case 'cursor':
      return 'cursor';
    case 'codebuddy':
      return 'codebuddy';
    case 'codebuddy_cn':
      return 'codebuddy_cn';
    case 'qoder':
      return 'qoder';
    case 'trae':
      return 'trae';
    case 'workbuddy':
      return 'workbuddy';
    default:
      return null;
  }
}

function basenameFromPath(path: string): string {
  const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '');
  const idx = normalized.lastIndexOf('/');
  return idx >= 0 ? normalized.slice(idx + 1) : normalized;
}

export function PlatformInstalledVersionBadge({
  platform,
}: {
  platform: PlatformOverviewHeaderId;
}) {
  const { t } = useTranslation();
  const detectAppId = useMemo(() => resolveDetectAppId(platform), [platform]);
  const productLabel = useMemo(
    () => getPlatformLabel(platform, t),
    [platform, t],
  );
  const [loaded, setLoaded] = useState(false);
  const [appPath, setAppPath] = useState<string | null>(null);

  useEffect(() => {
    if (!detectAppId) {
      return;
    }

    let cancelled = false;
    let timer = 0;

    const loadPath = async () => {
      try {
        const detected = await invoke<string | null>('detect_app_path', {
          app: detectAppId,
          force: false,
        });
        if (!cancelled) {
          setAppPath(detected?.trim() || null);
          setLoaded(true);
        }
      } catch (error) {
        console.warn(
          `[PlatformInstalledVersionBadge] failed to detect ${detectAppId}:`,
          error,
        );
        if (!cancelled) {
          setAppPath(null);
          setLoaded(true);
        }
      }
    };

    timer = window.setTimeout(() => {
      void loadPath();
    }, DETECT_DEFER_MS);

    return () => {
      cancelled = true;
      if (timer) {
        window.clearTimeout(timer);
      }
    };
  }, [detectAppId]);

  const title = useMemo(() => {
    if (!loaded) {
      return t('runtime.installedVersion.loading', '正在检测安装版本');
    }
    if (!appPath) {
      return t('runtime.installedVersion.missing', '未检测到已安装版本');
    }
    return `${productLabel}\n${appPath}`;
  }, [appPath, loaded, productLabel, t]);

  if (!detectAppId) {
    return null;
  }

  if (!loaded) {
    return (
      <div className="installed-version-badge is-loading" title={title}>
        <span className="installed-version-dot" />
        <span className="installed-version-value">
          {t('runtime.installedVersion.detecting', '检测中')}
        </span>
      </div>
    );
  }

  if (!appPath) {
    return (
      <div className="installed-version-badge is-missing" title={title}>
        <span className="installed-version-dot" />
        <span className="installed-version-value">
          {t('runtime.installedVersion.notFound', '未检测到版本')}
        </span>
      </div>
    );
  }

  return (
    <div className="installed-version-badge" title={title}>
      <span className="installed-version-dot" />
      <span className="installed-version-name">{productLabel}</span>
      <span className="installed-version-value">{basenameFromPath(appPath)}</span>
    </div>
  );
}
