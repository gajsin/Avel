pub mod brightness;
pub mod clipboard_listener;
pub mod commands;
pub mod db;
pub mod desktop;
pub mod hotkeys;
pub mod recent;
pub mod screenshots;
pub mod theme;
pub mod thumbnails;
pub mod types;
pub mod win32_clipboard;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::brightness::{
    brightness_adjust, brightness_cancel_test, brightness_confirm_test, brightness_get_diagnostics,
    brightness_get_preferences, brightness_get_state, brightness_refresh,
    brightness_save_preferences, brightness_set, brightness_set_group, brightness_start_test,
    BrightnessEventWatcher, BrightnessService, NativeBrightnessBackend,
};
use crate::clipboard_listener::{start_clipboard_listener, ClipboardState};
use crate::commands::*;
use crate::db::Db;
use crate::desktop::DesktopManager;
use crate::hotkeys::register_all_hotkeys;
use crate::recent::RecentWatcher;
use crate::screenshots::ScreenshotWatcher;
use crate::theme::{read_registry_theme, update_system_icons, ThemeCoordinator};
use crate::thumbnails::ThumbnailService;

fn get_tray_icon(is_light_system: bool) -> Option<Image<'static>> {
    let bytes: &'static [u8] = if is_light_system {
        include_bytes!("../icons/tray-light.png")
    } else {
        include_bytes!("../icons/tray-dark.png")
    };
    Image::from_bytes(bytes).ok()
}

fn show_main_window(app: &AppHandle, section: Option<&str>) {
    if let Some(win) = app.get_webview_window("main") {
        let db = app.state::<Arc<Db>>();
        if let Some(sec) = section {
            if let Ok(mut s) = db.get_settings() {
                s.last_section = sec.to_string();
                let _ = db.save_settings(&s);
            }
            let _ = app.emit("navigate-section", sec);
        }
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app, None);
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--minimized"])
                .build(),
        )
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("avel".into()),
                    }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(state) = app.try_state::<Arc<hotkeys::HotkeyState>>() {
                            state.handle_event(app, shortcut);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                dirs::data_local_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("Avel")
            });

            let db = Arc::new(
                Db::new(&app_data_dir).expect("Failed to initialize Avel SQLite database"),
            );

            let thumbnail_service = Arc::new(ThumbnailService::new(&app_data_dir));
            let desktop_manager = Arc::new(DesktopManager::new());
            let theme_coordinator = Arc::new(ThemeCoordinator::new());
            let clip_state = Arc::new(ClipboardState::new());
            let screenshot_watcher = Arc::new(ScreenshotWatcher::new());
            let recent_watcher = Arc::new(RecentWatcher::new());

            let brightness_backend = Arc::new(NativeBrightnessBackend::new());
            let brightness_service =
                BrightnessService::new(db.clone(), brightness_backend, Some(app.handle().clone()));
            let brightness_watcher = Arc::new(BrightnessEventWatcher::new());

            let hotkey_state = Arc::new(hotkeys::HotkeyState::new());

            // 1. Startup Recovery: Restore icons/taskbars if leftover marker exists from previous crash
            desktop_manager.restore_all_on_startup_or_exit(&db);

            // 2. Manage state in Tauri
            app.manage(db.clone());
            app.manage(thumbnail_service.clone());
            app.manage(desktop_manager.clone());
            app.manage(theme_coordinator.clone());
            app.manage(clip_state.clone());
            app.manage(screenshot_watcher.clone());
            app.manage(recent_watcher.clone());
            app.manage(brightness_service.clone());
            app.manage(brightness_watcher.clone());
            app.manage(hotkey_state.clone());

            let app_handle = app.handle().clone();

            // 3. Show Window Immediately (Before heavy background indexing)
            if let Some(win) = app.get_webview_window("main") {
                let is_minimized_arg = std::env::args().any(|arg| arg == "--minimized");
                if !is_minimized_arg {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }

            // 4. Start background native services asynchronously
            start_clipboard_listener(app_handle.clone(), clip_state.clone());
            screenshot_watcher.start(app_handle.clone(), None);
            recent_watcher.start(app_handle.clone());
            theme_coordinator.start_scheduler(app_handle.clone());
            brightness_watcher.start(brightness_service.clone());
            brightness_service.refresh_async();
            register_all_hotkeys(&app_handle);

            // 5. Setup Tray Menu
            let settings = db.get_settings().unwrap_or_default();
            let is_en = settings.language == "en";
            let tray_menu = create_tray_menu(app.handle(), is_en)?;

            let is_light_system = read_registry_theme();
            let tray_builder = if let Some(icon) = get_tray_icon(is_light_system) {
                TrayIconBuilder::with_id("avel-tray").icon(icon)
            } else if let Some(icon) = app.default_window_icon().cloned() {
                TrayIconBuilder::with_id("avel-tray").icon(icon)
            } else {
                TrayIconBuilder::with_id("avel-tray")
            };

            let _tray = tray_builder
                .menu(&tray_menu)
                .tooltip("Avel")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_main_window(app, None),
                    "clip" => show_main_window(app, Some("clipboard")),
                    "rec" => show_main_window(app, Some("recent")),
                    "bright" => show_main_window(app, Some("brightness")),
                    "theme" => {
                        let current_is_light = read_registry_theme();
                        let target_light = !current_is_light;
                        let coordinator = app.state::<Arc<ThemeCoordinator>>();
                        let db = app.state::<Arc<Db>>();
                        let cfg = db.get_appearance_config().unwrap_or_default();
                        let wp = if target_light {
                            cfg.wallpaper_light_path
                        } else {
                            cfg.wallpaper_dark_path
                        };
                        let _ = coordinator.apply_theme(
                            app,
                            target_light,
                            cfg.switch_wallpaper,
                            Some(wp),
                        );
                    }
                    "desk" => {
                        let db = app.state::<Arc<Db>>();
                        let desktop = app.state::<Arc<DesktopManager>>();
                        if let Ok(state) = desktop.toggle_mode(&db) {
                            let _ = app.emit("desktop-state-changed", &state);
                        }
                    }
                    "exit" => {
                        let db = app.state::<Arc<Db>>();
                        let desktop = app.state::<Arc<DesktopManager>>();
                        desktop.restore_all_on_startup_or_exit(&db);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        show_main_window(app, None);
                    }
                })
                .build(app)?;

            update_system_icons(app.handle(), is_light_system);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_view_mode,
            set_view_mode,
            get_clipboard_items,
            delete_clipboard_item,
            delete_clipboard_items,
            clear_clipboard_history,
            copy_clipboard_text,
            copy_clipboard_image,
            copy_clipboard_files,
            open_external_url,
            get_recent_items,
            open_recent_item,
            delete_recent_item,
            clear_recent_history,
            get_appearance_config,
            save_appearance_config,
            get_wallpaper_thumbnail,
            get_clean_desktop_state,
            set_clean_desktop_mode,
            update_global_hotkey_spec,
            get_hotkey_statuses,
            toggle_pin_clipboard_item,
            toggle_pin_recent_item,
            minimize_window,
            hide_window,
            brightness_get_state,
            brightness_refresh,
            brightness_set,
            brightness_adjust,
            brightness_set_group,
            brightness_start_test,
            brightness_cancel_test,
            brightness_confirm_test,
            brightness_get_preferences,
            brightness_save_preferences,
            brightness_get_diagnostics
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Avel");
}

pub fn create_tray_menu(
    app: &AppHandle,
    is_en: bool,
) -> Result<tauri::menu::Menu<tauri::Wry>, tauri::Error> {
    let (open_text, clip_text, rec_text, bright_text, theme_text, desk_text, exit_text) = if is_en {
        (
            "Open Avel",
            "Clipboard",
            "Recent",
            "Brightness",
            "Toggle Theme",
            "Toggle Desktop",
            "Exit",
        )
    } else {
        (
            "Открыть Avel",
            "Буфер",
            "Недавнее",
            "Яркость",
            "Переключить тему",
            "Скрыть/показать рабочий стол",
            "Выход",
        )
    };

    let item_open = MenuItemBuilder::with_id("open", open_text).build(app)?;
    let item_clip = MenuItemBuilder::with_id("clip", clip_text).build(app)?;
    let item_rec = MenuItemBuilder::with_id("rec", rec_text).build(app)?;
    let item_bright = MenuItemBuilder::with_id("bright", bright_text).build(app)?;
    let item_theme = MenuItemBuilder::with_id("theme", theme_text).build(app)?;
    let item_desk = MenuItemBuilder::with_id("desk", desk_text).build(app)?;
    let item_exit = MenuItemBuilder::with_id("exit", exit_text).build(app)?;

    MenuBuilder::new(app)
        .item(&item_open)
        .separator()
        .item(&item_clip)
        .item(&item_rec)
        .item(&item_bright)
        .separator()
        .item(&item_theme)
        .item(&item_desk)
        .separator()
        .item(&item_exit)
        .build()
}
