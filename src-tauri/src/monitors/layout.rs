//! Editing and applying the virtual desktop layout (monitor positions).
//!
//! Validation is pure (no OS calls) so it can be unit tested; `apply_positions`
//! is the only part that talks to the OS (Win32 on Windows, xrandr on Linux/X11).

use serde::Deserialize;

use super::Monitor;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorPosition {
    pub id: usize,
    pub x: i32,
    pub y: i32,
}

/// A monitor rectangle in virtual-desktop pixels.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    /// True if the two rectangles share an edge segment of positive length
    /// (touching only at a corner does not count — Windows won't keep that).
    fn shares_edge(&self, o: &Rect) -> bool {
        let x_overlap = self.x.max(o.x) < (self.x + self.w).min(o.x + o.w);
        let y_overlap = self.y.max(o.y) < (self.y + self.h).min(o.y + o.h);
        let touch_x = self.x + self.w == o.x || o.x + o.w == self.x;
        let touch_y = self.y + self.h == o.y || o.y + o.h == self.y;
        (touch_x && y_overlap) || (touch_y && x_overlap)
    }
}

/// Check that a layout is one Windows will accept as-is: the primary monitor at
/// (0, 0), no overlapping monitors, and every monitor connected to the rest by
/// shared edges. (Windows silently rearranges layouts that break these rules.)
pub fn validate_layout(rects: &[Rect], primary: usize) -> Result<(), String> {
    if rects.is_empty() {
        return Err("No monitors to arrange".into());
    }
    let p = rects.get(primary).ok_or("Primary monitor not found")?;
    if p.x != 0 || p.y != 0 {
        return Err("The primary monitor must be at (0, 0)".into());
    }
    for (i, a) in rects.iter().enumerate() {
        for (j, b) in rects.iter().enumerate().skip(i + 1) {
            if a.overlaps(b) {
                return Err(format!("Displays {} and {} overlap", i + 1, j + 1));
            }
        }
    }
    // Flood fill over shared edges from the primary monitor.
    let mut reached = vec![false; rects.len()];
    let mut stack = vec![primary];
    reached[primary] = true;
    while let Some(i) = stack.pop() {
        for j in 0..rects.len() {
            if !reached[j] && rects[i].shares_edge(&rects[j]) {
                reached[j] = true;
                stack.push(j);
            }
        }
    }
    if let Some(j) = reached.iter().position(|r| !r) {
        return Err(format!(
            "Display {} must share an edge with another display",
            j + 1
        ));
    }
    Ok(())
}

/// Move each monitor to the matching rect (same order), writing the new positions
/// to the registry and then applying them all at once. Only positions change —
/// resolution, refresh rate and orientation are left alone.
#[cfg(windows)]
pub fn apply_positions(monitors: &[Monitor], rects: &[Rect]) -> Result<(), String> {
    use std::mem;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, POINTL};
    use windows::Win32::Graphics::Gdi::*;

    for (m, r) in monitors.iter().zip(rects) {
        let (device_name, x, y) = (&m.device_name, &r.x, &r.y);
        let name: Vec<u16> = device_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut dm = DEVMODEW {
            dmSize: mem::size_of::<DEVMODEW>() as u16,
            dmFields: DM_POSITION,
            ..Default::default()
        };
        dm.Anonymous1.Anonymous2.dmPosition = POINTL { x: *x, y: *y };

        let r = unsafe {
            ChangeDisplaySettingsExW(
                PCWSTR(name.as_ptr()),
                Some(&dm),
                HWND::default(),
                CDS_UPDATEREGISTRY | CDS_NORESET,
                None,
            )
        };
        if r != DISP_CHANGE_SUCCESSFUL {
            return Err(format!(
                "Windows rejected the new position for {device_name} (error {})",
                r.0
            ));
        }
    }

    // Apply every staged change in one go.
    let r = unsafe {
        ChangeDisplaySettingsExW(PCWSTR::null(), None, HWND::default(), CDS_TYPE(0), None)
    };
    if r != DISP_CHANGE_SUCCESSFUL {
        return Err(format!("Windows failed to apply the layout (error {})", r.0));
    }
    Ok(())
}

/// Move each monitor to the matching rect (same order) with one `xrandr` call.
/// X11 only: Wayland compositors don't let clients rearrange outputs.
#[cfg(target_os = "linux")]
pub fn apply_positions(monitors: &[Monitor], rects: &[Rect]) -> Result<(), String> {
    use std::process::Command;

    if crate::gtk_main::is_wayland_session() {
        return Err("Changing the display arrangement isn't supported on Wayland. \
                    Use your desktop's display settings instead."
            .into());
    }

    let query = Command::new("xrandr")
        .arg("--query")
        .output()
        .map_err(|e| format!("Could not run xrandr ({e}). Install it (often in a package named x11-xserver-utils or xrandr) and try again."))?;
    if !query.status.success() {
        return Err(format!("xrandr failed: {}", String::from_utf8_lossy(&query.stderr).trim()));
    }
    let outputs = parse_xrandr_outputs(&String::from_utf8_lossy(&query.stdout));

    // Our positions are relative to the primary monitor and xrandr's to the screen
    // origin, so compare both relative to their top-left corner.
    let current: Vec<Rect> = monitors
        .iter()
        .map(|m| Rect {
            x: m.position_x,
            y: m.position_y,
            w: m.resolution_x as i32,
            h: m.resolution_y as i32,
        })
        .collect();
    let names = match_outputs(&current, &outputs)
        .ok_or("Could not match the displays to xrandr outputs. Refresh and try again.")?;

    // X11 has no negative coordinates: shift the new layout so it starts at (0, 0).
    let min_x = rects.iter().map(|r| r.x).min().unwrap_or(0);
    let min_y = rects.iter().map(|r| r.y).min().unwrap_or(0);
    let mut cmd = Command::new("xrandr");
    for (name, r) in names.iter().zip(rects) {
        cmd.arg("--output")
            .arg(name)
            .arg("--pos")
            .arg(format!("{}x{}", r.x - min_x, r.y - min_y));
    }
    let out = cmd
        .output()
        .map_err(|e| format!("Could not run xrandr: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "xrandr rejected the new layout: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Active outputs from `xrandr --query`: lines like
/// `DP-1 connected primary 2560x1440+1920+0 (normal left ...) 597mm x 336mm`.
#[cfg(any(target_os = "linux", test))]
fn parse_xrandr_outputs(text: &str) -> Vec<(String, Rect)> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            continue; // mode lines
        }
        let mut tokens = line.split_whitespace();
        let (Some(name), Some("connected")) = (tokens.next(), tokens.next()) else {
            continue;
        };
        if let Some(rect) = tokens.find_map(parse_geometry) {
            out.push((name.to_string(), rect));
        }
    }
    out
}

/// Parse `WxH+X+Y`.
#[cfg(any(target_os = "linux", test))]
fn parse_geometry(token: &str) -> Option<Rect> {
    let (size, rest) = token.split_once('+')?;
    let (x, y) = rest.split_once('+')?;
    let (w, h) = size.split_once('x')?;
    Some(Rect {
        x: x.parse().ok()?,
        y: y.parse().ok()?,
        w: w.parse().ok()?,
        h: h.parse().ok()?,
    })
}

/// For each monitor rect, the name of the xrandr output with the same size and
/// position (both sets taken relative to their own top-left corner).
#[cfg(any(target_os = "linux", test))]
fn match_outputs(monitors: &[Rect], outputs: &[(String, Rect)]) -> Option<Vec<String>> {
    let origin = |rs: &mut dyn Iterator<Item = &Rect>| {
        rs.fold((i32::MAX, i32::MAX), |(x, y), r| (x.min(r.x), y.min(r.y)))
    };
    let (mx, my) = origin(&mut monitors.iter());
    let (ox, oy) = origin(&mut outputs.iter().map(|(_, r)| r));
    let mut used = vec![false; outputs.len()];
    monitors
        .iter()
        .map(|m| {
            let i = outputs.iter().enumerate().position(|(i, (_, o))| {
                !used[i] && o.w == m.w && o.h == m.h && o.x - ox == m.x - mx && o.y - oy == m.y - my
            })?;
            used[i] = true;
            Some(outputs[i].0.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_xrandr_query() {
        let text = "Screen 0: minimum 8 x 8, current 4480 x 1440, maximum 32767 x 32767
eDP-1 connected 1920x1080+0+180 (normal left inverted right x axis y axis) 309mm x 174mm
   1920x1080     60.01*+
DP-1 connected primary 2560x1440+1920+0 (normal left inverted right x axis y axis) 597mm x 336mm
   2560x1440     59.95*+
HDMI-1 disconnected (normal left inverted right x axis y axis)
DP-2 connected (normal left inverted right x axis y axis)
";
        let outs = parse_xrandr_outputs(text);
        assert_eq!(outs.len(), 2);
        assert_eq!(outs[0].0, "eDP-1");
        assert_eq!((outs[0].1.x, outs[0].1.y, outs[0].1.w, outs[0].1.h), (0, 180, 1920, 1080));
        assert_eq!(outs[1].0, "DP-1");
        assert_eq!((outs[1].1.x, outs[1].1.w), (1920, 2560));
    }

    #[test]
    fn matches_outputs_relative_to_origin() {
        // Monitors are relative to the primary (DP-1), xrandr to the screen origin.
        let monitors = [r(-1920, 180, 1920, 1080), r(0, 0, 2560, 1440)];
        let outputs = vec![
            ("DP-1".to_string(), r(1920, 0, 2560, 1440)),
            ("eDP-1".to_string(), r(0, 180, 1920, 1080)),
        ];
        assert_eq!(match_outputs(&monitors, &outputs).unwrap(), vec!["eDP-1", "DP-1"]);
        assert!(match_outputs(&[r(0, 0, 800, 600)], &outputs).is_none());
    }

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn side_by_side_with_offset_is_valid() {
        let rects = [r(0, 0, 2560, 1440), r(2560, 137, 1920, 1080)];
        assert!(validate_layout(&rects, 0).is_ok());
    }

    #[test]
    fn primary_must_be_at_origin() {
        let rects = [r(5, 0, 1920, 1080), r(1925, 0, 1920, 1080)];
        assert!(validate_layout(&rects, 0).is_err());
    }

    #[test]
    fn overlap_is_rejected() {
        let rects = [r(0, 0, 1920, 1080), r(1919, 0, 1920, 1080)];
        assert!(validate_layout(&rects, 0).unwrap_err().contains("overlap"));
    }

    #[test]
    fn gap_is_rejected() {
        let rects = [r(0, 0, 1920, 1080), r(1921, 0, 1920, 1080)];
        assert!(validate_layout(&rects, 0).unwrap_err().contains("edge"));
    }

    #[test]
    fn corner_only_contact_is_rejected() {
        let rects = [r(0, 0, 1920, 1080), r(1920, 1080, 1920, 1080)];
        assert!(validate_layout(&rects, 0).is_err());
    }

    #[test]
    fn chain_through_non_primary_is_valid() {
        // Third monitor only touches the second one.
        let rects = [
            r(0, 0, 1920, 1080),
            r(1920, 0, 1920, 1080),
            r(1920, 1080, 1920, 1080),
        ];
        assert!(validate_layout(&rects, 0).is_ok());
    }

    #[test]
    fn negative_coordinates_are_fine() {
        let rects = [r(0, 0, 1920, 1080), r(-1080, -400, 1080, 1920)];
        assert!(validate_layout(&rects, 0).is_ok());
    }
}
