use std::sync::Arc;
use tauri::State;

use super::model::{BrightnessPreferences, BrightnessStateSnapshot};
use super::service::BrightnessService;

#[tauri::command]
pub fn brightness_get_state(
    service: State<'_, Arc<BrightnessService>>,
) -> Result<BrightnessStateSnapshot, String> {
    Ok(service.get_snapshot())
}

#[tauri::command]
pub async fn brightness_refresh(
    service: State<'_, Arc<BrightnessService>>,
) -> Result<BrightnessStateSnapshot, String> {
    let s = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || s.refresh())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn brightness_set(
    service: State<'_, Arc<BrightnessService>>,
    display_id: String,
    target_percent: u32,
) -> Result<(), String> {
    service.set_brightness(&display_id, target_percent)
}

#[tauri::command]
pub fn brightness_adjust(
    service: State<'_, Arc<BrightnessService>>,
    delta_percent: i32,
    target: Option<String>,
) -> Result<(), String> {
    service.adjust_brightness(delta_percent, target.as_deref())
}

#[tauri::command]
pub fn brightness_set_group(
    service: State<'_, Arc<BrightnessService>>,
    target_percent: u32,
) -> Result<Vec<(String, bool)>, String> {
    service.set_group_brightness(target_percent)
}

#[tauri::command]
pub fn brightness_start_test(
    service: State<'_, Arc<BrightnessService>>,
    display_id: String,
) -> Result<(), String> {
    service.start_test(&display_id)
}

#[tauri::command]
pub fn brightness_cancel_test(
    service: State<'_, Arc<BrightnessService>>,
    display_id: String,
) -> Result<(), String> {
    service.cancel_test(&display_id)
}

#[tauri::command]
pub fn brightness_confirm_test(
    service: State<'_, Arc<BrightnessService>>,
    display_id: String,
    user_confirmed: bool,
) -> Result<(), String> {
    service.confirm_test(&display_id, user_confirmed)
}

#[tauri::command]
pub fn brightness_get_preferences(
    service: State<'_, Arc<BrightnessService>>,
) -> Result<BrightnessPreferences, String> {
    Ok(service.get_preferences())
}

#[tauri::command]
pub fn brightness_save_preferences(
    service: State<'_, Arc<BrightnessService>>,
    preferences: BrightnessPreferences,
) -> Result<(), String> {
    service.save_preferences(preferences)
}

#[tauri::command]
pub fn brightness_get_diagnostics(
    service: State<'_, Arc<BrightnessService>>,
) -> Result<String, String> {
    Ok(service.get_diagnostics_report())
}
