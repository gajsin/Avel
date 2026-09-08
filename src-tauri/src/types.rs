use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShortcutSpec {
    pub code: String, // e.g. "KeyD", "ArrowUp", "F8", "Equal"
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub meta: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub app_theme: String, // "System", "Light", "Dark"
    pub language: String,  // "ru", "en"
    pub autostart: bool,
    pub last_section: String, // "clipboard", "recent", "appearance", "desktop", "settings"
    pub sidebar_collapsed: bool,
    pub hotkey_clipboard: String,
    pub hotkey_recent: String,
    pub hotkey_appearance: String,
    pub hotkey_desktop: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            app_theme: "System".to_string(),
            language: "ru".to_string(),
            autostart: false,
            last_section: "clipboard".to_string(),
            sidebar_collapsed: false,
            hotkey_clipboard:
                r#"{"code":"Equal","ctrl":false,"alt":false,"shift":false,"meta":false}"#
                    .to_string(),
            hotkey_recent: r#"{"code":"F8","ctrl":false,"alt":false,"shift":false,"meta":false}"#
                .to_string(),
            hotkey_appearance:
                r#"{"code":"KeyD","ctrl":true,"alt":true,"shift":false,"meta":false}"#.to_string(),
            hotkey_desktop:
                r#"{"code":"ArrowDown","ctrl":false,"alt":false,"shift":false,"meta":false}"#
                    .to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String, // "text", "link", "code", "image", "screenshot"
    pub text_content: Option<String>,
    pub image_path: Option<String>,
    pub thumbnail_b64: Option<String>,
    pub char_count: Option<i64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub hash: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentItem {
    pub id: i64,
    pub entry_type: String, // "file", "folder"
    pub title: String,
    pub path: String,
    pub thumbnail_b64: Option<String>,
    pub last_accessed_at: String,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    pub windows_theme_target: String, // "System", "Light", "Dark"
    pub schedule_enabled: bool,
    pub schedule_light_time: String,
    pub schedule_dark_time: String,
    pub switch_wallpaper: bool,
    pub wallpaper_light_path: String,
    pub wallpaper_dark_path: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            windows_theme_target: "System".to_string(),
            schedule_enabled: false,
            schedule_light_time: "07:00".to_string(),
            schedule_dark_time: "19:00".to_string(),
            switch_wallpaper: false,
            wallpaper_light_path: String::new(),
            wallpaper_dark_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanDesktopState {
    pub current_mode: String, // "none", "icons", "taskbar", "all"
    pub is_hidden: bool,
    pub actual_icons_hidden: bool,
    pub actual_taskbar_hidden: bool,
}

impl Default for CleanDesktopState {
    fn default() -> Self {
        Self {
            current_mode: "icons".to_string(),
            is_hidden: false,
            actual_icons_hidden: false,
            actual_taskbar_hidden: false,
        }
    }
}
