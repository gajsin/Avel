use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use chrono::{Local, NaiveTime};
use tauri::{AppHandle, Emitter, Manager};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE, WM_SYSCOLORCHANGE,
    WM_THEMECHANGED,
};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

use crate::db::Db;

const REG_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";

pub struct ThemeCoordinator {
    generation: AtomicU64,
    is_scheduler_running: AtomicBool,
}

impl Default for ThemeCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeCoordinator {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            is_scheduler_running: AtomicBool::new(false),
        }
    }

    pub fn apply_theme(
        &self,
        app: &AppHandle,
        light: bool,
        apply_wallpaper: bool,
        wallpaper_path: Option<String>,
    ) -> Result<bool, String> {
        let current_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // 1. Write both registry keys
        set_registry_theme(light)?;

        // 2. Read back verify
        let effective_light = read_registry_theme();
        if effective_light != light {
            return Err("Registry theme read-back verification failed".to_string());
        }

        // 3. If wallpaper enabled and generation still current, apply wallpaper on COM STA
        if apply_wallpaper {
            if let Some(wp) = wallpaper_path {
                if !wp.is_empty() {
                    let gen_now = self.generation.load(Ordering::SeqCst);
                    if gen_now == current_gen {
                        set_desktop_wallpaper(&wp)?;
                    }
                }
            }
        }

        // 4. Update taskbar/window and tray icons dynamically
        update_system_icons(app, effective_light);

        // 5. Emit updated theme to frontend
        let _ = app.emit("theme-changed", effective_light);

        Ok(effective_light)
    }

    pub fn start_scheduler(&self, app: AppHandle) {
        if self.is_scheduler_running.swap(true, Ordering::SeqCst) {
            return;
        }

        let app_clone = app.clone();
        thread::spawn(move || {
            loop {
                let db = app_clone.state::<Arc<Db>>();
                let cfg = match db.get_appearance_config() {
                    Ok(c) => c,
                    Err(_) => {
                        thread::sleep(Duration::from_secs(10));
                        continue;
                    }
                };

                if cfg.schedule_enabled {
                    if let (Ok(light_t), Ok(dark_t)) = (
                        NaiveTime::parse_from_str(&cfg.schedule_light_time, "%H:%M"),
                        NaiveTime::parse_from_str(&cfg.schedule_dark_time, "%H:%M"),
                    ) {
                        let now = Local::now().time();
                        let target_is_light = is_time_in_light_window(now, light_t, dark_t);
                        let current_is_light = read_registry_theme();

                        if target_is_light != current_is_light {
                            let coordinator = app_clone.state::<Arc<ThemeCoordinator>>();
                            let wp = if target_is_light {
                                cfg.wallpaper_light_path.clone()
                            } else {
                                cfg.wallpaper_dark_path.clone()
                            };
                            let _ = coordinator.apply_theme(
                                &app_clone,
                                target_is_light,
                                cfg.switch_wallpaper,
                                Some(wp),
                            );
                        }
                    }
                }

                // Sleep until next minute check
                thread::sleep(Duration::from_secs(15));
            }
        });
    }
}

pub fn is_time_in_light_window(
    now: NaiveTime,
    light_time: NaiveTime,
    dark_time: NaiveTime,
) -> bool {
    if light_time < dark_time {
        // Standard window: e.g. 07:00 to 19:00
        now >= light_time && now < dark_time
    } else {
        // Overnight window: e.g. 20:00 to 06:00
        now >= light_time || now < dark_time
    }
}

pub fn read_registry_theme() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(REG_PATH, KEY_READ)
        .ok()
        .and_then(|k| k.get_value::<u32, _>("AppsUseLightTheme").ok())
        .unwrap_or(1)
        != 0
}

pub fn set_registry_theme(light: bool) -> Result<(), String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(REG_PATH, KEY_SET_VALUE)
        .map_err(|e| format!("open theme registry key: {e}"))?;

    let val = if light { 1u32 } else { 0u32 };
    key.set_value("AppsUseLightTheme", &val)
        .map_err(|e| format!("write AppsUseLightTheme: {e}"))?;
    key.set_value("SystemUsesLightTheme", &val)
        .map_err(|e| format!("write SystemUsesLightTheme: {e}"))?;

    broadcast_setting_change();
    Ok(())
}

fn broadcast_setting_change() {
    unsafe {
        for name in ["ImmersiveColorSet", "WindowsTheme", ""] {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(wide.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                1000,
                None,
            );
        }

        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_THEMECHANGED,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );

        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SYSCOLORCHANGE,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
}

pub fn set_desktop_wallpaper(wallpaper_path: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    let p = PathBuf::from(wallpaper_path.trim_matches(['"', '\'']));
    if !p.is_file() {
        return Err("Файл обоев не найден или недоступен".to_string());
    }

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let wallpaper: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)
            .map_err(|e| format!("CoCreateInstance DesktopWallpaper failed: {e}"))?;

        let wide: Vec<u16> = p
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        wallpaper
            .SetWallpaper(None, PCWSTR(wide.as_ptr()))
            .map_err(|e| format!("SetWallpaper failed: {e}"))?;

        Ok(())
    }
}

pub fn update_system_icons(app: &AppHandle, light: bool) {
    if let Some(tray) = app.tray_by_id("avel-tray") {
        let bytes: &'static [u8] = if light {
            include_bytes!("../icons/tray-light.png")
        } else {
            include_bytes!("../icons/tray-dark.png")
        };
        if let Ok(img) = tauri::image::Image::from_bytes(bytes) {
            let _ = tray.set_icon(Some(img));
        }
    }
    if let Some(win) = app.get_webview_window("main") {
        let bytes: &'static [u8] = if light {
            include_bytes!("../icons/32x32.png")
        } else {
            include_bytes!("../icons/tray-dark.png")
        };
        if let Ok(img) = tauri::image::Image::from_bytes(bytes) {
            let _ = win.set_icon(img);
        }
    }
    update_start_menu_shortcut_icon(app, light);
}

pub fn update_start_menu_shortcut_icon(app: &AppHandle, light: bool) {
    use std::fs;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::Interface;
    use windows::Win32::System::Com::{IPersistFile, CLSCTX_INPROC_SERVER, STGM_READWRITE};
    use windows::Win32::UI::Shell::{
        IShellLinkW, SHChangeNotify, ShellLink, SHCNE_ASSOCCHANGED, SHCNF_IDLIST,
    };

    let app_data = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(_) => return,
    };
    let icons_dir = app_data.join("icons");
    let _ = fs::create_dir_all(&icons_dir);

    let light_bytes = include_bytes!("../icons/icon-light.ico");
    let dark_bytes = include_bytes!("../icons/icon-dark.ico");

    let light_path = icons_dir.join("icon-light.ico");
    if !light_path.exists()
        || fs::metadata(&light_path).map(|m| m.len()).unwrap_or(0) != light_bytes.len() as u64
    {
        let _ = fs::write(&light_path, light_bytes);
    }

    let dark_path = icons_dir.join("icon-dark.ico");
    if !dark_path.exists()
        || fs::metadata(&dark_path).map(|m| m.len()).unwrap_or(0) != dark_bytes.len() as u64
    {
        let _ = fs::write(&dark_path, dark_bytes);
    }

    let target_icon_path = if light { light_path } else { dark_path };
    let wide_icon: Vec<u16> = target_icon_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut candidate_dirs = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        candidate_dirs
            .push(PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    if let Some(programdata) = std::env::var_os("ProgramData") {
        candidate_dirs
            .push(PathBuf::from(programdata).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        candidate_dirs.push(PathBuf::from(userprofile).join("Desktop"));
    }

    let mut shortcut_paths = Vec::new();
    for dir in candidate_dirs {
        if !dir.is_dir() {
            continue;
        }
        for name in &["avel.lnk", "Avel.lnk"] {
            let p = dir.join(name);
            if p.is_file() {
                shortcut_paths.push(p);
            }
            let sub = dir.join("Avel").join(name);
            if sub.is_file() {
                shortcut_paths.push(sub);
            }
        }
    }

    if shortcut_paths.is_empty() {
        return;
    }

    let mut any_updated = false;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        for lnk_path in shortcut_paths {
            let link: Result<IShellLinkW, _> =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER);
            if let Ok(link) = link {
                if let Ok(persist) = link.cast::<IPersistFile>() {
                    let wide_path: Vec<u16> = lnk_path
                        .as_os_str()
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();

                    if persist
                        .Load(PCWSTR(wide_path.as_ptr()), STGM_READWRITE)
                        .is_ok()
                    {
                        let _ = link.SetIconLocation(PCWSTR(wide_icon.as_ptr()), 0);
                        if persist.Save(PCWSTR(wide_path.as_ptr()), true).is_ok() {
                            any_updated = true;
                        }
                    }
                }
            }
        }

        if any_updated {
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
        }
    }
}
