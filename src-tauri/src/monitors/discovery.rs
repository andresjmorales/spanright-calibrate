use super::models::Monitor;
use std::mem;
use windows::core::PCWSTR;
use windows::Win32::Devices::Display::*;
use windows::Win32::Graphics::Gdi::*;

fn wchar_to_string(wchars: &[u16]) -> String {
    let len = wchars.iter().position(|&c| c == 0).unwrap_or(wchars.len());
    String::from_utf16_lossy(&wchars[..len])
}

pub fn enumerate_monitors() -> Result<Vec<Monitor>, String> {
    let mut monitors = Vec::new();
    let mut id = 0usize;
    let mut adapter_idx = 0u32;

    loop {
        let mut adapter = DISPLAY_DEVICEW {
            cb: mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };

        let ok = unsafe { EnumDisplayDevicesW(PCWSTR::null(), adapter_idx, &mut adapter, 0) };

        if !ok.as_bool() {
            break;
        }
        adapter_idx += 1;

        if adapter.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0 {
            continue;
        }
        if adapter.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER != 0 {
            continue;
        }

        let device_name = wchar_to_string(&adapter.DeviceName);
        let adapter_string = wchar_to_string(&adapter.DeviceString);
        let is_primary = adapter.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0;

        let mut devmode = DEVMODEW {
            dmSize: mem::size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };

        let settings_ok = unsafe {
            EnumDisplaySettingsW(
                PCWSTR(adapter.DeviceName.as_ptr()),
                ENUM_REGISTRY_SETTINGS,
                &mut devmode,
            )
        };

        if !settings_ok.as_bool() {
            continue;
        }

        // Get monitor device info (sub-device of the adapter)
        let mut monitor_dev = DISPLAY_DEVICEW {
            cb: mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        let has_monitor = unsafe {
            EnumDisplayDevicesW(PCWSTR(adapter.DeviceName.as_ptr()), 0, &mut monitor_dev, 1)
        };

        let (monitor_name, monitor_device_id) = if has_monitor.as_bool() {
            (
                wchar_to_string(&monitor_dev.DeviceString),
                wchar_to_string(&monitor_dev.DeviceID),
            )
        } else {
            (String::new(), String::new())
        };

        let (pos_x, pos_y) = unsafe {
            let pos = devmode.Anonymous1.Anonymous2.dmPosition;
            (pos.x, pos.y)
        };

        let orientation = unsafe { devmode.Anonymous1.Anonymous2.dmDisplayOrientation.0 };

        monitors.push(Monitor {
            id,
            device_name,
            friendly_name: String::new(),
            monitor_name,
            adapter_name: adapter_string,
            monitor_device_id,
            is_primary,
            resolution_x: devmode.dmPelsWidth,
            resolution_y: devmode.dmPelsHeight,
            position_x: pos_x,
            position_y: pos_y,
            orientation,
            physical_width_mm: None,
            physical_height_mm: None,
            physical_width_in: None,
            physical_height_in: None,
            diagonal_in: None,
            ppi: None,
            size_source: "none".into(),
        });
        id += 1;
    }

    Ok(monitors)
}

/// Get refresh rate and connection type for a given GDI device name.
pub fn get_display_extras(device_name: &str) -> (Option<u32>, Option<String>) {
    let mut refresh = None;

    // Refresh rate from DEVMODEW
    let dev_name_w: Vec<u16> = device_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut devmode = DEVMODEW {
        dmSize: mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    let ok = unsafe {
        EnumDisplaySettingsW(
            PCWSTR(dev_name_w.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut devmode,
        )
    };
    if ok.as_bool() && devmode.dmDisplayFrequency > 0 {
        refresh = Some(devmode.dmDisplayFrequency);
    }

    // Connection type from DisplayConfig
    let conn = get_connection_type(device_name);

    (refresh, conn)
}

fn get_connection_type(device_name: &str) -> Option<String> {
    let mut path_count = 0u32;
    let mut mode_count = 0u32;

    let err = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if err.0 != 0 {
        return None;
    }

    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];

    let err = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if err.0 != 0 {
        return None;
    }
    paths.truncate(path_count as usize);

    for path in &paths {
        let mut source_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            ..Default::default()
        };

        let source_ok =
            unsafe { DisplayConfigGetDeviceInfo(&mut source_name.header as *mut _) };
        if source_ok != 0 {
            continue;
        }

        let gdi = wchar_to_string(&source_name.viewGdiDeviceName);
        if gdi != device_name {
            continue;
        }

        let tech = path.targetInfo.outputTechnology;
        let name = match tech.0 {
            0 => "VGA",
            1 => "S-Video",
            2 => "Composite",
            3 => "Component",
            4 => "DVI",
            5 => "HDMI",
            6 => "LVDS",
            8 => "D-JPeg",
            9 => "SDI",
            10 => "DisplayPort (External)",
            11 => "DisplayPort (Embedded)",
            12 => "UDI (External)",
            13 => "UDI (Embedded)",
            14 => "SDTV Dongle",
            15 => "Miracast",
            16 => "Indirect Wired",
            -2147483648_i32 => "Internal",
            _ => "Unknown",
        };
        return Some(name.to_string());
    }
    None
}

pub fn populate_friendly_names(monitors: &mut [Monitor]) -> Result<(), String> {
    let mut path_count = 0u32;
    let mut mode_count = 0u32;

    let err = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if err.0 != 0 {
        return Err(format!("GetDisplayConfigBufferSizes: error {}", err.0));
    }

    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];

    let err = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if err.0 != 0 {
        return Err(format!("QueryDisplayConfig: error {}", err.0));
    }

    paths.truncate(path_count as usize);

    for path in &paths {
        // Get source GDI device name
        let mut source_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            ..Default::default()
        };

        let source_ok =
            unsafe { DisplayConfigGetDeviceInfo(&mut source_name.header as *mut _) };
        if source_ok != 0 {
            continue;
        }

        let gdi_name = wchar_to_string(&source_name.viewGdiDeviceName);

        // Get target friendly name
        let mut target_name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                size: mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            },
            ..Default::default()
        };

        let target_ok =
            unsafe { DisplayConfigGetDeviceInfo(&mut target_name.header as *mut _) };
        if target_ok != 0 {
            continue;
        }

        let friendly = wchar_to_string(&target_name.monitorFriendlyDeviceName);

        if let Some(monitor) = monitors.iter_mut().find(|m| m.device_name == gdi_name) {
            if !friendly.is_empty() {
                monitor.friendly_name = friendly;
            }
        }
    }

    Ok(())
}
