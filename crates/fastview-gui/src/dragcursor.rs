//! The channel drag's move-drop cursor. The original's drag is an OLE one and
//! its drop targets answer "move": OLE shows its own cursor for that, an
//! arrow with a small box under it (clone: the cursor over the view
//! and the diagrams during a drag is a system-made one, not a stock IDC_
//! shape; over the ribbon it is the stock "no" one). winit's cursor set has
//! no such shape, so on Windows the main window's WM_SETCURSOR is answered
//! with a cursor made here while a move drop is shown (#157); elsewhere the
//! plain arrow stays the nearest.

/// The cursor's size in pixels (a standard one).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub const SIZE: usize = 32;

/// The arrow, the tip at the top-left corner (the hot spot): `X` black,
/// `o` white.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
const ARROW: [&str; 16] = [
    "X",
    "XX",
    "XoX",
    "XooX",
    "XoooX",
    "XooooX",
    "XoooooX",
    "XooooooX",
    "XoooooooX",
    "XooooooXXX",
    "XooXooX",
    "XoX XooX",
    "XX  XooX",
    "X    XooX",
    "     XooX",
    "      XX",
];

/// One pixel of the cursor image.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Px {
    Clear,
    Black,
    White,
}

/// The cursor image, row by row: the box (white inside, a black outline 2 px
/// thick) at the lower left, the arrow over it.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn image() -> [[Px; SIZE]; SIZE] {
    let mut img = [[Px::Clear; SIZE]; SIZE];
    let (x0, y0, x1, y1) = (4usize, 16usize, 16usize, 27usize);
    for (y, row) in img.iter_mut().enumerate().take(y1 + 1).skip(y0) {
        for (x, px) in row.iter_mut().enumerate().take(x1 + 1).skip(x0) {
            let edge = x < x0 + 2 || x + 2 > x1 || y < y0 + 2 || y + 2 > y1;
            *px = if edge { Px::Black } else { Px::White };
        }
    }
    for (row, art) in img.iter_mut().zip(ARROW) {
        for (px, c) in row.iter_mut().zip(art.chars()) {
            match c {
                'X' => *px = Px::Black,
                'o' => *px = Px::White,
                _ => {}
            }
        }
    }
    img
}

/// Tell the window a move drop is shown (`true`) or not any more: on Windows
/// the cursor changes at once if it was not this one.
#[cfg(not(windows))]
pub fn show_move_drop(_on: bool) {}

#[cfg(windows)]
pub use win::{handle_set_cursor, show_move_drop};

#[cfg(windows)]
mod win {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, PostMessageW, SetCursor, HCURSOR, HTCLIENT, ICONINFO, WM_MOUSEMOVE, WM_SETCURSOR};

    static ON: AtomicBool = AtomicBool::new(false);
    static HWND_OURS: AtomicIsize = AtomicIsize::new(0);
    static CURSOR: AtomicIsize = AtomicIsize::new(0);

    /// The cursor, made on first use.
    fn cursor() -> HCURSOR {
        let have = CURSOR.load(Ordering::Relaxed);
        if have != 0 {
            return have as HCURSOR;
        }
        let img = image();
        // The AND mask (1 = the screen shows through) and the XOR bits
        // (1 = white where the mask is 0), one bit per pixel, rows of 4 bytes.
        let (mut and, mut xor) = (vec![0xffu8; SIZE * SIZE / 8], vec![0u8; SIZE * SIZE / 8]);
        for (y, row) in img.iter().enumerate() {
            for (x, px) in row.iter().enumerate() {
                let (i, bit) = (y * SIZE / 8 + x / 8, 0x80u8 >> (x % 8));
                match px {
                    Px::Clear => {}
                    Px::Black => and[i] &= !bit,
                    Px::White => {
                        and[i] &= !bit;
                        xor[i] |= bit;
                    }
                }
            }
        }
        // A black-and-white icon: one mask bitmap twice as tall, the AND
        // mask over the XOR bits.
        let mut bits = and;
        bits.extend_from_slice(&xor);
        // SAFETY: plain GDI / USER calls on a buffer that outlives them; the
        // bitmap is freed after the cursor copied it.
        let h = unsafe {
            let m = CreateBitmap(SIZE as i32, 2 * SIZE as i32, 1, 1, bits.as_ptr().cast());
            let info = ICONINFO { fIcon: 0, xHotspot: 0, yHotspot: 0, hbmMask: m, hbmColor: std::ptr::null_mut() };
            let h = CreateIconIndirect(&info);
            DeleteObject(m);
            h
        };
        CURSOR.store(h as isize, Ordering::Relaxed);
        h
    }

    /// Remember the window the cursor is for (the subclass sees it first).
    pub fn show_move_drop(on: bool) {
        if ON.swap(on, Ordering::Relaxed) != on {
            // egui-winit sets its own cursor after the frame; ask the window
            // to choose again once that is done.
            let hwnd = HWND_OURS.load(Ordering::Relaxed) as HWND;
            if !hwnd.is_null() {
                // SAFETY: a plain post to our own window.
                unsafe {
                    PostMessageW(hwnd, WM_SETCURSOR, hwnd as WPARAM, (HTCLIENT as isize) | ((WM_MOUSEMOVE as isize) << 16));
                }
            }
        }
    }

    /// The subclass's WM_SETCURSOR: over the client area while a move drop is
    /// shown the cursor is ours (true: handled).
    pub fn handle_set_cursor(hwnd: HWND, lparam: LPARAM) -> Option<LRESULT> {
        HWND_OURS.store(hwnd as isize, Ordering::Relaxed);
        if ON.load(Ordering::Relaxed) && (lparam & 0xffff) as u32 == HTCLIENT {
            let c = cursor();
            if !c.is_null() {
                // SAFETY: a cursor handle made above.
                unsafe {
                    SetCursor(c);
                }
                return Some(1);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The image: an arrow with the hot spot at its tip, a box lower left of
    /// it, everything else clear.
    #[test]
    fn the_move_drop_cursor_is_an_arrow_over_a_box() {
        let img = image();
        assert_eq!(img[0][0], Px::Black, "the arrow's tip is the hot spot");
        assert_eq!(img[4][2], Px::White, "the arrow's inside");
        assert_eq!(img[4][6], Px::Clear, "beside the arrow");
        assert_eq!(img[22][10], Px::White, "the box's inside");
        assert_eq!(img[16][8], Px::Black, "the box's outline: black");
        assert_eq!(img[17][8], Px::Black, "two pixels thick");
        assert_eq!(img[18][8], Px::White, "and no more");
        assert_eq!(img[30][30], Px::Clear, "the corner is clear");
        let drawn = img.iter().flatten().filter(|p| **p != Px::Clear).count();
        assert!(drawn > 150 && drawn < 400, "{drawn}");
    }
}
