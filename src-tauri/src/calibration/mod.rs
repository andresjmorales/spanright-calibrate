pub mod flow;
pub mod overlay;

use crate::monitors::Monitor;
use serde::{Deserialize, Serialize};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationResult {
    pub monitor_id: usize,
    pub scale: f64,
    pub relative_x: f64,
    pub relative_y: f64,
    pub gap: i32,
    pub bound_to: usize,
    pub bind_horizontal: bool,
}

pub fn run_calibration(monitors: &[Monitor]) -> Result<Vec<CalibrationResult>, String> {
    if monitors.len() < 2 {
        return Err("Need at least 2 monitors for calibration".to_string());
    }

    let pairs = flow::compute_calibration_order(monitors);
    let mut results = Vec::new();
    let mut scales: Vec<f64> = vec![1.0; monitors.len()];

    let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };

    let monitor_rects: Vec<overlay::MonitorRect> = monitors
        .iter()
        .map(|m| overlay::MonitorRect {
            x: m.position_x - vx,
            y: m.position_y - vy,
            w: m.resolution_x as i32,
            h: m.resolution_y as i32,
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
            bind_horizontal,
            temp_middles: None,
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

        let (scale, relative_offset) = if bind_horizontal {
            let off = [
                scale_result.segments[0] - m1r.y,
                scale_result.segments[1] - m2r.y,
                scale_result.segments[2] - m1r.y,
                scale_result.segments[3] - m2r.y,
            ];

            let span_m1 = (off[2] - off[0]).abs() as f64;
            let span_m2 = (off[3] - off[1]).abs() as f64;

            let scale = if span_m2 > 1.0 {
                scales[*bound_idx] * (span_m1 / span_m2)
            } else {
                scales[*bound_idx]
            };

            let rel_y = off[1] as f64 - off[0] as f64 * scale;
            (scale, rel_y)
        } else {
            let off = [
                scale_result.segments[0] - m1r.x,
                scale_result.segments[1] - m2r.x,
                scale_result.segments[2] - m1r.x,
                scale_result.segments[3] - m2r.x,
            ];

            let span_m1 = (off[2] - off[0]).abs() as f64;
            let span_m2 = (off[3] - off[1]).abs() as f64;

            let scale = if span_m2 > 1.0 {
                scales[*bound_idx] * (span_m1 / span_m2)
            } else {
                scales[*bound_idx]
            };

            let rel_x = off[1] as f64 - off[0] as f64 * scale;
            (scale, rel_x)
        };

        scales[*unbound_idx] = scale;

        // --- Gap step ---
        let gap_result = overlay::run_overlay(overlay::OverlayConfig {
            step: overlay::OverlayStep::Gap,
            m1_idx: *unbound_idx,
            m2_idx: *bound_idx,
            monitors: monitor_rects.clone(),
            bind_horizontal,
            temp_middles: Some([temp_mid_m1, temp_mid_m2]),
        })?;

        if gap_result.cancelled {
            return Err("Calibration cancelled".to_string());
        }

        let gap = gap_result.gap;

        let (relative_x, relative_y) = if bind_horizontal {
            let m1 = &monitors[*unbound_idx];
            let m2 = &monitors[*bound_idx];
            let m1_w = m1.resolution_x as f64;
            let m2_w = m2.resolution_x as f64;

            let rx = if m1.position_x < m2.position_x {
                -(gap as f64 * 2.0) - m1_w * scale
            } else {
                m2_w * scales[*bound_idx] + gap as f64 * 2.0
            };

            (rx, relative_offset)
        } else {
            let m1 = &monitors[*unbound_idx];
            let m2 = &monitors[*bound_idx];
            let m1_h = m1.resolution_y as f64;
            let m2_h = m2.resolution_y as f64;

            let ry = if m1.position_y < m2.position_y {
                -(gap as f64 * 2.0) - m1_h * scale
            } else {
                m2_h * scales[*bound_idx] + gap as f64 * 2.0
            };

            (relative_offset, ry)
        };

        results.push(CalibrationResult {
            monitor_id: *unbound_idx,
            scale,
            relative_x,
            relative_y,
            gap,
            bound_to: *bound_idx,
            bind_horizontal,
        });
    }

    Ok(results)
}
