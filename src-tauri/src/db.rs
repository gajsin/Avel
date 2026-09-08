use crate::brightness::model::{
    BrightnessBackend, BrightnessMonitorPreference, BrightnessPreferences,
    BrightnessTestRecoveryRecord, VerificationState,
};
use crate::types::{AppSettings, AppearanceConfig, CleanDesktopState, ClipboardItem, RecentItem};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Db {
    conn: Mutex<Connection>,
    pub images_dir: PathBuf,
}

impl Db {
    pub fn new(app_dir: &Path) -> Result<Self, String> {
        let db_path = app_dir.join("avel.db");
        let images_dir = app_dir.join("clipboard").join("images");
        let thumbs_dir = app_dir.join("cache").join("thumbnails");
        let logs_dir = app_dir.join("logs");

        fs::create_dir_all(&images_dir).map_err(|e| format!("create images dir: {e}"))?;
        fs::create_dir_all(&thumbs_dir).map_err(|e| format!("create thumbs dir: {e}"))?;
        fs::create_dir_all(&logs_dir).map_err(|e| format!("create logs dir: {e}"))?;

        let conn = Connection::open(&db_path).map_err(|e| format!("open db: {e}"))?;

        // Enable WAL mode, foreign keys, synchronous NORMAL
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
        )
        .map_err(|e| format!("pragma init: {e}"))?;

        let db = Self {
            conn: Mutex::new(conn),
            images_dir,
        };

        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
            [],
        )
        .map_err(|e| format!("create migrations table: {e}"))?;

        let current_version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations;",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if current_version < 1 {
            conn.execute_batch(
                r#"BEGIN TRANSACTION;
                
                CREATE TABLE IF NOT EXISTS app_settings (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    app_theme TEXT NOT NULL DEFAULT 'System',
                    language TEXT NOT NULL DEFAULT 'ru',
                    autostart INTEGER NOT NULL DEFAULT 0,
                    last_section TEXT NOT NULL DEFAULT 'clipboard',
                    sidebar_collapsed INTEGER NOT NULL DEFAULT 0,
                    hotkey_clipboard TEXT NOT NULL DEFAULT '{"code":"Equal","ctrl":false,"alt":false,"shift":false,"meta":false}',
                    hotkey_recent TEXT NOT NULL DEFAULT '{"code":"F8","ctrl":false,"alt":false,"shift":false,"meta":false}',
                    hotkey_appearance TEXT NOT NULL DEFAULT '{"code":"KeyD","ctrl":true,"alt":true,"shift":false,"meta":false}',
                    hotkey_desktop TEXT NOT NULL DEFAULT '{"code":"ArrowDown","ctrl":false,"alt":false,"shift":false,"meta":false}'
                );

                INSERT OR IGNORE INTO app_settings (id) VALUES (1);

                CREATE TABLE IF NOT EXISTS view_preferences (
                    tab_id TEXT PRIMARY KEY,
                    view_mode TEXT NOT NULL DEFAULT 'details'
                );

                CREATE TABLE IF NOT EXISTS clipboard_items (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    content_type TEXT NOT NULL,
                    text_content TEXT,
                    image_path TEXT,
                    thumbnail_b64 TEXT,
                    char_count INTEGER,
                    width INTEGER,
                    height INTEGER,
                    hash TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    is_pinned INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_clipboard_updated_at ON clipboard_items(updated_at DESC);
                CREATE INDEX IF NOT EXISTS idx_clipboard_hash ON clipboard_items(hash);

                CREATE TABLE IF NOT EXISTS recent_items (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    entry_type TEXT NOT NULL,
                    title TEXT NOT NULL,
                    path TEXT NOT NULL UNIQUE,
                    thumbnail_b64 TEXT,
                    last_accessed_at TEXT NOT NULL,
                    is_pinned INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_recent_last_accessed ON recent_items(last_accessed_at DESC);

                CREATE TABLE IF NOT EXISTS appearance_config (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    windows_theme_target TEXT NOT NULL DEFAULT 'System',
                    schedule_enabled INTEGER NOT NULL DEFAULT 0,
                    schedule_light_time TEXT NOT NULL DEFAULT '07:00',
                    schedule_dark_time TEXT NOT NULL DEFAULT '19:00',
                    switch_wallpaper INTEGER NOT NULL DEFAULT 0,
                    wallpaper_light_path TEXT NOT NULL DEFAULT '',
                    wallpaper_dark_path TEXT NOT NULL DEFAULT ''
                );
                INSERT OR IGNORE INTO appearance_config (id) VALUES (1);

                CREATE TABLE IF NOT EXISTS clean_desktop_state (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    current_mode TEXT NOT NULL DEFAULT 'icons_only',
                    is_hidden INTEGER NOT NULL DEFAULT 0,
                    prev_icons_hidden INTEGER NOT NULL DEFAULT 0,
                    prev_taskbars_json TEXT NOT NULL DEFAULT '[]',
                    recovery_marker INTEGER NOT NULL DEFAULT 0,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT OR IGNORE INTO clean_desktop_state (id) VALUES (1);

                INSERT INTO schema_migrations (version, applied_at) VALUES (1, CURRENT_TIMESTAMP);
                COMMIT;"#,
            ).map_err(|e| format!("migration v1 error: {e}"))?;
        }

        if current_version < 2 {
            conn.execute_batch(
                "BEGIN TRANSACTION;
                -- Clear legacy damaged folder thumbnails to re-extract with true alpha transparency
                UPDATE recent_items SET thumbnail_b64 = NULL WHERE entry_type = 'folder';
                INSERT INTO schema_migrations (version, applied_at) VALUES (2, CURRENT_TIMESTAMP);
                COMMIT;",
            )
            .map_err(|e| format!("migration v2 error: {e}"))?;
        }

        if current_version < 3 {
            conn.execute_batch(
                "BEGIN TRANSACTION;
                -- Deduplicate existing duplicate screenshots by image_path or hash
                DELETE FROM clipboard_items WHERE id NOT IN (
                    SELECT MIN(id) FROM clipboard_items GROUP BY COALESCE(image_path, hash)
                );
                INSERT INTO schema_migrations (version, applied_at) VALUES (3, CURRENT_TIMESTAMP);
                COMMIT;",
            )
            .map_err(|e| format!("migration v3 error: {e}"))?;
        }

        if current_version < 4 {
            let has_col: bool = conn
                .prepare("PRAGMA table_info(recent_items);")
                .and_then(|mut stmt| {
                    let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
                    for col in cols.flatten() {
                        if col == "is_pinned" {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                })
                .unwrap_or(false);

            if !has_col {
                conn.execute(
                    "ALTER TABLE recent_items ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;",
                    [],
                )
                .map_err(|e| format!("migration v4 error adding is_pinned: {e}"))?;
            }
            conn.execute(
                "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (4, CURRENT_TIMESTAMP);",
                [],
            )
            .map_err(|e| format!("migration v4 error recording migration: {e}"))?;
        }

        if current_version < 5 {
            conn.execute_batch(
                "BEGIN TRANSACTION;
                CREATE TABLE IF NOT EXISTS brightness_preferences (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    sync_group INTEGER NOT NULL DEFAULT 0,
                    step_percent INTEGER NOT NULL DEFAULT 5,
                    group_display_ids TEXT NOT NULL DEFAULT '[]',
                    hotkey_brightness_target TEXT NOT NULL DEFAULT 'cursor',
                    hotkey_brightness_up TEXT,
                    hotkey_brightness_down TEXT,
                    hotkey_brightness_screen TEXT,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                INSERT OR IGNORE INTO brightness_preferences (id) VALUES (1);

                CREATE TABLE IF NOT EXISTS brightness_monitor_preferences (
                    display_key TEXT PRIMARY KEY,
                    preferred_backend TEXT,
                    last_verification_state TEXT NOT NULL DEFAULT 'not_tested',
                    last_verification_connection TEXT,
                    custom_name TEXT,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE IF NOT EXISTS brightness_test_recovery (
                    display_key TEXT PRIMARY KEY,
                    connection_key TEXT NOT NULL,
                    backend TEXT NOT NULL,
                    original_raw INTEGER NOT NULL,
                    test_raw INTEGER NOT NULL,
                    stage TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                INSERT INTO schema_migrations (version, applied_at) VALUES (5, CURRENT_TIMESTAMP);
                COMMIT;",
            )
            .map_err(|e| format!("migration v5 error: {e}"))?;
        }

        if current_version < 6 {
            let has_col: bool = conn
                .prepare("PRAGMA table_info(brightness_test_recovery);")
                .and_then(|mut stmt| {
                    let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
                    for col in cols.flatten() {
                        if col == "test_id" {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                })
                .unwrap_or(false);

            if !has_col {
                let _ = conn.execute(
                    "ALTER TABLE brightness_test_recovery ADD COLUMN test_id TEXT NOT NULL DEFAULT '';",
                    [],
                );
            }
            conn.execute(
                "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (6, CURRENT_TIMESTAMP);",
                [],
            )
            .map_err(|e| format!("migration v6 error: {e}"))?;
        }

        Ok(())
    }

    fn fetch_clipboard_item_by_id(
        &self,
        conn: &rusqlite::Connection,
        id: i64,
    ) -> Result<ClipboardItem, String> {
        conn.query_row(
            "SELECT id, content_type, text_content, image_path, thumbnail_b64,
                char_count, width, height, hash, created_at, updated_at, is_pinned
             FROM clipboard_items WHERE id = ?1;",
            params![id],
            |r| self.row_to_clipboard_item(r),
        )
        .map_err(|e| format!("fetch clipboard item by id: {e}"))
    }

    // App Settings
    pub fn get_settings(&self) -> Result<AppSettings, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut stmt = conn
            .prepare(
                "SELECT app_theme, language, autostart, last_section, sidebar_collapsed,
                    hotkey_clipboard, hotkey_recent, hotkey_appearance, hotkey_desktop
             FROM app_settings WHERE id = 1;",
            )
            .map_err(|e| e.to_string())?;

        let settings = stmt
            .query_row([], |r| {
                Ok(AppSettings {
                    app_theme: r.get(0)?,
                    language: r.get(1)?,
                    autostart: r.get::<_, i64>(2)? != 0,
                    last_section: r.get(3)?,
                    sidebar_collapsed: r.get::<_, i64>(4)? != 0,
                    hotkey_clipboard: r.get(5)?,
                    hotkey_recent: r.get(6)?,
                    hotkey_appearance: r.get(7)?,
                    hotkey_desktop: r.get(8)?,
                })
            })
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();

        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "UPDATE app_settings SET
                app_theme = ?1,
                language = ?2,
                autostart = ?3,
                last_section = ?4,
                sidebar_collapsed = ?5,
                hotkey_clipboard = ?6,
                hotkey_recent = ?7,
                hotkey_appearance = ?8,
                hotkey_desktop = ?9
             WHERE id = 1;",
            params![
                settings.app_theme,
                settings.language,
                if settings.autostart { 1 } else { 0 },
                settings.last_section,
                if settings.sidebar_collapsed { 1 } else { 0 },
                settings.hotkey_clipboard,
                settings.hotkey_recent,
                settings.hotkey_appearance,
                settings.hotkey_desktop,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_view_mode(&self, tab_id: &str) -> Result<String, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mode: Option<String> = conn
            .query_row(
                "SELECT view_mode FROM view_preferences WHERE tab_id = ?1;",
                params![tab_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let fallback = if tab_id.contains("image") || tab_id.contains("screenshot") {
            "grid".to_string()
        } else {
            "details".to_string()
        };
        let m = mode.unwrap_or(fallback);
        if m == "list" {
            Ok("details".to_string())
        } else {
            Ok(m)
        }
    }

    pub fn set_view_mode(&self, tab_id: &str, view_mode: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "INSERT INTO view_preferences (tab_id, view_mode) VALUES (?1, ?2)
             ON CONFLICT(tab_id) DO UPDATE SET view_mode = excluded.view_mode;",
            params![tab_id, view_mode],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    // Clipboard
    #[allow(clippy::too_many_arguments)]
    pub fn insert_or_update_clipboard_item(
        &self,
        content_type: &str,
        text_content: Option<&str>,
        image_path: Option<&str>,
        thumbnail_b64: Option<&str>,
        char_count: Option<i64>,
        width: Option<u32>,
        height: Option<u32>,
        hash: &str,
    ) -> Result<ClipboardItem, String> {
        let now = chrono::Utc::now().to_rfc3339();

        // Screenshot / Image deduplication:
        // Windows Snipping Tool simultaneously writes to Pictures\Screenshots and copies DIB to clipboard.
        let window_start = (chrono::Utc::now() - chrono::Duration::seconds(15)).to_rfc3339();
        if content_type == "screenshot" {
            // 1. Check if this exact image file is already stored in the clipboard DB
            if let Some(target_path) = image_path {
                let existing_path_match: Option<i64> = {
                    let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
                    conn.query_row(
                        "SELECT id FROM clipboard_items WHERE image_path = ?1 LIMIT 1;",
                        params![target_path],
                        |r| r.get(0),
                    )
                    .ok()
                };

                if let Some(existing_id) = existing_path_match {
                    let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
                    if thumbnail_b64.is_some() {
                        conn.execute(
                            "UPDATE clipboard_items SET
                                thumbnail_b64 = COALESCE(?1, thumbnail_b64),
                                updated_at = ?2
                             WHERE id = ?3;",
                            params![thumbnail_b64, now, existing_id],
                        )
                        .map_err(|e| format!("update existing screenshot thumbnail: {e}"))?;
                    }
                    return self.fetch_clipboard_item_by_id(&conn, existing_id);
                }
            }

            // 2. Check candidate by dimensions
            let candidate: Option<(i64, String)> = if let (Some(w), Some(h)) = (width, height) {
                let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
                conn.query_row(
                    "SELECT id, image_path FROM clipboard_items
                     WHERE content_type = 'image'
                       AND width = ?1 AND height = ?2
                       AND created_at >= ?3
                       AND image_path IS NOT NULL
                     ORDER BY id DESC LIMIT 1;",
                    params![w, h, window_start],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok()
            } else {
                None
            };

            if let Some((existing_id, old_img_path)) = candidate {
                let old_full = self.images_dir.join(&old_img_path);
                let new_full = image_path.map(PathBuf::from);

                // Pixel-content comparison performed OUTSIDE DB mutex lock
                if is_plausible_image_duplicate(Some(&old_full), new_full.as_deref()) {
                    let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
                    let still_exists: bool = conn
                        .query_row(
                            "SELECT 1 FROM clipboard_items WHERE id = ?1;",
                            params![existing_id],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if still_exists {
                        conn.execute(
                            "UPDATE clipboard_items SET
                                content_type = 'screenshot',
                                image_path = ?1,
                                thumbnail_b64 = COALESCE(?2, thumbnail_b64),
                                updated_at = ?3,
                                hash = ?5
                             WHERE id = ?4;",
                            params![image_path, thumbnail_b64, now, existing_id, hash],
                        )
                        .map_err(|e| format!("upgrade screenshot item: {e}"))?;

                        let item = self.fetch_clipboard_item_by_id(&conn, existing_id)?;
                        drop(conn); // Drop DB lock before filesystem deletion
                        self.remove_image_file_if_owned(&old_img_path);
                        return Ok(item);
                    }
                }
            }
        } else if content_type == "image" {
            let candidate: Option<(i64, String)> = if let (Some(w), Some(h)) = (width, height) {
                let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
                conn.query_row(
                    "SELECT id, image_path FROM clipboard_items
                     WHERE content_type = 'screenshot'
                       AND width = ?1 AND height = ?2
                       AND created_at >= ?3
                       AND image_path IS NOT NULL
                     ORDER BY id DESC LIMIT 1;",
                    params![w, h, window_start],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok()
            } else {
                None
            };

            if let Some((existing_id, existing_img_path)) = candidate {
                let old_full = PathBuf::from(&existing_img_path);
                let new_full = image_path.map(|p| self.images_dir.join(p));

                // Pixel-content comparison performed OUTSIDE DB mutex lock
                if is_plausible_image_duplicate(Some(&old_full), new_full.as_deref()) {
                    let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
                    let still_exists: bool = conn
                        .query_row(
                            "SELECT 1 FROM clipboard_items WHERE id = ?1;",
                            params![existing_id],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if still_exists {
                        if thumbnail_b64.is_some() {
                            conn.execute(
                                "UPDATE clipboard_items SET
                                    thumbnail_b64 = COALESCE(thumbnail_b64, ?1),
                                    updated_at = ?2
                                 WHERE id = ?3;",
                                params![thumbnail_b64, now, existing_id],
                            )
                            .map_err(|e| format!("update screenshot thumbnail: {e}"))?;
                        }

                        let item = self.fetch_clipboard_item_by_id(&conn, existing_id)?;
                        drop(conn); // Drop DB lock before filesystem cleanup
                        if let Some(new_p) = image_path {
                            self.remove_image_file_if_owned(new_p);
                        }
                        return Ok(item);
                    }
                }
            }
        }

        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "INSERT INTO clipboard_items (
                content_type, text_content, image_path, thumbnail_b64,
                char_count, width, height, hash, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(hash) DO UPDATE SET
                thumbnail_b64 = COALESCE(excluded.thumbnail_b64, clipboard_items.thumbnail_b64),
                updated_at = excluded.updated_at;",
            params![
                content_type,
                text_content,
                image_path,
                thumbnail_b64,
                char_count,
                width,
                height,
                hash,
                now,
            ],
        )
        .map_err(|e| format!("insert/update clipboard: {e}"))?;

        // Limit to 500 entries (prune oldest unpinned)
        let mut pruned_paths: Vec<String> = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT image_path FROM clipboard_items WHERE is_pinned = 0 AND content_type = 'image' AND image_path IS NOT NULL AND id NOT IN (
                SELECT id FROM (
                    SELECT id FROM clipboard_items WHERE is_pinned = 0 ORDER BY updated_at DESC LIMIT 500
                )
            );",
        ) {
            if let Ok(rows) = stmt.query_map([], |r| r.get(0)) {
                for p in rows.flatten() {
                    pruned_paths.push(p);
                }
            }
        }

        conn.execute(
            "DELETE FROM clipboard_items WHERE id NOT IN (
                SELECT id FROM clipboard_items WHERE is_pinned = 1
                UNION
                SELECT id FROM (
                    SELECT id FROM clipboard_items WHERE is_pinned = 0 ORDER BY updated_at DESC LIMIT 500
                )
            );",
            [],
        )
        .map_err(|e| format!("prune clipboard: {e}"))?;

        let item = conn
            .query_row(
                "SELECT id, content_type, text_content, image_path,
                    thumbnail_b64, char_count, width, height, hash, created_at, updated_at, is_pinned
             FROM clipboard_items WHERE hash = ?1;",
                params![hash],
                |r| self.row_to_clipboard_item(r),
            )
            .map_err(|e| format!("fetch inserted clipboard item: {e}"))?;

        drop(conn); // Drop DB lock before filesystem cleanup

        for path_str in pruned_paths {
            self.remove_image_file_if_owned(&path_str);
        }

        Ok(item)
    }

    pub fn get_clipboard_items(
        &self,
        filter_type: Option<&str>,
        search_query: Option<&str>,
    ) -> Result<Vec<ClipboardItem>, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut query = "SELECT id, content_type, text_content, image_path,
                                CASE WHEN image_path IS NOT NULL AND image_path != '' THEN NULL ELSE thumbnail_b64 END,
                                char_count, width, height, hash, created_at, updated_at, is_pinned
                         FROM clipboard_items WHERE 1=1"
            .to_string();

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ft) = filter_type {
            if ft != "all" && !ft.is_empty() {
                if ft == "text" {
                    query.push_str(" AND content_type IN ('text', 'link', 'code')");
                } else if ft == "images"
                    || ft == "image"
                    || ft == "screenshots"
                    || ft == "screenshot"
                {
                    query.push_str(" AND content_type IN ('image', 'screenshot')");
                } else if ft == "favorites" || ft == "pinned" {
                    query.push_str(" AND is_pinned = 1");
                }
            }
        }

        if let Some(search) = search_query {
            let trimmed = search.trim();
            if !trimmed.is_empty() {
                params_vec.push(Box::new(format!("%{}%", trimmed)));
                let idx = params_vec.len();
                query.push_str(&format!(
                    " AND (text_content LIKE ?{} OR image_path LIKE ?{})",
                    idx, idx
                ));
            }
        }

        query.push_str(" ORDER BY is_pinned DESC, updated_at DESC LIMIT 1000;");

        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        let params_slice: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(params_slice.as_slice(), |r| self.row_to_clipboard_item(r))
            .map_err(|e| e.to_string())?;

        let items: Result<Vec<ClipboardItem>, _> = rows.collect();
        let items = items.map_err(|e| format!("read clipboard row: {e}"))?;
        Ok(items)
    }

    fn row_to_clipboard_item(&self, r: &rusqlite::Row) -> rusqlite::Result<ClipboardItem> {
        let ct: String = r.get(1)?;
        let p: Option<String> = r.get(3)?;
        Ok(ClipboardItem {
            id: r.get(0)?,
            content_type: ct.clone(),
            text_content: r.get(2)?,
            image_path: self.resolve_image_path(&ct, p),
            thumbnail_b64: r.get(4)?,
            char_count: r.get(5)?,
            width: r.get(6)?,
            height: r.get(7)?,
            hash: r.get(8)?,
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
            is_pinned: r.get::<_, i64>(11)? != 0,
        })
    }

    fn resolve_image_path(&self, content_type: &str, raw_path: Option<String>) -> Option<String> {
        raw_path.map(|p| {
            if content_type == "image" && !Path::new(&p).is_absolute() {
                self.images_dir.join(&p).to_string_lossy().to_string()
            } else {
                p
            }
        })
    }

    fn remove_image_file_if_owned(&self, path_str: &str) {
        let p = Path::new(path_str);
        if !p.is_absolute() {
            // Check if another record in clipboard_items still references this path
            let is_referenced = if let Ok(conn) = self.conn.lock() {
                conn.query_row(
                    "SELECT 1 FROM clipboard_items WHERE image_path = ?1 LIMIT 1;",
                    params![path_str],
                    |_| Ok(true),
                )
                .unwrap_or(false)
            } else {
                true // Fail-closed on poisoned mutex
            };

            if !is_referenced {
                let full_path = self.images_dir.join(p);
                if let (Ok(canon_full), Ok(canon_dir)) =
                    (full_path.canonicalize(), self.images_dir.canonicalize())
                {
                    if canon_full.starts_with(canon_dir) && canon_full.is_file() {
                        let _ = fs::remove_file(canon_full);
                    }
                }
            }
        }
    }

    pub fn delete_clipboard_item(&self, id: i64) -> Result<(), String> {
        self.delete_clipboard_items(&[id])
    }

    pub fn delete_clipboard_items(&self, ids: &[i64]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut img_paths: Vec<String> = Vec::new();

        let tx = conn.transaction().map_err(|e| e.to_string())?;

        for chunk in ids.chunks(400) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{i}")).collect();
            let in_clause = placeholders.join(",");

            // 1. Retrieve image paths for any owned image files
            let select_query = format!(
                "SELECT image_path FROM clipboard_items WHERE id IN ({in_clause}) AND content_type = 'image' AND image_path IS NOT NULL;"
            );
            {
                let mut stmt = tx.prepare(&select_query).map_err(|e| e.to_string())?;
                let params_vec: Vec<&dyn rusqlite::ToSql> =
                    chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
                let rows = stmt
                    .query_map(params_vec.as_slice(), |r| r.get(0))
                    .map_err(|e| e.to_string())?;
                for r in rows {
                    let p: String = r.map_err(|e| format!("read delete path error: {e}"))?;
                    img_paths.push(p);
                }
            }

            // 2. Delete items in batch
            let delete_query = format!("DELETE FROM clipboard_items WHERE id IN ({in_clause});");
            {
                let params_vec: Vec<&dyn rusqlite::ToSql> =
                    chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
                tx.execute(&delete_query, params_vec.as_slice())
                    .map_err(|e| e.to_string())?;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        drop(conn); // Drop DB mutex before disk removal

        // 3. Delete associated files from disk only after successful commit and outside DB mutex
        for path_str in img_paths {
            self.remove_image_file_if_owned(&path_str);
        }

        Ok(())
    }

    pub fn clear_clipboard_history(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut paths: Vec<String> = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT image_path FROM clipboard_items WHERE is_pinned = 0 AND content_type = 'image' AND image_path IS NOT NULL;",
        ) {
            if let Ok(rows) = stmt.query_map([], |r| r.get(0)) {
                for p in rows.flatten() {
                    paths.push(p);
                }
            }
        }

        conn.execute("DELETE FROM clipboard_items WHERE is_pinned = 0;", [])
            .map_err(|e| e.to_string())?;

        drop(conn); // Drop DB mutex before disk removal

        for path_str in paths {
            self.remove_image_file_if_owned(&path_str);
        }

        Ok(())
    }

    // Recent Explorer Items
    pub fn insert_or_update_recent_item(
        &self,
        entry_type: &str,
        title: &str,
        path: &str,
        thumbnail_b64: Option<&str>,
    ) -> Result<RecentItem, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO recent_items (entry_type, title, path, thumbnail_b64, last_accessed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                thumbnail_b64 = COALESCE(excluded.thumbnail_b64, recent_items.thumbnail_b64),
                last_accessed_at = excluded.last_accessed_at;",
            params![entry_type, title, path, thumbnail_b64, now],
        )
        .map_err(|e| format!("insert/update recent: {e}"))?;

        // Keep maximum 100 unpinned entries, preserving all pinned entries
        conn.execute(
            "DELETE FROM recent_items WHERE id NOT IN (
                SELECT id FROM recent_items WHERE is_pinned = 1
                UNION
                SELECT id FROM (
                    SELECT id FROM recent_items WHERE is_pinned = 0 ORDER BY last_accessed_at DESC LIMIT 100
                )
            );",
            [],
        )
        .map_err(|e| format!("prune recent: {e}"))?;

        let item = conn
            .query_row(
                "SELECT id, entry_type, title, path, thumbnail_b64, last_accessed_at, is_pinned
             FROM recent_items WHERE path = ?1;",
                params![path],
                |r| {
                    Ok(RecentItem {
                        id: r.get(0)?,
                        entry_type: r.get(1)?,
                        title: r.get(2)?,
                        path: r.get(3)?,
                        thumbnail_b64: r.get(4)?,
                        last_accessed_at: r.get(5)?,
                        is_pinned: r.get::<_, i64>(6)? != 0,
                    })
                },
            )
            .map_err(|e| format!("fetch recent item: {e}"))?;

        Ok(item)
    }

    pub fn get_recent_items(
        &self,
        filter_type: Option<&str>,
        search_query: Option<&str>,
    ) -> Result<Vec<RecentItem>, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut query =
            "SELECT id, entry_type, title, path, thumbnail_b64, last_accessed_at, is_pinned
                         FROM recent_items WHERE 1=1"
                .to_string();

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ft) = filter_type {
            if ft == "files" {
                query.push_str(" AND entry_type = 'file'");
            } else if ft == "folders" {
                query.push_str(" AND entry_type = 'folder'");
            } else if ft == "favorites" || ft == "pinned" {
                query.push_str(" AND is_pinned = 1");
            }
        }

        if let Some(search) = search_query {
            let trimmed = search.trim();
            if !trimmed.is_empty() {
                params_vec.push(Box::new(format!("%{}%", trimmed)));
                let idx = params_vec.len();
                query.push_str(&format!(" AND (title LIKE ?{} OR path LIKE ?{})", idx, idx));
            }
        }

        query.push_str(" ORDER BY is_pinned DESC, last_accessed_at DESC LIMIT 500;");

        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        let params_slice: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(params_slice.as_slice(), |r| {
                Ok(RecentItem {
                    id: r.get(0)?,
                    entry_type: r.get(1)?,
                    title: r.get(2)?,
                    path: r.get(3)?,
                    thumbnail_b64: r.get(4)?,
                    last_accessed_at: r.get(5)?,
                    is_pinned: r.get::<_, i64>(6)? != 0,
                })
            })
            .map_err(|e| e.to_string())?;

        let items: Result<Vec<RecentItem>, _> = rows.collect();
        let items = items.map_err(|e| format!("read recent row: {e}"))?;
        Ok(items)
    }

    pub fn toggle_pin_clipboard_item(&self, id: i64) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let current: i64 = conn
            .query_row(
                "SELECT is_pinned FROM clipboard_items WHERE id = ?1;",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| format!("clipboard item not found: {e}"))?;
        let next = if current == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE clipboard_items SET is_pinned = ?1 WHERE id = ?2;",
            params![next, id],
        )
        .map_err(|e| format!("toggle pin clipboard item: {e}"))?;
        Ok(next == 1)
    }

    pub fn toggle_pin_recent_item(&self, id: i64) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let current: i64 = conn
            .query_row(
                "SELECT is_pinned FROM recent_items WHERE id = ?1;",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| format!("recent item not found: {e}"))?;
        let next = if current == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE recent_items SET is_pinned = ?1 WHERE id = ?2;",
            params![next, id],
        )
        .map_err(|e| format!("toggle pin recent item: {e}"))?;
        Ok(next == 1)
    }

    pub fn delete_recent_item(&self, id: i64) -> Result<(), String> {
        self.delete_recent_items(&[id])
    }

    pub fn delete_recent_items(&self, ids: &[i64]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        for chunk in ids.chunks(400) {
            let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("?{i}")).collect();
            let in_clause = placeholders.join(",");
            let delete_query = format!("DELETE FROM recent_items WHERE id IN ({in_clause});");
            let params_vec: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            tx.execute(&delete_query, params_vec.as_slice())
                .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_recent_history(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute("DELETE FROM recent_items WHERE is_pinned = 0;", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // Appearance Config
    pub fn get_appearance_config(&self) -> Result<AppearanceConfig, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut stmt = conn.prepare(
            "SELECT windows_theme_target, schedule_enabled, schedule_light_time, schedule_dark_time,
                    switch_wallpaper, wallpaper_light_path, wallpaper_dark_path
             FROM appearance_config WHERE id = 1;"
        ).map_err(|e| e.to_string())?;

        let config = stmt
            .query_row([], |r| {
                Ok(AppearanceConfig {
                    windows_theme_target: r.get(0)?,
                    schedule_enabled: r.get::<_, i64>(1)? != 0,
                    schedule_light_time: r.get(2)?,
                    schedule_dark_time: r.get(3)?,
                    switch_wallpaper: r.get::<_, i64>(4)? != 0,
                    wallpaper_light_path: r.get(5)?,
                    wallpaper_dark_path: r.get(6)?,
                })
            })
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();

        Ok(config)
    }

    pub fn save_appearance_config(&self, config: &AppearanceConfig) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "UPDATE appearance_config SET
                windows_theme_target = ?1,
                schedule_enabled = ?2,
                schedule_light_time = ?3,
                schedule_dark_time = ?4,
                switch_wallpaper = ?5,
                wallpaper_light_path = ?6,
                wallpaper_dark_path = ?7
             WHERE id = 1;",
            params![
                config.windows_theme_target,
                if config.schedule_enabled { 1 } else { 0 },
                config.schedule_light_time,
                config.schedule_dark_time,
                if config.switch_wallpaper { 1 } else { 0 },
                config.wallpaper_light_path,
                config.wallpaper_dark_path,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    // Clean Desktop State & Recovery Marker
    pub fn get_clean_desktop_state(&self) -> Result<CleanDesktopState, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut stmt = conn
            .prepare("SELECT current_mode, is_hidden FROM clean_desktop_state WHERE id = 1;")
            .map_err(|e| e.to_string())?;

        let state = stmt
            .query_row([], |r| {
                Ok(CleanDesktopState {
                    current_mode: r.get(0)?,
                    is_hidden: r.get::<_, i64>(1)? != 0,
                    actual_icons_hidden: false,
                    actual_taskbar_hidden: false,
                })
            })
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();

        Ok(state)
    }

    pub fn save_clean_desktop_state(&self, mode: &str, is_hidden: bool) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "UPDATE clean_desktop_state SET
                current_mode = ?1,
                is_hidden = ?2,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = 1;",
            params![mode, if is_hidden { 1 } else { 0 }],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_recovery_marker(&self, marker: bool) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "UPDATE clean_desktop_state SET
                recovery_marker = ?1,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = 1;",
            params![if marker { 1 } else { 0 }],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_recovery_marker(&self) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let res: Option<bool> = conn
            .query_row(
                "SELECT recovery_marker FROM clean_desktop_state WHERE id = 1;",
                [],
                |r| Ok(r.get::<_, i64>(0)? != 0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(res.unwrap_or(false))
    }

    pub fn save_clean_desktop_baseline(
        &self,
        icons_hidden: bool,
        taskbar_autohide: bool,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "UPDATE clean_desktop_state SET
                prev_icons_hidden = ?1,
                prev_taskbars_json = ?2,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = 1;",
            params![
                if icons_hidden { 1 } else { 0 },
                if taskbar_autohide { "1" } else { "0" }
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_clean_desktop_baseline(&self) -> Result<(bool, bool), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let res = conn
            .query_row(
                "SELECT prev_icons_hidden, prev_taskbars_json FROM clean_desktop_state WHERE id = 1;",
                [],
                |r| {
                    let icons: i64 = r.get(0)?;
                    let tb_str: String = r.get(1)?;
                    let tb = tb_str == "1" || tb_str == "true";
                    Ok((icons != 0, tb))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(res.unwrap_or((false, false)))
    }

    // Brightness Preferences
    pub fn get_brightness_preferences(&self) -> Result<BrightnessPreferences, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let prefs: Option<BrightnessPreferences> = conn
            .query_row(
                "SELECT sync_group, step_percent, group_display_ids,
                        hotkey_brightness_target, hotkey_brightness_up,
                        hotkey_brightness_down, hotkey_brightness_screen
                 FROM brightness_preferences WHERE id = 1;",
                [],
                |r| {
                    let sync_group: i64 = r.get(0)?;
                    let step_percent: i64 = r.get(1)?;
                    let group_ids_json: String = r.get(2)?;
                    let target: String = r.get(3)?;
                    let hk_up: Option<String> = r.get(4)?;
                    let hk_down: Option<String> = r.get(5)?;
                    let hk_screen: Option<String> = r.get(6)?;

                    let group_display_ids: Vec<String> =
                        serde_json::from_str(&group_ids_json).unwrap_or_default();

                    Ok(BrightnessPreferences {
                        sync_group: sync_group != 0,
                        step_percent: step_percent as u32,
                        group_display_ids,
                        hotkey_brightness_target: target,
                        hotkey_brightness_up: hk_up,
                        hotkey_brightness_down: hk_down,
                        hotkey_brightness_screen: hk_screen,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;

        Ok(prefs.unwrap_or_default())
    }

    pub fn save_brightness_preferences(&self, prefs: &BrightnessPreferences) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let group_ids_json =
            serde_json::to_string(&prefs.group_display_ids).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            "UPDATE brightness_preferences SET
                sync_group = ?1,
                step_percent = ?2,
                group_display_ids = ?3,
                hotkey_brightness_target = ?4,
                hotkey_brightness_up = ?5,
                hotkey_brightness_down = ?6,
                hotkey_brightness_screen = ?7,
                updated_at = CURRENT_TIMESTAMP
             WHERE id = 1;",
            params![
                if prefs.sync_group { 1 } else { 0 },
                prefs.step_percent as i64,
                group_ids_json,
                prefs.hotkey_brightness_target,
                prefs.hotkey_brightness_up,
                prefs.hotkey_brightness_down,
                prefs.hotkey_brightness_screen,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn get_monitor_preferences(&self) -> Result<Vec<BrightnessMonitorPreference>, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut stmt = conn
            .prepare(
                "SELECT display_key, preferred_backend, last_verification_state,
                        last_verification_connection, custom_name
                 FROM brightness_monitor_preferences;",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |r| {
                let key: String = r.get(0)?;
                let backend_str: Option<String> = r.get(1)?;
                let ver_str: String = r.get(2)?;
                let conn_str: Option<String> = r.get(3)?;
                let name: Option<String> = r.get(4)?;

                let preferred_backend = backend_str
                    .as_deref()
                    .and_then(BrightnessBackend::from_str_opt);
                let last_verification_state = VerificationState::from_str_safe(&ver_str);

                Ok(BrightnessMonitorPreference {
                    display_key: key,
                    preferred_backend,
                    last_verification_state,
                    last_verification_connection: conn_str,
                    custom_name: name,
                })
            })
            .map_err(|e| e.to_string())?;

        let list = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(list)
    }

    pub fn save_monitor_preference(
        &self,
        pref: &BrightnessMonitorPreference,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let backend_str = pref.preferred_backend.map(|b| b.as_str().to_string());
        let ver_str = pref.last_verification_state.as_str();

        conn.execute(
            "INSERT INTO brightness_monitor_preferences (
                display_key, preferred_backend, last_verification_state,
                last_verification_connection, custom_name, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
             ON CONFLICT(display_key) DO UPDATE SET
                preferred_backend = excluded.preferred_backend,
                last_verification_state = excluded.last_verification_state,
                last_verification_connection = excluded.last_verification_connection,
                custom_name = COALESCE(excluded.custom_name, brightness_monitor_preferences.custom_name),
                updated_at = CURRENT_TIMESTAMP;",
            params![
                pref.display_key,
                backend_str,
                ver_str,
                pref.last_verification_connection,
                pref.custom_name,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn get_brightness_recovery(
        &self,
        display_key: &str,
    ) -> Result<Option<BrightnessTestRecoveryRecord>, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let rec = conn
            .query_row(
                "SELECT display_key, connection_key, backend, original_raw, test_raw, stage, created_at, test_id
                 FROM brightness_test_recovery WHERE display_key = ?1;",
                params![display_key],
                |r| {
                    Ok(BrightnessTestRecoveryRecord {
                        display_key: r.get(0)?,
                        connection_key: r.get(1)?,
                        backend: r.get(2)?,
                        original_raw: r.get::<_, i64>(3)? as u32,
                        test_raw: r.get::<_, i64>(4)? as u32,
                        stage: r.get(5)?,
                        created_at: r.get(6)?,
                        test_id: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;

        Ok(rec)
    }

    pub fn get_all_brightness_recoveries(
        &self,
    ) -> Result<Vec<BrightnessTestRecoveryRecord>, String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        let mut stmt = conn
            .prepare(
                "SELECT display_key, connection_key, backend, original_raw, test_raw, stage, created_at, test_id
                 FROM brightness_test_recovery;",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |r| {
                Ok(BrightnessTestRecoveryRecord {
                    display_key: r.get(0)?,
                    connection_key: r.get(1)?,
                    backend: r.get(2)?,
                    original_raw: r.get::<_, i64>(3)? as u32,
                    test_raw: r.get::<_, i64>(4)? as u32,
                    stage: r.get(5)?,
                    created_at: r.get(6)?,
                    test_id: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                })
            })
            .map_err(|e| e.to_string())?;

        let list = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(list)
    }

    pub fn set_brightness_recovery(
        &self,
        record: &BrightnessTestRecoveryRecord,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "INSERT INTO brightness_test_recovery (
                display_key, connection_key, backend, original_raw, test_raw, stage, created_at, test_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP, ?7)
             ON CONFLICT(display_key) DO UPDATE SET
                connection_key = excluded.connection_key,
                backend = excluded.backend,
                original_raw = excluded.original_raw,
                test_raw = excluded.test_raw,
                stage = excluded.stage,
                test_id = excluded.test_id,
                created_at = CURRENT_TIMESTAMP;",
            params![
                record.display_key,
                record.connection_key,
                record.backend,
                record.original_raw as i64,
                record.test_raw as i64,
                record.stage,
                record.test_id,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn delete_brightness_recovery_for_test(
        &self,
        display_key: &str,
        test_id: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "DELETE FROM brightness_test_recovery WHERE display_key = ?1 AND (test_id = ?2 OR test_id = '');",
            params![display_key, test_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn delete_brightness_recovery(&self, display_key: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "db mutex poisoned")?;
        conn.execute(
            "DELETE FROM brightness_test_recovery WHERE display_key = ?1;",
            params![display_key],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }
}

fn is_plausible_image_duplicate(path1: Option<&Path>, path2: Option<&Path>) -> bool {
    match (path1, path2) {
        (Some(p1), Some(p2)) if p1.exists() && p2.exists() => {
            if let (Ok(img1), Ok(img2)) = (image::open(p1), image::open(p2)) {
                if img1.width() != img2.width() || img1.height() != img2.height() {
                    return false;
                }
                let h1 = blake3::hash(img1.to_rgba8().as_raw());
                let h2 = blake3::hash(img2.to_rgba8().as_raw());
                return h1 == h2;
            }
            false
        }
        _ => false,
    }
}
