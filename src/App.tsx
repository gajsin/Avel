import React from 'react';
import { Titlebar } from './components/Titlebar/Titlebar';
import { Sidebar } from './components/Sidebar/Sidebar';
import { ClipboardScreen } from './screens/ClipboardScreen/ClipboardScreen';
import { RecentScreen } from './screens/RecentScreen/RecentScreen';
import { BrightnessScreen } from './screens/BrightnessScreen/BrightnessScreen';
import { AppearanceScreen } from './screens/AppearanceScreen/AppearanceScreen';
import { DesktopScreen } from './screens/DesktopScreen/DesktopScreen';
import { SettingsScreen } from './screens/SettingsScreen/SettingsScreen';
import { useApp } from './context/AppContext';

export const App: React.FC = () => {
  const { activeSection } = useApp();

  const renderActiveScreen = () => {
    switch (activeSection) {
      case 'clipboard':
        return <ClipboardScreen />;
      case 'recent':
        return <RecentScreen />;
      case 'brightness':
        return <BrightnessScreen />;
      case 'appearance':
        return <AppearanceScreen />;
      case 'desktop':
        return <DesktopScreen />;
      case 'settings':
        return <SettingsScreen />;
      default:
        return <ClipboardScreen />;
    }
  };

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100vh',
        width: '100vw',
        overflow: 'hidden',
        background: 'var(--bg-app)',
        color: 'var(--text-primary)',
      }}
    >
      <Titlebar />
      <div
        style={{
          display: 'flex',
          flex: 1,
          minHeight: 0,
          overflow: 'hidden',
        }}
      >
        <Sidebar />
        <main
          style={{
            flex: 1,
            display: 'flex',
            minWidth: 0,
            overflow: 'hidden',
          }}
          key={activeSection}
        >
          {renderActiveScreen()}
        </main>
      </div>
    </div>
  );
};
