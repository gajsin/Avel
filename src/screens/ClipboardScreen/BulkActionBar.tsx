import React, { useState, useRef } from 'react';
import { Copy, Trash2, X, ChevronDown, Check } from 'lucide-react';
import { OverlayMenu, OverlayMenuItem } from '../../components/OverlayMenu/OverlayMenu';
import { ClipboardItem } from '../../types';
import { useApp } from '../../context/AppContext';
import { useCopyFeedback } from '../../hooks/useCopyFeedback';
import { splitSelectedClipboardItems } from '../../utils/clipboard';
import styles from './BulkActionBar.module.css';

export interface BulkActionBarProps {
  selectedItems: ClipboardItem[];
  onClearSelection: () => void;
  onCopyImages: (items: ClipboardItem[]) => void;
  onCopyTexts: (items: ClipboardItem[]) => void;
  onDeleteSelected: () => void;
}

export const BulkActionBar: React.FC<BulkActionBarProps> = ({
  selectedItems,
  onClearSelection,
  onCopyImages,
  onCopyTexts,
  onDeleteSelected,
}) => {
  const { t } = useApp();
  const [menuOpen, setMenuOpen] = useState(false);
  const [copiedState, triggerCopiedState] = useCopyFeedback<string>(1200);
  const justCopied = Boolean(copiedState);
  const splitBtnRef = useRef<HTMLButtonElement>(null);

  const count = selectedItems.length;
  if (count === 0) return null;

  const { imageItems, textItems, isOnlyImages, isOnlyTexts, isMixed } =
    splitSelectedClipboardItems(selectedItems);

  const handleCopyImages = () => {
    onCopyImages(imageItems);
    triggerCopiedState('copied');
  };

  const handleCopyTexts = () => {
    onCopyTexts(textItems);
    triggerCopiedState('copied');
  };

  const countText = `${count} ${t.bulk.selectedCount}`;

  return (
    <div className={styles.barContainer} role="region" aria-label="Bulk actions">
      <div className={styles.barPill}>
        <span className={styles.countBadge}>{countText}</span>

        <div className={styles.divider} />

        {isOnlyImages && (
          <>
            <button
              type="button"
              className={styles.actionBtn}
              onClick={handleCopyImages}
              title={t.bulk.copyImages}
            >
              {justCopied ? <Check size={14} color="var(--accent-text)" /> : <Copy size={14} />}
              <span>{`${t.bulk.copyImages} (${imageItems.length})`}</span>
            </button>
            <div className={styles.divider} />
          </>
        )}

        {isOnlyTexts && (
          <>
            <button
              type="button"
              className={styles.actionBtn}
              onClick={handleCopyTexts}
              title={t.bulk.copyTexts}
            >
              {justCopied ? <Check size={14} color="var(--accent-text)" /> : <Copy size={14} />}
              <span>{`${t.bulk.copyTexts} (${textItems.length})`}</span>
            </button>
            <div className={styles.divider} />
          </>
        )}

        {isMixed && (
          <>
            <button
              ref={splitBtnRef}
              type="button"
              className={`${styles.actionBtn} ${styles.dropdownBtn}`}
              onClick={() => setMenuOpen(!menuOpen)}
            >
              {justCopied ? <Check size={14} color="var(--accent-text)" /> : <Copy size={14} />}
              <span>{t.clipboard.copy}</span>
              <ChevronDown size={13} className={styles.chevron} />
            </button>

            <OverlayMenu
              isOpen={menuOpen}
              onClose={() => setMenuOpen(false)}
              anchorEl={splitBtnRef.current}
              align="start"
            >
              <OverlayMenuItem
                icon={<Copy size={14} />}
                label={`${t.clipboard.image} (${imageItems.length})`}
                onClick={() => {
                  handleCopyImages();
                  setMenuOpen(false);
                }}
              />
              <OverlayMenuItem
                icon={<Copy size={14} />}
                label={`${t.clipboard.text} (${textItems.length})`}
                onClick={() => {
                  handleCopyTexts();
                  setMenuOpen(false);
                }}
              />
            </OverlayMenu>
            <div className={styles.divider} />
          </>
        )}

        <button
          type="button"
          className={`${styles.actionBtn} ${styles.dangerBtn}`}
          onClick={onDeleteSelected}
          title={t.bulk.deleteSelected}
        >
          <Trash2 size={14} />
        </button>

        <button
          type="button"
          className={styles.closeBtn}
          onClick={onClearSelection}
          title={`${t.bulk.clearSelection} (Esc)`}
          aria-label={t.bulk.clearSelection}
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
};
