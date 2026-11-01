//! The frame's minimum size on every resize, not only a drag of its edge.
//! The original's frame passes WM_WINDOWPOSCHANGING to DefWindowProc, which
//! asks WM_GETMINMAXINFO (`0x4728a0`) and grows a smaller size to the
//! minimum: a SetWindowPos, Snap or a program asking 640 × 500 gets the
//! minimum (VM, L9.393). winit answers that message itself and applies its
//! minimum only to a drag, so the main window is subclassed here.

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect, IsIconic, IsZoomed, SWP_NOSIZE, WINDOWPOS, WM_KEYDOWN, WM_SYSKEYDOWN, WM_WINDOWPOSCHANGING};

/// Subclass `hwnd` (once; later calls for it do nothing).
pub fn install(hwnd: HWND) {
    use std::sync::atomic::{AtomicIsize, Ordering};
    static DONE: AtomicIsize = AtomicIsize::new(0);
    if DONE.swap(hwnd as isize, Ordering::Relaxed) == hwnd as isize {
        return;
    }
    // SAFETY: our own top-level window, on its thread; the callback lives as
    // long as the program.
    unsafe {
        SetWindowSubclass(hwnd, Some(proc_), 1, 0);
    }
}

/// [`install`] on the window behind a window handle.
pub fn install_on(handle: &impl winit::raw_window_handle::HasWindowHandle) {
    if let Ok(h) = handle.window_handle() {
        if let winit::raw_window_handle::RawWindowHandle::Win32(h) = h.as_raw() {
            install(h.hwnd.get() as HWND);
        }
    }
}

/// The smallest window size: [`crate::MIN_VISIBLE`] (physical pixels) as
/// the client, plus what the window's frame adds to it now.
fn min_window(hwnd: HWND) -> (i32, i32) {
    let (mut w, mut c) = (RECT { left: 0, top: 0, right: 0, bottom: 0 }, RECT { left: 0, top: 0, right: 0, bottom: 0 });
    // SAFETY: plain queries into local RECTs.
    let ok = unsafe { GetWindowRect(hwnd, &mut w) != 0 && GetClientRect(hwnd, &mut c) != 0 };
    let (fx, fy) = if ok { ((w.right - w.left) - c.right, (w.bottom - w.top) - c.bottom) } else { (0, 0) };
    (crate::MIN_VISIBLE[0] as i32 + fx.max(0), crate::MIN_VISIBLE[1] as i32 + fy.max(0))
}

unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    // The key presses egui-winit turns into copy / paste events (`winkeys`).
    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
        let held = |vk: u16| GetKeyState(vk as i32) < 0;
        crate::winkeys::note_key_down(wparam as u32, lparam & (1 << 30) != 0, held(VK_CONTROL), held(VK_SHIFT), held(VK_MENU));
    }
    // A minimised or maximised window keeps the size Windows gives it.
    if msg == WM_WINDOWPOSCHANGING && lparam != 0 && IsIconic(hwnd) == 0 && IsZoomed(hwnd) == 0 {
        let wp = &mut *(lparam as *mut WINDOWPOS);
        if wp.flags & SWP_NOSIZE == 0 {
            let (mw, mh) = min_window(hwnd);
            wp.cx = wp.cx.max(mw);
            wp.cy = wp.cy.max(mh);
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
