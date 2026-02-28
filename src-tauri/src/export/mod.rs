use crate::calibration::CalibrationResult;
use crate::monitors::Monitor;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibratedMonitor {
    pub device_name: String,
    pub friendly_name: String,
    pub resolution: [u32; 2],
    pub physical_size_mm: Option<[u32; 2]>,
    pub physical_size_source: String,
    pub is_primary: bool,
    pub virtual_position: [i32; 2],
    pub scale: f64,
    pub relative_x: f64,
    pub relative_y: f64,
    pub gap: i32,
    pub bound_to: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationConfig {
    pub version: u32,
    pub calibrated_at: String,
    pub monitors: Vec<CalibratedMonitor>,
}

pub fn build_config(
    monitors: &[Monitor],
    results: &[CalibrationResult],
) -> CalibrationConfig {
    let primary_idx = monitors.iter().position(|m| m.is_primary).unwrap_or(0);

    let calibrated: Vec<CalibratedMonitor> = monitors
        .iter()
        .map(|m| {
            let cal = results.iter().find(|r| r.monitor_id == m.id);

            let (scale, rel_x, rel_y, gap, bound_to) = if let Some(c) = cal {
                (c.scale, c.relative_x, c.relative_y, c.gap, Some(c.bound_to))
            } else if m.id == primary_idx {
                (1.0, 0.0, 0.0, 0, None)
            } else {
                (1.0, 0.0, 0.0, 0, None)
            };

            let phys_source = if m.physical_width_mm.is_some() {
                "edid"
            } else {
                "none"
            };

            CalibratedMonitor {
                device_name: m.device_name.clone(),
                friendly_name: if m.friendly_name.is_empty() {
                    m.monitor_name.clone()
                } else {
                    m.friendly_name.clone()
                },
                resolution: [m.resolution_x, m.resolution_y],
                physical_size_mm: match (m.physical_width_mm, m.physical_height_mm) {
                    (Some(w), Some(h)) => Some([w, h]),
                    _ => None,
                },
                physical_size_source: phys_source.to_string(),
                is_primary: m.is_primary,
                virtual_position: [m.position_x, m.position_y],
                scale,
                relative_x: rel_x,
                relative_y: rel_y,
                gap,
                bound_to,
            }
        })
        .collect();

    CalibrationConfig {
        version: 1,
        calibrated_at: current_timestamp(),
        monitors: calibrated,
    }
}

pub fn export_json(
    monitors: &[Monitor],
    results: &[CalibrationResult],
) -> Result<String, String> {
    let config = build_config(monitors, results);
    serde_json::to_string_pretty(&config).map_err(|e| format!("JSON serialization: {e}"))
}

fn current_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let since_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = since_epoch.as_secs();
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hours = day_secs / 3600;
    let mins = (day_secs % 3600) / 60;
    let s = day_secs % 60;

    // Approximate date from days since epoch (good enough for timestamps)
    let (year, month, day) = days_to_date(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, mins, s
    )
}

fn days_to_date(days: u64) -> (u64, u64, u64) {
    // Simplified date calculation from days since 1970-01-01
    let mut y = 1970;
    let mut remaining = days;
    loop {
        let year_days = if is_leap(y) { 366 } else { 365 };
        if remaining < year_days {
            break;
        }
        remaining -= year_days;
        y += 1;
    }
    let month_days: [u64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0;
    for days_in_month in &month_days {
        if remaining < *days_in_month {
            break;
        }
        remaining -= *days_in_month;
        m += 1;
    }
    (y, m + 1, remaining + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
