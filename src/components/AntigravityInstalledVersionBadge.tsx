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

export function AntigravityInstalledVersionBadge() {
  const { t } = useTranslation();
  const runtimeTarget = useAntigravityRuntimeTarget();
  const [info, setInfo] = useState<AntigravityInstalledVersionInfo | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [installSupported, setInstallSupported] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [phase, setPhase] = useState('');
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
    if (installing) {
      return;
    }
    setInstalling(true);
    setPhase('resolving');
    setProgress(0);
    try {
      const result = await installMissingPlatform('antigravity', (payload) => {
        setPhase(payload.phase);
        setProgress(payload.progress ?? 0);
      });
      const installedPath = (result.installedPath || '').trim();
      if (installedPath) {
        await invoke('set_app_path', { app: 'antigravity', path: installedPath });
        setRefreshTrigger((prev) => prev + 1);
        return;
      }
    } catch (error) {
      console.warn('[AntigravityInstalledVersionBadge] install failed:', error);
    } finally {
      setInstalling(false);
      setPhase('');
      setProgress(0);
    }
  }, [installing]);

  const title = useMemo(() => {
    if (!loaded) {
      return t('runtime.installedVersion.loading', '正在检测安装版本');
    }
    if (installing) {
      return phase === 'downloading'
        ? `${t('appPath.install.downloading', '下载中')} ${progress}%`
        : t('appPath.install.inProgress', '安装中…');
    }
    if (!info?.version) {
      return installSupported
        ? `Antigravity\n${t('appPath.install.downloadAndInstall', '下载并静默安装')}`
        : t('runtime.installedVersion.missing', '未检测到已安装版本');
    }
    return `${info.product_name || 'Antigravity'} v${info.version}\n${info.app_path || ''}\n${t('appPath.install.clickToReinstall', '点击执行静默安装')}`;
  }, [info, loaded, t, installSupported, installing, phase, progress]);

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

  const isClickable = !installing;
  const badgeClass = [
    'installed-version-badge',
    !info?.version && 'is-missing',
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
      <span className="installed-version-name">{info?.product_name || 'Antigravity'}</span>
      <span className="installed-version-value">
        {installing
          ? phase === 'downloading'
            ? `${t('appPath.install.downloading', '下载中')} ${progress}%`
            : t('appPath.install.inProgress', '安装中…')
          : info?.version
            ? `v${info.version}`
            : t('runtime.installedVersion.notFound', '未检测到版本')}
      </span>
    </div>
  );
}
