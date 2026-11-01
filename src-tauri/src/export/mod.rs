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

/// Per-monitor Windows virtual-desktop position (pixel coords).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanrightWindowsPosition {
    pub pixel_x: i32,
    pub pixel_y: i32,
}

/// Matches Spanright's SavedConfig
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanrightSavedConfig {
    pub id: String,
    pub name: String,
    pub saved_at: u64,
    pub monitors: Vec<SpanrightMonitor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_arrangement: Option<Vec<SpanrightWindowsPosition>>,
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
    include_virtual_layout: bool,
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
                // Windows reports a rotated monitor's resolution in its already-rotated
                // logical form (e.g. 1080x1920 for a portrait panel), and the preset
                // resolution/aspectRatio plus physical placement above are all derived
                // from those rotated dimensions. Emitting rotation here too would make
                // Spanright rotate an already-portrait monitor a second time. The
                // orientation is therefore encoded entirely in the dimensions.
                rotation: None,
                display_name,
            }
        })
        .collect();

    let windows_arrangement = if include_virtual_layout {
        Some(
            placements
                .iter()
                .map(|p| {
                    let m = &monitors[p.monitor_idx];
                    SpanrightWindowsPosition {
                        pixel_x: m.position_x,
                        pixel_y: m.position_y,
                    }
                })
                .collect(),
        )
    } else {
        None
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    SpanrightSavedConfig {
        id: generate_uuid(),
        name: "Calibrated Layout".to_string(),
        saved_at: now,
        monitors: spanright_monitors,
        windows_arrangement,
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

        let offset_in =
            r.align_offset_bound / ppi_bound - r.align_offset_unbound / ppi_unbound;

        let (x, y) = if r.bind_horizontal {
            // `gap` is measured in pixels of the right-hand monitor (see CalibrationResult::gap).
            let unbound_first = m.position_x < m_bound.position_x;
            let gap_ppi = if unbound_first { ppi_bound } else { ppi_unbound };
            let gap_in = (r.gap as f64).max(0.0) / gap_ppi;

            let px = if unbound_first {
                bound_placement.0 - w - gap_in
            } else {
                bound_placement.0 + bound_placement.2 + gap_in
            };
            (px, bound_placement.1 + offset_in)
        } else {
            // `gap` is measured in pixels of the bottom monitor.
            let unbound_first = m.position_y < m_bound.position_y;
            let gap_ppi = if unbound_first { ppi_bound } else { ppi_unbound };
            let gap_in = (r.gap as f64).max(0.0) / gap_ppi;

            let py = if unbound_first {
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
    include_virtual_layout: bool,
) -> Result<String, String> {
    // Results reference monitors by index; if displays were added/removed since
    // calibration those indices are stale and would panic below.
    if monitors.is_empty()
        || results
            .iter()
            .any(|r| r.monitor_id >= monitors.len() || r.bound_to >= monitors.len())
    {
        return Err(
            "Monitor configuration changed since calibration. Please recalibrate.".to_string(),
        );
    }
    let config = build_spanright_config(monitors, results, include_virtual_layout);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(id: usize, rx: u32, ry: u32, px: i32, py: i32, ppi: Option<f64>) -> Monitor {
        Monitor {
            id,
            device_name: format!("\\\\.\\DISPLAY{}", id + 1),
            friendly_name: String::new(),
            monitor_name: String::new(),
            adapter_name: String::new(),
            monitor_device_id: String::new(),
            is_primary: id == 0,
            resolution_x: rx,
            resolution_y: ry,
            position_x: px,
            position_y: py,
            orientation: 0,
            physical_width_mm: None,
            physical_height_mm: None,
            physical_width_in: None,
            physical_height_in: None,
            diagonal_in: None,
            ppi,
            size_source: "none".into(),
        }
    }

    fn result(id: usize, bound_to: usize, scale: f64, gap: i32) -> CalibrationResult {
        CalibrationResult {
            monitor_id: id,
            scale,
            relative_x: 0.0,
            relative_y: 0.0,
            gap,
            bound_to,
            bind_horizontal: true,
            align_offset_unbound: 0.0,
            align_offset_bound: 0.0,
        }
    }

    #[test]
    fn gap_uses_ppi_of_right_hand_monitor() {
        // Primary 100 PPI on the left, unbound 200 PPI on the right.
        // The gap overlay measures in right-monitor pixels: 200 px = 1".
        let mons = vec![
            monitor(0, 1000, 500, 0, 0, Some(100.0)),
            monitor(1, 1000, 500, 1000, 0, Some(200.0)),
        ];
        let p = compute_physical_placements(&mons, &[result(1, 0, 2.0, 200)]);
        let right = p.iter().find(|p| p.monitor_idx == 1).unwrap();
        assert!((right.x - (10.0 + 1.0)).abs() < 1e-9, "x = {}", right.x);

        // Mirror: unbound on the left, so the bound (primary) monitor is on the right.
        let mons = vec![
            monitor(0, 1000, 500, 0, 0, Some(100.0)),
            monitor(1, 1000, 500, -1000, 0, Some(200.0)),
        ];
        let p = compute_physical_placements(&mons, &[result(1, 0, 2.0, 100)]);
        let left = p.iter().find(|p| p.monitor_idx == 1).unwrap();
        assert!((left.x - (-5.0 - 1.0)).abs() < 1e-9, "x = {}", left.x);
    }

    #[test]
    fn ppi_propagates_through_chain_with_pairwise_scale() {
        // Only the primary has a known PPI; B is bound to A, A to the primary.
        let mons = vec![
            monitor(0, 1000, 500, 0, 0, Some(100.0)),
            monitor(1, 1000, 500, 1000, 0, None),
            monitor(2, 1000, 500, 2000, 0, None),
        ];
        let results = [result(1, 0, 1.5, 0), result(2, 1, 2.0, 0)];
        let p = compute_physical_placements(&mons, &results);
        let b = p.iter().find(|p| p.monitor_idx == 2).unwrap();
        // ppi_B = 100 * 1.5 * 2.0 = 300 -> width 1000/300
        assert!((b.w - 1000.0 / 300.0).abs() < 1e-9, "w = {}", b.w);
    }

    #[test]
    fn negative_gap_is_clamped_to_touching() {
        // Active areas can't physically overlap; a negative gap is a mis-alignment.
        let mons = vec![
            monitor(0, 1000, 500, 0, 0, Some(100.0)),
            monitor(1, 1000, 500, 1000, 0, Some(100.0)),
        ];
        let p = compute_physical_placements(&mons, &[result(1, 0, 1.0, -50)]);
        let right = p.iter().find(|p| p.monitor_idx == 1).unwrap();
        assert!((right.x - 10.0).abs() < 1e-9, "x = {}", right.x);
    }

    // ---- End-to-end simulation of the two overlay steps ----

    /// Ground truth for one monitor: virtual-desktop position + physical placement.
    struct Truth {
        ppi: f64,
        phys_x: f64, // inches, top-left of the active area
        phys_y: f64,
    }

    fn to_phys(m: &Monitor, t: &Truth, px: f64, py: f64) -> (f64, f64) {
        (
            t.phys_x + (px - m.position_x as f64) / t.ppi,
            t.phys_y + (py - m.position_y as f64) / t.ppi,
        )
    }

    /// What a user produces in the overlay for the pair (unbound, bound), given the
    /// true physical layout. Scale step: two lines per monitor at the same physical
    /// coordinates. Gap step: the gap value at which the far monitor's diagonal lines
    /// (drawn exactly as in overlay::draw_gap) continue the near monitor's lines.
    fn simulate_pair(
        mons: &[Monitor],
        truth: &[Truth],
        unbound: usize,
        bound: usize,
        bound_scale: f64,
    ) -> (CalibrationResult, f64) {
        use crate::calibration::{flow, pair};
        let (mu, mb) = (&mons[unbound], &mons[bound]);
        let (tu, tb) = (&truth[unbound], &truth[bound]);
        let horizontal = flow::determine_bind_horizontal(mu, mb);

        // Scale step: pick two physical coordinates inside both monitors' shared span.
        let (lo, hi) = if horizontal {
            let top = tu.phys_y.max(tb.phys_y);
            let bot = (tu.phys_y + mu.resolution_y as f64 / tu.ppi)
                .min(tb.phys_y + mb.resolution_y as f64 / tb.ppi);
            (top + (bot - top) * 0.2, top + (bot - top) * 0.8)
        } else {
            let left = tu.phys_x.max(tb.phys_x);
            let right = (tu.phys_x + mu.resolution_x as f64 / tu.ppi)
                .min(tb.phys_x + mb.resolution_x as f64 / tb.ppi);
            (left + (right - left) * 0.2, left + (right - left) * 0.8)
        };
        let px_at = |m: &Monitor, t: &Truth, phys: f64| -> i32 {
            if horizontal {
                m.position_y + ((phys - t.phys_y) * t.ppi).round() as i32
            } else {
                m.position_x + ((phys - t.phys_x) * t.ppi).round() as i32
            }
        };
        let segments = [px_at(mu, tu, lo), px_at(mb, tb, lo), px_at(mu, tu, hi), px_at(mb, tb, hi)];
        let mid_u = (segments[0] + segments[2]) / 2;
        let mid_b = (segments[1] + segments[3]) / 2;

        // Gap step, mirroring draw_gap: near (left/top) lines meet the near monitor's far
        // edge at near_mid; far lines start at the far monitor's own edge, shifted by gap.
        let unbound_first = if horizontal {
            mu.position_x < mb.position_x
        } else {
            mu.position_y < mb.position_y
        };
        let (near, tn, near_mid, far, tf, far_mid) = if unbound_first {
            (mu, tu, mid_u, mb, tb, mid_b)
        } else {
            (mb, tb, mid_b, mu, tu, mid_u)
        };
        let misalignment = |gap: i32| -> f64 {
            // color1 runs +45° (down-right), color2 the mirror; both must line up.
            let mut err = 0.0;
            for sign in [1.0, -1.0] {
                let (ex, ey, fx, fy) = if horizontal {
                    let edge = (near.position_x + near.resolution_x as i32) as f64;
                    let (ex, ey) = to_phys(near, tn, edge, near_mid as f64);
                    let (fx, fy) = to_phys(far, tf, far.position_x as f64, far_mid as f64 + sign * gap as f64);
                    (ex, ey, fx, fy)
                } else {
                    let edge = (near.position_y + near.resolution_y as i32) as f64;
                    let (ex, ey) = to_phys(near, tn, near_mid as f64, edge);
                    let (fx, fy) = to_phys(far, tf, far_mid as f64 + sign * gap as f64, far.position_y as f64);
                    (ex, ey, fx, fy)
                };
                // On a 45° line through (ex, ey): the cross-boundary distance equals the
                // along-boundary shift (times the line's direction sign).
                err += if horizontal {
                    ((fy - ey) - sign * (fx - ex)).abs()
                } else {
                    ((fx - ex) - sign * (fy - ey)).abs()
                };
            }
            err
        };
        let gap = (-500..=2000)
            .min_by(|a, b| misalignment(*a).partial_cmp(&misalignment(*b)).unwrap())
            .unwrap();

        pair::pair_result(
            mons,
            unbound,
            bound,
            pair::Origin { x: mu.position_x, y: mu.position_y },
            pair::Origin { x: mb.position_x, y: mb.position_y },
            horizontal,
            segments,
            gap,
            bound_scale,
        )
    }

    fn assert_layout_recovered(mons: Vec<Monitor>, truth: Vec<Truth>) {
        let order = crate::calibration::flow::compute_calibration_order(&mons);
        let mut scales = vec![1.0; mons.len()];
        let mut results = Vec::new();
        for (u, b) in order {
            let (r, s) = simulate_pair(&mons, &truth, u, b, scales[b]);
            scales[u] = s;
            results.push(r);
        }
        let placements = compute_physical_placements(&mons, &results);
        assert_eq!(placements.len(), mons.len());
        let primary = mons.iter().position(|m| m.is_primary).unwrap();
        let (rx, ry) = (truth[primary].phys_x, truth[primary].phys_y);
        for p in &placements {
            let t = &truth[p.monitor_idx];
            let m = &mons[p.monitor_idx];
            // Placement is relative to the primary at (0, 0). Allow ~1.5 px of
            // rounding (scale lines and midpoints are integer pixels).
            let tol = 1.5 / t.ppi;
            assert!((p.x - (t.phys_x - rx)).abs() < tol, "monitor {} x {} vs {}", p.monitor_idx, p.x, t.phys_x - rx);
            assert!((p.y - (t.phys_y - ry)).abs() < tol, "monitor {} y {} vs {}", p.monitor_idx, p.y, t.phys_y - ry);
            assert!((p.w - m.resolution_x as f64 / t.ppi).abs() < 0.02, "monitor {} w", p.monitor_idx);
        }
    }

    #[test]
    fn stacked_unbound_below_with_offset_and_different_ppi() {
        // Primary 27" QHD on top; a 1080p laptop-ish panel below, shifted right, 0.9" gap.
        let mons = vec![
            monitor(0, 2560, 1440, 0, 0, Some(108.8)),
            monitor(1, 1920, 1080, 400, 1440, Some(141.2)),
        ];
        let truth = vec![
            Truth { ppi: 108.8, phys_x: 0.0, phys_y: 0.0 },
            Truth { ppi: 141.2, phys_x: 5.3, phys_y: 1440.0 / 108.8 + 0.9 },
        ];
        assert_layout_recovered(mons, truth);
    }

    #[test]
    fn stacked_unbound_above() {
        let mons = vec![
            monitor(0, 1920, 1080, 0, 0, Some(141.2)),
            monitor(1, 2560, 1440, -300, -1440, Some(108.8)),
        ];
        let truth = vec![
            Truth { ppi: 141.2, phys_x: 0.0, phys_y: 0.0 },
            Truth { ppi: 108.8, phys_x: -2.1, phys_y: -(1440.0 / 108.8) - 1.25 },
        ];
        assert_layout_recovered(mons, truth);
    }

    #[test]
    fn stacked_size_derived_from_scale_step() {
        // Bottom monitor has no known size: it must come out of the scale step.
        let mons = vec![
            monitor(0, 2560, 1440, 0, 0, Some(108.8)),
            monitor(1, 1920, 1080, 300, 1440, None),
        ];
        let truth = vec![
            Truth { ppi: 108.8, phys_x: 0.0, phys_y: 0.0 },
            Truth { ppi: 141.2, phys_x: 3.0, phys_y: 1440.0 / 108.8 + 0.6 },
        ];
        assert_layout_recovered(mons, truth);
    }

    #[test]
    fn three_high_stack_and_side_monitor() {
        // Laptop (primary) at the bottom, monitor stacked above it, a third to the
        // right of that monitor; the third binds to the middle one through the chain.
        let mons = vec![
            monitor(0, 1920, 1080, 0, 0, Some(141.2)),
            monitor(1, 2560, 1440, -320, -1440, None),
            monitor(2, 1920, 1080, 2240, -1440, Some(91.8)),
        ];
        let top_y = -(1440.0 / 108.8) - 1.0;
        let truth = vec![
            Truth { ppi: 141.2, phys_x: 0.0, phys_y: 0.0 },
            Truth { ppi: 108.8, phys_x: -4.0, phys_y: top_y },
            Truth { ppi: 91.8, phys_x: -4.0 + 2560.0 / 108.8 + 0.7, phys_y: top_y + 1.5 },
        ];
        assert_layout_recovered(mons, truth);
    }

    #[test]
    fn side_by_side_still_recovered() {
        let mons = vec![
            monitor(0, 2560, 1440, 0, 0, Some(108.8)),
            monitor(1, 1920, 1080, 2560, 200, Some(141.2)),
        ];
        let truth = vec![
            Truth { ppi: 108.8, phys_x: 0.0, phys_y: 0.0 },
            Truth { ppi: 141.2, phys_x: 2560.0 / 108.8 + 1.1, phys_y: 2.4 },
        ];
        assert_layout_recovered(mons, truth);
    }

    #[test]
    fn stale_results_are_rejected() {
        let mons = vec![monitor(0, 1000, 500, 0, 0, Some(100.0))];
        assert!(export_json(&mons, &[result(1, 0, 1.0, 0)], false).is_err());
    }
}
