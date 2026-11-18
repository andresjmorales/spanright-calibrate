pub mod flow;
pub mod overlay;
pub mod pair;

pub use pair::CalibrationResult;

use crate::monitors::Monitor;

pub fn run_calibration(monitors: &[Monitor], color1: [u8; 3], color2: [u8; 3]) -> Result<Vec<CalibrationResult>, String> {
    if monitors.len() < 2 {
        return Err("Need at least 2 monitors for calibration".to_string());
    }
    let result = run_pairs(monitors, color1, color2);
    overlay::close_all();
    result
}

/// Top-left corner of the virtual desktop, which the overlay treats as (0, 0).
#[cfg(windows)]
fn virtual_origin(_monitors: &[Monitor]) -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::*;
    unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN)) }
}

#[cfg(not(windows))]
fn virtual_origin(monitors: &[Monitor]) -> (i32, i32) {
    (
        monitors.iter().map(|m| m.position_x).min().unwrap_or(0),
        monitors.iter().map(|m| m.position_y).min().unwrap_or(0),
    )
}

fn run_pairs(monitors: &[Monitor], color1: [u8; 3], color2: [u8; 3]) -> Result<Vec<CalibrationResult>, String> {

    let pairs = flow::compute_calibration_order(monitors);
    let mut results = Vec::new();
    let mut scales: Vec<f64> = vec![1.0; monitors.len()];

    let (vx, vy) = virtual_origin(monitors);

    let monitor_rects: Vec<overlay::MonitorRect> = monitors
        .iter()
        .map(|m| overlay::MonitorRect {
            x: m.position_x - vx,
            y: m.position_y - vy,
            w: m.resolution_x as i32,
            h: m.resolution_y as i32,
        })
        .collect();

    let monitor_labels: Vec<String> = monitors
        .iter()
        .map(|m| {
            let name = if m.friendly_name.is_empty() {
                &m.device_name
            } else {
                &m.friendly_name
            };
            format!("{}\n{}×{}", name, m.resolution_x, m.resolution_y)
        })
        .collect();

    for (unbound_idx, bound_idx) in &pairs {
        let bind_horizontal = flow::determine_bind_horizontal(
            &monitors[*unbound_idx],
            &monitors[*bound_idx],
        );

        // --- Scale step ---
        let scale_result = overlay::run_overlay(overlay::OverlayConfig {
            step: overlay::OverlayStep::Scale,
            m1_idx: *unbound_idx,
            m2_idx: *bound_idx,
            monitors: monitor_rects.clone(),
            monitor_labels: monitor_labels.clone(),
            bind_horizontal,
            temp_middles: None,
            color1,
            color2,
        })?;

        if scale_result.cancelled {
            return Err("Calibration cancelled".to_string());
        }

        let m1r = &monitor_rects[*unbound_idx];
        let m2r = &monitor_rects[*bound_idx];

        // Compute per-monitor midpoints from the scale step line positions.
        // segments: [blue_m1, blue_m2, red_m1, red_m2]
        let temp_mid_m1 = (scale_result.segments[0] + scale_result.segments[2]) / 2;
        let temp_mid_m2 = (scale_result.segments[1] + scale_result.segments[3]) / 2;

        // --- Gap step ---
        let gap_result = overlay::run_overlay(overlay::OverlayConfig {
            step: overlay::OverlayStep::Gap,
            m1_idx: *unbound_idx,
            m2_idx: *bound_idx,
            monitors: monitor_rects.clone(),
            monitor_labels: monitor_labels.clone(),
            bind_horizontal,
            temp_middles: Some([temp_mid_m1, temp_mid_m2]),
            color1,
            color2,
        })?;

        if gap_result.cancelled {
            return Err("Calibration cancelled".to_string());
        }

        let (result, scale) = pair::pair_result(
            monitors,
            *unbound_idx,
            *bound_idx,
            pair::Origin { x: m1r.x, y: m1r.y },
            pair::Origin { x: m2r.x, y: m2r.y },
            bind_horizontal,
            scale_result.segments,
            gap_result.gap,
            scales[*bound_idx],
        );
        scales[*unbound_idx] = scale;
        results.push(result);
    }

    Ok(results)
}
