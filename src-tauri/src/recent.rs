use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use base64::Engine;
use image::{DynamicImage, ImageFormat, RgbaImage};
use tauri::{AppHandle, Emitter, Manager};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
use windows::Win32::System::Com::*;
use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::db::Db;

struct ComScope(bool);
impl ComScope {
    fn new() -> Self {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
            ComScope(hr.is_ok())
        }
    }
}
impl Drop for ComScope {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

pub struct RecentWatcher {
    pub is_running: Arc<AtomicBool>,
}

impl Default for RecentWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl RecentWatcher {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    pub fn start(&self, app: AppHandle) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return;
        }

        let is_running = self.is_running.clone();

        thread::spawn(move || {
            let _com = ComScope::new();

            let mut known_windows: HashMap<String, String> = HashMap::new();
            let mut known_recent_files: HashMap<PathBuf, SystemTime> = HashMap::new();
            let mut last_explorer_active = Instant::now();

            let recent_dir =
                dirs::data_dir().map(|p| p.join("Microsoft").join("Windows").join("Recent"));

            // 1. Snapshot initial Recent directory to ignore preexisting historical shortcuts
            if let Some(ref rdir) = recent_dir {
                if let Ok(entries) = fs::read_dir(rdir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("lnk"))
                            .unwrap_or(false)
                        {
                            if let Ok(meta) = entry.metadata() {
                                if let Ok(mtime) = meta.modified() {
                                    known_recent_files.insert(path, mtime);
                                }
                            }
                        }
                    }
                }
            }

            // Snapshot initial open Explorer windows
            if let Ok(initial_wins) = get_current_explorer_windows() {
                known_windows = initial_wins;
            }

            while is_running.load(Ordering::Relaxed) {
                // Check Explorer foreground activity
                let explorer_active = is_explorer_active();
                if explorer_active {
                    last_explorer_active = Instant::now();
                }

                // Check closed Explorer windows
                if let Ok(current_windows) = get_current_explorer_windows() {
                    let closed_paths =
                        detect_closed_explorer_windows(&mut known_windows, &current_windows);
                    for path in closed_paths {
                        process_recent_entry(app.clone(), path, "folder");
                    }
                }

                // Check new Recent .lnk files
                if let Some(ref rdir) = recent_dir {
                    if let Ok(entries) = fs::read_dir(rdir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if !path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.eq_ignore_ascii_case("lnk"))
                                .unwrap_or(false)
                            {
                                continue;
                            }

                            if let Ok(meta) = entry.metadata() {
                                if let Ok(mtime) = meta.modified() {
                                    let is_new = match known_recent_files.get(&path) {
                                        Some(&prev) => mtime > prev,
                                        None => true,
                                    };

                                    if is_new {
                                        known_recent_files.insert(path.clone(), mtime);
                                        if explorer_active
                                            || last_explorer_active.elapsed()
                                                <= Duration::from_millis(3000)
                                        {
                                            if let Some(target) = resolve_shortcut_target(&path) {
                                                let target_p = Path::new(&target);
                                                if target_p.is_file() {
                                                    process_recent_entry(
                                                        app.clone(),
                                                        target,
                                                        "file",
                                                    );
                                                } else if target_p.is_dir() {
                                                    process_recent_entry(
                                                        app.clone(),
                                                        target,
                                                        "folder",
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                thread::sleep(Duration::from_millis(500));
            }
        });
    }
}

impl Drop for RecentWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

static ACTIVE_THUMBNAIL_TASKS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn process_recent_entry(app: AppHandle, raw_path: String, entry_type: &'static str) {
    tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&raw_path);
        if !p.exists() {
            return;
        }

        let title = p
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(&raw_path)
            .to_string();
        let norm_path = raw_path.trim().to_string();

        // Limit concurrent COM thumbnail extraction tasks to at most 3
        let thumb_b64 = if ACTIVE_THUMBNAIL_TASKS.fetch_add(1, Ordering::SeqCst) < 3 {
            let res = extract_shell_thumbnail(&norm_path);
            ACTIVE_THUMBNAIL_TASKS.fetch_sub(1, Ordering::SeqCst);
            res
        } else {
            ACTIVE_THUMBNAIL_TASKS.fetch_sub(1, Ordering::SeqCst);
            None
        };

        let db = app.state::<Arc<Db>>();
        if let Ok(item) =
            db.insert_or_update_recent_item(entry_type, &title, &norm_path, thumb_b64.as_deref())
        {
            let _ = app.emit("recent-updated", &item);
        }
    });
}

pub fn extract_shell_thumbnail(path_str: &str) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;

    let p = Path::new(path_str);
    let is_dir = p.is_dir();

    let _com = ComScope::new();

    unsafe {
        let wide_path: Vec<u16> = p
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        if let Ok(shell_item) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(wide_path.as_ptr()), None)
        {
            if let Ok(factory) = shell_item.cast::<IShellItemImageFactory>() {
                if is_dir {
                    // For folders, strictly request ICON ONLY (no random content composite) at 256x256 / 128x128
                    if let Ok(hbitmap) =
                        factory.GetImage(SIZE { cx: 256, cy: 256 }, SIIGBF_ICONONLY)
                    {
                        let res = hbitmap_to_png_base64(hbitmap);
                        let _ = DeleteObject(HGDIOBJ(hbitmap.0 as _));
                        if res.is_some() {
                            return res;
                        }
                    }
                    if let Ok(hbitmap) =
                        factory.GetImage(SIZE { cx: 128, cy: 128 }, SIIGBF_ICONONLY)
                    {
                        let res = hbitmap_to_png_base64(hbitmap);
                        let _ = DeleteObject(HGDIOBJ(hbitmap.0 as _));
                        if res.is_some() {
                            return res;
                        }
                    }
                } else {
                    // For files, try full size first for real images, then fallback to icon
                    let size = SIZE { cx: 256, cy: 256 };
                    if let Ok(hbitmap) = factory.GetImage(size, SIIGBF_BIGGERSIZEOK) {
                        let res = hbitmap_to_png_base64(hbitmap);
                        let _ = DeleteObject(HGDIOBJ(hbitmap.0 as _));
                        if res.is_some() {
                            return res;
                        }
                    }
                    if let Ok(hbitmap) =
                        factory.GetImage(SIZE { cx: 128, cy: 128 }, SIIGBF_ICONONLY)
                    {
                        let res = hbitmap_to_png_base64(hbitmap);
                        let _ = DeleteObject(HGDIOBJ(hbitmap.0 as _));
                        if res.is_some() {
                            return res;
                        }
                    }
                }
            }
        }
    }

    None
}

unsafe fn hbitmap_to_png_base64(hbitmap: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<String> {
    let mut bm = BITMAP::default();
    if GetObjectW(
        HGDIOBJ(hbitmap.0 as _),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    ) == 0
    {
        return None;
    }

    let (w, h) = (bm.bmWidth as u32, bm.bmHeight.unsigned_abs());
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return None;
    }

    let pixel_bytes = (w as usize).checked_mul(h as usize)?.checked_mul(4)?;

    let hdc = CreateCompatibleDC(None);
    if hdc.0 as usize == 0 {
        return None;
    }

    let mut bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32), // Top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD::default()],
    };

    let mut pixels = vec![0u8; pixel_bytes];
    let lines = GetDIBits(
        hdc,
        hbitmap,
        0,
        h,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bi,
        DIB_RGB_COLORS,
    );
    let _ = DeleteDC(hdc);

    if lines == 0 {
        return None;
    }

    let has_any_alpha = pixels.iter().skip(3).step_by(4).any(|&p| p > 0);

    let mut img = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let b = pixels[idx];
            let g = pixels[idx + 1];
            let r = pixels[idx + 2];
            let raw_a = pixels[idx + 3];

            let alpha = if has_any_alpha { raw_a } else { 255 };

            img.put_pixel(x, y, image::Rgba([r, g, b, alpha]));
        }
    }

    let mut thumb_bytes = Vec::new();
    let dyn_img = DynamicImage::ImageRgba8(img);
    let _ = dyn_img.write_to(&mut Cursor::new(&mut thumb_bytes), ImageFormat::Png);

    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&thumb_bytes)
    ))
}

fn resolve_shortcut_target(lnk_path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;

    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        let wide_path: Vec<u16> = lnk_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        persist.Load(PCWSTR(wide_path.as_ptr()), STGM_READ).ok()?;

        let mut path_buf = [0u16; 1024];
        let mut find_data = WIN32_FIND_DATAW::default();
        link.GetPath(&mut path_buf, &mut find_data, 0).ok()?;

        let len = path_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(path_buf.len());
        if len == 0 {
            return None;
        }

        let target_str = String::from_utf16_lossy(&path_buf[..len]);
        if target_str.is_empty() {
            return None;
        }

        Some(target_str)
    }
}

fn is_explorer_active() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 as usize == 0 {
            return false;
        }

        let root_hwnd = GetAncestor(hwnd, GA_ROOT);
        for h in [hwnd, root_hwnd] {
            if h.0 as usize == 0 {
                continue;
            }
            let mut class_name = [0u16; 256];
            let len = GetClassNameW(h, &mut class_name);
            if len > 0 {
                let cls = String::from_utf16_lossy(&class_name[..len as usize]);
                if cls == "CabinetWClass"
                    || cls == "ExploreWClass"
                    || cls == "Progman"
                    || cls == "WorkerW"
                    || cls == "Shell_TrayWnd"
                {
                    return true;
                }
            }
        }

        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 {
            if let Ok(proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut name_buf = [0u16; 256];
                let nlen = GetModuleBaseNameW(proc, None, &mut name_buf);
                let _ = CloseHandle(proc);
                if nlen > 0 {
                    let proc_name =
                        String::from_utf16_lossy(&name_buf[..nlen as usize]).to_lowercase();
                    if proc_name == "explorer.exe" || proc_name == "explorer" {
                        return true;
                    }
                }
            }
        }

        false
    }
}

fn get_current_explorer_windows() -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    unsafe {
        if let Ok(shell_windows) =
            CoCreateInstance::<_, IShellWindows>(&ShellWindows, None, CLSCTX_ALL)
        {
            let count = shell_windows.Count().unwrap_or(0);
            for i in 0..count {
                let var = windows::core::VARIANT::from(i);
                if let Ok(disp) = shell_windows.Item(&var) {
                    if let Ok(browser) = disp.cast::<IWebBrowser2>() {
                        let hwnd = browser.HWND().map(|h| h.0 as usize).unwrap_or(0);
                        let url = browser
                            .LocationURL()
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        let name = browser
                            .LocationName()
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        let path = parse_explorer_url_or_path(&url, &name);
                        if !path.is_empty() {
                            // Key by HWND and COM pointer identity to distinguish tabs and track navigations
                            let key = format!("0x{:X}_{:p}", hwnd, browser.as_raw());
                            map.insert(key, path);
                        }
                    }
                }
            }
        }
    }

    Ok(map)
}

pub fn detect_closed_explorer_windows(
    known: &mut HashMap<String, String>,
    current: &HashMap<String, String>,
) -> Vec<String> {
    let mut closed = Vec::new();

    // Any window/tab key in known that is no longer in current was closed
    let removed_keys: Vec<String> = known
        .keys()
        .filter(|k| !current.contains_key(*k))
        .cloned()
        .collect();

    for key in removed_keys {
        if let Some(path) = known.remove(&key) {
            if is_valid_folder_path(&path) {
                closed.push(path);
            }
        }
    }

    // Update existing or add newly opened windows with their latest path
    for (key, path) in current {
        known.insert(key.clone(), path.clone());
    }

    closed
}

pub fn parse_explorer_url_or_path(url: &str, name: &str) -> String {
    if let Some(raw) = url.strip_prefix("file:///") {
        let decoded = url_decode(&raw.replace('/', "\\"));
        if decoded.starts_with(r"\\") {
            decoded
        } else if decoded.starts_with('\\') && !decoded.contains(':') {
            format!(r"\{}", decoded)
        } else {
            decoded
        }
    } else if let Some(raw) = url.strip_prefix("file://") {
        let decoded = url_decode(&raw.replace('/', "\\"));
        if decoded.starts_with(r"\\") {
            decoded
        } else {
            format!(r"\\{}", decoded.trim_start_matches('\\'))
        }
    } else if !url.is_empty() && !url.starts_with("http") && !url.starts_with("shell:") {
        url.to_string()
    } else if !name.is_empty()
        && !name.starts_with("shell:")
        && (name.contains('\\') || name.contains(':'))
    {
        name.to_string()
    } else {
        String::new()
    }
}

pub fn is_valid_folder_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    let is_drive = trimmed.len() >= 2 && trimmed.chars().nth(1) == Some(':');
    let is_unc = trimmed.starts_with(r"\\");
    is_drive || is_unc
}

pub fn url_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = bytes[i + 1] as char;
            let h2 = bytes[i + 2] as char;
            if let (Some(d1), Some(d2)) = (h1.to_digit(16), h2.to_digit(16)) {
                out.push(((d1 << 4) | d2) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
