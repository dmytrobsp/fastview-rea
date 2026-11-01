//! The undecorated window's frame as Windows sees it (#34). The window has no system caption
//! (the ribbon draws the caption row, L9.150), so Windows only knew a client area: no snap-layout
//! flyout on the maximise button and no shadow. Here the program tells the frame where its
//! maximise button is, and on Windows the window answers `WM_NCHITTEST` there with `HTMAXBUTTON`
//! (the flyout of Windows 11 shows over such a button), handles that button's non-client mouse
//! messages itself and draws the hover from them, and gets DWM's shadow back.
//!
//! The registry (the button's rectangle and the hover) is plain data, shared with the headless
//! tests; the window procedure is Windows only.

use eframe::egui;
use std::cell::Cell;

thread_local! {
    /// The maximise button's rectangle in client pixels (left, top, right, bottom). The window
    /// procedure and the frames run on the one UI thread.
    static MAX_BUTTON: Cell<Option<[i32; 4]>> = const { Cell::new(None) };
    /// The pointer is over that button as non-client input (the client gets no pointer there).
    static MAX_HOT: Cell<bool> = const { Cell::new(false) };
    /// A modal dialog is up: the frame's buttons are disabled (the original's frame is).
    static MODAL: Cell<bool> = const { Cell::new(false) };
}

/// The frame's state for this frame: while a modal dialog is up no button is published.
pub fn set_modal(up: bool) {
    MODAL.with(|m| m.set(up));
    if up {
        publish_max_button(None, 1.0);
    }
}

/// A modal dialog is up.
pub fn modal() -> bool {
    MODAL.with(Cell::get)
}

/// The button as drawn this frame, in points (`None`: not drawn or not enabled).
pub fn publish_max_button(rect: Option<egui::Rect>, pixels_per_point: f32) {
    let px = |v: f32| (v * pixels_per_point).round() as i32;
    let r = rect.map(|r| [px(r.left()), px(r.top()), px(r.right()), px(r.bottom())]);
    MAX_BUTTON.with(|c| c.set(r));
    if r.is_none() {
        MAX_HOT.with(|h| h.set(false));
    }
    #[cfg(windows)]
    win::frame_to(r.map_or(0, |r| r[3]));
}

/// The published button, client pixels.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn max_button() -> Option<[i32; 4]> {
    MAX_BUTTON.with(Cell::get).filter(|r| r[2] > r[0] && r[3] > r[1])
}

/// Whether the pointer rests on the maximise button as non-client input.
pub fn max_hot() -> bool {
    MAX_HOT.with(Cell::get)
}

/// The top band of a restored window that stays the resize edge (`App::resize_edges`), in pixels.
const RESIZE_BAND: i32 = 5;

/// `WM_NCHITTEST` answers `HTMAXBUTTON` at the client point `(x, y)`: on the published button, but
/// not in the top resize band of a window that is not maximised.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn answers_max_button(x: i32, y: i32, maximised: bool) -> bool {
    on_max_button(x, y) && (maximised || y >= RESIZE_BAND)
}

/// The client point `(x, y)` is on the published maximise button.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn on_max_button(x: i32, y: i32) -> bool {
    max_button().is_some_and(|r| x >= r[0] && x < r[2] && y >= r[1] && y < r[3])
}

#[cfg(windows)]
pub use win::install_on;

#[cfg(windows)]
mod win {
    use super::*;
    use std::sync::atomic::{AtomicI32, AtomicIsize, Ordering};
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::Controls::MARGINS;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, IsZoomed, PostMessageW, HTMAXBUTTON, SC_MAXIMIZE, SC_RESTORE, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_MOUSEMOVE, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_SYSCOMMAND};

    static CTX: OnceLock<egui::Context> = OnceLock::new();

    /// Subclass the window behind `handle` (once): the maximise button's hit test, and DWM's shadow
    /// for a window with no system caption.
    pub fn install_on(handle: &impl winit::raw_window_handle::HasWindowHandle, ctx: &egui::Context) {
        let Ok(h) = handle.window_handle() else { return };
        let winit::raw_window_handle::RawWindowHandle::Win32(h) = h.as_raw() else { return };
        let hwnd = h.hwnd.get() as HWND;
        if CTX.set(ctx.clone()).is_err() {
            return;
        }
        HWND_OURS.store(hwnd as isize, Ordering::Relaxed);
        // SAFETY: our own top-level window, on its thread; the callback lives as long as the
        // program.
        unsafe {
            SetWindowSubclass(hwnd, Some(proc_), 2, 0);
        }
        extend(1);
    }

    static HWND_OURS: AtomicIsize = AtomicIsize::new(0);
    static TOP: AtomicI32 = AtomicI32::new(0);

    /// DWM's frame extended into the client by a pixel on three sides and `top` pixels at the top:
    /// that brings the shadow (and Windows 11's rounded corners and border) back to a window
    /// whose client fills the frame, and makes the caption row's strip part of the frame, where
    /// Windows raises the snap-layout flyout for a button answered `HTMAXBUTTON` (VM: with a
    /// one-pixel top margin the flyout never came). Our picture covers it all, opaque.
    fn extend(top: i32) {
        let hwnd = HWND_OURS.load(Ordering::Relaxed) as HWND;
        if hwnd.is_null() || TOP.swap(top, Ordering::Relaxed) == top {
            return;
        }
        let m = MARGINS { cxLeftWidth: 1, cxRightWidth: 1, cyTopHeight: top.max(1), cyBottomHeight: 1 };
        // SAFETY: our own window; a plain call with a local struct.
        unsafe {
            DwmExtendFrameIntoClientArea(hwnd, &m);
        }
    }

    /// The caption row's height in pixels (the button's bottom edge), as the top margin.
    pub fn frame_to(bottom: i32) {
        if bottom > 0 {
            extend(bottom + 2);
        }
    }

    fn set_hot(hwnd: HWND, on: bool) {
        if MAX_HOT.with(|h| h.replace(on)) != on {
            if on {
                // The leave of the non-client area, so the hover cannot stick when the pointer
                // leaves the window straight from the button.
                let mut t = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE | TME_NONCLIENT, hwndTrack: hwnd, dwHoverTime: 0 };
                // SAFETY: a local struct for our own window.
                unsafe {
                    TrackMouseEvent(&mut t);
                }
            }
            if let Some(c) = CTX.get() {
                c.request_repaint();
            }
        }
    }

    unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
        match msg {
            // The point in screen pixels: over our maximise button Windows treats the window as
            // having one there (the snap-layout flyout, Windows 11).
            WM_NCHITTEST if IsIconic(hwnd) == 0 => {
                let r = DefSubclassProc(hwnd, msg, wparam, lparam);
                if r == 1 /* HTCLIENT */ {
                    let mut p = POINT { x: (lparam & 0xffff) as i16 as i32, y: ((lparam >> 16) & 0xffff) as i16 as i32 };
                    if ScreenToClient(hwnd, &mut p) != 0 && answers_max_button(p.x, p.y, IsZoomed(hwnd) != 0) {
                        return HTMAXBUTTON as LRESULT;
                    }
                }
                r
            }
            // Non-client input on the button: the hover is ours to draw; a click is swallowed on
            // the press and acts on the release, as a caption button does.
            WM_NCMOUSEMOVE => {
                set_hot(hwnd, wparam == HTMAXBUTTON as WPARAM);
                // Windows' own handling of the move (its hover tracking) is what raises the flyout.
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_MOUSEMOVE => {
                set_hot(hwnd, false);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_NCMOUSELEAVE => {
                set_hot(hwnd, false);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if wparam == HTMAXBUTTON as WPARAM => 0,
            WM_NCLBUTTONUP if wparam == HTMAXBUTTON as WPARAM => {
                let cmd = if IsZoomed(hwnd) != 0 { SC_RESTORE } else { SC_MAXIMIZE };
                PostMessageW(hwnd, WM_SYSCOMMAND, cmd as WPARAM, 0);
                0
            }
            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_published_button_is_hit_in_client_pixels() {
        publish_max_button(Some(egui::Rect::from_min_size(egui::pos2(100.0, 0.0), egui::vec2(46.0, 31.0))), 1.25);
        assert_eq!(max_button(), Some([125, 0, 183, 39]));
        assert!(on_max_button(125, 0) && on_max_button(182, 38));
        assert!(!on_max_button(183, 10) && !on_max_button(124, 10) && !on_max_button(150, 39));
        publish_max_button(None, 1.25);
        assert!(!on_max_button(150, 10), "no button published: none hit");
        publish_max_button(Some(egui::Rect::from_min_size(egui::pos2(100.0, 0.0), egui::vec2(46.0, 31.0))), 1.0);
        assert!(!answers_max_button(120, 2, false), "the top resize band of a restored window stays the client's");
        assert!(answers_max_button(120, 6, false) && answers_max_button(120, 2, true));
        MAX_HOT.with(|h| h.set(true));
        publish_max_button(None, 1.0);
        assert!(!max_hot(), "unpublished: no hover");
    }
}
