import React from 'react';
import { FolderOpen, FileText, Trash2 } from 'lucide-react';
import { Copy as CopyIconNode, Check as CheckIconNode, Star as StarIconNode } from 'lucide';
import { MorphIcon } from '../../components/MorphIcon/MorphIcon';
import { OverlayMenu, OverlayMenuItem } from '../../components/OverlayMenu/OverlayMenu';
import { RecentItem } from '../../types';
import { useApp } from '../../context/AppContext';

export interface RecentItemMenuProps {
  item: RecentItem;
  isOpen: boolean;
  onClose: () => void;
  anchorEl?: HTMLElement | null;
  virtualCoord?: { x: number; y: number } | null;
  onOpen: (item: RecentItem) => void;
  onCopyPath: (path: string) => void;
  onDelete: (id: number) => void;
  onTogglePin?: (item: RecentItem) => void;
  copiedPath?: string | null;
}

export const RecentItemMenu: React.FC<RecentItemMenuProps> = ({
  item,
  isOpen,
  onClose,
  anchorEl,
  virtualCoord,
  onOpen,
  onCopyPath,
  onDelete,
  onTogglePin,
  copiedPath,
}) => {
  const { t } = useApp();

  return (
    <OverlayMenu
      isOpen={isOpen}
      onClose={onClose}
      anchorEl={anchorEl}
      virtualCoord={virtualCoord}
    >
      <OverlayMenuItem
        icon={item.entry_type === 'folder' ? <FolderOpen size={14} /> : <FileText size={14} />}
        label={item.entry_type === 'folder' ? t.recent.openFolder : t.recent.openFile}
        onClick={() => {
          onOpen(item);
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

      <OverlayMenuItem
        icon={
          <MorphIcon
            icon={copiedPath === item.path ? CheckIconNode : CopyIconNode}
            size={14}
            spring="snappy"
            color="currentColor"
          />
        }
        label={t.recent.copyPath}
        onClick={() => {
          onCopyPath(item.path);
        }}
      />

      <OverlayMenuItem
        danger
        icon={<Trash2 size={14} />}
        label={t.recent.removeStale}
        onClick={() => {
          onDelete(item.id);
          onClose();
        }}
      />
    </OverlayMenu>
  );
};
