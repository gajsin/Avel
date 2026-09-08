import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import {
  AppearanceConfig,
  AppSettings,
  CleanDesktopMode,
  CleanDesktopState,
  ClipboardFilter,
  ClipboardItem,
  RecentFilter,
  RecentItem,
  ShortcutSpec,
  ViewMode,
} from '../types';

export const api = {
  // Settings
  async getSettings(): Promise<AppSettings> {
    return invoke<AppSettings>('get_settings');
  },

  async saveSettings(settings: AppSettings): Promise<void> {
    return invoke('save_settings', { settings });
  },

  // View Preferences
  async getViewMode(tabId: string): Promise<ViewMode> {
    const res = await invoke<string>('get_view_mode', { tabId });
    return (res as ViewMode) || 'details';
  },

  async setViewMode(tabId: string, viewMode: ViewMode): Promise<void> {
    return invoke('set_view_mode', { tabId, viewMode });
  },

  // Clipboard
  async getClipboardItems(
    filterType?: ClipboardFilter,
    searchQuery?: string
  ): Promise<ClipboardItem[]> {
    return invoke<ClipboardItem[]>('get_clipboard_items', {
      filterType: filterType || 'all',
      searchQuery: searchQuery || null,
    });
  },

  async deleteClipboardItem(id: number): Promise<void> {
    return invoke('delete_clipboard_item', { id });
  },

  async deleteClipboardItems(ids: number[]): Promise<void> {
    return invoke('delete_clipboard_items', { ids });
  },

  async clearClipboardHistory(): Promise<void> {
    return invoke('clear_clipboard_history');
  },

  async togglePinClipboard(id: number): Promise<boolean> {
    return invoke<boolean>('toggle_pin_clipboard_item', { id });
  },

  async copyText(text: string): Promise<void> {
    return invoke('copy_clipboard_text', { text });
  },

  async copyImage(imagePath: string): Promise<void> {
    return invoke('copy_clipboard_image', { imageRelOrAbs: imagePath });
  },

  async copyFiles(imagePaths: string[]): Promise<void> {
    return invoke('copy_clipboard_files', { imagePaths });
  },

  // Recent
  async getRecentItems(
    filterType?: RecentFilter,
    searchQuery?: string
  ): Promise<RecentItem[]> {
    return invoke<RecentItem[]>('get_recent_items', {
      filterType: filterType || 'all',
      searchQuery: searchQuery || null,
    });
  },

  async togglePinRecent(id: number): Promise<boolean> {
    return invoke<boolean>('toggle_pin_recent_item', { id });
  },

  async openRecentItem(path: string): Promise<void> {
    return invoke('open_recent_item', { path });
  },

  async deleteRecentItem(id: number): Promise<void> {
    return invoke('delete_recent_item', { id });
  },

  async clearRecentHistory(): Promise<void> {
    return invoke('clear_recent_history');
  },

  // Appearance
  async getAppearanceConfig(): Promise<AppearanceConfig> {
    return invoke<AppearanceConfig>('get_appearance_config');
  },

  async saveAppearanceConfig(config: AppearanceConfig): Promise<AppearanceConfig> {
    return invoke<AppearanceConfig>('save_appearance_config', { config });
  },

  async getWallpaperThumbnail(
    path: string,
    width?: number,
    height?: number
  ): Promise<string> {
    return invoke<string>('get_wallpaper_thumbnail', {
      path,
      width: width || 200,
      height: height || 200,
    });
  },

  async pickWallpaperFile(): Promise<string | null> {
    const selected = await openDialog({
      multiple: false,
      filters: [
        {
          name: 'Images',
          extensions: ['png', 'jpg', 'jpeg', 'webp'],
        },
      ],
    });
    if (typeof selected === 'string') {
      return selected;
    }
    return null;
  },

  // Clean Desktop
  async getCleanDesktopState(): Promise<CleanDesktopState> {
    return invoke<CleanDesktopState>('get_clean_desktop_state');
  },

  async setCleanDesktopMode(
    mode: CleanDesktopMode
  ): Promise<CleanDesktopState> {
    return invoke<CleanDesktopState>('set_clean_desktop_mode', { mode });
  },

  // Hotkeys
  async updateGlobalHotkeySpec(
    action: string,
    newSpec: ShortcutSpec
  ): Promise<void> {
    return invoke('update_global_hotkey_spec', { action, newSpec });
  },

  async getHotkeyStatuses(): Promise<
    Record<string, { is_registered: boolean; error_message: string | null }>
  > {
    return invoke('get_hotkey_statuses');
  },

  // Window Controls
  async minimize(): Promise<void> {
    return invoke('minimize_window');
  },

  async hide(): Promise<void> {
    return invoke('hide_window');
  },

  async openExternal(url: string): Promise<void> {
    return invoke('open_external_url', { url });
  },

  // Brightness
  async getBrightnessState(): Promise<import('../types').BrightnessStateSnapshot> {
    return invoke('brightness_get_state');
  },

  async refreshBrightness(): Promise<import('../types').BrightnessStateSnapshot> {
    return invoke('brightness_refresh');
  },

  async setBrightness(displayId: string, targetPercent: number): Promise<void> {
    return invoke('brightness_set', { displayId, targetPercent });
  },

  async setGroupBrightness(targetPercent: number): Promise<[string, boolean][]> {
    return invoke('brightness_set_group', { targetPercent });
  },

  async getBrightnessPreferences(): Promise<import('../types').BrightnessPreferences> {
    return invoke('brightness_get_preferences');
  },

  async saveBrightnessPreferences(preferences: import('../types').BrightnessPreferences): Promise<void> {
    return invoke('brightness_save_preferences', { preferences });
  },
};
