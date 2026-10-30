//! Merging oscillograms («Объединение», ledger L9.15,
//! `docs/original/specs/ui-behaviour.md` §7.8): synchronising a pasted record against
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
    /// The frequency correction when its gate is on (`0x427670`, the
    /// cache `0x428490`): the base record's — for a pasted channel with its
    /// own rate and the base frequency read at its own index.
    pub corr: Option<crate::freqcorr::FreqCorr<'a>>,
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
    // A: the `fault:` dispatcher — corrected (signed) under the gate.
    let a = match c.corr {
        Some(fc) => fc.fault(c.x, k - 1),
        None => fault_at(c.x, k - 1, c.n),
    };
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
        let z = match c.corr {
            Some(fc) => fc.phasor(c.x, k),
            None => phasor_at(c.x, k, c.n),
        };
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

/// A channel prepared for the resampling (`0x47dd20`–`0x47e0d8`): its
/// sample times relative to its first (padded) sample, its own step, and
/// its sub-sample shift (padded at the front only).
struct Lane {
    t: Vec<f64>,
    step: f64,
    shift: f64,
}

impl Lane {
    /// The time of sample `k`, extrapolated past the end at the own step.
    fn at(&self, k: usize) -> f64 {
        match self.t.get(k) {
            Some(&v) => v,
            None => self.t.last().copied().unwrap_or(0.0) + (k + 1 - self.t.len()) as f64 * self.step,
        }
    }

    /// The first sample from index 1 whose time plus the shift reaches `t`
    /// (`0x47e61e`; no cap — past the end the samples read 0).
    fn after(&self, t: f64) -> usize {
        let n = self.t.len();
        if n < 2 {
            return 1;
        }
        1 + self.t[1..].partition_point(|&s| s + self.shift < t)
    }
}

/// Times in whole µs, as `0x428c20` stores the padding's.
fn us(t: f64) -> f64 {
    (t * 1e6).round() / 1e6
}

/// The padder `0x428c20` shifts and pads a channel's own frequency vector
/// (`ch+0x50`) with its samples when it has one: n copies of the base
/// record's nominal frequency (`rec+0x220`) in front or behind
/// (`0x428ea7` / `0x429094`, `0x42912b` / `0x429243`).
fn pad_own(mut own: Vec<f64>, n: usize, front: bool, f_nom: f64) -> Vec<f64> {
    if !own.is_empty() && n > 0 {
        if front {
            own.splice(0..0, std::iter::repeat_n(f_nom, n));
        } else {
            own.extend(std::iter::repeat_n(f_nom, n));
        }
    }
    own
}

/// `0x428c20`: `n` zero samples before (`front`) or after the channel at
/// its own step; the times relative to the new first sample.
fn pad<T: Copy>(t: &mut Vec<f64>, x: &mut Vec<T>, zero: T, n: usize, step: f64, front: bool) {
    if n == 0 {
        return;
    }
    if front {
        let first = t.first().copied().unwrap_or(0.0);
        let mut nt: Vec<f64> = (0..n).map(|i| us(first - (n - i) as f64 * step)).collect();
        nt.extend_from_slice(t);
        let t0 = nt[0];
        *t = nt.into_iter().map(|v| v - t0).collect();
        let mut nx = vec![zero; n];
        nx.extend_from_slice(x);
        *x = nx;
    } else {
        let last = t.last().copied().unwrap_or(0.0);
        t.extend((1..=n).map(|i| us(last + i as f64 * step)));
        x.extend(std::iter::repeat_n(zero, n));
    }
}

/// The analog value at grid time `t` (`0x47e3fb`): the quadratic through
/// samples j − 1, j, j + 1 — j the first from index 1 at or after `t` with
/// the shift — over the times without the shift, evaluated at t − shift; a
/// sample past the end reads 0 at an extrapolated time.
fn fit_at(l: &Lane, x: &[f64], t: f64) -> f64 {
    let j = l.after(t);
    let p = |k: usize| (l.at(k), x.get(k).copied().unwrap_or(0.0));
    let ((t0, y0), (t1, y1), (t2, y2)) = (p(j - 1), p(j), p(j + 1));
    let u = t - l.shift;
    let l0 = (u - t1) * (u - t2) / ((t0 - t1) * (t0 - t2));
    let l1 = (u - t0) * (u - t2) / ((t1 - t0) * (t1 - t2));
    let l2 = (u - t0) * (u - t1) / ((t2 - t0) * (t2 - t1));
    let v = y0 * l0 + y1 * l1 + y2 * l2;
    if v.is_finite() {
        v
    } else {
        y1
    }
}

/// The discrete value at grid time `t` (`0x47e5b8`–`0x47e6a9`): j as for
/// the fit, capped at the grid count; it is taken when its time without
/// the shift is less than half a grid step past `t`, else the next one;
/// past the end false.
fn pick_at(l: &Lane, x: &[bool], t: f64, step: f64, count: usize) -> bool {
    let j = l.after(t).min(count);
    let d = l.at(j) - t;
    let k = if d < step / 2.0 { j } else { j + 1 };
    x.get(k).copied().unwrap_or(false)
}

/// A pasted channel: its own times (from its record start) and values.
#[derive(Debug, Clone)]
pub struct Pasted {
    pub channel: Result<AnalogChannel, DigitalChannel>,
    pub times: Vec<f64>,
    pub rate: f64,
    /// Its padding as the accessor reads a zero sample (minus the DC,
    /// negated; a digital one non-zero when inverted).
    pub pad: f64,
}

/// «Объединить» (`0x47d990`): every channel on one uniform grid at the
/// merged rate R. With the pasted record `offset` seconds after the
/// document, the later side is padded at its front with ceil(r·|offset|)
/// zero samples at its own rate r and shifted by −(1/R − |fmod(offset,
/// 1/R)|) — the base channels when the offset is negative, the pasted
/// ones when positive (exact only when r = R and the offset is not a
/// multiple of 1/R, as in the original); every channel shorter than the
/// longest padded span is padded at its back; the grid has
/// round(R·span) − 4 samples, its times stored in whole µs. `base_pads`
/// are the base analog channels' padding as read through the accessor
/// (the zero minus the DC, inverted…), `dig_pads` the base digital ones'
/// (an inverted channel's zero reads 1).
pub fn merge(base: &Oscillogram, pasted: &[Pasted], offset: f64, base_pads: &[f64], dig_pads: &[bool]) -> Oscillogram {
    let t0 = base.time.first().copied().unwrap_or(0.0);
    let base_t: Vec<f64> = base.time.iter().map(|t| t - t0).collect();
    let mut rates = vec![base.sample_rate];
    rates.extend(pasted.iter().map(|p| p.rate));
    let r = merged_rate(&rates, base.line_freq);
    let g = 1.0 / r;
    let shift = -(g - (offset % g).abs());
    let own = |rate: f64| if rate > 0.0 { rate } else { base.sample_rate };
    // The lanes with their values, base analog / digital, then pasted.
    enum Vals {
        A(Vec<f64>),
        D(Vec<bool>),
    }
    // Each lane with its source's own frequency vector (ch+0x50).
    let mut lanes: Vec<(Lane, Vals, Vec<f64>)> = Vec::new();
    // Each lane's padding value, for the trailing pads too.
    let mut pads: Vec<f64> = Vec::new();
    let front = |rate: f64, base_side: bool| if (base_side && offset < 0.0) || (!base_side && offset > 0.0) { (own(rate) * offset.abs()).ceil() as usize } else { 0 };
    let base_step = 1.0 / own(base.sample_rate);
    let nb = front(base.sample_rate, true);
    for (i, c) in base.analog.iter().enumerate() {
        let (mut t, mut x) = (base_t.clone(), c.samples.clone());
        pad(&mut t, &mut x, base_pads.get(i).copied().unwrap_or(0.0), nb, base_step, true);
        lanes.push((Lane { t, step: base_step, shift: if nb > 0 { shift } else { 0.0 } }, Vals::A(x), pad_own(c.own_freq.clone(), nb, true, base.line_freq)));
        pads.push(base_pads.get(i).copied().unwrap_or(0.0));
    }
    for (i, c) in base.digital.iter().enumerate() {
        let (mut t, mut x) = (base_t.clone(), c.samples.clone());
        pad(&mut t, &mut x, dig_pads.get(i).copied().unwrap_or(false), nb, base_step, true);
        lanes.push((Lane { t, step: base_step, shift: if nb > 0 { shift } else { 0.0 } }, Vals::D(x), Vec::new()));
        pads.push(f64::from(u8::from(dig_pads.get(i).copied().unwrap_or(false))));
    }
    for p in pasted {
        let step = 1.0 / own(p.rate);
        let n = front(p.rate, false);
        let t0 = p.times.first().copied().unwrap_or(0.0);
        let mut t: Vec<f64> = p.times.iter().map(|v| v - t0).collect();
        let sh = if n > 0 { shift } else { 0.0 };
        match &p.channel {
            Ok(a) => {
                let mut x: Vec<f64> = a.samples.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect();
                pad(&mut t, &mut x, p.pad, n, step, true);
                lanes.push((Lane { t, step, shift: sh }, Vals::A(x), pad_own(a.own_freq.clone(), n, true, base.line_freq)));
                pads.push(p.pad);
            }
            Err(d) => {
                let mut x = d.samples.clone();
                pad(&mut t, &mut x, p.pad != 0.0, n, step, true);
                lanes.push((Lane { t, step, shift: sh }, Vals::D(x), Vec::new()));
                pads.push(p.pad);
            }
        }
    }
    // The trailing pads up to the longest padded span.
    let span = |l: &Lane| l.t.last().copied().unwrap_or(0.0);
    let maxdur = lanes.iter().map(|(l, _, _)| span(l)).fold(0.0, f64::max);
    for ((l, v, own), &z) in lanes.iter_mut().zip(&pads) {
        let n = ((maxdur - span(l)) / l.step).abs().ceil() as usize;
        match v {
            Vals::A(x) => {
                pad(&mut l.t, x, z, n, l.step, false);
                *own = pad_own(std::mem::take(own), n, false, base.line_freq);
            }
            Vals::D(x) => pad(&mut l.t, x, z != 0.0, n, l.step, false),
        }
    }
    let count = ((r * maxdur).round() as i64 - 4).max(0) as usize;
    let grid: Vec<f64> = (0..count).map(|i| i as f64 / r).collect();
    let base_track = base.freq_track.clone();
    let mut out = base.clone();
    out.time = grid.iter().map(|&t| us(t)).collect();
    out.sample_rate = r;
    // The base's segments stay; R as an integer (`0x47e7d0`–`0x47e83f`),
    // then the track over the grid (`0x47e84b`).
    out.rate_r = f64::from(crate::comtrade::round_fv(r) as i32);
    out.rebuild_freq_track();
    let (na, nd) = (base.analog.len(), base.digital.len());
    let mut it = lanes.into_iter();
    // An analog channel's values and its own frequency at the fit's source
    // sample j: its own vector, else the record's track, else 0
    // (`0x47e4b5`–`0x47e4f2`).
    let analog = |(l, v, own): (Lane, Vals, Vec<f64>)| -> (Vec<f64>, Vec<f64>) {
        match v {
            Vals::A(x) => (
                grid.iter().map(|&t| fit_at(&l, &x, t)).collect(),
                grid.iter().map(|&t| {
                    let j = l.after(t);
                    own.get(j).or(base_track.get(j)).copied().unwrap_or(0.0)
                }).collect(),
            ),
            Vals::D(_) => (Vec::new(), Vec::new()),
        }
    };
    let digital = |(l, v, _): (Lane, Vals, Vec<f64>)| match v {
        Vals::D(x) => grid.iter().map(|&t| pick_at(&l, &x, t, g, count)).collect(),
        Vals::A(_) => Vec::new(),
    };
    for dst in out.analog.iter_mut().take(na) {
        (dst.samples, dst.own_freq) = it.next().map(analog).unwrap_or_default();
    }
    for dst in out.digital.iter_mut().take(nd) {
        dst.samples = it.next().map(digital).unwrap_or_default();
    }
    for (p, lane) in pasted.iter().zip(it) {
        match &p.channel {
            Ok(a) => {
                let mut c = a.clone();
                (c.samples, c.own_freq) = analog(lane);
                out.analog.push(c);
            }
            Err(d) => {
                let mut c = d.clone();
                c.samples = digital(lane);
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
        let c1 = SyncChan { times: &t, x: &x1, n: per, corr: None };
        let c2 = SyncChan { times: &t, x: &x2, n: per, corr: None };
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
        let c1 = SyncChan { times: &t, x: &x1, n: per, corr: None };
        let c2 = SyncChan { times: &t, x: &x2, n: per, corr: None };
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
        let c3 = SyncChan { times: &t, x: &x3, n: per, corr: None };
        let (off, msg) = sync_by_prefault(&c1, &c3, 100, 100, 50.0).unwrap();
        assert_eq!(msg, SyncMsg::Done);
        assert!((off + 0.001).abs() < 1e-6, "{off}");
    }

    /// Under the frequency correction (L9.126) the phasors are the
    /// corrected ones; at the nominal frequency they agree with the DFT.
    #[test]
    fn prefault_sync_under_the_correction() {
        let (per, n) = (20, 400);
        let t: Vec<f64> = (0..n).map(|k| k as f64 * 0.001).collect();
        let (x1, x2) = (sine(n, per, 0.0, None), sine(n, per, -std::f64::consts::PI / 10.0, None));
        let freq = vec![50.0; n];
        let fc = crate::freqcorr::FreqCorr { chan: crate::freqcorr::OTHER, freq: &freq, fs: 1000.0 };
        let c1 = SyncChan { times: &t, x: &x1, n: per, corr: Some(fc) };
        let c2 = SyncChan { times: &t, x: &x2, n: per, corr: Some(fc) };
        let (off, msg) = sync_by_prefault(&c1, &c2, 100, 100, 50.0).unwrap();
        assert_eq!(msg, SyncMsg::Done);
        assert!((off + 0.001).abs() < 1e-5, "{off}");
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
        // The fit is exact on a parabola; past the end the samples read 0.
        let l = Lane { t: vec![0.0, 1.0, 2.0, 3.0], step: 1.0, shift: 0.0 };
        let x = [0.0, 1.0, 4.0, 9.0];
        assert!((fit_at(&l, &x, 1.5) - 2.25).abs() < 1e-12);
        // 2.5: j = 3, so samples 2, 3 and a zero past the end at 4 — not
        // the parabola (no clamp at the end).
        assert!((fit_at(&l, &x, 2.5) - 8.25).abs() < 1e-12);
        // The pick: the first sample at or after t (from 1), the next
        // when it is half a step or more away.
        let d = [false, true, false, true];
        assert!(!pick_at(&l, &d, 1.9, 0.5, 10), "sample 2 is 0.1 away");
        assert!(pick_at(&l, &d, 1.6, 0.5, 10), "sample 2 is 0.4 away: sample 3");
        assert!(!pick_at(&l, &d, 0.0, 0.5, 10), "from index 1: sample 1 is 1.0 away, so sample 2");
        // The shift moves the search, not the measure (`0x47e696`).
        let sh = Lane { t: vec![0.0, 1.0, 2.0, 3.0], step: 1.0, shift: -0.5 };
        assert!(pick_at(&sh, &d, 1.0, 0.5, 10), "j = 2 (1.5 ≥ 1.0) is 1.0 away: sample 3");
    }

    /// `0x47dd20`–`0x47e0d8`: the later record padded at its front at its
    /// own rate and shifted by −(1/R − |fmod(offset, 1/R)|); the span the
    /// longest padded one; the count round(R·span) − 4; µs times.
    #[test]
    fn merge_pads_and_shifts_the_later_record() {
        let mut cfg = String::from("S,D,1999\n1,1A,0D\n1,A,,,V,1,0,0,-32768,32767,1,1,S\n50\n1\n1000,100\n01/01/2024,00:00:00.000000\n01/01/2024,00:00:00.000000\nASCII\n1\n");
        cfg.push('\n');
        let dat: String = (0..100).map(|k| format!("{},{},{k}\n", k + 1, k * 1000)).collect();
        let base = crate::comtrade::parse(&cfg, dat.as_bytes()).unwrap();
        let mut b_ch = base.analog[0].clone();
        b_ch.samples = vec![5.0; 100];
        let pasted = [Pasted { channel: Ok(b_ch), times: (0..100).map(|k| k as f64 / 1000.0).collect(), rate: 1000.0, pad: 0.0 }];
        // 2.5 ms later: 3 zero samples in front, shift −0.5 ms; the span
        // 0.102 s, so round(102) − 4 samples.
        let m = merge(&base, &pasted, 0.0025, &[0.0], &[]);
        assert_eq!(m.time.len(), 98);
        assert!((m.analog[0].samples[10] - 10.0).abs() < 1e-9, "the base on the grid");
        let b = &m.analog[1].samples;
        assert_eq!(b[0], 0.0, "the padding");
        assert!(b[1] < 0.0, "the fit runs across the zeros into the values (as the original)");
        assert!((b[4] - 5.0).abs() < 1e-9, "the pasted values from 3.5 ms");
        // The base's segment stays; the nominal track over a longer grid
        // reaches its `end` and stops: 0 after it (`0x47e84b`, `0x454960`).
        let m = merge(&base, &pasted, 0.05, &[0.0], &[]);
        assert_eq!(m.rate_segs, base.rate_segs);
        assert!(m.time.len() > 101);
        assert_eq!((m.freq_track[100], m.freq_track[101], m.freq_track.len()), (50.0, 0.0, m.time.len()));
        // Every merged analog channel gets its own frequency at the fit's
        // source sample: its own vector, else the base record's track
        // (`0x47e4b5`).
        let mut own = pasted[0].clone();
        if let Ok(a) = &mut own.channel {
            a.own_freq = vec![50.5; 100];
        }
        let m = merge(&base, &[own], 0.0025, &[0.0], &[]);
        assert_eq!(m.analog[0].own_freq[50], 50.0, "the base channel: the record's track");
        assert_eq!(m.analog[1].own_freq[50], 50.5, "the pasted one: its own");
        // Front-padded with the base's nominal frequency (`0x428c20`).
        assert_eq!(m.analog[1].own_freq[0], base.line_freq, "the padding reads f_nom");
        // An inverted digital channel's padding reads 1 through the accessor.
        let dig = Pasted {
            channel: Err(crate::comtrade::DigitalChannel { index: 1, id: "D".into(), phase: String::new(), normal: 0, samples: vec![false; 100] }),
            times: (0..100).map(|k| k as f64 / 1000.0).collect(),
            rate: 1000.0,
            pad: 1.0,
        };
        let m = merge(&base, &[dig], 0.0025, &[0.0], &[]);
        let d = &m.digital.last().unwrap().samples;
        assert!(d[0] && !d[10], "the front padding on, the samples off");
    }
}
