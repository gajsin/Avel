use std::sync::Mutex;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, APPBARDATA};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, IsWindowVisible, SetWindowPos, ShowWindow, ShowWindowAsync,
    SWP_HIDEWINDOW, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW,
};

use crate::db::Db;
use crate::types::CleanDesktopState;

const ABS_AUTOHIDE: u32 = 1;
const ABS_ALWAYSONTOP: u32 = 2;

pub trait DesktopBackend: Send + Sync {
    fn are_desktop_icons_hidden(&self) -> Result<bool, String>;
    fn set_desktop_icons_hidden(&self, hidden: bool) -> Result<(), String>;
    fn is_primary_taskbar_hidden(&self) -> Result<bool, String>;
    fn set_taskbars_hidden(&self, hidden: bool) -> Result<(), String>;
}

#[derive(Default)]
pub struct Win32DesktopBackend;

impl Win32DesktopBackend {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopBackend for Win32DesktopBackend {
    fn are_desktop_icons_hidden(&self) -> Result<bool, String> {
        Ok(are_desktop_icons_hidden())
    }

    fn set_desktop_icons_hidden(&self, hidden: bool) -> Result<(), String> {
        set_desktop_icons_hidden(hidden);
        Ok(())
    }

    fn is_primary_taskbar_hidden(&self) -> Result<bool, String> {
        Ok(is_primary_taskbar_hidden())
    }

    fn set_taskbars_hidden(&self, hidden: bool) -> Result<(), String> {
        set_taskbars_hidden(hidden);
        Ok(())
    }
}

pub struct DesktopManager {
    lock: Mutex<()>,
    backend: Box<dyn DesktopBackend>,
}

impl Default for DesktopManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopManager {
    pub fn new() -> Self {
        Self::with_backend(Box::new(Win32DesktopBackend::new()))
    }

    pub fn with_backend(backend: Box<dyn DesktopBackend>) -> Self {
        Self {
            lock: Mutex::new(()),
            backend,
        }
    }

    pub fn get_actual_state(&self, db: &Db) -> CleanDesktopState {
        let icons_hidden = self.backend.are_desktop_icons_hidden().unwrap_or(false);
        let taskbar_hidden = self.backend.is_primary_taskbar_hidden().unwrap_or(false);
        let saved = db.get_clean_desktop_state().unwrap_or_default();

        let mode_normalized = match saved.current_mode.as_str() {
            "icons" | "icons_only" => "icons",
            "taskbar" | "taskbar_only" => "taskbar",
            "all" => "all",
            _ => "none",
        };

        let effective_mode = if saved.is_hidden && mode_normalized != "none" {
            mode_normalized.to_string()
        } else {
            "none".to_string()
        };

        CleanDesktopState {
            current_mode: effective_mode,
            is_hidden: saved.is_hidden && mode_normalized != "none",
            actual_icons_hidden: icons_hidden,
            actual_taskbar_hidden: taskbar_hidden,
        }
    }

    pub fn apply_mode(&self, db: &Db, mode: &str) -> Result<CleanDesktopState, String> {
        let _guard = self.lock.lock().map_err(|_| "Desktop lock poisoned")?;

        let saved = db.get_clean_desktop_state().unwrap_or_default();

        let normalized_mode = match mode {
            "none" => "none",
            "icons" | "icons_only" => "icons",
            "taskbar" | "taskbar_only" => "taskbar",
            "all" => "all",
            _ => return Err(format!("Unknown mode: {mode}")),
        };

        if normalized_mode == "none" {
            // Restore to baseline
            let (orig_icons, orig_taskbar) = db.get_clean_desktop_baseline()?;

            self.backend.set_desktop_icons_hidden(orig_icons)?;
            self.backend.set_taskbars_hidden(orig_taskbar)?;

            // Verify restoration before clearing marker
            let actual_icons = self
                .backend
                .are_desktop_icons_hidden()
                .unwrap_or(orig_icons);
            let actual_taskbar = self
                .backend
                .is_primary_taskbar_hidden()
                .unwrap_or(orig_taskbar);
            if actual_icons == orig_icons && actual_taskbar == orig_taskbar {
                db.set_recovery_marker(false)?;
            }

            // Retain the last chosen mode (or default to icons if none) so toggling on restores it!
            let retained_mode = if saved.current_mode.is_empty() || saved.current_mode == "none" {
                "icons".to_string()
            } else {
                saved.current_mode
            };
            db.save_clean_desktop_state(&retained_mode, false)?;
        } else {
            // Hiding
            if !saved.is_hidden {
                // If not currently hidden, snapshot current OS state as baseline
                let current_icons = self.backend.are_desktop_icons_hidden()?;
                let current_taskbar = self.backend.is_primary_taskbar_hidden()?;
                db.save_clean_desktop_baseline(current_icons, current_taskbar)?;
            }

            db.set_recovery_marker(true)?;

            match normalized_mode {
                "icons" => {
                    self.backend.set_desktop_icons_hidden(true)?;
                }
                "taskbar" => {
                    self.backend.set_taskbars_hidden(true)?;
                }
                "all" => {
                    self.backend.set_desktop_icons_hidden(true)?;
                    self.backend.set_taskbars_hidden(true)?;
                }
                _ => unreachable!(),
            }

            db.save_clean_desktop_state(normalized_mode, true)?;
        }

        Ok(self.get_actual_state(db))
    }

    pub fn toggle_mode(&self, db: &Db) -> Result<CleanDesktopState, String> {
        let saved = db.get_clean_desktop_state().unwrap_or_default();
        if saved.is_hidden {
            self.apply_mode(db, "none")
        } else {
            let target_mode = if saved.current_mode.is_empty() || saved.current_mode == "none" {
                "icons"
            } else {
                &saved.current_mode
            };
            self.apply_mode(db, target_mode)
        }
    }

    pub fn restore_all_on_startup_or_exit(&self, db: &Db) {
        if db.get_recovery_marker().unwrap_or(false) {
            let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
            let (orig_icons, orig_taskbar) =
                db.get_clean_desktop_baseline().unwrap_or((false, false));
            let _ = self.backend.set_desktop_icons_hidden(orig_icons);
            let _ = self.backend.set_taskbars_hidden(orig_taskbar);
            let _ = db.set_recovery_marker(false);
            let saved = db.get_clean_desktop_state().unwrap_or_default();
            let _ = db.save_clean_desktop_state(&saved.current_mode, false);
        }
    }
}

pub fn get_all_defview_windows() -> Vec<HWND> {
    let mut list = Vec::new();
    unsafe {
        // 1. Direct Progman -> SHELLDLL_DefView
        if let Ok(progman) = FindWindowW(w!("Progman"), None) {
            if progman.0 as usize != 0 {
                if let Ok(dv) = FindWindowExW(
                    progman,
                    HWND(std::ptr::null_mut()),
                    w!("SHELLDLL_DefView"),
                    None,
                ) {
                    if dv.0 as usize != 0 {
                        list.push(dv);
                    }
                }
            }
        }

        // 2. Iterate all top-level WorkerW windows -> SHELLDLL_DefView
        let mut worker = HWND(std::ptr::null_mut());
        loop {
            match FindWindowExW(HWND(std::ptr::null_mut()), worker, w!("WorkerW"), None) {
                Ok(w) if w.0 as usize != 0 => {
                    worker = w;
                    if let Ok(dv) = FindWindowExW(
                        worker,
                        HWND(std::ptr::null_mut()),
                        w!("SHELLDLL_DefView"),
                        None,
                    ) {
                        if dv.0 as usize != 0 {
                            list.push(dv);
                        }
                    }
                }
                _ => break,
            }
        }
    }
    list
}

pub fn get_desktop_listview_windows() -> Vec<HWND> {
    let mut list = Vec::new();
    let defviews = get_all_defview_windows();
    for dv in defviews {
        unsafe {
            if let Ok(lv) = FindWindowExW(dv, HWND(std::ptr::null_mut()), w!("SysListView32"), None)
            {
                if lv.0 as usize != 0 {
                    list.push(lv);
                }
            }
        }
    }
    list
}

pub fn are_desktop_icons_hidden() -> bool {
    let listviews = get_desktop_listview_windows();
    if !listviews.is_empty() {
        for lv in listviews {
            unsafe {
                if !IsWindowVisible(lv).as_bool() {
                    return true;
                }
            }
        }
    }
    false
}

pub fn set_desktop_icons_hidden(hidden: bool) {
    // 1. Ensure SHELLDLL_DefView is always visible so wallpaper NEVER disappears
    for dv in get_all_defview_windows() {
        unsafe {
            let _ = ShowWindow(dv, SW_SHOW);
            let _ = ShowWindowAsync(dv, SW_SHOW);
            let _ = SetWindowPos(
                dv,
                HWND(std::ptr::null_mut()),
                0,
                0,
                0,
                0,
                SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }

    // 2. Hide or show only the SysListView32 icon control
    let cmd = if hidden { SW_HIDE } else { SW_SHOW };
    let swp_flags = if hidden {
        SWP_HIDEWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER
    } else {
        SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER
    };

    for lv in get_desktop_listview_windows() {
        unsafe {
            let _ = ShowWindow(lv, cmd);
            let _ = ShowWindowAsync(lv, cmd);
            let _ = SetWindowPos(lv, HWND(std::ptr::null_mut()), 0, 0, 0, 0, swp_flags);
        }
    }
}

pub fn is_primary_taskbar_hidden() -> bool {
    unsafe {
        let hwnd = FindWindowW(w!("Shell_TrayWnd"), None).unwrap_or(HWND(std::ptr::null_mut()));
        let mut abd = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            ..Default::default()
        };
        let state = SHAppBarMessage(ABM_GETSTATE, &mut abd);
        (state & (ABS_AUTOHIDE as usize)) != 0
    }
}

pub fn set_taskbars_hidden(hidden: bool) {
    unsafe {
        let hwnd = FindWindowW(w!("Shell_TrayWnd"), None).unwrap_or(HWND(std::ptr::null_mut()));
        let mut abd = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            lParam: LPARAM(if hidden {
                (ABS_AUTOHIDE | ABS_ALWAYSONTOP) as isize
            } else {
                ABS_ALWAYSONTOP as isize
            }),
            ..Default::default()
        };
        SHAppBarMessage(ABM_SETSTATE, &mut abd);
    }
}
