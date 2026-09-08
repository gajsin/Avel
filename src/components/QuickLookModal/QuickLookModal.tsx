import React, { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import styles from './QuickLookModal.module.css';

export interface QuickLookModalProps {
  isOpen: boolean;
  title: string;
  onClose: () => void;
  imageSrc?: string | null;
  textContent?: string | null;
  footerInfo?: React.ReactNode;
  actionButton?: React.ReactNode;
  closeAriaLabel?: string;
}

export const QuickLookModal: React.FC<QuickLookModalProps> = ({
  isOpen,
  title,
  onClose,
  imageSrc,
  textContent,
  footerInfo,
  actionButton,
  closeAriaLabel = 'Close',
}) => {
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };

    document.addEventListener('keydown', handleKeyDown, true);
    return () => {
      document.removeEventListener('keydown', handleKeyDown, true);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return createPortal(
    <div
      className={styles.overlay}
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title} title={title}>
            {title}
          </span>
          <button
            type="button"
            className={styles.closeBtn}
            onClick={onClose}
            title={`${closeAriaLabel} (Esc)`}
            aria-label={closeAriaLabel}
          >
            <X size={16} />
          </button>
        </div>

        <div className={styles.body}>
          {imageSrc ? (
            <img src={imageSrc} alt="" className={styles.image} />
          ) : textContent ? (
            <div className={styles.textContent}>{textContent}</div>
          ) : null}
        </div>

        <div className={styles.footer}>
          <div className={styles.footerInfo}>{footerInfo}</div>
          {actionButton && <div className={styles.footerActions}>{actionButton}</div>}
        </div>
      </div>
    </div>,
    document.body
  );
};
