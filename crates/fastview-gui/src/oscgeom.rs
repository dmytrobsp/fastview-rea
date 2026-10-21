//! Geometry of the oscillogram view, as the original computes it (ledger
//! L9.4, `notes/UI_BEHAVIOUR_SPEC.md` §3): a sample-indexed horizontal scale
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
pub fn value_y(v: f64, top: f32, h: f32, min: f64, max: f64) -> f32 {
    if max <= min || !v.is_finite() {
        return top + h / 2.0;
    }
    top + h - 5.0 - ((v - min) * f64::from(h - 10.0) / (max - min)).round() as f32
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
fn micros(t: f64) -> i64 {
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
        assert_eq!(value_y(3.0, 100.0, 93.0, 3.0, 3.0), 146.5, "flat channel: lane middle");
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
