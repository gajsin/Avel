use avel_lib::db::Db;
use avel_lib::thumbnails::ThumbnailService;
use image::{ImageBuffer, Rgba};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use avel_lib::desktop::DesktopBackend;
use avel_lib::brightness::{BrightnessDeviceBackend, BrightnessDisplay, BrightnessError};

#[derive(Default)]
struct MockDesktopBackend {
    icons_hidden: AtomicBool,
    taskbar_hidden: AtomicBool,
    should_fail_icons: AtomicBool,
    should_fail_taskbar: AtomicBool,
}

impl MockDesktopBackend {
    fn new() -> Self {
        Self::default()
    }
}

impl DesktopBackend for MockDesktopBackend {
    fn are_desktop_icons_hidden(&self) -> Result<bool, String> {
        Ok(self.icons_hidden.load(Ordering::SeqCst))
    }

    fn set_desktop_icons_hidden(&self, hidden: bool) -> Result<(), String> {
        if self.should_fail_icons.load(Ordering::SeqCst) {
            return Err("Mock desktop icons failure".to_string());
        }
        self.icons_hidden.store(hidden, Ordering::SeqCst);
        Ok(())
    }

    fn is_primary_taskbar_hidden(&self) -> Result<bool, String> {
        Ok(self.taskbar_hidden.load(Ordering::SeqCst))
    }

    fn set_taskbars_hidden(&self, hidden: bool) -> Result<(), String> {
        if self.should_fail_taskbar.load(Ordering::SeqCst) {
            return Err("Mock taskbar failure".to_string());
        }
        self.taskbar_hidden.store(hidden, Ordering::SeqCst);
        Ok(())
    }
}

struct FakeBrightnessBackend {
    current_raw: Mutex<u32>,
    min_raw: u32,
    max_raw: u32,
    should_fail_write: Mutex<bool>,
    should_fail_read: Mutex<bool>,
    write_call_count: Mutex<usize>,
}

impl FakeBrightnessBackend {
    fn new(min: u32, initial: u32, max: u32) -> Self {
        Self {
            current_raw: Mutex::new(initial),
            min_raw: min,
            max_raw: max,
            should_fail_write: Mutex::new(false),
            should_fail_read: Mutex::new(false),
            write_call_count: Mutex::new(0),
        }
    }
}

impl BrightnessDeviceBackend for FakeBrightnessBackend {
    fn read_brightness(
        &self,
        _display: &BrightnessDisplay,
    ) -> Result<(u32, u32, u32, Option<Vec<u32>>), BrightnessError> {
        if *self.should_fail_read.lock().unwrap() {
            return Err(BrightnessError {
                code: "fake_read_failure".to_string(),
                native_code: None,
                operation: "fake_read".to_string(),
                retryable: true,
            });
        }
        let cur = *self.current_raw.lock().unwrap();
        Ok((self.min_raw, cur, self.max_raw, None))
    }

    fn write_brightness(
        &self,
        _display: &BrightnessDisplay,
        raw_value: u32,
    ) -> Result<(), BrightnessError> {
        if *self.should_fail_write.lock().unwrap() {
            return Err(BrightnessError {
                code: "fake_write_failure".to_string(),
                native_code: None,
                operation: "fake_write".to_string(),
                retryable: true,
            });
        }
        *self.write_call_count.lock().unwrap() += 1;
        *self.current_raw.lock().unwrap() = raw_value.clamp(self.min_raw, self.max_raw);
        Ok(())
    }
}

#[test]
fn test_database_initialization_and_settings() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let settings = db.get_settings().expect("get settings");
    assert_eq!(settings.app_theme, "System");
    assert_eq!(settings.language, "ru");
    assert_eq!(
        settings.hotkey_clipboard,
        r#"{"code":"Equal","ctrl":false,"alt":false,"shift":false,"meta":false}"#
    );
    assert_eq!(
        settings.hotkey_recent,
        r#"{"code":"F8","ctrl":false,"alt":false,"shift":false,"meta":false}"#
    );
    assert_eq!(
        settings.hotkey_appearance,
        r#"{"code":"KeyD","ctrl":true,"alt":true,"shift":false,"meta":false}"#
    );
    assert_eq!(
        settings.hotkey_desktop,
        r#"{"code":"ArrowDown","ctrl":false,"alt":false,"shift":false,"meta":false}"#
    );

    // Update settings
    let mut modified = settings.clone();
    modified.language = "en".to_string();
    modified.app_theme = "Dark".to_string();
    modified.hotkey_clipboard = "Ctrl+Shift+V".to_string();
    db.save_settings(&modified).expect("save settings");

    let updated = db.get_settings().expect("get updated settings");
    assert_eq!(updated.language, "en");
    assert_eq!(updated.app_theme, "Dark");
    assert_eq!(updated.hotkey_clipboard, "Ctrl+Shift+V");
}

#[test]
fn test_view_mode_persistence() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    assert_eq!(db.get_view_mode("clipboard_all").unwrap(), "details");

    db.set_view_mode("clipboard_all", "grid")
        .expect("set view mode");
    assert_eq!(db.get_view_mode("clipboard_all").unwrap(), "grid");

    db.set_view_mode("clipboard_text", "details")
        .expect("set view mode");
    assert_eq!(db.get_view_mode("clipboard_text").unwrap(), "details");
    assert_eq!(db.get_view_mode("clipboard_all").unwrap(), "grid");
}

#[test]
fn test_clipboard_deduplication_and_filtering() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let hash1 = "hash_hello_world";
    let item1 = db
        .insert_or_update_clipboard_item(
            "text",
            Some("Hello World"),
            None,
            None,
            Some(11),
            None,
            None,
            hash1,
        )
        .expect("insert item 1");

    assert_eq!(item1.text_content.as_deref(), Some("Hello World"));

    // Insert image item
    let hash_img = "hash_image_123";
    let img_item = db
        .insert_or_update_clipboard_item(
            "image",
            None,
            Some("C:\\images\\test.png"),
            Some("data:image/jpeg;base64,..."),
            None,
            Some(1920),
            Some(1080),
            hash_img,
        )
        .expect("insert image");

    assert_eq!(img_item.content_type, "image");

    // Re-insert same text -> should update timestamp and deduplicate
    std::thread::sleep(std::time::Duration::from_millis(50));
    let item1_again = db
        .insert_or_update_clipboard_item(
            "text",
            Some("Hello World"),
            None,
            None,
            Some(11),
            None,
            None,
            hash1,
        )
        .expect("update item 1");

    assert_eq!(item1_again.id, item1.id);

    // List all
    let all = db.get_clipboard_items(Some("all"), None).expect("get all");
    assert_eq!(all.len(), 2);
    // Most recently updated item is first
    assert_eq!(all[0].id, item1.id);

    // Filter text
    let texts = db
        .get_clipboard_items(Some("text"), None)
        .expect("get texts");
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].text_content.as_deref(), Some("Hello World"));

    // Filter images
    let imgs = db
        .get_clipboard_items(Some("images"), None)
        .expect("get images");
    assert_eq!(imgs.len(), 1);

    // Search query
    let searched = db
        .get_clipboard_items(Some("all"), Some("Hello"))
        .expect("search");
    assert_eq!(searched.len(), 1);

    let searched_empty = db
        .get_clipboard_items(Some("all"), Some("NonExistent"))
        .expect("search empty");
    assert_eq!(searched_empty.len(), 0);

    // Delete item (uses batch delete under the hood)
    db.delete_clipboard_item(item1.id).expect("delete item 1");
    let after_del = db.get_clipboard_items(Some("all"), None).expect("get all");
    assert_eq!(after_del.len(), 1);

    // Batch delete
    db.delete_clipboard_items(&[img_item.id])
        .expect("batch delete");
    let after_batch_del = db.get_clipboard_items(Some("all"), None).expect("get all");
    assert_eq!(after_batch_del.len(), 0);

    // Clear history
    db.clear_clipboard_history().expect("clear history");
    let after_clear = db.get_clipboard_items(Some("all"), None).expect("get all");
    assert_eq!(after_clear.len(), 0);
}

#[test]
fn test_recent_items_and_grouping() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let r1 = db
        .insert_or_update_recent_item("file", "Report.docx", "C:\\Docs\\Report.docx", None)
        .expect("insert r1");

    assert_eq!(r1.title, "Report.docx");

    let r2 = db
        .insert_or_update_recent_item("folder", "Projects", "C:\\Dev\\Projects", None)
        .expect("insert r2");

    assert_eq!(r2.entry_type, "folder");

    let all_recent = db.get_recent_items(Some("all"), None).expect("get recent");
    assert_eq!(all_recent.len(), 2);

    let files_only = db.get_recent_items(Some("files"), None).expect("get files");
    assert_eq!(files_only.len(), 1);
    assert_eq!(files_only[0].title, "Report.docx");

    let folders_only = db
        .get_recent_items(Some("folders"), None)
        .expect("get folders");
    assert_eq!(folders_only.len(), 1);
    assert_eq!(folders_only[0].title, "Projects");

    // Search
    let search_res = db
        .get_recent_items(Some("all"), Some("Proj"))
        .expect("search recent");
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].title, "Projects");

    // Delete
    db.delete_recent_item(r1.id).expect("delete recent");
    assert_eq!(db.get_recent_items(Some("all"), None).unwrap().len(), 1);

    // Clear
    db.clear_recent_history().expect("clear recent");
    assert_eq!(db.get_recent_items(Some("all"), None).unwrap().len(), 0);
}

#[test]
fn test_appearance_config_and_recovery_marker() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let initial_app = db.get_appearance_config().expect("get appearance");
    assert_eq!(initial_app.windows_theme_target, "System");
    assert!(!initial_app.schedule_enabled);

    let mut modified = initial_app;
    modified.windows_theme_target = "Dark".to_string();
    modified.schedule_enabled = true;
    modified.schedule_light_time = "08:00".to_string();
    modified.schedule_dark_time = "20:00".to_string();
    modified.switch_wallpaper = true;
    modified.wallpaper_dark_path = "C:\\Wallpapers\\dark.jpg".to_string();

    db.save_appearance_config(&modified)
        .expect("save appearance");
    let saved_app = db.get_appearance_config().expect("get saved appearance");
    assert_eq!(saved_app.windows_theme_target, "Dark");
    assert!(saved_app.schedule_enabled);
    assert_eq!(saved_app.wallpaper_dark_path, "C:\\Wallpapers\\dark.jpg");

    // Clean desktop state and recovery marker
    db.save_clean_desktop_state("all", true)
        .expect("save clean state");
    let clean_st = db.get_clean_desktop_state().expect("get clean state");
    assert_eq!(clean_st.current_mode, "all");
    assert!(clean_st.is_hidden);

    db.set_recovery_marker(true).expect("set marker");
    let marker = db.get_recovery_marker().expect("get marker");
    assert!(marker);
}

#[test]
fn test_thumbnail_service_generation_and_cache() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let thumb_service = ThumbnailService::new(temp_dir.path());

    // Create a dummy 400x300 PNG image
    let img_path = temp_dir.path().join("sample.png");
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(400, 300, Rgba([21, 212, 87, 255]));
    img.save(&img_path).expect("save sample image");

    // Generate thumbnail for 80x60 @ 2x DPR
    let b64_1 = thumb_service
        .get_or_generate_thumbnail(&img_path.to_string_lossy(), 80, 60, 2.0)
        .expect("generate thumbnail");

    assert!(b64_1.starts_with("data:image/png;base64,"));

    // Second call should return cached result instantly
    let b64_2 = thumb_service
        .get_or_generate_thumbnail(&img_path.to_string_lossy(), 80, 60, 2.0)
        .expect("get cached thumbnail");

    assert_eq!(b64_1, b64_2);
}

#[test]
fn test_hotkeys_shortcut_spec_and_code_parsing() {
    use avel_lib::hotkeys::{parse_code, parse_shortcut_spec_or_str, spec_to_shortcut};
    use avel_lib::types::ShortcutSpec;

    // Single keys without modifiers
    assert!(parse_code("ArrowUp").is_ok());
    assert!(parse_code("ArrowDown").is_ok());
    assert!(parse_code("ArrowLeft").is_ok());
    assert!(parse_code("ArrowRight").is_ok());
    assert!(parse_code("F8").is_ok());
    assert!(parse_code("Equal").is_ok());
    assert!(parse_code("=").is_ok());
    assert!(parse_code("KeyD").is_ok());
    assert!(parse_code("Numpad1").is_ok());

    let spec_arrow_up = ShortcutSpec {
        code: "ArrowUp".to_string(),
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
    };
    assert!(spec_to_shortcut(&spec_arrow_up).is_ok());

    let spec_f8 = ShortcutSpec {
        code: "F8".to_string(),
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
    };
    assert!(spec_to_shortcut(&spec_f8).is_ok());

    let spec_equal = ShortcutSpec {
        code: "Equal".to_string(),
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
    };
    assert!(spec_to_shortcut(&spec_equal).is_ok());

    // Combos: Ctrl + Alt + KeyD
    let spec_ctrl_alt_d = ShortcutSpec {
        code: "KeyD".to_string(),
        ctrl: true,
        alt: true,
        shift: false,
        meta: false,
    };
    assert!(spec_to_shortcut(&spec_ctrl_alt_d).is_ok());

    // Combos: Ctrl + Shift + KeyV
    let spec_ctrl_shift_v = ShortcutSpec {
        code: "KeyV".to_string(),
        ctrl: true,
        alt: false,
        shift: true,
        meta: false,
    };
    assert!(spec_to_shortcut(&spec_ctrl_shift_v).is_ok());

    // JSON serialization / deserialization roundtrip
    let json = serde_json::to_string(&spec_ctrl_alt_d).expect("serialize spec");
    let parsed_spec = parse_shortcut_spec_or_str(&json, "KeyD");
    assert_eq!(parsed_spec, spec_ctrl_alt_d);
    let parsed_legacy = parse_shortcut_spec_or_str("Ctrl+Alt+D", "KeyD");
    assert_eq!(parsed_legacy.code, "KeyD");
    assert!(parsed_legacy.ctrl);
    assert!(parsed_legacy.alt);

    // Canonicalization of F8, ArrowDown, PageUp, Ctrl+Alt+D
    let parsed_f8 = parse_shortcut_spec_or_str("f8", "");
    assert_eq!(parsed_f8.code, "F8");
    assert!(!parsed_f8.ctrl && !parsed_f8.alt && !parsed_f8.shift && !parsed_f8.meta);

    let parsed_f9 = parse_shortcut_spec_or_str("f9", "");
    assert_eq!(parsed_f9.code, "F9");

    let parsed_f10 = parse_shortcut_spec_or_str("F10", "");
    assert_eq!(parsed_f10.code, "F10");

    let parsed_left = parse_shortcut_spec_or_str("ArrowLeft", "");
    assert_eq!(parsed_left.code, "ArrowLeft");

    let parsed_left_alias = parse_shortcut_spec_or_str("left", "");
    assert_eq!(parsed_left_alias.code, "ArrowLeft");

    let parsed_down = parse_shortcut_spec_or_str("arrowdown", "");
    assert_eq!(parsed_down.code, "ArrowDown");
    assert!(!parsed_down.ctrl && !parsed_down.alt);

    let parsed_pageup = parse_shortcut_spec_or_str("pageup", "");
    assert_eq!(parsed_pageup.code, "PageUp");

    let parsed_cad = parse_shortcut_spec_or_str("ctrl+alt+d", "");
    assert_eq!(parsed_cad.code, "KeyD");
    assert!(parsed_cad.ctrl && parsed_cad.alt);

    // JSON with missing modifiers defaults to false
    let json_partial = r#"{"code":"F8"}"#;
    let parsed_partial = parse_shortcut_spec_or_str(json_partial, "");
    assert_eq!(parsed_partial.code, "F8");
    assert!(!parsed_partial.ctrl && !parsed_partial.alt);

    // Unset values must NOT invent fake Ctrl+Alt fallbacks
    let unset_empty = parse_shortcut_spec_or_str("", "");
    assert_eq!(unset_empty.code, "");
    assert!(!unset_empty.ctrl && !unset_empty.alt);

    let unset_none = parse_shortcut_spec_or_str("none", "");
    assert_eq!(unset_none.code, "");
    assert!(!unset_none.ctrl && !unset_none.alt);

    // Test update_hotkey_in_db persistence
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");
    avel_lib::hotkeys::update_hotkey_in_db(
        &db,
        avel_lib::hotkeys::HotkeyAction::Clipboard,
        r#"{"code":"KeyV","ctrl":true,"alt":true,"shift":false,"meta":false}"#,
    )
    .expect("update clipboard hotkey in db");
    assert_eq!(
        db.get_settings().unwrap().hotkey_clipboard,
        r#"{"code":"KeyV","ctrl":true,"alt":true,"shift":false,"meta":false}"#
    );
}

#[test]
fn test_hotkey_state_first_event_resolution_never_dropped() {
    use avel_lib::hotkeys::{HotkeyAction, HotkeyState};
    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

    let state = HotkeyState::new();

    // 1. Equal -> Clipboard
    let sc_equal = Shortcut::new(None, Code::Equal);
    state
        .shortcut_routes
        .lock()
        .unwrap()
        .insert(sc_equal, HotkeyAction::Clipboard);
    state
        .id_routes
        .lock()
        .unwrap()
        .insert(sc_equal.id(), HotkeyAction::Clipboard);

    // 2. F8 -> Recent
    let sc_f8 = Shortcut::new(None, Code::F8);
    state
        .shortcut_routes
        .lock()
        .unwrap()
        .insert(sc_f8, HotkeyAction::Recent);
    state
        .id_routes
        .lock()
        .unwrap()
        .insert(sc_f8.id(), HotkeyAction::Recent);

    // 3. Ctrl+Alt+D -> Appearance
    let sc_d = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD);
    state
        .shortcut_routes
        .lock()
        .unwrap()
        .insert(sc_d, HotkeyAction::Appearance);
    state
        .id_routes
        .lock()
        .unwrap()
        .insert(sc_d.id(), HotkeyAction::Appearance);

    // 4. ArrowDown -> Desktop
    let sc_down = Shortcut::new(None, Code::ArrowDown);
    state
        .shortcut_routes
        .lock()
        .unwrap()
        .insert(sc_down, HotkeyAction::Desktop);
    state
        .id_routes
        .lock()
        .unwrap()
        .insert(sc_down.id(), HotkeyAction::Desktop);

    // Immediate first-event resolutions (MUST NEVER BE DROPPED OR RETURN NONE)
    assert_eq!(
        state.resolve_action(&sc_equal),
        Some(HotkeyAction::Clipboard)
    );
    assert_eq!(state.resolve_action(&sc_f8), Some(HotkeyAction::Recent));
    assert_eq!(state.resolve_action(&sc_d), Some(HotkeyAction::Appearance));
    assert_eq!(state.resolve_action(&sc_down), Some(HotkeyAction::Desktop));

    // Rapid successive resolutions (no throttle dropping)
    for _ in 0..10 {
        assert_eq!(
            state.resolve_action(&sc_equal),
            Some(HotkeyAction::Clipboard)
        );
        assert_eq!(state.resolve_action(&sc_f8), Some(HotkeyAction::Recent));
        assert_eq!(state.resolve_action(&sc_d), Some(HotkeyAction::Appearance));
        assert_eq!(state.resolve_action(&sc_down), Some(HotkeyAction::Desktop));
    }
}

#[test]
fn test_screenshot_dedup_removes_orphan_png() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let sample_img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(1920, 1080, Rgba([10, 20, 30, 255]));

    let rel_png = "test_image.png";
    let owned_png_path = db.images_dir.join(rel_png);
    sample_img.save(&owned_png_path).expect("save owned png");
    assert!(owned_png_path.exists());

    // 1. First arrives clipboard image
    let img_item = db
        .insert_or_update_clipboard_item(
            "image",
            None,
            Some(rel_png),
            None,
            None,
            Some(1920),
            Some(1080),
            "hash_123",
        )
        .expect("insert image");
    assert_eq!(img_item.content_type, "image");

    // 2. Shortly arrives screenshot with same width, height and matching pixels
    let screenshot_file = temp_dir.path().join("Screenshot_1.png");
    sample_img.save(&screenshot_file).expect("save screenshot");
    let screenshot_abs = screenshot_file.to_string_lossy().to_string();

    let merged = db
        .insert_or_update_clipboard_item(
            "screenshot",
            None,
            Some(&screenshot_abs),
            None,
            None,
            Some(1920),
            Some(1080),
            "hash_screenshot_456",
        )
        .expect("merge screenshot");

    assert_eq!(merged.content_type, "screenshot");
    assert_eq!(merged.image_path.as_deref(), Some(screenshot_abs.as_str()));
    // The old owned PNG in images_dir must have been deleted!
    assert!(
        !owned_png_path.exists(),
        "Old owned PNG must be removed upon screenshot merge"
    );

    // 3. Reverse case: screenshot exists, image arrives
    let rel_png2 = "test_image2.png";
    let owned_png_path2 = db.images_dir.join(rel_png2);
    sample_img.save(&owned_png_path2).expect("save owned png 2");
    assert!(owned_png_path2.exists());

    let dedup_img = db
        .insert_or_update_clipboard_item(
            "image",
            None,
            Some(rel_png2),
            None,
            None,
            Some(1920),
            Some(1080),
            "hash_image_789",
        )
        .expect("dedup image");

    assert_eq!(dedup_img.content_type, "screenshot");
    assert_eq!(
        dedup_img.image_path.as_deref(),
        Some(screenshot_abs.as_str())
    );
    // The newly created image PNG must be deleted since screenshot already exists
    assert!(
        !owned_png_path2.exists(),
        "Newly saved image PNG must be deleted when screenshot exists"
    );
}

#[test]
fn test_clean_desktop_state_persistence_preserves_mode() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    db.save_clean_desktop_state("icons", true)
        .expect("save state");
    let st1 = db.get_clean_desktop_state().unwrap();
    assert_eq!(st1.current_mode, "icons");
    assert!(st1.is_hidden);

    // Hotkey restore sets is_hidden to false while preserving "icons"
    db.save_clean_desktop_state(&st1.current_mode, false)
        .expect("restore desktop");
    let st2 = db.get_clean_desktop_state().unwrap();
    assert_eq!(st2.current_mode, "icons");
    assert!(!st2.is_hidden);
}

#[test]
fn test_restore_all_on_startup_or_exit_preserves_mode() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");
    let desktop = avel_lib::desktop::DesktopManager::with_backend(Box::new(
        MockDesktopBackend::new(),
    ));

    // User selected "taskbar" mode and desktop was hidden
    db.save_clean_desktop_state("taskbar", true)
        .expect("save state");
    db.set_recovery_marker(true).expect("set marker");

    desktop.restore_all_on_startup_or_exit(&db);

    let state = db.get_clean_desktop_state().unwrap();
    assert_eq!(state.current_mode, "taskbar");
    assert!(!state.is_hidden);

    let marker = db.get_recovery_marker().unwrap();
    assert!(!marker);
}

#[test]
fn test_desktop_manager_backend_isolation_and_restoration() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");
    let desktop = avel_lib::desktop::DesktopManager::with_backend(Box::new(
        MockDesktopBackend::new(),
    ));

    // Apply "all" mode
    let st1 = desktop.apply_mode(&db, "all").expect("apply all mode");
    assert!(st1.is_hidden);
    assert!(st1.actual_icons_hidden);
    assert!(st1.actual_taskbar_hidden);

    // Apply "none" (restore)
    let st2 = desktop.apply_mode(&db, "none").expect("apply none mode");
    assert!(!st2.is_hidden);
    assert!(!st2.actual_icons_hidden);
    assert!(!st2.actual_taskbar_hidden);
}

#[test]
fn test_brightness_db_migration_and_preferences() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    // Default preferences
    let prefs = db
        .get_brightness_preferences()
        .expect("get default brightness prefs");
    assert!(!prefs.sync_group);
    assert_eq!(prefs.step_percent, 5);
    assert_eq!(prefs.hotkey_brightness_target, "cursor");

    // Modify and save preferences
    let mut modified = prefs.clone();
    modified.sync_group = true;
    modified.step_percent = 10;
    modified.hotkey_brightness_target = "group".to_string();
    modified.hotkey_brightness_up = Some("Ctrl+Shift+PageUp".to_string());
    db.save_brightness_preferences(&modified)
        .expect("save brightness prefs");

    let updated = db
        .get_brightness_preferences()
        .expect("get updated brightness prefs");
    assert!(updated.sync_group);
    assert_eq!(updated.step_percent, 10);
    assert_eq!(updated.hotkey_brightness_target, "group");
    assert_eq!(
        updated.hotkey_brightness_up.as_deref(),
        Some("Ctrl+Shift+PageUp")
    );

    // Monitor preferences
    let mon_pref = avel_lib::brightness::BrightnessMonitorPreference {
        display_key: "MONITOR\\TEST001".to_string(),
        preferred_backend: Some(avel_lib::brightness::BrightnessBackend::DdcHigh),
        last_verification_state: avel_lib::brightness::VerificationState::UserConfirmed,
        last_verification_connection: Some("DisplayPort".to_string()),
        custom_name: Some("Custom Monitor".to_string()),
    };
    db.save_monitor_preference(&mon_pref)
        .expect("save monitor pref");

    let loaded_map = db.get_monitor_preferences().expect("get monitor prefs");
    assert_eq!(loaded_map.len(), 1);
    let loaded_item = loaded_map
        .iter()
        .find(|p| p.display_key == "MONITOR\\TEST001")
        .expect("found monitor");
    assert_eq!(
        loaded_item.last_verification_state,
        avel_lib::brightness::VerificationState::UserConfirmed
    );
    assert_eq!(loaded_item.custom_name.as_deref(), Some("Custom Monitor"));

    // Test Recovery Record
    let recovery = avel_lib::brightness::BrightnessTestRecoveryRecord {
        test_id: "test_42".to_string(),
        display_key: "MONITOR\\TEST001".to_string(),
        connection_key: "DisplayPort".to_string(),
        backend: "ddc_high".to_string(),
        original_raw: 80,
        test_raw: 70,
        stage: "testing".to_string(),
        created_at: "2026-09-08T12:00:00Z".to_string(),
    };
    db.set_brightness_recovery(&recovery).expect("set recovery");
    let fetched = db
        .get_brightness_recovery("MONITOR\\TEST001")
        .expect("get recovery")
        .expect("must exist");
    assert_eq!(fetched.display_key, "MONITOR\\TEST001");
    assert_eq!(fetched.test_id, "test_42");
    assert_eq!(fetched.original_raw, 80);

    // Mismatched test_id must NOT delete newer record
    db.delete_brightness_recovery_for_test("MONITOR\\TEST001", "old_stale_test")
        .expect("delete with wrong test_id");
    assert!(db
        .get_brightness_recovery("MONITOR\\TEST001")
        .expect("must still exist")
        .is_some());

    // Matching test_id successfully deletes record
    db.delete_brightness_recovery_for_test("MONITOR\\TEST001", "test_42")
        .expect("delete with matching test_id");
    assert!(db
        .get_brightness_recovery("MONITOR\\TEST001")
        .expect("get recovery after delete")
        .is_none());
}

#[test]
fn test_brightness_range_normalization() {
    // Standard 0-100
    assert_eq!(avel_lib::brightness::percent_to_raw(0, 0, 100), 0);
    assert_eq!(avel_lib::brightness::percent_to_raw(50, 0, 100), 50);
    assert_eq!(avel_lib::brightness::percent_to_raw(100, 0, 100), 100);
    assert_eq!(avel_lib::brightness::raw_to_percent(50, 0, 100), 50);

    // Custom range (e.g. 0-255)
    assert_eq!(avel_lib::brightness::percent_to_raw(0, 0, 255), 0);
    assert_eq!(avel_lib::brightness::percent_to_raw(100, 0, 255), 255);
    let mid_raw = avel_lib::brightness::percent_to_raw(50, 0, 255);
    assert_eq!(mid_raw, 128);
    assert_eq!(avel_lib::brightness::raw_to_percent(mid_raw, 0, 255), 50);

    // Offset range (e.g. 20-80)
    assert_eq!(avel_lib::brightness::percent_to_raw(0, 20, 80), 20);
    assert_eq!(avel_lib::brightness::percent_to_raw(100, 20, 80), 80);
    assert_eq!(avel_lib::brightness::percent_to_raw(50, 20, 80), 50);
    assert_eq!(avel_lib::brightness::raw_to_percent(50, 20, 80), 50);
}

#[test]
fn test_fake_brightness_backend_and_service() {
    use avel_lib::brightness::BrightnessDeviceBackend;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = std::sync::Arc::new(Db::new(temp_dir.path()).expect("init db"));

    let backend = std::sync::Arc::new(FakeBrightnessBackend::new(0, 60, 100));

    let display_1 = avel_lib::brightness::BrightnessDisplay {
        id: "display-1".to_string(),
        generation: 1,
        name: "Dell UltraSharp 27".to_string(),
        connection: Some("DisplayPort".to_string()),
        is_primary: true,
        backend: Some(avel_lib::brightness::BrightnessBackend::DdcHigh),
        probe_state: avel_lib::brightness::ProbeState::ReadOk,
        verification: avel_lib::brightness::VerificationState::UserConfirmed,
        current_percent: Some(60),
        target_percent: None,
        raw_min: Some(0),
        raw_max: Some(100),
        available_levels: None,
        busy: false,
        error: None,
        device_name: None,
    };

    // Test FakeBrightnessBackend read and write
    let (min, cur, max, levels) = backend.read_brightness(&display_1).expect("read");
    assert_eq!(min, 0);
    assert_eq!(cur, 60);
    assert_eq!(max, 100);
    assert!(levels.is_none());

    backend.write_brightness(&display_1, 75).expect("write");
    let (_, cur_after, _, _) = backend
        .read_brightness(&display_1)
        .expect("read after write");
    assert_eq!(cur_after, 75);

    // Test service with displays injected
    let service = avel_lib::brightness::BrightnessService::new(db.clone(), backend.clone(), None);
    service.set_test_displays(vec![display_1.clone()]);

    let snap = service.get_snapshot();
    assert_eq!(snap.displays.len(), 1);
    assert_eq!(snap.displays[0].id, "display-1");
    assert_eq!(snap.displays[0].current_percent, Some(60));

    // Test set brightness on service
    service
        .set_brightness("display-1", 45)
        .expect("set brightness");
    let snap2 = service.get_snapshot();
    assert_eq!(snap2.displays[0].target_percent, Some(45));
    // Wait for debounced worker thread to apply
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(service.get_snapshot().displays[0].current_percent, Some(45));

    // Test adjust brightness (+5)
    service
        .adjust_brightness(5, None)
        .expect("adjust brightness");
    assert_eq!(service.get_snapshot().displays[0].target_percent, Some(50));
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(service.get_snapshot().displays[0].current_percent, Some(50));

    // Test start test
    service.start_test("display-1").expect("start test");
    assert_eq!(
        service.get_snapshot().active_test_display_id.as_deref(),
        Some("display-1")
    );
    let rec = db
        .get_brightness_recovery("display-1")
        .expect("get rec")
        .expect("must exist");
    assert_eq!(rec.display_key, "display-1");

    // Test confirm test
    service
        .confirm_test("display-1", true)
        .expect("confirm test");
    assert!(service.get_snapshot().active_test_display_id.is_none());
    assert!(db
        .get_brightness_recovery("display-1")
        .expect("rec cleared")
        .is_none());

    let snap4 = service.get_snapshot();
    assert_eq!(
        snap4.displays[0].verification,
        avel_lib::brightness::VerificationState::UserConfirmed
    );
}

#[test]
fn test_desktop_manager_with_mock_backend() {
    use avel_lib::desktop::DesktopManager;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let backend = Box::new(MockDesktopBackend::new());
    let manager = DesktopManager::with_backend(backend);

    // Initial state: not hidden
    let state0 = manager.get_actual_state(&db);
    assert!(!state0.is_hidden);
    assert_eq!(state0.current_mode, "none");

    // Apply "icons" mode
    let state1 = manager.apply_mode(&db, "icons").expect("apply icons mode");
    assert!(state1.is_hidden);
    assert!(state1.actual_icons_hidden);
    assert!(!state1.actual_taskbar_hidden);
    assert_eq!(state1.current_mode, "icons");

    // Apply "all" mode
    let state2 = manager.apply_mode(&db, "all").expect("apply all mode");
    assert!(state2.is_hidden);
    assert!(state2.actual_icons_hidden);
    assert!(state2.actual_taskbar_hidden);
    assert_eq!(state2.current_mode, "all");

    // Toggle mode -> should become "none"
    let state3 = manager.toggle_mode(&db).expect("toggle off");
    assert!(!state3.is_hidden);
    assert_eq!(state3.current_mode, "none");

    // Toggle again -> should restore previous mode ("all")
    let state4 = manager.toggle_mode(&db).expect("toggle on");
    assert!(state4.is_hidden);
    assert_eq!(state4.current_mode, "all");

    // Test restore_all_on_startup_or_exit
    manager.restore_all_on_startup_or_exit(&db);
    let state_restored = manager.get_actual_state(&db);
    assert!(!state_restored.is_hidden);
    assert_eq!(state_restored.current_mode, "none");
}

#[test]
fn test_recent_items_batch_deletion_and_pinned_preservation() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let item1 = db
        .insert_or_update_recent_item("file", "Report.docx", "C:\\docs\\Report.docx", None)
        .expect("insert 1");
    let item2 = db
        .insert_or_update_recent_item(
            "file",
            "Presentation.pptx",
            "C:\\docs\\Presentation.pptx",
            None,
        )
        .expect("insert 2");
    let item3 = db
        .insert_or_update_recent_item("folder", "Projects", "C:\\Projects", None)
        .expect("insert 3");

    // Pin item 1
    db.toggle_pin_recent_item(item1.id).expect("pin item 1");

    // Batch delete item 2 and item 3
    db.delete_recent_items(&[item2.id, item3.id])
        .expect("batch delete");

    let remaining = db.get_recent_items(None, None).expect("get remaining");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, item1.id);
    assert!(remaining[0].is_pinned);

    // Add unpinned item and clear history -> pinned item must remain
    let _item4 = db
        .insert_or_update_recent_item("file", "Notes.txt", "C:\\docs\\Notes.txt", None)
        .expect("insert 4");
    assert_eq!(db.get_recent_items(None, None).expect("get").len(), 2);

    db.clear_recent_history().expect("clear unpinned");
    let after_clear = db.get_recent_items(None, None).expect("get after clear");
    assert_eq!(after_clear.len(), 1);
    assert_eq!(after_clear[0].id, item1.id);
}

#[test]
fn test_image_deduplication_exact_pixel_hash() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let img1_path = temp_dir.path().join("red.png");
    let img2_path = temp_dir.path().join("blue.png");

    let red_img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(100, 100, Rgba([255, 0, 0, 255]));
    red_img.save(&img1_path).expect("save red");

    let blue_img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(100, 100, Rgba([0, 0, 255, 255]));
    blue_img.save(&img2_path).expect("save blue");

    // Two different images of identical dimensions (100x100) must NOT be deduplicated
    let item1 = db
        .insert_or_update_clipboard_item(
            "image",
            None,
            Some(&img1_path.to_string_lossy()),
            None,
            None,
            Some(100),
            Some(100),
            "hash_red",
        )
        .expect("insert red");

    let item2 = db
        .insert_or_update_clipboard_item(
            "screenshot",
            Some("screenshot_blue.png"),
            Some(&img2_path.to_string_lossy()),
            None,
            None,
            Some(100),
            Some(100),
            "hash_blue",
        )
        .expect("insert blue");

    assert_ne!(item1.id, item2.id);
    let all = db.get_clipboard_items(None, None).expect("get all");
    assert_eq!(all.len(), 2);
}

#[test]
fn test_desktop_manager_independent_toggles_and_baseline_preservation() {
    use avel_lib::desktop::DesktopManager;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = Db::new(temp_dir.path()).expect("init db");

    let mock = MockDesktopBackend::new();
    mock.taskbar_hidden
        .store(true, std::sync::atomic::Ordering::SeqCst); // initial OS state
    mock.icons_hidden
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let manager = DesktopManager::with_backend(Box::new(mock));

    // Apply "icons" mode -> icons should hide, but taskbar autohide must remain true!
    let s1 = manager.apply_mode(&db, "icons").expect("apply icons");
    assert!(s1.is_hidden);
    assert_eq!(s1.current_mode, "icons");
    assert!(s1.actual_icons_hidden);
    assert!(
        s1.actual_taskbar_hidden,
        "Initial taskbar autohide must not be disabled by icons-only mode"
    );

    // Toggle off -> icons should restore to false, taskbar must stay true!
    let s2 = manager.toggle_mode(&db).expect("toggle off");
    assert!(!s2.is_hidden);
    assert!(!s2.actual_icons_hidden);
    assert!(s2.actual_taskbar_hidden);

    // Toggle on -> MUST restore "icons" mode, NOT "all"!
    let s3 = manager.toggle_mode(&db).expect("toggle on");
    assert!(s3.is_hidden);
    assert_eq!(
        s3.current_mode, "icons",
        "Toggle on must restore previous 'icons' mode, not 'all'"
    );
    assert!(s3.actual_icons_hidden);

    // Case 2: Apply "taskbar" mode -> toggle off -> toggle on must restore "taskbar"
    let _s4 = manager.apply_mode(&db, "taskbar").expect("apply taskbar");
    let _s5 = manager.toggle_mode(&db).expect("toggle off");
    let s6 = manager.toggle_mode(&db).expect("toggle on");
    assert_eq!(
        s6.current_mode, "taskbar",
        "Toggle on must restore 'taskbar', not 'all'"
    );
}

#[test]
fn test_brightness_service_live_test_session_refresh_isolation() {
    use avel_lib::brightness::BrightnessDisplay;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let db = std::sync::Arc::new(Db::new(temp_dir.path()).expect("init db"));
    let backend = std::sync::Arc::new(FakeBrightnessBackend::new(0, 50, 100));

    let display = BrightnessDisplay {
        id: "disp_1".to_string(),
        generation: 1,
        name: "Test Monitor".to_string(),
        device_name: Some("DISPLAY1".to_string()),
        connection: Some("DP-1".to_string()),
        is_primary: true,
        backend: Some(avel_lib::brightness::BrightnessBackend::DdcHigh),
        probe_state: avel_lib::brightness::ProbeState::ReadOk,
        verification: avel_lib::brightness::VerificationState::UserConfirmed,
        current_percent: Some(50),
        target_percent: None,
        raw_min: Some(0),
        raw_max: Some(100),
        available_levels: None,
        busy: false,
        error: None,
    };

    let service = avel_lib::brightness::BrightnessService::new(db.clone(), backend, None);
    service.set_test_displays(vec![display.clone()]);

    // Start a test on disp_1
    service.start_test("disp_1").expect("start test");
    assert_eq!(
        service.get_snapshot().active_test_display_id.as_deref(),
        Some("disp_1")
    );
    assert!(db
        .get_brightness_recovery("disp_1")
        .expect("recovery check")
        .is_some());

    // Calling refresh() must NOT treat the active live test as an orphan or clear its recovery journal!
    service.set_test_displays(vec![display]);
    service.refresh();

    assert_eq!(
        service.get_snapshot().active_test_display_id.as_deref(),
        Some("disp_1"),
        "Active test must not be killed by refresh()"
    );
    assert!(
        db.get_brightness_recovery("disp_1")
            .expect("recovery check")
            .is_some(),
        "Recovery journal of active test must not be deleted by refresh()"
    );
}

#[test]
fn test_explorer_navigation_does_not_trigger_false_close() {
    use avel_lib::recent::detect_closed_explorer_windows;
    use std::collections::HashMap;

    let mut known = HashMap::new();

    // 1. Window 1 opens at C:\FolderA
    let mut current = HashMap::new();
    current.insert("0x1000_browser1".to_string(), "C:\\FolderA".to_string());
    let closed1 = detect_closed_explorer_windows(&mut known, &current);
    assert!(
        closed1.is_empty(),
        "Initial window open must not trigger closed event"
    );
    assert_eq!(known.get("0x1000_browser1").unwrap(), "C:\\FolderA");

    // 2. Window 1 navigates from C:\FolderA to C:\FolderB (same window/tab)
    let mut current2 = HashMap::new();
    current2.insert("0x1000_browser1".to_string(), "C:\\FolderB".to_string());
    let closed2 = detect_closed_explorer_windows(&mut known, &current2);
    assert!(
        closed2.is_empty(),
        "Navigating inside open window must NOT trigger closed event"
    );
    assert_eq!(known.get("0x1000_browser1").unwrap(), "C:\\FolderB");

    // 3. Window 2 opens at C:\Projects
    let mut current3 = HashMap::new();
    current3.insert("0x1000_browser1".to_string(), "C:\\FolderB".to_string());
    current3.insert("0x2000_browser2".to_string(), "C:\\Projects".to_string());
    let closed3 = detect_closed_explorer_windows(&mut known, &current3);
    assert!(closed3.is_empty());

    // 4. Window 2 closes, while Window 1 remains open
    let mut current4 = HashMap::new();
    current4.insert("0x1000_browser1".to_string(), "C:\\FolderB".to_string());
    let closed4 = detect_closed_explorer_windows(&mut known, &current4);
    assert_eq!(
        closed4,
        vec!["C:\\Projects".to_string()],
        "Closed Window 2 must be reported"
    );
    assert_eq!(known.len(), 1);

    // 5. Window 1 finally closes
    let current5 = HashMap::new();
    let closed5 = detect_closed_explorer_windows(&mut known, &current5);
    assert_eq!(
        closed5,
        vec!["C:\\FolderB".to_string()],
        "Closed Window 1 must report last visited folder"
    );
    assert!(known.is_empty());
}

#[test]
fn test_clipboard_chunk_deletion_boundary_450_items() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = avel_lib::db::Db::new(dir.path()).expect("init db");

    // Insert 450 items to exceed the 400-item SQL batch chunk limit
    let mut ids = Vec::new();
    for i in 0..450 {
        let hash = format!("chunk_hash_{i:04}");
        let item = db
            .insert_or_update_clipboard_item(
                "text",
                Some(&format!("Text content {i}")),
                None,
                None,
                Some(14),
                None,
                None,
                &hash,
            )
            .expect("insert item");
        ids.push(item.id);
    }

    assert_eq!(ids.len(), 450);
    let items_before = db
        .get_clipboard_items(None, None)
        .expect("get items before");
    assert_eq!(items_before.len(), 450);

    // Delete all 450 items across chunk boundaries
    db.delete_clipboard_items(&ids).expect("delete 450 items");

    let items_after = db.get_clipboard_items(None, None).expect("get items after");
    assert_eq!(
        items_after.len(),
        0,
        "All 450 items across chunk boundaries must be deleted"
    );
}

#[test]
fn test_mismatched_display_id_returns_device_not_found() {
    use avel_lib::brightness::{
        BrightnessBackend, BrightnessDeviceBackend, BrightnessDisplay, NativeBrightnessBackend,
        ProbeState, VerificationState,
    };

    let backend = NativeBrightnessBackend::new();
    let unknown_display = BrightnessDisplay {
        id: "DISPLAY\\NON_EXISTENT_MONITOR_9999\\12345678".to_string(),
        generation: 1,
        name: "NonExistent".to_string(),
        connection: Some("HDMI".to_string()),
        is_primary: false,
        backend: Some(BrightnessBackend::Wmi),
        probe_state: ProbeState::NotChecked,
        verification: VerificationState::NotTested,
        current_percent: None,
        target_percent: None,
        raw_min: Some(0),
        raw_max: Some(100),
        available_levels: None,
        busy: false,
        error: None,
        device_name: None,
    };

    let read_res = backend.read_brightness(&unknown_display);
    assert!(
        matches!(read_res, Err(ref e) if e.code == "device_not_found"),
        "Non-matching monitor ID must return device_not_found error without writing to any hardware"
    );

    let write_res = backend.write_brightness(&unknown_display, 50);
    assert!(
        matches!(write_res, Err(ref e) if e.code == "device_not_found"),
        "Non-matching monitor ID write must fail with device_not_found without writing to wrong device"
    );
}
