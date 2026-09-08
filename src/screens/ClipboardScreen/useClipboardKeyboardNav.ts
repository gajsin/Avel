import { useEffect } from 'react';
import { ClipboardItem } from '../../types';

export interface UseClipboardKeyboardNavOptions {
  items: ClipboardItem[];
  selectedIds: Set<number>;
  anchorId: number | null;
  quickLookItem: ClipboardItem | null;
  hasActiveMenu: boolean;
  clearConfirmOpen: boolean;
  deleteConfirmOpen: boolean;
  onSelectAll: () => void;
  onCopyTrigger: () => void;
  onQuickLook: (item: ClipboardItem | null) => void;
  onCloseMenu: () => void;
  onCloseClearConfirm: () => void;
  onCloseDeleteConfirm: () => void;
  onClearSelection: () => void;
  onSelectOnly: (id: number) => void;
  onExtendSelection: (id: number) => void;
}

export function useClipboardKeyboardNav({
  items,
  selectedIds,
  anchorId,
  quickLookItem,
  hasActiveMenu,
  clearConfirmOpen,
  deleteConfirmOpen,
  onSelectAll,
  onCopyTrigger,
  onQuickLook,
  onCloseMenu,
  onCloseClearConfirm,
  onCloseDeleteConfirm,
  onClearSelection,
  onSelectOnly,
  onExtendSelection,
}: UseClipboardKeyboardNavOptions): void {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const isInput = ['INPUT', 'TEXTAREA'].includes((e.target as HTMLElement)?.tagName);
      if (isInput) return;

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
        e.preventDefault();
        onSelectAll();
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') {
        e.preventDefault();
        onCopyTrigger();
        return;
      }

      if (e.code === 'Enter') {
        e.preventDefault();
        const targetId = anchorId ?? (selectedIds.size === 1 ? Array.from(selectedIds)[0] : null);
        const target = targetId != null ? items.find((i) => i.id === targetId) : items[0];
        if (target) {
          onQuickLook(target);
        }
        return;
      }

      if (e.code === 'Escape') {
        if (quickLookItem) {
          onQuickLook(null);
        } else if (hasActiveMenu) {
          onCloseMenu();
        } else if (clearConfirmOpen) {
          onCloseClearConfirm();
        } else if (deleteConfirmOpen) {
          onCloseDeleteConfirm();
        } else if (selectedIds.size > 0) {
          onClearSelection();
        }
        return;
      }

      if (e.code === 'Space') {
        e.preventDefault();
        if (quickLookItem) {
          onQuickLook(null);
        } else {
          const targetId = anchorId ?? (selectedIds.size === 1 ? Array.from(selectedIds)[0] : null);
          const target = targetId != null ? items.find((i) => i.id === targetId) : items[0];
          if (target) {
            onQuickLook(target);
          }
        }
        return;
      }

      if (quickLookItem) {
        if (e.code === 'ArrowRight' || e.code === 'ArrowDown') {
          e.preventDefault();
          const currIdx = items.findIndex((i) => i.id === quickLookItem.id);
          if (currIdx !== -1 && currIdx < items.length - 1) {
            onQuickLook(items[currIdx + 1]);
          }
        } else if (e.code === 'ArrowLeft' || e.code === 'ArrowUp') {
          e.preventDefault();
          const currIdx = items.findIndex((i) => i.id === quickLookItem.id);
          if (currIdx > 0) {
            onQuickLook(items[currIdx - 1]);
          }
        }
        return;
      }

      if (e.code === 'ArrowDown' || e.code === 'ArrowRight') {
        e.preventDefault();
        const currentTargetId =
          anchorId ?? (selectedIds.size === 1 ? Array.from(selectedIds)[0] : null);
        const currIdx =
          currentTargetId != null ? items.findIndex((i) => i.id === currentTargetId) : -1;
        if (currIdx < items.length - 1) {
          const nextItem = items[currIdx + 1];
          if (e.shiftKey) {
            onExtendSelection(nextItem.id);
          } else {
            onSelectOnly(nextItem.id);
          }
        }
      } else if (e.code === 'ArrowUp' || e.code === 'ArrowLeft') {
        e.preventDefault();
        const currentTargetId =
          anchorId ?? (selectedIds.size === 1 ? Array.from(selectedIds)[0] : null);
        const currIdx =
          currentTargetId != null ? items.findIndex((i) => i.id === currentTargetId) : items.length;
        if (currIdx > 0) {
          const prevItem = items[currIdx - 1];
          if (e.shiftKey) {
            onExtendSelection(prevItem.id);
          } else {
            onSelectOnly(prevItem.id);
          }
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    items,
    selectedIds,
    anchorId,
    quickLookItem,
    hasActiveMenu,
    clearConfirmOpen,
    deleteConfirmOpen,
    onSelectAll,
    onCopyTrigger,
    onQuickLook,
    onCloseMenu,
    onCloseClearConfirm,
    onCloseDeleteConfirm,
    onClearSelection,
    onSelectOnly,
    onExtendSelection,
  ]);
}
