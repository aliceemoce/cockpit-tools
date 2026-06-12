import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type InstallableAppPath =
  | 'antigravity'
  | 'codex'
  | 'vscode'
  | 'windsurf'
  | 'kiro'
  | 'cursor'
  | 'gemini'
  | 'codebuddy'
  | 'codebuddy_cn'
  | 'qoder'
  | 'trae'
  | 'workbuddy'
  | 'zed';

export const INSTALLABLE_APP_PATHS: InstallableAppPath[] = [
  'cursor',
  'windsurf',
  'kiro',
  'vscode',
  'codex',
  'antigravity',
  'gemini',
  'codebuddy',
  'codebuddy_cn',
  'qoder',
  'trae',
  'workbuddy',
  'zed',
];

export type PlatformInstallProgress = {
  platformId: string;
  phase: string;
  progress?: number;
  message?: string;
};

export type PlatformInstallResult = {
  platformId: string;
  installedPath?: string | null;
  usedManualFallback: boolean;
  message: string;
};

export function isInstallableAppPath(app: string): app is InstallableAppPath {
  return (INSTALLABLE_APP_PATHS as string[]).includes(app);
}

export async function isPlatformInstallSupported(app: string): Promise<boolean> {
  if (!isInstallableAppPath(app)) return false;
  try {
    return await invoke<boolean>('is_platform_install_supported', { platform_id: app });
  } catch (error) {
    console.warn('[platformInstall] is_platform_install_supported failed:', app, error);
    // Assume supported so Settings keeps the install action visible; install will surface errors.
    return true;
  }
}

export async function installMissingPlatform(
  app: InstallableAppPath,
  onProgress?: (payload: PlatformInstallProgress) => void,
): Promise<PlatformInstallResult> {
  let unlisten: UnlistenFn | undefined;
  if (onProgress) {
    unlisten = await listen<{
      platform_id: string;
      phase: string;
      progress?: number;
      message?: string;
    }>('platform-install://progress', (event) => {
      onProgress({
        platformId: event.payload.platform_id,
        phase: event.payload.phase,
        progress: event.payload.progress,
        message: event.payload.message,
      });
    });
  }

  try {
    const result = await invoke<{
      platform_id: string;
      installed_path?: string | null;
      used_manual_fallback: boolean;
      message: string;
    }>('install_missing_platform', { platform_id: app });

    return {
      platformId: result.platform_id,
      installedPath: result.installed_path,
      usedManualFallback: result.used_manual_fallback,
      message: result.message,
    };
  } finally {
    if (unlisten) {
      await unlisten();
    }
  }
}
