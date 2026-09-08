import React, { useEffect, useRef, useState } from 'react';
import { Sun, Moon } from 'lucide-react';
import { Sun as SunIcon, Moon as MoonIcon, Monitor as MonitorIcon } from 'lucide';
import { MorphIcon } from '../../components/MorphIcon/MorphIcon';
import { useApp } from '../../context/AppContext';
import { api } from '../../services/api';
import { AppearanceConfig, AppTheme } from '../../types';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { TimeField } from '../../components/TimeField/TimeField';
import { UnderlineTabs } from '../../components/UnderlineTabs/UnderlineTabs';
import { ToggleSwitch } from '../../components/ToggleSwitch/ToggleSwitch';
import styles from './AppearanceScreen.module.css';

const defaultConfig: AppearanceConfig = {
  windows_theme_target: 'System',
  schedule_enabled: false,
  schedule_light_time: '07:00',
  schedule_dark_time: '19:00',
  switch_wallpaper: false,
  wallpaper_light_path: '',
  wallpaper_dark_path: '',
};

export const AppearanceScreen: React.FC = () => {
  const { t, showToast } = useApp();
  const [config, setConfig] = useState<AppearanceConfig>(defaultConfig);
  const [lightThumb, setLightThumb] = useState<string | null>(null);
  const [darkThumb, setDarkThumb] = useState<string | null>(null);
  const [lightThumbErr, setLightThumbErr] = useState<string | null>(null);
  const [darkThumbErr, setDarkThumbErr] = useState<string | null>(null);

  const draftRef = useRef<AppearanceConfig>(defaultConfig);
  const confirmedRef = useRef<AppearanceConfig>(defaultConfig);
  const pendingPatchesRef = useRef<Array<{ id: number; patch: Partial<AppearanceConfig> }>>([]);
  const nextPatchIdRef = useRef(1);
  const isMountedRef = useRef(true);
  const lightReqIdRef = useRef(0);
  const darkReqIdRef = useRef(0);
  const lastLightPathRef = useRef<string | null>(null);
  const lastDarkPathRef = useRef<string | null>(null);
  const saveQueueRef = useRef<Promise<void>>(Promise.resolve());

  const loadThumbnails = (cfg: AppearanceConfig) => {
    const lightPath = cfg.wallpaper_light_path || '';
    if (lightPath !== lastLightPathRef.current) {
      lastLightPathRef.current = lightPath;
      const reqId = ++lightReqIdRef.current;
      if (lightPath) {
        api
          .getWallpaperThumbnail(lightPath, 200, 200)
          .then((b64) => {
            if (isMountedRef.current && lightReqIdRef.current === reqId) {
              setLightThumb(b64);
              setLightThumbErr(null);
            }
          })
          .catch(() => {
            if (isMountedRef.current && lightReqIdRef.current === reqId) {
              setLightThumb(null);
              setLightThumbErr(t.appearance.wallpaperNotFound);
            }
          });
      } else {
        if (isMountedRef.current) {
          setLightThumb(null);
          setLightThumbErr(null);
        }
      }
    }

    const darkPath = cfg.wallpaper_dark_path || '';
    if (darkPath !== lastDarkPathRef.current) {
      lastDarkPathRef.current = darkPath;
      const reqId = ++darkReqIdRef.current;
      if (darkPath) {
        api
          .getWallpaperThumbnail(darkPath, 200, 200)
          .then((b64) => {
            if (isMountedRef.current && darkReqIdRef.current === reqId) {
              setDarkThumb(b64);
              setDarkThumbErr(null);
            }
          })
          .catch(() => {
            if (isMountedRef.current && darkReqIdRef.current === reqId) {
              setDarkThumb(null);
              setDarkThumbErr(t.appearance.wallpaperNotFound);
            }
          });
      } else {
        if (isMountedRef.current) {
          setDarkThumb(null);
          setDarkThumbErr(null);
        }
      }
    }
  };

  useEffect(() => {
    isMountedRef.current = true;
    api
      .getAppearanceConfig()
      .then((cfg) => {
        if (!isMountedRef.current) return;
        confirmedRef.current = cfg;
        // Rebase any user edits performed before initial load resolved
        let rebased = { ...cfg };
        for (const item of pendingPatchesRef.current) {
          rebased = { ...rebased, ...item.patch };
        }
        draftRef.current = rebased;
        setConfig(rebased);
        loadThumbnails(rebased);
      })
      .catch(console.error);

    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const handleUpdate = (partial: Partial<AppearanceConfig>): Promise<AppearanceConfig> => {
    const patchId = nextPatchIdRef.current++;
    pendingPatchesRef.current.push({ id: patchId, patch: partial });

    // Optimistically update draft
    const nextDraft = { ...draftRef.current, ...partial };
    draftRef.current = nextDraft;
    setConfig(nextDraft);

    if (
      partial.wallpaper_light_path !== undefined ||
      partial.wallpaper_dark_path !== undefined
    ) {
      loadThumbnails(nextDraft);
    }

    // Schedule save in serialized queue
    const op = saveQueueRef.current.then(async () => {
      try {
        const toSave = { ...draftRef.current };
        const saved = await api.saveAppearanceConfig(toSave);
        confirmedRef.current = saved;

        // Remove this patch and any earlier patches
        pendingPatchesRef.current = pendingPatchesRef.current.filter((p) => p.id > patchId);

        // Rebase remaining pending patches on top of server confirmed config
        let rebased = { ...saved };
        for (const p of pendingPatchesRef.current) {
          rebased = { ...rebased, ...p.patch };
        }
        draftRef.current = rebased;
        if (isMountedRef.current) {
          setConfig(rebased);
        }
        return rebased;
      } catch (err: any) {
        pendingPatchesRef.current = pendingPatchesRef.current.filter((p) => p.id !== patchId);
        let rebased = { ...confirmedRef.current };
        for (const p of pendingPatchesRef.current) {
          rebased = { ...rebased, ...p.patch };
        }
        draftRef.current = rebased;
        if (isMountedRef.current) {
          setConfig(rebased);
          showToast(err?.toString() || t.appearance.saveError);
        }
        throw err;
      }
    });

    saveQueueRef.current = op.then(() => {}).catch(() => {});
    return op;
  };

  const handlePickWallpaper = async (theme: 'light' | 'dark') => {
    const selected = await api.pickWallpaperFile();
    if (selected) {
      if (theme === 'light') {
        handleUpdate({ wallpaper_light_path: selected });
      } else {
        handleUpdate({ wallpaper_dark_path: selected });
      }
    }
  };

  const getFileName = (pathStr: string) => {
    if (!pathStr) return t.appearance.noWallpaper;
    return pathStr.split(/[\\/]/).pop() || pathStr;
  };

  const themeTabs: { value: AppTheme; label: string }[] = [
    { value: 'System', label: t.appearance.system },
    { value: 'Light', label: t.appearance.light },
    { value: 'Dark', label: t.appearance.dark },
  ];

  const headerIcon = config.windows_theme_target === 'Dark'
    ? MoonIcon
    : config.windows_theme_target === 'Light'
    ? SunIcon
    : MonitorIcon;

  return (
    <div className={styles.container}>
      <div className={styles.settingsBody}>
        {/* Page Header */}
        <PageHeader title={t.nav.appearance} />

        {/* Section 1: Windows Theme (System / Light / Dark) */}
        <div className={styles.section}>
          <div className={styles.sectionHeader}>
            <div className={styles.headerTitle}>
              <MorphIcon
                icon={headerIcon}
                size={16}
                spring="snappy"
                color="var(--accent)"
              />
              <span>{t.appearance.title}</span>
            </div>
            <UnderlineTabs<AppTheme>
              value={config.windows_theme_target}
              options={themeTabs}
              onChange={(mode) => handleUpdate({ windows_theme_target: mode })}
              ariaLabel={t.appearance.title}
            />
          </div>
        </div>

        {/* Section 2: Theme Schedule (Separate section with toggle) */}
        <div className={styles.section}>
          <div className={styles.sectionHeaderBetween}>
            <span className={styles.sectionTitle}>{t.appearance.scheduleTitle}</span>
            <ToggleSwitch
              checked={config.schedule_enabled}
              onChange={(checked) => handleUpdate({ schedule_enabled: checked })}
              ariaLabel={t.appearance.scheduleTitle}
            />
          </div>

        <div className={`${styles.scheduleRows} ${!config.schedule_enabled ? styles.disabled : ''}`}>
          <div className={styles.scheduleRow}>
            <div className={styles.scheduleLabelArea}>
              <Sun size={15} className={styles.scheduleIcon} />
              <span>{t.appearance.lightAt}</span>
            </div>
            <TimeField
              value={config.schedule_light_time}
              onChange={(value) => handleUpdate({ schedule_light_time: value })}
              disabled={!config.schedule_enabled}
              aria-label={t.appearance.lightAt}
            />
          </div>

          <div className={styles.scheduleRow}>
            <div className={styles.scheduleLabelArea}>
              <Moon size={15} className={styles.scheduleIcon} />
              <span>{t.appearance.darkAt}</span>
            </div>
            <TimeField
              value={config.schedule_dark_time}
              onChange={(value) => handleUpdate({ schedule_dark_time: value })}
              disabled={!config.schedule_enabled}
              aria-label={t.appearance.darkAt}
            />
          </div>
        </div>
      </div>

      {/* Section 3: Wallpaper Switching */}
      <div className={styles.section}>
        <div className={styles.sectionHeaderBetween}>
          <span className={styles.sectionTitle}>{t.appearance.switchWallpaper}</span>
          <ToggleSwitch
            checked={config.switch_wallpaper}
            onChange={(checked) => handleUpdate({ switch_wallpaper: checked })}
            ariaLabel={t.appearance.switchWallpaper}
          />
        </div>

        <div className={`${styles.wallpaperRows} ${!config.switch_wallpaper ? styles.disabled : ''}`}>
          {/* Light Theme Wallpaper */}
          <div className={styles.wallpaperRow}>
            <span className={styles.wallpaperLabel}>{t.appearance.lightThemeRow}</span>
            <div className={styles.wallpaperPreviewBox}>
              {lightThumb ? (
                <img
                  src={lightThumb}
                  alt=""
                  className={styles.wallpaperPreviewImg}
                />
              ) : (
                <Sun size={15} color="var(--text-muted)" />
              )}
            </div>
            <div className={styles.wallpaperInfo}>
              <span
                className={styles.wallpaperName}
                title={config.wallpaper_light_path || t.appearance.noWallpaper}
              >
                {getFileName(config.wallpaper_light_path)}
              </span>
              {lightThumbErr && (
                <span className={styles.wallpaperErr}>{lightThumbErr}</span>
              )}
            </div>
            <div className={styles.wallpaperActions}>
              <button
                className={styles.pickBtn}
                onClick={() => handlePickWallpaper('light')}
                disabled={!config.switch_wallpaper}
              >
                {t.appearance.chooseWallpaper}
              </button>
            </div>
          </div>

          {/* Dark Theme Wallpaper */}
          <div className={styles.wallpaperRow}>
            <span className={styles.wallpaperLabel}>{t.appearance.darkThemeRow}</span>
            <div className={styles.wallpaperPreviewBox}>
              {darkThumb ? (
                <img
                  src={darkThumb}
                  alt=""
                  className={styles.wallpaperPreviewImg}
                />
              ) : (
                <Moon size={15} color="var(--text-muted)" />
              )}
            </div>
            <div className={styles.wallpaperInfo}>
              <span
                className={styles.wallpaperName}
                title={config.wallpaper_dark_path || t.appearance.noWallpaper}
              >
                {getFileName(config.wallpaper_dark_path)}
              </span>
              {darkThumbErr && (
                <span className={styles.wallpaperErr}>{darkThumbErr}</span>
              )}
            </div>
            <div className={styles.wallpaperActions}>
              <button
                className={styles.pickBtn}
                onClick={() => handlePickWallpaper('dark')}
                disabled={!config.switch_wallpaper}
              >
                {t.appearance.chooseWallpaper}
              </button>
            </div>
          </div>
        </div>
      </div>
      </div>
    </div>
  );
};
