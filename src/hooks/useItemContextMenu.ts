import { useState, useCallback } from 'react';

export interface ActiveContextMenu<T> {
  item: T;
  anchorEl?: HTMLElement | null;
  virtualCoord?: { x: number; y: number } | null;
}

export function useItemContextMenu<T>() {
  const [activeMenu, setActiveMenu] = useState<ActiveContextMenu<T> | null>(null);

  const openContextMenu = useCallback((item: T, coord: { x: number; y: number }) => {
    setActiveMenu({ item, virtualCoord: coord, anchorEl: null });
  }, []);

  const openAnchorMenu = useCallback((item: T, anchorEl: HTMLElement) => {
    setActiveMenu({ item, anchorEl, virtualCoord: null });
  }, []);

  const closeMenu = useCallback(() => {
    setActiveMenu(null);
  }, []);

  return {
    activeMenu,
    setActiveMenu,
    openContextMenu,
    openAnchorMenu,
    closeMenu,
  };
}
