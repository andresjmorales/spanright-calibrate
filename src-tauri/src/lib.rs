#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calibration;
mod export;
mod monitors;

use std::collections::HashMap;
use std::sync::Mutex;

struct DiagonalOverrides(Mutex<HashMap<usize, f64>>);
struct OverlayColors(Mutex<[[u8; 3]; 2]>);

fn get_monitors(overrides: &DiagonalOverrides) -> Result<Vec<monitors::Monitor>, String> {
    let mut mons = monitors::discover_all()?;
    let map = overrides.0.lock().unwrap();
    for m in &mut mons {
        // EDID-reported sizes are authoritative; manual overrides only fill in
        // guessed or missing sizes.
        if m.size_source == "edid" {
            continue;
        }
        if let Some(&diag) = map.get(&m.id) {
            monitors::set_physical_from_diagonal(m, diag);
            m.size_source = "manual".into();
            m.compute_derived();
        }
    }
    Ok(mons)
}

#[tauri::command]
fn discover_monitors(
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<Vec<monitors::Monitor>, String> {
    get_monitors(&overrides)
}

#[tauri::command]
fn set_monitor_diagonal(
    id: usize,
    diagonal: f64,
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<(), String> {
    if diagonal <= 0.0 || diagonal > 200.0 {
        return Err("Diagonal must be between 0 and 200 inches".into());
    }
    let mons = monitors::discover_all()?;
    match mons.iter().find(|m| m.id == id) {
        None => return Err("Monitor not found".into()),
        Some(m) if m.size_source == "edid" => {
            return Err("This monitor reports its own size (EDID); it can't be overridden".into())
        }
        Some(_) => {}
    }
    overrides.0.lock().unwrap().insert(id, diagonal);
    Ok(())
}

#[tauri::command]
fn set_overlay_colors(
    color1: [u8; 3],
    color2: [u8; 3],
    colors: tauri::State<'_, OverlayColors>,
) -> Result<(), String> {
    let mut c = colors.0.lock().unwrap();
    *c = [color1, color2];
    Ok(())
}

#[tauri::command]
fn get_overlay_colors(
    colors: tauri::State<'_, OverlayColors>,
) -> Result<[[u8; 3]; 2], String> {
    let c = colors.0.lock().unwrap();
    Ok(*c)
}

#[tauri::command]
fn start_calibration(
    overrides: tauri::State<'_, DiagonalOverrides>,
    colors: tauri::State<'_, OverlayColors>,
) -> Result<Vec<calibration::CalibrationResult>, String> {
    let monitors = get_monitors(&overrides)?;
    let c = colors.0.lock().unwrap();
    calibration::run_calibration(&monitors, c[0], c[1])
}

#[tauri::command]
fn export_calibration_json(
    results: Vec<calibration::CalibrationResult>,
    include_virtual_layout: bool,
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<String, String> {
    let monitors = get_monitors(&overrides)?;
    export::export_json(&monitors, &results, include_virtual_layout)
}

#[tauri::command]
fn save_calibration_file(
    results: Vec<calibration::CalibrationResult>,
    include_virtual_layout: bool,
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<String, String> {
    let monitors = get_monitors(&overrides)?;
    let json = export::export_json(&monitors, &results, include_virtual_layout)?;

    let file = rfd::FileDialog::new()
        .set_title("Save Spanright Layout")
        .add_filter("JSON", &["json"])
        .set_file_name("spanright-calibration.json")
        .save_file();

    match file {
        Some(path) => {
            std::fs::write(&path, &json).map_err(|e| format!("Write failed: {e}"))?;
            Ok(path.display().to_string())
        }
        None => Ok("cancelled".to_string()),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorDetailInfo {
    edid: Option<monitors::edid::EdidInfo>,
    refresh_rate_hz: Option<u32>,
    connection_type: Option<String>,
}

#[tauri::command]
fn get_monitor_info(id: usize) -> Result<Option<MonitorDetailInfo>, String> {
    let mons = monitors::discover_all()?;
    let m = match mons.iter().find(|m| m.id == id) {
        Some(m) => m,
        None => return Ok(None),
    };

    let edid = {
        let info_map = monitors::edid::read_all_edid_info()?;
        info_map
            .iter()
            .find(|(key, _)| m.monitor_device_id.contains(key.as_str()))
            .map(|(_, info)| info.clone())
    };

    let (refresh_rate_hz, connection_type) =
        monitors::discovery::get_display_extras(&m.device_name);

    Ok(Some(MonitorDetailInfo {
        edid,
        refresh_rate_hz,
        connection_type,
    }))
}

/// Open an http(s) URL in the default browser. Uses ShellExecuteW directly rather than
/// `cmd /C start`, which would treat characters like `&`, `|` or `%` in the URL as shell syntax.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("Only http(s) URLs can be opened".into());
    }
    let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns a value > 32 on success.
    if result.0 as isize <= 32 {
        return Err(format!("Failed to open URL (ShellExecute error {})", result.0 as isize));
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(DiagonalOverrides(Mutex::new(HashMap::new())))
        .manage(OverlayColors(Mutex::new([[0, 229, 255], [255, 109, 0]])))
        .invoke_handler(tauri::generate_handler![
            discover_monitors,
            set_monitor_diagonal,
            get_monitor_info,
            set_overlay_colors,
            get_overlay_colors,
            start_calibration,
            export_calibration_json,
            save_calibration_file,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
