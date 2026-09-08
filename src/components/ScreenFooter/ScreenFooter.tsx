import React from 'react';
import { formatItemsCount } from '../../i18n';
import { useApp } from '../../context/AppContext';
import styles from './ScreenFooter.module.css';

export interface ScreenFooterProps {
  count: number;
  onClear?: () => void;
  clearLabel?: string;
  className?: string;
}

export const ScreenFooter: React.FC<ScreenFooterProps> = ({
  count,
  onClear,
  clearLabel,
  className = '',
}) => {
  const { language } = useApp();

  return (
    <div className={`${styles.footer} ${className}`}>
      <span className={styles.countText}>{formatItemsCount(count, language)}</span>
      {count > 0 && onClear && clearLabel && (
        <button
          type="button"
          className={styles.clearBtn}
          onClick={onClear}
        >
          {clearLabel}
        </button>
      )}
    </div>
  );
};
