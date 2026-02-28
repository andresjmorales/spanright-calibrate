use std::collections::HashMap;
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;

#[derive(Debug, Clone)]
pub struct EdidPhysicalSize {
    pub width_mm: u32,
    pub height_mm: u32,
}

/// Read EDID data for all connected monitors via the Setup API.
/// Returns a map of (monitor hardware ID fragment → physical size).
pub fn read_all_edid() -> Result<HashMap<String, EdidPhysicalSize>, String> {
    let mut result = HashMap::new();

    let dev_info = unsafe {
        SetupDiGetClassDevsW(
            Some(&GUID_DEVCLASS_MONITOR),
            PCWSTR::null(),
            HWND::default(),
            DIGCF_PRESENT,
        )
        .map_err(|e| format!("SetupDiGetClassDevsW: {e}"))?
    };

    let mut idx = 0u32;
    loop {
        let mut dev_info_data = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };

        let ok =
            unsafe { SetupDiEnumDeviceInfo(dev_info, idx, &mut dev_info_data) };

        if ok.is_err() {
            break;
        }
        idx += 1;

        // Get device instance ID for matching
        let instance_id = get_device_instance_id(dev_info, &dev_info_data);

        // Read EDID from device registry
        if let Some(edid_bytes) = read_edid_from_registry(dev_info, &mut dev_info_data) {
            if let Some(size) = parse_edid_physical_size(&edid_bytes) {
                if size.width_mm > 0 && size.height_mm > 0 && size.width_mm < 2000 && size.height_mm < 2000 {
                    let key = extract_hardware_id_fragment(&instance_id);
                    result.insert(key, size);
                }
            }
        }
    }

    unsafe {
        let _ = SetupDiDestroyDeviceInfoList(dev_info);
    }

    Ok(result)
}

/// Apply EDID data to monitors by matching hardware ID fragments
/// against the monitor_device_id (e.g. "MONITOR\HPN3645\{guid}\0001").
pub fn apply_edid_to_monitors(
    monitors: &mut [super::models::Monitor],
    edid_map: &HashMap<String, EdidPhysicalSize>,
) {
    for monitor in monitors.iter_mut() {
        for (key, size) in edid_map {
            if monitor.monitor_device_id.contains(key) {
                monitor.physical_width_mm = Some(size.width_mm);
                monitor.physical_height_mm = Some(size.height_mm);
                monitor.size_source = "edid".into();
                break;
            }
        }
    }
}

fn get_device_instance_id(dev_info: HDEVINFO, data: &SP_DEVINFO_DATA) -> String {
    let mut buf = [0u16; 512];
    let mut required = 0u32;
    let ok = unsafe {
        SetupDiGetDeviceInstanceIdW(
            dev_info,
            data,
            Some(&mut buf),
            Some(&mut required as *mut _),
        )
    };
    if ok.is_ok() {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..len])
    } else {
        String::new()
    }
}

fn read_edid_from_registry(
    dev_info: HDEVINFO,
    data: &mut SP_DEVINFO_DATA,
) -> Option<Vec<u8>> {
    let hkey = unsafe {
        SetupDiOpenDevRegKey(
            dev_info,
            data,
            1, // DICS_FLAG_GLOBAL
            0,
            1, // DIREG_DEV
            KEY_READ.0,
        )
    };

    let hkey = match hkey {
        Ok(k) => k,
        Err(_) => return None,
    };

    let edid_name: Vec<u16> = "EDID\0".encode_utf16().collect();
    let mut data_type = REG_VALUE_TYPE::default();
    let mut size = 0u32;

    let status = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(edid_name.as_ptr()),
            None,
            Some(&mut data_type),
            None,
            Some(&mut size),
        )
    };

    if status != ERROR_SUCCESS || size == 0 {
        unsafe { let _ = RegCloseKey(hkey); }
        return None;
    }

    let mut buf = vec![0u8; size as usize];
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(edid_name.as_ptr()),
            None,
            Some(&mut data_type),
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        )
    };

    unsafe { let _ = RegCloseKey(hkey); }

    if status == ERROR_SUCCESS {
        buf.truncate(size as usize);
        Some(buf)
    } else {
        None
    }
}

fn parse_edid_physical_size(edid: &[u8]) -> Option<EdidPhysicalSize> {
    if edid.len() < 128 {
        return None;
    }

    // Validate EDID header
    if edid[0..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }

    // Try precise values from detailed timing descriptor (bytes 54-71)
    if edid.len() >= 69 {
        let h_mm = (((edid[68] as u32) >> 4) << 8) | edid[66] as u32;
        let v_mm = (((edid[68] as u32) & 0x0F) << 8) | edid[67] as u32;

        if h_mm > 0 && v_mm > 0 && h_mm < 2000 && v_mm < 2000 {
            return Some(EdidPhysicalSize {
                width_mm: h_mm,
                height_mm: v_mm,
            });
        }
    }

    // Fallback to coarse values (bytes 21-22, in cm)
    let w_cm = edid[21] as u32;
    let h_cm = edid[22] as u32;
    if w_cm > 0 && h_cm > 0 {
        return Some(EdidPhysicalSize {
            width_mm: w_cm * 10,
            height_mm: h_cm * 10,
        });
    }

    None
}

/// Extract a hardware ID fragment for matching.
/// Input like "DISPLAY\\DEL0001\\5&abc&0&UID256" → "DEL0001"
fn extract_hardware_id_fragment(instance_id: &str) -> String {
    let parts: Vec<&str> = instance_id.split('\\').collect();
    if parts.len() >= 2 {
        parts[1].to_string()
    } else {
        instance_id.to_string()
    }
}
