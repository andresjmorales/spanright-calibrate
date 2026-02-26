#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .build(tauri::tauri_build_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {});
}
