import React from 'react';
import { PanelLeftClose, PanelLeftOpen } from 'lucide';
import { MorphIcon } from '../MorphIcon/MorphIcon';
import { useApp } from '../../context/AppContext';
import styles from './SidebarCollapseButton.module.css';

export interface SidebarCollapseButtonProps {
  collapsed: boolean;
  onToggle: () => void;
}

export const SidebarCollapseButton: React.FC<SidebarCollapseButtonProps> = ({
  collapsed,
  onToggle,
}) => {
  const { t } = useApp();
  const title = collapsed ? t.sidebar.expand : t.sidebar.collapse;

  return (
    <button
      type="button"
      className={styles.toggleBtn}
      onClick={onToggle}
      title={title}
      aria-label={title}
    >
      <MorphIcon
        icon={collapsed ? PanelLeftOpen : PanelLeftClose}
        size={16}
        spring="snappy"
        color="currentColor"
      />
    </button>
  );
};
