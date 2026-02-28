#![allow(unused_must_use)]

use std::ffi::c_void;
use std::sync::mpsc;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone)]
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
    pub bind_horizontal: bool,
    /// Per-monitor midpoints from the scale step (y-coords for horizontal, x for vertical).
    /// [0] = midpoint on m1, [1] = midpoint on m2.
    pub temp_middles: Option<[i32; 2]>,
}

pub struct OverlayResult {
    pub cancelled: bool,
    pub segments: [i32; 4],
    pub gap: i32,
}

struct State {
    step: OverlayStep,
    m1_idx: usize,
    m2_idx: usize,
    monitors: Vec<MonitorRect>,
    bind_horizontal: bool,

    segments: [i32; 4],
    gap: i32,
    mid_m1: i32,
    mid_m2: i32,

    selected: Option<usize>,
    dragging: bool,
    drag_start: i32,
    drag_start_val: i32,
    last_interacted: Option<usize>,

    confirmed: bool,
    cancelled: bool,
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn run_overlay(config: OverlayConfig) -> Result<OverlayResult, String> {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let result = run_overlay_window(config);
        let _ = tx.send(result);
    });

    rx.recv()
        .map_err(|_| "Overlay thread failed".to_string())?
}

fn run_overlay_window(config: OverlayConfig) -> Result<OverlayResult, String> {
    unsafe {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        let m1 = &config.monitors[config.m1_idx];
        let m2 = &config.monitors[config.m2_idx];

        let initial_segments = if config.step == OverlayStep::Scale {
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

        let [mid_m1, mid_m2] = config.temp_middles.unwrap_or_else(|| {
            if config.bind_horizontal {
                [m1.y + m1.h / 2, m2.y + m2.h / 2]
            } else {
                [m1.x + m1.w / 2, m2.x + m2.w / 2]
            }
        });

        let mut state = Box::new(State {
            step: config.step,
            m1_idx: config.m1_idx,
            m2_idx: config.m2_idx,
            monitors: config.monitors,
            bind_horizontal: config.bind_horizontal,
            segments: initial_segments,
            gap: 0,
            mid_m1,
            mid_m2,
            selected: None,
            dragging: false,
            drag_start: 0,
            drag_start_val: 0,
            last_interacted: None,
            confirmed: false,
            cancelled: false,
        });

        let class_name = encode_wide("SpanrightCalibrationOverlay");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: HINSTANCE::default(),
            hCursor: LoadCursorW(HINSTANCE::default(), IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let state_ptr = &mut *state as *mut State as *const c_void;
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            vx,
            vy,
            vw,
            vh,
            HWND::default(),
            HMENU::default(),
            HINSTANCE::default(),
            Some(state_ptr),
        )
        .map_err(|e| format!("CreateWindowExW: {e}"))?;

        ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);

        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, HWND::default(), 0, 0);
            if ret.0 <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnregisterClassW(PCWSTR(class_name.as_ptr()), HINSTANCE::default()).ok();

        Ok(OverlayResult {
            cancelled: state.cancelled,
            segments: state.segments,
            gap: state.gap,
        })
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        let cs = &*(lparam.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        return LRESULT(0);
    }

    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let state = &mut *ptr;

    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            match state.step {
                OverlayStep::Scale => draw_scale(state, hdc),
                OverlayStep::Gap => draw_gap(state, hdc),
            }
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = VIRTUAL_KEY(wparam.0 as u16);
            match vk {
                VK_RETURN => {
                    state.confirmed = true;
                    DestroyWindow(hwnd);
                }
                VK_ESCAPE => {
                    state.cancelled = true;
                    DestroyWindow(hwnd);
                }
                VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT => {
                    let delta = if vk == VK_UP || vk == VK_LEFT {
                        -1
                    } else {
                        1
                    };
                    if state.step == OverlayStep::Scale {
                        if let Some(idx) = state.last_interacted {
                            state.segments[idx] += delta;
                            InvalidateRect(hwnd, None, BOOL(0));
                        }
                    } else {
                        state.gap += delta;
                        InvalidateRect(hwnd, None, BOOL(0));
                    }
                }
                _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (mx, my) = mouse_pos(lparam);
            if state.step == OverlayStep::Scale {
                state.selected = hit_test_scale(state, mx, my);
                if state.selected.is_some() {
                    state.dragging = true;
                    state.drag_start = if state.bind_horizontal { my } else { mx };
                    state.drag_start_val = state.segments[state.selected.unwrap()];
                    state.last_interacted = state.selected;
                    SetCapture(hwnd);
                }
            } else {
                state.dragging = true;
                state.drag_start = if state.bind_horizontal { mx } else { my };
                state.drag_start_val = state.gap;
                SetCapture(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if state.dragging {
                let (mx, my) = mouse_pos(lparam);
                if state.step == OverlayStep::Scale {
                    if let Some(idx) = state.selected {
                        let pos = if state.bind_horizontal { my } else { mx };
                        let delta = pos - state.drag_start;
                        state.segments[idx] = state.drag_start_val + delta;
                        InvalidateRect(hwnd, None, BOOL(0));
                    }
                } else {
                    let pos = if state.bind_horizontal { mx } else { my };
                    let delta = pos - state.drag_start;
                    state.gap = state.drag_start_val + delta;
                    InvalidateRect(hwnd, None, BOOL(0));
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if state.dragging {
                state.dragging = false;
                state.selected = None;
                ReleaseCapture().ok();
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn mouse_pos(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn hit_test_scale(state: &State, mx: i32, my: i32) -> Option<usize> {
    let m1 = &state.monitors[state.m1_idx];
    let m2 = &state.monitors[state.m2_idx];
    let threshold = 20;

    if state.bind_horizontal {
        let rects = [
            (m1.x, m1.x + m1.w, state.segments[0]),
            (m2.x, m2.x + m2.w, state.segments[1]),
            (m1.x, m1.x + m1.w, state.segments[2]),
            (m2.x, m2.x + m2.w, state.segments[3]),
        ];
        for (i, &(left, right, y)) in rects.iter().enumerate() {
            if mx >= left && mx <= right && (my - y).abs() <= threshold {
                return Some(i);
            }
        }
    } else {
        let rects = [
            (m1.y, m1.y + m1.h, state.segments[0]),
            (m2.y, m2.y + m2.h, state.segments[1]),
            (m1.y, m1.y + m1.h, state.segments[2]),
            (m2.y, m2.y + m2.h, state.segments[3]),
        ];
        for (i, &(top, bottom, x)) in rects.iter().enumerate() {
            if my >= top && my <= bottom && (mx - x).abs() <= threshold {
                return Some(i);
            }
        }
    }
    None
}

unsafe fn draw_scale(state: &State, hdc: HDC) {
    let m1 = &state.monitors[state.m1_idx];
    let m2 = &state.monitors[state.m2_idx];

    fill_background(hdc, state);

    // Highlight paired monitors
    draw_monitor_frame(hdc, m1);
    draw_monitor_frame(hdc, m2);

    let blue = rgb(100, 141, 250);
    let red = rgb(250, 90, 110);
    let line_h = 6;

    if state.bind_horizontal {
        // Blue line segments
        fill_rect(hdc, m1.x, state.segments[0] - line_h / 2, m1.w, line_h, blue);
        fill_rect(hdc, m2.x, state.segments[1] - line_h / 2, m2.w, line_h, blue);
        // Red line segments
        fill_rect(hdc, m1.x, state.segments[2] - line_h / 2, m1.w, line_h, red);
        fill_rect(hdc, m2.x, state.segments[3] - line_h / 2, m2.w, line_h, red);
    } else {
        let line_w = 6;
        fill_rect(hdc, state.segments[0] - line_w / 2, m1.y, line_w, m1.h, blue);
        fill_rect(hdc, state.segments[1] - line_w / 2, m2.y, line_w, m2.h, blue);
        fill_rect(hdc, state.segments[2] - line_w / 2, m1.y, line_w, m1.h, red);
        fill_rect(hdc, state.segments[3] - line_w / 2, m2.y, line_w, m2.h, red);
    }

    // Instructions
    let text = "Drag each colored line so it sits at the same physical height on both displays.\n\
                Keep blue and red as far apart as possible for best accuracy.\n\
                Arrow keys: \u{00B1}1px  |  Enter: confirm  |  Esc: cancel";
    draw_text_at(hdc, m1.x + 20, m1.y + m1.h - 60, text);
    draw_text_at(hdc, m2.x + 20, m2.y + m2.h - 60, text);
}

unsafe fn draw_gap(state: &State, hdc: HDC) {
    let m1 = &state.monitors[state.m1_idx];
    let m2 = &state.monitors[state.m2_idx];

    fill_background(hdc, state);
    draw_monitor_frame(hdc, m1);
    draw_monitor_frame(hdc, m2);

    let blue = rgb(100, 141, 250);
    let red = rgb(250, 90, 110);
    let gap = state.gap;
    let pen_w = 4;

    if state.bind_horizontal {
        let (left_mid, right_mid, left_m, right_m) = if m1.x < m2.x {
            (state.mid_m1, state.mid_m2, m1, m2)
        } else {
            (state.mid_m2, state.mid_m1, m2, m1)
        };

        let bx = left_m.x + left_m.w;
        let arm = (left_m.w.min(right_m.w) * 2 / 5).max(150);
        let inset = pen_w + 2; // pull lines back from edges to prevent bleed

        // All lines stay at exactly 45 degrees. The gap translates the
        // right-side lines vertically: a 45-degree line crossing G pixels of
        // horizontal gap drops G pixels vertically, so the right-side pair
        // shifts by +gap / -gap respectively.

        // Left monitor lines are fixed at left_mid (anchored)
        draw_line(hdc, bx - arm, left_mid - arm, bx - inset, left_mid - inset, blue, pen_w);
        draw_line(hdc, bx - arm, left_mid + arm, bx - inset, left_mid + inset, red, pen_w);

        // Right monitor lines translated by gap
        draw_line(hdc, bx + inset, right_mid + gap + inset, bx + arm, right_mid + gap + arm, blue, pen_w);
        draw_line(hdc, bx + inset, right_mid - gap - inset, bx + arm, right_mid - gap - arm, red, pen_w);
    } else {
        let (top_mid, bottom_mid, top_m, bottom_m) = if m1.y < m2.y {
            (state.mid_m1, state.mid_m2, m1, m2)
        } else {
            (state.mid_m2, state.mid_m1, m2, m1)
        };

        let by = top_m.y + top_m.h;
        let arm = (top_m.h.min(bottom_m.h) * 2 / 5).max(150);
        let inset = pen_w + 2;

        // Top monitor lines fixed at top_mid
        draw_line(hdc, top_mid - arm, by - arm, top_mid - inset, by - inset, blue, pen_w);
        draw_line(hdc, top_mid + arm, by - arm, top_mid + inset, by - inset, red, pen_w);

        // Bottom monitor lines translated by gap
        draw_line(hdc, bottom_mid + gap + inset, by + inset, bottom_mid + gap + arm, by + arm, blue, pen_w);
        draw_line(hdc, bottom_mid - gap - inset, by + inset, bottom_mid - gap - arm, by + arm, red, pen_w);
    }

    let text = format!(
        "Gap: {}px  |  Drag or arrow keys to adjust  |  Enter: confirm  |  Esc: cancel",
        gap
    );
    draw_text_at(hdc, m1.x + 20, m1.y + m1.h - 40, &text);
    draw_text_at(hdc, m2.x + 20, m2.y + m2.h - 40, &text);
}

unsafe fn fill_background(hdc: HDC, state: &State) {
    let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
    let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);

    let bg = CreateSolidBrush(rgb(10, 12, 18));
    let full = RECT {
        left: 0,
        top: 0,
        right: vw,
        bottom: vh,
    };
    FillRect(hdc, &full, bg);
    DeleteObject(HGDIOBJ(bg.0));

    // Slightly lighter bg on paired monitors
    let light = CreateSolidBrush(rgb(22, 25, 32));
    for &idx in &[state.m1_idx, state.m2_idx] {
        let m = &state.monitors[idx];
        let r = RECT {
            left: m.x,
            top: m.y,
            right: m.x + m.w,
            bottom: m.y + m.h,
        };
        FillRect(hdc, &r, light);
    }
    DeleteObject(HGDIOBJ(light.0));
}

unsafe fn draw_monitor_frame(hdc: HDC, m: &MonitorRect) {
    let pen = CreatePen(PS_SOLID, 1, rgb(60, 65, 80));
    let old = SelectObject(hdc, HGDIOBJ(pen.0));
    let null_brush = GetStockObject(NULL_BRUSH);
    let old_brush = SelectObject(hdc, null_brush);

    Rectangle(hdc, m.x, m.y, m.x + m.w, m.y + m.h);

    SelectObject(hdc, old_brush);
    SelectObject(hdc, old);
    DeleteObject(HGDIOBJ(pen.0));
}

unsafe fn fill_rect(hdc: HDC, x: i32, y: i32, w: i32, h: i32, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    let r = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    FillRect(hdc, &r, brush);
    DeleteObject(HGDIOBJ(brush.0));
}

unsafe fn draw_line(hdc: HDC, x1: i32, y1: i32, x2: i32, y2: i32, color: COLORREF, width: i32) {
    let pen = CreatePen(PS_SOLID, width, color);
    let old = SelectObject(hdc, HGDIOBJ(pen.0));
    MoveToEx(hdc, x1, y1, None);
    LineTo(hdc, x2, y2);
    SelectObject(hdc, old);
    DeleteObject(HGDIOBJ(pen.0));
}

unsafe fn draw_text_at(hdc: HDC, x: i32, y: i32, text: &str) {
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, rgb(160, 165, 175));
    let wide: Vec<u16> = text.encode_utf16().collect();
    TextOutW(hdc, x, y, &wide);
}
