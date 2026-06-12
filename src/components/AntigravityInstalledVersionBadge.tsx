import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import {
  AntigravityInstalledVersionInfo,
  getAntigravityInstalledVersionInfo,
} from '../services/antigravityRuntimeService';
import { useAntigravityRuntimeTarget } from '../hooks/useAntigravityRuntimeTarget';
import {
  installMissingPlatform,
  isPlatformInstallSupported,
} from '../utils/platformInstall';

const INSTALLED_VERSION_DEFER_MS = 250;
const INSTALL_RESULT_DISPLAY_MS = 3000;

type InstallUiState = 'idle' | 'downloading' | 'installing' | 'success' | 'failure';

export function AntigravityInstalledVersionBadge() {
  const { t } = useTranslation();
  const runtimeTarget = useAntigravityRuntimeTarget();
  const [info, setInfo] = useState<AntigravityInstalledVersionInfo | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [installSupported, setInstallSupported] = useState(false);
  const [installState, setInstallState] = useState<InstallUiState>('idle');
  const [progress, setProgress] = useState(0);
  const [refreshTrigger, setRefreshTrigger] = useState(0);

  useEffect(() => {
    let cancelled = false;
    let timer = 0;
    setLoaded(false);

    const loadVersion = async () => {
      try {
        const supported = await isPlatformInstallSupported('antigravity');
        if (!cancelled) {
          setInstallSupported(supported);
        }
      } catch (e) {
        console.warn('[AntigravityInstalledVersionBadge] supported check failed:', e);
      }

      try {
        const quickInfo = await getAntigravityInstalledVersionInfo(runtimeTarget, 'quick');
        if (!cancelled) {
          setInfo(quickInfo);
          setLoaded(!!quickInfo);
        }
      } catch (error) {
        console.warn('[AntigravityInstalledVersionBadge] failed to load installed version:', error);
        if (!cancelled) {
          setInfo(null);
        }
      }

      try {
        const fullInfo = await getAntigravityInstalledVersionInfo(runtimeTarget, 'full');
        if (!cancelled) {
          if (fullInfo) {
            setInfo(fullInfo);
          }
          setLoaded(true);
        }
      } catch (error) {
        console.warn('[AntigravityInstalledVersionBadge] failed to complete installed version scan:', error);
        if (!cancelled) {
          setLoaded(true);
        }
      }
    };

    timer = window.setTimeout(() => {
      void loadVersion();
    }, INSTALLED_VERSION_DEFER_MS);

    return () => {
      cancelled = true;
      if (timer) {
        window.clearTimeout(timer);
      }
    };
  }, [runtimeTarget, refreshTrigger]);

  const handleSilentInstall = useCallback(async () => {
    if (installState !== 'idle') {
      return;
    }
    setInstallState('downloading');
    setProgress(0);
    try {
      const result = await installMissingPlatform('antigravity', (payload) => {
        if (payload.phase === 'installing' || payload.phase === 'resolving') {
          setInstallState('installing');
          setProgress(100);
          return;
        }
        if (payload.phase === 'downloading') {
          setInstallState('downloading');
          setProgress(payload.progress ?? 0);
        }
      });
      const installedPath = (result.installedPath || '').trim();
      if (installedPath) {
        await invoke('set_app_path', { app: 'antigravity', path: installedPath });
        setInstallState('success');
        setProgress(100);
        setRefreshTrigger((prev) => prev + 1);
        window.setTimeout(() => {
          setInstallState('idle');
          setProgress(0);
        }, INSTALL_RESULT_DISPLAY_MS);
        return;
      }
      setInstallState('failure');
      window.setTimeout(() => {
        setInstallState('idle');
        setProgress(0);
      }, INSTALL_RESULT_DISPLAY_MS);
    } catch (error) {
      console.warn('[AntigravityInstalledVersionBadge] install failed:', error);
      setInstallState('failure');
      window.setTimeout(() => {
        setInstallState('idle');
        setProgress(0);
      }, INSTALL_RESULT_DISPLAY_MS);
    }
  }, [installState]);

  const title = useMemo(() => {
    if (!loaded) {
      return t('runtime.installedVersion.loading', '正在检测安装版本');
    }
    if (installState === 'downloading') {
      return `${t('appPath.install.downloading', '下载中')} ${progress}%`;
    }
    if (installState === 'installing') {
      return t('appPath.install.inProgress', '安装中…');
    }
    if (installState === 'success') {
      return t('appPath.install.success', '安装成功');
    }
    if (installState === 'failure') {
      return t('appPath.install.failed', '安装失败');
    }
    if (!info?.version) {
      return installSupported
        ? `Antigravity\n${t('appPath.install.downloadAndInstall', '下载并静默安装')}`
        : t('runtime.installedVersion.missing', '未检测到已安装版本');
    }
    return `${info.product_name || 'Antigravity'} v${info.version}\n${info.app_path || ''}\n${t('appPath.install.clickToReinstall', '点击执行静默安装')}`;
  }, [info, installState, installSupported, loaded, progress, t]);

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

  const isClickable = installState === 'idle';
  const badgeClass = [
    'installed-version-badge',
    !info?.version && installState === 'idle' && 'is-missing',
    isClickable && 'is-clickable',
    installState === 'downloading' && 'is-installing',
    installState === 'installing' && 'is-installing',
    installState === 'success' && 'is-install-success',
    installState === 'failure' && 'is-install-failure',
  ]
    .filter(Boolean)
    .join(' ');

  const statusText = (() => {
    if (installState === 'downloading') {
      return `${t('appPath.install.downloading', '下载中')} ${progress}%`;
    }
    if (installState === 'installing') {
      return t('appPath.install.inProgress', '安装中…');
    }
    if (installState === 'success') {
      return t('appPath.install.success', '安装成功');
    }
    if (installState === 'failure') {
      return t('appPath.install.failed', '安装失败');
    }
    if (info?.version) {
      return `v${info.version}`;
    }
    return t('runtime.installedVersion.notFound', '未检测到版本');
  })();

  return (
    <div
      className={badgeClass}
      title={title}
      onClick={isClickable ? () => { void handleSilentInstall(); } : undefined}
    >
      {(installState === 'downloading' || installState === 'installing' || installState === 'success') && (
        <div className="installed-version-progress-bg" style={{ width: `${progress}%` }} />
      )}
      <span className="installed-version-dot" />
      <span className="installed-version-name">{info?.product_name || 'Antigravity'}</span>
      <span className="installed-version-value">{statusText}</span>
    </div>
  );
}
