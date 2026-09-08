import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import styles from './OverlayMenu.module.css';

export interface OverlayMenuProps {
  isOpen: boolean;
  onClose: () => void;
  anchorEl?: HTMLElement | null;
  virtualCoord?: { x: number; y: number } | null;
  children: React.ReactNode;
  className?: string;
  align?: 'start' | 'end';
}

const VIEWPORT_PADDING = 8;
const ANCHOR_GAP = 4;

export const OverlayMenu: React.FC<OverlayMenuProps> = ({
  isOpen,
  onClose,
  anchorEl,
  virtualCoord,
  children,
  className,
  align = 'end',
}) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const [coords, setCoords] = useState<{ top: number; left: number; flippedY: boolean } | null>(null);

  const updatePosition = () => {
    if (!menuRef.current) return;

    let anchorRect: { top: number; bottom: number; left: number; right: number; width: number; height: number };

    const hasValidAnchor =
      anchorEl &&
      anchorEl.isConnected &&
      (anchorEl.offsetWidth > 0 || anchorEl.offsetHeight > 0);

    if (hasValidAnchor) {
      anchorRect = anchorEl.getBoundingClientRect();
      // If anchor scrolled off screen, close
      if (
        anchorRect.bottom < 0 ||
        anchorRect.top > window.innerHeight ||
        anchorRect.right < 0 ||
        anchorRect.left > window.innerWidth
      ) {
        onClose();
        return;
      }
    } else if (virtualCoord) {
      anchorRect = {
        top: virtualCoord.y,
        bottom: virtualCoord.y,
        left: virtualCoord.x,
        right: virtualCoord.x,
        width: 0,
        height: 0,
      };
    } else if (anchorEl) {
      anchorRect = anchorEl.getBoundingClientRect();
    } else {
      return;
    }

    const menuRect = menuRef.current.getBoundingClientRect();
    const menuWidth = menuRect.width || 160;
    const menuHeight = menuRect.height || 120;

    let top = anchorRect.bottom + ANCHOR_GAP;
    let flippedY = false;

    // Vertical flip check
    if (top + menuHeight > window.innerHeight - VIEWPORT_PADDING) {
      if (anchorRect.top - ANCHOR_GAP - menuHeight >= VIEWPORT_PADDING) {
        top = anchorRect.top - ANCHOR_GAP - menuHeight;
        flippedY = true;
      } else {
        // Fallback: clamp within viewport
        top = Math.max(VIEWPORT_PADDING, window.innerHeight - VIEWPORT_PADDING - menuHeight);
      }
    }

    // Horizontal alignment
    let left = align === 'start'
      ? anchorRect.left
      : anchorRect.right - menuWidth;

    // Horizontal shift check
    if (left + menuWidth > window.innerWidth - VIEWPORT_PADDING) {
      left = window.innerWidth - VIEWPORT_PADDING - menuWidth;
    }
    if (left < VIEWPORT_PADDING) {
      left = VIEWPORT_PADDING;
    }

    setCoords({ top, left, flippedY });
  };

  useLayoutEffect(() => {
    if (isOpen) {
      updatePosition();
    } else {
      setCoords(null);
    }
  }, [isOpen, anchorEl, virtualCoord]);

  // Click outside & Escape dismissal
  useEffect(() => {
    if (!isOpen) return;

    const handlePointerDown = (e: MouseEvent | PointerEvent) => {
      const target = e.target as Node;
      if (menuRef.current && menuRef.current.contains(target)) return;
      if (anchorEl && anchorEl.contains(target)) return;
      onClose();
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        if (!menuRef.current) return;
        e.preventDefault();
        const focusables = Array.from(
          menuRef.current.querySelectorAll<HTMLElement>('button:not([disabled]), [tabindex="0"]')
        );
        if (focusables.length === 0) return;
        const currentIndex = focusables.indexOf(document.activeElement as HTMLElement);
        if (e.key === 'ArrowDown') {
          const nextIndex = (currentIndex + 1) % focusables.length;
          focusables[nextIndex]?.focus();
        } else {
          const prevIndex = (currentIndex - 1 + focusables.length) % focusables.length;
          focusables[prevIndex]?.focus();
        }
      }
    };

    const handleScrollOrResize = () => {
      updatePosition();
    };

    document.addEventListener('pointerdown', handlePointerDown, true);
    document.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('scroll', handleScrollOrResize, true);
    window.addEventListener('resize', handleScrollOrResize);

    return () => {
      document.removeEventListener('pointerdown', handlePointerDown, true);
      document.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('scroll', handleScrollOrResize, true);
      window.removeEventListener('resize', handleScrollOrResize);
    };
  }, [isOpen, anchorEl, onClose]);

  // Focus management
  useEffect(() => {
    if (isOpen && menuRef.current) {
      const firstBtn = menuRef.current.querySelector<HTMLElement>('button:not([disabled])');
      firstBtn?.focus();
    }
  }, [isOpen]);

  if (!isOpen) return null;

  return createPortal(
    <div
      ref={menuRef}
      role="menu"
      className={`${styles.overlayMenu} ${coords?.flippedY ? styles.flippedUp : ''} ${className || ''}`}
      style={{
        top: coords ? `${coords.top}px` : '-9999px',
        left: coords ? `${coords.left}px` : '-9999px',
        opacity: coords ? 1 : 0,
      }}
      onClick={(e) => e.stopPropagation()}
    >
      {children}
    </div>,
    document.body
  );
};

export interface OverlayMenuItemProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  danger?: boolean;
  icon?: React.ReactNode;
  label?: React.ReactNode;
}

export const OverlayMenuItem: React.FC<OverlayMenuItemProps> = ({
  danger = false,
  icon,
  label,
  children,
  className = '',
  ...props
}) => {
  return (
    <button
      type="button"
      className={`${styles.menuItem} ${danger ? styles.danger : ''} ${className}`}
      {...props}
    >
      {icon}
      {label && <span>{label}</span>}
      {children}
    </button>
  );
};

