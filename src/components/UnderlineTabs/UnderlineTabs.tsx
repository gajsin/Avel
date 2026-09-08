import styles from './UnderlineTabs.module.css';

export interface TabOption<T extends string> {
  value: T;
  label: string;
}

export interface UnderlineTabsProps<T extends string> {
  value: T;
  options: TabOption<T>[];
  onChange: (value: T) => void;
  className?: string;
  ariaLabel?: string;
}

export function UnderlineTabs<T extends string>({
  value,
  options,
  onChange,
  className = '',
  ariaLabel,
}: UnderlineTabsProps<T>) {
  return (
    <div
      className={`${styles.tabsControl} ${className}`}
      role="tablist"
      aria-label={ariaLabel}
    >
      {options.map((opt) => {
        const isActive = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            role="tab"
            aria-selected={isActive}
            className={`${styles.tabBtn} ${isActive ? styles.tabActive : ''}`}
            onClick={() => onChange(opt.value)}
          >
            <span>{opt.label}</span>
            {isActive && <div className={styles.tabIndicator} />}
          </button>
        );
      })}
    </div>
  );
}
