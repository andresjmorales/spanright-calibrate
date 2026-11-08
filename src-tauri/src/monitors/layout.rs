//! Editing and applying the Windows virtual desktop layout (monitor positions).
//!
//! Validation is pure (no Win32) so it can be unit tested; `apply_positions`
//! is the only part that talks to Windows.

use serde::Deserialize;

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

/// Write new positions for the given GDI device names to the registry, then apply
/// them all at once. Only positions change — resolution, refresh rate and
/// orientation are left alone.
#[cfg(windows)]
pub fn apply_positions(positions: &[(String, i32, i32)]) -> Result<(), String> {
    use std::mem;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, POINTL};
    use windows::Win32::Graphics::Gdi::*;

    for (device_name, x, y) in positions {
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

#[cfg(test)]
mod tests {
    use super::*;

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
