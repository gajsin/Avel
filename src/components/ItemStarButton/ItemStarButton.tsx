import { Star as StarIconNode } from 'lucide';
import { MorphIcon } from '../MorphIcon/MorphIcon';
import styles from './ItemStarButton.module.css';

export interface ItemStarButtonProps {
  isPinned: boolean;
  isAnimating?: boolean;
  onToggle: (e: React.MouseEvent<HTMLButtonElement>) => void;
  size?: number;
  className?: string;
  title?: string;
  ariaLabel?: string;
}

export const ItemStarButton: React.FC<ItemStarButtonProps> = ({
  isPinned,
  isAnimating = false,
  onToggle,
  size = 12,
  className = '',
  title,
  ariaLabel,
}) => {
  return (
    <button
      type="button"
      className={`${styles.starBtn} ${isPinned ? styles.starred : ''} ${
        isAnimating ? styles.animating : ''
      } ${className}`}
      onClick={(e) => {
        e.stopPropagation();
        onToggle(e);
      }}
      title={title}
      aria-label={ariaLabel || (isPinned ? 'Unfavorite' : 'Favorite')}
    >
      <MorphIcon
        icon={StarIconNode}
        size={size}
        spring={{ stiffness: 90, damping: 12 }}
        color={isPinned ? '#F59E0B' : 'currentColor'}
        style={{
          fill: isPinned ? '#F59E0B' : 'transparent',
          transition:
            'fill 0.45s ease, color 0.45s ease, transform 0.45s cubic-bezier(0.34, 1.56, 0.64, 1)',
        }}
      />
    </button>
  );
};
