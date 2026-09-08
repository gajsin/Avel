import React from 'react';
import { MoreVertical, Check as CheckIcon } from 'lucide-react';
import { Copy as CopyIconNode, Check as CheckIconNode } from 'lucide';
import { MorphIcon } from '../../components/MorphIcon/MorphIcon';
import { ItemStarButton } from '../../components/ItemStarButton/ItemStarButton';
import { ClipboardItem } from '../../types';
import { useApp } from '../../context/AppContext';
import styles from './ClipboardScreen.module.css';

export interface ClipboardTableViewProps {
  items: ClipboardItem[];
  selectedIds: Set<number>;
  copiedId: number | null;
  animatingPinId: number | null;
  activeMenuId?: number | null;
  onItemClick: (e: React.MouseEvent, item: ClipboardItem) => void;
  onItemDoubleClick: (e: React.MouseEvent, item: ClipboardItem) => void;
  onItemContextMenu: (e: React.MouseEvent, item: ClipboardItem) => void;
  onCheckboxClick: (e: React.MouseEvent, id: number) => void;
  onHeaderCheckboxClick: (e: React.MouseEvent) => void;
  onTogglePin: (item: ClipboardItem) => void;
  onCopy: (item: ClipboardItem) => void;
  onMenuBtnClick: (e: React.MouseEvent, item: ClipboardItem) => void;
  getItemDisplayName: (item: ClipboardItem) => string;
  renderItemVisual: (item: ClipboardItem, size?: number) => React.ReactNode;
  renderSubtitle: (item: ClipboardItem, compact?: boolean) => string;
  formatTime: (iso: string) => string;
}

export const ClipboardTableView: React.FC<ClipboardTableViewProps> = ({
  items,
  selectedIds,
  copiedId,
  animatingPinId,
  activeMenuId,
  onItemClick,
  onItemDoubleClick,
  onItemContextMenu,
  onCheckboxClick,
  onHeaderCheckboxClick,
  onTogglePin,
  onCopy,
  onMenuBtnClick,
  getItemDisplayName,
  renderItemVisual,
  renderSubtitle,
  formatTime,
}) => {
  const { t } = useApp();

  const allFilteredSelected = items.length > 0 && items.every((i) => selectedIds.has(i.id));
  const someFilteredSelected = items.some((i) => selectedIds.has(i.id));
  const isIndeterminate = someFilteredSelected && !allFilteredSelected;

  return (
    <div className={styles.tableContainer}>
      <table className={styles.table}>
        <thead>
          <tr>
            <th style={{ width: '36px', textAlign: 'center' }}>
              <button
                type="button"
                className={`${styles.tableCheckBtn} ${
                  allFilteredSelected ? styles.checked : isIndeterminate ? styles.indeterminate : ''
                }`}
                onClick={onHeaderCheckboxClick}
                title={allFilteredSelected ? t.bulk.deselectAll : t.bulk.selectAll}
                aria-label={allFilteredSelected ? t.bulk.deselectAll : t.bulk.selectAll}
              >
                {allFilteredSelected && <CheckIcon size={11} strokeWidth={3} />}
                {isIndeterminate && <div className={styles.indeterminateDash} />}
              </button>
            </th>
            <th style={{ width: '44px' }}></th>
            <th>{t.clipboard.name}</th>
            <th style={{ width: '180px' }}>{t.clipboard.typeDetails}</th>
            <th style={{ width: '90px' }}>{t.clipboard.time}</th>
            <th style={{ width: '96px' }}></th>
          </tr>
        </thead>
        <tbody>
          {items.map((item) => {
            const isSelected = selectedIds.has(item.id);
            return (
              <tr
                key={item.id}
                className={isSelected ? styles.selected : ''}
                onClick={(e) => onItemClick(e, item)}
                onDoubleClick={(e) => onItemDoubleClick(e, item)}
                onContextMenu={(e) => onItemContextMenu(e, item)}
              >
                <td style={{ textAlign: 'center', width: '36px' }}>
                  <button
                    type="button"
                    className={`${styles.tableCheckBtn} ${isSelected ? styles.checked : ''}`}
                    onClick={(e) => onCheckboxClick(e, item.id)}
                    title={isSelected ? t.common.deselect : t.common.select}
                    aria-label={isSelected ? t.common.deselect : t.common.select}
                  >
                    {isSelected && <CheckIcon size={11} strokeWidth={3} />}
                  </button>
                </td>
                <td style={{ textAlign: 'center', width: '44px' }}>{renderItemVisual(item, 32)}</td>
                <td className={styles.tableNameCell}>
                  <span className={styles.tableNameText} title={getItemDisplayName(item)}>
                    {getItemDisplayName(item)}
                  </span>
                </td>
                <td className={styles.tableTypeCell}>
                  <span className={styles.tableTypeText} title={renderSubtitle(item, true)}>
                    {renderSubtitle(item, true)}
                  </span>
                </td>
                <td className={styles.tableTimeCell}>
                  <span className={styles.tableTimeText}>
                    {formatTime(item.updated_at)}
                  </span>
                </td>
                <td className={styles.tableTrailingCell}>
                  {(selectedIds.size === 0 || activeMenuId === item.id) && (
                    <div className={styles.tableActionCell}>
                      <ItemStarButton
                        className={styles.menuBtn}
                        isPinned={item.is_pinned}
                        isAnimating={animatingPinId === item.id}
                        onToggle={() => onTogglePin(item)}
                        size={14}
                        title={item.is_pinned ? t.common.unfavorite : t.common.favorite}
                      />
                      <button
                        type="button"
                        className={styles.menuBtn}
                        onClick={(e) => {
                          e.stopPropagation();
                          onCopy(item);
                        }}
                        title={t.clipboard.copy}
                        aria-label={t.clipboard.copy}
                      >
                        <MorphIcon
                          icon={copiedId === item.id ? CheckIconNode : CopyIconNode}
                          size={14}
                          spring="snappy"
                          color={copiedId === item.id ? 'var(--accent-text)' : 'currentColor'}
                        />
                      </button>
                      <button
                        type="button"
                        className={styles.menuBtn}
                        onClick={(e) => onMenuBtnClick(e, item)}
                        title={t.common.options}
                        aria-label={t.common.options}
                      >
                        <MoreVertical size={14} />
                      </button>
                    </div>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};
