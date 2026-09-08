import React from 'react';
import { Pencil, X } from 'lucide-react';
import styles from './HotkeyRow.module.css';

export interface HotkeyRowProps {
  label: string;
  displayValue: string;
  isRecording: boolean;
  hasError?: boolean;
  errorText?: string;
  badgeTitle?: string;
  editTitle: string;
  onEditToggle: () => void;
}

export const HotkeyRow: React.FC<HotkeyRowProps> = ({
  label,
  displayValue,
  isRecording,
  hasError = false,
  errorText,
  badgeTitle,
  editTitle,
  onEditToggle,
}) => {
  return (
    <div className={styles.hotkeyRow}>
      <div className={styles.hotkeyLabelCol}>
        <span className={styles.hotkeyActionName}>{label}</span>
        {hasError && errorText && (
          <span className={styles.hotkeyErrorText}>{errorText}</span>
        )}
      </div>
      <div className={styles.hotkeyBadgeArea}>
        <span
          className={`${styles.kbdBadge} ${isRecording ? styles.recordingBadge : ''} ${
            hasError ? styles.errorBadge : ''
          }`}
          title={badgeTitle}
        >
          {displayValue}
        </span>
        <button
          type="button"
          className={`${styles.btnEditHotkey} ${isRecording ? styles.recording : ''}`}
          onClick={onEditToggle}
          title={editTitle}
          aria-label={editTitle}
        >
          {isRecording ? <X size={14} /> : <Pencil size={13} />}
        </button>
      </div>
    </div>
  );
};
