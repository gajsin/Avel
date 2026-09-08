import React from 'react';
import { MoreVertical, Check as CheckIcon } from 'lucide-react';
import { Copy as CopyIconNode, Check as CheckIconNode } from 'lucide';
import { convertFileSrc } from '@tauri-apps/api/core';
import { MorphIcon } from '../../components/MorphIcon/MorphIcon';
import { ItemStarButton } from '../../components/ItemStarButton/ItemStarButton';
import { ClipboardItem } from '../../types';
import { useApp } from '../../context/AppContext';
import styles from './ClipboardScreen.module.css';

export interface ClipboardGridViewProps {
  items: ClipboardItem[];
  selectedIds: Set<number>;
  copiedId: number | null;
  animatingPinId: number | null;
  activeMenuId?: number | null;
  onItemClick: (e: React.MouseEvent, item: ClipboardItem) => void;
  onItemDoubleClick: (e: React.MouseEvent, item: ClipboardItem) => void;
  onItemContextMenu: (e: React.MouseEvent, item: ClipboardItem) => void;
  onCheckboxClick: (e: React.MouseEvent, id: number) => void;
  onTogglePin: (item: ClipboardItem) => void;
  onCopy: (item: ClipboardItem) => void;
  onMenuBtnClick: (e: React.MouseEvent, item: ClipboardItem) => void;
  formatCardTime: (iso: string) => string;
}

export const ClipboardGridView: React.FC<ClipboardGridViewProps> = ({
  items,
  selectedIds,
  copiedId,
  animatingPinId,
  activeMenuId,
  onItemClick,
  onItemDoubleClick,
  onItemContextMenu,
  onCheckboxClick,
  onTogglePin,
  onCopy,
  onMenuBtnClick,
  formatCardTime,
}) => {
  const { t } = useApp();

  return (
    <div className={styles.gridWrapper}>
      {items.map((item) => {
        const isImg = item.content_type === 'image' || item.content_type === 'screenshot';
        const imgSrc = item.thumbnail_b64 || (item.image_path ? convertFileSrc(item.image_path) : '');
        const isScreenshot = item.content_type === 'screenshot';
        const isSelected = selectedIds.has(item.id);

        return (
          <div
            key={item.id}
            className={`${styles.gridCard} ${isSelected ? styles.selected : ''}`}
            onClick={(e) => onItemClick(e, item)}
            onDoubleClick={(e) => onItemDoubleClick(e, item)}
            onContextMenu={(e) => onItemContextMenu(e, item)}
          >
            <button
              type="button"
              className={`${styles.gridCardCheckBtn} ${isSelected ? styles.checked : ''}`}
              onClick={(e) => onCheckboxClick(e, item.id)}
              title={isSelected ? t.common.deselect : t.common.select}
              aria-label={isSelected ? t.common.deselect : t.common.select}
            >
              {isSelected && <CheckIcon size={11} strokeWidth={3} />}
            </button>

            <div className={styles.gridThumbArea}>
              {isImg && imgSrc ? (
                <img
                  src={imgSrc}
                  alt=""
                  loading="lazy"
                  decoding="async"
                  className={isScreenshot ? styles.gridContainImg : styles.gridCoverImg}
                />
              ) : (
                <div className={styles.gridTextPreview}>
                  <p className={styles.gridTextSnippet}>{item.text_content}</p>
                </div>
              )}

              {(selectedIds.size === 0 || activeMenuId === item.id) && (
                <div className={styles.gridCardActions}>
                  <ItemStarButton
                    className={styles.gridActionBtn}
                    isPinned={item.is_pinned}
                    isAnimating={animatingPinId === item.id}
                    onToggle={() => onTogglePin(item)}
                    size={11}
                    title={item.is_pinned ? t.common.unfavorite : t.common.favorite}
                  />
                  <button
                    type="button"
                    className={styles.gridActionBtn}
                    onClick={(e) => {
                      e.stopPropagation();
                      onCopy(item);
                    }}
                    title={t.clipboard.copy}
                    aria-label={t.clipboard.copy}
                  >
                    <MorphIcon
                      icon={copiedId === item.id ? CheckIconNode : CopyIconNode}
                      size={11}
                      spring="snappy"
                      color={copiedId === item.id ? 'var(--accent-text)' : 'currentColor'}
                    />
                  </button>
                  <button
                    type="button"
                    className={styles.gridActionBtn}
                    onClick={(e) => onMenuBtnClick(e, item)}
                    title={t.common.options}
                    aria-label={t.common.options}
                  >
                    <MoreVertical size={11} />
                  </button>
                </div>
              )}
            </div>

            <div className={styles.gridInfo}>
              {isImg ? (
                <>
                  <div className={styles.gridImageHeader}>
                    <span className={styles.gridImageTitle}>
                      {t.clipboard.image}
                    </span>
                    <span className={styles.gridMetaRight}>
                      {formatCardTime(item.created_at)}
                    </span>
                  </div>
                  <div className={styles.gridSub}>
                    {item.content_type === 'screenshot'
                      ? t.clipboard.screenshot
                      : t.clipboard.image}
                    {item.width && item.height ? ` · ${item.width}×${item.height}` : ''}
                  </div>
                </>
              ) : (
                <div className={styles.gridInfoText}>
                  <span className={styles.gridMetaLeft}>
                    {item.content_type === 'link'
                      ? t.clipboard.link
                      : `${t.clipboard.text} · ${item.char_count || item.text_content?.length || 0} ${t.clipboard.chars}`}
                  </span>
                  <span className={styles.gridMetaRight}>
                    {formatCardTime(item.created_at)}
                  </span>
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};
