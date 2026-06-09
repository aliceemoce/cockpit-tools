import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
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

function resolveInstallAppId(
  platform: PlatformOverviewHeaderId,
): InstallableAppPath | null {
  const detectId = resolveDetectAppId(platform);
  if (!detectId || !isInstallableAppPath(detectId)) {
    return null;
  }
  return detectId;
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
  const installAppId = useMemo(() => resolveInstallAppId(platform), [platform]);
  const productLabel = useMemo(
    () => getPlatformLabel(platform, t),
    [platform, t],
  );
  const [loaded, setLoaded] = useState(false);
  const [appPath, setAppPath] = useState<string | null>(null);
  const [installSupported, setInstallSupported] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [phase, setPhase] = useState('');
  const [progress, setProgress] = useState(0);

  const reloadPath = useCallback(async () => {
    if (!detectAppId) {
      return;
    }
    try {
      const detected = await invoke<string | null>('detect_app_path', {
        app: detectAppId,
        force: false,
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
      if (installAppId) {
        const supported = await isPlatformInstallSupported(installAppId);
        if (!cancelled) {
          setInstallSupported(supported);
        }
      }
      if (!cancelled) {
        setLoaded(false);
        await reloadPath();
      }
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
  }, [detectAppId, installAppId, reloadPath]);

  const handleSilentInstall = useCallback(async () => {
    if (!installAppId || installing) {
      return;
    }
    setInstalling(true);
    setPhase('resolving');
    setProgress(0);
    try {
      const result = await installMissingPlatform(installAppId, (payload) => {
        setPhase(payload.phase);
        setProgress(payload.progress ?? 0);
      });
      const installedPath = (result.installedPath || '').trim();
      if (installedPath) {
        await invoke('set_app_path', { app: installAppId, path: installedPath });
        setAppPath(installedPath);
        return;
      }
    } catch (error) {
      console.warn('[PlatformInstalledVersionBadge] install failed:', error);
    } finally {
      setInstalling(false);
      setPhase('');
      setProgress(0);
    }
  }, [installAppId, installing]);

  const title = useMemo(() => {
    if (!loaded) {
      return t('runtime.installedVersion.loading', '正在检测安装版本');
    }
    if (installing) {
      return phase === 'downloading'
        ? `${t('appPath.install.downloading', '下载中')} ${progress}%`
        : t('appPath.install.inProgress', '安装中…');
    }
    if (!appPath) {
      return installSupported
        ? `${productLabel}\n${t('appPath.install.downloadAndInstall', '下载并静默安装')}`
        : t('runtime.installedVersion.missing', '未检测到已安装版本');
    }
    return `${productLabel}\n${appPath}\n${t('appPath.install.clickToReinstall', '点击执行静默安装')}`;
  }, [appPath, installSupported, loaded, productLabel, t, installing, phase, progress]);

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

  const isClickable = !installing && !!installAppId;
  const badgeClass = [
    'installed-version-badge',
    loaded && !appPath && 'is-missing',
    isClickable && 'is-clickable',
    installing && 'is-installing',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div
      className={badgeClass}
      title={title}
      onClick={isClickable ? () => { void handleSilentInstall(); } : undefined}
    >
      {installing && phase === 'downloading' && (
        <div className="installed-version-progress-bg" style={{ width: `${progress}%` }} />
      )}
      <span className="installed-version-dot" />
      <span className="installed-version-name">{productLabel}</span>
      <span className="installed-version-value">
        {installing
          ? phase === 'downloading'
            ? `${t('appPath.install.downloading', '下载中')} ${progress}%`
            : t('appPath.install.inProgress', '安装中…')
          : appPath
            ? basenameFromPath(appPath)
            : t('runtime.installedVersion.notFound', '未检测到版本')}
      </span>
    </div>
  );
}
