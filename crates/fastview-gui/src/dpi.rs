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
/// at 150 % on the VM), to the nearest pixel (MFC rounds the scaled side;
/// it only shows at scales between, 154 dpi: 16 px is 26).
pub fn img(v: f32) -> f32 {
    // MFC sets the ribbon image scale to dpi / 96 and back to 1.0 when it
    // is above 1 and below 1.1 (`0x100a6ee6`–`0x100a6f14`): 97–105 dpi
    // keep the images at 100 %.
    if unscaled() || (k() > 1.0 && k() < 1.1) {
        v
    } else {
        (v * k()).round()
    }
}

/// A chrome font size given at 100 % (ours stand in for the 9 pt UI font
/// and its bold / small kin), at the system DPI: the system's menu font
/// (Segoe UI 9 pt, MFC's `fontRegular`) is `MulDiv(9, dpi, 72)` px high —
/// 12, 15, 18; other sizes on whole pixels too (a font's cell height is).
pub fn font(px: f32) -> f32 {
    if unscaled() {
        px
    } else {
        (px * mul_div(9, dpi() as i32, 72) as f32 / 12.0).round()
    }
}

/// A dialog font size given at 100 %, at the system DPI: the dialogs' 8 pt
/// font is `MulDiv(8, dpi, 72)` px — 11, 13, 16 (11 px at 100 % is the 8 pt
/// font itself; other sizes scale from it, on whole pixels).
pub fn dlg_font(px: f32) -> f32 {
    if unscaled() {
        px
    } else {
        (px * mul_div(8, dpi() as i32, 72) as f32 / 11.0).round()
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

/// A dialog length measured in pixels at 100 % (the 6 × 13 base units) on
/// the x axis, at the system DPI as MapDialogRect gives it: a template
/// coordinate in dialog units maps to whole pixels by `dlu · bx / 4`
/// rounded, so a 100 % pixel that some dialog unit maps to is that unit
/// mapped again at the system DPI's base units (11 px is 7 DLU: 12, not
/// 11 · 7 / 6 = 13, at 125 %); a pixel no unit maps to (a length not in
/// the template) scales by the base-unit ratio, as [`dx`].
pub fn dlg_x(px: f32) -> f32 {
    dlg_axis(px, 6, base_units().0, 4)
}

/// The same vertically (`dlu · by / 8`).
pub fn dlg_y(px: f32) -> f32 {
    dlg_axis(px, 13, base_units().1, 8)
}

fn dlg_axis(px: f32, b96: i32, b: i32, per: i32) -> f32 {
    if unscaled() {
        return px;
    }
    if px.fract() == 0.0 && px.abs() < 4096.0 {
        let p = px as i32;
        // The unit nearest `px · per / b96`, if it maps back to `px`.
        let dlu = mul_div(p, per, b96);
        if mul_div(dlu, b96, per) == p {
            return mul_div(dlu, b, per) as f32;
        }
    }
    (px * b as f32 / b96 as f32).round()
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

/// A length measured on clones at 100, 125, 150, 175 and 200 % (96, 120, 144, 168, 192 dpi): exact there, linear
/// between, and past 200 % along the last two. Whole pixels. No formula fits these (Segoe UI's hinting), so
/// they stay tables.
pub fn measured5(v: [f32; 5]) -> f32 {
    let d = dpi() as f32;
    if unscaled() {
        return v[0];
    }
    let at = [96.0, 120.0, 144.0, 168.0, 192.0];
    let i = (0..4).find(|&i| d <= at[i + 1]).unwrap_or(3);
    (v[i] + (v[i + 1] - v[i]) * (d - at[i]) / (at[i + 1] - at[i])).round()
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

    /// #78: a template length goes through MapDialogRect at the system
    /// DPI's units (`dlu · bx / 4` rounded), not through the ratio of the
    /// base units: every dialog unit's 100 % pixels map to the unit's own
    /// pixels at 125 / 150 %; a length no unit gives scales by the ratio.
    #[test]
    fn template_lengths_map_as_mapdialogrect() {
        for d in [120, 144] {
            set(d);
            let mut off = 0;
            for dlu in 0..700 {
                let at96 = mul_div(dlu, 6, 4) as f32;
                assert_eq!(dlg_x(at96), dlu_x(dlu), "{d}: {dlu} DLU wide");
                off += usize::from(dx(at96) != dlu_x(dlu));
                let at96 = mul_div(dlu, 13, 8) as f32;
                assert_eq!(dlg_y(at96), dlu_y(dlu), "{d}: {dlu} DLU high");
            }
            assert!(off > 0, "{d}: the ratio is off by a pixel for some units");
        }
        // 7 DLU: 11 px at 100 %, 12 at 125 % (11 · 7 / 6 would say 13).
        set(120);
        assert_eq!((dlg_x(11.0), dx(11.0), dlu_y(7)), (12.0, 13.0, 14.0));
        // A length no unit maps to (10 px) keeps the ratio.
        assert_eq!(dlg_x(10.0), dx(10.0));
        set(96);
        assert_eq!((dlg_x(11.0), dlg_y(7.5)), (11.0, 7.5));
    }

    /// #78: the 8 pt fonts are whole pixels at every scale (13 / 16), and
    /// MFC rounds an image side where it scales it (a custom 154 dpi).
    #[test]
    fn fonts_are_whole_pixels_and_images_round() {
        for (d, f8, f9) in [(120, 13.0, 15.0), (144, 16.0, 18.0), (154, 17.0, 19.0)] {
            set(d);
            assert_eq!((dlg_font(11.0), font(12.0)), (f8, f9), "{d}");
            for px in [9.0, 10.5, 11.0, 13.0, 14.0] {
                assert_eq!(font(px).fract() + dlg_font(px).fract(), 0.0, "{d}: {px}");
            }
        }
        set(120);
        assert_eq!(dlg_font(13.0), 15.0);
        set(154);
        assert_eq!((img(16.0), img(32.0)), (26.0, 51.0));
        // 104 dpi (108 %): MFC keeps its images unscaled, fonts and
        // dialog units follow.
        set(104);
        assert_eq!((img(16.0), img(32.0)), (16.0, 32.0));
        assert_eq!(dlg_font(11.0), mul_div(8, 104, 72) as f32);
        set(106);
        assert_eq!(img(16.0), 18.0);
        set(96);
    }
}
