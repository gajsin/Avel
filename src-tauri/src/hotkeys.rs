use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use crate::brightness::BrightnessService;
use crate::db::Db;
use crate::desktop::DesktopManager;
use crate::theme::{read_registry_theme, ThemeCoordinator};
use crate::types::ShortcutSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HotkeyAction {
    Clipboard,
    Recent,
    Appearance,
    Desktop,
    BrightnessUp,
    BrightnessDown,
    BrightnessScreen,
}

impl HotkeyAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            HotkeyAction::Clipboard => "clipboard",
            HotkeyAction::Recent => "recent",
            HotkeyAction::Appearance => "appearance",
            HotkeyAction::Desktop => "desktop",
            HotkeyAction::BrightnessUp => "brightness_up",
            HotkeyAction::BrightnessDown => "brightness_down",
            HotkeyAction::BrightnessScreen => "brightness_screen",
        }
    }
}

impl std::str::FromStr for HotkeyAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "clipboard" => Ok(HotkeyAction::Clipboard),
            "recent" => Ok(HotkeyAction::Recent),
            "appearance" => Ok(HotkeyAction::Appearance),
            "desktop" => Ok(HotkeyAction::Desktop),
            "brightness_up" => Ok(HotkeyAction::BrightnessUp),
            "brightness_down" => Ok(HotkeyAction::BrightnessDown),
            "brightness_screen" => Ok(HotkeyAction::BrightnessScreen),
            _ => Err(format!("Неизвестное действие для хоткея: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyStatusInfo {
    pub is_registered: bool,
    pub error_message: Option<String>,
}

pub struct HotkeyState {
    // Maps Shortcut -> HotkeyAction
    pub shortcut_routes: Mutex<HashMap<Shortcut, HotkeyAction>>,
    // Maps Shortcut ID -> HotkeyAction
    pub id_routes: Mutex<HashMap<u32, HotkeyAction>>,
    // Maps HotkeyAction -> current registered Shortcut
    pub active_shortcuts: Mutex<HashMap<HotkeyAction, Shortcut>>,
    // Status info for frontend: action str -> HotkeyStatusInfo
    pub statuses: Mutex<HashMap<String, HotkeyStatusInfo>>,
}

impl Default for HotkeyState {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyState {
    pub fn new() -> Self {
        Self {
            shortcut_routes: Mutex::new(HashMap::new()),
            id_routes: Mutex::new(HashMap::new()),
            active_shortcuts: Mutex::new(HashMap::new()),
            statuses: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_status(&self, action: HotkeyAction, is_reg: bool, err: Option<String>) {
        if let Ok(mut map) = self.statuses.lock() {
            map.insert(
                action.as_str().to_string(),
                HotkeyStatusInfo {
                    is_registered: is_reg,
                    error_message: err,
                },
            );
        }
    }

    pub fn get_statuses(&self) -> HashMap<String, HotkeyStatusInfo> {
        self.statuses.lock().map(|m| m.clone()).unwrap_or_default()
    }

    pub fn resolve_action(&self, shortcut: &Shortcut) -> Option<HotkeyAction> {
        let sc_map = self.shortcut_routes.lock().ok();
        let by_sc = sc_map.as_ref().and_then(|m| m.get(shortcut).copied());
        if by_sc.is_some() {
            by_sc
        } else {
            let id_map = self.id_routes.lock().ok();
            id_map.as_ref().and_then(|m| m.get(&shortcut.id()).copied())
        }
    }

    pub fn handle_event(&self, app: &AppHandle, shortcut: &Shortcut) {
        if let Some(action) = self.resolve_action(shortcut) {
            log::info!(
                "[hotkey_pressed] action={:?} shortcut={:?} id={}",
                action,
                shortcut,
                shortcut.id()
            );
            execute_hotkey_action(app, action);
        } else {
            log::warn!(
                "[hotkey_unmatched] shortcut={:?} id={}",
                shortcut,
                shortcut.id()
            );
        }
    }
}

pub fn normalize_key_code(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_lowercase();
    match lower.as_str() {
        "up" | "arrowup" => "ArrowUp".to_string(),
        "down" | "arrowdown" => "ArrowDown".to_string(),
        "left" | "arrowleft" => "ArrowLeft".to_string(),
        "right" | "arrowright" => "ArrowRight".to_string(),
        "pageup" => "PageUp".to_string(),
        "pagedown" => "PageDown".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "space" => "Space".to_string(),
        "enter" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" => "Backspace".to_string(),
        "del" | "delete" => "Delete".to_string(),
        "ins" | "insert" => "Insert".to_string(),
        "esc" | "escape" => "Escape".to_string(),
        "equal" | "=" => "Equal".to_string(),
        "minus" | "-" => "Minus".to_string(),
        "bracketleft" | "[" => "BracketLeft".to_string(),
        "bracketright" | "]" => "BracketRight".to_string(),
        "backslash" | "\\" => "Backslash".to_string(),
        "semicolon" | ";" => "Semicolon".to_string(),
        "quote" | "'" => "Quote".to_string(),
        "comma" | "," => "Comma".to_string(),
        "period" | "." => "Period".to_string(),
        "slash" | "/" => "Slash".to_string(),
        "backquote" | "`" => "Backquote".to_string(),
        "numpad0" => "Numpad0".to_string(),
        "numpad1" => "Numpad1".to_string(),
        "numpad2" => "Numpad2".to_string(),
        "numpad3" => "Numpad3".to_string(),
        "numpad4" => "Numpad4".to_string(),
        "numpad5" => "Numpad5".to_string(),
        "numpad6" => "Numpad6".to_string(),
        "numpad7" => "Numpad7".to_string(),
        "numpad8" => "Numpad8".to_string(),
        "numpad9" => "Numpad9".to_string(),
        "numpadadd" => "NumpadAdd".to_string(),
        "numpadsubtract" => "NumpadSubtract".to_string(),
        "numpadmultiply" => "NumpadMultiply".to_string(),
        "numpaddivide" => "NumpadDivide".to_string(),
        "numpaddecimal" => "NumpadDecimal".to_string(),
        "numpadenter" => "NumpadEnter".to_string(),
        "numpadequal" => "NumpadEqual".to_string(),
        _ => {
            if let Some(rest) = lower.strip_prefix('f') {
                if let Ok(n) = rest.parse::<u32>() {
                    if (1..=24).contains(&n) {
                        return format!("F{n}");
                    }
                }
            }
            if trimmed.len() == 1 {
                let ch = trimmed.chars().next().unwrap();
                if ch.is_ascii_alphabetic() {
                    return format!("Key{}", ch.to_ascii_uppercase());
                }
                if ch.is_ascii_digit() {
                    return format!("Digit{ch}");
                }
            }
            if let Some(rest) = lower.strip_prefix("key") {
                if rest.len() == 1 && rest.chars().next().unwrap().is_ascii_alphabetic() {
                    return format!("Key{}", rest.to_ascii_uppercase());
                }
            }
            if let Some(rest) = lower.strip_prefix("digit") {
                if rest.len() == 1 && rest.chars().next().unwrap().is_ascii_digit() {
                    return format!("Digit{rest}");
                }
            }
            trimmed.to_string()
        }
    }
}

pub fn parse_code(code_str: &str) -> Result<Code, String> {
    let normalized = normalize_key_code(code_str);
    let trimmed = normalized.as_str();
    match trimmed {
        // Letters
        "KeyA" => Ok(Code::KeyA),
        "KeyB" => Ok(Code::KeyB),
        "KeyC" => Ok(Code::KeyC),
        "KeyD" => Ok(Code::KeyD),
        "KeyE" => Ok(Code::KeyE),
        "KeyF" => Ok(Code::KeyF),
        "KeyG" => Ok(Code::KeyG),
        "KeyH" => Ok(Code::KeyH),
        "KeyI" => Ok(Code::KeyI),
        "KeyJ" => Ok(Code::KeyJ),
        "KeyK" => Ok(Code::KeyK),
        "KeyL" => Ok(Code::KeyL),
        "KeyM" => Ok(Code::KeyM),
        "KeyN" => Ok(Code::KeyN),
        "KeyO" => Ok(Code::KeyO),
        "KeyP" => Ok(Code::KeyP),
        "KeyQ" => Ok(Code::KeyQ),
        "KeyR" => Ok(Code::KeyR),
        "KeyS" => Ok(Code::KeyS),
        "KeyT" => Ok(Code::KeyT),
        "KeyU" => Ok(Code::KeyU),
        "KeyV" => Ok(Code::KeyV),
        "KeyW" => Ok(Code::KeyW),
        "KeyX" => Ok(Code::KeyX),
        "KeyY" => Ok(Code::KeyY),
        "KeyZ" => Ok(Code::KeyZ),

        // Digits
        "Digit0" => Ok(Code::Digit0),
        "Digit1" => Ok(Code::Digit1),
        "Digit2" => Ok(Code::Digit2),
        "Digit3" => Ok(Code::Digit3),
        "Digit4" => Ok(Code::Digit4),
        "Digit5" => Ok(Code::Digit5),
        "Digit6" => Ok(Code::Digit6),
        "Digit7" => Ok(Code::Digit7),
        "Digit8" => Ok(Code::Digit8),
        "Digit9" => Ok(Code::Digit9),

        // Arrows
        "ArrowUp" => Ok(Code::ArrowUp),
        "ArrowDown" => Ok(Code::ArrowDown),
        "ArrowLeft" => Ok(Code::ArrowLeft),
        "ArrowRight" => Ok(Code::ArrowRight),

        // Function Keys
        "F1" => Ok(Code::F1),
        "F2" => Ok(Code::F2),
        "F3" => Ok(Code::F3),
        "F4" => Ok(Code::F4),
        "F5" => Ok(Code::F5),
        "F6" => Ok(Code::F6),
        "F7" => Ok(Code::F7),
        "F8" => Ok(Code::F8),
        "F9" => Ok(Code::F9),
        "F10" => Ok(Code::F10),
        "F11" => Ok(Code::F11),
        "F12" => Err("F12 зарезервирована Windows для системного отладчика".to_string()),
        "F13" => Ok(Code::F13),
        "F14" => Ok(Code::F14),
        "F15" => Ok(Code::F15),
        "F16" => Ok(Code::F16),
        "F17" => Ok(Code::F17),
        "F18" => Ok(Code::F18),
        "F19" => Ok(Code::F19),
        "F20" => Ok(Code::F20),
        "F21" => Ok(Code::F21),
        "F22" => Ok(Code::F22),
        "F23" => Ok(Code::F23),
        "F24" => Ok(Code::F24),

        // Standard Keys
        "Space" => Ok(Code::Space),
        "Enter" => Ok(Code::Enter),
        "Tab" => Ok(Code::Tab),
        "Backspace" => Ok(Code::Backspace),
        "Delete" => Ok(Code::Delete),
        "Insert" => Ok(Code::Insert),
        "Home" => Ok(Code::Home),
        "End" => Ok(Code::End),
        "PageUp" => Ok(Code::PageUp),
        "PageDown" => Ok(Code::PageDown),
        "Escape" => Ok(Code::Escape),

        // Symbols
        "Equal" => Ok(Code::Equal),
        "Minus" => Ok(Code::Minus),
        "BracketLeft" => Ok(Code::BracketLeft),
        "BracketRight" => Ok(Code::BracketRight),
        "Backslash" => Ok(Code::Backslash),
        "Semicolon" => Ok(Code::Semicolon),
        "Quote" => Ok(Code::Quote),
        "Comma" => Ok(Code::Comma),
        "Period" => Ok(Code::Period),
        "Slash" => Ok(Code::Slash),
        "Backquote" => Ok(Code::Backquote),

        // Numpad
        "Numpad0" => Ok(Code::Numpad0),
        "Numpad1" => Ok(Code::Numpad1),
        "Numpad2" => Ok(Code::Numpad2),
        "Numpad3" => Ok(Code::Numpad3),
        "Numpad4" => Ok(Code::Numpad4),
        "Numpad5" => Ok(Code::Numpad5),
        "Numpad6" => Ok(Code::Numpad6),
        "Numpad7" => Ok(Code::Numpad7),
        "Numpad8" => Ok(Code::Numpad8),
        "Numpad9" => Ok(Code::Numpad9),
        "NumpadAdd" => Ok(Code::NumpadAdd),
        "NumpadSubtract" => Ok(Code::NumpadSubtract),
        "NumpadMultiply" => Ok(Code::NumpadMultiply),
        "NumpadDivide" => Ok(Code::NumpadDivide),
        "NumpadDecimal" => Ok(Code::NumpadDecimal),
        "NumpadEnter" => Ok(Code::NumpadEnter),
        "NumpadEqual" => Ok(Code::NumpadEqual),

        _ => Err(format!("Неизвестная клавиша: {trimmed}")),
    }
}

pub fn spec_to_shortcut(spec: &ShortcutSpec) -> Result<Shortcut, String> {
    if spec.code.trim().is_empty() {
        return Err("Хоткей не задан".to_string());
    }
    let code = parse_code(&spec.code)?;
    let mut mods = Modifiers::empty();
    if spec.ctrl {
        mods |= Modifiers::CONTROL;
    }
    if spec.alt {
        mods |= Modifiers::ALT;
    }
    if spec.shift {
        mods |= Modifiers::SHIFT;
    }
    if spec.meta {
        mods |= Modifiers::SUPER;
    }

    if mods.is_empty() {
        Ok(Shortcut::new(None, code))
    } else {
        Ok(Shortcut::new(Some(mods), code))
    }
}

pub fn parse_shortcut_spec_or_str(s: &str, fallback_code: &str) -> ShortcutSpec {
    let trimmed = s.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("unset")
    {
        return ShortcutSpec {
            code: if fallback_code.is_empty() {
                String::new()
            } else {
                fallback_code.to_string()
            },
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        };
    }

    if let Ok(mut spec) = serde_json::from_str::<ShortcutSpec>(trimmed) {
        if spec.code.is_empty() && !fallback_code.is_empty() {
            spec.code = fallback_code.to_string();
        }
        return spec;
    }

    let parts: Vec<&str> = trimmed.split('+').map(|p| p.trim()).collect();
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    let mut code = String::new();

    for p in parts {
        match p.to_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            "win" | "meta" | "cmd" | "command" => meta = true,
            _ => {
                let norm = normalize_key_code(p);
                if !norm.is_empty() {
                    code = norm;
                }
            }
        }
    }

    if code.is_empty() && !fallback_code.is_empty() {
        code = fallback_code.to_string();
    }

    ShortcutSpec {
        code,
        ctrl,
        alt,
        shift,
        meta,
    }
}

pub fn register_action_shortcut(app: &AppHandle, action: HotkeyAction, spec: &ShortcutSpec) {
    let state = match app.try_state::<Arc<HotkeyState>>() {
        Some(s) => s,
        None => return,
    };

    if spec.code.trim().is_empty() {
        // Unset shortcut -> do not register in OS, leave status inactive without error
        state.set_status(action, false, None);
        return;
    }

    let shortcut = match spec_to_shortcut(spec) {
        Ok(sc) => {
            log::info!(
                "[hotkey_build_success] action={:?} spec={:?} shortcut={:?}",
                action,
                spec,
                sc
            );
            sc
        }
        Err(e) => {
            log::warn!(
                "[hotkey_build_failed] action={:?} spec={:?} error={}",
                action,
                spec,
                e
            );
            state.set_status(action, false, Some(e));
            return;
        }
    };

    let gs = app.global_shortcut();
    log::info!(
        "[hotkey_register_attempt] action={:?} shortcut={:?} id={}",
        action,
        shortcut,
        shortcut.id()
    );

    match gs.register(shortcut) {
        Ok(()) => {
            log::info!(
                "[hotkey_register_success] action={:?} shortcut={:?} id={}",
                action,
                shortcut,
                shortcut.id()
            );
            if let Ok(mut sc_routes) = state.shortcut_routes.lock() {
                sc_routes.insert(shortcut, action);
            }
            if let Ok(mut id_routes) = state.id_routes.lock() {
                id_routes.insert(shortcut.id(), action);
            }
            if let Ok(mut active) = state.active_shortcuts.lock() {
                active.insert(action, shortcut);
            }
            state.set_status(action, true, None);
        }
        Err(e) => {
            let err_msg = format!(
                "{} уже используется другой программой или занят системой",
                spec.code
            );
            log::warn!(
                "[hotkey_register_failed] action={:?} shortcut={:?} raw_err={:?}",
                action,
                shortcut,
                e
            );
            state.set_status(action, false, Some(err_msg));
        }
    }
}

pub fn update_hotkey_in_db(db: &Db, action: HotkeyAction, spec_json: &str) -> Result<(), String> {
    match action {
        HotkeyAction::Clipboard => {
            let mut settings = db.get_settings()?;
            settings.hotkey_clipboard = spec_json.to_string();
            db.save_settings(&settings)
        }
        HotkeyAction::Recent => {
            let mut settings = db.get_settings()?;
            settings.hotkey_recent = spec_json.to_string();
            db.save_settings(&settings)
        }
        HotkeyAction::Appearance => {
            let mut settings = db.get_settings()?;
            settings.hotkey_appearance = spec_json.to_string();
            db.save_settings(&settings)
        }
        HotkeyAction::Desktop => {
            let mut settings = db.get_settings()?;
            settings.hotkey_desktop = spec_json.to_string();
            db.save_settings(&settings)
        }
        HotkeyAction::BrightnessUp => {
            let mut bp = db.get_brightness_preferences()?;
            bp.hotkey_brightness_up = Some(spec_json.to_string());
            db.save_brightness_preferences(&bp)
        }
        HotkeyAction::BrightnessDown => {
            let mut bp = db.get_brightness_preferences()?;
            bp.hotkey_brightness_down = Some(spec_json.to_string());
            db.save_brightness_preferences(&bp)
        }
        HotkeyAction::BrightnessScreen => {
            let mut bp = db.get_brightness_preferences()?;
            bp.hotkey_brightness_screen = Some(spec_json.to_string());
            db.save_brightness_preferences(&bp)
        }
    }
}

static HOTKEY_MUTEX: Mutex<()> = Mutex::new(());

pub fn update_hotkey_transactional(
    app: &AppHandle,
    action: HotkeyAction,
    new_spec: &ShortcutSpec,
) -> Result<(), String> {
    let _mutex_guard = HOTKEY_MUTEX.lock().map_err(|_| "Hotkey mutex poisoned")?;

    let state = app
        .try_state::<Arc<HotkeyState>>()
        .ok_or_else(|| "HotkeyState not available".to_string())?;

    let new_shortcut = spec_to_shortcut(new_spec)?;

    // 1. Check internal conflict with another action
    let old_shortcut = {
        let active = state.active_shortcuts.lock().unwrap();
        for (act, sc) in active.iter() {
            if *act != action && *sc == new_shortcut {
                return Err(
                    "Эта комбинация уже используется для другого действия в Avel.".to_string(),
                );
            }
        }
        // If exact same shortcut already registered for this action, it's a no-op
        if let Some(current_sc) = active.get(&action) {
            if *current_sc == new_shortcut {
                state.set_status(action, true, None);
                return Ok(());
            }
        }
        active.get(&action).copied()
    };

    let gs = app.global_shortcut();
    log::info!(
        "[hotkey_update_attempt] action={:?} new_shortcut={:?}",
        action,
        new_shortcut
    );

    // 2. Try registering new shortcut in OS first
    gs.register(new_shortcut).map_err(|e| {
        log::warn!(
            "[hotkey_update_failed] action={:?} new_shortcut={:?} err={:?}",
            action,
            new_shortcut,
            e
        );
        format!(
            "{} уже используется другой программой или процессом.",
            new_spec.code
        )
    })?;

    // 3. Persist to DB. If DB fails, rollback the OS registration immediately!
    let db = app.state::<Arc<Db>>();
    let json_str = serde_json::to_string(new_spec).unwrap_or_else(|_| new_spec.code.clone());
    let db_res = update_hotkey_in_db(&db, action, &json_str);

    if let Err(e) = db_res {
        // Rollback: unregister the newly registered shortcut
        let unreg_res = gs.unregister(new_shortcut);
        if let Some(old) = old_shortcut {
            let _ = gs.register(old);
        }
        if let Err(unreg_err) = unreg_res {
            return Err(format!(
                "Ошибка БД ({e}) и сбой компенсации отката регистрации хоткея в ОС: {unreg_err:?}"
            ));
        }
        return Err(format!(
            "Не удалось сохранить настройки хоткея в базу данных: {e}"
        ));
    }

    // 4. Unregister old shortcut if present
    if let Some(old_sc) = old_shortcut {
        if old_sc != new_shortcut {
            log::info!(
                "[hotkey_unregister_attempt] action={:?} old_shortcut={:?}",
                action,
                old_sc
            );
            let _ = gs.unregister(old_sc);
            if let Ok(mut sc_routes) = state.shortcut_routes.lock() {
                sc_routes.remove(&old_sc);
            }
            if let Ok(mut id_routes) = state.id_routes.lock() {
                id_routes.remove(&old_sc.id());
            }
        }
    }

    // 5. Register in internal maps
    if let Ok(mut sc_routes) = state.shortcut_routes.lock() {
        sc_routes.insert(new_shortcut, action);
    }
    if let Ok(mut id_routes) = state.id_routes.lock() {
        id_routes.insert(new_shortcut.id(), action);
    }
    if let Ok(mut active) = state.active_shortcuts.lock() {
        active.insert(action, new_shortcut);
    }
    state.set_status(action, true, None);

    Ok(())
}

pub fn register_all_hotkeys(app: &AppHandle) {
    let db = app.state::<Arc<Db>>();
    let settings = db.get_settings().unwrap_or_default();
    let b_prefs = db.get_brightness_preferences().unwrap_or_default();

    log::info!("[hotkey_startup_load] Registering all global hotkeys independently...");

    let clip_spec = parse_shortcut_spec_or_str(&settings.hotkey_clipboard, "Equal");
    let rec_spec = parse_shortcut_spec_or_str(&settings.hotkey_recent, "F8");
    let app_spec = parse_shortcut_spec_or_str(&settings.hotkey_appearance, "KeyD");
    let desk_spec = parse_shortcut_spec_or_str(&settings.hotkey_desktop, "ArrowDown");

    register_action_shortcut(app, HotkeyAction::Clipboard, &clip_spec);
    register_action_shortcut(app, HotkeyAction::Recent, &rec_spec);
    register_action_shortcut(app, HotkeyAction::Appearance, &app_spec);
    register_action_shortcut(app, HotkeyAction::Desktop, &desk_spec);

    if let Some(ref up_str) = b_prefs.hotkey_brightness_up {
        if !up_str.trim().is_empty() {
            let b_up = parse_shortcut_spec_or_str(up_str, "PageUp");
            register_action_shortcut(app, HotkeyAction::BrightnessUp, &b_up);
        }
    }
    if let Some(ref down_str) = b_prefs.hotkey_brightness_down {
        if !down_str.trim().is_empty() {
            let b_down = parse_shortcut_spec_or_str(down_str, "PageDown");
            register_action_shortcut(app, HotkeyAction::BrightnessDown, &b_down);
        }
    }
    if let Some(ref screen_str) = b_prefs.hotkey_brightness_screen {
        if !screen_str.trim().is_empty() {
            let b_screen = parse_shortcut_spec_or_str(screen_str, "KeyB");
            register_action_shortcut(app, HotkeyAction::BrightnessScreen, &b_screen);
        }
    }
}

fn execute_hotkey_action(app: &AppHandle, action: HotkeyAction) {
    log::info!("[hotkey_action_started] action={:?}", action);
    match action {
        HotkeyAction::Clipboard => {
            show_or_toggle_section(app, "clipboard");
            log::info!("[hotkey_action_success] action=Clipboard");
        }
        HotkeyAction::Recent => {
            show_or_toggle_section(app, "recent");
            log::info!("[hotkey_action_success] action=Recent");
        }
        HotkeyAction::Appearance => {
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
            let _ = coordinator.apply_theme(app, target_light, cfg.switch_wallpaper, Some(wp));
            log::info!(
                "[hotkey_action_success] action=Appearance target_light={}",
                target_light
            );
        }
        HotkeyAction::Desktop => {
            let db = app.state::<Arc<Db>>();
            let desktop = app.state::<Arc<DesktopManager>>();
            if let Ok(state) = desktop.toggle_mode(&db) {
                let _ = app.emit("desktop-state-changed", &state);
                log::info!("[hotkey_action_success] action=Desktop state={:?}", state);
            }
        }
        HotkeyAction::BrightnessScreen => {
            show_or_toggle_section(app, "brightness");
            log::info!("[hotkey_action_success] action=BrightnessScreen");
        }
        HotkeyAction::BrightnessUp => {
            if let Some(service) = app.try_state::<Arc<BrightnessService>>() {
                let db = app.state::<Arc<Db>>();
                let prefs = db.get_brightness_preferences().unwrap_or_default();
                let step = prefs.step_percent as i32;
                let target = if prefs.sync_group {
                    Some("group")
                } else {
                    None
                };
                let _ = service.adjust_brightness(step, target);
                log::info!("[hotkey_action_success] action=BrightnessUp step=+{}", step);
            }
        }
        HotkeyAction::BrightnessDown => {
            if let Some(service) = app.try_state::<Arc<BrightnessService>>() {
                let db = app.state::<Arc<Db>>();
                let prefs = db.get_brightness_preferences().unwrap_or_default();
                let step = prefs.step_percent as i32;
                let target = if prefs.sync_group {
                    Some("group")
                } else {
                    None
                };
                let _ = service.adjust_brightness(-step, target);
                log::info!(
                    "[hotkey_action_success] action=BrightnessDown step=-{}",
                    step
                );
            }
        }
    }
}

fn show_or_toggle_section(app: &AppHandle, section: &'static str) {
    if let Some(win) = app.get_webview_window("main") {
        let is_vis = win.is_visible().unwrap_or(false);
        let is_min = win.is_minimized().unwrap_or(false);

        if is_vis && !is_min {
            let db = app.state::<Arc<Db>>();
            if let Ok(settings) = db.get_settings() {
                if settings.last_section == section {
                    let _ = win.hide();
                    return;
                }
            }
        }

        let db = app.state::<Arc<Db>>();
        if let Ok(mut s) = db.get_settings() {
            s.last_section = section.to_string();
            let _ = db.save_settings(&s);
        }

        let _ = app.emit("navigate-section", section);
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}
