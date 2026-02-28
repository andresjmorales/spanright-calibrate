pub mod models;
mod discovery;
mod edid;

pub use models::Monitor;

pub fn discover_all() -> Result<Vec<Monitor>, String> {
    let mut monitors = discovery::enumerate_monitors()?;

    if let Err(e) = discovery::populate_friendly_names(&mut monitors) {
        eprintln!("Warning: could not get friendly names: {e}");
    }

    match edid::read_all_edid() {
        Ok(edid_map) => {
            edid::apply_edid_to_monitors(&mut monitors, &edid_map);
        }
        Err(e) => {
            eprintln!("Warning: could not read EDID data: {e}");
        }
    }

    for m in &mut monitors {
        if m.physical_width_mm.is_none() {
            if let Some(diag) = guess_diagonal_from_names(m) {
                set_physical_from_diagonal(m, diag);
                m.size_source = "guessed".into();
            }
        }
        m.compute_derived();
    }

    Ok(monitors)
}

/// Try to extract a plausible diagonal (inches) from monitor/adapter names.
/// Looks for numbers 10-65 in the friendly name, monitor name, and adapter name.
fn guess_diagonal_from_names(m: &Monitor) -> Option<f64> {
    let candidates = [&m.friendly_name, &m.monitor_name, &m.adapter_name];
    for name in &candidates {
        if let Some(diag) = extract_diagonal_from_string(name) {
            return Some(diag);
        }
    }
    None
}

fn extract_diagonal_from_string(s: &str) -> Option<f64> {
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if let Ok(n) = s[start..i].parse::<u32>() {
                if (10..=65).contains(&n) {
                    let after = if i < bytes.len() { bytes[i] } else { 0 };
                    let before = if start > 0 { bytes[start - 1] } else { 0 };
                    // Skip if it looks like a resolution (e.g., "1920", "1080", "2560")
                    if n >= 100 {
                        i += 1;
                        continue;
                    }
                    // Skip if surrounded by digits on both sides (part of bigger number)
                    if before.is_ascii_digit() || after.is_ascii_digit() {
                        i += 1;
                        continue;
                    }
                    return Some(n as f64);
                }
            }
        }
        i += 1;
    }
    None
}

/// Set physical dimensions from a diagonal size, using the monitor's
/// pixel aspect ratio (falls back to 16:9).
pub fn set_physical_from_diagonal(m: &mut Monitor, diagonal_in: f64) {
    let aspect = if m.resolution_x > 0 && m.resolution_y > 0 {
        m.resolution_x as f64 / m.resolution_y as f64
    } else {
        16.0 / 9.0
    };
    let h_in = diagonal_in / (1.0 + aspect * aspect).sqrt();
    let w_in = h_in * aspect;
    m.physical_width_mm = Some((w_in * 25.4).round() as u32);
    m.physical_height_mm = Some((h_in * 25.4).round() as u32);
}
