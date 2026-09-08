use windows::core::{BSTR, PCWSTR, VARIANT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::System::Wmi::{
    IEnumWbemClassObject, IWbemClassObject, IWbemLocator, IWbemServices, WbemLocator,
    WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE,
};

use super::model::BrightnessError;

pub struct WmiBrightnessInfo {
    pub instance_name: String,
    pub current: u32,
    pub levels: Vec<u32>,
}

pub fn find_nearest_level(target: u32, levels: &[u32]) -> u32 {
    if levels.is_empty() {
        return target.clamp(0, 100);
    }
    let mut best = levels[0];
    let mut min_diff = (target as i32 - best as i32).abs();

    for &lvl in levels.iter().skip(1) {
        let diff = (target as i32 - lvl as i32).abs();
        if diff < min_diff {
            min_diff = diff;
            best = lvl;
        }
    }
    best
}

struct ComGuard(bool);
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

pub fn read_wmi_monitors() -> Result<Vec<WmiBrightnessInfo>, BrightnessError> {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let _guard = ComGuard(hr.is_ok());

        let res = (|| -> Result<Vec<WmiBrightnessInfo>, windows::core::Error> {
            let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
            let root = BSTR::from("ROOT\\WMI");
            let services: IWbemServices = locator.ConnectServer(
                &root,
                &BSTR::default(),
                &BSTR::default(),
                &BSTR::default(),
                0,
                &BSTR::default(),
                None,
            )?;

            let query_lang = BSTR::from("WQL");
            let query = BSTR::from("SELECT InstanceName, CurrentBrightness, Level FROM WmiMonitorBrightness WHERE Active = TRUE");
            let enum_objs: IEnumWbemClassObject = services.ExecQuery(
                &query_lang,
                &query,
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )?;

            let mut results = Vec::new();

            loop {
                let mut p_obj: Option<IWbemClassObject> = None;
                let mut returned = 0u32;
                let hr = enum_objs.Next(
                    WBEM_INFINITE,
                    std::slice::from_mut(&mut p_obj),
                    &mut returned,
                );

                if hr.is_err() || returned == 0 {
                    break;
                }

                if let Some(obj) = p_obj {
                    let mut var_name = VARIANT::default();
                    let mut var_cur = VARIANT::default();
                    let mut var_levels = VARIANT::default();

                    let name_prop = BSTR::from("InstanceName");
                    let cur_prop = BSTR::from("CurrentBrightness");
                    let level_prop = BSTR::from("Level");

                    let _ = obj.Get(PCWSTR(name_prop.as_ptr()), 0, &mut var_name, None, None);
                    let _ = obj.Get(PCWSTR(cur_prop.as_ptr()), 0, &mut var_cur, None, None);
                    let _ = obj.Get(PCWSTR(level_prop.as_ptr()), 0, &mut var_levels, None, None);

                    let instance_name = BSTR::try_from(&var_name)
                        .map(|b| b.to_string())
                        .unwrap_or_default();

                    let current = match u32::try_from(&var_cur) {
                        Ok(c) => c,
                        Err(_) => continue, // Do not substitute fake 50%
                    };
                    let mut levels = Vec::new();

                    let raw = var_levels.as_raw();
                    let l_vt = raw.Anonymous.Anonymous.vt;

                    if (l_vt & 0x2000) != 0 && !raw.Anonymous.Anonymous.Anonymous.parray.is_null() {
                        let psa = raw.Anonymous.Anonymous.Anonymous.parray
                            as *const windows::Win32::System::Com::SAFEARRAY;
                        let mut p_data = std::ptr::null_mut();
                        if (*psa).cDims == 1
                            && windows::Win32::System::Ole::SafeArrayAccessData(psa, &mut p_data)
                                .is_ok()
                        {
                            let rgsabound = (*psa).rgsabound[0];
                            let count = rgsabound.cElements as usize;
                            if count > 0
                                && count <= 256
                                && (l_vt & 0x0FFF) == 17
                                && !p_data.is_null()
                            {
                                let byte_slice =
                                    std::slice::from_raw_parts(p_data as *const u8, count);
                                levels = byte_slice.iter().map(|&b| b as u32).collect();
                            }
                            let _ = windows::Win32::System::Ole::SafeArrayUnaccessData(psa);
                        }
                    }

                    if !instance_name.is_empty() {
                        results.push(WmiBrightnessInfo {
                            instance_name,
                            current,
                            levels,
                        });
                    }
                }
            }

            Ok(results)
        })();

        res.map_err(|e| BrightnessError {
            code: "wmi_read_failed".to_string(),
            native_code: Some(e.code().0 as u32),
            operation: "WmiMonitorBrightness".to_string(),
            retryable: true,
        })
    }
}

pub fn write_wmi_brightness(instance_name: &str, brightness: u32) -> Result<(), BrightnessError> {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let _guard = ComGuard(hr.is_ok());

        let res = (|| -> Result<(), windows::core::Error> {
            let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
            let root = BSTR::from("ROOT\\WMI");
            let services: IWbemServices = locator.ConnectServer(
                &root,
                &BSTR::default(),
                &BSTR::default(),
                &BSTR::default(),
                0,
                &BSTR::default(),
                None,
            )?;

            let escaped_instance = instance_name.replace('\\', "\\\\").replace('"', "\\\"");
            let query_str = format!(
                "SELECT * FROM WmiMonitorBrightnessMethods WHERE InstanceName = \"{}\"",
                escaped_instance
            );
            let query_lang = BSTR::from("WQL");
            let query = BSTR::from(query_str.as_str());

            let enum_objs: IEnumWbemClassObject = services.ExecQuery(
                &query_lang,
                &query,
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )?;

            let mut p_obj: Option<IWbemClassObject> = None;
            let mut returned = 0u32;
            let hr = enum_objs.Next(
                WBEM_INFINITE,
                std::slice::from_mut(&mut p_obj),
                &mut returned,
            );

            if hr.is_err() || returned == 0 || p_obj.is_none() {
                return Err(windows::core::Error::new(
                    windows::core::HRESULT(-2147217406), // WBEM_E_NOT_FOUND (0x80041002)
                    "WMI monitor brightness instance not found",
                ));
            }

            let obj = p_obj.unwrap();
            let class_str = BSTR::from("WmiMonitorBrightnessMethods");
            let method_str = BSTR::from("WmiSetBrightness");
            let mut p_class: Option<IWbemClassObject> = None;
            services.GetObject(
                &class_str,
                WBEM_GENERIC_FLAG_TYPE(0),
                None,
                Some(&mut p_class),
                None,
            )?;

            if let Some(cls) = p_class {
                let mut p_in_params: Option<IWbemClassObject> = None;
                cls.GetMethod(&method_str, 0, &mut p_in_params, std::ptr::null_mut())?;

                if let Some(in_params_def) = p_in_params {
                    let in_inst = in_params_def.SpawnInstance(0)?;

                    let var_timeout = VARIANT::from(5u32);
                    let var_brightness = VARIANT::from(brightness.clamp(0, 100) as u8);

                    let timeout_prop = BSTR::from("Timeout");
                    let bright_prop = BSTR::from("Brightness");

                    in_inst.Put(PCWSTR(timeout_prop.as_ptr()), 0, &var_timeout, 0)?;
                    in_inst.Put(PCWSTR(bright_prop.as_ptr()), 0, &var_brightness, 0)?;

                    let mut path_var = VARIANT::default();
                    let path_prop = BSTR::from("__RELPATH");
                    obj.Get(PCWSTR(path_prop.as_ptr()), 0, &mut path_var, None, None)?;

                    if let Ok(rel_path_bstr) = BSTR::try_from(&path_var) {
                        let mut out_params: Option<IWbemClassObject> = None;
                        services.ExecMethod(
                            &rel_path_bstr,
                            &method_str,
                            WBEM_GENERIC_FLAG_TYPE(0),
                            None,
                            &in_inst,
                            Some(&mut out_params),
                            None,
                        )?;

                        if let Some(out) = out_params {
                            let mut ret_val = VARIANT::default();
                            let ret_prop = BSTR::from("ReturnValue");
                            let _ = out.Get(PCWSTR(ret_prop.as_ptr()), 0, &mut ret_val, None, None);
                            let ret_code = u32::try_from(&ret_val).unwrap_or(0);
                            if ret_code != 0 {
                                return Err(windows::core::Error::new(
                                    windows::core::HRESULT(-2147467259),
                                    format!("WmiSetBrightness failed with ReturnValue {ret_code}"),
                                ));
                            }
                        } else {
                            return Err(windows::core::Error::new(
                                windows::core::HRESULT(-2147467259),
                                "WmiSetBrightness returned no out_params",
                            ));
                        }
                    } else {
                        return Err(windows::core::Error::new(
                            windows::core::HRESULT(-2147467259), // E_FAIL
                            "Failed to get WMI RELPATH",
                        ));
                    }
                } else {
                    return Err(windows::core::Error::new(
                        windows::core::HRESULT(-2147467259),
                        "Failed to get WMI in-params",
                    ));
                }
            } else {
                return Err(windows::core::Error::new(
                    windows::core::HRESULT(-2147467259),
                    "Failed to get WMI class object",
                ));
            }

            Ok(())
        })();

        res.map_err(|e| BrightnessError {
            code: "wmi_write_failed".to_string(),
            native_code: Some(e.code().0 as u32),
            operation: "WmiSetBrightness".to_string(),
            retryable: true,
        })
    }
}
