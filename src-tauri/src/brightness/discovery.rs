use std::collections::HashMap;
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
    GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, PHYSICAL_MONITOR, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{BOOL, HANDLE, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};

use super::ddc::{raw_to_percent, read_ddc_high, read_ddc_vcp, PhysicalMonitorGuard};
use super::model::{
    BrightnessBackend, BrightnessDisplay, BrightnessError, ProbeState, VerificationState,
};
use super::wmi::read_wmi_monitors;

#[derive(Clone, Debug)]
pub struct DisplayConnectionMeta {
    pub friendly_name: String,
    pub connection_type: String,
}

pub fn compute_display_id(device_name: &str, desc: &str, phys_idx: usize) -> String {
    let stable_hash_input = format!("{}:{}:{}", device_name, desc, phys_idx);
    format!(
        "disp_{}",
        &blake3::hash(stable_hash_input.as_bytes()).to_hex()[..12]
    )
}

pub fn get_display_config_metas() -> HashMap<String, DisplayConnectionMeta> {
    let mut map = HashMap::new();

    unsafe {
        let mut path_count = 0u32;
        let mut mode_count = 0u32;

        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
            .is_err()
        {
            return map;
        }

        let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![std::mem::zeroed(); path_count as usize];
        let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![std::mem::zeroed(); mode_count as usize];

        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
        .is_err()
        {
            return map;
        }

        for path in &paths[..path_count as usize] {
            let mut target_name: DISPLAYCONFIG_TARGET_DEVICE_NAME = std::mem::zeroed();
            target_name.header.size =
                std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            target_name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            target_name.header.adapterId = path.targetInfo.adapterId;
            target_name.header.id = path.targetInfo.id;

            let info_res = DisplayConfigGetDeviceInfo(&mut target_name.header);
            if info_res == 0 {
                let friendly_name = if target_name.monitorFriendlyDeviceName[0] != 0 {
                    let len = target_name
                        .monitorFriendlyDeviceName
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(target_name.monitorFriendlyDeviceName.len());
                    String::from_utf16_lossy(&target_name.monitorFriendlyDeviceName[..len])
                } else {
                    String::new()
                };

                let tech_val = target_name.outputTechnology.0;
                let conn_str = if tech_val == windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL.0
                    || tech_val == windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED.0
                {
                    "Internal".to_string()
                } else if tech_val == windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EXTERNAL.0 {
                    "DisplayPort".to_string()
                } else if tech_val == windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI.0 {
                    "HDMI".to_string()
                } else if tech_val == windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DVI.0 {
                    "DVI".to_string()
                } else if tech_val == windows::Win32::Devices::Display::DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HD15.0 {
                    "VGA".to_string()
                } else {
                    "External".to_string()
                };

                let dev_path_len = target_name
                    .monitorDevicePath
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(target_name.monitorDevicePath.len());
                let dev_path =
                    String::from_utf16_lossy(&target_name.monitorDevicePath[..dev_path_len]);

                if !dev_path.is_empty() {
                    map.insert(
                        dev_path,
                        DisplayConnectionMeta {
                            friendly_name,
                            connection_type: conn_str,
                        },
                    );
                }
            }
        }
    }

    map
}

pub struct EnumeratedMonitor {
    pub device_name: String,
    pub is_primary: bool,
    pub physical_handles: Vec<(isize, String)>,
}

unsafe extern "system" fn monitor_enum_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<EnumeratedMonitor>);

    let mut info: MONITORINFOEXW = std::mem::zeroed();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut _ as *mut _).as_bool() {
        let is_primary = (info.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY
        let len = info
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szDevice.len());
        let dev_name = String::from_utf16_lossy(&info.szDevice[..len]);

        let mut phys_count = 0u32;
        let mut physical_handles = Vec::new();

        if GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut phys_count).is_ok() && phys_count > 0
        {
            let mut phys_arr: Vec<PHYSICAL_MONITOR> = vec![std::mem::zeroed(); phys_count as usize];
            if GetPhysicalMonitorsFromHMONITOR(hmon, &mut phys_arr).is_ok() {
                for pm in phys_arr {
                    let desc_raw = pm.szPhysicalMonitorDescription;
                    let desc_len = desc_raw
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(desc_raw.len());
                    let desc = String::from_utf16_lossy(&desc_raw[..desc_len]);
                    physical_handles.push((pm.hPhysicalMonitor.0 as isize, desc));
                }
            }
        }

        monitors.push(EnumeratedMonitor {
            device_name: dev_name,
            is_primary,
            physical_handles,
        });
    }

    BOOL(1)
}

pub fn enumerate_all_monitors() -> Vec<EnumeratedMonitor> {
    let mut monitors = Vec::new();
    unsafe {
        let lparam = LPARAM(&mut monitors as *mut _ as isize);
        let _ = EnumDisplayMonitors(
            HDC(std::ptr::null_mut()),
            None,
            Some(monitor_enum_proc),
            lparam,
        );
    }
    monitors
}

pub fn discover_displays(generation: u64) -> Vec<BrightnessDisplay> {
    let metas = get_display_config_metas();
    let enum_monitors = enumerate_all_monitors();
    let wmi_monitors = read_wmi_monitors().unwrap_or_default();

    let mut displays = Vec::new();
    let mut display_idx = 1;

    for em in enum_monitors {
        let mut friendly_name = String::new();
        let mut conn_type = None;

        for (path_key, meta) in &metas {
            if em.device_name.contains(path_key) || path_key.contains(&em.device_name) {
                if !meta.friendly_name.is_empty() {
                    friendly_name = meta.friendly_name.clone();
                }
                conn_type = Some(meta.connection_type.clone());
                break;
            }
        }

        for (phys_idx, (h_raw, desc)) in em.physical_handles.iter().enumerate() {
            let guard = PhysicalMonitorGuard {
                handle: HANDLE(*h_raw as *mut _),
            };

            let display_name = if !friendly_name.is_empty() {
                friendly_name.clone()
            } else if !desc.is_empty() && desc != "Generic PnP Monitor" {
                desc.clone()
            } else {
                format!("Монитор {}", display_idx)
            };

            let id = compute_display_id(&em.device_name, desc, phys_idx);

            // Probe DDC High
            let (backend, probe_state, cur_pct, min_val, max_val, error) =
                if let Ok((min, cur, max)) = read_ddc_high(guard.handle) {
                    let pct = raw_to_percent(cur, min, max);
                    (
                        Some(BrightnessBackend::DdcHigh),
                        ProbeState::ReadOk,
                        Some(pct),
                        Some(min),
                        Some(max),
                        None,
                    )
                } else if let Ok((min, cur, max)) = read_ddc_vcp(guard.handle) {
                    let pct = raw_to_percent(cur, min, max);
                    (
                        Some(BrightnessBackend::DdcVcp),
                        ProbeState::ReadOk,
                        Some(pct),
                        Some(min),
                        Some(max),
                        None,
                    )
                } else {
                    (
                        None,
                        ProbeState::NoResponse,
                        None,
                        None,
                        None,
                        Some(BrightnessError {
                            code: "ddc_not_supported".to_string(),
                            native_code: None,
                            operation: "DDC/CI Probe".to_string(),
                            retryable: true,
                        }),
                    )
                };

            displays.push(BrightnessDisplay {
                id,
                generation,
                name: display_name,
                connection: conn_type.clone().or(Some("HDMI / DP".to_string())),
                is_primary: em.is_primary,
                backend,
                probe_state,
                verification: VerificationState::NotTested,
                current_percent: cur_pct,
                target_percent: cur_pct,
                raw_min: min_val,
                raw_max: max_val,
                available_levels: None,
                busy: false,
                error,
                device_name: Some(em.device_name.clone()),
            });

            display_idx += 1;
        }
    }

    for (i, wmi) in wmi_monitors.iter().enumerate() {
        let id = format!(
            "wmi_{}",
            &blake3::hash(wmi.instance_name.as_bytes()).to_hex()[..12]
        );
        if !displays.iter().any(|d| d.id == id) {
            let is_primary = displays.is_empty() && i == 0;
            displays.push(BrightnessDisplay {
                id,
                generation,
                name: format!("Встроенный экран {}", i + 1),
                connection: Some("Internal".to_string()),
                is_primary,
                backend: Some(BrightnessBackend::Wmi),
                probe_state: ProbeState::ReadOk,
                verification: VerificationState::NotTested,
                current_percent: Some(wmi.current),
                target_percent: Some(wmi.current),
                raw_min: Some(0),
                raw_max: Some(100),
                available_levels: if wmi.levels.is_empty() {
                    None
                } else {
                    Some(wmi.levels.clone())
                },
                busy: false,
                error: None,
                device_name: None,
            });
        }
    }

    displays
}

pub fn get_device_name_under_cursor() -> Option<String> {
    unsafe {
        let mut pt = windows::Win32::Foundation::POINT::default();
        if windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt).is_ok() {
            let hmon = windows::Win32::Graphics::Gdi::MonitorFromPoint(
                pt,
                windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
            );
            if !hmon.0.is_null() {
                let mut info: MONITORINFOEXW = std::mem::zeroed();
                info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
                if GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut _ as *mut _).as_bool() {
                    let len = info
                        .szDevice
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(info.szDevice.len());
                    return Some(String::from_utf16_lossy(&info.szDevice[..len]));
                }
            }
        }
    }
    None
}
