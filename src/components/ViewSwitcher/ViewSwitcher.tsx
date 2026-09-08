import React from 'react';
import { Table as TableIcon, LayoutGrid } from 'lucide-react';
import { useApp } from '../../context/AppContext';
import styles from './ViewSwitcher.module.css';

export interface ViewSwitcherProps {
  viewMode: 'details' | 'grid';
  onChange: (mode: 'details' | 'grid') => void;
}

export const ViewSwitcher: React.FC<ViewSwitcherProps> = ({ viewMode, onChange }) => {
  const { t } = useApp();

  const tableTitle = t.common.viewDetails;
  const gridTitle = t.common.viewGrid;

  return (
    <div className={styles.viewModeGroup} role="radiogroup" aria-label={tableTitle}>
      <button
        type="button"
        className={`${styles.viewBtn} ${viewMode === 'details' ? styles.active : ''}`}
        onClick={() => onChange('details')}
        title={tableTitle}
        aria-label={tableTitle}
      >
        <TableIcon size={16} />
      </button>
      <button
        type="button"
        className={`${styles.viewBtn} ${viewMode === 'grid' ? styles.active : ''}`}
        onClick={() => onChange('grid')}
        title={gridTitle}
        aria-label={gridTitle}
      >
        <LayoutGrid size={16} />
      </button>
    </div>
  );
};
