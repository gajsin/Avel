import React from 'react';
import { Clock } from 'lucide-react';
import styles from './TimeField.module.css';

export interface TimeFieldProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  'aria-label'?: string;
}

export const TimeField: React.FC<TimeFieldProps> = ({
  value,
  onChange,
  disabled = false,
  'aria-label': ariaLabel,
}) => {
  return (
    <div className={`${styles.timeInputWrapper} ${disabled ? styles.disabled : ''}`}>
      <input
        type="time"
        data-unstyled="true"
        className={styles.timeInput}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        aria-label={ariaLabel}
      />
      <Clock size={13} color="var(--text-muted)" />
    </div>
  );
};
