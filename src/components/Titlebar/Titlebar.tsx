import React from 'react';
import { Minus, X } from 'lucide-react';
import { api } from '../../services/api';
import { useApp } from '../../context/AppContext';
import styles from './Titlebar.module.css';

export const Titlebar: React.FC = () => {
  const { t } = useApp();

  const handleMinimize = () => {
    api.minimize().catch(console.error);
  };

  const handleClose = () => {
    api.hide().catch(console.error);
  };

  const minimizeLabel = t.titlebar.minimize;
  const closeLabel = t.titlebar.hideToTray;

  return (
    <div className={styles.titlebar} data-tauri-drag-region>
      <div className={styles.dragRegion} data-tauri-drag-region />
      <div className={styles.windowControls}>
        <button
          className={styles.controlBtn}
          onClick={handleMinimize}
          title={minimizeLabel}
          aria-label={minimizeLabel}
        >
          <Minus size={14} />
        </button>
        <button
          className={`${styles.controlBtn} ${styles.closeBtn}`}
          onClick={handleClose}
          title={closeLabel}
          aria-label={closeLabel}
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
};
