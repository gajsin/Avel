
import styles from './FilterTabs.module.css';

export interface FilterTabItem<T extends string> {
  id: T;
  label: string;
}

export interface FilterTabsProps<T extends string> {
  tabs: readonly FilterTabItem<T>[];
  activeTab: T;
  onChange: (id: T) => void;
}

export function FilterTabs<T extends string>({
  tabs,
  activeTab,
  onChange,
}: FilterTabsProps<T>) {
  return (
    <div className={styles.tabsRow} role="tablist">
      {tabs.map((tab) => {
        const isActive = activeTab === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={isActive}
            className={`${styles.tabBtn} ${isActive ? styles.active : ''}`}
            onClick={() => onChange(tab.id)}
          >
            {tab.label}
            {isActive && <div className={styles.tabIndicator} />}
          </button>
        );
      })}
    </div>
  );
}
