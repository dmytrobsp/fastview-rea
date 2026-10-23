//! Merging oscillograms («Объединение», ledger L9.15,
//! `notes/UI_BEHAVIOUR_SPEC.md` §7.8): synchronising a pasted record against
//! the document (`0x47bb90`: by start time `0x47ce10`, by the fault component
//! `0x47bc00`, by the pre-fault mode `0x47c440`) and resampling both onto one
//! grid («Объединить», `0x47d990`).

use crate::comtrade::{AnalogChannel, DigitalChannel, Oscillogram};
use crate::signal::{fault_at, phasor_at};

/// The outcome of a synchronisation, shown in the merge bar (219 is an
/// error box in the original).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMsg {
    /// 3456
    Done,
    /// 219
    NoOverlap,
    /// 3260
    CursorBefore3Periods,
    /// 3259
    CursorBefore15Periods,
    /// 3162
    CursorOutside,
    /// 3507
    NoTransition,
    /// 3305
    Unstable,
}

impl SyncMsg {
    pub fn text(self) -> &'static str {
        match self {
            SyncMsg::Done => "Синхронизация произведена успешно",
            SyncMsg::NoOverlap => "Осциллограммы не пересекаются по времени",
            SyncMsg::CursorBefore3Periods => "Красный визир должен отстоять от начала осциллограммы на расстояние более 3 периодов",
            SyncMsg::CursorBefore15Periods => "Красный визир должен отстоять от начала осциллограммы на расстояние более 1.5 периода",
            SyncMsg::CursorOutside => "Выход визира за пределы осциллограммы",
            SyncMsg::NoTransition => "Установите визир на участок перехода от нормального режима в аварийный",
            SyncMsg::Unstable => "Нестационарный участок. Установите красный визир на участок нормального режима",
        }
    }
}

/// One channel taking part in a synchronisation: its sample times (seconds
/// from its own record start), its values (flagged, `0x426020`) and its
/// samples per period.
#[derive(Debug, Clone, Copy)]
pub struct SyncChan<'a> {
    pub times: &'a [f64],
    pub x: &'a [f64],
    pub n: usize,
}

/// Method 0 (`0x47ce10`): the offset of the pasted record is the difference
/// of the two start times; the records must overlap.
pub fn sync_by_start(base_start: f64, base_len: f64, pasted_start: f64, pasted_len: f64) -> Result<f64, SyncMsg> {
    let offset = pasted_start - base_start;
    if offset > base_len || offset + pasted_len < 0.0 {
        Err(SyncMsg::NoOverlap)
    } else {
        Ok(offset)
    }
}

/// The normalised second difference of `0x47bc00`:
/// `|x[k] − 2x[k−N] + x[k−2N]| / A(k−1)`, `A` the fault component.
fn jump(c: &SyncChan, k: usize) -> f64 {
    let a = fault_at(c.x, k - 1, c.n);
    if a == 0.0 || k < 2 * c.n + 1 {
        return 0.0;
    }
    ((c.x[k] - 2.0 * c.x[k - c.n] + c.x[k - 2 * c.n]) / a).abs()
}

/// The peak of the jump in the window around the cursor and whether it
/// stands out (≥ 4 × the window mean).
fn peak(c: &SyncChan, cur: usize) -> (usize, bool) {
    let last = c.x.len() - 1;
    let lo = if cur > 4 * c.n { cur - c.n } else { 3 * c.n + 1 };
    let hi = if cur + c.n < last { cur + c.n } else { last };
    let d: Vec<f64> = (lo..=hi).map(|k| if k >= 3 * c.n + 2 && k < last { jump(c, k) } else { 0.0 }).collect();
    let (mut best, mut at) = (d[0], lo);
    for (j, &v) in d.iter().enumerate().take(d.len() - 1).skip(1) {
        if v > best {
            best = v;
            at = lo + j;
        }
    }
    let mean = d.iter().sum::<f64>() / d.len() as f64;
    (at, 4.0 * mean <= best)
}

/// Method 1 (`0x47bc00`): align the transitions into the fault. `cur1` /
/// `cur2` are the cursor's sample in each channel; the offset is set to
/// `t1(peak1) − t2(peak2)` even when the peaks do not stand out.
pub fn sync_by_fault(c1: &SyncChan, c2: &SyncChan, cur1: usize, cur2: usize) -> Result<(f64, SyncMsg), SyncMsg> {
    if cur1 < 3 * c1.n + 1 || cur2 < 3 * c2.n + 1 {
        return Err(SyncMsg::CursorBefore3Periods);
    }
    if cur1 >= c1.x.len() || cur2 >= c2.x.len() {
        return Err(SyncMsg::CursorOutside);
    }
    let ((p1, ok1), (p2, ok2)) = (peak(c1, cur1), peak(c2, cur2));
    let msg = if ok1 && ok2 { SyncMsg::Done } else { SyncMsg::NoTransition };
    Ok((c1.times[p1] - c2.times[p2], msg))
}

/// A synchronised channel's samples per period (`0x47bd43`, `0x47c573`):
/// its own rate ÷ 50 (a fixed 50, rounded) when it has one — a pasted
/// channel —, else the record's integer rate divided as an integer by the
/// truncated network frequency.
pub fn period_samples(own_rate: f64, record_rate: f64, f_net: f64) -> usize {
    if own_rate != 0.0 {
        (own_rate / 50.0).round() as usize
    } else {
        (record_rate as u32).checked_div(f_net.trunc() as u32).unwrap_or(0) as usize
    }
}

/// The cursor's sample in the pasted channel (`0x47bd24`, `0x47c555`):
/// `round(r₂·(c/R − offset))`, `c` the cursor's record sample, `R` the
/// record rate, `r₂` the channel's own rate, the offset in seconds.
pub fn cursor_in(c: usize, record_rate: f64, own_rate: f64, offset: f64) -> i64 {
    let r2 = if own_rate != 0.0 { own_rate } else { record_rate };
    let v = r2 * c as f64 / record_rate - r2 * offset;
    (v + 0.5).floor() as i64
}

/// The population standard deviation (`0x4776f0`).
fn std_dev(v: &[f64]) -> f64 {
    let m = v.iter().sum::<f64>() / v.len().max(1) as f64;
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt()
}

/// The median (`0x477810`): sorted, the mean of the middle one or two.
fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let m = (s.len().saturating_sub(1)) as f64 / 2.0;
    let (lo, hi) = (m.floor() as usize, m.ceil() as usize);
    s[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64
}

/// Method 2 «По доаварийному режиму» (`0x47c440`): align the
/// fundamental's phase in the pre-fault state around the cursor. Coarse
/// Δ = t₁(c) − t₂(c₂); in [c − N₁, c + N₁) (to the last sample near the
/// end) the one-period window whose amplitude product |P₁|·|P₂| (P₂ at the
/// first sample with t₂ + Δ ≥ t₁) has the least non-zero spread
/// (`0x4789a0`); over it the phase differences to P₂ interpolated at
/// t₁ − Δ, wrapped once into ±180°; the offset is Δ − median°/(f·360), the
/// population spread above 1° reported as unstable (the offset set either
/// way).
pub fn sync_by_prefault(c1: &SyncChan, c2: &SyncChan, cur1: usize, cur2: i64, f_net: f64) -> Result<(f64, SyncMsg), SyncMsg> {
    let (n1, n2) = (c1.n, c2.n);
    if 1.5 * n1 as f64 + 1.0 > cur1 as f64 || 1.5 * n2 as f64 + 1.0 > cur2 as f64 {
        return Err(SyncMsg::CursorBefore15Periods);
    }
    let (len1, len2) = (c1.x.len().min(c1.times.len()), c2.x.len().min(c2.times.len()));
    if n1 == 0 || len2 < 2 || cur1 + 1 > len1 || cur2 as usize + 1 > len2 {
        return Err(SyncMsg::CursorOutside);
    }
    let cur2 = cur2 as usize;
    let p = |c: &SyncChan, k: usize| {
        let z = phasor_at(c.x, k, c.n);
        if z.re.is_nan() || z.im.is_nan() {
            Default::default()
        } else {
            z
        }
    };
    let delta = c1.times[cur1] - c2.times[cur2];
    let lo = cur1 - n1;
    let hi = if cur1 + n1 < len1 - 1 { cur1 + n1 } else { len1 - 1 };
    // The first channel-2 sample at or after each channel-1 time, scanned
    // forward, at most the last but one.
    let mut j = 1usize;
    let mut after = |t1: f64| {
        while j < len2 - 2 && c2.times[j] + delta < t1 {
            j += 1;
        }
        j
    };
    let js: Vec<usize> = (lo..hi).map(|i| after(c1.times[i])).collect();
    let prod: Vec<f64> = (lo..hi).zip(&js).map(|(i, &j)| p(c1, i).norm() * p(c2, j).norm()).collect();
    let mut best = (f64::MAX, 0usize);
    for s in 0..prod.len().saturating_sub(n1 - 1) {
        let sd = std_dev(&prod[s..s + n1]);
        if sd > 0.0 && sd < best.0 {
            best = (sd, s);
        }
    }
    let s = best.1;
    let diffs: Vec<f64> = (s..(s + n1).min(js.len()))
        .map(|w| {
            let (k, j) = (lo + w, js[w].max(1));
            let t = c1.times[k] - delta;
            let (ta, tb) = (c2.times[j - 1], c2.times[j]);
            let (a, b) = (p(c2, j - 1), p(c2, j));
            let z2 = if tb > ta { a + (b - a) * ((t - ta) / (tb - ta)) } else { b };
            let mut d = (p(c1, k).arg() - z2.arg()).to_degrees();
            if d < -180.0 {
                d += 360.0;
            } else if d > 180.0 {
                d -= 360.0;
            }
            d
        })
        .collect();
    if diffs.is_empty() {
        return Err(SyncMsg::CursorOutside);
    }
    let msg = if std_dev(&diffs) > 1.0 { SyncMsg::Unstable } else { SyncMsg::Done };
    Ok((delta - median(&diffs) / (f_net * 360.0), msg))
}

/// «Объединить»: the common rate, the highest one rounded to a multiple of
/// the network frequency.
pub fn merged_rate(rates: &[f64], f_net: f64) -> f64 {
    let r = rates.iter().cloned().fold(0.0, f64::max);
    if f_net > 0.0 {
        ((r / f_net).round() * f_net).max(f_net)
    } else {
        r
    }
}

/// The value at time `t` by a local three-point quadratic fit; 0 outside
/// the record (the zero padding).
pub fn quad_at(times: &[f64], x: &[f64], t: f64) -> f64 {
    let n = times.len().min(x.len());
    if n == 0 || t < times[0] || t > times[n - 1] {
        return 0.0;
    }
    if n < 3 {
        return x[0];
    }
    let i = times.partition_point(|&s| s <= t).clamp(1, n - 1);
    let k = i.clamp(1, n - 2);
    let (t0, t1, t2) = (times[k - 1], times[k], times[k + 1]);
    let l0 = (t - t1) * (t - t2) / ((t0 - t1) * (t0 - t2));
    let l1 = (t - t0) * (t - t2) / ((t1 - t0) * (t1 - t2));
    let l2 = (t - t0) * (t - t1) / ((t2 - t0) * (t2 - t1));
    x[k - 1] * l0 + x[k] * l1 + x[k + 1] * l2
}

/// A discrete value at grid time `t` (`0x47e5b8`–`0x47e6a9`): the first
/// sample from index 1 at or after `t`, or the one after it when that one
/// is half a grid `step` or more away; false outside the record (the zero
/// padding) and past its end.
pub fn hold_at(times: &[f64], x: &[bool], t: f64, step: f64) -> bool {
    let n = times.len().min(x.len());
    if n == 0 || t < times[0] || t > times[n - 1] {
        return false;
    }
    let j = times[1.min(n - 1)..].partition_point(|&s| s < t) + 1.min(n - 1);
    let j = j.min(n - 1);
    let k = if times[j] - t < step / 2.0 { j } else { j + 1 };
    x.get(k).copied().unwrap_or(false)
}

/// A pasted channel: its own times (from its record start) and values.
#[derive(Debug, Clone)]
pub struct Pasted {
    pub channel: Result<AnalogChannel, DigitalChannel>,
    pub times: Vec<f64>,
    pub rate: f64,
}

/// «Объединить» (`0x47d990`): the document and the pasted channels (shifted
/// by `offset` seconds) on one uniform grid at the merged rate, spanning both
/// with zero padding, `round(R·duration) − 4` samples.
pub fn merge(base: &Oscillogram, pasted: &[Pasted], offset: f64) -> Oscillogram {
    let t0 = base.time.first().copied().unwrap_or(0.0);
    let base_t: Vec<f64> = base.time.iter().map(|t| t - t0).collect();
    let base_len = base_t.last().copied().unwrap_or(0.0);
    let mut rates = vec![base.sample_rate];
    rates.extend(pasted.iter().map(|p| p.rate));
    let r = merged_rate(&rates, base.line_freq);
    let start = pasted.iter().map(|_| offset).fold(0.0f64, f64::min);
    let end = pasted.iter().map(|p| offset + p.times.last().copied().unwrap_or(0.0)).fold(base_len, f64::max);
    let count = ((r * (end - start)).round() as i64 - 4).max(0) as usize;
    let grid: Vec<f64> = (0..count).map(|i| start + i as f64 / r).collect();
    let step = 1.0 / r;
    let mut out = base.clone();
    out.time = grid.iter().map(|t| t - start).collect();
    out.sample_rate = r;
    out.segments = 1;
    for (dst, src) in out.analog.iter_mut().zip(&base.analog) {
        dst.samples = grid.iter().map(|&t| quad_at(&base_t, &src.samples, t)).collect();
    }
    for (dst, src) in out.digital.iter_mut().zip(&base.digital) {
        dst.samples = grid.iter().map(|&t| hold_at(&base_t, &src.samples, t, step)).collect();
    }
    for p in pasted {
        let shifted: Vec<f64> = p.times.iter().map(|t| t + offset).collect();
        match &p.channel {
            Ok(a) => {
                let mut c = a.clone();
                // Read through the accessor: a NaN reads 0 (`0x426020`).
                let x: Vec<f64> = a.samples.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect();
                c.samples = grid.iter().map(|&t| quad_at(&shifted, &x, t)).collect();
                out.analog.push(c);
            }
            Err(d) => {
                let mut c = d.clone();
                c.samples = grid.iter().map(|&t| hold_at(&shifted, &d.samples, t, step)).collect();
                out.digital.push(c);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(n: usize, per: usize, phase: f64, jump_at: Option<usize>) -> Vec<f64> {
        (0..n)
            .map(|k| {
                let a = if jump_at.is_some_and(|j| k >= j) { 5.0 } else { 1.0 };
                a * (2.0 * std::f64::consts::PI * k as f64 / per as f64 + phase).sin()
            })
            .collect()
    }

    #[test]
    fn start_time_sync_needs_overlap() {
        assert_eq!(sync_by_start(100.0, 1.0, 100.4, 1.0), Ok(0.4000000000000057));
        assert_eq!(sync_by_start(100.0, 1.0, 102.0, 1.0), Err(SyncMsg::NoOverlap));
    }

    #[test]
    fn fault_sync_aligns_the_transitions() {
        let (per, n) = (20, 400);
        let t: Vec<f64> = (0..n).map(|k| k as f64 * 0.001).collect();
        let (x1, x2) = (sine(n, per, 0.0, Some(200)), sine(n, per, 0.0, Some(180)));
        let c1 = SyncChan { times: &t, x: &x1, n: per };
        let c2 = SyncChan { times: &t, x: &x2, n: per };
        let (off, msg) = sync_by_fault(&c1, &c2, 205, 185).unwrap();
        assert_eq!(msg, SyncMsg::Done);
        assert!((off - 0.020).abs() < 1e-9, "{off}");
        assert_eq!(sync_by_fault(&c1, &c2, 30, 185), Err(SyncMsg::CursorBefore3Periods));
    }

    #[test]
    fn prefault_sync_corrects_by_the_phase() {
        let (per, n) = (20, 400);
        let t: Vec<f64> = (0..n).map(|k| k as f64 * 0.001).collect();
        // The pasted record lags by 18° = 1 ms at 50 Hz.
        let (x1, x2) = (sine(n, per, 0.0, None), sine(n, per, -std::f64::consts::PI / 10.0, None));
        let c1 = SyncChan { times: &t, x: &x1, n: per };
        let c2 = SyncChan { times: &t, x: &x2, n: per };
        let (off, msg) = sync_by_prefault(&c1, &c2, 100, 100, 50.0).unwrap();
        assert_eq!(msg, SyncMsg::Done);
        assert!((off + 0.001).abs() < 1e-6, "{off}");
        // 1.5·N + 1 = 31 must not exceed the cursor; past the end only the
        // last sample itself is refused (no period after it needed).
        assert_eq!(sync_by_prefault(&c1, &c2, 30, 100, 50.0), Err(SyncMsg::CursorBefore15Periods));
        assert!(sync_by_prefault(&c1, &c2, 31, 100, 50.0).is_ok());
        assert!(sync_by_prefault(&c1, &c2, 399, 100, 50.0).is_ok());
        assert_eq!(sync_by_prefault(&c1, &c2, 400, 100, 50.0), Err(SyncMsg::CursorOutside));
        // A fault after the cursor's window: the least-spread window lies
        // in the steady part, the result stays clean.
        let x3 = sine(n, per, -std::f64::consts::PI / 10.0, Some(118));
        let c3 = SyncChan { times: &t, x: &x3, n: per };
        let (off, msg) = sync_by_prefault(&c1, &c3, 100, 100, 50.0).unwrap();
        assert_eq!(msg, SyncMsg::Done);
        assert!((off + 0.001).abs() < 1e-6, "{off}");
    }

    /// `0x47bd43` / `0x47c573`, `0x47c555`.
    #[test]
    fn period_samples_and_the_pasted_cursor() {
        assert_eq!(period_samples(0.0, 1000.0, 50.0), 20, "the record's: R div trunc(f)");
        assert_eq!(period_samples(0.0, 1000.0, 49.9), 20, "1000 div 49");
        assert_eq!(period_samples(0.0, 1000.0, 60.0), 16, "1000 div 60 truncates");
        assert_eq!(period_samples(1200.0, 1000.0, 60.0), 24, "a pasted channel: its rate ÷ 50");
        assert_eq!(cursor_in(100, 1000.0, 2000.0, 0.01), 180);
        assert_eq!(cursor_in(101, 1000.0, 500.0, 0.0), 51, "half up");
    }

    #[test]
    fn merge_rate_and_resampling() {
        assert_eq!(merged_rate(&[1000.0, 1200.0], 50.0), 1200.0);
        assert_eq!(merged_rate(&[1210.0], 50.0), 1200.0);
        let t = [0.0, 1.0, 2.0, 3.0];
        let x = [0.0, 1.0, 4.0, 9.0];
        assert!((quad_at(&t, &x, 1.5) - 2.25).abs() < 1e-12, "exact on a parabola");
        assert_eq!(quad_at(&t, &x, 3.5), 0.0, "zero padding outside");
        // The first sample at or after t, the next one when ≥ half a step away.
        let d = [false, true, false, true];
        assert!(!hold_at(&t, &d, 1.9, 0.5), "sample 2 is 0.1 away");
        assert!(hold_at(&t, &d, 1.6, 0.5), "sample 2 is 0.4 away: sample 3");
        assert!(!hold_at(&t, &d, 0.0, 0.5), "from index 1: sample 1 is 1.0 away, so sample 2");
        assert!(!hold_at(&t, &[true; 4], -0.1, 0.5));
    }
}
