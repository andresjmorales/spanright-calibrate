use crate::monitors::Monitor;

/// Compute pairwise calibration order.
/// Returns vec of (unbound_monitor_idx, bound_monitor_idx).
pub fn compute_calibration_order(monitors: &[Monitor]) -> Vec<(usize, usize)> {
    if monitors.len() < 2 {
        return vec![];
    }

    let primary_idx = monitors
        .iter()
        .position(|m| m.is_primary)
        .unwrap_or(0);

    let mut bound = vec![false; monitors.len()];
    bound[primary_idx] = true;

    let mut pairs = Vec::new();

    while bound.iter().any(|&b| !b) {
        let mut best_dist = f64::MAX;
        let mut best_unbound = 0;
        let mut best_bound = 0;

        for (i, m) in monitors.iter().enumerate() {
            if bound[i] {
                continue;
            }
            let cx_i = m.position_x as f64 + m.resolution_x as f64 / 2.0;
            let cy_i = m.position_y as f64 + m.resolution_y as f64 / 2.0;

            for (j, n) in monitors.iter().enumerate() {
                if !bound[j] {
                    continue;
                }
                let cx_j = n.position_x as f64 + n.resolution_x as f64 / 2.0;
                let cy_j = n.position_y as f64 + n.resolution_y as f64 / 2.0;

                let dist = ((cx_i - cx_j).powi(2) + (cy_i - cy_j).powi(2)).sqrt();
                if dist < best_dist {
                    best_dist = dist;
                    best_unbound = i;
                    best_bound = j;
                }
            }
        }

        bound[best_unbound] = true;
        pairs.push((best_unbound, best_bound));
    }

    pairs
}

/// Determine if two monitors are side-by-side (horizontal binding)
/// or stacked (vertical binding).
pub fn determine_bind_horizontal(m1: &Monitor, m2: &Monitor) -> bool {
    let m1_top = m1.position_y;
    let m1_bottom = m1.position_y + m1.resolution_y as i32;
    let m2_top = m2.position_y;
    let m2_bottom = m2.position_y + m2.resolution_y as i32;
    let v_overlap = (m1_bottom.min(m2_bottom) - m1_top.max(m2_top)).max(0);

    let m1_left = m1.position_x;
    let m1_right = m1.position_x + m1.resolution_x as i32;
    let m2_left = m2.position_x;
    let m2_right = m2.position_x + m2.resolution_x as i32;
    let h_overlap = (m1_right.min(m2_right) - m1_left.max(m2_left)).max(0);

    v_overlap >= h_overlap
}
