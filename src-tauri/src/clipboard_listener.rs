use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassExW, TranslateMessage, HWND_MESSAGE, MSG, WM_CLIPBOARDUPDATE, WM_DESTROY,
    WNDCLASSEXW,
};

use crate::db::Db;
use crate::win32_clipboard::{get_current_sequence_number, read_clipboard_content};

pub struct ClipboardState {
    pub last_own_seq: AtomicU32,
    pub last_own_hash: Mutex<String>,
    pub is_running: AtomicBool,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardState {
    pub fn new() -> Self {
        Self {
            last_own_seq: AtomicU32::new(0),
            last_own_hash: Mutex::new(String::new()),
            is_running: AtomicBool::new(false),
        }
    }

    pub fn set_own_copy(&self, seq: u32, hash: String) {
        self.last_own_seq.store(seq, Ordering::SeqCst);
        if let Ok(mut lock) = self.last_own_hash.lock() {
            *lock = hash;
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn start_clipboard_listener(app: AppHandle, clip_state: Arc<ClipboardState>) {
    if clip_state.is_running.swap(true, Ordering::SeqCst) {
        return; // Already running
    }

    let clip_state_clone = clip_state.clone();

    thread::spawn(move || unsafe {
        let class_name: Vec<u16> = "AvelClipboardListenerClass\0".encode_utf16().collect();

        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: ptr::null_mut(),
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        RegisterClassExW(&wnd_class);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
        );

        if hwnd.is_null() {
            clip_state_clone.is_running.store(false, Ordering::SeqCst);
            return;
        }

        if AddClipboardFormatListener(hwnd) == 0 {
            DestroyWindow(hwnd);
            clip_state_clone.is_running.store(false, Ordering::SeqCst);
            return;
        }

        let mut msg: MSG = std::mem::zeroed();

        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_CLIPBOARDUPDATE {
                let current_seq = get_current_sequence_number();
                let last_own_seq = clip_state_clone.last_own_seq.load(Ordering::SeqCst);

                // Small pause for system write finalization
                thread::sleep(Duration::from_millis(25));

                let db = app.state::<Arc<Db>>();
                if let Some(extracted) = read_clipboard_content(&db.images_dir) {
                    let is_own = if current_seq == last_own_seq {
                        true
                    } else if let Ok(lock) = clip_state_clone.last_own_hash.lock() {
                        !lock.is_empty() && *lock == extracted.hash
                    } else {
                        false
                    };

                    if is_own {
                        clip_state_clone.last_own_seq.store(0, Ordering::SeqCst);
                        if let Ok(mut lock) = clip_state_clone.last_own_hash.lock() {
                            lock.clear();
                        }
                    } else {
                        let img_rel = if extracted.content_type == "image" {
                            Some(format!("{}.png", extracted.hash))
                        } else {
                            None
                        };

                        if let Ok(item) = db.insert_or_update_clipboard_item(
                            &extracted.content_type,
                            extracted.text_content.as_deref(),
                            img_rel.as_deref(),
                            extracted.thumbnail_b64.as_deref(),
                            extracted.char_count,
                            extracted.width,
                            extracted.height,
                            &extracted.hash,
                        ) {
                            let _ = app.emit("clipboard-updated", &item);
                        }
                    }
                }
            }

            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        RemoveClipboardFormatListener(hwnd);
        DestroyWindow(hwnd);
        clip_state_clone.is_running.store(false, Ordering::SeqCst);
    });
}
