import { Settings, Rocket, GaugeCircle, LayoutGrid, SlidersHorizontal, Zap } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useState, useRef, useCallback, useEffect, useMemo } from 'react';
import { Page } from '../../types/navigation';
import { PlatformId, PLATFORM_PAGE_MAP } from '../../types/platform';
import {
  resolveGroupChildIcon,
  resolveGroupChildName,
  parseGroupEntryId,
  parsePlatformEntryId,
  PlatformLayoutEntryId,
  PlatformLayoutGroup,
  resolveEntryDefaultPlatformId,
  resolveEntryIdForPlatform,
  usePlatformLayoutStore,
} from '../../stores/usePlatformLayoutStore';
import { getPlatformLabel, renderPlatformIcon } from '../../utils/platformMeta';

interface SideNavProps {
  page: Page;
  setPage: (page: Page) => void;
  onOpenPlatformLayout: () => void;
  easterEggClickCount: number;
  onEasterEggTriggerClick: () => void;
  hasBreakoutSession: boolean;
  updateActionState: 'hidden' | 'available' | 'downloading' | 'installing' | 'ready';
  updateProgress: number;
  onUpdateActionClick: () => void;
}

interface FlyingRocket {
  id: number;
  x: number;
}

interface SideNavEntry {
  id: PlatformLayoutEntryId;
  label: string;
  hidden: boolean;
  targetPlatformId: PlatformId;
  platformIds: PlatformId[];
  group: PlatformLayoutGroup | null;
}

const PAGE_PLATFORM_MAP: Partial<Record<Page, PlatformId>> = {
  overview: 'antigravity',
  codex: 'codex',
  'github-copilot': 'github-copilot',
  windsurf: 'windsurf',
  kiro: 'kiro',
  cursor: 'cursor',
  gemini: 'gemini',
  codebuddy: 'codebuddy',
  'codebuddy-cn': 'codebuddy_cn',
  qoder: 'qoder',
  trae: 'trae',
  workbuddy: 'workbuddy',
};

function renderEntryIcon(entry: SideNavEntry, size: number) {
  if (entry.group && entry.group.iconKind === 'custom' && entry.group.iconCustomDataUrl) {
    return (
      <img
        src={entry.group.iconCustomDataUrl}
        alt={entry.label}
        className="side-nav-group-icon"
        style={{ width: size, height: size }}
      />
    );
  }

  if (entry.group) {
    const iconPlatform = entry.group.iconPlatformId ?? entry.targetPlatformId;
    return renderPlatformIcon(iconPlatform, size);
  }

  return renderPlatformIcon(entry.targetPlatformId, size);
}

export function SideNav({
  page,
  setPage,
  onOpenPlatformLayout,
  easterEggClickCount,
  onEasterEggTriggerClick,
  hasBreakoutSession,
  updateActionState,
  updateProgress,
  onUpdateActionClick,
}: SideNavProps) {
  const { t } = useTranslation();
  const [flyingRockets, setFlyingRockets] = useState<FlyingRocket[]>([]);
  const [showMore, setShowMore] = useState(false);
  const rocketIdRef = useRef(0);
  const logoRef = useRef<HTMLDivElement>(null);
  const morePopoverRef = useRef<HTMLDivElement>(null);
  const moreButtonRef = useRef<HTMLButtonElement>(null);

  const {
    orderedEntryIds,
    hiddenEntryIds,
    sidebarEntryIds,
    platformGroups,
  } = usePlatformLayoutStore();

  const currentPlatformId = PAGE_PLATFORM_MAP[page] ?? null;
  const currentEntryId = useMemo(
    () => (currentPlatformId ? resolveEntryIdForPlatform(currentPlatformId, platformGroups) : null),
    [currentPlatformId, platformGroups],
  );

  const hiddenSet = useMemo(() => new Set(hiddenEntryIds), [hiddenEntryIds]);
  const sidebarSet = useMemo(() => new Set(sidebarEntryIds), [sidebarEntryIds]);

  const orderedEntries = useMemo<SideNavEntry[]>(() => {
    return orderedEntryIds
      .map((entryId) => {
        const platformId = parsePlatformEntryId(entryId);
        if (platformId) {
          return {
            id: entryId,
            label: getPlatformLabel(platformId, t),
            hidden: hiddenSet.has(entryId),
            targetPlatformId: platformId,
            platformIds: [platformId],
            group: null,
          };
        }

        const groupId = parseGroupEntryId(entryId);
        if (!groupId) {
          return null;
        }
        const group = platformGroups.find((item) => item.id === groupId);
        if (!group) {
          return null;
        }

        const targetPlatformId = resolveEntryDefaultPlatformId(entryId, platformGroups);
        if (!targetPlatformId) {
          return null;
        }

        return {
          id: entryId,
          label: group.name,
          hidden: hiddenSet.has(entryId),
          targetPlatformId,
          platformIds: [...group.platformIds],
          group,
        };
      })
      .filter((entry): entry is SideNavEntry => !!entry);
  }, [orderedEntryIds, platformGroups, hiddenSet, t]);

  const sidebarVisibleEntries = useMemo(
    () => orderedEntries.filter((entry) => sidebarSet.has(entry.id) && !entry.hidden),
    [orderedEntries, sidebarSet],
  );

  const isMoreActive = !!currentEntryId && !sidebarVisibleEntries.some((entry) => entry.id === currentEntryId);

  const handleLogoClick = useCallback(() => {
    if (hasBreakoutSession) {
      onEasterEggTriggerClick();
      return;
    }

    const newRocket: FlyingRocket = {
      id: rocketIdRef.current++,
      x: (Math.random() - 0.5) * 40,
    };

    setFlyingRockets((prev) => [...prev, newRocket]);

    setTimeout(() => {
      setFlyingRockets((prev) => prev.filter((rocket) => rocket.id !== newRocket.id));
    }, 1500);

    onEasterEggTriggerClick();
  }, [hasBreakoutSession, onEasterEggTriggerClick]);

  useEffect(() => {
    if (!showMore) return;
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (morePopoverRef.current?.contains(target)) return;
      if (moreButtonRef.current?.contains(target)) return;
      setShowMore(false);
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [showMore]);

  const clampedUpdateProgress = Math.max(0, Math.min(100, Math.round(updateProgress)));
  const updateVisualState = updateActionState === 'ready'
    ? 'restart'
    : updateActionState === 'downloading' || updateActionState === 'installing'
      ? 'progress'
      : 'update';

  return (
    <nav className="side-nav">
      {updateActionState !== 'hidden' && (
        <div className="side-nav-update-entry">
          <button
            type="button"
            className={`side-nav-update-btn is-${updateVisualState}`}
            onClick={onUpdateActionClick}
            title={
              updateActionState === 'downloading'
                ? t('update_notification.downloading', '下载中...')
                : updateActionState === 'installing'
                  ? t('nav.quickUpdate.installing', '安装中')
                  : updateActionState === 'ready'
                    ? t('nav.quickUpdate.restart', '重启')
                    : t('nav.quickUpdate.update', '更新')
            }
            disabled={updateActionState === 'installing'}
          >
            {updateActionState === 'downloading' ? (
              <span className="side-nav-update-progress-lr">
                <span
                  className={`side-nav-update-progress-fill${clampedUpdateProgress >= 100 ? ' is-full' : ''}`}
                  style={{ width: `${clampedUpdateProgress}%` }}
                >
                  <span className="side-nav-update-progress-ripple side-nav-update-progress-ripple-a" />
                  <span className="side-nav-update-progress-ripple side-nav-update-progress-ripple-b" />
                </span>
                <span className="side-nav-update-progress-percent">{clampedUpdateProgress}%</span>
              </span>
            ) : updateActionState === 'installing' ? (
              <span className="side-nav-update-text">{t('nav.quickUpdate.installing', '安装中')}</span>
            ) : (
              <span className="side-nav-update-text">
                {updateActionState === 'ready'
                  ? t('nav.quickUpdate.restart', '重启')
                  : t('nav.quickUpdate.update', '更新')}
              </span>
            )}
          </button>
        </div>
      )}

      <div className="nav-brand" style={{ position: 'relative', zIndex: 10 }}>
        <div
          ref={logoRef}
          className={`brand-logo rocket-easter-egg${hasBreakoutSession ? ' rocket-easter-egg-active' : ''}`}
          onClick={handleLogoClick}
          title={hasBreakoutSession ? t('breakout.resumeGameNav', '继续游戏') : undefined}
        >
          <Rocket size={20} />
          {hasBreakoutSession && <span className="rocket-session-indicator" aria-hidden="true" />}
          {!hasBreakoutSession && easterEggClickCount > 0 && (
            <span className="rocket-click-count">{easterEggClickCount}</span>
          )}
        </div>

        <div
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            width: '100%',
            height: '100%',
            pointerEvents: 'none',
          }}
        >
          {flyingRockets.map((rocket) => (
            <span
              key={rocket.id}
              className="flying-rocket"
              style={{ '--rocket-x': `${rocket.x}px` } as React.CSSProperties}
            >
              🚀
            </span>
          ))}
        </div>
      </div>

      <div className="nav-items">
        <button
          className={`nav-item ${page === 'dashboard' ? 'active' : ''}`}
          onClick={() => setPage('dashboard')}
          title={t('nav.dashboard')}
        >
          <GaugeCircle size={20} />
          <span className="tooltip">{t('nav.dashboard')}</span>
        </button>

        <button
          className={`nav-item ${page === 'token-monitor' ? 'active' : ''}`}
          onClick={() => setPage('token-monitor')}
          title="Token Monitor"
        >
          <Zap size={20} />
          <span className="tooltip">Token Monitor</span>
        </button>

        {sidebarVisibleEntries.map((entry) => {
          const active = currentEntryId === entry.id;
          return (
            <button
              key={entry.id}
              className={`nav-item ${active ? 'active' : ''}`}
              onClick={() => setPage(PLATFORM_PAGE_MAP[entry.targetPlatformId])}
              title={entry.label}
            >
              {renderEntryIcon(entry, 20)}
              <span className="tooltip">{entry.label}</span>
            </button>
          );
        })}

        <button
          ref={moreButtonRef}
          className={`nav-item ${showMore || isMoreActive ? 'active' : ''}`}
          onClick={() => setShowMore((prev) => !prev)}
          title={t('nav.morePlatforms', '更多平台')}
        >
          <LayoutGrid size={20} />
          <span className="tooltip">{t('nav.morePlatforms', '更多平台')}</span>
        </button>

        {showMore && (
          <div className="side-nav-more-popover" ref={morePopoverRef}>
            <div className="side-nav-more-title">{t('nav.morePlatforms', '更多平台')}</div>
            <div className="side-nav-more-list">
              {orderedEntries.map((entry) => {
                const active = currentEntryId === entry.id;
                return (
                  <div className="side-nav-more-group" key={entry.id}>
                    <button
                      className={`side-nav-more-item ${active ? 'active' : ''}`}
                      onClick={() => {
                        setPage(PLATFORM_PAGE_MAP[entry.targetPlatformId]);
                        setShowMore(false);
                      }}
                    >
                      <span className="side-nav-more-item-icon">{renderEntryIcon(entry, 16)}</span>
                      <span className="side-nav-more-item-label">{entry.label}</span>
                      {entry.hidden && (
                        <span className="side-nav-more-item-badge">
                          {t('platformLayout.hiddenBadge', '已隐藏')}
                        </span>
                      )}
                    </button>

                    {entry.group && entry.platformIds.length > 1 && (
                      <div className="side-nav-more-sub-list">
                        {entry.platformIds.map((platformId) => {
                          const icon = resolveGroupChildIcon(entry.group!, platformId);
                          const label = resolveGroupChildName(
                            entry.group!,
                            platformId,
                            getPlatformLabel(platformId, t),
                          );
                          return (
                            <button
                              key={`${entry.id}:${platformId}`}
                              className={`side-nav-more-sub-item ${currentPlatformId === platformId ? 'active' : ''}`}
                              onClick={() => {
                                setPage(PLATFORM_PAGE_MAP[platformId]);
                                setShowMore(false);
                              }}
                            >
                              <span className="side-nav-more-sub-item-icon">
                                {icon.iconKind === 'custom' && icon.iconCustomDataUrl ? (
                                  <img
                                    src={icon.iconCustomDataUrl}
                                    alt={label}
                                    className="side-nav-group-icon"
                                    style={{ width: 14, height: 14 }}
                                  />
                                ) : (
                                  renderPlatformIcon(icon.iconPlatformId, 14)
                                )}
                              </span>
                              <span className="side-nav-more-sub-item-label">{label}</span>
                            </button>
                          );
                        })}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
            <button
              className="side-nav-more-manage"
              onClick={() => {
                setShowMore(false);
                onOpenPlatformLayout();
              }}
            >
              <SlidersHorizontal size={14} />
              <span>{t('platformLayout.openFromMore', '管理平台布局')}</span>
            </button>
          </div>
        )}
      </div>

      <div className="nav-footer">
        <button
          className={`nav-item ${page === 'settings' ? 'active' : ''}`}
          onClick={() => setPage('settings')}
          title={t('nav.settings')}
        >
          <Settings size={20} />
          <span className="tooltip">{t('nav.settings')}</span>
        </button>
      </div>
    </nav>
  );
}
