use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW,
    PostQuitMessage, RegisterClassExW, TranslateMessage, MSG, WM_CLOSE, WM_DESTROY,
    WM_DISPLAYCHANGE, WNDCLASSEXW, WS_POPUP,
};

use super::service::BrightnessService;

const WM_POWERBROADCAST: u32 = 0x0218;

pub struct BrightnessEventWatcher {
    is_running: Arc<AtomicBool>,
    hwnd: Arc<AtomicIsize>,
    notify_tx: Arc<Mutex<Option<mpsc::SyncSender<()>>>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Default for BrightnessEventWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl BrightnessEventWatcher {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            hwnd: Arc::new(AtomicIsize::new(0)),
            notify_tx: Arc::new(Mutex::new(None)),
            threads: Mutex::new(Vec::new()),
        }
    }

    pub fn start(&self, service: Arc<BrightnessService>) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return;
        }

        let (tx, rx) = mpsc::sync_channel::<()>(1);
        if let Ok(mut lock) = self.notify_tx.lock() {
            *lock = Some(tx.clone());
        }

        let is_running_coord = self.is_running.clone();
        let s = service;

        // Debounce coordinator thread: replaces spawning a new thread on every display change / power event
        let coord_handle = thread::spawn(move || {
            while is_running_coord.load(Ordering::Relaxed) {
                match rx.recv() {
                    Ok(()) => {
                        if !is_running_coord.load(Ordering::Relaxed) {
                            break;
                        }

                        // Drain burst events for 500ms debounce
                        loop {
                            match rx.recv_timeout(Duration::from_millis(500)) {
                                Ok(()) => {
                                    // Received newer event within debounce window, keep extending
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    break;
                                }
                                Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    return;
                                }
                            }
                            if !is_running_coord.load(Ordering::Relaxed) {
                                return;
                            }
                        }

                        if is_running_coord.load(Ordering::Relaxed) {
                            s.refresh();
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let is_running_wnd = self.is_running.clone();
        let hwnd_atomic = self.hwnd.clone();
        let tx_wnd = tx;

        let wnd_handle = thread::spawn(move || unsafe {
            let class_name: Vec<u16> = "AvelBrightnessEventClass\0".encode_utf16().collect();

            unsafe extern "system" fn wnd_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                match msg {
                    WM_CLOSE => {
                        DestroyWindow(hwnd);
                        0
                    }
                    WM_DESTROY => {
                        PostQuitMessage(0);
                        0
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }

            let wnd_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: std::ptr::null_mut(),
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: std::ptr::null_mut(),
            };

            RegisterClassExW(&wnd_class);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );

            if hwnd.is_null() {
                is_running_wnd.store(false, Ordering::SeqCst);
                return;
            }

            hwnd_atomic.store(hwnd as isize, Ordering::SeqCst);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if msg.message == WM_DISPLAYCHANGE || msg.message == WM_POWERBROADCAST {
                    let _ = tx_wnd.try_send(());
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            hwnd_atomic.store(0, Ordering::SeqCst);
            is_running_wnd.store(false, Ordering::SeqCst);
        });

        if let Ok(mut threads) = self.threads.lock() {
            threads.push(coord_handle);
            threads.push(wnd_handle);
        }
    }

    pub fn stop(&self) {
        if self.is_running.swap(false, Ordering::SeqCst) {
            // Drop sender to unblock coordinator
            if let Ok(mut lock) = self.notify_tx.lock() {
                *lock = None;
            }
            let h = self.hwnd.swap(0, Ordering::SeqCst);
            if h != 0 {
                unsafe {
                    PostMessageW(h as HWND, WM_CLOSE, 0, 0);
                }
            }
        }
        let handles: Vec<JoinHandle<()>> = if let Ok(mut lock) = self.threads.lock() {
            std::mem::take(&mut *lock)
        } else {
            Vec::new()
        };
        for h in handles {
            let _ = h.join();
        }
    }
}

impl Drop for BrightnessEventWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}
