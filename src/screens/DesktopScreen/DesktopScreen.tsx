import React, { useEffect, useState } from 'react';
import { Trash2, Folder, Search } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { useApp } from '../../context/AppContext';
import { api } from '../../services/api';
import { CleanDesktopMode, CleanDesktopState } from '../../types';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { PreviewSectionHeader } from '../../components/PreviewSectionHeader/PreviewSectionHeader';
import desktopWallpaper from '../../assets/desktop-wallpaper.jpg';
import styles from './DesktopScreen.module.css';

const defaultState: CleanDesktopState = {
  current_mode: 'none',
  is_hidden: false,
  actual_icons_hidden: false,
  actual_taskbar_hidden: false,
};

interface DesktopMockupProps {
  mode: CleanDesktopMode;
  isAfter: boolean;
  recycleBinLabel: string;
  filesLabel: string;
  lang: string;
}

const DesktopMockup: React.FC<DesktopMockupProps> = ({
  mode,
  isAfter,
  recycleBinLabel,
  filesLabel,
  lang,
}) => {
  // In "До" (before), icons and taskbar are always visible
  // In "После" (after), hide elements based on selected mode
  const showIcons = isAfter ? (mode === 'taskbar' || mode === 'none') : true;
  const showTaskbar = isAfter ? (mode === 'icons' || mode === 'none') : true;

  return (
    <div className={styles.mockupContainer}>
      <img src={desktopWallpaper} alt="Desktop Preview" className={styles.mockupWallpaper} />

      {showIcons && (
        <div className={styles.iconsOverlay}>
          <div className={styles.desktopIconItem}>
            <div className={styles.iconBoxRecycle}>
              <Trash2 size={13} strokeWidth={2} />
            </div>
            <span className={styles.iconLabel}>{recycleBinLabel}</span>
          </div>
          <div className={styles.desktopIconItem}>
            <div className={styles.iconBoxFolder}>
              <Folder size={13} strokeWidth={2} />
            </div>
            <span className={styles.iconLabel}>{filesLabel}</span>
          </div>
        </div>
      )}

      {showTaskbar && (
        <div className={styles.taskbarOverlay}>
          <div className={styles.taskbarLeft}>
            <div className={styles.startBtn}>
              <div style={{ width: 8, height: 8, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 1 }}>
                <span style={{ background: '#38BDF8', borderRadius: 0.5 }} />
                <span style={{ background: '#38BDF8', borderRadius: 0.5 }} />
                <span style={{ background: '#38BDF8', borderRadius: 0.5 }} />
                <span style={{ background: '#38BDF8', borderRadius: 0.5 }} />
              </div>
            </div>
            <div className={styles.searchMini}>
              <Search size={8} strokeWidth={2.5} />
            </div>
          </div>

          <div className={styles.taskbarCenter}>
            <div className={styles.appIconExplorer} />
            <div className={styles.appIconBrowser} />
            <div className={styles.appIconAvel} />
            <div className={styles.appIconCode} />
          </div>

          <div className={styles.taskbarTray}>
            <span className={styles.trayLang}>{lang}</span>
            <span className={styles.trayTime}>12:00</span>
          </div>
        </div>
      )}
    </div>
  );
};

export const DesktopScreen: React.FC = () => {
  const { t, language, showToast } = useApp();
  const [state, setState] = useState<CleanDesktopState>(defaultState);
  const [isApplying, setIsApplying] = useState(false);
  const [showExample, setShowExample] = useState(true);

  const loadState = () => {
    api.getCleanDesktopState().then(setState).catch(console.error);
  };

  useEffect(() => {
    loadState();
  }, []);

  useEffect(() => {
    const unlisten = listen<CleanDesktopState>('desktop-state-changed', (event) => {
      setState(event.payload);
    });

    return () => {
      unlisten.then((u) => u());
    };
  }, []);

  const currentEffectiveMode: CleanDesktopMode =
    state.current_mode === 'icons' ||
    state.current_mode === 'taskbar' ||
    state.current_mode === 'all'
      ? state.current_mode
      : 'none';

  const handleSelectRadio = async (mode: CleanDesktopMode) => {
    if (isApplying) return;
    if (currentEffectiveMode === mode) return;
    setIsApplying(true);
    try {
      const res = await api.setCleanDesktopMode(mode);
      setState(res);
    } catch (err: any) {
      showToast(err?.toString() || t.desktop.switchError);
      loadState();
    } finally {
      setIsApplying(false);
    }
  };

  const radioOptions: { id: CleanDesktopMode; label: string }[] = [
    { id: 'none', label: t.desktop.modeNone },
    { id: 'icons', label: t.desktop.modeIcons },
    { id: 'taskbar', label: t.desktop.modeTaskbar },
    { id: 'all', label: t.desktop.modeAll },
  ];

  return (
    <div className={styles.container}>
      {/* Page Header */}
      <PageHeader title={t.nav.desktop} />

      {/* Radio Options Section */}
      <div className={styles.section}>
        <span className={styles.sectionTitle}>{t.desktop.whatToHide}</span>

        <div className={styles.radioList}>
          {radioOptions.map((opt) => {
            const isSelected = currentEffectiveMode === opt.id;
            return (
              <label
                key={opt.id}
                className={`${styles.radioItem} ${isSelected ? styles.active : ''} ${isApplying ? styles.disabled : ''}`}
                onClick={(e) => {
                  e.preventDefault();
                  if (!isApplying) handleSelectRadio(opt.id);
                }}
              >
                <div className={styles.customRadio}>
                  {isSelected && <div className={styles.radioDot} />}
                </div>
                <span>{opt.label}</span>
              </label>
            );
          })}
        </div>
      </div>

      {/* Example Showcase */}
      <div className={styles.previewCard}>
        <PreviewSectionHeader
          title={t.desktop.showExample}
          expanded={showExample}
          onToggle={() => setShowExample(!showExample)}
        />

        {showExample && (
          <div className={styles.previewGrid}>
            <div className={styles.previewCol}>
              <span className={styles.previewLabel}>{t.desktop.before}</span>
              <div className={styles.previewMockup}>
                <DesktopMockup
                  mode={currentEffectiveMode}
                  isAfter={false}
                  recycleBinLabel={t.desktop.recycleBin}
                  filesLabel={t.desktop.files}
                  lang={language.toUpperCase()}
                />
              </div>
            </div>

            <div className={styles.previewCol}>
              <span className={styles.previewLabel}>{t.desktop.after}</span>
              <div className={styles.previewMockup}>
                <DesktopMockup
                  mode={currentEffectiveMode}
                  isAfter={true}
                  recycleBinLabel={t.desktop.recycleBin}
                  filesLabel={t.desktop.files}
                  lang={language.toUpperCase()}
                />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
