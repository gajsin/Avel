import React from 'react';
import { Search, X } from 'lucide-react';
import { useApp } from '../../context/AppContext';
import styles from './SearchInput.module.css';

export interface SearchInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  autoFocus?: boolean;
}

export const SearchInput: React.FC<SearchInputProps> = ({
  value,
  onChange,
  placeholder,
  autoFocus,
}) => {
  const { t } = useApp();

  return (
    <div className={styles.searchWrapper}>
      <Search size={16} className={styles.searchIcon} />
      <input
        type="text"
        data-unstyled="true"
        className={styles.searchInput}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        autoFocus={autoFocus}
      />
      {value && (
        <button
          type="button"
          className={styles.clearSearchBtn}
          onClick={() => onChange('')}
          title={t.common.clear}
          aria-label={t.common.clear}
        >
          <X size={14} />
        </button>
      )}
    </div>
  );
};
