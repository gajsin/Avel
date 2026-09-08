import React, { createContext, useContext, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { dictionaries, Language, TranslationKeys } from '../i18n';
import { api } from '../services/api';
import { AppSettings, AppTheme, NavSection } from '../types';

interface AppContextType {
  activeSection: NavSection;
  setActiveSection: (sec: NavSection) => void;
  settings: AppSettings;
  updateSettings: (newSettings: Partial<AppSettings>) => Promise<void>;
  setLocalSettings: (partial: Partial<AppSettings>) => void;
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  language: Language;
  setLanguage: (lang: Language) => void;
  appTheme: AppTheme;
  setAppTheme: (theme: AppTheme) => void;
  effectiveTheme: 'light' | 'dark';
  t: TranslationKeys;
  toast: string | null;
  showToast: (msg: string) => void;
}

const defaultSettings: AppSettings = {
  app_theme: 'System',
  language: 'ru',
  autostart: false,
  last_section: 'clipboard',
  sidebar_collapsed: false,
  hotkey_clipboard: '{"code":"Equal","ctrl":false,"alt":false,"shift":false,"meta":false}',
  hotkey_recent: '{"code":"F8","ctrl":false,"alt":false,"shift":false,"meta":false}',
  hotkey_appearance: '{"code":"KeyD","ctrl":true,"alt":true,"shift":false,"meta":false}',
  hotkey_desktop: '{"code":"ArrowDown","ctrl":false,"alt":false,"shift":false,"meta":false}',
};

const AppContext = createContext<AppContextType>({
  activeSection: 'clipboard',
  setActiveSection: () => {},
  settings: defaultSettings,
  updateSettings: async () => {},
  setLocalSettings: () => {},
  sidebarCollapsed: false,
  toggleSidebar: () => {},
  language: 'ru',
  setLanguage: () => {},
  appTheme: 'System',
  setAppTheme: () => {},
  effectiveTheme: 'light',
  t: dictionaries.ru,
  toast: null,
  showToast: () => {},
});

export const AppProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [activeSection, setActiveSectionState] = useState<NavSection>('clipboard');
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [language, setLanguageState] = useState<Language>('ru');
  const [appTheme, setAppThemeState] = useState<AppTheme>('System');
  const [systemIsDark, setSystemIsDark] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const settingsRef = useRef<AppSettings>(defaultSettings);
  const saveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const toastTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load initial settings
  useEffect(() => {
    api.getSettings().then((s) => {
      settingsRef.current = s;
      setSettings(s);
      setActiveSectionState(s.last_section || 'clipboard');
      setSidebarCollapsed(s.sidebar_collapsed);
      setLanguageState(s.language || 'ru');
      setAppThemeState(s.app_theme || 'System');
    }).catch(() => {});

    // System dark mode listener
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    setSystemIsDark(mq.matches);
    const handler = (e: MediaQueryListEvent) => setSystemIsDark(e.matches);
    mq.addEventListener('change', handler);

    // Tauri backend event listeners
    const unlistenNavigate = listen<string>('navigate-section', (event) => {
      if (['clipboard', 'recent', 'brightness', 'appearance', 'desktop', 'settings'].includes(event.payload)) {
        const sec = event.payload as NavSection;
        setActiveSectionState(sec);
        settingsRef.current = { ...settingsRef.current, last_section: sec };
        setSettings((prev) => ({ ...prev, last_section: sec }));
      }
    });

    const unlistenTheme = listen<boolean>('theme-changed', (event) => {
      setSystemIsDark(!event.payload);
    });

    return () => {
      if (toastTimeoutRef.current) {
        clearTimeout(toastTimeoutRef.current);
        toastTimeoutRef.current = null;
      }
      mq.removeEventListener('change', handler);
      unlistenNavigate.then((u) => u());
      unlistenTheme.then((u) => u());
    };
  }, []);

  const updateSettings = (partial: Partial<AppSettings>): Promise<void> => {
    const next = { ...settingsRef.current, ...partial };
    settingsRef.current = next;
    setSettings(next);

    const thisSave = saveQueueRef.current.then(() => api.saveSettings(next));
    saveQueueRef.current = thisSave.catch(() => {});
    return thisSave;
  };

  const setLocalSettings = (partial: Partial<AppSettings>) => {
    const next = { ...settingsRef.current, ...partial };
    settingsRef.current = next;
    setSettings(next);
  };

  const setActiveSection = (sec: NavSection) => {
    setActiveSectionState(sec);
    updateSettings({ last_section: sec });
  };

  const toggleSidebar = () => {
    const next = !sidebarCollapsed;
    setSidebarCollapsed(next);
    updateSettings({ sidebar_collapsed: next });
  };

  const setLanguage = (lang: Language) => {
    setLanguageState(lang);
    updateSettings({ language: lang });
  };

  const setAppTheme = (theme: AppTheme) => {
    setAppThemeState(theme);
    updateSettings({ app_theme: theme });
  };

  const showToast = (msg: string) => {
    if (toastTimeoutRef.current) {
      clearTimeout(toastTimeoutRef.current);
    }
    setToast(msg);
    toastTimeoutRef.current = setTimeout(() => {
      setToast((cur) => (cur === msg ? null : cur));
      toastTimeoutRef.current = null;
    }, 2500);
  };

  const effectiveTheme: 'light' | 'dark' =
    appTheme === 'Dark' ? 'dark' : appTheme === 'Light' ? 'light' : systemIsDark ? 'dark' : 'light';

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', effectiveTheme);
  }, [effectiveTheme]);

  const t = dictionaries[language] || dictionaries.ru;

  return (
    <AppContext.Provider
      value={{
        activeSection,
        setActiveSection,
        settings,
        updateSettings,
        setLocalSettings,
        sidebarCollapsed,
        toggleSidebar,
        language,
        setLanguage,
        appTheme,
        setAppTheme,
        effectiveTheme,
        t,
        toast,
        showToast,
      }}
    >
      {children}
      {toast && (
        <div
          style={{
            position: 'fixed',
            bottom: '20px',
            right: '20px',
            background: 'var(--text-primary)',
            color: 'var(--bg-app)',
            padding: '8px 16px',
            borderRadius: 'var(--radius-md)',
            fontSize: '13px',
            fontWeight: 500,
            boxShadow: 'var(--shadow-md)',
            zIndex: 9999,
            pointerEvents: 'none',
          }}
        >
          {toast}
        </div>
      )}
    </AppContext.Provider>
  );
};

export const useApp = () => useContext(AppContext);
