import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Download, RefreshCw } from 'lucide-react';
import type { PlatformOverviewHeaderId } from './PlatformOverviewTabsHeader';
import { getPlatformLabel } from '../../utils/platformMeta';
import {
  installMissingPlatform,
  isInstallableAppPath,
  isPlatformInstallSupported,
  type InstallableAppPath,
} from '../../utils/platformInstall';

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

function resolveInstallableApp(
  platform: PlatformOverviewHeaderId,
): InstallableAppPath | null {
  const detectAppId = resolveDetectAppId(platform);
  if (!detectAppId || !isInstallableAppPath(detectAppId)) {
    return null;
  }
  return detectAppId;
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
  const installableApp = useMemo(() => resolveInstallableApp(platform), [platform]);
  const productLabel = useMemo(
    () => getPlatformLabel(platform, t),
    [platform, t],
  );
  const [loaded, setLoaded] = useState(false);
  const [appPath, setAppPath] = useState<string | null>(null);
  const [installSupported, setInstallSupported] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installProgress, setInstallProgress] = useState('');

  const reloadPath = useCallback(async (force = false) => {
    if (!detectAppId) {
      return;
    }
    try {
      const detected = await invoke<string | null>('detect_app_path', {
        app: detectAppId,
        force,
      });
      setAppPath(detected?.trim() || null);
    } catch (error) {
      console.warn(
        `[PlatformInstalledVersionBadge] failed to detect ${detectAppId}:`,
        error,
      );
      setAppPath(null);
    } finally {
      setLoaded(true);
    }
  }, [detectAppId]);

  useEffect(() => {
    if (!detectAppId) {
      return;
    }

    let cancelled = false;
    let timer = 0;

    const load = async () => {
      if (installableApp) {
        const supported = await isPlatformInstallSupported(installableApp);
        if (!cancelled) {
          setInstallSupported(supported);
        }
      }
      await reloadPath(false);
    };

    timer = window.setTimeout(() => {
      void load();
    }, DETECT_DEFER_MS);

    return () => {
      cancelled = true;
      if (timer) {
        window.clearTimeout(timer);
      }
    };
  }, [detectAppId, installableApp, reloadPath]);

  const handleSilentInstall = useCallback(async () => {
    if (!installableApp || installing) {
      return;
    }
    setInstalling(true);
    setInstallProgress(
      t('appPath.install.inProgress', '正在下载并安装…'),
    );
    try {
      const result = await installMissingPlatform(installableApp, (payload) => {
        if (payload.message) {
          setInstallProgress(payload.message);
        }
      });
      setInstallProgress(result.message);
      await reloadPath(true);
    } catch (error) {
      console.warn('[PlatformInstalledVersionBadge] install failed:', error);
      setInstallProgress(
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setInstalling(false);
    }
  }, [installableApp, installing, reloadPath, t]);

  const title = useMemo(() => {
    if (!loaded) {
      return t('runtime.installedVersion.loading', '正在检测安装版本');
    }
    if (!appPath) {
      if (installableApp && installSupported) {
        return t('appPath.install.downloadAndInstall', '下载并静默安装');
      }
      return t('runtime.installedVersion.missing', '未检测到已安装版本');
    }
    return `${productLabel}\n${appPath}`;
  }, [appPath, installSupported, installableApp, loaded, productLabel, t]);

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
    const canInstall = Boolean(installableApp && installSupported);
    return (
      <div
        className={`installed-version-badge is-missing${canInstall ? ' has-install' : ''}`}
        title={title}
      >
        <span className="installed-version-dot" />
        <span className="installed-version-value">
          {t('runtime.installedVersion.notFound', '未检测到版本')}
        </span>
        {canInstall ? (
          <button
            type="button"
            className="installed-version-install-btn"
            disabled={installing}
            title={t('appPath.install.downloadAndInstall', '下载并静默安装')}
            onClick={() => {
              void handleSilentInstall();
            }}
          >
            {installing ? (
              <RefreshCw size={12} className="spin" />
            ) : (
              <Download size={12} />
            )}
          </button>
        ) : null}
        {installProgress ? (
          <span className="installed-version-install-hint">{installProgress}</span>
        ) : null}
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
