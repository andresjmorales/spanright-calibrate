use crate::calibration::CalibrationResult;
use crate::monitors::Monitor;
use serde::Serialize;

/// Matches Spanright's MonitorPreset
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanrightPreset {
    pub name: String,
    pub diagonal: f64,
    pub aspect_ratio: [u32; 2],
    pub resolution_x: u32,
    pub resolution_y: u32,
}

/// Matches Spanright's per-monitor entry in SavedConfig.monitors[]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanrightMonitor {
    pub preset: SpanrightPreset,
    pub physical_x: f64,
    pub physical_y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// Matches Spanright's SavedConfig
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanrightSavedConfig {
    pub id: String,
    pub name: String,
    pub saved_at: u64,
    pub monitors: Vec<SpanrightMonitor>,
}

#[derive(Clone)]
struct PhysicalPlacement {
    monitor_idx: usize,
    x: f64, // inches, relative layout coords (before centering)
    y: f64,
    w: f64,
    h: f64,
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn aspect_ratio(rx: u32, ry: u32) -> [u32; 2] {
    let g = gcd(rx, ry);
    if g == 0 {
        [16, 9]
    } else {
        [rx / g, ry / g]
    }
}

pub fn build_spanright_config(
    monitors: &[Monitor],
    results: &[CalibrationResult],
) -> SpanrightSavedConfig {
    let placements = compute_physical_placements(monitors, results);

    // Center the layout on Spanright's canvas (144" × 96")
    const CANVAS_CX: f64 = 72.0;
    const CANVAS_CY: f64 = 48.0;

    let (layout_cx, layout_cy) = if placements.is_empty() {
        (0.0, 0.0)
    } else {
        let min_x = placements.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        let max_x = placements.iter().map(|p| p.x + p.w).fold(f64::MIN, f64::max);
        let min_y = placements.iter().map(|p| p.y).fold(f64::MAX, f64::min);
        let max_y = placements.iter().map(|p| p.y + p.h).fold(f64::MIN, f64::max);
        ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
    };

    let offset_x = CANVAS_CX - layout_cx;
    let offset_y = CANVAS_CY - layout_cy;

    let spanright_monitors: Vec<SpanrightMonitor> = placements
        .iter()
        .map(|p| {
            let m = &monitors[p.monitor_idx];
            let diagonal = m.diagonal_in.unwrap_or_else(|| {
                let w_in = p.w;
                let h_in = p.h;
                (w_in * w_in + h_in * h_in).sqrt()
            });
            let display_name = if !m.friendly_name.is_empty() {
                Some(m.friendly_name.clone())
            } else {
                None
            };

            SpanrightMonitor {
                preset: SpanrightPreset {
                    name: format!("{:.0}\" {}", diagonal, format_resolution(m.resolution_x, m.resolution_y)),
                    diagonal: round2(diagonal),
                    aspect_ratio: aspect_ratio(m.resolution_x, m.resolution_y),
                    resolution_x: m.resolution_x,
                    resolution_y: m.resolution_y,
                },
                physical_x: round4(p.x + offset_x),
                physical_y: round4(p.y + offset_y),
                rotation: if m.orientation == 1 { Some(90) } else { None },
                display_name,
            }
        })
        .collect();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    SpanrightSavedConfig {
        id: generate_uuid(),
        name: "Calibrated Layout".to_string(),
        saved_at: now,
        monitors: spanright_monitors,
    }
}

fn compute_physical_placements(
    monitors: &[Monitor],
    results: &[CalibrationResult],
) -> Vec<PhysicalPlacement> {
    // Derive PPI for all monitors through calibration chain
    let mut ppi_map: Vec<Option<f64>> = monitors.iter().map(|m| m.ppi).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for r in results {
            if ppi_map[r.monitor_id].is_none() && ppi_map[r.bound_to].is_some() {
                ppi_map[r.monitor_id] = Some(ppi_map[r.bound_to].unwrap() * r.scale);
                changed = true;
            }
            if ppi_map[r.bound_to].is_none() && ppi_map[r.monitor_id].is_some() {
                ppi_map[r.bound_to] = Some(ppi_map[r.monitor_id].unwrap() / r.scale);
                changed = true;
            }
        }
    }

    let mut placements: Vec<Option<PhysicalPlacement>> = vec![None; monitors.len()];

    // Place reference monitor at (0, 0)
    let calibrated_ids: std::collections::HashSet<usize> =
        results.iter().map(|r| r.monitor_id).collect();
    let ref_idx = monitors
        .iter()
        .position(|m| !calibrated_ids.contains(&m.id) && ppi_map[m.id].is_some())
        .unwrap_or(0);

    if let Some(ppi) = ppi_map[ref_idx] {
        let m = &monitors[ref_idx];
        placements[ref_idx] = Some(PhysicalPlacement {
            monitor_idx: ref_idx,
            x: 0.0,
            y: 0.0,
            w: m.resolution_x as f64 / ppi,
            h: m.resolution_y as f64 / ppi,
        });
    }

    // Place calibrated monitors
    for r in results {
        let bound_placement = match &placements[r.bound_to] {
            Some(p) => (p.x, p.y, p.w, p.h),
            None => continue,
        };

        let ppi_unbound = match ppi_map[r.monitor_id] {
            Some(p) => p,
            None => continue,
        };
        let ppi_bound = match ppi_map[r.bound_to] {
            Some(p) => p,
            None => continue,
        };

        let m = &monitors[r.monitor_id];
        let m_bound = &monitors[r.bound_to];
        let w = m.resolution_x as f64 / ppi_unbound;
        let h = m.resolution_y as f64 / ppi_unbound;

        let (x, y) = if r.bind_horizontal {
            let gap_in = (r.gap as f64).abs() / ppi_bound;
            let offset_in =
                r.align_offset_bound / ppi_bound - r.align_offset_unbound / ppi_unbound;

            let px = if m.position_x < m_bound.position_x {
                bound_placement.0 - w - gap_in
            } else {
                bound_placement.0 + bound_placement.2 + gap_in
            };
            (px, bound_placement.1 + offset_in)
        } else {
            let gap_in = (r.gap as f64).abs() / ppi_bound;
            let offset_in =
                r.align_offset_bound / ppi_bound - r.align_offset_unbound / ppi_unbound;

            let py = if m.position_y < m_bound.position_y {
                bound_placement.1 - h - gap_in
            } else {
                bound_placement.1 + bound_placement.3 + gap_in
            };
            (bound_placement.0 + offset_in, py)
        };

        placements[r.monitor_id] = Some(PhysicalPlacement {
            monitor_idx: r.monitor_id,
            x,
            y,
            w,
            h,
        });
    }

    placements.into_iter().flatten().collect()
}

pub fn export_json(
    monitors: &[Monitor],
    results: &[CalibrationResult],
) -> Result<String, String> {
    let config = build_spanright_config(monitors, results);
    // Wrap in array to match Spanright's import format (array of SavedConfigs)
    serde_json::to_string_pretty(&[config]).map_err(|e| format!("JSON serialization: {e}"))
}

fn format_resolution(rx: u32, ry: u32) -> &'static str {
    match (rx, ry) {
        (1920, 1080) => "FHD",
        (1920, 1200) => "WUXGA",
        (2560, 1080) => "UWFHD",
        (2560, 1440) => "QHD",
        (3440, 1440) => "UWQHD",
        (3840, 2160) => "4K",
        (3840, 1600) => "UW4K",
        (5120, 2160) => "5K UW",
        (5120, 1440) => "DQHD",
        _ => "",
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

fn generate_uuid() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.as_nanos();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (nanos & 0xFFFFFFFF) as u32,
        ((nanos >> 32) & 0xFFFF) as u16,
        ((nanos >> 48) & 0x0FFF) as u16,
        (0x8000 | ((nanos >> 60) & 0x3FFF)) as u16,
        (nanos.wrapping_mul(6364136223846793005) & 0xFFFFFFFFFFFF) as u64
    )
}
