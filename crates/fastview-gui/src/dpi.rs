//! The system DPI the MFC-side chrome is laid out for (#12).
//!
//! The original is system-DPI aware (its manifest's `dpiAware`): what it
//! paints itself — the lanes, their label column and 15 / 13 px fonts
//! (`0x404df9`–`0x404f38`), waveforms, rulers, diagrams — stays in device
//! pixels at every scale, while MFC's own parts grow with the system DPI:
//! the fonts it takes from the system (the 9 pt menu font, the dialogs'
//! 8 pt one), the images it stretches by `dpi / 96` (the ribbon's,
//! toolbars', menus'), and the heights it derives from them (the ribbon's
//! rows, pane captions, tab strips, the status bar, menu rows), dialogs in
//! dialog units of their font. Ours keeps one point per device pixel (#5,
//! #6) and scales those parts by this module; at 96 dpi every function
//! here returns its argument unchanged, so 100 % stays pixel for pixel.
//!
//! The scale is the process's, fixed at start-up as the original's system
//! DPI is (a per-thread value: the UI runs on one thread, each test on its
//! own).

use std::cell::Cell;

thread_local! {
    static DPI: Cell<u32> = const { Cell::new(96) };
}

/// Lay the chrome out for `dpi` (96 = 100 %).
pub fn set(dpi: u32) {
    DPI.with(|d| d.set(dpi.max(48)));
}

/// The system DPI the chrome is laid out for.
pub fn dpi() -> u32 {
    DPI.with(|d| d.get())
}

/// Whether the chrome is at 100 %.
pub fn unscaled() -> bool {
    dpi() == 96
}

/// The display scale, `dpi / 96`.
pub fn k() -> f32 {
    dpi() as f32 / 96.0
}

/// Windows' MulDiv: `a · b / c` rounded half away from zero.
pub fn mul_div(a: i32, b: i32, c: i32) -> i32 {
    let p = a as i64 * b as i64;
    let q = c as i64;
    (if p >= 0 { (2 * p + q) / (2 * q) } else { -((-2 * p + q) / (2 * q)) }) as i32
}

/// A chrome length laid out at 100 % at the system DPI, on whole pixels
/// (unchanged at 100 %, fractions included).
pub fn s(v: f32) -> f32 {
    if unscaled() {
        v
    } else {
        (v * k()).round()
    }
}

/// A half-pixel length (a 1 px stroke's centre offset, `n + 0.5`) at the
/// system DPI: the whole part scaled, the half kept.
pub fn half(v: f32) -> f32 {
    if unscaled() {
        v
    } else {
        ((v - 0.5) * k()).round() + 0.5
    }
}

/// An MFC image side at the system DPI: the ribbon's, toolbars' and
/// menus' images are stretched by `dpi / 96` (CMFCToolBarImages with
/// `GetRibbonImageScale`, the 16 / 32 px images 20 / 40 at 125 %, 24 / 48
/// at 150 % on the VM).
pub fn img(v: f32) -> f32 {
    if unscaled() {
        v
    } else {
        (v * k()).floor()
    }
}

/// The 9 pt UI font's scale: the system's menu font (Segoe UI 9 pt,
/// MFC's `fontRegular`) is `MulDiv(9, dpi, 72)` px high — 12, 15, 18.
pub fn ui_font_k() -> f32 {
    mul_div(9, dpi() as i32, 72) as f32 / 12.0
}

/// A chrome font size given at 100 % (ours stand in for the 9 pt UI font
/// and its bold / small kin), at the system DPI.
pub fn font(px: f32) -> f32 {
    if unscaled() {
        px
    } else {
        px * mul_div(9, dpi() as i32, 72) as f32 / 12.0
    }
}

/// The dialogs' 8 pt font's scale: `MulDiv(8, dpi, 72)` px — 11, 13, 16.
pub fn dlg_font_k() -> f32 {
    mul_div(8, dpi() as i32, 72) as f32 / 11.0
}

/// A dialog font size given at 100 %, at the system DPI.
pub fn dlg_font(px: f32) -> f32 {
    if unscaled() {
        px
    } else {
        px * mul_div(8, dpi() as i32, 72) as f32 / 11.0
    }
}

/// The dialogs' base units (GetDialogBaseUnits of their 8 pt font): (6,
/// 13) at 96 dpi, (7, 16) at 120, (9, 19) at 144 (VM: the 341 × 258 DLU
/// «Печать» 512 × 419, 597 × 516 and 767 × 613 px). Elsewhere from the
/// font: its height 1.2 em, its mean letter width 0.53 em.
pub fn base_units() -> (i32, i32) {
    match dpi() {
        96 => (6, 13),
        120 => (7, 16),
        144 => (9, 19),
        d => {
            let em = mul_div(8, d as i32, 72) as f32;
            ((0.53 * em + 0.25).round() as i32, (1.2 * em).round() as i32)
        }
    }
}

/// A dialog x length in pixels (MapDialogRect: `dlu · bx / 4`).
pub fn dlu_x(dlu: i32) -> f32 {
    mul_div(dlu, base_units().0, 4) as f32
}

/// A dialog y length in pixels (`dlu · by / 8`).
pub fn dlu_y(dlu: i32) -> f32 {
    mul_div(dlu, base_units().1, 8) as f32
}

/// A dialog's pixel length laid out at 100 % (`px` of the 6 × 13 base
/// units) at the system DPI's base units, horizontally.
pub fn dx(px: f32) -> f32 {
    if unscaled() {
        px
    } else {
        (px * base_units().0 as f32 / 6.0).round()
    }
}

/// The same vertically.
pub fn dy(px: f32) -> f32 {
    if unscaled() {
        px
    } else {
        (px * base_units().1 as f32 / 13.0).round()
    }
}

/// Which of a bitmap's `n` rows (its first a line) shows on row `i` of the
/// `h` it is stretched over: the first on the first, the rest spread
/// evenly (the identity at `h == n`).
pub fn stretch_row(i: usize, h: usize, n: usize) -> usize {
    if i == 0 || h <= 1 || n <= 1 {
        0
    } else {
        (1 + (i - 1) * (n - 1) / (h - 1).max(1)).min(n - 1)
    }
}

/// A length measured on the VM at 100 %, 125 % and 150 % (`a`, `b`, `c`):
/// exact there, in between and beyond along the nearest two, whole pixels.
pub fn measured(a: f32, b: f32, c: f32) -> f32 {
    let k = k();
    if unscaled() {
        return a;
    }
    let v = if k <= 1.25 { a + (b - a) * (k - 1.0) / 0.25 } else { b + (c - b) * (k - 1.25) / 0.25 };
    v.round()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_at_96_and_windows_metrics_above() {
        set(96);
        assert_eq!((s(22.5), img(16.0), font(10.5), dlg_font(11.0), dx(7.5), dy(0.5)), (22.5, 16.0, 10.5, 11.0, 7.5, 0.5));
        assert_eq!((dlu_x(341), dlu_y(258)), (512.0, 419.0));
        set(120);
        assert_eq!((img(16.0), img(32.0), font(12.0), dlg_font(11.0)), (20.0, 40.0, 15.0, 13.0));
        assert_eq!((dlu_x(341), dlu_y(258)), (597.0, 516.0));
        assert_eq!(measured(30.0, 36.0, 43.0), 36.0);
        set(144);
        assert_eq!((img(16.0), img(32.0), font(12.0), dlg_font(11.0)), (24.0, 48.0, 18.0, 16.0));
        assert_eq!((dlu_x(341), dlu_y(258)), (767.0, 613.0));
        assert_eq!(measured(30.0, 36.0, 43.0), 43.0);
        // The reporter's 154 dpi: past 150 % along the 125–150 % slope.
        set(154);
        assert_eq!(measured(30.0, 36.0, 43.0), 46.0);
        set(96);
    }
}
