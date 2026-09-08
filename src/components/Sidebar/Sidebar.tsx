import React from 'react';
import {
  Clock,
  Monitor,
  Paintbrush,
  Settings,
  Clipboard,
  Sun,
} from 'lucide-react';
import { useApp } from '../../context/AppContext';
import { NavSection } from '../../types';
import { AvelLogo } from '../AvelLogo/AvelLogo';
import { SidebarCollapseButton } from '../SidebarCollapseButton/SidebarCollapseButton';
import styles from './Sidebar.module.css';

export const Sidebar: React.FC = () => {
  const {
    activeSection,
    setActiveSection,
    sidebarCollapsed,
    toggleSidebar,
    effectiveTheme,
    t,
  } = useApp();

  const navItems: { id: NavSection; label: string; icon: React.ReactNode }[] = [
    { id: 'clipboard', label: t.nav.clipboard, icon: <Clipboard size={16} /> },
    { id: 'recent', label: t.nav.recent, icon: <Clock size={16} /> },
    { id: 'brightness', label: t.nav.brightness, icon: <Sun size={16} /> },
    { id: 'appearance', label: t.nav.appearance, icon: <Paintbrush size={16} /> },
    { id: 'desktop', label: t.nav.desktop, icon: <Monitor size={16} /> },
    { id: 'settings', label: t.nav.settings, icon: <Settings size={16} /> },
  ];

  return (
    <aside
      className={`${styles.sidebar} ${
        sidebarCollapsed ? styles.collapsed : styles.expanded
      }`}
    >
      <div className={styles.header}>
        <div
          className={styles.logoArea}
          onClick={() => setActiveSection('clipboard')}
          title="Avel"
        >
          <AvelLogo
            mode={sidebarCollapsed ? 'mark' : 'full'}
            theme={effectiveTheme}
            size={sidebarCollapsed ? 26 : undefined}
          />
        </div>
      </div>

      <ul className={styles.navList}>
        {navItems.map((item) => {
          const isActive = activeSection === item.id;
          return (
            <li key={item.id}>
              <button
                className={`${styles.navItem} ${isActive ? styles.active : ''}`}
                onClick={() => setActiveSection(item.id)}
                title={sidebarCollapsed ? item.label : undefined}
                aria-current={isActive ? 'page' : undefined}
              >
                <span className={styles.navIcon}>{item.icon}</span>
                {!sidebarCollapsed && (
                  <span className={styles.navLabel}>{item.label}</span>
                )}
              </button>
            </li>
          );
        })}
      </ul>

      <div className={styles.footer}>
        <SidebarCollapseButton
          collapsed={sidebarCollapsed}
          onToggle={toggleSidebar}
        />
      </div>
    </aside>
  );
};
