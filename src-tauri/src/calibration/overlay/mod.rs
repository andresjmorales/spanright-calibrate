//! The full-screen alignment overlay shown during each calibration step.
//!
//! The types and the starting line positions are shared; drawing and input are
//! platform specific: a GDI window spanning the virtual desktop on Windows
//! (`win.rs`), and one full-screen webview per monitor on Linux (`linux.rs`).

use serde::Serialize;

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::run_overlay;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{close_all, current_view, finish, run_overlay};

#[derive(Clone, Serialize)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum OverlayStep {
    Scale,
    Gap,
}

pub struct OverlayConfig {
    pub step: OverlayStep,
    pub m1_idx: usize,
    pub m2_idx: usize,
    pub monitors: Vec<MonitorRect>,
    pub monitor_labels: Vec<String>,
    pub bind_horizontal: bool,
    /// Per-monitor midpoints from the scale step (y-coords for horizontal, x for vertical).
    /// [0] = midpoint on m1, [1] = midpoint on m2.
    pub temp_middles: Option<[i32; 2]>,
    pub color1: [u8; 3],
    pub color2: [u8; 3],
}

pub struct OverlayResult {
    pub cancelled: bool,
    pub segments: [i32; 4],
    pub gap: i32,
}

/// One overlay step as the Linux overlay windows draw it. Coordinates are in the
/// same virtual-desktop pixels as `OverlayConfig::monitors`.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayView {
    /// Changes every step, so a late answer for an old step is ignored.
    pub token: u64,
    pub step: &'static str,
    pub m1_idx: usize,
    pub m2_idx: usize,
    pub monitors: Vec<MonitorRect>,
    pub labels: Vec<String>,
    pub bind_horizontal: bool,
    pub segments: [i32; 4],
    pub mid_m1: i32,
    pub mid_m2: i32,
    pub color1: [u8; 3],
    pub color2: [u8; 3],
}

/// Starting positions of the four scale lines (`[color1_m1, color1_m2, color2_m1, color2_m2]`;
/// all zero for the gap step) and the per-monitor midpoints the gap lines meet at.
pub fn initial_lines(config: &OverlayConfig) -> ([i32; 4], [i32; 2]) {
    let m1 = &config.monitors[config.m1_idx];
    let m2 = &config.monitors[config.m2_idx];

    let segments = if config.step == OverlayStep::Scale {
        if config.bind_horizontal {
            let min_h = m1.h.min(m2.h);
            [
                m1.y + min_h / 4,
                m2.y + min_h / 4,
                m1.y + 3 * min_h / 4,
                m2.y + 3 * min_h / 4,
            ]
        } else {
            let min_w = m1.w.min(m2.w);
            [
                m1.x + min_w / 4,
                m2.x + min_w / 4,
                m1.x + 3 * min_w / 4,
                m2.x + 3 * min_w / 4,
            ]
        }
    } else {
        [0; 4]
    };

    let mids = config.temp_middles.unwrap_or_else(|| {
        if config.bind_horizontal {
            [m1.y + m1.h / 2, m2.y + m2.h / 2]
        } else {
            [m1.x + m1.w / 2, m2.x + m2.w / 2]
        }
    });

    (segments, mids)
}

// The overlay webview commands exist on every platform so the handler list is the
// same everywhere; on Windows there is never a webview overlay to talk to.
#[cfg(not(target_os = "linux"))]
pub fn close_all() {}

#[cfg(not(target_os = "linux"))]
pub fn current_view() -> Option<OverlayView> {
    None
}

#[cfg(not(target_os = "linux"))]
pub fn finish(_token: u64, _cancelled: bool, _segments: [i32; 4], _gap: i32) {}
