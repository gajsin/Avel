export type NavSection = 'clipboard' | 'recent' | 'brightness' | 'appearance' | 'desktop' | 'settings';

export type AppTheme = 'System' | 'Light' | 'Dark';
export type Language = 'ru' | 'en';
export type ViewMode = 'details' | 'grid';
export type ClipboardFilter = 'all' | 'text' | 'images' | 'favorites';
export type RecentFilter = 'all' | 'files' | 'folders' | 'favorites';
export type CleanDesktopMode = 'none' | 'icons' | 'taskbar' | 'all';

export interface ShortcutSpec {
  code: string; // e.g. "KeyD", "ArrowUp", "F8", "Equal"
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
}

export interface AppSettings {
  app_theme: AppTheme;
  language: Language;
  autostart: boolean;
  last_section: NavSection;
  sidebar_collapsed: boolean;
  hotkey_clipboard: string;
  hotkey_recent: string;
  hotkey_appearance: string;
  hotkey_desktop: string;
}

export interface ClipboardItem {
  id: number;
  content_type: 'text' | 'link' | 'code' | 'image' | 'screenshot';
  text_content: string | null;
  image_path: string | null;
  thumbnail_b64: string | null;
  char_count: number | null;
  width: number | null;
  height: number | null;
  hash: string;
  created_at: string;
  updated_at: string;
  is_pinned: boolean;
}

export interface RecentItem {
  id: number;
  entry_type: 'file' | 'folder';
  title: string;
  path: string;
  thumbnail_b64: string | null;
  last_accessed_at: string;
  is_pinned: boolean;
}

export interface AppearanceConfig {
  windows_theme_target: AppTheme;
  schedule_enabled: boolean;
  schedule_light_time: string;
  schedule_dark_time: string;
  switch_wallpaper: boolean;
  wallpaper_light_path: string;
  wallpaper_dark_path: string;
}

export interface CleanDesktopState {
  current_mode: CleanDesktopMode;
  is_hidden: boolean;
  actual_icons_hidden: boolean;
  actual_taskbar_hidden: boolean;
}

export * from './brightness';
