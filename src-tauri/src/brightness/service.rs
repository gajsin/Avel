use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use super::backend::BrightnessDeviceBackend;
use super::ddc::{percent_to_raw, raw_to_percent};
use super::diagnostics::generate_diagnostics_report;
use super::discovery::{discover_displays, get_device_name_under_cursor};
use super::model::{
    BrightnessDisplay, BrightnessMonitorPreference, BrightnessPreferences, BrightnessStateSnapshot,
    BrightnessTestRecoveryRecord, ProbeState, VerificationState,
};
use crate::db::Db;

#[derive(Clone, Debug)]
struct WriteCommand {
    display_id: String,
    target_percent: u32,
    generation: u64,
}

#[derive(Clone, Debug)]
pub struct ActiveTestSession {
    pub test_id: u64,
    pub display_id: String,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct BrightnessService {
    db: Arc<Db>,
    backend: Arc<dyn BrightnessDeviceBackend>,
    app_handle: Option<AppHandle>,
    generation: Arc<AtomicU64>,
    revision: Arc<AtomicU64>,
    snapshot: Arc<RwLock<BrightnessStateSnapshot>>,
    cmd_tx: Arc<Mutex<Option<Sender<WriteCommand>>>>,
    active_test_lock: Arc<Mutex<Option<String>>>, // display_id currently under test
    next_test_id: Arc<AtomicU64>,
    active_session: Arc<Mutex<Option<ActiveTestSession>>>,
}

impl BrightnessService {
    pub fn new(
        db: Arc<Db>,
        backend: Arc<dyn BrightnessDeviceBackend>,
        app_handle: Option<AppHandle>,
    ) -> Arc<Self> {
        let prefs = db.get_brightness_preferences().unwrap_or_default();
        let initial_snapshot = BrightnessStateSnapshot {
            revision: 1,
            displays: Vec::new(),
            preferences: prefs,
            active_test_display_id: None,
        };

        let service = Arc::new(Self {
            db,
            backend,
            app_handle,
            generation: Arc::new(AtomicU64::new(1)),
            revision: Arc::new(AtomicU64::new(1)),
            snapshot: Arc::new(RwLock::new(initial_snapshot)),
            cmd_tx: Arc::new(Mutex::new(None)),
            active_test_lock: Arc::new(Mutex::new(None)),
            next_test_id: Arc::new(AtomicU64::new(1)),
            active_session: Arc::new(Mutex::new(None)),
        });

        service.restore_pending_tests_on_startup();
        service.start_worker();
        service
    }

    fn emit_state_changed(&self) {
        let snap = self.get_snapshot();
        if let Some(ref app) = self.app_handle {
            let _ = app.emit("brightness-state-changed", &snap);
        }
    }

    pub fn get_snapshot(&self) -> BrightnessStateSnapshot {
        self.snapshot.read().unwrap().clone()
    }

    pub fn set_test_displays(&self, displays: Vec<BrightnessDisplay>) {
        let mut w = self.snapshot.write().unwrap();
        w.displays = displays;
    }

    pub fn refresh(&self) -> BrightnessStateSnapshot {
        let next_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut discovered = discover_displays(next_gen);

        // Merge preferences and verification state from DB
        let mon_prefs = self.db.get_monitor_preferences().unwrap_or_default();
        let mon_pref_map: HashMap<String, BrightnessMonitorPreference> = mon_prefs
            .into_iter()
            .map(|p| (p.display_key.clone(), p))
            .collect();

        for d in &mut discovered {
            if let Some(p) = mon_pref_map.get(&d.id) {
                d.verification = p.last_verification_state;
                if let Some(ref name) = p.custom_name {
                    d.name = name.clone();
                }
                if let Some(backend) = p.preferred_backend {
                    d.backend = Some(backend);
                }
            }
        }

        let rev = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        let prefs = self.db.get_brightness_preferences().unwrap_or_default();
        let active_test = self.active_test_lock.lock().unwrap().clone();

        let next_snapshot = BrightnessStateSnapshot {
            revision: rev,
            displays: discovered,
            preferences: prefs,
            active_test_display_id: active_test,
        };

        {
            let mut w = self.snapshot.write().unwrap();
            *w = next_snapshot.clone();
        }

        self.emit_state_changed();
        self.get_snapshot()
    }

    pub fn refresh_async(self: &Arc<Self>) {
        let s = self.clone();
        tauri::async_runtime::spawn_blocking(move || {
            s.refresh();
        });
    }

    fn start_worker(self: &Arc<Self>) {
        let (tx, rx): (Sender<WriteCommand>, Receiver<WriteCommand>) = channel();
        *self.cmd_tx.lock().unwrap() = Some(tx);

        let weak_service = Arc::downgrade(self);

        thread::spawn(move || {
            let mut pending_targets: HashMap<String, (u32, u64)> = HashMap::new();
            let mut last_write: HashMap<String, std::time::Instant> = HashMap::new();

            loop {
                // Wait for commands or timeout to flush debounced writes
                let first_cmd = match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(cmd) => Some(cmd),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                };

                let service = match weak_service.upgrade() {
                    Some(s) => s,
                    None => break, // Service dropped, clean exit
                };

                if let Some(cmd) = first_cmd {
                    pending_targets.insert(cmd.display_id, (cmd.target_percent, cmd.generation));

                    // Drain all immediately available commands into map (coalesce)
                    while let Ok(next) = rx.try_recv() {
                        pending_targets
                            .insert(next.display_id, (next.target_percent, next.generation));
                    }
                }

                // If we have pending targets and reached debounce delay, execute them
                if !pending_targets.is_empty() {
                    let targets_to_apply: Vec<(String, u32, u64)> = pending_targets
                        .drain()
                        .map(|(id, (target, gen))| (id, target, gen))
                        .collect();

                    for (display_id, target_pct, gen) in targets_to_apply {
                        let current_gen = service.generation.load(Ordering::SeqCst);
                        if gen < current_gen {
                            continue; // Stale generation
                        }

                        let display_opt = {
                            let r = service.snapshot.read().unwrap();
                            r.displays.iter().find(|d| d.id == display_id).cloned()
                        };

                        if let Some(display) = display_opt {
                            if display.backend.is_none() {
                                continue;
                            }

                            // Minimum 50ms interval between DDC/CI writes per display
                            if let Some(last) = last_write.get(&display_id) {
                                let elapsed = last.elapsed();
                                if elapsed < Duration::from_millis(50) {
                                    thread::sleep(Duration::from_millis(50) - elapsed);
                                }
                            }

                            let min = display.raw_min.unwrap_or(0);
                            let max = display.raw_max.unwrap_or(100);
                            let raw = percent_to_raw(target_pct, min, max);

                            let res = service.backend.write_brightness(&display, raw);
                            last_write.insert(display_id.clone(), std::time::Instant::now());

                            {
                                let mut w = service.snapshot.write().unwrap();
                                let rev = service.revision.fetch_add(1, Ordering::SeqCst) + 1;
                                w.revision = rev;
                                if let Some(d) = w.displays.iter_mut().find(|d| d.id == display_id)
                                {
                                    d.busy = false;
                                    match res {
                                        Ok(()) => {
                                            d.current_percent = Some(target_pct);
                                            d.error = None;
                                        }
                                        Err(err) => {
                                            d.error = Some(err);
                                        }
                                    }
                                }
                            }
                            service.emit_state_changed();
                        }
                    }
                }
            }
        });
    }

    pub fn set_brightness(&self, display_id: &str, target_percent: u32) -> Result<(), String> {
        let current_gen = self.generation.load(Ordering::SeqCst);
        let pct = target_percent.clamp(0, 100);

        // Update target in snapshot immediately
        {
            let mut w = self.snapshot.write().unwrap();
            if let Some(d) = w.displays.iter_mut().find(|d| d.id == display_id) {
                if d.probe_state != ProbeState::ReadOk {
                    return Err("Устройство не готово к регулировке".to_string());
                }
                d.target_percent = Some(pct);
                d.busy = true;
            } else {
                return Err("Дисплей не найден".to_string());
            }
        }
        self.emit_state_changed();

        // Enqueue command
        if let Some(ref tx) = *self.cmd_tx.lock().unwrap() {
            let _ = tx.send(WriteCommand {
                display_id: display_id.to_string(),
                target_percent: pct,
                generation: current_gen,
            });
        }

        Ok(())
    }

    pub fn adjust_brightness(&self, delta: i32, target: Option<&str>) -> Result<(), String> {
        let snap = self.get_snapshot();
        let target_mode = target.unwrap_or(&snap.preferences.hotkey_brightness_target);

        if target_mode == "group" || snap.preferences.sync_group {
            for d in &snap.displays {
                if d.probe_state == ProbeState::ReadOk {
                    let cur = d.current_percent.unwrap_or(50) as i32;
                    let next = (cur + delta).clamp(0, 100) as u32;
                    let _ = self.set_brightness(&d.id, next);
                }
            }
        } else if target_mode == "cursor" {
            let cursor_target = if let Some(cur_dev) = get_device_name_under_cursor() {
                snap.displays.iter().find(|d| {
                    d.device_name.as_deref() == Some(&cur_dev)
                        && d.probe_state == ProbeState::ReadOk
                })
            } else {
                None
            };

            let target_disp = cursor_target
                .or_else(|| {
                    snap.displays
                        .iter()
                        .find(|d| d.is_primary && d.probe_state == ProbeState::ReadOk)
                })
                .or_else(|| {
                    snap.displays
                        .iter()
                        .find(|d| d.probe_state == ProbeState::ReadOk)
                });

            if let Some(target) = target_disp {
                let cur = target.current_percent.unwrap_or(50) as i32;
                let next = (cur + delta).clamp(0, 100) as u32;
                self.set_brightness(&target.id, next)?;
            }
        } else {
            // Target first available display (or primary)
            if let Some(primary) = snap
                .displays
                .iter()
                .find(|d| d.is_primary && d.probe_state == ProbeState::ReadOk)
                .or_else(|| {
                    snap.displays
                        .iter()
                        .find(|d| d.probe_state == ProbeState::ReadOk)
                })
            {
                let cur = primary.current_percent.unwrap_or(50) as i32;
                let next = (cur + delta).clamp(0, 100) as u32;
                self.set_brightness(&primary.id, next)?;
            }
        }

        Ok(())
    }

    pub fn set_group_brightness(&self, target_percent: u32) -> Result<Vec<(String, bool)>, String> {
        let snap = self.get_snapshot();
        let mut results = Vec::new();

        for d in &snap.displays {
            if d.probe_state == ProbeState::ReadOk {
                let res = self.set_brightness(&d.id, target_percent);
                results.push((d.id.clone(), res.is_ok()));
            }
        }

        Ok(results)
    }

    pub fn start_test(&self, display_id: &str) -> Result<(), String> {
        let display = {
            let r = self.snapshot.read().unwrap();
            r.displays.iter().find(|d| d.id == display_id).cloned()
        }
        .ok_or_else(|| "Дисплей не найден".to_string())?;

        if display.probe_state != ProbeState::ReadOk || display.backend.is_none() {
            return Err("Тестирование невозможно: экран не поддерживает управление".to_string());
        }

        // Exact baseline raw MUST be read before any hardware writes.
        // If read fails: ZERO hardware writes!
        let (min_raw, cur_raw, max_raw, _) =
            self.backend.read_brightness(&display).map_err(|e| {
                format!(
                    "Не удалось прочитать исходную аппаратную яркость перед тестом: {:?}",
                    e
                )
            })?;

        let test_id = self.next_test_id.fetch_add(1, Ordering::SeqCst);
        let cancelled = Arc::new(AtomicBool::new(false));

        // Cancel previous active session if running
        {
            let mut sess_guard = self.active_session.lock().unwrap();
            if let Some(ref prev) = *sess_guard {
                prev.cancelled.store(true, Ordering::SeqCst);
            }
            *sess_guard = Some(ActiveTestSession {
                test_id,
                display_id: display_id.to_string(),
                cancelled: cancelled.clone(),
            });
        }

        // Reference algorithm from C# (step = Math.Max(1, range / 5)):
        let range = max_raw.saturating_sub(min_raw);
        let step = (range / 5).max(1);
        let target_raw = if cur_raw.saturating_sub(min_raw) >= 2 * step {
            cur_raw - step
        } else {
            cur_raw + step.min(max_raw.saturating_sub(cur_raw))
        };
        let original_raw = cur_raw;

        // 1. Write recovery record to DB before modifying brightness
        let recovery = BrightnessTestRecoveryRecord {
            test_id: test_id.to_string(),
            display_key: display_id.to_string(),
            connection_key: display.connection.clone().unwrap_or_default(),
            backend: display.backend.unwrap().as_str().to_string(),
            original_raw,
            test_raw: target_raw,
            stage: format!("testing_{test_id}"),
            created_at: chrono::Local::now().to_rfc3339(),
        };
        self.db.set_brightness_recovery(&recovery)?;

        if let Ok(mut lock) = self.active_test_lock.lock() {
            *lock = Some(display_id.to_string());
        }

        // Update UI state
        {
            let mut w = self.snapshot.write().unwrap();
            w.active_test_display_id = Some(display_id.to_string());
            if let Some(d) = w.displays.iter_mut().find(|d| d.id == display_id) {
                d.verification = VerificationState::Testing;
            }
        }
        self.emit_state_changed();

        // 2. Perform test step on background thread using exact reference timing
        let s = self.clone();
        let disp_id_owned = display_id.to_string();
        let cancel_flag = cancelled;
        let test_id_str = test_id.to_string();

        tauri::async_runtime::spawn_blocking(move || {
            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }

            // Apply test brightness
            let _ = s.backend.write_brightness(&display, target_raw);

            // Wait 500ms and read back to confirm hardware response
            thread::sleep(Duration::from_millis(500));
            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }

            let mut hardware_confirmed = false;
            if let Ok((_min, read_cur, _max, _)) = s.backend.read_brightness(&display) {
                if read_cur == target_raw {
                    hardware_confirmed = true;
                }
            }

            // Wait remaining 2500ms (3.0s total test duration)
            thread::sleep(Duration::from_millis(2500));
            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }

            // Restore original brightness with up to 3 attempts (reference C# algorithm)
            let mut restored = false;
            for _ in 0..3 {
                if cancel_flag.load(Ordering::SeqCst) {
                    return;
                }
                let write_res = s.backend.write_brightness(&display, original_raw);
                thread::sleep(Duration::from_millis(500));
                if write_res.is_ok() {
                    if let Ok((_min, read_cur, _max, _)) = s.backend.read_brightness(&display) {
                        if read_cur == original_raw {
                            restored = true;
                            break;
                        }
                    } else {
                        restored = true;
                        break;
                    }
                }
            }

            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }

            // Only clear recovery journal if hardware restore succeeded!
            if restored {
                let _ =
                    s.db.delete_brightness_recovery_for_test(&disp_id_owned, &test_id_str);
            }

            {
                let mut w = s.snapshot.write().unwrap();
                if w.active_test_display_id.as_deref() == Some(&disp_id_owned) {
                    w.active_test_display_id = None;
                }
                if let Some(d) = w.displays.iter_mut().find(|d| d.id == disp_id_owned) {
                    let pct = raw_to_percent(original_raw, min_raw, max_raw);
                    d.current_percent = Some(pct);
                    d.target_percent = Some(pct);
                    if restored && hardware_confirmed {
                        d.verification = VerificationState::DeviceConfirmed;
                    } else if !restored {
                        d.verification = VerificationState::RecoveryPending;
                    }
                }
            }
            if let Ok(mut lock) = s.active_test_lock.lock() {
                if lock.as_deref() == Some(&disp_id_owned) {
                    *lock = None;
                }
            }
            if let Ok(mut sess_guard) = s.active_session.lock() {
                if let Some(ref sess) = *sess_guard {
                    if sess.test_id == test_id {
                        *sess_guard = None;
                    }
                }
            }
            s.emit_state_changed();
        });

        Ok(())
    }

    pub fn cancel_test(&self, display_id: &str) -> Result<(), String> {
        // 1. Cancel active session
        if let Ok(mut sess_guard) = self.active_session.lock() {
            if let Some(ref sess) = *sess_guard {
                if sess.display_id == display_id {
                    sess.cancelled.store(true, Ordering::SeqCst);
                }
            }
            *sess_guard = None;
        }

        let recovery_opt = self.db.get_brightness_recovery(display_id)?;
        let mut restored = false;
        if let Some(rec) = recovery_opt {
            let snap = self.get_snapshot();
            if let Some(d) = snap.displays.iter().find(|d| d.id == display_id) {
                if self.backend.write_brightness(d, rec.original_raw).is_ok() {
                    restored = true;
                    let min_raw = d.raw_min.unwrap_or(0);
                    let max_raw = d.raw_max.unwrap_or(100);
                    let pct = raw_to_percent(rec.original_raw, min_raw, max_raw);
                    let mut w = self.snapshot.write().unwrap();
                    if let Some(mut_d) = w.displays.iter_mut().find(|d| d.id == display_id) {
                        mut_d.current_percent = Some(pct);
                        mut_d.target_percent = Some(pct);
                    }
                }
            }
            if restored {
                let _ = self.db.delete_brightness_recovery(display_id);
            }
        }

        {
            let mut w = self.snapshot.write().unwrap();
            w.active_test_display_id = None;
            if let Some(d) = w.displays.iter_mut().find(|d| d.id == display_id) {
                d.verification = if restored {
                    VerificationState::NotTested
                } else {
                    VerificationState::RecoveryPending
                };
            }
        }
        if let Ok(mut lock) = self.active_test_lock.lock() {
            *lock = None;
        }
        self.emit_state_changed();
        Ok(())
    }

    pub fn confirm_test(&self, display_id: &str, user_confirmed: bool) -> Result<(), String> {
        let new_state = if user_confirmed {
            VerificationState::UserConfirmed
        } else {
            VerificationState::Failed
        };

        let pref = BrightnessMonitorPreference {
            display_key: display_id.to_string(),
            preferred_backend: None,
            last_verification_state: new_state,
            last_verification_connection: None,
            custom_name: None,
        };
        self.db.save_monitor_preference(&pref)?;

        // Restore original_raw if not yet restored, and only delete recovery if readback confirms original_raw
        if let Ok(Some(rec)) = self.db.get_brightness_recovery(display_id) {
            let snap = self.get_snapshot();
            if let Some(d) = snap.displays.iter().find(|d| d.id == display_id) {
                if let Ok((_min, cur, _max, _)) = self.backend.read_brightness(d) {
                    if cur != rec.original_raw {
                        let _ = self.backend.write_brightness(d, rec.original_raw);
                    }
                }
                if let Ok((_min, cur, _max, _)) = self.backend.read_brightness(d) {
                    if cur == rec.original_raw {
                        let _ = self
                            .db
                            .delete_brightness_recovery_for_test(display_id, &rec.test_id);
                    }
                }
            }
        }

        {
            let mut w = self.snapshot.write().unwrap();
            w.active_test_display_id = None;
            if let Some(d) = w.displays.iter_mut().find(|d| d.id == display_id) {
                d.verification = new_state;
            }
        }
        if let Ok(mut lock) = self.active_test_lock.lock() {
            *lock = None;
        }
        self.emit_state_changed();
        Ok(())
    }

    pub fn get_preferences(&self) -> BrightnessPreferences {
        self.db.get_brightness_preferences().unwrap_or_default()
    }

    pub fn save_preferences(&self, prefs: BrightnessPreferences) -> Result<(), String> {
        self.db.save_brightness_preferences(&prefs)?;
        {
            let mut w = self.snapshot.write().unwrap();
            w.preferences = prefs;
        }
        self.emit_state_changed();
        Ok(())
    }

    pub fn get_diagnostics_report(&self) -> String {
        let snap = self.get_snapshot();
        generate_diagnostics_report(&snap.displays, env!("CARGO_PKG_VERSION"))
    }

    pub fn restore_pending_tests_on_startup(&self) {
        if let Ok(records) = self.db.get_all_brightness_recoveries() {
            for rec in records {
                let snap = self.get_snapshot();
                if let Some(d) = snap.displays.iter().find(|d| d.id == rec.display_key) {
                    if self.backend.write_brightness(d, rec.original_raw).is_ok() {
                        let _ = self.db.delete_brightness_recovery(&rec.display_key);
                        let min_raw = d.raw_min.unwrap_or(0);
                        let max_raw = d.raw_max.unwrap_or(100);
                        let pct = raw_to_percent(rec.original_raw, min_raw, max_raw);
                        let mut w = self.snapshot.write().unwrap();
                        if let Some(mut_d) = w.displays.iter_mut().find(|d| d.id == rec.display_key)
                        {
                            mut_d.current_percent = Some(pct);
                            mut_d.target_percent = Some(pct);
                        }
                    }
                }
            }
        }
    }
}
