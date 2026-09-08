import React from 'react';
import { ChevronDown } from 'lucide-react';
import styles from './PreviewSectionHeader.module.css';

export interface PreviewSectionHeaderProps {
  title: string;
  expanded: boolean;
  onToggle: () => void;
}

export const PreviewSectionHeader: React.FC<PreviewSectionHeaderProps> = ({
  title,
  expanded,
  onToggle,
}) => {
  return (
    <div
      className={styles.previewHeader}
      onClick={onToggle}
      role="button"
      tabIndex={0}
      aria-expanded={expanded}
      aria-label={title}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onToggle();
        }
      }}
    >
      <span className={styles.previewCardTitle}>{title}</span>
      <div
        className={`${styles.previewChevronBtn} ${expanded ? styles.expanded : ''}`}
        aria-hidden="true"
      >
        <ChevronDown size={16} />
      </div>
    </div>
  );
};
