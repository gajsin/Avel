import React, { useEffect, useState, useCallback } from 'react';
import { RefreshCw, SunMedium } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { useApp } from '../../context/AppContext';
import { api } from '../../services/api';
import { BrightnessDisplay, BrightnessPreferences, BrightnessStateSnapshot } from '../../types';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { ToggleSwitch } from '../../components/ToggleSwitch/ToggleSwitch';
import { DisplayRow } from './DisplayRow';
import styles from './BrightnessScreen.module.css';

export const BrightnessScreen: React.FC = () => {
  const { t, showToast } = useApp();

  const [displays, setDisplays] = useState<BrightnessDisplay[]>([]);
  const [preferences, setPreferences] = useState<BrightnessPreferences>({
    sync_group: false,
    step_percent: 5,
    group_display_ids: [],
    hotkey_brightness_target: 'cursor',
    hotkey_brightness_up: null,
    hotkey_brightness_down: null,
    hotkey_brightness_screen: null,
  });

  const [isRefreshing, setIsRefreshing] = useState(false);

  const loadState = useCallback(async () => {
    try {
      const snap = await api.getBrightnessState();
      setDisplays(snap.displays);
      setPreferences(snap.preferences);
    } catch {
      // Backend error fallback
    }
  }, []);

  useEffect(() => {
    loadState();

    const unlistenPromise = listen<BrightnessStateSnapshot>(
      'brightness-state-changed',
      (event) => {
        if (event.payload) {
          setDisplays(event.payload.displays);
          setPreferences(event.payload.preferences);
        }
      }
    );

    return () => {
      unlistenPromise.then((u) => u());
    };
  }, [loadState]);

  const handleRefresh = async () => {
    if (isRefreshing) return;
    setIsRefreshing(true);
    try {
      const snap = await api.refreshBrightness();
      setDisplays(snap.displays);
      setPreferences(snap.preferences);
      showToast(t.brightness.refresh);
    } catch (err: any) {
      showToast(err?.toString() || 'Refresh failed');
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleSyncGroupToggle = async (checked: boolean) => {
    const nextPrefs = { ...preferences, sync_group: checked };
    setPreferences(nextPrefs);
    try {
      await api.saveBrightnessPreferences(nextPrefs);
    } catch (err: any) {
      showToast(err?.toString() || 'Failed to save preferences');
    }
  };

  const handleBrightnessChange = async (displayId: string, percent: number) => {
    // Optimistic local state update for zero latency
    setDisplays((prev) =>
      prev.map((d) => {
        if (preferences.sync_group || d.id === displayId) {
          return { ...d, target_percent: percent, current_percent: percent };
        }
        return d;
      })
    );

    try {
      if (preferences.sync_group) {
        await api.setGroupBrightness(percent);
      } else {
        await api.setBrightness(displayId, percent);
      }
    } catch {
      // Background write will retry or log
    }
  };

  return (
    <div className={styles.container}>
      <div className={styles.body}>
        <PageHeader
          title={t.brightness.title}
          action={
            <button
              type="button"
              className={`${styles.actionButton} ${isRefreshing ? styles.rotating : ''}`}
              onClick={handleRefresh}
              disabled={isRefreshing}
              title={t.brightness.refreshTooltip}
            >
              <RefreshCw size={13} />
              <span>{t.brightness.refresh}</span>
            </button>
          }
        />

        {/* Global Sync Card */}
        {displays.length > 1 && (
          <div className={styles.syncCard}>
            <div className={styles.syncMeta}>
              <span className={styles.syncTitle}>{t.brightness.syncGroup}</span>
              <span className={styles.syncDesc}>{t.brightness.syncGroupDesc}</span>
            </div>
            <ToggleSwitch
              checked={preferences.sync_group}
              onChange={handleSyncGroupToggle}
              ariaLabel={t.brightness.syncGroup}
            />
          </div>
        )}

        {/* Displays List */}
        {displays.length > 0 ? (
          <div className={styles.displayList}>
            {displays.map((display) => (
              <DisplayRow
                key={display.id}
                display={display}
                step={preferences.step_percent}
                onBrightnessChange={handleBrightnessChange}
              />
            ))}
          </div>
        ) : (
          <div className={styles.emptyCard}>
            <div className={styles.emptyIcon}>
              <SunMedium size={24} />
            </div>
            <h3 className={styles.emptyTitle}>{t.brightness.noDisplaysFound}</h3>
            <p className={styles.emptyDesc}>{t.brightness.noDisplaysDesc}</p>
          </div>
        )}
      </div>
    </div>
  );
};
