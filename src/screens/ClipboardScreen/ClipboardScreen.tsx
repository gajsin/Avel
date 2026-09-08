import React, { useEffect, useState, useRef, useCallback } from 'react';
import { FileText } from 'lucide-react';
import { Copy as CopyIconNode, Check as CheckIconNode } from 'lucide';
import { MorphIcon } from '../../components/MorphIcon/MorphIcon';
import { listen } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useApp } from '../../context/AppContext';
import { api } from '../../services/api';
import { ClipboardFilter, ClipboardItem, ViewMode } from '../../types';
import { ScreenFooter } from '../../components/ScreenFooter/ScreenFooter';
import { SearchInput } from '../../components/SearchInput/SearchInput';
import { ViewSwitcher } from '../../components/ViewSwitcher/ViewSwitcher';
import { FilterTabs } from '../../components/FilterTabs/FilterTabs';
import { ConfirmModal } from '../../components/ConfirmModal/ConfirmModal';
import { QuickLookModal } from '../../components/QuickLookModal/QuickLookModal';
import { useCopyFeedback } from '../../hooks/useCopyFeedback';
import { useItemContextMenu } from '../../hooks/useItemContextMenu';
import { splitSelectedClipboardItems } from '../../utils/clipboard';
import { formatRelativeTime as formatRelativeTimeUtil } from '../../utils/formatTime';
import { ClipboardItemMenu } from './ClipboardItemMenu';
import { BulkActionBar } from './BulkActionBar';
import { ClipboardTableView } from './ClipboardTableView';
import { ClipboardGridView } from './ClipboardGridView';
import { useClipboardKeyboardNav } from './useClipboardKeyboardNav';
import styles from './ClipboardScreen.module.css';

const INITIAL_VISIBLE_COUNT = 36;

export const ClipboardScreen: React.FC = () => {
  const { t, showToast } = useApp();
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [filter, setFilter] = useState<ClipboardFilter>('all');
  const [viewMode, setViewMode] = useState<ViewMode>('details');
  const [search, setSearch] = useState('');
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [anchorId, setAnchorId] = useState<number | null>(null);
  const { activeMenu, setActiveMenu, openContextMenu, closeMenu } =
    useItemContextMenu<ClipboardItem>();
  const [quickLookItem, setQuickLookItem] = useState<ClipboardItem | null>(null);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [copiedId, triggerCopy] = useCopyFeedback<number>(1300);
  const [animatingPinId, setAnimatingPinId] = useState<number | null>(null);
  const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_COUNT);
  const reqIdRef = useRef(0);

  useEffect(() => {
    setVisibleCount(INITIAL_VISIBLE_COUNT);
  }, [filter, search, viewMode]);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 300) {
      setVisibleCount((prev) => Math.min(prev + 24, items.length));
    }
  };

  const loadItems = useCallback(() => {
    const currentId = ++reqIdRef.current;
    api
      .getClipboardItems(filter, search)
      .then((data) => {
        if (currentId === reqIdRef.current) {
          setItems(data);
          setVisibleCount(INITIAL_VISIBLE_COUNT);
        }
      })
      .catch(console.error);
  }, [filter, search]);

  const loadItemsRef = useRef(loadItems);
  loadItemsRef.current = loadItems;

  useEffect(() => {
    loadItems();
    setSelectedIds(new Set());
    setAnchorId(null);
  }, [loadItems]);

  useEffect(() => {
    api
      .getViewMode(`clipboard_${filter}`)
      .then((mode) => {
        setViewMode(mode === 'grid' ? 'grid' : 'details');
      })
      .catch(() => setViewMode('details'));
  }, [filter]);

  const handleSetViewMode = (mode: ViewMode) => {
    const targetMode = mode === 'grid' ? 'grid' : 'details';
    setViewMode(targetMode);
    api.setViewMode(`clipboard_${filter}`, targetMode).catch(console.error);
  };

  useEffect(() => {
    const unlisten = listen<ClipboardItem>('clipboard-updated', () => {
      loadItemsRef.current();
    });

    return () => {
      unlisten.then((u) => u());
    };
  }, []);

  // Selection helpers
  const selectOnly = useCallback((id: number) => {
    setSelectedIds(new Set([id]));
    setAnchorId(id);
  }, []);

  const toggleSelection = useCallback((id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    setAnchorId(id);
  }, []);

  const selectRange = useCallback(
    (id: number) => {
      if (anchorId == null) {
        selectOnly(id);
        return;
      }
      const idxA = items.findIndex((i) => i.id === anchorId);
      const idxB = items.findIndex((i) => i.id === id);
      if (idxA !== -1 && idxB !== -1) {
        const start = Math.min(idxA, idxB);
        const end = Math.max(idxA, idxB);
        const next = new Set(selectedIds);
        for (let i = start; i <= end; i++) {
          next.add(items[i].id);
        }
        setSelectedIds(next);
      } else {
        selectOnly(id);
      }
    },
    [anchorId, items, selectOnly, selectedIds]
  );

  const selectAllFiltered = useCallback(() => {
    setSelectedIds(new Set(items.map((i) => i.id)));
  }, [items]);

  const clearSelection = useCallback(() => {
    setSelectedIds(new Set());
    setAnchorId(null);
  }, []);

  const extendSelection = useCallback(
    (id: number) => {
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.add(id);
        return next;
      });
      setAnchorId(id);
    },
    []
  );

  const handleCopy = useCallback(
    async (item: ClipboardItem) => {
      try {
        if (
          (item.content_type === 'image' || item.content_type === 'screenshot') &&
          item.image_path
        ) {
          await api.copyImage(item.image_path);
        } else if (item.text_content) {
          await api.copyText(item.text_content);
        }
        showToast(t.common.copied);
        triggerCopy(item.id);
      } catch (err) {
        console.error(err);
      }
    },
    [showToast, t.common.copied, triggerCopy]
  );

  const handleTogglePin = useCallback(
    async (item: ClipboardItem) => {
      try {
        setAnimatingPinId(item.id);
        setTimeout(() => {
          setAnimatingPinId((curr) => (curr === item.id ? null : curr));
        }, 500);

        const newPinned = await api.togglePinClipboard(item.id);
        setItems((prev) =>
          prev.map((i) => (i.id === item.id ? { ...i, is_pinned: newPinned } : i))
        );
        if (filter === 'favorites' && !newPinned) {
          setItems((prev) => prev.filter((i) => i.id !== item.id));
        }
        showToast(newPinned ? t.common.addedToFavorites : t.common.removedFromFavorites);
      } catch (err) {
        console.error('Failed to toggle pin:', err);
      }
    },
    [filter, showToast, t.common.addedToFavorites, t.common.removedFromFavorites]
  );

  const handleBulkCopyImages = useCallback(
    async (imageItems: ClipboardItem[]) => {
      try {
        const paths = imageItems.map((i) => i.image_path!).filter(Boolean);
        if (paths.length > 0) {
          await api.copyFiles(paths);
          showToast(`${t.bulk.imagesCopied}: ${paths.length}`);
        }
      } catch (err) {
        console.error(err);
      }
    },
    [showToast, t.bulk.imagesCopied]
  );

  const handleBulkCopyTexts = useCallback(
    async (textItems: ClipboardItem[]) => {
      try {
        const joined = textItems.map((i) => i.text_content).filter(Boolean).join('\n\n');
        if (joined) {
          await api.copyText(joined);
          showToast(`${t.bulk.itemsCopied}: ${textItems.length}`);
        }
      } catch (err) {
        console.error(err);
      }
    },
    [showToast, t.bulk.itemsCopied]
  );

  const dispatchBulkCopy = useCallback(
    (selectedList: ClipboardItem[]) => {
      const { imageItems, textItems } = splitSelectedClipboardItems(selectedList);
      if (imageItems.length > 0) {
        handleBulkCopyImages(imageItems);
      } else if (textItems.length > 0) {
        handleBulkCopyTexts(textItems);
      }
    },
    [handleBulkCopyImages, handleBulkCopyTexts]
  );

  const handleDelete = useCallback(
    async (id: number) => {
      try {
        await api.deleteClipboardItem(id);
        setItems((prev) => prev.filter((i) => i.id !== id));
        setSelectedIds((prev) => {
          const next = new Set(prev);
          next.delete(id);
          return next;
        });
        closeMenu();
        if (quickLookItem?.id === id) {
          setQuickLookItem(null);
        }
      } catch (err) {
        console.error(err);
      }
    },
    [closeMenu, quickLookItem?.id]
  );

  // Single atomic batch delete replacing N+1 IPC calls
  const handleBulkDelete = useCallback(async () => {
    try {
      const idsToDelete = Array.from(selectedIds);
      if (idsToDelete.length === 0) return;

      await api.deleteClipboardItems(idsToDelete);
      setItems((prev) => prev.filter((i) => !selectedIds.has(i.id)));
      clearSelection();
      setActiveMenu(null);
      showToast(`${t.bulk.itemsDeleted}: ${idsToDelete.length}`);
    } catch (err) {
      console.error(err);
    }
  }, [clearSelection, selectedIds, setActiveMenu, showToast, t.bulk.itemsDeleted]);

  const handleClearHistory = useCallback(async () => {
    try {
      await api.clearClipboardHistory();
      setItems((prev) => prev.filter((i) => i.is_pinned));
      setClearConfirmOpen(false);
      closeMenu();
      clearSelection();
      showToast(t.clipboard.empty);
    } catch (err) {
      console.error(err);
    }
  }, [clearSelection, closeMenu, showToast, t.clipboard.empty]);

  // Integrated Keyboard Navigation via Custom Hook
  const handleCopyTrigger = useCallback(() => {
    if (selectedIds.size > 0) {
      dispatchBulkCopy(items.filter((i) => selectedIds.has(i.id)));
    } else if (anchorId != null) {
      const target = items.find((i) => i.id === anchorId);
      if (target) handleCopy(target);
    } else if (items.length > 0) {
      handleCopy(items[0]);
    }
  }, [anchorId, dispatchBulkCopy, handleCopy, items, selectedIds]);

  useClipboardKeyboardNav({
    items,
    selectedIds,
    anchorId,
    quickLookItem,
    hasActiveMenu: Boolean(activeMenu),
    clearConfirmOpen,
    deleteConfirmOpen,
    onSelectAll: selectAllFiltered,
    onCopyTrigger: handleCopyTrigger,
    onQuickLook: setQuickLookItem,
    onCloseMenu: closeMenu,
    onCloseClearConfirm: () => setClearConfirmOpen(false),
    onCloseDeleteConfirm: () => setDeleteConfirmOpen(false),
    onClearSelection: clearSelection,
    onSelectOnly: selectOnly,
    onExtendSelection: extendSelection,
  });

  // Formatting helpers
  const formatRelativeTime = useCallback(
    (isoString: string) =>
      formatRelativeTimeUtil(isoString, {
        justNow: t.common.justNow,
        minAgo: t.common.minAgo,
        hoursAgo: t.common.hoursAgo,
      }),
    [t.common.hoursAgo, t.common.justNow, t.common.minAgo]
  );

  const formatCardTime = useCallback((isoString: string) => {
    return new Date(isoString).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
    });
  }, []);

  const getItemDisplayName = useCallback(
    (item: ClipboardItem): string => {
      if (item.content_type === 'text' || item.content_type === 'link') {
        return item.text_content || t.clipboard.text;
      }

      const rawFilename = item.image_path ? item.image_path.split(/[\\/]/).pop() || '' : '';
      const isHashName = /^[a-f0-9]{32,}\./i.test(rawFilename) || rawFilename.length > 32;

      if (item.content_type === 'screenshot' || item.content_type === 'image') {
        if (
          rawFilename &&
          !isHashName &&
          !rawFilename.toLowerCase().includes('screenshot.png') &&
          !rawFilename.toLowerCase().includes('image.png')
        ) {
          return rawFilename.replace(/\.[^/.]+$/, '');
        }
        return item.content_type === 'screenshot' ? t.clipboard.screenshot : t.clipboard.image;
      }

      return item.text_content || 'Item';
    },
    [t.clipboard.image, t.clipboard.screenshot, t.clipboard.text]
  );

  const renderItemVisual = useCallback((item: ClipboardItem, size: number = 38) => {
    const isImg = item.content_type === 'image' || item.content_type === 'screenshot';
    if (isImg && (item.thumbnail_b64 || item.image_path)) {
      const src = item.thumbnail_b64 || (item.image_path ? convertFileSrc(item.image_path) : '');
      const isScreenshot = item.content_type === 'screenshot';
      return (
        <div className={styles.thumbBox} style={{ width: size, height: size }}>
          <img
            src={src}
            alt=""
            loading="lazy"
            decoding="async"
            className={isScreenshot ? styles.containImg : styles.coverImg}
          />
        </div>
      );
    }

    switch (item.content_type) {
      case 'link':
        return (
          <div className={`${styles.typeBadge} ${styles.badgeLink}`} style={{ width: size, height: size }}>
            <span style={{ fontSize: 13, fontWeight: 700 }}>🔗</span>
          </div>
        );
      default:
        return (
          <div className={`${styles.typeBadge} ${styles.badgeText}`} style={{ width: size, height: size }}>
            <span style={{ fontFamily: 'var(--font-sans)', fontWeight: 600, fontSize: 14 }}>T</span>
          </div>
        );
    }
  }, []);

  const renderSubtitle = useCallback(
    (item: ClipboardItem, compact: boolean = false) => {
      if (item.content_type === 'screenshot' || item.content_type === 'image') {
        const dim = item.width && item.height ? ` · ${item.width}×${item.height}` : '';
        if (compact) {
          const label = item.content_type === 'screenshot' ? t.clipboard.screenshot : t.clipboard.image;
          return `${label}${dim}`;
        }
        const source = item.content_type === 'screenshot' ? t.clipboard.screenshot : t.clipboard.fromClipboard;
        return `${t.clipboard.image} · ${source}${dim}`;
      }
      if (item.content_type === 'link') {
        return `${t.clipboard.link} · ${t.clipboard.web}`;
      }
      if (item.content_type === 'code') {
        return t.clipboard.code;
      }
      const count = item.char_count || item.text_content?.length || 0;
      return `${t.clipboard.text} · ${count} ${t.clipboard.chars}`;
    },
    [t.clipboard]
  );

  // Item interaction handlers
  const handleItemClick = useCallback(
    (e: React.MouseEvent, item: ClipboardItem) => {
      if (e.ctrlKey || e.metaKey) {
        toggleSelection(item.id);
      } else if (e.shiftKey && anchorId != null) {
        selectRange(item.id);
      } else {
        handleCopy(item);
      }
    },
    [anchorId, handleCopy, selectRange, toggleSelection]
  );

  const handleItemDoubleClick = useCallback((e: React.MouseEvent, item: ClipboardItem) => {
    e.stopPropagation();
    setQuickLookItem(item);
  }, []);

  const handleItemContextMenu = useCallback(
    (e: React.MouseEvent, item: ClipboardItem) => {
      e.preventDefault();
      openContextMenu(item, { x: e.clientX, y: e.clientY });
    },
    [openContextMenu]
  );

  const handleCheckboxClick = useCallback(
    (e: React.MouseEvent, id: number) => {
      e.stopPropagation();
      toggleSelection(id);
    },
    [toggleSelection]
  );

  const handleMenuBtnClick = useCallback(
    (e: React.MouseEvent, item: ClipboardItem) => {
      e.stopPropagation();
      const btn = e.currentTarget as HTMLElement;
      const rect = btn.getBoundingClientRect();
      if (activeMenu?.item.id === item.id && activeMenu.anchorEl) {
        closeMenu();
      } else {
        setActiveMenu({
          item,
          anchorEl: btn,
          virtualCoord: { x: rect.right, y: rect.bottom },
        });
      }
    },
    [activeMenu?.anchorEl, activeMenu?.item.id, closeMenu, setActiveMenu]
  );

  const handleHeaderCheckboxClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      const allFilteredSelected = items.length > 0 && items.every((i) => selectedIds.has(i.id));
      if (allFilteredSelected) {
        clearSelection();
      } else {
        selectAllFiltered();
      }
    },
    [clearSelection, items, selectAllFiltered, selectedIds]
  );

  const visibleItems = items.slice(0, visibleCount);

  return (
    <div className={styles.container}>
      {/* Top Bar with Shared SearchInput and ViewSwitcher */}
      <div className={styles.topBar}>
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder={t.clipboard.searchPlaceholder}
        />
        <ViewSwitcher
          viewMode={viewMode === 'grid' ? 'grid' : 'details'}
          onChange={handleSetViewMode}
        />
      </div>

      {/* Filter Tabs */}
      <FilterTabs
        tabs={[
          { id: 'all', label: t.clipboard.all },
          { id: 'text', label: t.clipboard.text },
          { id: 'images', label: t.clipboard.images },
          { id: 'favorites', label: t.clipboard.favorites },
        ]}
        activeTab={filter}
        onChange={setFilter}
      />

      <div className={styles.contentArea}>
        <div
          className={`${styles.listWrapper} ${viewMode === 'grid' ? styles.gridListWrapper : ''} ${
            selectedIds.size > 0 ? styles.hasBulkBar : ''
          }`}
          onScroll={handleScroll}
        >
          {items.length === 0 ? (
            <div className={styles.emptyState}>
              <FileText size={32} />
              <span>{t.clipboard.empty}</span>
            </div>
          ) : viewMode === 'grid' ? (
            <ClipboardGridView
              items={visibleItems}
              selectedIds={selectedIds}
              copiedId={copiedId}
              animatingPinId={animatingPinId}
              activeMenuId={activeMenu?.item.id}
              onItemClick={handleItemClick}
              onItemDoubleClick={handleItemDoubleClick}
              onItemContextMenu={handleItemContextMenu}
              onCheckboxClick={handleCheckboxClick}
              onTogglePin={handleTogglePin}
              onCopy={handleCopy}
              onMenuBtnClick={handleMenuBtnClick}
              formatCardTime={formatCardTime}
            />
          ) : (
            <ClipboardTableView
              items={visibleItems}
              selectedIds={selectedIds}
              copiedId={copiedId}
              animatingPinId={animatingPinId}
              activeMenuId={activeMenu?.item.id}
              onItemClick={handleItemClick}
              onItemDoubleClick={handleItemDoubleClick}
              onItemContextMenu={handleItemContextMenu}
              onCheckboxClick={handleCheckboxClick}
              onHeaderCheckboxClick={handleHeaderCheckboxClick}
              onTogglePin={handleTogglePin}
              onCopy={handleCopy}
              onMenuBtnClick={handleMenuBtnClick}
              getItemDisplayName={getItemDisplayName}
              renderItemVisual={renderItemVisual}
              renderSubtitle={renderSubtitle}
              formatTime={formatRelativeTime}
            />
          )}
        </div>
      </div>

      <ScreenFooter
        count={items.length}
        onClear={() => setClearConfirmOpen(true)}
        clearLabel={t.clipboard.clearHistory}
      />

      {/* Quick Look Modal */}
      <QuickLookModal
        isOpen={!!quickLookItem}
        title={quickLookItem ? getItemDisplayName(quickLookItem) : ''}
        onClose={() => setQuickLookItem(null)}
        imageSrc={
          quickLookItem && (quickLookItem.thumbnail_b64 || quickLookItem.image_path)
            ? (quickLookItem.image_path ? convertFileSrc(quickLookItem.image_path) : '') ||
              quickLookItem.thumbnail_b64 ||
              ''
            : null
        }
        textContent={
          quickLookItem && !quickLookItem.thumbnail_b64 && !quickLookItem.image_path
            ? quickLookItem.text_content
            : null
        }
        footerInfo={
          quickLookItem
            ? `${renderSubtitle(quickLookItem)} • ${formatRelativeTime(quickLookItem.updated_at)}`
            : null
        }
        actionButton={
          quickLookItem && (
            <button
              type="button"
              className={styles.btnPrimary}
              onClick={() => {
                handleCopy(quickLookItem);
                setQuickLookItem(null);
              }}
            >
              <MorphIcon
                icon={copiedId === quickLookItem.id ? CheckIconNode : CopyIconNode}
                size={14}
                spring="snappy"
                color="currentColor"
              />
              <span>{t.clipboard.copy}</span>
            </button>
          )
        }
        closeAriaLabel={t.common.close}
      />

      {/* Clear Confirmation Modal */}
      <ConfirmModal
        isOpen={clearConfirmOpen}
        title={t.clipboard.clearHistory}
        message={t.clipboard.clearConfirm}
        confirmText={t.common.confirm}
        cancelText={t.common.cancel}
        danger
        onConfirm={handleClearHistory}
        onCancel={() => setClearConfirmOpen(false)}
      />

      {/* Bulk Delete Confirmation Modal */}
      <ConfirmModal
        isOpen={deleteConfirmOpen}
        title={t.bulk.deleteModalTitle}
        message={`${t.bulk.deleteModalConfirm} (${selectedIds.size})`}
        confirmText={t.common.confirm}
        cancelText={t.common.cancel}
        danger
        onConfirm={() => {
          setDeleteConfirmOpen(false);
          handleBulkDelete();
        }}
        onCancel={() => setDeleteConfirmOpen(false)}
      />

      {/* Floating Bulk Action Bar */}
      {selectedIds.size > 0 && (
        <BulkActionBar
          selectedItems={items.filter((i) => selectedIds.has(i.id))}
          onClearSelection={clearSelection}
          onCopyImages={handleBulkCopyImages}
          onCopyTexts={handleBulkCopyTexts}
          onDeleteSelected={() => setDeleteConfirmOpen(true)}
        />
      )}

      {/* Floating Clipboard Item Menu */}
      {activeMenu && (
        <ClipboardItemMenu
          item={activeMenu.item}
          isOpen={true}
          onClose={closeMenu}
          anchorEl={activeMenu.anchorEl}
          virtualCoord={activeMenu.virtualCoord}
          selectedCount={selectedIds.size}
          onCopy={handleCopy}
          onBulkCopy={
            selectedIds.size > 1
              ? () => dispatchBulkCopy(items.filter((i) => selectedIds.has(i.id)))
              : undefined
          }
          onDelete={handleDelete}
          onBulkDelete={selectedIds.size > 1 ? () => setDeleteConfirmOpen(true) : undefined}
          onQuickLook={setQuickLookItem}
          onTogglePin={handleTogglePin}
          onOpenExternal={(url) => api.openExternal(url)}
          onClearSelection={clearSelection}
          copiedId={copiedId}
        />
      )}
    </div>
  );
};