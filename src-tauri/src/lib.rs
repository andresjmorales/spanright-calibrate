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

pub fn run() {
    tauri::Builder::default()
        .manage(DiagonalOverrides(Mutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![
            discover_monitors,
            set_monitor_diagonal,
            start_calibration,
            export_calibration_json
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
