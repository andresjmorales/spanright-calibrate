#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calibration;
mod export;
mod monitors;

use std::collections::HashMap;
use std::sync::Mutex;

struct DiagonalOverrides(Mutex<HashMap<usize, f64>>);

fn get_monitors(overrides: &DiagonalOverrides) -> Result<Vec<monitors::Monitor>, String> {
    let mut mons = monitors::discover_all()?;
    let map = overrides.0.lock().unwrap();
    for m in &mut mons {
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
    overrides.0.lock().unwrap().insert(id, diagonal);
    Ok(())
}

#[tauri::command]
fn start_calibration(
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<Vec<calibration::CalibrationResult>, String> {
    let monitors = get_monitors(&overrides)?;
    calibration::run_calibration(&monitors)
}

#[tauri::command]
fn export_calibration_json(
    results: Vec<calibration::CalibrationResult>,
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<String, String> {
    let monitors = get_monitors(&overrides)?;
    export::export_json(&monitors, &results)
}

#[tauri::command]
fn save_calibration_file(
    results: Vec<calibration::CalibrationResult>,
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<String, String> {
    let monitors = get_monitors(&overrides)?;
    let json = export::export_json(&monitors, &results)?;

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

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn()
        .map_err(|e| format!("Failed to open URL: {e}"))?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(DiagonalOverrides(Mutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![
            discover_monitors,
            set_monitor_diagonal,
            get_monitor_info,
            start_calibration,
            export_calibration_json,
            save_calibration_file,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
