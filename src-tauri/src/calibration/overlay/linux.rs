//! Linux overlay: one borderless full-screen webview per monitor (a single
//! window can't span monitors on Wayland, and many X11 window managers won't
//! allow it either). Every window draws the same scene in virtual-desktop
//! pixels, offset to its own monitor, and broadcasts line changes to the others
//! (see `src/components/Overlay.tsx`). Rust only hands out the step and waits
//! for Enter or Esc.

use std::sync::{mpsc, Mutex};

use gtk::prelude::*;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use super::{initial_lines, OverlayConfig, OverlayResult, OverlayStep, OverlayView};
use crate::gtk_main;

const LABEL_PREFIX: &str = "overlay-";
const VIEW_EVENT: &str = "overlay://view";

struct Session {
    view: Option<OverlayView>,
    tx: Option<mpsc::Sender<OverlayResult>>,
    next_token: u64,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    view: None,
    tx: None,
    next_token: 1,
});

/// Show one calibration step and block until the user confirms or cancels it.
/// Must not be called on the GTK main thread. Overlay windows stay open between
/// steps; call `close_all` when calibration ends.
pub fn run_overlay(config: OverlayConfig) -> Result<OverlayResult, String> {
    let app = gtk_main::app()?;
    let (segments, [mid_m1, mid_m2]) = initial_lines(&config);
    let (tx, rx) = mpsc::channel();

    let view = {
        let mut s = SESSION.lock().unwrap();
        let token = s.next_token;
        s.next_token += 1;
        let view = OverlayView {
            token,
            step: match config.step {
                OverlayStep::Scale => "scale",
                OverlayStep::Gap => "gap",
            },
            m1_idx: config.m1_idx,
            m2_idx: config.m2_idx,
            monitors: config.monitors,
            labels: config.monitor_labels,
            bind_horizontal: config.bind_horizontal,
            segments,
            mid_m1,
            mid_m2,
            color1: config.color1,
            color2: config.color2,
        };
        s.view = Some(view.clone());
        s.tx = Some(tx);
        view
    };

    // Windows that are already open pick the new step up from this event; new
    // ones fetch it with the `overlay_view` command once their page has loaded.
    let _ = app.emit(VIEW_EVENT, &view);
    open_windows(app, view.monitors.len(), view.m1_idx)?;

    rx.recv()
        .map_err(|_| "Calibration cancelled".to_string())
}

/// The step currently shown, for overlay windows that just finished loading.
pub fn current_view() -> Option<OverlayView> {
    SESSION.lock().unwrap().view.clone()
}

/// Called by an overlay window on Enter (confirm) or Esc (cancel).
pub fn finish(token: u64, cancelled: bool, segments: [i32; 4], gap: i32) {
    let mut s = SESSION.lock().unwrap();
    if s.view.as_ref().map(|v| v.token) != Some(token) {
        return;
    }
    if let Some(tx) = s.tx.take() {
        let _ = tx.send(OverlayResult {
            cancelled,
            segments,
            gap,
        });
    }
}

/// Close every overlay window and give focus back to the main window.
pub fn close_all() {
    {
        let mut s = SESSION.lock().unwrap();
        s.view = None;
        s.tx = None;
    }
    let Ok(app) = gtk_main::app() else { return };
    for (label, w) in app.webview_windows() {
        if label.starts_with(LABEL_PREFIX) {
            let _ = w.destroy();
        }
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_focus();
    }
}

/// Treat an overlay window closed by the window manager (Alt+F4 etc.) as Esc.
fn cancel_pending() {
    let mut s = SESSION.lock().unwrap();
    if let Some(tx) = s.tx.take() {
        let _ = tx.send(OverlayResult {
            cancelled: true,
            segments: [0; 4],
            gap: 0,
        });
    }
}

/// Open a full-screen overlay window on every GDK monitor that doesn't have one.
/// Monitor ids are GDK monitor indices (see `monitors::linux`).
fn open_windows(app: &tauri::AppHandle, count: usize, focus_idx: usize) -> Result<(), String> {
    for idx in 0..count {
        let label = format!("{LABEL_PREFIX}{idx}");
        if app.get_webview_window(&label).is_some() {
            continue;
        }

        let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
            .title("Spanright Calibrate")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .build()
            .map_err(|e| format!("Could not open the calibration overlay: {e}"))?;

        win.on_window_event(|event| {
            if let WindowEvent::Destroyed = event {
                cancel_pending();
            }
        });

        let w = win.clone();
        gtk_main::on_main(move || -> Result<(), String> {
            let gw = w.gtk_window().map_err(|e| e.to_string())?;
            let screen = GtkWindowExt::screen(&gw).ok_or("No GDK screen")?;
            // Place the window on its monitor first (X11 window managers use
            // this), then ask for full screen on that monitor (works on Wayland).
            if let Some(mon) = gtk::gdk::Display::default().and_then(|d| d.monitor(idx as i32)) {
                let g = mon.geometry();
                gw.move_(g.x(), g.y());
                gw.resize(g.width(), g.height());
            }
            gw.fullscreen_on_monitor(&screen, idx as i32);
            w.show().map_err(|e| e.to_string())?;
            Ok(())
        })??;
    }

    // Give keyboard focus to an overlay on the calibrated pair so Enter, Esc and
    // the arrow keys work without clicking first.
    if let Some(w) = app.get_webview_window(&format!("{LABEL_PREFIX}{focus_idx}")) {
        let _ = w.set_focus();
    }
    Ok(())
}
