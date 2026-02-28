use std::collections::HashMap;
use serde::Serialize;
use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;

#[derive(Debug, Clone)]
pub struct EdidPhysicalSize {
    pub width_mm: u32,
    pub height_mm: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdidInfo {
    pub manufacturer: String,
    pub product_code: u16,
    pub serial_number: u32,
    pub manufacture_week: u8,
    pub manufacture_year: u16,
    pub edid_version: String,
    pub width_mm: u32,
    pub height_mm: u32,
    pub gamma: Option<f64>,
    pub display_type: String,
    pub dpms_standby: bool,
    pub dpms_suspend: bool,
    pub dpms_off: bool,
    pub bit_depth: Option<u8>,
    pub monitor_name: Option<String>,
    pub monitor_serial: Option<String>,
    pub min_v_rate_hz: Option<u32>,
    pub max_v_rate_hz: Option<u32>,
    pub min_h_rate_khz: Option<u32>,
    pub max_h_rate_khz: Option<u32>,
    pub max_pixel_clock_mhz: Option<u32>,
    pub native_resolution: Option<[u32; 2]>,
}

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

        let instance_id = get_device_instance_id(dev_info, &dev_info_data);

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

/// Read detailed EDID info for all monitors. Returns map of hardware ID fragment → EdidInfo.
pub fn read_all_edid_info() -> Result<HashMap<String, EdidInfo>, String> {
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
        let ok = unsafe { SetupDiEnumDeviceInfo(dev_info, idx, &mut dev_info_data) };
        if ok.is_err() {
            break;
        }
        idx += 1;

        let instance_id = get_device_instance_id(dev_info, &dev_info_data);

        if let Some(edid_bytes) = read_edid_from_registry(dev_info, &mut dev_info_data) {
            if let Some(info) = parse_edid_full(&edid_bytes) {
                let key = extract_hardware_id_fragment(&instance_id);
                result.insert(key, info);
            }
        }
    }

    unsafe {
        let _ = SetupDiDestroyDeviceInfoList(dev_info);
    }

    Ok(result)
}

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

fn parse_edid_full(edid: &[u8]) -> Option<EdidInfo> {
    if edid.len() < 128 {
        return None;
    }
    if edid[0..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }

    let mfg_raw = ((edid[8] as u16) << 8) | edid[9] as u16;
    let manufacturer = decode_manufacturer_id(mfg_raw);

    let product_code = (edid[11] as u16) << 8 | edid[10] as u16;
    let serial_number = u32::from_le_bytes([edid[12], edid[13], edid[14], edid[15]]);
    let manufacture_week = edid[16];
    let manufacture_year = edid[17] as u16 + 1990;

    let edid_version = format!("{}.{}", edid[18], edid[19]);

    // Physical size (coarse, bytes 21-22, in cm)
    let mut width_mm = edid[21] as u32 * 10;
    let mut height_mm = edid[22] as u32 * 10;

    // Try precise from detailed timing (bytes 54-68)
    if edid.len() >= 69 {
        let h_mm = (((edid[68] as u32) >> 4) << 8) | edid[66] as u32;
        let v_mm = (((edid[68] as u32) & 0x0F) << 8) | edid[67] as u32;
        if h_mm > 0 && v_mm > 0 && h_mm < 2000 && v_mm < 2000 {
            width_mm = h_mm;
            height_mm = v_mm;
        }
    }

    let gamma = if edid[23] != 0xFF {
        Some((edid[23] as f64 + 100.0) / 100.0)
    } else {
        None
    };

    // Byte 20: video input definition
    let is_digital = edid[20] & 0x80 != 0;
    let bit_depth = if is_digital {
        match (edid[20] >> 4) & 0x07 {
            1 => Some(6),
            2 => Some(8),
            3 => Some(10),
            4 => Some(12),
            5 => Some(14),
            6 => Some(16),
            _ => None,
        }
    } else {
        None
    };

    // Feature support (byte 24)
    let dpms_standby = edid[24] & 0x80 != 0;
    let dpms_suspend = edid[24] & 0x40 != 0;
    let dpms_off = edid[24] & 0x20 != 0;

    let display_type = if is_digital {
        match (edid[24] >> 3) & 0x03 {
            0 => "RGB 4:4:4",
            1 => "RGB 4:4:4 + YCrCb 4:4:4",
            2 => "RGB 4:4:4 + YCrCb 4:2:2",
            3 => "RGB 4:4:4 + YCrCb 4:4:4 + YCrCb 4:2:2",
            _ => "Unknown",
        }
    } else {
        match (edid[24] >> 3) & 0x03 {
            0 => "Monochrome",
            1 => "RGB Color",
            2 => "Non-RGB Color",
            3 => "Undefined",
            _ => "Unknown",
        }
    }
    .to_string();

    // Parse descriptor blocks (4 blocks at bytes 54, 72, 90, 108)
    let mut monitor_name = None;
    let mut monitor_serial = None;
    let mut min_v_rate = None;
    let mut max_v_rate = None;
    let mut min_h_rate = None;
    let mut max_h_rate = None;
    let mut max_pixel_clock = None;

    // Native resolution from first detailed timing
    let native_resolution = parse_detailed_timing_resolution(edid, 54);

    for offset in [72, 90, 108] {
        if offset + 18 > edid.len() {
            break;
        }
        let block = &edid[offset..offset + 18];
        if block[0] == 0 && block[1] == 0 && block[2] == 0 {
            let tag = block[3];
            match tag {
                0xFC => monitor_name = Some(parse_descriptor_string(&block[5..18])),
                0xFF => monitor_serial = Some(parse_descriptor_string(&block[5..18])),
                0xFD => {
                    min_v_rate = Some(block[5] as u32);
                    max_v_rate = Some(block[6] as u32);
                    min_h_rate = Some(block[7] as u32);
                    max_h_rate = Some(block[8] as u32);
                    max_pixel_clock = Some(block[9] as u32 * 10);
                }
                _ => {}
            }
        }
    }

    Some(EdidInfo {
        manufacturer,
        product_code,
        serial_number,
        manufacture_week,
        manufacture_year,
        edid_version,
        width_mm,
        height_mm,
        gamma,
        display_type,
        dpms_standby,
        dpms_suspend,
        dpms_off,
        bit_depth,
        monitor_name,
        monitor_serial,
        min_v_rate_hz: min_v_rate,
        max_v_rate_hz: max_v_rate,
        min_h_rate_khz: min_h_rate,
        max_h_rate_khz: max_h_rate,
        max_pixel_clock_mhz: max_pixel_clock,
        native_resolution,
    })
}

fn decode_manufacturer_id(raw: u16) -> String {
    let c1 = ((raw >> 10) & 0x1F) as u8 + b'A' - 1;
    let c2 = ((raw >> 5) & 0x1F) as u8 + b'A' - 1;
    let c3 = (raw & 0x1F) as u8 + b'A' - 1;
    format!("{}{}{}", c1 as char, c2 as char, c3 as char)
}

fn parse_descriptor_string(data: &[u8]) -> String {
    let s: String = data
        .iter()
        .take_while(|&&b| b != 0x0A && b != 0x00)
        .map(|&b| b as char)
        .collect();
    s.trim().to_string()
}

fn parse_detailed_timing_resolution(edid: &[u8], offset: usize) -> Option<[u32; 2]> {
    if offset + 18 > edid.len() {
        return None;
    }
    let block = &edid[offset..];
    // Not a detailed timing if first two bytes are zero
    if block[0] == 0 && block[1] == 0 {
        return None;
    }
    let h_active = (block[4] as u32 >> 4) << 8 | block[2] as u32;
    let v_active = (block[7] as u32 >> 4) << 8 | block[5] as u32;
    if h_active > 0 && v_active > 0 {
        Some([h_active, v_active])
    } else {
        None
    }
}

fn parse_edid_physical_size(edid: &[u8]) -> Option<EdidPhysicalSize> {
    if edid.len() < 128 {
        return None;
    }
    if edid[0..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }

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

fn extract_hardware_id_fragment(instance_id: &str) -> String {
    let parts: Vec<&str> = instance_id.split('\\').collect();
    if parts.len() >= 2 {
        parts[1].to_string()
    } else {
        instance_id.to_string()
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
