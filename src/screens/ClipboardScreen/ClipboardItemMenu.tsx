import React from 'react';
import { ExternalLink, Trash2, ZoomIn, Copy as CopyIcon, X } from 'lucide-react';
import { Copy as CopyIconNode, Check as CheckIconNode, Star as StarIconNode } from 'lucide';
import { MorphIcon } from '../../components/MorphIcon/MorphIcon';
import { OverlayMenu, OverlayMenuItem } from '../../components/OverlayMenu/OverlayMenu';
import { ClipboardItem } from '../../types';
import { useApp } from '../../context/AppContext';

export interface ClipboardItemMenuProps {
  item: ClipboardItem;
  isOpen: boolean;
  onClose: () => void;
  anchorEl?: HTMLElement | null;
  virtualCoord?: { x: number; y: number } | null;
  selectedCount?: number;
  onCopy: (item: ClipboardItem) => void;
  onBulkCopy?: () => void;
  onDelete: (id: number) => void;
  onBulkDelete?: () => void;
  onTogglePin?: (item: ClipboardItem) => void;
  onQuickLook?: (item: ClipboardItem) => void;
  onOpenExternal?: (url: string) => void;
  onClearSelection?: () => void;
  copiedId?: number | null;
}

export const ClipboardItemMenu: React.FC<ClipboardItemMenuProps> = ({
  item,
  isOpen,
  onClose,
  anchorEl,
  virtualCoord,
  selectedCount = 1,
  onCopy,
  onBulkCopy,
  onDelete,
  onBulkDelete,
  onTogglePin,
  onQuickLook,
  onOpenExternal,
  onClearSelection,
  copiedId,
}) => {
  const { t } = useApp();
  const isBulk = selectedCount > 1;

  return (
    <OverlayMenu
      isOpen={isOpen}
      onClose={onClose}
      anchorEl={anchorEl}
      virtualCoord={virtualCoord}
    >
      {isBulk ? (
        <>
          {onBulkCopy && (
            <OverlayMenuItem
              icon={<CopyIcon size={14} />}
              label={`${t.bulk.copySelected} (${selectedCount})`}
              onClick={() => {
                onBulkCopy();
                onClose();
              }}
            />
          )}

          {onBulkDelete && (
            <OverlayMenuItem
              danger
              icon={<Trash2 size={14} />}
              label={`${t.bulk.deleteSelected} (${selectedCount})`}
              onClick={() => {
                onBulkDelete();
                onClose();
              }}
            />
          )}

          {onClearSelection && (
            <OverlayMenuItem
              icon={<X size={14} />}
              label={t.bulk.clearSelection}
              onClick={() => {
                onClearSelection();
                onClose();
              }}
            />
          )}
        </>
      ) : (
        <>
          <OverlayMenuItem
            icon={
              <MorphIcon
                icon={copiedId === item.id ? CheckIconNode : CopyIconNode}
                size={14}
                spring="snappy"
                color={copiedId === item.id ? 'var(--accent-text)' : 'currentColor'}
              />
            }
            label={t.clipboard.copy}
            onClick={() => {
              onCopy(item);
              onClose();
            }}
          />

          {onTogglePin && (
            <OverlayMenuItem
              icon={
                <MorphIcon
                  icon={StarIconNode}
                  size={14}
                  spring={{ stiffness: 90, damping: 12 }}
                  color={item.is_pinned ? '#F59E0B' : 'currentColor'}
                  style={{
                    fill: item.is_pinned ? '#F59E0B' : 'transparent',
                    transition: 'fill 0.45s ease, color 0.45s ease',
                  }}
                />
              }
              label={item.is_pinned ? t.common.unfavorite : t.common.favorite}
              onClick={() => {
                onTogglePin(item);
                onClose();
              }}
            />
          )}

          {(item.content_type === 'image' || item.content_type === 'screenshot') && onQuickLook && (
            <OverlayMenuItem
              icon={<ZoomIn size={14} />}
              label={t.clipboard.preview}
              onClick={() => {
                onQuickLook(item);
                onClose();
              }}
            />
          )}

          {item.content_type === 'link' && item.text_content && onOpenExternal && (
            <OverlayMenuItem
              icon={<ExternalLink size={14} />}
              label={t.clipboard.open}
              onClick={() => {
                onOpenExternal(item.text_content!);
                onClose();
              }}
            />
          )}

          <OverlayMenuItem
            danger
            icon={<Trash2 size={14} />}
            label={t.clipboard.delete}
            onClick={() => {
              onDelete(item.id);
              onClose();
            }}
          />
        </>
      )}
    </OverlayMenu>
  );
};
