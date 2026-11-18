#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calibration;
mod export;
#[cfg(target_os = "linux")]
mod gtk_main;
mod monitors;

use std::collections::HashMap;
use std::sync::Mutex;

struct DiagonalOverrides(Mutex<HashMap<usize, f64>>);
struct OverlayColors(Mutex<[[u8; 3]; 2]>);

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
    // EDID sizes can be overridden too (the UI asks for an explicit "override"),
    // since some TVs and projectors report bogus physical sizes.
    if !monitors::discover_all()?.iter().any(|m| m.id == id) {
        return Err("Monitor not found".into());
    }
    overrides.0.lock().unwrap().insert(id, diagonal);
    Ok(())
}

#[tauri::command]
fn clear_monitor_diagonal(
    id: usize,
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<(), String> {
    overrides.0.lock().unwrap().remove(&id);
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

#[cfg(windows)]
#[tauri::command]
fn start_calibration(
    overrides: tauri::State<'_, DiagonalOverrides>,
    colors: tauri::State<'_, OverlayColors>,
) -> Result<Vec<calibration::CalibrationResult>, String> {
    let monitors = get_monitors(&overrides)?;
    let c = colors.0.lock().unwrap();
    calibration::run_calibration(&monitors, c[0], c[1])
}

/// On Linux the overlay windows are driven by the GTK main loop, so calibration
/// must wait for them on a worker thread rather than block the main thread.
#[cfg(not(windows))]
#[tauri::command]
async fn start_calibration(
    overrides: tauri::State<'_, DiagonalOverrides>,
    colors: tauri::State<'_, OverlayColors>,
) -> Result<Vec<calibration::CalibrationResult>, String> {
    let monitors = get_monitors(&overrides)?;
    let c = *colors.0.lock().unwrap();
    tauri::async_runtime::spawn_blocking(move || calibration::run_calibration(&monitors, c[0], c[1]))
        .await
        .map_err(|e| format!("Calibration failed: {e}"))?
}

/// The calibration step an overlay window should draw (Linux overlay only).
#[tauri::command]
fn overlay_view() -> Option<calibration::overlay::OverlayView> {
    calibration::overlay::current_view()
}

/// An overlay window confirmed (Enter) or cancelled (Esc) the step `token`.
#[tauri::command]
fn overlay_finish(token: u64, cancelled: bool, segments: [i32; 4], gap: i32) {
    calibration::overlay::finish(token, cancelled, segments, gap);
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

/// Move monitors in the virtual desktop. `positions` must cover every
/// connected monitor. Positions are normalized so the primary sits at (0, 0) and
/// validated before anything is applied. Returns the monitors as Windows reports
/// them afterwards (Windows may still nudge a layout it doesn't like).
#[tauri::command]
fn apply_monitor_positions(
    positions: Vec<monitors::layout::MonitorPosition>,
    overrides: tauri::State<'_, DiagonalOverrides>,
) -> Result<Vec<monitors::Monitor>, String> {
    use monitors::layout::{apply_positions, validate_layout, Rect};

    let current = get_monitors(&overrides)?;
    if positions.len() != current.len()
        || current.iter().any(|m| positions.iter().filter(|p| p.id == m.id).count() != 1)
    {
        return Err("Monitors changed since the layout was loaded. Refresh and try again.".into());
    }
    let primary = current
        .iter()
        .position(|m| m.is_primary)
        .ok_or("No primary monitor found")?;
    let pos_of = |id: usize| positions.iter().find(|p| p.id == id).unwrap();
    let (ox, oy) = {
        let p = pos_of(current[primary].id);
        (p.x, p.y)
    };

    let rects: Vec<Rect> = current
        .iter()
        .map(|m| {
            let p = pos_of(m.id);
            Rect {
                x: p.x - ox,
                y: p.y - oy,
                w: m.resolution_x as i32,
                h: m.resolution_y as i32,
            }
        })
        .collect();
    validate_layout(&rects, primary)?;
    apply_positions(&current, &rects)?;

    #[allow(unused_mut)]
    let mut updated = get_monitors(&overrides)?;
    // GDK only sees the new X11 layout once the main loop has handled the change
    // events, after this command returns, so report the layout xrandr was given.
    #[cfg(target_os = "linux")]
    for (m, r) in updated.iter_mut().zip(&rects) {
        m.position_x = r.x;
        m.position_y = r.y;
    }
    Ok(updated)
}

#[tauri::command]
fn get_monitor_info(id: usize) -> Result<Option<monitors::MonitorDetailInfo>, String> {
    monitors::monitor_details(id)
}

/// Open an http(s) URL in the default browser. Uses ShellExecuteW directly rather than
/// `cmd /C start`, which would treat characters like `&`, `|` or `%` in the URL as shell syntax.
#[cfg(windows)]
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

/// Open an http(s) URL in the default browser with `xdg-open`. The URL is passed
/// as a single argument (no shell), so `&` and friends are not interpreted.
#[cfg(not(windows))]
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("Only http(s) URLs can be opened".into());
    }
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&url);
    // Inside an AppImage, the bundled libraries must not leak into the browser.
    if std::env::var_os("APPIMAGE").is_some() {
        cmd.env_remove("LD_LIBRARY_PATH").env_remove("GDK_PIXBUF_MODULE_FILE");
    }
    cmd.spawn()
        .map_err(|e| format!("Failed to open URL (xdg-open: {e})"))?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            #[cfg(target_os = "linux")]
            gtk_main::set_app(_app.handle().clone());
            Ok(())
        })
        .manage(DiagonalOverrides(Mutex::new(HashMap::new())))
        .manage(OverlayColors(Mutex::new([[0, 229, 255], [255, 109, 0]])))
        .invoke_handler(tauri::generate_handler![
            discover_monitors,
            set_monitor_diagonal,
            clear_monitor_diagonal,
            get_monitor_info,
            set_overlay_colors,
            get_overlay_colors,
            start_calibration,
            export_calibration_json,
            save_calibration_file,
            apply_monitor_positions,
            open_url,
            overlay_view,
            overlay_finish
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
