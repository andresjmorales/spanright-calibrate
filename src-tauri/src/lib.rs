#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod monitors;

#[tauri::command]
fn discover_monitors() -> Result<Vec<monitors::Monitor>, String> {
    monitors::discover_all()
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![discover_monitors])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
