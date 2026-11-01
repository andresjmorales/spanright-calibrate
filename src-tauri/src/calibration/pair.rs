//! Pure per-pair calibration math (no Win32), so it can be unit tested.

use crate::monitors::Monitor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationResult {
    pub monitor_id: usize,
    /// PPI ratio of this monitor relative to the monitor it is bound to
    /// (ppi_unbound / ppi_bound). Consumers derive `ppi_unbound = ppi_bound * scale`,
    /// so this must be pairwise, not cumulative from the primary monitor.
    pub scale: f64,
    pub relative_x: f64,
    pub relative_y: f64,
    /// Gap between the two monitors' active areas, in pixels of whichever monitor
    /// is on the right (horizontal binding) or bottom (vertical binding): the gap
    /// overlay translates that monitor's diagonal lines by this many of its pixels.
    pub gap: i32,
    pub bound_to: usize,
    pub bind_horizontal: bool,
    /// Pixel offset from unbound monitor's top (or left) edge to alignment midpoint
    pub align_offset_unbound: f64,
    /// Pixel offset from bound monitor's top (or left) edge to alignment midpoint
    pub align_offset_bound: f64,
}

/// Overlay-window origin (top-left) of a monitor, in the same coordinates as `segments`.
#[derive(Clone, Copy)]
pub struct Origin {
    pub x: i32,
    pub y: i32,
}

/// Turn the scale-step line positions and the gap-step value for one pair into a
/// CalibrationResult. `segments` are overlay coordinates
/// `[color1_unbound, color1_bound, color2_unbound, color2_bound]` — y for a
/// horizontal (side-by-side) pair, x for a vertical (stacked) pair.
/// `bound_scale` is the bound monitor's PPI relative to the primary; the second
/// return value is the unbound monitor's (used only for the legacy relative_x/y).
#[allow(clippy::too_many_arguments)]
pub fn pair_result(
    monitors: &[Monitor],
    unbound_idx: usize,
    bound_idx: usize,
    unbound_origin: Origin,
    bound_origin: Origin,
    bind_horizontal: bool,
    segments: [i32; 4],
    gap: i32,
    bound_scale: f64,
) -> (CalibrationResult, f64) {
    // Offsets from each monitor's top (horizontal pair) or left (vertical pair) edge.
    let (o1, o2) = if bind_horizontal {
        (unbound_origin.y, bound_origin.y)
    } else {
        (unbound_origin.x, bound_origin.x)
    };
    let off = [
        segments[0] - o1,
        segments[1] - o2,
        segments[2] - o1,
        segments[3] - o2,
    ];

    // Pairwise PPI ratio (unbound / bound): the same physical span covers
    // span_m1 pixels on the unbound monitor and span_m2 on the bound one.
    let span_m1 = (off[2] - off[0]).abs() as f64;
    let span_m2 = (off[3] - off[1]).abs() as f64;
    let ratio = if span_m2 > 1.0 { span_m1 / span_m2 } else { 1.0 };
    let scale = bound_scale * ratio;

    let align_off_m1 = (off[0] + off[2]) as f64 / 2.0;
    let align_off_m2 = (off[1] + off[3]) as f64 / 2.0;
    let relative_offset = off[1] as f64 - off[0] as f64 * scale;

    let m1 = &monitors[unbound_idx];
    let m2 = &monitors[bound_idx];
    let (relative_x, relative_y) = if bind_horizontal {
        let rx = if m1.position_x < m2.position_x {
            -(gap as f64 * 2.0) - m1.resolution_x as f64 * scale
        } else {
            m2.resolution_x as f64 * bound_scale + gap as f64 * 2.0
        };
        (rx, relative_offset)
    } else {
        let ry = if m1.position_y < m2.position_y {
            -(gap as f64 * 2.0) - m1.resolution_y as f64 * scale
        } else {
            m2.resolution_y as f64 * bound_scale + gap as f64 * 2.0
        };
        (relative_offset, ry)
    };

    (
        CalibrationResult {
            monitor_id: unbound_idx,
            scale: ratio,
            relative_x,
            relative_y,
            gap,
            bound_to: bound_idx,
            bind_horizontal,
            align_offset_unbound: align_off_m1,
            align_offset_bound: align_off_m2,
        },
        scale,
    )
}
