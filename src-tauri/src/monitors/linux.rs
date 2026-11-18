//! Linux monitor discovery: geometry from GDK (works on X11 and Wayland), EDID
//! from the kernel's DRM connectors in /sys/class/drm.
//!
//! Coordinates: on X11 every monitor shares one scale, and GDK geometry times
//! that scale is the real pixel layout. On Wayland GDK reports logical (scaled)
//! positions; multiplying them by the largest scale gives a pixel space in which
//! no two monitors overlap, which is all calibration needs (each line position
//! is only ever read relative to its own monitor). Positions are then shifted
//! so the primary monitor sits at (0, 0), as on Windows.

use std::fs;
use std::path::Path;

use gtk::prelude::*;

use super::edid::{parse_edid_full, parse_edid_physical_size, EdidInfo};
use super::models::Monitor;
use super::MonitorDetailInfo;
use crate::gtk_main;

/// One GDK monitor, copied out of GDK on the main thread.
struct GdkMon {
    model: String,
    manufacturer: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    scale: i32,
    width_mm: i32,
    height_mm: i32,
    refresh_mhz: i32,
    primary: bool,
}

/// A connected DRM connector, e.g. `card1-DP-2`.
struct Connector {
    /// Connector name without the card prefix, e.g. `DP-2` or `HDMI-A-1`.
    name: String,
    edid: Vec<u8>,
    driver: Option<String>,
}

/// A discovered monitor plus what `get_monitor_info` needs later.
struct Found {
    monitor: Monitor,
    edid: Option<Vec<u8>>,
    connector: Option<String>,
    refresh_mhz: i32,
}

fn gdk_monitors() -> Result<Vec<GdkMon>, String> {
    gtk_main::on_main(|| -> Result<Vec<GdkMon>, String> {
        let display = gtk::gdk::Display::default().ok_or("No display found (is DISPLAY or WAYLAND_DISPLAY set?)")?;
        let primary = display.primary_monitor();
        let mut out = Vec::new();
        for i in 0..display.n_monitors() {
            let Some(m) = display.monitor(i) else { continue };
            let g = m.geometry();
            out.push(GdkMon {
                model: m.model().map(|s| s.to_string()).unwrap_or_default(),
                manufacturer: m.manufacturer().map(|s| s.to_string()).unwrap_or_default(),
                x: g.x(),
                y: g.y(),
                w: g.width(),
                h: g.height(),
                scale: m.scale_factor().max(1),
                width_mm: m.width_mm(),
                height_mm: m.height_mm(),
                refresh_mhz: m.refresh_rate(),
                primary: m.is_primary() || primary.as_ref() == Some(&m),
            });
        }
        Ok(out)
    })?
}

fn drm_connectors() -> Vec<Connector> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return out;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        // Connector directories look like "card0-DP-1"; plain "card0" is the GPU.
        let Some((card, name)) = file_name.split_once('-') else { continue };
        if !card.starts_with("card") {
            continue;
        }
        let dir = entry.path();
        let read = |f: &str| fs::read_to_string(dir.join(f)).unwrap_or_default();
        if read("status").trim() != "connected" || read("enabled").trim() == "disabled" {
            continue;
        }
        let driver = fs::read_link(Path::new("/sys/class/drm").join(card).join("device/driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
        out.push(Connector {
            name: name.to_string(),
            edid: fs::read(dir.join("edid")).unwrap_or_default(),
            driver,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// X11 drivers name outputs differently from the kernel ("HDMI-1" vs "HDMI-A-1").
fn normalize_connector(name: &str) -> String {
    name.to_ascii_lowercase().replace("-a-", "-").replace(['-', '_'], "")
}

/// How likely it is that a GDK monitor and a DRM connector are the same display.
fn match_score(g: &GdkMon, c: &Connector) -> u32 {
    let mut score = 0;
    if !g.model.is_empty() {
        if g.model == c.name {
            score += 10;
        } else if normalize_connector(&g.model) == normalize_connector(&c.name) {
            score += 6;
        }
    }
    if let Some(info) = parse_edid_full(&c.edid) {
        if let Some(edid_name) = info.monitor_name.as_deref().filter(|n| !n.is_empty()) {
            let (model, edid_name) = (g.model.to_ascii_lowercase(), edid_name.to_ascii_lowercase());
            if model == edid_name {
                score += 5;
            } else if !model.is_empty() && (model.contains(&edid_name) || edid_name.contains(&model)) {
                score += 3;
            }
        }
        let near = |a: i32, b: u32| (a - b as i32).abs() <= 10;
        let (w, h) = (info.width_mm, info.height_mm);
        if g.width_mm > 0
            && ((near(g.width_mm, w) && near(g.height_mm, h)) || (near(g.width_mm, h) && near(g.height_mm, w)))
        {
            score += 2;
        }
    }
    score
}

/// Pair GDK monitors with DRM connectors: best scores first, then a leftover
/// one-to-one pair if exactly one of each remains.
fn match_connectors(gdk: &[GdkMon], conns: &[Connector]) -> Vec<Option<usize>> {
    let mut pairs: Vec<(u32, usize, usize)> = Vec::new();
    for (gi, g) in gdk.iter().enumerate() {
        for (ci, c) in conns.iter().enumerate() {
            let s = match_score(g, c);
            if s >= 2 {
                pairs.push((s, gi, ci));
            }
        }
    }
    pairs.sort_by_key(|p| std::cmp::Reverse(p.0));
    let mut by_gdk = vec![None; gdk.len()];
    let mut used = vec![false; conns.len()];
    for (_, gi, ci) in pairs {
        if by_gdk[gi].is_none() && !used[ci] {
            by_gdk[gi] = Some(ci);
            used[ci] = true;
        }
    }
    let free_g: Vec<usize> = (0..gdk.len()).filter(|&i| by_gdk[i].is_none()).collect();
    let free_c: Vec<usize> = (0..conns.len()).filter(|&i| !used[i]).collect();
    if free_g.len() == 1 && free_c.len() == 1 {
        by_gdk[free_g[0]] = Some(free_c[0]);
    }
    by_gdk
}

fn discover() -> Result<Vec<Found>, String> {
    let gdk = gdk_monitors()?;
    if gdk.is_empty() {
        return Err("No monitors found".into());
    }
    let conns = drm_connectors();
    let matched = match_connectors(&gdk, &conns);

    let max_scale = gdk.iter().map(|g| g.scale).max().unwrap_or(1);
    let primary = gdk
        .iter()
        .position(|g| g.primary)
        .or_else(|| gdk.iter().position(|g| g.x <= 0 && g.y <= 0 && g.x + g.w > 0 && g.y + g.h > 0))
        .unwrap_or(0);
    let (ox, oy) = (gdk[primary].x * max_scale, gdk[primary].y * max_scale);

    let mut found = Vec::new();
    for (i, g) in gdk.iter().enumerate() {
        let conn = matched[i].map(|ci| &conns[ci]);
        let edid = conn.map(|c| c.edid.clone()).filter(|e| e.len() >= 128);
        let info = edid.as_deref().and_then(parse_edid_full);

        let resolution_x = (g.w * g.scale) as u32;
        let resolution_y = (g.h * g.scale) as u32;

        // Physical size: EDID first, then what GDK reports (also from EDID, via
        // the X server or compositor). Some drivers report nonsense like 0 or 1 mm.
        let mut size = edid
            .as_deref()
            .and_then(parse_edid_physical_size)
            .map(|s| (s.width_mm, s.height_mm));
        if size.is_none() && (50..2000).contains(&g.width_mm) && (50..2000).contains(&g.height_mm) {
            size = Some((g.width_mm as u32, g.height_mm as u32));
        }
        // EDID sizes are for the unrotated panel; GDK geometry is already rotated.
        if let Some((w, h)) = size {
            if (resolution_x > resolution_y) != (w > h) && w != h && resolution_x != resolution_y {
                size = Some((h, w));
            }
        }

        let mfg_model = format!("{} {}", g.manufacturer, g.model).trim().to_string();
        let friendly_name = info
            .as_ref()
            .and_then(|i| i.monitor_name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| mfg_model.clone());
        let device_name = conn
            .map(|c| c.name.clone())
            .unwrap_or_else(|| if g.model.is_empty() { format!("Display {}", i + 1) } else { g.model.clone() });

        found.push(Found {
            monitor: Monitor {
                id: i,
                device_name,
                friendly_name,
                monitor_name: mfg_model,
                adapter_name: conn.and_then(|c| c.driver.clone()).unwrap_or_default(),
                monitor_device_id: conn.map(|c| c.name.clone()).unwrap_or_default(),
                is_primary: i == primary,
                resolution_x,
                resolution_y,
                position_x: g.x * max_scale - ox,
                position_y: g.y * max_scale - oy,
                orientation: 0,
                physical_width_mm: size.map(|s| s.0),
                physical_height_mm: size.map(|s| s.1),
                physical_width_in: None,
                physical_height_in: None,
                diagonal_in: None,
                ppi: None,
                size_source: if size.is_some() { "edid" } else { "none" }.into(),
            },
            edid,
            connector: conn.map(|c| c.name.clone()),
            refresh_mhz: g.refresh_mhz,
        });
    }
    Ok(found)
}

pub fn enumerate_monitors() -> Result<Vec<Monitor>, String> {
    Ok(discover()?.into_iter().map(|f| f.monitor).collect())
}

pub fn monitor_details(id: usize) -> Result<Option<MonitorDetailInfo>, String> {
    let Some(f) = discover()?.into_iter().find(|f| f.monitor.id == id) else {
        return Ok(None);
    };
    let edid: Option<EdidInfo> = f.edid.as_deref().and_then(parse_edid_full);
    let refresh_rate_hz = (f.refresh_mhz > 0).then(|| ((f.refresh_mhz + 500) / 1000) as u32);
    Ok(Some(MonitorDetailInfo {
        edid,
        refresh_rate_hz,
        connection_type: f.connector.as_deref().map(connection_type),
    }))
}

/// Human-readable connection type from a DRM connector name.
fn connection_type(connector: &str) -> String {
    let kind = connector.rsplit_once('-').map(|(k, _)| k).unwrap_or(connector);
    match kind {
        "eDP" => "DisplayPort (Embedded)",
        "DP" => "DisplayPort (External)",
        k if k.starts_with("HDMI") => "HDMI",
        k if k.starts_with("DVI") => "DVI",
        "VGA" => "VGA",
        "LVDS" => "LVDS",
        "DSI" => "DSI",
        "Virtual" => "Virtual",
        "Writeback" => "Writeback",
        "USB" => "USB",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_types() {
        assert_eq!(connection_type("eDP-1"), "DisplayPort (Embedded)");
        assert_eq!(connection_type("DP-3"), "DisplayPort (External)");
        assert_eq!(connection_type("HDMI-A-1"), "HDMI");
        assert_eq!(connection_type("DVI-D-1"), "DVI");
    }

    #[test]
    fn x11_and_kernel_names_normalize_alike() {
        assert_eq!(normalize_connector("HDMI-1"), normalize_connector("HDMI-A-1"));
        assert_eq!(normalize_connector("DP-2"), normalize_connector("DP-2"));
        assert_ne!(normalize_connector("DP-1"), normalize_connector("DP-2"));
    }
}
