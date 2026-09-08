import React, { useEffect, useState, useRef } from 'react';
import {
  MoreVertical,
  Folder,
  FileText,
  FileCode,
  FileSpreadsheet,
  Image as ImageIcon,
} from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useApp } from '../../context/AppContext';
import { api } from '../../services/api';
import { RecentFilter, RecentItem, ViewMode } from '../../types';
import { ScreenFooter } from '../../components/ScreenFooter/ScreenFooter';
import { SearchInput } from '../../components/SearchInput/SearchInput';
import { ViewSwitcher } from '../../components/ViewSwitcher/ViewSwitcher';
import { FilterTabs } from '../../components/FilterTabs/FilterTabs';
import { FilledFolderIcon } from '../../components/FilledFolderIcon/FilledFolderIcon';
import { ConfirmModal } from '../../components/ConfirmModal/ConfirmModal';
import { QuickLookModal } from '../../components/QuickLookModal/QuickLookModal';
import { ItemStarButton } from '../../components/ItemStarButton/ItemStarButton';
import { useCopyFeedback } from '../../hooks/useCopyFeedback';
import { useItemContextMenu } from '../../hooks/useItemContextMenu';
import { formatClockTime } from '../../utils/formatTime';
import { RecentItemMenu } from './RecentItemMenu';
import styles from './RecentScreen.module.css';

export const RecentScreen: React.FC = () => {
  const { t, showToast } = useApp();
  const [items, setItems] = useState<RecentItem[]>([]);
  const [filter, setFilter] = useState<RecentFilter>('all');
  const [viewMode, setViewMode] = useState<ViewMode>('details');
  const [search, setSearch] = useState('');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const { activeMenu, openContextMenu, openAnchorMenu, closeMenu } =
    useItemContextMenu<RecentItem>();
  const [quickLookItem, setQuickLookItem] = useState<RecentItem | null>(null);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [copiedPath, triggerCopy] = useCopyFeedback<string>(1300);
  const [animatingPinId, setAnimatingPinId] = useState<number | null>(null);
  const reqIdRef = useRef(0);

  // Load items with race condition protection
  const loadItems = () => {
    const currentId = ++reqIdRef.current;
    api
      .getRecentItems(filter, search)
      .then((data) => {
        if (currentId === reqIdRef.current) {
          setItems(data);
        }
      })
      .catch(console.error);
  };

  const loadItemsRef = useRef(loadItems);
  loadItemsRef.current = loadItems;

  useEffect(() => {
    loadItems();
  }, [filter, search]);

  useEffect(() => {
    api
      .getViewMode(`recent_${filter}`)
      .then((mode) => {
        if (mode === 'grid') {
          setViewMode('grid');
        } else {
          setViewMode('details');
        }
      })
      .catch(() => setViewMode('details'));
  }, [filter]);

  const handleSetViewMode = (mode: 'details' | 'grid') => {
    setViewMode(mode);
    api.setViewMode(`recent_${filter}`, mode).catch(console.error);
  };

  useEffect(() => {
    const unlisten = listen<RecentItem>('recent-updated', () => {
      loadItemsRef.current();
    });

    return () => {
      unlisten.then((u) => u());
    };
  }, []);

  // Keyboard navigation & Quick Look
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        e.code === 'Space' &&
        !['INPUT', 'TEXTAREA'].includes((e.target as HTMLElement)?.tagName)
      ) {
        e.preventDefault();
        if (quickLookItem) {
          setQuickLookItem(null);
        } else if (selectedId != null) {
          const sel = items.find((i) => i.id === selectedId);
          if (sel && isImageFile(sel.path)) {
            setQuickLookItem(sel);
          }
        }
        return;
      }

      if (e.code === 'Escape') {
        if (quickLookItem) {
          setQuickLookItem(null);
        } else if (activeMenu) {
          closeMenu();
        } else if (clearConfirmOpen) {
          setClearConfirmOpen(false);
        } else if (selectedId != null) {
          setSelectedId(null);
        }
        return;
      }

      if (quickLookItem && isImageFile(quickLookItem.path)) {
        const imageItems = items.filter((i) => isImageFile(i.path));
        const currIdx = imageItems.findIndex((i) => i.id === quickLookItem.id);
        if (currIdx !== -1) {
          if (e.code === 'ArrowRight' || e.code === 'ArrowDown') {
            e.preventDefault();
            if (currIdx < imageItems.length - 1) {
              setQuickLookItem(imageItems[currIdx + 1]);
              setSelectedId(imageItems[currIdx + 1].id);
            }
          } else if (e.code === 'ArrowLeft' || e.code === 'ArrowUp') {
            e.preventDefault();
            if (currIdx > 0) {
              setQuickLookItem(imageItems[currIdx - 1]);
              setSelectedId(imageItems[currIdx - 1].id);
            }
          }
        }
        return;
      }

      if (['INPUT', 'TEXTAREA'].includes((e.target as HTMLElement)?.tagName)) return;

      if (e.code === 'ArrowDown' || e.code === 'ArrowRight') {
        e.preventDefault();
        const currIdx = selectedId != null ? items.findIndex((i) => i.id === selectedId) : -1;
        if (currIdx < items.length - 1) {
          setSelectedId(items[currIdx + 1].id);
        }
      } else if (e.code === 'ArrowUp' || e.code === 'ArrowLeft') {
        e.preventDefault();
        const currIdx = selectedId != null ? items.findIndex((i) => i.id === selectedId) : items.length;
        if (currIdx > 0) {
          setSelectedId(items[currIdx - 1].id);
        }
      } else if (e.code === 'Enter' && selectedId != null) {
        e.preventDefault();
        const sel = items.find((i) => i.id === selectedId);
        if (sel) handleOpen(sel);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [items, selectedId, quickLookItem, activeMenu, clearConfirmOpen]);

  const handleOpen = async (item: RecentItem) => {
    try {
      await api.openRecentItem(item.path);
    } catch {
      showToast(t.recent.pathNotFound);
    }
  };

  const handleCopyPath = async (pathStr: string) => {
    try {
      await api.copyText(pathStr);
      showToast(t.common.copied);
      triggerCopy(pathStr);
    } catch (err) {
      console.error(err);
    }
  };

  const handleTogglePin = async (item: RecentItem) => {
    try {
      setAnimatingPinId(item.id);
      setTimeout(() => {
        setAnimatingPinId((curr) => (curr === item.id ? null : curr));
      }, 500);

      const newPinned = await api.togglePinRecent(item.id);
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
  };

  const handleDelete = async (id: number) => {
    try {
      await api.deleteRecentItem(id);
      setItems((prev) => prev.filter((i) => i.id !== id));
      closeMenu();
      if (selectedId === id) setSelectedId(null);
      if (quickLookItem?.id === id) setQuickLookItem(null);
    } catch (err) {
      console.error(err);
    }
  };

  const handleClearHistory = async () => {
    try {
      await api.clearRecentHistory();
      setItems((prev) => prev.filter((i) => i.is_pinned));
      setClearConfirmOpen(false);
      setSelectedId(null);
      setQuickLookItem(null);
      showToast(t.recent.empty);
    } catch (err) {
      console.error(err);
    }
  };

  const isImageFile = (path: string) => {
    const ext = path.split('.').pop()?.toLowerCase() || '';
    return ['png', 'jpg', 'jpeg', 'webp', 'svg'].includes(ext);
  };

  const renderIcon = (item: RecentItem, size: number = 16) => {
    if (item.thumbnail_b64) {
      return (
        <img
          src={item.thumbnail_b64}
          alt=""
          className={styles.thumbImg}
          style={{ width: size, height: size }}
        />
      );
    }
    if (item.entry_type === 'folder') {
      return <Folder size={size} color="#F59E0B" fill="#F59E0B" />;
    }
    const ext = item.path.split('.').pop()?.toLowerCase() || '';
    if (['doc', 'docx', 'txt', 'rtf'].includes(ext)) {
      return <FileText size={size} color="#2563EB" />;
    }
    if (['xls', 'xlsx', 'csv'].includes(ext)) {
      return <FileSpreadsheet size={size} color="#16A34A" />;
    }
    if (['png', 'jpg', 'jpeg', 'webp', 'svg'].includes(ext)) {
      return <ImageIcon size={size} color="#10B981" />;
    }
    if (['pdf'].includes(ext)) {
      return <FileText size={size} color="#DC2626" />;
    }
    return <FileCode size={size} color="#6B7280" />;
  };

  return (
    <div className={styles.container}>
      {/* Top Bar with Shared SearchInput and ViewSwitcher */}
      <div className={styles.topBar}>
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder={t.recent.searchPlaceholder}
        />
        <ViewSwitcher
          viewMode={viewMode === 'grid' ? 'grid' : 'details'}
          onChange={handleSetViewMode}
        />
      </div>

      {/* Filter Tabs with Shared FilterTabs (No full-width horizontal divider) */}
      <FilterTabs
        tabs={[
          { id: 'all', label: t.recent.all },
          { id: 'files', label: t.recent.files },
          { id: 'folders', label: t.recent.folders },
          { id: 'favorites', label: t.recent.favorites },
        ]}
        activeTab={filter}
        onChange={setFilter}
      />

      {/* Content Area: Strictly Table or Grid */}
      <div className={styles.contentArea}>
        {items.length === 0 ? (
          <div className={styles.emptyState}>
            <Folder size={32} />
            <span>{t.recent.empty}</span>
          </div>
        ) : viewMode === 'grid' ? (
          /* Grid View */
          <div className={styles.gridWrapper}>
            {items.map((item) => {
              const isImg = isImageFile(item.path);
              const isSelected = selectedId === item.id;
              return (
                <div
                  key={item.id}
                  className={`${styles.gridCard} ${isSelected ? styles.selected : ''}`}
                  onClick={() => setSelectedId(item.id)}
                  onDoubleClick={() => handleOpen(item)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setSelectedId(item.id);
                    openContextMenu(item, { x: e.clientX, y: e.clientY });
                  }}
                >
                  <div className={styles.gridIconArea}>
                    {item.entry_type === 'folder' ? (
                      <div className={styles.gridFolderIconBox}>
                        <FilledFolderIcon size={50} />
                      </div>
                    ) : isImg ? (
                      <img
                        src={item.thumbnail_b64 || convertFileSrc(item.path)}
                        alt=""
                        className={styles.gridImageCover}
                      />
                    ) : (
                      <div className={styles.gridDocIconBox}>
                        {item.thumbnail_b64 ? (
                          <img src={item.thumbnail_b64} alt="" className={styles.gridDocThumbImg} />
                        ) : (
                          renderIcon(item, 36)
                        )}
                      </div>
                    )}
                    <div className={styles.gridCardActions}>
                      <ItemStarButton
                        className={styles.gridActionBtn}
                        isPinned={item.is_pinned}
                        isAnimating={animatingPinId === item.id}
                        onToggle={() => handleTogglePin(item)}
                        size={11}
                        title={item.is_pinned ? t.common.unfavorite : t.common.favorite}
                      />
                      <button
                        className={styles.gridActionBtn}
                        onClick={(e) => {
                          e.stopPropagation();
                          setSelectedId(item.id);
                          if (activeMenu?.item.id === item.id && activeMenu.anchorEl) {
                            closeMenu();
                          } else {
                            openAnchorMenu(item, e.currentTarget as HTMLElement);
                          }
                        }}
                        title={t.common.options}
                        aria-label={t.common.options}
                      >
                        <MoreVertical size={11} />
                      </button>
                    </div>
                  </div>
                  <div className={styles.gridInfo}>
                    <div className={styles.gridTitle} title={item.title}>
                      {item.title}
                    </div>
                    <div className={styles.gridPath} title={item.path}>
                      {item.path}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          /* Table / Details View (Primary Dense View) */
          <div className={styles.tableContainer}>
            <table className={styles.table}>
              <thead>
                <tr>
                  <th style={{ width: '40px' }}></th>
                  <th style={{ width: '220px' }}>{t.recent.name}</th>
                  <th>{t.recent.path}</th>
                  <th style={{ width: '90px' }}>{t.recent.time}</th>
                  <th style={{ width: '72px' }}></th>
                </tr>
              </thead>
              <tbody>
                {items.map((item) => {
                  const isSelected = selectedId === item.id;
                  return (
                    <tr
                      key={item.id}
                      className={isSelected ? styles.selected : ''}
                      onClick={() => setSelectedId(item.id)}
                      onDoubleClick={() => handleOpen(item)}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setSelectedId(item.id);
                        openContextMenu(item, { x: e.clientX, y: e.clientY });
                      }}
                    >
                      <td style={{ textAlign: 'center' }}>{renderIcon(item, 18)}</td>
                      <td className={styles.tableNameCell}>
                        <span className={styles.tableNameText} title={item.title}>
                          {item.title}
                        </span>
                      </td>
                      <td className={styles.tablePathCell}>
                        <span className={styles.tablePathText} title={item.path}>
                          {item.path}
                        </span>
                      </td>
                      <td style={{ color: 'var(--text-muted)', fontSize: '12.5px' }}>
                        {formatClockTime(item.last_accessed_at)}
                      </td>
                      <td>
                        <div className={styles.tableActionCell}>
                          <ItemStarButton
                            className={styles.menuBtn}
                            isPinned={item.is_pinned}
                            isAnimating={animatingPinId === item.id}
                            onToggle={() => handleTogglePin(item)}
                            size={14}
                            title={item.is_pinned ? t.common.unfavorite : t.common.favorite}
                          />
                          <button
                            className={styles.menuBtn}
                            onClick={(e) => {
                              e.stopPropagation();
                              setSelectedId(item.id);
                              if (activeMenu?.item.id === item.id && activeMenu.anchorEl) {
                                closeMenu();
                              } else {
                                openAnchorMenu(item, e.currentTarget as HTMLElement);
                              }
                            }}
                            title={t.common.options}
                            aria-label={t.common.options}
                          >
                            <MoreVertical size={16} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Bottom Bar */}
      <ScreenFooter
        count={items.length}
        onClear={() => setClearConfirmOpen(true)}
        clearLabel={t.recent.clearHistory}
      />

      {/* Quick Look Modal for Recent Images */}
      <QuickLookModal
        isOpen={!!quickLookItem}
        title={quickLookItem?.title || ''}
        onClose={() => setQuickLookItem(null)}
        imageSrc={
          (quickLookItem ? convertFileSrc(quickLookItem.path) : '') ||
          quickLookItem?.thumbnail_b64 ||
          ''
        }
        footerInfo={quickLookItem?.path}
        actionButton={
          <button
            type="button"
            className={styles.btnPrimary}
            onClick={() => quickLookItem && handleOpen(quickLookItem)}
          >
            {t.recent.openFile}
          </button>
        }
        closeAriaLabel={t.common.close}
      />

      {/* Clear Confirmation Modal */}
      <ConfirmModal
        isOpen={clearConfirmOpen}
        title={t.recent.clearHistory}
        message={t.recent.clearConfirm}
        confirmText={t.common.confirm}
        cancelText={t.common.cancel}
        danger
        onConfirm={handleClearHistory}
        onCancel={() => setClearConfirmOpen(false)}
      />

      {/* Shared Floating Recent Item Menu */}
      {activeMenu && (
        <RecentItemMenu
          item={activeMenu.item}
          isOpen={true}
          onClose={closeMenu}
          anchorEl={activeMenu.anchorEl}
          virtualCoord={activeMenu.virtualCoord}
          onOpen={handleOpen}
          onCopyPath={handleCopyPath}
          onDelete={handleDelete}
          onTogglePin={handleTogglePin}
          copiedPath={copiedPath}
        />
      )}
    </div>
  );
};
