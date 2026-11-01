//! Geometry of the oscillogram view, as the original computes it (ledger
//! L9.4, `docs/original/specs/ui-behaviour.md` §3): a sample-indexed horizontal scale
//! (first visible sample, power-of-two `f`, 25 px per sample at `f = 1`),
//! fixed lane heights, the per-channel min..max amplitude mapping, the time
//! ruler and the wheel zoom steps.

use crate::app::TimeUnit;

/// Pixels per sample at `f = 1.0`.
pub const PX_PER_SAMPLE: f64 = 25.0;
/// Height of a digital lane (`0x460290`).
pub const DIGITAL_H: f32 = 20.0;
/// Default analog lane height `View/YAnalogSize`.
pub const Y_ANALOG_DEFAULT: f32 = 93.0;
/// Time ruler height at the bottom of the view.
pub const RULER_H: f32 = 30.0;
/// Extra height of the difference band (`doc+0x870`, 33033) above the ruler.
pub const DIFF_BAND_H: f32 = 45.0;
/// Vertical scroll per wheel notch.
pub const WHEEL_NOTCH_PX: f32 = 120.0;

/// The fit scale (`0x45c430`): start at 1.0 and halve while
/// `width / (n·25) < f` — the largest power of two that shows the whole record.
pub fn fit_scale(width: f32, n: usize) -> f64 {
    let ratio = f64::from(width) / (n.max(1) as f64 * PX_PER_SAMPLE);
    let mut f = 1.0f64;
    while ratio < f && f > 1e-12 {
        f *= 0.5;
    }
    f
}

/// Windows' window metrics at a system DPI (GetSystemMetrics), which fix how a default MDI child
/// is sized and how wide the view's scroll bars are (#147, #45). Measured on clones at 96, 120,
/// 144, 168 and 192 dpi (SM_CXSIZEFRAME, SM_CXPADDEDBORDER, SM_CYCAPTION, SM_CXVSCROLL): the
/// system DPI's, in physical pixels — the original is system-DPI aware, so none of them scale
/// with anything else. All five points follow, with Windows' MulDiv rounding: the caption
/// `MulDiv(22, dpi, 96) + 1`, the sizing frame `MulDiv(1, dpi, 96) + 3`, the padded border
/// `MulDiv(4, dpi, 96)`, the scroll bar `MulDiv(17, dpi, 96)`; any other dpi uses them too.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WinMetrics {
    pub caption: f32,
    pub size_frame: f32,
    pub padded: f32,
    pub scroll: f32,
}

impl WinMetrics {
    pub fn at(dpi: u32) -> WinMetrics {
        let md = |v: i32| crate::dpi::mul_div(v, dpi as i32, 96) as f32;
        WinMetrics { caption: md(22) + 1.0, size_frame: md(1) + 3.0, padded: md(4), scroll: md(17) }
    }

    /// One side of a window's sizing border (SM_CXSIZEFRAME + SM_CXPADDEDBORDER: 8 px at 100 %).
    pub fn border(&self) -> f32 {
        self.size_frame + self.padded
    }

    /// The cascade step of a default MDI child (SM_CYCAPTION + SM_CYSIZEFRAME − 1: 26 px at 100 %).
    pub fn step(&self) -> f32 {
        self.caption + self.size_frame - 1.0
    }
}

/// The shortest view (its client after the horizontal bar) that shows a vertical scroll bar: 74 px at 100 %
/// (clone, #175: resizing the original's frame step by step, the bar appears when the view goes from 73 to 74
/// px high, with lanes far taller than the view; none below), the smallest lane 70 px plus the view's 4 px
/// edge. Not scaled: the view paints in device pixels at every scale (L9.406).
pub const VBAR_FLOOR: f32 = 74.0;

/// The width the view measures for its first zoom (`0x45c430`, `GetClientRect` of the view, in its
/// `OnInitialUpdate` `0x45ab70`), from the MDI client `area_w` × `area_h`, the lanes' total height
/// `lanes_h`, the ruler band's `band` (30, 75 with the difference band) and the system DPI's
/// window metrics `m`.
///
/// Windows places a new MDI child with CW_USEDEFAULT at the MDI client less one cascade step
/// ([`WinMetrics::step`]: Wine's `MDI_CalcDefaultChildPos`, and measured on a clone with a plain
/// MDICLIENT at 96 / 120 / 144 / 168 / 192 dpi) per three steps of its height; the child's frame
/// takes two sizing borders across (16 px at 100 %) and, below its caption, the same again, the
/// view's own edge 4 (2 · SM_CXEDGE, never scaled). The view's vertical scroll bar is already
/// there when the zoom is measured if it has something to scroll: measured with a window-event
/// log of the original, the view's client went 442 → 442 × 195 (the horizontal bar) → 425 (the
/// vertical one) on a maximised start at 100 %, and 184 → 184 × 54 (the horizontal bar only) on a
/// first run's small frame. Its height there, 54 px, is under the floor ([`VBAR_FLOOR`], pinned on a clone
/// by resizing the frame, #175). Above the floor it is the
/// original's own scroll-bar rule (`0x45d4a0`): the lanes taller than the view (after its
/// horizontal bar) less the ruler band plus 2 — a clone at 125 %: one lane (93 px) in a view 166
/// high, the band 75 (the difference band shown), is exactly at the limit and shows no bar; at 150 % in 148 it does. On a first
/// run's small frame the bar is also missing at 125 % / 150 % (the view 9 / 0 px high after its
/// horizontal bar), opened from the command line or in the running program with the frame shown:
/// not the frame's visibility.
///
/// At 125 % / 150 % (clone, the original: first run 230 × 113 → view 176 wide, 229 × 86 → 203;
/// maximised 568 × 334 → 450 and 571 × 310 → 469 before the bar; a synthetic record's first zoom
/// at 150 % showed the bar counted in the maximised case, 469 − 26 = 443, and not in the first
/// run's 203) the rule is the same with the metrics of that DPI.
pub fn view_width_at_creation(area_w: f32, area_h: f32, lanes_h: f32, band: f32, m: WinMetrics) -> f32 {
    let step = m.step();
    let k = (area_h / (3.0 * step)).floor();
    let child_h = area_h - step * k;
    // The view: the child's frame (the borders and the caption) and its own edge (4), then the
    // horizontal bar.
    let view_h = child_h - m.caption - 2.0 * m.border() - 4.0 - m.scroll;
    let vbar = view_h >= VBAR_FLOOR && lanes_h > view_h - band + 2.0;
    (area_w - step * k - 2.0 * m.border() - 4.0 - if vbar { m.scroll } else { 0.0 }).max(1.0)
}

/// Initial scale of a new view (`0x45ab70`): the stored `View/XScale`, by
/// default twice the fit scale.
pub fn initial_scale(stored: Option<f64>, width: f32, n: usize) -> f64 {
    stored.unwrap_or_else(|| 2.0 * fit_scale(width, n))
}

/// Samples in view: `(width − left)/25/f − 1`.
pub fn visible_count(width: f32, left: f32, f: f64) -> usize {
    let v = (f64::from(width - left) / PX_PER_SAMPLE / f).floor() - 1.0;
    if v > 0.0 {
        v as usize
    } else {
        0
    }
}

/// Largest valid first sample: `n − visible − 1` (0 when it all fits).
pub fn max_first(n: usize, visible: usize) -> usize {
    n.saturating_sub(visible + 1)
}

/// x of sample `k`: `left + round((k − first)·25·f)`.
pub fn x_of(k: f64, first: usize, f: f64, left: f32) -> f32 {
    left + ((k - first as f64) * PX_PER_SAMPLE * f).round() as f32
}

/// Sample under `x`, rounded to the nearest (ties up) (`0x495980`).
pub fn sample_at(x: f32, first: usize, f: f64, left: f32) -> i64 {
    (f64::from(x - left) / (PX_PER_SAMPLE * f) + first as f64 + 0.5).floor() as i64
}

/// Ctrl+wheel up (32802): `f ×2` while below 1.0, anchored on a визир:
/// `first' = c − (c − first)/2`.
pub fn zoom_in(f: f64, first: usize, anchor: Option<usize>) -> (f64, usize) {
    if f >= 1.0 {
        return (f, first);
    }
    let first = match anchor {
        // Signed int arithmetic as in the original (the anchor may be left of `first`).
        Some(c) => (c as i64 - (c as i64 - first as i64) / 2).max(0) as usize,
        None => first,
    };
    (f * 2.0, first)
}

/// Ctrl+wheel down (32803): `f ÷2`, not below the fit scale:
/// `first' = 2·first − c`.
pub fn zoom_out(f: f64, fit: f64, first: usize, anchor: Option<usize>) -> (f64, usize) {
    if f / 2.0 < fit {
        return (f, first);
    }
    let first = match anchor {
        Some(c) => (2 * first).saturating_sub(c),
        None => first,
    };
    (f / 2.0, first)
}

/// Alt+wheel (32800 / 32801): the analog lane height ×1.3 (below 500) or
/// ÷1.3 (above 70), rounded.
pub fn lane_zoom(y: f32, grow: bool) -> f32 {
    if grow && y < 500.0 {
        (y * 1.3).round()
    } else if !grow && y > 70.0 {
        (y / 1.3).round()
    } else {
        y
    }
}

/// Value → y in a lane (`0x4944a0`): `top + h − 5 − round((v − min)(h − 10)/(max − min))`,
/// the lane middle when the range is empty.
/// Where a level line sits: `draw` for the painter, `hit` for the hit test
/// and the info box (`None`: off-screen, never hit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelY {
    pub draw: f32,
    pub hit: Option<f32>,
}

/// A level-line comment's y (`0x471020`): `t = (v − lo)/(hi − lo)` only for
/// lo ≤ v ≤ hi (else neither drawn nor hit); `y = trunc(top + h − 5 −
/// (h − 10)·f)` — the whole expression truncated — with f = t, or on a
/// digital lane 1 when t > 0.5 else 0. The hit test `0x470d20` and the info
/// anchor `0x471430` take the unsnapped t. An empty window (hi = lo) makes
/// t = 0/0: an analog line lands off-screen, a digital one is drawn at the
/// bottom (NaN does not exceed 0.5) and cannot be hit.
pub fn level_y(v: f64, top: f32, h: f32, lo: f64, hi: f64, digital: bool) -> Option<LevelY> {
    if !(lo <= v && v <= hi) {
        return None;
    }
    let t = (v - lo) / (hi - lo);
    let y = |f: f64| (!f.is_nan()).then(|| top + (f64::from(h) - 5.0 - f64::from(h - 10.0) * f).trunc() as f32);
    let hit = y(t);
    let draw = if digital { y(if t > 0.5 { 1.0 } else { 0.0 }) } else { hit };
    Some(LevelY { draw: draw?, hit })
}

/// A digital channel's display window (`0x428490`, kind ≠ 0): both ends
/// start at the first raw sample; a later 0 sets the low end to 0, a later
/// 1 the high end to 1 (the invert flag does not enter).
pub fn digital_window(raw: &[bool]) -> (f64, f64) {
    let Some(&first) = raw.first() else { return (0.0, 0.0) };
    let f = if first { 1.0 } else { 0.0 };
    let (mut lo, mut hi) = (f, f);
    for &b in &raw[1..] {
        if b {
            hi = 1.0;
        } else {
            lo = 0.0;
        }
    }
    (lo, hi)
}

pub fn value_y(v: f64, top: f32, h: f32, min: f64, max: f64) -> f32 {
    if max <= min || !v.is_finite() {
        // The lane's middle in whole pixels (VM: a flat channel of a 93 px
        // lane on its row 46, not 46.5).
        return top + (h / 2.0).floor();
    }
    top + h - 5.0 - round_half_up((v - min) * f64::from(h - 10.0) / (max - min)) as f32
}

/// Inverse of [`value_y`] (`0x495b40`).
pub fn y_value(y: f32, top: f32, h: f32, min: f64, max: f64) -> f64 {
    if max <= min || h <= 10.0 {
        return min;
    }
    min + f64::from(top + h - 5.0 - y) * (max - min) / f64::from(h - 10.0)
}

/// A channel's vertical window zoom (`ch+0x114` on, window `+0x128` /
/// `+0x130`, kind `+0x138` = the display mode when it was made: 0
/// instantaneous, 1 RMS, 2 first harmonic).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub lo: f64,
    pub hi: f64,
    pub kind: u8,
}

/// The window for the lane fractions `a < b` (from the lane top) of the
/// range `[lo, hi]` (`0x42afb0`); `None` unless `0 ≤ a < 1`, `0 < b ≤ 1`.
pub fn window_from_fractions(lo: f64, hi: f64, a: f64, b: f64, kind: u8) -> Option<Window> {
    ((0.0..1.0).contains(&a) && b > 0.0 && b <= 1.0 && a < b).then_some(Window {
        lo: (1.0 - b) * (hi - lo) + lo,
        hi: hi - (hi - lo) * a,
        kind,
    })
}

/// The range a channel is drawn in (`0x42b640`): its whole-record
/// `[min, max]`, or its window — adapted when the display mode changed
/// since: an instantaneous window shown as RMS/harmonic is `[0, max |·|]`,
/// an RMS/harmonic window shown instantaneous is `±1.5·hi`.
pub fn effective_range(min: f64, max: f64, window: Option<Window>, instantaneous: bool) -> (f64, f64) {
    match window {
        None => (min, max),
        Some(w) if w.kind == 0 && !instantaneous => (0.0, w.lo.abs().max(w.hi.abs())),
        Some(w) if (w.kind == 1 || w.kind == 2) && instantaneous => (-1.5 * w.hi, 1.5 * w.hi),
        Some(w) if w.kind <= 2 => (w.lo, w.hi),
        Some(_) => (min, max),
    }
}

/// One ruler tick: x, length, and whether it carries a label (every 4th).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    pub i: usize,
    pub x: f32,
    pub len: f32,
    pub labelled: bool,
}

/// Ticks every 25 px from the stats panel edge: length 10 / 5 / 7 / 5, a
/// label on every 4th (`0x464350`).
pub fn ruler_ticks(left: f32, width: f32) -> Vec<Tick> {
    let n = ((width - left).max(0.0) / 25.0) as usize;
    (0..=n)
        .map(|i| Tick {
            i,
            x: left + 25.0 * i as f32,
            len: match i % 4 {
                0 => 10.0,
                2 => 7.0,
                _ => 5.0,
            },
            labelled: i % 4 == 0,
        })
        .collect()
}

/// The sample a ruler tick stands for: `first + i·⌊1/f⌋`.
pub fn tick_sample(first: usize, i: usize, f: f64) -> usize {
    first + i * (1.0 / f).floor().max(1.0) as usize
}

/// An integer with spaces between thousands groups (`-12 345`).
pub fn group_thousands(v: i64) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    if v < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// The original's rounding (`0x411bf0` / `0x42df60`): `floor(x)` when it is
/// less than ½ away, else `ceil(x)` — halves go up, also when negative.
pub fn round_half_up(x: f64) -> i64 {
    let f = x.floor();
    if (f - x).abs() < 0.5 {
        f as i64
    } else {
        x.ceil() as i64
    }
}

/// Whole microseconds of `t` seconds (the original keeps sample times as
/// integer µs, `0x454b40`).
pub fn micros(t: f64) -> i64 {
    (t * 1e6).round() as i64
}

/// A time of day `x` seconds as `%02d:%02d:%02d.%03d` (`0x464350`,
/// `0x466170`): hours modulo 24, whole minutes and seconds, then the
/// fraction in rounded milliseconds (which can print as 1000).
pub fn time_of_day_text(x: f64) -> String {
    let whole = x.floor() as i64;
    let mins = whole.div_euclid(60);
    let s = x - (mins * 60) as f64;
    let ms = round_half_up((s - s.floor()) * 1000.0);
    format!("{:02}:{:02}:{:02}.{:03}", mins.div_euclid(60).rem_euclid(24), mins.rem_euclid(60), s.floor() as i64, ms)
}

/// A ruler label: time `t` (seconds) and sample `k`, both relative to the
/// trigger, in the ruler unit; the unit is appended only to the first label
/// (templates 386–388). `start_tod` = the record's start time of day for
/// «Абсолютное время», which has no unit.
pub fn ruler_label(unit: TimeUnit, t: f64, k: i64, first_label: bool, start_tod: f64) -> String {
    let (text, suffix) = match unit {
        TimeUnit::Micro => (group_thousands(micros(t)), " мкс"),
        TimeUnit::Milli => (group_thousands(round_half_up(micros(t) as f64 / 1000.0)), " мс"),
        TimeUnit::Sec => (if t.abs() < 0.0005 { "0.000".to_string() } else { format!("{t:.3}") }, " с"),
        TimeUnit::Absolute => (time_of_day_text(start_tod + micros(t) as f64 / 1e6), ""),
        TimeUnit::Samples => (format!("{}", k + 1), ""),
    };
    if first_label {
        format!("{text}{suffix}")
    } else {
        text
    }
}

/// A marker's label in the difference band (`0x466170`, not a difference):
/// plain numbers without unit or grouping — µs `%i`, ms rounded `%i`, s
/// `%.3f`, the absolute time of day, samples `k + 1`.
pub fn mark_label(unit: TimeUnit, t: f64, k: i64, start_tod: f64) -> String {
    match unit {
        TimeUnit::Micro => micros(t).to_string(),
        TimeUnit::Milli => round_half_up(micros(t) as f64 / 1000.0).to_string(),
        TimeUnit::Sec => format!("{:.3}", micros(t) as f64 / 1e6),
        TimeUnit::Absolute => time_of_day_text(start_tod + micros(t) as f64 / 1e6),
        TimeUnit::Samples => (k + 1).to_string(),
    }
}

/// The |Δt| label between two band markers (`0x466170` with the difference
/// flag): as [`mark_label`] but samples are the plain count and the
/// absolute mode prints the span `h:mm:ss.mmm` from rounded milliseconds
/// (hours not wrapped).
pub fn diff_label(unit: TimeUnit, dt: f64, dk: i64) -> String {
    match unit {
        TimeUnit::Absolute => {
            let ms = round_half_up(micros(dt) as f64 / 1000.0);
            let s = ms.div_euclid(1000);
            format!("{:02}:{:02}:{:02}.{:03}", s / 3600, (s / 60) % 60, s % 60, ms.rem_euclid(1000))
        }
        TimeUnit::Samples => dk.to_string(),
        u => mark_label(u, dt, 0, 0.0),
    }
}

/// The status-bar V difference in absolute mode (string 202,
/// `0x4951a3`): `%02d.%03d` = seconds within the minute (the minutes are
/// dropped, as in the original) and rounded milliseconds.
pub fn status_abs_diff(dt: f64) -> String {
    let mins = dt.floor() as i64 / 60;
    let s = dt - (mins * 60) as f64;
    format!("{:02}.{:03}", s.floor() as i64, round_half_up((s - s.floor()) * 1000.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_is_the_largest_power_of_two_showing_the_record() {
        // 1000 px for 400 samples: 1000/(400·25) = 0.1 → f = 1/16.
        assert_eq!(fit_scale(1000.0, 400), 0.0625);
        assert_eq!(fit_scale(1000.0, 10), 1.0);
        assert_eq!(initial_scale(None, 1000.0, 400), 0.125);
        assert_eq!(initial_scale(Some(0.5), 1000.0, 400), 0.5);
        // VM: on a 1552 × 955 MDI area (one lane: no vertical bar) the view measured 1212.5…1225 px at
        // its creation (388 samples open at 0.25, 392 at 0.125); the rule gives 1220.
        let w = view_width_at_creation(1552.0, 955.0, 170.0, 30.0, WinMetrics::at(96));
        assert_eq!(w, 1220.0);
        assert_eq!((initial_scale(None, w, 388), initial_scale(None, w, 392)), (0.25, 0.125));
    }

    /// A first run's default frame (230 × 140 MDI area, VM, synthetic records of 1250 Hz): 235 samples
    /// open at 0.0625, 236 at 0.03125 — the width is 184, not 189 (#69: the original's view showed the
    /// record half as wide, so a click at the same point put В1 twice as far in ours).
    #[test]
    fn a_default_frames_first_view_zoom_matches_the_originals() {
        let w = view_width_at_creation(230.0, 140.0, 670.0, 30.0, WinMetrics::at(96));
        assert_eq!(w, 184.0, "a 54 px view shows no vertical bar");
        assert_eq!((initial_scale(None, w, 235), initial_scale(None, w, 236)), (0.0625, 0.03125));
    }

    /// #175 (clone, 100 %): stepping the original's frame height one pixel at a time, the view's vertical bar
    /// appears when the view (after its horizontal bar) goes from 73 to 74 px high — with lanes far taller than
    /// the view — and is never there below; the same floor decides the first zoom's width.
    #[test]
    fn the_vertical_bar_needs_a_view_74_px_high() {
        let m = WinMetrics::at(96);
        // Two cascade steps down (a default child is placed at the MDI client less one step per three of its height).
        let area_h = |view_h: f32| view_h + m.caption + 2.0 * m.border() + 4.0 + m.scroll + 2.0 * m.step();
        let w = |view_h: f32| view_width_at_creation(300.0, area_h(view_h), 670.0, 30.0, m);
        assert_eq!(w(74.0) + m.scroll, w(73.0), "a bar from 74 px, none at 73");
        assert_eq!(w(54.0), w(73.0));
        assert_eq!(w(100.0), w(74.0), "the bar stays above the floor");
        assert_eq!(VBAR_FLOOR, 74.0);
    }

    /// #69, a frame restored maximised on 1280 × 800 (the MDI area 566 × 359, VM, the record
    /// on the command line or opened in the running program): 266 samples open at 0.125, 274 at 0.0625
    /// on the original (a width of 415.6…428.1, the view's vertical bar included); 445 px (ours before)
    /// opened 274 at 0.125.
    #[test]
    fn a_maximised_starts_first_view_zoom_matches_the_originals() {
        let w = view_width_at_creation(566.0, 359.0, 670.0, 30.0, WinMetrics::at(96));
        assert_eq!(w, 425.0, "the original's view, from its window-event log");
        assert!((415.6..428.1).contains(&w), "{w}");
        // A record that fits the view's height has no bar: 17 px wider.
        assert_eq!(view_width_at_creation(566.0, 359.0, 70.0, 30.0, WinMetrics::at(96)), w + 17.0);
        assert_eq!((initial_scale(None, w, 266), initial_scale(None, w, 274)), (0.125, 0.0625));
    }

    /// #147, #45: at 125 % / 150 % the default MDI child's cascade step, borders and the view's
    /// bars are the system DPI's metrics (GetSystemMetrics on clones), not the 100 % ones.
    #[test]
    fn window_metrics_follow_the_system_dpi() {
        let step = |d| WinMetrics::at(d).step();
        // Measured with a plain MDICLIENT and a CW_USEDEFAULT child: 26, 32, 38, 44 (168), 49.
        assert_eq!((step(96), step(120), step(144), step(168), step(192)), (26.0, 32.0, 38.0, 44.0, 49.0));
        let m = WinMetrics::at(144);
        assert_eq!((m.border(), m.caption, m.scroll), (11.0, 34.0, 26.0));
        // The measured table (clones: caption, sizing frame, padded border, scroll bar).
        for (dpi, c, f, p, s) in [(96, 23.0, 4.0, 4.0, 17.0), (120, 29.0, 4.0, 5.0, 21.0), (144, 34.0, 5.0, 6.0, 26.0), (168, 40.0, 5.0, 7.0, 30.0), (192, 45.0, 5.0, 8.0, 34.0)] {
            assert_eq!(WinMetrics::at(dpi), WinMetrics { caption: c, size_frame: f, padded: p, scroll: s }, "dpi {dpi}");
        }
        // MulDiv rounds half away from zero: 22 * 216 / 96 = 49.5, 1 * 240 / 96 = 2.5.
        assert_eq!(WinMetrics::at(216).caption, 51.0);
        assert_eq!(WinMetrics::at(240).size_frame, 6.0);
        // A custom scale (110 % = 106 dpi).
        assert_eq!(WinMetrics::at(106), WinMetrics { caption: 25.0, size_frame: 4.0, padded: 4.0, scroll: 19.0 });
    }

    /// The first zoom at 150 % (clone, the original, synthetic records of 1250 Hz): a first run's
    /// 229 × 86 MDI area gives a view 203 wide (60 samples open at 0.25, the view's 26 px height
    /// shows no bar), a maximised start's 571 × 310 one 443 after the vertical bar (146 samples
    /// open at 0.125; 469 would open them at 0.25). With the 100 % metrics ours gave 184 and 452.
    #[test]
    fn the_first_zoom_at_150_percent_matches_the_originals() {
        let m = WinMetrics::at(144);
        let first = view_width_at_creation(229.0, 86.0, 670.0, 30.0, m);
        assert_eq!(first, 203.0);
        assert_eq!(initial_scale(None, first, 60), 0.25);
        let max = view_width_at_creation(571.0, 310.0, 93.0, 75.0, m);
        assert_eq!(max, 443.0);
        assert_eq!(initial_scale(None, max, 146), 0.125);
        assert_eq!(view_width_at_creation(571.0, 310.0, 70.0, 30.0, m), 469.0, "no bar for lanes that fit");
        assert_eq!(initial_scale(None, view_width_at_creation(229.0, 86.0, 670.0, 30.0, WinMetrics::at(96)), 60), 0.125, "the 100 % metrics");
    }

    /// And at 125 % (the original's window-event log): a first run's 230 × 113 area is cut by one
    /// 32 px step, the view 176 wide before its bars; a maximised 568 × 334 one by three, 450
    /// before the vertical bar (21 px), 429 after.
    #[test]
    fn the_first_view_at_125_percent_is_cut_by_the_scaled_steps() {
        let m = WinMetrics::at(120);
        assert_eq!(view_width_at_creation(230.0, 113.0, 670.0, 30.0, m), 176.0);
        assert_eq!(view_width_at_creation(568.0, 334.0, 670.0, 30.0, m), 429.0);
        assert_eq!(view_width_at_creation(568.0, 334.0, 70.0, 30.0, m), 450.0);
        // One lane (93 px, band 75) in the 166 px the view has left: exactly at the limit, no bar
        // (clone: the original's view stayed 450 wide and opened 140 samples at 0.25); a pixel more,
        // the bar.
        assert_eq!(view_width_at_creation(568.0, 334.0, 93.0, 75.0, m), 450.0);
        assert_eq!(view_width_at_creation(568.0, 334.0, 94.0, 75.0, m), 429.0);
        assert_eq!(initial_scale(None, 450.0, 140), 0.25);
    }

    #[test]
    fn sample_x_round_trip_and_visible_count() {
        let (first, f, left) = (100, 0.25, 125.0);
        assert_eq!(x_of(104.0, first, f, left), 150.0);
        assert_eq!(sample_at(150.0, first, f, left), 104);
        assert_eq!(sample_at(156.0, first, f, left), 105, "6.25 px = half a sample: ties up");
        assert_eq!(visible_count(1125.0, 125.0, 0.25), 159);
        assert_eq!(max_first(1000, 159), 840);
        assert_eq!(max_first(100, 159), 0);
    }

    #[test]
    fn ctrl_wheel_zoom_is_anchored_on_the_vizir() {
        assert_eq!(zoom_in(0.25, 100, Some(300)), (0.5, 200));
        assert_eq!(zoom_out(0.5, 0.0625, 200, Some(300)), (0.25, 100));
        assert_eq!(zoom_in(1.0, 100, Some(300)), (1.0, 100), "never above 25 px/sample");
        assert_eq!(zoom_out(0.0625, 0.0625, 100, Some(300)), (0.0625, 100), "never below fit");
        assert_eq!(zoom_in(0.25, 100, None), (0.5, 100), "no визир: left edge stays");
    }

    #[test]
    fn lane_zoom_steps_by_1_3_within_70_500() {
        assert_eq!(lane_zoom(93.0, true), 121.0);
        assert_eq!(lane_zoom(93.0, false), 72.0);
        assert_eq!(lane_zoom(72.0, false), 55.0, "applied while above 70");
        assert_eq!(lane_zoom(55.0, false), 55.0);
        assert_eq!(lane_zoom(510.0, true), 510.0);
    }

    #[test]
    fn amplitude_maps_min_max_with_5px_margins() {
        assert_eq!(value_y(-2.0, 100.0, 93.0, -2.0, 6.0), 188.0);
        assert_eq!(value_y(6.0, 100.0, 93.0, -2.0, 6.0), 105.0);
        assert_eq!(value_y(3.0, 100.0, 93.0, 3.0, 3.0), 146.0, "flat channel: lane middle");
        assert!((y_value(188.0, 100.0, 93.0, -2.0, 6.0) + 2.0).abs() < 1e-9);
    }

    #[test]
    fn window_zoom_from_lane_fractions() {
        // Top quarter to the middle of [-10, 10]: [0, 5].
        let w = window_from_fractions(-10.0, 10.0, 0.25, 0.5, 0).unwrap();
        assert_eq!((w.lo, w.hi), (0.0, 5.0));
        assert!(window_from_fractions(-10.0, 10.0, 0.5, 0.5, 0).is_none());
        assert_eq!(effective_range(-10.0, 10.0, Some(w), true), (0.0, 5.0));
        assert_eq!(effective_range(-10.0, 10.0, Some(w), false), (0.0, 5.0), "now RMS: [0, max |·|]");
        let r = Window { lo: 1.0, hi: 4.0, kind: 1 };
        assert_eq!(effective_range(-10.0, 10.0, Some(r), true), (-6.0, 6.0), "RMS window shown instantaneous");
        assert_eq!(effective_range(-10.0, 10.0, None, true), (-10.0, 10.0));
    }

    #[test]
    fn ruler_ticks_and_labels() {
        let t = ruler_ticks(125.0, 330.0);
        let lens: Vec<f32> = t.iter().map(|t| t.len).collect();
        assert_eq!(lens, [10.0, 5.0, 7.0, 5.0, 10.0, 5.0, 7.0, 5.0, 10.0]);
        assert_eq!(t.iter().filter(|t| t.labelled).map(|t| t.x).collect::<Vec<_>>(), [125.0, 225.0, 325.0]);
        assert_eq!(tick_sample(10, 4, 0.25), 26);
        assert_eq!(ruler_label(TimeUnit::Milli, 0.0123, 0, true, 0.0), "12 мс");
        assert_eq!(ruler_label(TimeUnit::Micro, 1.2345678, 0, false, 0.0), "1 234 568");
        assert_eq!(ruler_label(TimeUnit::Sec, 0.0004, 0, true, 0.0), "0.000 с");
        assert_eq!(ruler_label(TimeUnit::Samples, 0.0, 41, true, 0.0), "42");
    }

    #[test]
    fn absolute_time_labels() {
        // Start stamp 23:59:59.9995 → the ruler wraps the hours, rounds the
        // milliseconds half up (to 1000, as the original prints) and has no unit.
        let tod = fvb_core::comtrade::time_of_day("23:59:59.9995");
        assert!((tod - 86_399.999_5).abs() < 1e-9);
        assert_eq!(ruler_label(TimeUnit::Absolute, 0.0, 0, true, tod), "23:59:59.1000");
        assert_eq!(ruler_label(TimeUnit::Absolute, 0.0105, 0, true, tod), "00:00:00.010");
        assert_eq!(fvb_core::comtrade::time_of_day("10:05"), 36_300.0);
        assert_eq!(fvb_core::comtrade::time_of_day("1:2:3.5x"), 3723.5);
        assert_eq!(time_of_day_text(3723.25), "01:02:03.250");
        // Band labels: no unit or grouping; differences in samples are counts.
        assert_eq!(mark_label(TimeUnit::Micro, 1.234567, 9, 0.0), "1234567");
        assert_eq!(mark_label(TimeUnit::Milli, -0.0025, 9, 0.0), "-2");
        assert_eq!(mark_label(TimeUnit::Samples, 0.0, 9, 0.0), "10");
        assert_eq!(diff_label(TimeUnit::Samples, 0.0, 9), "9");
        assert_eq!(diff_label(TimeUnit::Absolute, 3725.0125, 0), "01:02:05.013");
        assert_eq!(status_abs_diff(125.0125), "05.013");
        assert_eq!(round_half_up(-2.5), -2);
        assert_eq!(round_half_up(2.5), 3);
        assert_eq!(group_thousands(-1234), "-1 234");
    }
}

#[cfg(test)]
mod level_tests {
    use super::*;

    /// `0x471020`: inside the range y = trunc(top + h − 5 − (h − 10)·t)
    /// (46.5 → 46, not 88 − trunc(41.5)); outside it (e.g. off a zoomed
    /// window) the level line is not drawn.
    #[test]
    fn level_lines_stay_in_their_lane() {
        let at = |y: f32| Some(LevelY { draw: y, hit: Some(y) });
        assert_eq!(level_y(0.0, 100.0, 93.0, -1.0, 1.0, false), at(146.0));
        assert_eq!(level_y(2.0, 100.0, 93.0, -1.0, 1.0, false), None);
        assert_eq!(level_y(1.0, 100.0, 93.0, 1.0, 1.0, false), None, "0/0 off-screen");
        // A digital lane (h 20): drawn snapped, hit unsnapped; 0.5 snaps to 0.
        assert_eq!(level_y(0.3, 100.0, 20.0, 0.0, 1.0, true), Some(LevelY { draw: 115.0, hit: Some(112.0) }));
        assert_eq!(level_y(0.5, 100.0, 20.0, 0.0, 1.0, true).map(|l| l.draw), Some(115.0));
        assert_eq!(level_y(0.51, 100.0, 20.0, 0.0, 1.0, true).map(|l| l.draw), Some(105.0));
        assert_eq!(level_y(1.0, 100.0, 20.0, 1.0, 1.0, true), Some(LevelY { draw: 115.0, hit: None }), "constant: bottom, no hit");
        assert_eq!(level_y(1.5, 100.0, 20.0, 0.0, 1.0, true), None);
    }

    /// `0x428490`'s digital branch.
    #[test]
    fn digital_windows_from_raw_samples() {
        assert_eq!(digital_window(&[false, true, false]), (0.0, 1.0));
        assert_eq!(digital_window(&[true, true]), (1.0, 1.0));
        assert_eq!(digital_window(&[false, false]), (0.0, 0.0));
        assert_eq!(digital_window(&[true, false]), (0.0, 1.0));
        assert_eq!(digital_window(&[]), (0.0, 0.0));
    }
}

/// A vertical визир's x relative to the view's left edge (`0x493110`,
/// `0x49316e`–`0x4936ca`): the sample is looked up among the drawn columns
/// — offsets 1..=M at `trunc(o·k)` when k = 25·f ≥ 0.5, else the decimated
/// columns 1..P−1 — and found at `leftW + xrel`; when not found the original
/// falls back to `trunc((s − first)·k)` **without** leftW (a quirk: a визир
/// on `first` lands at 0, one just past the right edge is drawn inside the
/// plot).
pub fn vizir_x(s: usize, first: usize, n: usize, f: f64, left_w: f32, w: f32) -> f32 {
    let k = PX_PER_SAMPLE * f;
    let rem = n as i64 - first as i64 - 1;
    let cnt = (f64::from(w - left_w) / k).ceil() as i64 + 1;
    let m = rem.min(cnt);
    let d = s as i64 - first as i64;
    let found = if k >= 0.5 {
        (1..=m).contains(&d).then(|| (d as f64 * k).trunc())
    } else {
        let p = (((n as f64 - 1.0 - first as f64) * k + 0.5).floor()).min(f64::from(w - left_w - 4.0)) as i64;
        let r = if p > 0 { m as f64 / p as f64 } else { 0.0 };
        let tail = rem <= cnt;
        (1..p.max(1))
            .find(|&c| {
                let lo = (c as f64 * r).trunc() as i64;
                let hi = ((c + 1) as f64 * r).trunc() as i64 + i64::from(c + 1 == p && tail);
                d >= lo && d < hi
            })
            .map(|c| c as f64)
    };
    match found {
        Some(x) => left_w + x as f32,
        None => (d as f64 * k).trunc() as f32,
    }
}

/// One В label to draw: which визир's colour, its text, its centre x
/// (relative to the view's left edge).
#[derive(Debug, Clone, PartialEq)]
pub struct VizLabel {
    pub idx: usize,
    pub text: String,
    pub x: f32,
}

/// The В label row (`0x4938f3`–`0x49411f`): shown визиры left of the plot
/// clamp to leftW + 2 + 13·n, past the right edge to W − 5 − 13·n (n per
/// side, in order В1 В2 В3 — the step is sideways); unclamped labels on the
/// plot closer than half their widths merge with one space — «В1 В2 [В3]»,
/// else «В1 В3 [В2]», else «В2 В3» — keeping the first member's colour and
/// x; clamped labels never merge. `xs` = (shown, x); `width` measures a text.
pub fn vizir_labels(xs: [(bool, f32); 3], left_w: f32, w: f32, width: &dyn Fn(&str) -> f32) -> Vec<VizLabel> {
    let mut x: Vec<f32> = xs.iter().map(|v| v.1).collect();
    let mut clamped = [false; 3];
    let mut n = 0.0;
    for i in 0..3 {
        if xs[i].0 && x[i] <= left_w {
            x[i] = left_w + 2.0 + 13.0 * n;
            clamped[i] = true;
            n += 1.0;
        }
    }
    n = 0.0;
    for i in 0..3 {
        if xs[i].0 && !clamped[i] && x[i] >= w {
            x[i] = w - 5.0 - 13.0 * n;
            clamped[i] = true;
            n += 1.0;
        }
    }
    let mut cand: Vec<bool> = (0..3).map(|i| xs[i].0 && x[i] > left_w && !clamped[i]).collect();
    let mut text: Vec<String> = (1..=3).map(|i| format!("В{i}")).collect();
    let mut wd: Vec<f32> = text.iter().map(|t| width(t)).collect();
    let close = |a: usize, b: usize, wd: &[f32], x: &[f32]| (x[a] - x[b]).abs() < (wd[a] / 2.0).trunc() + (wd[b] / 2.0).trunc();
    let mut absorbed = false;
    if cand[0] && cand[1] && close(0, 1, &wd, &x) {
        text[0] = format!("{} {}", text[0], text[1]);
        wd[0] = width(&text[0]);
        cand[1] = false;
        absorbed = true;
        if cand[2] && close(0, 2, &wd, &x) {
            text[0] = format!("{} В3", text[0]);
            cand[2] = false;
        }
    } else if cand[0] && cand[2] && close(0, 2, &wd, &x) {
        text[0] = format!("{} В3", text[0]);
        wd[0] = width(&text[0]);
        cand[2] = false;
        absorbed = true;
        if cand[1] && close(0, 1, &wd, &x) {
            text[0] = format!("{} В2", text[0]);
            cand[1] = false;
        }
    }
    if !absorbed && cand[1] && cand[2] && close(1, 2, &wd, &x) {
        text[1] = format!("{} В3", text[1]);
        cand[2] = false;
    }
    (0..3).filter(|&i| cand[i] || clamped[i]).map(|i| VizLabel { idx: i, text: text[i].clone(), x: x[i] }).collect()
}

#[cfg(test)]
mod vizir_tests {
    use super::*;

    #[test]
    fn vizir_x_is_the_drawn_column_with_the_fallback_quirk() {
        // 25 px per sample (f = 1), first 10, leftW 150, W 1000.
        assert_eq!(vizir_x(14, 10, 480, 1.0, 150.0, 1000.0), 150.0 + 100.0);
        // On `first` itself: not searched — the fallback gives 0.
        assert_eq!(vizir_x(10, 10, 480, 1.0, 150.0, 1000.0), 0.0);
        // Past the right edge by less than leftW: drawn inside the plot.
        let beyond = 10 + 36;
        let x = vizir_x(beyond, 10, 480, 1.0, 150.0, 1000.0);
        assert_eq!(x, (36.0f64 * 25.0).trunc() as f32);
        assert!(x > 150.0 && x < 1000.0);
        // Truncation, not rounding (k = 12.5).
        assert_eq!(vizir_x(11, 10, 480, 0.5, 150.0, 1000.0), 150.0 + 12.0);
    }

    #[test]
    fn labels_clamp_sideways_and_merge_with_a_space() {
        let w = |t: &str| 8.0 * t.chars().count() as f32;
        // В1 left of the plot, В2 and В3 past the right edge.
        let l = vizir_labels([(true, 50.0), (true, 1200.0), (true, 1100.0)], 150.0, 1000.0, &w);
        assert_eq!(l.iter().map(|v| (v.idx, v.x)).collect::<Vec<_>>(), vec![(0, 152.0), (1, 995.0), (2, 982.0)]);
        // Close labels merge: «В1 В2 В3» in В1's colour at В1's x.
        let l = vizir_labels([(true, 500.0), (true, 505.0), (true, 510.0)], 150.0, 1000.0, &w);
        assert_eq!(l, vec![VizLabel { idx: 0, text: "В1 В2 В3".into(), x: 500.0 }]);
        // В1 with В3 first, then В2 within the widened label: «В1 В3 В2».
        let l = vizir_labels([(true, 500.0), (true, 525.0), (true, 504.0)], 150.0, 1000.0, &w);
        assert_eq!(l[0].text, "В1 В3 В2");
        // В1 apart: «В2 В3» in В2's colour.
        let l = vizir_labels([(true, 300.0), (true, 600.0), (true, 604.0)], 150.0, 1000.0, &w);
        assert_eq!(l.iter().map(|v| v.text.as_str()).collect::<Vec<_>>(), ["В1", "В2 В3"]);
        // Hidden визиры take no part.
        assert!(vizir_labels([(false, 500.0), (false, 0.0), (false, 0.0)], 150.0, 1000.0, &w).is_empty());
    }
}

/// The last sample the analog painter draws (`0x4295fc`, `0x429d24`):
/// offsets 0..min(⌈W/(k·ratio)⌉, len − first) from `first`, W the plot
/// width, k = 25·f, `ratio` the lane's [`lane_ratio`].
pub fn drawn_last(len: usize, first: usize, f: f64, plot_w: f32, ratio: f64) -> Option<usize> {
    let k = PX_PER_SAMPLE * f * ratio;
    let count = ((f64::from(plot_w.max(0.0)) / k).ceil() as usize).min(len.saturating_sub(first));
    (count > 0).then(|| first + count - 1)
}

/// The painters' scale of a channel with an own rate (`0x4295dc`,
/// `0x429cf1`, `0x463e2d`): the record's integer rate ÷ the channel's own
/// rate, 1 when it has none. After «Объединить» and a rate change in
/// «Параметры файла» (which sets only the record's) it is not 1: the analog
/// painters visit ⌈W/(25f·ratio)⌉ samples — a lane of a record whose rate
/// was raised stops short of the right edge — and a digital lane's base bar
/// is ratio times as long.
pub fn lane_ratio(rec_rate: f64, own_rate: f64) -> f64 {
    if own_rate == 0.0 {
        1.0
    } else {
        rec_rate / own_rate
    }
}

/// GDI draws a pen on whole pixels; egui centres a stroke on its
/// coordinate, so a 1 px line at a whole coordinate smears over two pixels
/// at half strength. These snap axis-aligned strokes to pixel centres (an
/// even width to the pixel edge), in physical pixels.
pub trait PixelLines {
    fn px_hline(&self, x: impl Into<eframe::egui::Rangef>, y: f32, s: eframe::egui::Stroke);
    fn px_vline(&self, x: f32, y: impl Into<eframe::egui::Rangef>, s: eframe::egui::Stroke);
    /// A horizontal or vertical segment snapped; any other as it is.
    fn px_segment(&self, pts: [eframe::egui::Pos2; 2], s: eframe::egui::Stroke);
    fn px_rect_stroke(&self, r: eframe::egui::Rect, rounding: f32, s: eframe::egui::Stroke);
}

fn px_snap(ppp: f32, v: f32, w: f32) -> f32 {
    if (w * ppp).round() as i64 % 2 == 1 {
        ((v * ppp).floor() + 0.5) / ppp
    } else {
        (v * ppp).round() / ppp
    }
}

impl PixelLines for eframe::egui::Painter {
    fn px_hline(&self, x: impl Into<eframe::egui::Rangef>, y: f32, s: eframe::egui::Stroke) {
        self.hline(x, px_snap(self.ctx().pixels_per_point(), y, s.width), s);
    }
    fn px_vline(&self, x: f32, y: impl Into<eframe::egui::Rangef>, s: eframe::egui::Stroke) {
        self.vline(px_snap(self.ctx().pixels_per_point(), x, s.width), y, s);
    }
    fn px_segment(&self, [a, b]: [eframe::egui::Pos2; 2], s: eframe::egui::Stroke) {
        let ppp = self.ctx().pixels_per_point();
        if a.y == b.y {
            let y = px_snap(ppp, a.y, s.width);
            self.line_segment([eframe::egui::pos2(a.x, y), eframe::egui::pos2(b.x, y)], s);
        } else if a.x == b.x {
            let x = px_snap(ppp, a.x, s.width);
            self.line_segment([eframe::egui::pos2(x, a.y), eframe::egui::pos2(x, b.y)], s);
        } else {
            self.line_segment([a, b], s);
        }
    }
    fn px_rect_stroke(&self, r: eframe::egui::Rect, rounding: f32, s: eframe::egui::Stroke) {
        let ppp = self.ctx().pixels_per_point();
        let q = |v: f32| px_snap(ppp, v, s.width);
        let r = eframe::egui::Rect::from_min_max(eframe::egui::pos2(q(r.left()), q(r.top())), eframe::egui::pos2(q(r.right()), q(r.bottom())));
        if rounding > 0.0 {
            self.rect_stroke(r, rounding, s);
            return;
        }
        // Square: four lines — a stroked rect straddles its outline and
        // comes out over two pixels — on GDI's Rectangle pixels: the left
        // and top edges in, the right and bottom ones out (seen on the VM:
        // a 16 px box is 16 pixels).
        let (rt, bt) = (r.right() - 1.0 / ppp, r.bottom() - 1.0 / ppp);
        let h = s.width / 2.0;
        self.hline(r.left() - h..=rt + h, r.top(), s);
        self.hline(r.left() - h..=rt + h, bt, s);
        self.vline(r.left(), r.top()..=bt, s);
        self.vline(rt, r.top()..=bt, s);
    }
}

/// An aliased 1 px polyline as GDI draws it (the analog painters' Polyline,
/// L9.45): the vertices on whole pixels, each segment's pixels by rounding
/// along its major axis with the end pixel left out (the next segment
/// starts there), merged into vertical runs — one rectangle per run,
/// `[x, x + 1) × [y0, y1 + 1)`. Steps are taken per column, so a long record
/// costs one or two runs per vertex.
pub fn aliased_runs(pts: &[eframe::egui::Pos2]) -> Vec<eframe::egui::Rect> {
    use eframe::egui::{pos2, Rect};
    // Round half up: ⌊(2a + b) / 2b⌋, b > 0.
    let rhu = |a: i64, b: i64| (2 * a + b).div_euclid(2 * b);
    let mut runs = Vec::new();
    let mut cur: Option<(i64, i64, i64)> = None;
    let mut span = |x: i64, a: i64, b: i64, runs: &mut Vec<Rect>| {
        let (a, b) = (a.min(b), a.max(b));
        match &mut cur {
            Some((cx, lo, hi)) if *cx == x && a <= *hi + 1 && b >= *lo - 1 => {
                *lo = (*lo).min(a);
                *hi = (*hi).max(b);
            }
            c => {
                if let Some((cx, lo, hi)) = c.take() {
                    runs.push(Rect::from_min_max(pos2(cx as f32, lo as f32), pos2(cx as f32 + 1.0, hi as f32 + 1.0)));
                }
                *c = Some((x, a, b));
            }
        }
    };
    for w in pts.windows(2) {
        let (x0, y0) = (w[0].x.round() as i64, w[0].y.round() as i64);
        let (x1, y1) = (w[1].x.round() as i64, w[1].y.round() as i64);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let n = dx.abs().max(dy.abs());
        if n == 0 {
            continue;
        }
        let y_at = |s: i64| y0 + dy.signum() * rhu(dy.abs() * s, n);
        if dy.abs() > dx.abs() {
            // Y-major: in each column a contiguous range of steps.
            let adx = dx.abs();
            for k in 0..=adx {
                let lo = if k == 0 { 0 } else { ((2 * k - 1) * n).div_euclid(2 * adx) + i64::from(((2 * k - 1) * n).rem_euclid(2 * adx) != 0) };
                let hi = if adx == 0 { n - 1 } else { (((2 * k + 1) * n).div_euclid(2 * adx) + i64::from(((2 * k + 1) * n).rem_euclid(2 * adx) != 0) - 1).min(n - 1) };
                if lo <= hi {
                    span(x0 + dx.signum() * k, y_at(lo), y_at(hi), &mut runs);
                }
            }
        } else {
            for s in 0..n {
                span(x0 + dx.signum() * s, y_at(s), y_at(s), &mut runs);
            }
        }
    }
    span(i64::MIN, 0, 0, &mut runs);
    runs
}

/// What the digital painter draws (`0x42a0f0`), x relative to leftW: the
/// filled runs (left, right) and, zoomed out, the transition ticks.
#[derive(Debug, Default, PartialEq)]
pub struct DigitalShapes {
    pub runs: Vec<(f32, f32)>,
    pub ticks: Vec<f32>,
}

/// `on(i)` is sample i's state (false past the end). k = 25·f ≥ 0.5: one
/// rectangle per run at truncated x, a run ending at an off sample reaching
/// trunc(i·k) + 1, one at the data end + 4; k < 0.5: 1-px columns
/// j = 1..P−1 (P = min(rhu(M·k), W − 4), r = count / P) OR-ed into runs, a
/// red tick where a column mixes on and off. A run that only starts at the
/// last visible offset is not drawn (as in the original).
pub fn digital_shapes(on: &dyn Fn(usize) -> bool, len: usize, first: usize, f: f64, plot_w: f32) -> DigitalShapes {
    let mut out = DigitalShapes::default();
    if len == 0 || first >= len {
        return out;
    }
    let k = PX_PER_SAMPLE * f;
    let m = len as i64 - first as i64 - 1;
    let n0 = (f64::from(plot_w) / k).ceil() as i64 + 1;
    let count = n0.min(m).max(0);
    let tx = |i: i64| (i as f64 * k).trunc() as f32;
    if k >= 0.5 {
        let (mut state, mut start) = (on(first), 0i64);
        for i in 1..=count {
            let idx = first + i as usize;
            let at_end = idx + 1 >= len;
            let v = on(idx);
            if !state && v {
                start = i;
                if at_end {
                    out.runs.push((tx(i), tx(i) + 4.0));
                }
            } else if state && (!v || i == count) {
                out.runs.push((tx(start), tx(i) + if at_end { 4.0 } else { 1.0 }));
            }
            state = v;
        }
        return out;
    }
    let p = ((m as f64 * k + 0.5).floor() as i64).min((plot_w - 4.0) as i64);
    if p <= 1 {
        return out;
    }
    let r = count as f64 / p as f64;
    let tail = m <= n0;
    let column = |j: i64, extend: bool| {
        let lo = (j as f64 * r).trunc() as i64;
        let hi = ((j + 1) as f64 * r).trunc() as i64 + i64::from(extend && j == p - 1 && tail);
        let (mut any, mut all) = (false, true);
        for o in lo.max(0)..hi {
            let idx = first + o as usize;
            if idx >= len {
                continue;
            }
            let v = on(idx);
            any |= v;
            all &= v;
        }
        (any, all)
    };
    let (mut state, mut start) = (on(first), 0i64);
    for j in 1..p {
        let flag = j == p - 1 && tail;
        let (any, _) = column(j, true);
        if !state && any {
            start = j;
            if flag {
                out.runs.push((j as f32, j as f32 + 4.0));
            }
        } else if state && (!any || j == p - 1) {
            out.runs.push((start as f32, j as f32 + if flag { 4.0 } else { 0.0 }));
        }
        state = any;
    }
    for j in 1..p {
        let (any, all) = column(j, false);
        if any && !all {
            out.ticks.push(j as f32);
        }
    }
    out
}

/// Time of record sample `i` in µs from a times array in seconds
/// (`0x454b40`): past the end the last time plus ⌊(i − len + 1)·10⁶/rate⌉
/// (half up).
pub fn time_us(t: &[f64], rate: f64, i: usize) -> f64 {
    extrapolated_us(t, i, |n| round_half_up(n * 1e6 / rate) as f64)
}

/// A channel's own sample time (`0x428b40` with own times): the same, but
/// the step past the end, (i − len + 1)·10⁶/rate, is not rounded.
pub fn own_time_us(t: &[f64], rate: f64, i: usize) -> f64 {
    extrapolated_us(t, i, |n| n * 1e6 / rate)
}

fn extrapolated_us(t: &[f64], i: usize, step: impl Fn(f64) -> f64) -> f64 {
    let t0 = t.first().copied().unwrap_or(0.0);
    match t.get(i) {
        Some(&v) => ((v - t0) * 1e6).round(),
        None => {
            let last = t.last().map_or(0.0, |&v| ((v - t0) * 1e6).round());
            // The index is a 32-bit unsigned count (−1 wraps).
            let n = (i as u32).wrapping_sub(t.len() as u32).wrapping_add(1);
            last + step(f64::from(n))
        }
    }
}

/// A pasted, not yet merged (bit 14) channel against the record: its own
/// times and rate, the record's, and the merge offset (`doc+0x6d8`).
pub struct PastedTimes<'a> {
    pub own: &'a [f64],
    pub own_rate: f64,
    pub rec: &'a [f64],
    pub rec_rate: f64,
    pub offset: f64,
}

impl PastedTimes<'_> {
    /// `0x428b40`: the channel's own time (the record's rate when its own
    /// is 0); the record's time (`0x454b40`) without own times.
    fn ct(&self, i: usize) -> f64 {
        if self.own.is_empty() {
            return time_us(self.rec, self.rec_rate, i);
        }
        let rate = if self.own_rate == 0.0 { self.rec_rate } else { self.own_rate };
        own_time_us(self.own, rate, i)
    }

    /// The own samples around record sample `k` as the stats panel scans
    /// them (`0x461a24`–`0x461b06`): t = T(k) − offset·10⁶; i = the first own
    /// index **from 1** with ct(i) ≥ t (past the values the times run on at
    /// the own rate), j = i − 1, and frac = (t − ct(j)) / (ct(i) − ct(j)) —
    /// negative before the paste, so the line through samples 0 and 1
    /// extends backwards.
    pub fn bracket(&self, k: usize, n: usize) -> (usize, usize, f64) {
        self.scan(k, n, 1)
    }

    /// The scan from own index `from`; with `from` = 0 and the first own
    /// time already ≥ t, j = i − 1 wraps (a 32-bit count): v(j) reads 0 and
    /// ct(j) lies 2³² − len own periods on, so frac ≈ 1.
    fn scan(&self, k: usize, n: usize, from: usize) -> (usize, usize, f64) {
        let t = time_us(self.rec, self.rec_rate, k) - self.offset * 1e6;
        let last = n.max(from + 1) - 1;
        let mut i = if t <= self.ct(last) {
            (from..=last).find(|&i| self.ct(i) >= t).unwrap_or(last)
        } else {
            // Past the values: jump by the own period, then settle.
            let rate = if self.own_rate == 0.0 { self.rec_rate } else { self.own_rate };
            let step = 1e6 / rate;
            last + (((t - self.ct(last)) / step).floor().max(0.0) as usize)
        };
        while i > from && self.ct(i - 1) >= t {
            i -= 1;
        }
        while self.ct(i) < t {
            i += 1;
        }
        let j = if i == 0 { u32::MAX as usize } else { i - 1 };
        let (cj, ci) = (self.ct(j), self.ct(i));
        (j, i, (t - cj) / (ci - cj))
    }

    /// The vector diagram's phasor of a pasted channel at record sample `k`
    /// (`0x490120`): its own phasor cache scanned by own time **from index
    /// 0**, v(j) + (v(i) − v(j))·frac, an entry past the cache or with a NaN
    /// part reading 0.
    pub fn phasor_at(&self, k: usize, cache: &[num_complex::Complex64]) -> num_complex::Complex64 {
        let v = |m: usize| cache.get(m).copied().filter(|z| !z.re.is_nan() && !z.im.is_nan()).unwrap_or_default();
        let (j, i, frac) = self.scan(k, cache.len(), 0);
        v(j) + (v(i) - v(j)) * frac
    }

    /// The value at record sample `k` the stats panel reads out (`0x461a24`):
    /// v(j) + (v(i) − v(j))·frac over [`Self::bracket`], a sample past the
    /// values reading 0 — so past the last one it falls to 0 within one own
    /// period, then stays 0.
    pub fn value_at(&self, k: usize, n: usize, v: &dyn Fn(usize) -> f64) -> f64 {
        if n == 0 {
            return 0.0;
        }
        let (j, i, frac) = self.bracket(k, n);
        let val = |m: usize| if m < n { v(m) } else { 0.0 };
        val(j) + (val(i) - val(j)) * frac
    }

    /// A per-index statistic (СКЗ `0x4272a0`, A1 `0x426240`) of a pasted
    /// channel at record sample `k` (`0x4623d7`–`0x46244d`,
    /// `0x462624`–`0x46269a`): R(j) + (R(j) − R(i))·frac — the
    /// interpolation mirrored, as the original computes it.
    pub fn stat_at(&self, k: usize, n: usize, r: &dyn Fn(usize) -> f64) -> f64 {
        if n == 0 {
            return 0.0;
        }
        let (j, i, frac) = self.bracket(k, n);
        let rj = r(j);
        rj + (rj - r(i)) * frac
    }

    /// The window of a pasted channel's own statistics (`0x427315`):
    /// round(own rate / 50) — 50 Hz fixed (`0x4d1718`) — or `rec_n` without
    /// an own rate.
    pub fn own_period(&self, rec_n: usize) -> usize {
        if self.own_rate == 0.0 {
            rec_n
        } else {
            (self.own_rate / 50.0).round().max(1.0) as usize
        }
    }

    /// R/r (`0x4295dc`, `0x42ab37`): 1 when the channel has no rate.
    pub fn ratio(&self) -> f64 {
        if self.own_rate == 0.0 {
            1.0
        } else {
            self.rec_rate / self.own_rate
        }
    }

    /// The first drawn sample (`0x4293a0`): d = offset − (T(s) − T(0)); when
    /// d < 0 the first one whose own time reaches −d, else 0.
    pub fn first(&self, s: usize) -> usize {
        let d = self.offset * 1e6 - time_us(self.rec, self.rec_rate, s);
        if d >= 0.0 {
            return 0;
        }
        (0..).find(|&i| self.ct(i) >= -d).unwrap_or(0)
    }

    /// The x offsets from the plot left of samples `from..to`
    /// (`0x42ac42`, `0x429930`): x_t = own time + offset; a forward scan
    /// keeps T(j) ≤ x_t ≤ T(j+1) (skipped while x_t < T(j));
    /// ((x_t − T(j)) / (T(j+1) − T(j)) + j − s)·25f rounded half up.
    pub fn xs(&self, from: usize, to: usize, s: usize, f: f64) -> Vec<f32> {
        let t = |j: usize| time_us(self.rec, self.rec_rate, j);
        let (mut j, mut tj, mut tj1) = (0usize, t(0), t(1));
        (from..to)
            .map(|i| {
                let x = self.ct(i) + self.offset * 1e6;
                if x >= tj {
                    while tj1 < x {
                        j += 1;
                        (tj, tj1) = (tj1, t(j + 1));
                    }
                }
                let px = ((x - tj) / (tj1 - tj) + j as f64 - s as f64) * PX_PER_SAMPLE * f;
                round_half_up(px) as f32
            })
            .collect()
    }

    /// Samples visited by the analog painter: k..min(N, ⌈W/(25f·R/r)⌉ + k)
    /// (`0x42960f`).
    pub fn analog_end(&self, n: usize, k: usize, f: f64, plot_w: f32) -> usize {
        let c = (f64::from(plot_w) / (PX_PER_SAMPLE * f * self.ratio())).ceil() as usize + k;
        c.min(n)
    }

    /// Samples visited by the digital painter: count = min(⌈W/(25f·R/r)⌉ +
    /// 5, N − s) — the record index taken from the channel's count, an
    /// unsigned compare (N < s: no clamp) — then k..k + ⌊11·count/10⌋
    /// (`0x42ab69`).
    pub fn digital_end(&self, n: usize, s: usize, k: usize, f: f64, plot_w: f32) -> usize {
        let c = (f64::from(plot_w) / (PX_PER_SAMPLE * f * self.ratio())).ceil() as usize + 5;
        let c = match n.checked_sub(s) {
            Some(m) => c.min(m),
            None => c,
        };
        k + 11 * c / 10
    }

    /// The offset in px the base bar of a pasted digital lane moves by
    /// (`0x4653b0`): R·offset·25f rounded half up.
    pub fn offset_px(&self, f: f64) -> f32 {
        round_half_up(self.rec_rate * self.offset * PX_PER_SAMPLE * f) as f32
    }
}

/// A pasted digital lane's rectangles (`0x42ac42`…`0x42ae27`), x offsets
/// from the plot left: a run opens at an on sample (a lone last sample
/// gives x..x+4); it closes at the next off sample, or at the last
/// visited one, reaching that sample's x (+4 at the channel's last
/// sample). Past the values a sample is off; NaN is on.
pub fn pasted_digital_runs(on: &dyn Fn(usize) -> bool, n: usize, from: usize, xs: &[f32]) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let (mut open, mut start) = (false, 0.0f32);
    let end = from + xs.len();
    for (o, &x) in xs.iter().enumerate() {
        let i = from + o;
        let v = on(i);
        let last = i + 1 >= n;
        if !open {
            if v {
                open = true;
                start = x;
                if last {
                    out.push((x, x + 4.0));
                }
            }
        } else if !v || i + 1 == end {
            out.push((start, x + if last { 4.0 } else { 0.0 }));
            open = v;
        }
    }
    out
}

/// The right end of a digital lane's base bar (`0x463e9a`), from the plot
/// left: trunc(25·N·f·R/r) — the channel's sample count, whatever the
/// scroll position — plus the paste offset for a bit-14 channel.
pub fn base_bar_len(n: usize, f: f64, ratio: f64) -> f32 {
    (PX_PER_SAMPLE * n as f64 * f * ratio).trunc() as f32
}

#[cfg(test)]
mod draw_tests {
    use super::*;
    use eframe::egui::{Pos2, Rect};

    #[test]
    fn pasted_value_at_the_cursor() {
        // Record every 0.5 ms; a paste of 1, 2, 3, 4, 5 at 1 kHz from 1 ms.
        let rec: Vec<f64> = (0..20).map(|i| f64::from(i) / 2000.0).collect();
        let own: Vec<f64> = (0..5).map(|i| f64::from(i) / 1000.0).collect();
        let vals = [1.0, 2.0, 3.0, 4.0, 5.0];
        let p = PastedTimes { own: &own, own_rate: 1000.0, rec: &rec, rec_rate: 2000.0, offset: 0.001 };
        let at = |k: usize| p.value_at(k, 5, &|m| vals[m]);
        assert_eq!(at(2), 1.0, "on the first sample");
        assert_eq!(at(4), 2.0);
        assert_eq!(at(5), 2.5, "linear between own samples");
        // Before the paste: the line through own samples 0 and 1 extended
        // backwards (the scan starts at own index 1).
        assert_eq!((at(0), at(1)), (0.0, 0.5));
        // СКЗ / A1: R(j) + (R(j) − R(i))·frac, the mirrored interpolation.
        let r = |m: usize| [10.0, 20.0, 30.0, 40.0, 50.0][m.min(4)];
        // k = 5: t = 1500 µs between own samples 1 and 2.
        assert_eq!(p.stat_at(5, 5, &r), 20.0 + (20.0 - 30.0) * 0.5);
        assert_eq!(p.own_period(40), 20, "round(own rate / 50)");
        // Past the last own sample (5 ms): falls to 0 within one period.
        assert_eq!(at(10), 5.0);
        assert_eq!(at(11), 2.5);
        assert_eq!(at(12), 0.0);
        assert_eq!(at(19), 0.0);
        // The vector phasor (`0x490120`) scans from own index 0: before the
        // paste j wraps, v(j) = 0 and frac ≈ 1 — about the first entry.
        let cache: Vec<num_complex::Complex64> = vals.iter().map(|&v| num_complex::Complex64::new(v, -v)).collect();
        let ph = |k: usize| p.phasor_at(k, &cache);
        assert_eq!(ph(2), cache[0]);
        assert_eq!(ph(5), num_complex::Complex64::new(2.5, -2.5));
        assert!((ph(0) - cache[0]).norm() < 1e-6 && ph(0).re > 1.0, "{:?}", ph(0));
        assert_eq!((ph(10), ph(11), ph(12)), (cache[4], cache[4] / 2.0, num_complex::Complex64::default()));
        let mut nan = cache.clone();
        nan[1].im = f64::NAN;
        assert_eq!(p.phasor_at(4, &nan), num_complex::Complex64::default(), "a NaN part reads 0");
        // The own step past the end is not rounded (`0x428b40`).
        assert_eq!(own_time_us(&own, 3000.0, 5), 4000.0 + 1e6 / 3000.0);
        assert_eq!(time_us(&own, 3000.0, 5), 4333.0);
    }

    #[test]
    fn pasted_times_and_positions() {
        // Record at 1 kHz (1000 µs), a paste at 2 kHz shifted by 2.5 ms.
        let rec: Vec<f64> = (0..10).map(|i| f64::from(i) / 1000.0).collect();
        let own: Vec<f64> = (0..8).map(|i| f64::from(i) / 2000.0).collect();
        assert_eq!(time_us(&rec, 1000.0, 9), 9000.0);
        assert_eq!(time_us(&rec, 1000.0, 11), 11000.0, "extrapolated");
        let p = PastedTimes { own: &own, own_rate: 2000.0, rec: &rec, rec_rate: 1000.0, offset: 0.0025 };
        assert_eq!(p.ratio(), 0.5);
        assert_eq!(p.first(0), 0, "the paste starts after the view's left edge");
        // View from sample 4 (4 ms): the first pasted sample at ≥ 1.5 ms own.
        assert_eq!(p.first(4), 3);
        // f = 1: 25 px per record sample. Own 0 → 2.5 ms → 62.5 px → 63.
        assert_eq!(p.xs(0, 3, 0, 1.0), [63.0, 75.0, 88.0]);
        assert_eq!(p.xs(3, 4, 4, 1.0), [0.0]);
        // ⌈100 / (25·1·0.5)⌉ = 8 → min(8, 8 + 0); digital ⌊11·min(13, 8)/10⌋.
        assert_eq!(p.analog_end(8, 0, 1.0, 100.0), 8);
        assert_eq!(p.digital_end(8, 0, 0, 1.0, 100.0), 8);
        assert_eq!(p.digital_end(8, 10, 0, 1.0, 100.0), 14, "N < s: no clamp");
        assert_eq!(p.offset_px(1.0), 63.0);
        assert_eq!(base_bar_len(8, 1.0, 0.5), 100.0);
    }

    #[test]
    fn pasted_digital_rectangles() {
        let v = [false, true, true, false, true, true];
        let on = |i: usize| v.get(i).copied().unwrap_or(false);
        let xs = [0.0, 10.0, 20.0, 30.0, 40.0, 50.0];
        // A run closes at the off sample's x; the last sample of the channel adds 4.
        assert_eq!(pasted_digital_runs(&on, 6, 0, &xs), [(10.0, 30.0), (40.0, 54.0)]);
        // The last visited sample closes an open run.
        assert_eq!(pasted_digital_runs(&on, 6, 0, &xs[..3]), [(10.0, 20.0)]);
    }

    #[test]
    fn analog_draws_ceil_w_over_k_samples() {
        // 25 px per sample, 100 px plot: offsets 0..3 (no point at R).
        assert_eq!(drawn_last(480, 10, 1.0, 100.0, 1.0), Some(13));
        assert_eq!(drawn_last(12, 10, 1.0, 100.0, 1.0), Some(11), "the data end");
        // A lane whose own rate is half the record's: twice as few samples visited.
        assert_eq!(drawn_last(480, 10, 1.0, 100.0, 2.0), Some(11));
        assert_eq!(drawn_last(480, 10, 1.0, 100.0, 0.5), Some(17));
        assert_eq!((lane_ratio(2450.0, 0.0), lane_ratio(2450.0, 1225.0)), (1.0, 2.0));
        assert_eq!(drawn_last(10, 10, 1.0, 100.0, 1.0), None);
    }

    #[test]
    fn aliased_polyline_runs_as_gdi_draws_them() {
        let p = |v: &[(f32, f32)]| v.iter().map(|&(x, y)| Pos2::new(x, y)).collect::<Vec<_>>();
        let r = |runs: Vec<Rect>| runs.iter().map(|r| (r.left() as i32, r.top() as i32, r.bottom() as i32)).collect::<Vec<_>>();
        // A horizontal segment: one pixel per column, the end left out.
        assert_eq!(r(aliased_runs(&p(&[(0.0, 5.0), (3.0, 5.0)]))), [(0, 5, 6), (1, 5, 6), (2, 5, 6)]);
        // Vertical: one run, the end left out; the next segment goes on.
        assert_eq!(r(aliased_runs(&p(&[(2.0, 1.0), (2.0, 5.0), (2.0, 9.0)]))), [(2, 1, 9)]);
        // A steep step to the next column: half in each.
        assert_eq!(r(aliased_runs(&p(&[(0.0, 0.0), (1.0, 4.0)]))), [(0, 0, 2), (1, 2, 4)]);
        // Many samples in one column: the column's envelope as one run.
        let dense: Vec<Pos2> = (0..100).map(|i| Pos2::new(10.0 + i as f32 * 0.004, if i % 2 == 0 { 0.0 } else { 20.0 })).collect();
        assert_eq!(r(aliased_runs(&dense)), [(10, 0, 21)]);
    }

    #[test]
    fn digital_runs_and_columns() {
        let s = [false, true, true, false, false, true, true, true];
        let on = |i: usize| s.get(i).copied().unwrap_or(false);
        // k = 25: run 1..2 ends at the off sample 3 → trunc(3·25) + 1;
        // run 5..7 reaches the data end → +4.
        let d = digital_shapes(&on, s.len(), 0, 1.0, 1000.0);
        assert_eq!(d.runs, vec![(25.0, 76.0), (125.0, 179.0)]);
        assert!(d.ticks.is_empty());
        // Zoomed out (k = 0.25): 1-px columns, mixed columns get a tick.
        let big: Vec<bool> = (0..4000).map(|i| (i / 3) % 2 == 0).collect();
        let on2 = |i: usize| big.get(i).copied().unwrap_or(false);
        let d = digital_shapes(&on2, big.len(), 0, 0.01, 2000.0);
        assert!(!d.ticks.is_empty());
        assert!(d.runs.iter().all(|r| r.1 >= r.0));
        // k = 0.25, 401 samples: P = 100 columns of r = 4 samples; on at
        // 10..=21 → columns 2..5 OR to one run ending at column 6, columns
        // 2 (8..11) and 5 (20..23) mix on and off.
        let on3 = |i: usize| (10..=21).contains(&i) && i < 401;
        let d = digital_shapes(&on3, 401, 0, 0.01, 2000.0);
        assert_eq!(d.runs, vec![(2.0, 6.0)]);
        assert_eq!(d.ticks, vec![2.0, 5.0]);
    }
}
