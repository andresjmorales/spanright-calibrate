//! Linux only: access to the app handle and the GTK main thread.
//!
//! GDK (monitor discovery) and GTK (overlay windows) may only be used from the
//! thread running the GTK main loop, but commands can run elsewhere (async
//! commands always do), so anything touching GTK goes through `on_main`.

use std::sync::{mpsc, OnceLock};
use tauri::AppHandle;

static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn set_app(handle: AppHandle) {
    let _ = APP.set(handle);
}

pub fn app() -> Result<&'static AppHandle, String> {
    APP.get().ok_or_else(|| "App is not initialized yet".to_string())
}

/// Run `f` on the GTK main thread and wait for its result. Runs it directly
/// when already on that thread (blocking there would deadlock).
pub fn on_main<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    if gtk::is_initialized_main_thread() {
        return Ok(f());
    }
    let (tx, rx) = mpsc::channel();
    app()?
        .run_on_main_thread(move || {
            let _ = tx.send(f());
        })
        .map_err(|e| format!("Could not reach the main thread: {e}"))?;
    rx.recv()
        .map_err(|_| "The main thread did not respond".to_string())
}

/// True in a Wayland session (including X11 apps running under XWayland, where
/// X11 tools can't rearrange the real outputs either).
pub fn is_wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|t| t.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}
