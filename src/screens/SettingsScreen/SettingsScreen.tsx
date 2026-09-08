import React, { useState, useEffect } from 'react';
import { ExternalLink } from 'lucide-react';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
import { useApp } from '../../context/AppContext';
import { api } from '../../services/api';
import { AppTheme, ShortcutSpec } from '../../types';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { HotkeyRow } from '../../components/HotkeyRow/HotkeyRow';
import { UnderlineTabs } from '../../components/UnderlineTabs/UnderlineTabs';
import { ToggleSwitch } from '../../components/ToggleSwitch/ToggleSwitch';
import styles from './SettingsScreen.module.css';

import { parseShortcutSpec, formatShortcutDisplay, areSpecsEqual } from '../../utils/hotkeys';

export const SettingsScreen: React.FC = () => {
  const {
    settings,
    updateSettings,
    language,
    setLanguage,
    appTheme,
    setAppTheme,
    t,
    showToast,
    setLocalSettings,
  } = useApp();

  const [recordingAction, setRecordingAction] = useState<string | null>(null);
  const [pendingModifiers, setPendingModifiers] = useState<{
    ctrl: boolean;
    alt: boolean;
    shift: boolean;
    meta: boolean;
  } | null>(null);
  const [autostartActive, setAutostartActive] = useState(false);

  useEffect(() => {
    isEnabled().then(setAutostartActive).catch(() => {});
  }, []);

  const handleAutostartToggle = async (enabledState: boolean) => {
    try {
      if (enabledState) {
        await enable();
      } else {
        await disable();
      }
      setAutostartActive(enabledState);
      await updateSettings({ autostart: enabledState });
      showToast(t.settings.autostart);
    } catch (err: any) {
      showToast(err?.toString() || t.settings.autostartError);
    }
  };

  const [hotkeyStatuses, setHotkeyStatuses] = useState<
    Record<string, { is_registered: boolean; error_message: string | null }>
  >({});

  const [brightnessPrefs, setBrightnessPrefs] = useState<import('../../types').BrightnessPreferences | null>(null);

  const loadStatuses = () => {
    api.getHotkeyStatuses().then(setHotkeyStatuses).catch(() => {});
    api.getBrightnessPreferences().then(setBrightnessPrefs).catch(() => {});
  };

  useEffect(() => {
    loadStatuses();
  }, [settings]);

  const hotkeysList = [
    {
      action: 'clipboard',
      label: t.nav.clipboard,
      spec: parseShortcutSpec(settings.hotkey_clipboard, 'Equal'),
    },
    {
      action: 'recent',
      label: t.nav.recent,
      spec: parseShortcutSpec(settings.hotkey_recent, 'F8'),
    },
    {
      action: 'appearance',
      label: t.nav.appearance,
      spec: parseShortcutSpec(settings.hotkey_appearance, 'KeyD'),
    },
    {
      action: 'desktop',
      label: t.nav.desktop,
      spec: parseShortcutSpec(settings.hotkey_desktop, 'ArrowDown'),
    },
    {
      action: 'brightness_up',
      label: t.settings.brightnessUp,
      spec: parseShortcutSpec(brightnessPrefs?.hotkey_brightness_up || ''),
    },
    {
      action: 'brightness_down',
      label: t.settings.brightnessDown,
      spec: parseShortcutSpec(brightnessPrefs?.hotkey_brightness_down || ''),
    },
    {
      action: 'brightness_screen',
      label: t.settings.brightnessScreen,
      spec: parseShortcutSpec(brightnessPrefs?.hotkey_brightness_screen || ''),
    },
  ];

  const hotkeyUnavailableText = t.settings.hotkeyConflict;

  // Hotkey recorder keydown listener (capture phase!)
  useEffect(() => {
    if (!recordingAction) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.repeat) return;

      if (e.code === 'Escape') {
        setRecordingAction(null);
        setPendingModifiers(null);
        return;
      }

      const isModifierOnly = [
        'ControlLeft',
        'ControlRight',
        'AltLeft',
        'AltRight',
        'ShiftLeft',
        'ShiftRight',
        'MetaLeft',
        'MetaRight',
      ].includes(e.code);

      if (isModifierOnly) {
        setPendingModifiers({
          ctrl: e.ctrlKey,
          alt: e.altKey,
          shift: e.shiftKey,
          meta: e.metaKey,
        });
        return;
      }

      // Valid physical key code pressed!
      const newSpec: ShortcutSpec = {
        code: e.code,
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        meta: e.metaKey,
      };

      // Check conflict with other Avel actions
      const conflict = hotkeysList.find(
        (h) => h.action !== recordingAction && areSpecsEqual(h.spec, newSpec)
      );
      if (conflict) {
        showToast(`${t.settings.shortcutInUse} (${conflict.label})`);
        return;
      }

      saveNewHotkey(recordingAction, newSpec);
      setRecordingAction(null);
      setPendingModifiers(null);
    };

    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [recordingAction, settings]);

  const saveNewHotkey = async (action: string, newSpec: ShortcutSpec) => {
    try {
      await api.updateGlobalHotkeySpec(action, newSpec);

      const jsonStr = JSON.stringify(newSpec);
      if (action === 'clipboard') setLocalSettings({ hotkey_clipboard: jsonStr });
      if (action === 'recent') setLocalSettings({ hotkey_recent: jsonStr });
      if (action === 'appearance') setLocalSettings({ hotkey_appearance: jsonStr });
      if (action === 'desktop') setLocalSettings({ hotkey_desktop: jsonStr });
      if (brightnessPrefs) {
        const next = { ...brightnessPrefs };
        if (action === 'brightness_up') next.hotkey_brightness_up = jsonStr;
        if (action === 'brightness_down') next.hotkey_brightness_down = jsonStr;
        if (action === 'brightness_screen') next.hotkey_brightness_screen = jsonStr;
        setBrightnessPrefs(next);
      }

      showToast(`${t.settings.hotkeySaved}: ${formatShortcutDisplay(newSpec)}`);
    } catch {
      showToast(hotkeyUnavailableText);
    }
  };

  const getRecordingDisplay = () => {
    if (!pendingModifiers) {
      return t.settings.pressKey;
    }
    const parts: string[] = [];
    if (pendingModifiers.ctrl) parts.push('Ctrl');
    if (pendingModifiers.alt) parts.push('Alt');
    if (pendingModifiers.shift) parts.push('Shift');
    if (pendingModifiers.meta) parts.push('Win');
    if (parts.length === 0) return t.settings.pressKey;
    return `${parts.join(' + ')} + ...`;
  };

  const themeOptions: { value: AppTheme; label: string }[] = [
    { value: 'System', label: t.settings.system },
    { value: 'Light', label: t.settings.light },
    { value: 'Dark', label: t.settings.dark },
  ];

  return (
    <div className={styles.container}>
      <div className={styles.settingsBody}>
        {/* Page Header */}
        <PageHeader title={t.nav.settings} />

        {/* General Settings */}
        <div className={styles.section}>
          {/* App Theme Underline Tabs */}
          <div className={styles.settingsRowTabs}>
            <span className={styles.rowLabelTabs}>{t.settings.appTheme}</span>
            <UnderlineTabs<AppTheme>
              value={appTheme}
              options={themeOptions}
              onChange={(theme) => setAppTheme(theme)}
              ariaLabel={t.settings.appTheme}
            />
          </div>

          {/* Language Underline Tabs */}
          <div className={styles.settingsRowTabs}>
            <span className={styles.rowLabelTabs}>{t.settings.language}</span>
            <UnderlineTabs<'ru' | 'en'>
              value={language}
              options={[
                { value: 'ru', label: t.settings.russian },
                { value: 'en', label: t.settings.english },
              ]}
              onChange={(lang) => setLanguage(lang)}
              ariaLabel={t.settings.language}
            />
          </div>

          {/* Autostart */}
          <div className={styles.settingsRow}>
            <span className={styles.rowLabel}>{t.settings.autostartDesc || t.settings.autostart}</span>
            <ToggleSwitch
              checked={autostartActive}
              onChange={(checked) => handleAutostartToggle(checked)}
              ariaLabel={t.settings.autostart}
            />
          </div>
        </div>

      {/* Hotkeys Settings */}
      <div className={styles.section}>
        <h3 className={styles.sectionTitle}>{t.settings.globalHotkeys}</h3>

        <div className={styles.hotkeyTable}>
          {hotkeysList.map((item) => {
            const isRecording = recordingAction === item.action;
            const status = hotkeyStatuses[item.action];
            const hasError = status && !status.is_registered;

            return (
              <HotkeyRow
                key={item.action}
                label={item.label}
                displayValue={isRecording ? getRecordingDisplay() : formatShortcutDisplay(item.spec)}
                isRecording={isRecording}
                hasError={hasError}
                errorText={hotkeyUnavailableText}
                badgeTitle={hasError ? hotkeyUnavailableText : t.settings.active}
                editTitle={isRecording ? t.settings.cancelEsc : t.settings.changeHotkey}
                onEditToggle={() => {
                  if (isRecording) {
                    setRecordingAction(null);
                    setPendingModifiers(null);
                  } else {
                    setRecordingAction(item.action);
                    setPendingModifiers(null);
                  }
                }}
              />
            );
          })}
        </div>
      </div>

      {/* Compact Minimal Footer */}
      <div className={styles.footer}>
        <button
          type="button"
          className={styles.githubLink}
          onClick={() => api.openExternal('https://github.com/gajsin/avel')}
        >
          <span>{t.settings.github}</span>
          <ExternalLink size={13} />
        </button>
        <span className={styles.versionText}>{t.settings.version}</span>
      </div>
      </div>
    </div>
  );
};
