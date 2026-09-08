use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitor, GetMonitorBrightness, GetVCPFeatureAndVCPFeatureReply,
    SetMonitorBrightness, SetVCPFeature, MC_VCP_CODE_TYPE,
};
use windows::Win32::Foundation::HANDLE;

use super::model::BrightnessError;

pub struct PhysicalMonitorGuard {
    pub handle: HANDLE,
}

impl Drop for PhysicalMonitorGuard {
    fn drop(&mut self) {
        if !self.handle.is_invalid() && !self.handle.0.is_null() {
            unsafe {
                let _ = DestroyPhysicalMonitor(self.handle);
            }
        }
    }
}

pub fn raw_to_percent(raw: u32, min: u32, max: u32) -> u32 {
    if max <= min {
        return 0;
    }
    let clamped_raw = raw.clamp(min, max);
    let ratio = (clamped_raw - min) as f64 / (max - min) as f64;
    (ratio * 100.0).round().clamp(0.0, 100.0) as u32
}

pub fn percent_to_raw(percent: u32, min: u32, max: u32) -> u32 {
    if max <= min {
        return min;
    }
    let p = percent.clamp(0, 100) as f64;
    let raw = min as f64 + (p / 100.0) * (max - min) as f64;
    raw.round().clamp(min as f64, max as f64) as u32
}

pub fn read_ddc_high(handle: HANDLE) -> Result<(u32, u32, u32), BrightnessError> {
    let mut min = 0u32;
    let mut cur = 0u32;
    let mut max = 0u32;

    let res = unsafe { GetMonitorBrightness(handle, &mut min, &mut cur, &mut max) };
    if res != 0 && max > min {
        Ok((min, cur, max))
    } else {
        Err(BrightnessError {
            code: "ddc_read_failed".to_string(),
            native_code: None,
            operation: "GetMonitorBrightness".to_string(),
            retryable: true,
        })
    }
}

pub fn write_ddc_high(handle: HANDLE, raw: u32) -> Result<(), BrightnessError> {
    let res = unsafe { SetMonitorBrightness(handle, raw) };
    if res != 0 {
        Ok(())
    } else {
        Err(BrightnessError {
            code: "ddc_write_failed".to_string(),
            native_code: None,
            operation: "SetMonitorBrightness".to_string(),
            retryable: true,
        })
    }
}

pub fn read_ddc_vcp(handle: HANDLE) -> Result<(u32, u32, u32), BrightnessError> {
    let mut vcp_type = MC_VCP_CODE_TYPE(0);
    let mut cur = 0u32;
    let mut max = 0u32;

    let res = unsafe {
        GetVCPFeatureAndVCPFeatureReply(handle, 0x10, Some(&mut vcp_type), &mut cur, Some(&mut max))
    };
    if res != 0 && max > 0 {
        Ok((0, cur, max))
    } else {
        Err(BrightnessError {
            code: "ddc_vcp_read_failed".to_string(),
            native_code: None,
            operation: "GetVCPFeature(0x10)".to_string(),
            retryable: true,
        })
    }
}

pub fn write_ddc_vcp(handle: HANDLE, raw: u32) -> Result<(), BrightnessError> {
    let res = unsafe { SetVCPFeature(handle, 0x10, raw) };
    if res != 0 {
        Ok(())
    } else {
        Err(BrightnessError {
            code: "ddc_vcp_write_failed".to_string(),
            native_code: None,
            operation: "SetVCPFeature(0x10)".to_string(),
            retryable: true,
        })
    }
}
