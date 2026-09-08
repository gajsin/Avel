use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::UI::Shell::{FOLDERID_Screenshots, SHGetKnownFolderPath, KF_FLAG_DEFAULT};
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::CancelIoEx;

use crate::db::Db;
use crate::thumbnails::ThumbnailService;

#[repr(C, align(4))]
struct AlignedNotifyBuffer([u8; 8192]);

pub fn get_default_screenshots_dir() -> Option<PathBuf> {
    unsafe {
        if let Ok(pwstr) = SHGetKnownFolderPath(&FOLDERID_Screenshots, KF_FLAG_DEFAULT, None) {
            let path_str = pwstr.to_string().unwrap_or_default();
            windows::Win32::System::Com::CoTaskMemFree(Some(pwstr.0 as *mut _));
            if !path_str.is_empty() {
                let p = PathBuf::from(path_str);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
    }

    if let Some(pic) = dirs::picture_dir() {
        let sc = pic.join("Screenshots");
        if sc.is_dir() {
            return Some(sc);
        }
    }

    None
}

pub struct ScreenshotWatcher {
    pub is_running: Arc<AtomicBool>,
    dir_handle: Arc<AtomicIsize>,
    join_handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl Default for ScreenshotWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ScreenshotWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

impl ScreenshotWatcher {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            dir_handle: Arc::new(AtomicIsize::new(0)),
            join_handle: Mutex::new(None),
        }
    }

    pub fn stop(&self) {
        if self.is_running.swap(false, Ordering::SeqCst) {
            let h = self.dir_handle.swap(0, Ordering::SeqCst);
            if h != 0 && h != -1 {
                unsafe {
                    CancelIoEx(h as _, ptr::null());
                }
            }
        }
        if let Ok(mut lock) = self.join_handle.lock() {
            if let Some(jh) = lock.take() {
                let _ = jh.join();
            }
        }
    }

    pub fn start(&self, app: AppHandle, custom_dir: Option<PathBuf>) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return;
        }

        let target_dir = match custom_dir.or_else(get_default_screenshots_dir) {
            Some(dir) if dir.is_dir() => dir,
            _ => {
                // Directory absent: do not run with a fake substitute
                self.is_running.store(false, Ordering::SeqCst);
                return;
            }
        };

        let is_running = self.is_running.clone();
        let dir_handle = self.dir_handle.clone();

        let jh = thread::spawn(move || {
            // Background scan for recent screenshots (non-blocking)
            background_scan(&app, &target_dir);

            let wide_dir: Vec<u16> = target_dir
                .as_os_str()
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let handle = unsafe {
                CreateFileW(
                    wide_dir.as_ptr(),
                    FILE_LIST_DIRECTORY,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS,
                    ptr::null_mut(),
                )
            };

            if handle == INVALID_HANDLE_VALUE || handle.is_null() {
                is_running.store(false, Ordering::SeqCst);
                return;
            }

            dir_handle.store(handle as isize, Ordering::SeqCst);

            let mut buffer = AlignedNotifyBuffer([0u8; 8192]);
            let mut bytes_returned = 0u32;
            let mut last_processed: std::collections::HashMap<PathBuf, std::time::Instant> =
                std::collections::HashMap::new();

            while is_running.load(Ordering::Relaxed) {
                let success = unsafe {
                    ReadDirectoryChangesW(
                        handle,
                        buffer.0.as_mut_ptr() as *mut _,
                        buffer.0.len() as u32,
                        0,
                        FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE,
                        &mut bytes_returned,
                        ptr::null_mut(),
                        None,
                    )
                };

                if !is_running.load(Ordering::Relaxed) {
                    break;
                }

                if success == 0 || bytes_returned == 0 {
                    thread::sleep(Duration::from_millis(200));
                    continue;
                }

                let mut offset = 0usize;
                let total = bytes_returned as usize;

                while offset + 12 <= total {
                    if !offset.is_multiple_of(4) {
                        break;
                    }

                    let next_offset = match buffer.0[offset..offset + 4].try_into() {
                        Ok(arr) => u32::from_ne_bytes(arr) as usize,
                        Err(_) => break,
                    };
                    let file_name_bytes = match buffer.0[offset + 8..offset + 12].try_into() {
                        Ok(arr) => u32::from_ne_bytes(arr) as usize,
                        Err(_) => break,
                    };

                    if file_name_bytes == 0 || (file_name_bytes % 2) != 0 {
                        break;
                    }

                    let name_start = offset + 12;
                    let _name_end = match name_start.checked_add(file_name_bytes) {
                        Some(end) if end <= total => end,
                        _ => break,
                    };

                    let char_count = file_name_bytes / 2;
                    let mut u16_chars = Vec::with_capacity(char_count);
                    for i in 0..char_count {
                        let idx = name_start + i * 2;
                        u16_chars.push(u16::from_ne_bytes([buffer.0[idx], buffer.0[idx + 1]]));
                    }
                    let filename = String::from_utf16_lossy(&u16_chars);

                    if is_image_extension(&filename) {
                        let full_path = target_dir.join(&filename);
                        let now = std::time::Instant::now();
                        let should_process = !matches!(
                            last_processed.get(&full_path),
                            Some(&prev) if now.duration_since(prev) < Duration::from_millis(1500)
                        );

                        if should_process {
                            last_processed.insert(full_path.clone(), now);
                            if last_processed.len() > 100 {
                                last_processed.retain(|_, &mut prev| {
                                    now.duration_since(prev) < Duration::from_secs(60)
                                });
                            }
                            let app_c = app.clone();
                            let path_c = full_path;
                            tauri::async_runtime::spawn_blocking(move || {
                                process_screenshot_file_with_retries(&app_c, &path_c);
                            });
                        }
                    }

                    if next_offset == 0 {
                        break;
                    }
                    if !next_offset.is_multiple_of(4)
                        || offset.checked_add(next_offset).is_none_or(|o| o >= total)
                    {
                        break;
                    }
                    offset += next_offset;
                }
            }

            dir_handle.store(0, Ordering::SeqCst);
            unsafe {
                CloseHandle(handle);
            }
        });

        if let Ok(mut lock) = self.join_handle.lock() {
            *lock = Some(jh);
        }
    }
}

fn is_image_extension(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".webp")
}

fn background_scan(app: &AppHandle, dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_image_extension(&p.to_string_lossy()))
            .collect();

        files.sort_by_key(|p| {
            fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });
        files.reverse();

        for file in files.into_iter().take(30) {
            process_screenshot_file_with_retries(app, &file);
        }
    }
}

fn process_screenshot_file_with_retries(app: &AppHandle, path: &Path) {
    // Retry up to 4 times with backoff to wait for file writer to finish writing
    for attempt in 0..4 {
        if path.is_file() {
            if let Ok(meta) = fs::metadata(path) {
                if meta.len() > 0 {
                    if let Ok(reader) = image::ImageReader::open(path) {
                        if reader.into_dimensions().is_ok() {
                            process_screenshot_file(app, path);
                            return;
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(50 * (attempt + 1)));
    }
}

fn process_screenshot_file(app: &AppHandle, path: &Path) {
    if !path.is_file() {
        return;
    }

    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("screenshot.png");
    let path_str = path.to_string_lossy().to_string();

    // Startup scanning must not make an already-known screenshot look new again.
    let db = app.state::<Arc<Db>>();
    if db
        .get_clipboard_items(Some("images"), None)
        .map(|items| {
            items.iter().any(|item| {
                item.content_type == "screenshot"
                    && item.image_path.as_deref() == Some(path_str.as_str())
            })
        })
        .unwrap_or(false)
    {
        return;
    }

    let hash_str = blake3::hash(path_str.as_bytes()).to_hex().to_string();

    // Check if dimensions are known or generate thumbnail with ThumbnailService
    let thumb_service = app.state::<Arc<ThumbnailService>>();
    let thumb_b64 = thumb_service
        .get_or_generate_thumbnail(&path_str, 220, 160, 2.0)
        .ok();

    let (w, h) = if let Ok(reader) = image::ImageReader::open(path) {
        reader
            .into_dimensions()
            .ok()
            .map(|(w, h)| (Some(w), Some(h)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    if let Ok(item) = db.insert_or_update_clipboard_item(
        "screenshot",
        Some(filename),
        Some(&path_str),
        thumb_b64.as_deref(),
        None,
        w,
        h,
        &hash_str,
    ) {
        let _ = app.emit("clipboard-updated", &item);
    }
}
