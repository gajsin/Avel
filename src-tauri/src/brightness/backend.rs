use windows::Win32::Foundation::HANDLE;

use super::ddc::{
    read_ddc_high, read_ddc_vcp, write_ddc_high, write_ddc_vcp, PhysicalMonitorGuard,
};
use super::discovery::{compute_display_id, enumerate_all_monitors};
use super::model::{BrightnessBackend, BrightnessDisplay, BrightnessError};
use super::wmi::{find_nearest_level, read_wmi_monitors, write_wmi_brightness, WmiBrightnessInfo};

pub type BrightnessReadResult = Result<(u32, u32, u32, Option<Vec<u32>>), BrightnessError>;

pub trait BrightnessDeviceBackend: Send + Sync {
    fn read_brightness(&self, display: &BrightnessDisplay) -> BrightnessReadResult;

    fn write_brightness(
        &self,
        display: &BrightnessDisplay,
        raw_value: u32,
    ) -> Result<(), BrightnessError>;
}

pub struct NativeBrightnessBackend;

impl Default for NativeBrightnessBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn find_wmi_monitor<'a>(
    monitors: &'a [WmiBrightnessInfo],
    display_id: &str,
) -> Option<&'a WmiBrightnessInfo> {
    monitors.iter().find(|m| {
        let wmi_id = format!(
            "wmi_{}",
            &blake3::hash(m.instance_name.as_bytes()).to_hex()[..12]
        );
        wmi_id == display_id || m.instance_name == display_id
    })
}

impl NativeBrightnessBackend {
    pub fn new() -> Self {
        Self
    }

    fn with_physical_monitor<F, R>(
        &self,
        display: &BrightnessDisplay,
        f: F,
    ) -> Result<R, BrightnessError>
    where
        F: FnOnce(HANDLE) -> Result<R, BrightnessError>,
    {
        let all = enumerate_all_monitors();
        let mut target_res = None;

        let mut f_opt = Some(f);
        for em in &all {
            for (phys_idx, (h_raw, desc)) in em.physical_handles.iter().enumerate() {
                let guard = PhysicalMonitorGuard {
                    handle: HANDLE(*h_raw as *mut _),
                };
                let id1 = compute_display_id(&em.device_name, desc, phys_idx);

                if target_res.is_none() && id1 == display.id {
                    if let Some(f_action) = f_opt.take() {
                        target_res = Some(f_action(guard.handle));
                    }
                }
            }
        }

        target_res.unwrap_or_else(|| {
            Err(BrightnessError {
                code: "device_not_found".to_string(),
                native_code: None,
                operation: "with_physical_monitor".to_string(),
                retryable: false,
            })
        })
    }
}

impl BrightnessDeviceBackend for NativeBrightnessBackend {
    fn read_brightness(
        &self,
        display: &BrightnessDisplay,
    ) -> Result<(u32, u32, u32, Option<Vec<u32>>), BrightnessError> {
        match display.backend {
            Some(BrightnessBackend::DdcHigh) => self.with_physical_monitor(display, |h| {
                let (min, cur, max) = read_ddc_high(h)?;
                Ok((min, cur, max, None))
            }),
            Some(BrightnessBackend::DdcVcp) => self.with_physical_monitor(display, |h| {
                let (min, cur, max) = read_ddc_vcp(h)?;
                Ok((min, cur, max, None))
            }),
            Some(BrightnessBackend::Wmi) => {
                let monitors = read_wmi_monitors()?;
                let matched = find_wmi_monitor(&monitors, &display.id);

                if let Some(m) = matched {
                    let levels = if m.levels.is_empty() {
                        None
                    } else {
                        Some(m.levels.clone())
                    };
                    Ok((0, m.current, 100, levels))
                } else {
                    Err(BrightnessError {
                        code: "device_not_found".to_string(),
                        native_code: None,
                        operation: "read_wmi_monitors".to_string(),
                        retryable: false,
                    })
                }
            }
            None => Err(BrightnessError {
                code: "unsupported_backend".to_string(),
                native_code: None,
                operation: "read_brightness".to_string(),
                retryable: false,
            }),
        }
    }

    fn write_brightness(
        &self,
        display: &BrightnessDisplay,
        raw_value: u32,
    ) -> Result<(), BrightnessError> {
        match display.backend {
            Some(BrightnessBackend::DdcHigh) => {
                self.with_physical_monitor(display, |h| write_ddc_high(h, raw_value))
            }
            Some(BrightnessBackend::DdcVcp) => {
                self.with_physical_monitor(display, |h| write_ddc_vcp(h, raw_value))
            }
            Some(BrightnessBackend::Wmi) => {
                let monitors = read_wmi_monitors()?;
                let matched = find_wmi_monitor(&monitors, &display.id);

                if let Some(m) = matched {
                    let target_lvl = if m.levels.is_empty() {
                        raw_value.clamp(0, 100)
                    } else {
                        find_nearest_level(raw_value, &m.levels)
                    };
                    write_wmi_brightness(&m.instance_name, target_lvl)
                } else {
                    Err(BrightnessError {
                        code: "device_not_found".to_string(),
                        native_code: None,
                        operation: "write_wmi_brightness".to_string(),
                        retryable: false,
                    })
                }
            }
            None => Err(BrightnessError {
                code: "unsupported_backend".to_string(),
                native_code: None,
                operation: "write_brightness".to_string(),
                retryable: false,
            }),
        }
    }
}
