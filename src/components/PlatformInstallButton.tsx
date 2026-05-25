import { useEffect, useState } from 'react';
import { Download } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  installMissingPlatform,
  isInstallableAppPath,
  isPlatformInstallSupported,
  type InstallableAppPath,
} from '../utils/platformInstall';

type PlatformInstallButtonProps = {
  app: string;
  disabled?: boolean;
  onInstalled?: (path: string) => void | Promise<void>;
  onError?: (message: string) => void;
  className?: string;
};

export function PlatformInstallButton({
  app,
  disabled = false,
  onInstalled,
  onError,
  className = 'btn btn-secondary',
}: PlatformInstallButtonProps) {
  const { t } = useTranslation();
  const [installing, setInstalling] = useState(false);
  const [progressLabel, setProgressLabel] = useState('');

  useEffect(() => {
    let active = true;
    if (!isInstallableAppPath(app)) {
      return () => {
        active = false;
      };
    }
    // Probe backend support for logging/diagnostics only — visibility does not depend on it.
    void isPlatformInstallSupported(app).then((value) => {
      if (!active || value) return;
      console.warn(`[PlatformInstallButton] platform install probe returned false for "${app}"`);
    });
    return () => {
      active = false;
    };
  }, [app]);

  if (!isInstallableAppPath(app)) {
    return null;
  }

  const handleInstall = async () => {
    if (installing || disabled) return;
    setInstalling(true);
    setProgressLabel('');
    try {
      const result = await installMissingPlatform(app as InstallableAppPath, (payload) => {
        if (payload.message) {
          setProgressLabel(payload.message);
        } else if (payload.progress != null) {
          setProgressLabel(`${payload.progress}%`);
        } else {
          setProgressLabel(payload.phase);
        }
      });
      const path = (result.installedPath || '').trim();
      if (path) {
        await onInstalled?.(path);
      } else if (result.usedManualFallback) {
        onError?.(result.message);
      } else {
        onError?.(
          t(
            app === 'codex'
              ? 'appPath.install.codexNotDetected'
              : 'appPath.install.notDetected',
          ),
        );
      }
    } catch (error) {
      onError?.(String(error));
    } finally {
      setInstalling(false);
    }
  };

  return (
    <button
      type="button"
      className={className}
      onClick={() => void handleInstall()}
      disabled={disabled || installing}
      title={progressLabel || undefined}
    >
      <Download size={14} />
      {installing
        ? progressLabel ||
          (app === 'codex'
            ? t('appPath.install.codexInProgress')
            : t('appPath.install.inProgress'))
        : app === 'codex'
          ? t('appPath.install.codexDownloadAndInstall')
          : t('appPath.install.downloadAndInstall')}
    </button>
  );
}
