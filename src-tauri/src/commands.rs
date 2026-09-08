use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::clipboard_listener::ClipboardState;
use crate::db::Db;
use crate::desktop::DesktopManager;
use crate::hotkeys;
use crate::theme::ThemeCoordinator;
use crate::thumbnails::ThumbnailService;
use crate::types::{
    AppSettings, AppearanceConfig, CleanDesktopState, ClipboardItem, RecentItem, ShortcutSpec,
};
use crate::win32_clipboard::{set_clipboard_files, set_clipboard_image, set_clipboard_text};

fn allow_asset_file(app: &AppHandle, path: &str) {
    let path = Path::new(path);
    if path.is_file() {
        let _ = app.asset_protocol_scope().allow_file(path);
    }
}

fn is_previewable_image(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "svg"
            )
        })
        .unwrap_or(false)
}

pub fn open_path_or_url(path_or_url: &str) -> Result<(), String> {
    let wide: Vec<u16> = path_or_url
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let op: Vec<u16> = "open\0".encode_utf16().collect();
    unsafe {
        let res = ShellExecuteW(
            HWND(std::ptr::null_mut()),
            PCWSTR(op.as_ptr()),
            PCWSTR(wide.as_ptr()),
            PCWSTR(std::ptr::null()),
            PCWSTR(std::ptr::null()),
            SW_SHOWNORMAL,
        );
        if (res.0 as isize) > 32 {
            Ok(())
        } else {
            Err(format!("ShellExecute failed with code {}", res.0 as isize))
        }
    }
}

#[tauri::command]
pub fn get_settings(db: State<'_, Arc<Db>>) -> Result<AppSettings, String> {
    db.get_settings()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    db: State<'_, Arc<Db>>,
    settings: AppSettings,
) -> Result<(), String> {
    db.save_settings(&settings)?;
    if let Some(tray) = app.tray_by_id("avel-tray") {
        if let Ok(menu) = crate::create_tray_menu(&app, settings.language == "en") {
            let _ = tray.set_menu(Some(menu));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_view_mode(db: State<'_, Arc<Db>>, tab_id: String) -> Result<String, String> {
    db.get_view_mode(&tab_id)
}

#[tauri::command]
pub fn set_view_mode(
    db: State<'_, Arc<Db>>,
    tab_id: String,
    view_mode: String,
) -> Result<(), String> {
    db.set_view_mode(&tab_id, &view_mode)
}

#[tauri::command]
pub fn get_clipboard_items(
    app: AppHandle,
    db: State<'_, Arc<Db>>,
    filter_type: Option<String>,
    search_query: Option<String>,
) -> Result<Vec<ClipboardItem>, String> {
    let items = db.get_clipboard_items(filter_type.as_deref(), search_query.as_deref())?;
    for item in &items {
        if matches!(item.content_type.as_str(), "image" | "screenshot") {
            if let Some(path) = item.image_path.as_deref() {
                allow_asset_file(&app, path);
            }
        }
    }
    Ok(items)
}

#[tauri::command]
pub fn delete_clipboard_item(db: State<'_, Arc<Db>>, id: i64) -> Result<(), String> {
    db.delete_clipboard_item(id)
}

#[tauri::command]
pub fn delete_clipboard_items(db: State<'_, Arc<Db>>, ids: Vec<i64>) -> Result<(), String> {
    db.delete_clipboard_items(&ids)
}

#[tauri::command]
pub fn clear_clipboard_history(db: State<'_, Arc<Db>>) -> Result<(), String> {
    db.clear_clipboard_history()
}

#[tauri::command]
pub fn copy_clipboard_text(
    clip_state: State<'_, Arc<ClipboardState>>,
    text: String,
) -> Result<(), String> {
    let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
    let seq = set_clipboard_text(&text)?;
    clip_state.set_own_copy(seq, hash);
    Ok(())
}

#[tauri::command]
pub fn copy_clipboard_image(
    db: State<'_, Arc<Db>>,
    clip_state: State<'_, Arc<ClipboardState>>,
    image_rel_or_abs: String,
) -> Result<(), String> {
    let p = if Path::new(&image_rel_or_abs).is_absolute() {
        PathBuf::from(image_rel_or_abs)
    } else {
        db.images_dir.join(image_rel_or_abs)
    };

    if !p.is_file() {
        return Err("Image file not found".to_string());
    }

    let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
    let hash = blake3::hash(&bytes).to_hex().to_string();

    let seq = set_clipboard_image(&p)?;
    clip_state.set_own_copy(seq, hash);
    Ok(())
}

#[tauri::command]
pub fn copy_clipboard_files(
    db: State<'_, Arc<Db>>,
    clip_state: State<'_, Arc<ClipboardState>>,
    image_paths: Vec<String>,
) -> Result<(), String> {
    let mut resolved_paths: Vec<PathBuf> = Vec::new();
    for p_str in &image_paths {
        let p = if Path::new(p_str).is_absolute() {
            PathBuf::from(p_str)
        } else {
            db.images_dir.join(p_str)
        };
        if p.is_file() {
            resolved_paths.push(p);
        }
    }

    if resolved_paths.is_empty() {
        return Err("No valid image files found to copy".to_string());
    }

    let hash_input = resolved_paths
        .iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join("|");
    let hash = blake3::hash(hash_input.as_bytes()).to_hex().to_string();

    let seq = set_clipboard_files(&resolved_paths)?;
    clip_state.set_own_copy(seq, hash);
    Ok(())
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    open_path_or_url(&url)
}

#[tauri::command]
pub fn get_recent_items(
    app: AppHandle,
    db: State<'_, Arc<Db>>,
    filter_type: Option<String>,
    search_query: Option<String>,
) -> Result<Vec<RecentItem>, String> {
    let items = db.get_recent_items(filter_type.as_deref(), search_query.as_deref())?;
    for item in &items {
        if is_previewable_image(&item.path) {
            allow_asset_file(&app, &item.path);
        }
    }
    Ok(items)
}

#[tauri::command]
pub fn open_recent_item(path: String) -> Result<(), String> {
    open_path_or_url(&path)
}

#[tauri::command]
pub fn delete_recent_item(db: State<'_, Arc<Db>>, id: i64) -> Result<(), String> {
    db.delete_recent_item(id)
}

#[tauri::command]
pub fn clear_recent_history(db: State<'_, Arc<Db>>) -> Result<(), String> {
    db.clear_recent_history()
}

#[tauri::command]
pub fn get_appearance_config(db: State<'_, Arc<Db>>) -> Result<AppearanceConfig, String> {
    db.get_appearance_config()
}

#[tauri::command]
pub fn save_appearance_config(
    app: AppHandle,
    db: State<'_, Arc<Db>>,
    coordinator: State<'_, Arc<ThemeCoordinator>>,
    config: AppearanceConfig,
) -> Result<AppearanceConfig, String> {
    db.save_appearance_config(&config)?;

    // If manual theme selection was changed ("Light" or "Dark"), apply immediately
    if config.windows_theme_target == "Light" {
        let wp = if config.switch_wallpaper && !config.wallpaper_light_path.is_empty() {
            Some(config.wallpaper_light_path.clone())
        } else {
            None
        };
        coordinator.apply_theme(&app, true, config.switch_wallpaper, wp)?;
    } else if config.windows_theme_target == "Dark" {
        let wp = if config.switch_wallpaper && !config.wallpaper_dark_path.is_empty() {
            Some(config.wallpaper_dark_path.clone())
        } else {
            None
        };
        coordinator.apply_theme(&app, false, config.switch_wallpaper, wp)?;
    }

    get_appearance_config(db)
}

#[tauri::command]
pub async fn get_wallpaper_thumbnail(
    thumb_svc: State<'_, Arc<ThumbnailService>>,
    path: String,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<String, String> {
    let w = width.unwrap_or(200);
    let h = height.unwrap_or(200);
    let svc = thumb_svc.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        svc.get_or_generate_wallpaper_thumbnail(&path, w, h)
    })
    .await
    .map_err(|e| format!("thumbnail worker error: {e}"))?
}

#[tauri::command]
pub fn get_clean_desktop_state(
    db: State<'_, Arc<Db>>,
    desktop: State<'_, Arc<DesktopManager>>,
) -> Result<CleanDesktopState, String> {
    Ok(desktop.get_actual_state(&db))
}

#[tauri::command]
pub fn set_clean_desktop_mode(
    db: State<'_, Arc<Db>>,
    desktop: State<'_, Arc<DesktopManager>>,
    mode: String,
) -> Result<CleanDesktopState, String> {
    desktop.apply_mode(&db, &mode)
}

#[tauri::command]
pub fn get_hotkey_statuses(
    hotkey_state: State<'_, Arc<hotkeys::HotkeyState>>,
) -> std::collections::HashMap<String, hotkeys::HotkeyStatusInfo> {
    hotkey_state.get_statuses()
}

#[tauri::command]
pub fn update_global_hotkey_spec(
    app: AppHandle,
    action: String,
    new_spec: ShortcutSpec,
) -> Result<(), String> {
    let act = action.parse::<hotkeys::HotkeyAction>()?;
    hotkeys::update_hotkey_transactional(&app, act, &new_spec)
}

#[tauri::command]
pub fn toggle_pin_clipboard_item(db: State<'_, Arc<Db>>, id: i64) -> Result<bool, String> {
    db.toggle_pin_clipboard_item(id)
}

#[tauri::command]
pub fn toggle_pin_recent_item(db: State<'_, Arc<Db>>, id: i64) -> Result<bool, String> {
    db.toggle_pin_recent_item(id)
}

#[tauri::command]
pub fn minimize_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.minimize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn hide_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}
