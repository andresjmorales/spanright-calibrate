#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calibration;
mod export;
mod monitors;

#[tauri::command]
fn discover_monitors() -> Result<Vec<monitors::Monitor>, String> {
    monitors::discover_all()
}

#[tauri::command]
fn start_calibration() -> Result<Vec<calibration::CalibrationResult>, String> {
    let monitors = monitors::discover_all()?;
    calibration::run_calibration(&monitors)
}

#[tauri::command]
fn export_calibration_json(
    results: Vec<calibration::CalibrationResult>,
) -> Result<String, String> {
    let monitors = monitors::discover_all()?;
    export::export_json(&monitors, &results)
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            discover_monitors,
            start_calibration,
            export_calibration_json
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
