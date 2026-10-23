//! Correction of computed values by a measured frequency (ledger L9.65):
//! «Частота для коррекции расчётных значений» (`rec+0x874`). With a
//! frequency channel selected, the per-channel methods integrate over
//! exactly `P` *measured* periods instead of `N = rate / f_nom` samples:
//! the phasor / harmonic `0x4267e0`, rms `0x427450`, a0 `0x426df0`, and a
//! different fault `0x427830`.
//!
//! Every method takes `f` = the frequency channel's stored sample at the
//! window's newest index (no ratio, no flags), 50 when it is 0; the window
//! is `M = P·fs/f` samples, `K = ⌊M⌋`, `τ = P/f − K/fs`; the result is 0
//! while `idx ≤ K`. Integrals are the trapezoid rule over the samples plus
//! the linearly interpolated part interval at the old end, i.e. the weight
//! of `x[idx − j]` is `dt/2` (j = 0), `dt` (0 < j < K),
//! `dt/2 + τ − τ²/(2dt)` (j = K) and `τ²/(2dt)` (j = K + 1).

use crate::comtrade::Oscillogram;
use num_complex::Complex64;

/// The frequency used when the channel reads 0 (a constant, not the
/// record's nominal frequency).
const F_ZERO: f64 = 50.0;

/// The selected frequency channel of a record, as the methods use it.
#[derive(Clone, Copy, Debug)]
pub struct FreqCorr<'a> {
    /// Its analog index; that channel itself is computed normally.
    pub chan: usize,
    /// Its stored samples (Hz).
    pub freq: &'a [f64],
    /// The record's sample rate.
    pub fs: f64,
}

/// The frequency units of the candidate test (exact compares).
const UNITS: [&str; 8] = ["Гц", "ГЦ", "Герц", "ГЕРЦ", "Hz", "HZ", "Herz", "HERZ"];

/// The candidates (`0x42ebd0` with `0x42aee0`) among the file's analog
/// channels: ph or ccbm exactly «system», or one of the frequency units.
/// The formula channels' ([`Oscillogram::extra_freq`]) follow them.
pub fn candidates(osc: &Oscillogram) -> Vec<usize> {
    osc.analog
        .iter()
        .enumerate()
        .filter(|(_, c)| c.phase == "system" || c.ccbm == "system" || is_freq_unit(&c.unit))
        .map(|(i, _)| i)
        .collect()
}

/// Is `unit` a frequency unit for the candidate test? (A formula channel
/// has empty ph / ccbm, so its unit alone decides.)
pub fn is_freq_unit(unit: &str) -> bool {
    UNITS.contains(&unit)
}

/// A frequency candidate: a file analog channel or an [`Oscillogram::extra_freq`]
/// entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cand {
    File(usize),
    Extra(usize),
}

/// The candidate list in vector order (`0x42ec10`–`0x42eca6`): the file's
/// candidates with the extra ones at their places.
pub fn ordered(osc: &Oscillogram) -> Vec<Cand> {
    let file = candidates(osc);
    let mut extra: Vec<usize> = (0..osc.extra_freq.len()).collect();
    extra.sort_by_key(|&j| osc.extra_freq[j].pos);
    let mut out = Vec::new();
    let mut e = extra.into_iter().peekable();
    for i in 0..osc.analog.len() {
        while let Some(j) = e.next_if(|&j| osc.extra_freq[j].pos <= i) {
            out.push(Cand::Extra(j));
        }
        if file.contains(&i) {
            out.push(Cand::File(i));
        }
    }
    out.extend(e.map(Cand::Extra));
    out
}

/// Every candidate's name in list order.
pub fn candidate_names(osc: &Oscillogram) -> Vec<String> {
    ordered(osc)
        .into_iter()
        .map(|c| match c {
            Cand::File(i) => osc.analog[i].id.clone(),
            Cand::Extra(j) => osc.extra_freq[j].name.clone(),
        })
        .collect()
}

/// The selected candidate, when it is an extra one: its holder's id. The
/// gate compares the channel itself (`0x4262b6`), so that channel is not
/// corrected by its own samples.
pub fn selected_extra(osc: &Oscillogram) -> Option<usize> {
    let k = usize::try_from(osc.freq_sel).ok()?.checked_sub(1)?;
    match ordered(osc).get(k)? {
        Cand::Extra(j) => Some(osc.extra_freq[*j].id),
        Cand::File(_) => None,
    }
}

/// The tail of `0x42ebd0`: a selection of 0 or past the list becomes 1 when
/// there is a candidate (so «Не корректировать» does not survive a
/// rebuild); a negative one is kept.
pub fn refresh_selection(osc: &mut Oscillogram) {
    let n = ordered(osc).len() as i32;
    if (osc.freq_sel == 0 || osc.freq_sel > n) && n > 0 {
        osc.freq_sel = 1;
    }
}

/// The shared gate (`0x4262ac` …): one rate segment, a candidate list, a
/// positive selection. The per-channel part (not the frequency channel
/// itself, analog only) is [`FreqCorr::for_chan`].
pub fn of(osc: &Oscillogram) -> Option<FreqCorr<'_>> {
    if osc.segments != 1 || osc.freq_sel <= 0 {
        return None;
    }
    let fs = osc.sample_rate.trunc();
    match *ordered(osc).get(osc.freq_sel as usize - 1)? {
        Cand::File(chan) => Some(FreqCorr { chan, freq: &osc.analog[chan].samples, fs }),
        // Not a file channel: no file channel is the frequency channel.
        Cand::Extra(j) => Some(FreqCorr { chan: usize::MAX, freq: &osc.extra_freq[j].samples, fs }),
    }
}

impl<'a> FreqCorr<'a> {
    /// The correction for analog channel `ch`: none for the frequency
    /// channel itself.
    pub fn for_chan(self, ch: usize) -> Option<FreqCorr<'a>> {
        (ch != self.chan).then_some(self)
    }

    /// `f` at `idx`, 50 when it is 0; `None` past the channel's end.
    fn f_at(&self, idx: usize) -> Option<f64> {
        self.freq.get(idx).map(|&f| if f == 0.0 { F_ZERO } else { f })
    }

    /// The window over `p` measured periods: `(K, τ, dt)`, `None` when it
    /// is not a usable length (our guard; the original runs on).
    fn window(&self, f: f64, p: f64) -> Option<(usize, f64, f64)> {
        let dt = 1.0 / self.fs;
        let m = p * self.fs / f;
        if !m.is_finite() || m < 1.0 {
            return None;
        }
        let k = m.floor() as usize;
        Some((k, p / f - k as f64 / self.fs, dt))
    }

    /// The integration weight of `x[idx − j]`.
    fn weight(j: usize, k: usize, tau: f64, dt: f64) -> f64 {
        let tail = tau * tau / (2.0 * dt);
        if j == 0 {
            dt / 2.0
        } else if j < k {
            dt
        } else if j == k {
            dt / 2.0 + tau - tail
        } else {
            tail
        }
    }

    /// `0x4267e0`: harmonic `n` over `p` measured periods (`by_p`: `n` is a
    /// bin of the `p`-period window, so the angle per sample is
    /// `2π·(n/p)·f·dt`), RMS-scaled, sine-referenced at the newest sample:
    /// `(√2·f/p)·Σ w_j·x[idx−j]·(−sin jθ + i·cos jθ)`. Past the frequency
    /// channel's end `f` is 50.
    pub fn harmonic(&self, x: &[f64], idx: usize, n: f64, by_p: bool, p: f64) -> Complex64 {
        let f = self.f_at(idx).unwrap_or(F_ZERO);
        let Some((k, tau, dt)) = self.window(f, p) else { return Complex64::default() };
        if idx <= k || idx >= x.len() {
            return Complex64::default();
        }
        let th = 2.0 * std::f64::consts::PI * if by_p { n / p } else { n } * f * dt;
        let mut s = Complex64::default();
        for j in 0..=k + 1 {
            let v = Self::weight(j, k, tau, dt) * x[idx - j];
            let a = j as f64 * th;
            s += Complex64::new(-a.sin(), a.cos()) * v;
        }
        s * (2f64.sqrt() * f / p)
    }

    /// The fundamental phasor (`0x428490`'s cache: n = 1 over one period).
    pub fn phasor(&self, x: &[f64], idx: usize) -> Complex64 {
        self.harmonic(x, idx, 1.0, false, 1.0)
    }

    /// `0x427450`: `sqrt((f/p)·Σ w_j·x[idx−j]²)`; 0 past the frequency
    /// channel's end.
    pub fn rms(&self, x: &[f64], idx: usize, p: f64) -> f64 {
        let Some(f) = self.f_at(idx) else { return 0.0 };
        let Some((k, tau, dt)) = self.window(f, p) else { return 0.0 };
        if idx <= k || idx >= x.len() {
            return 0.0;
        }
        let s: f64 = (0..=k + 1).map(|j| Self::weight(j, k, tau, dt) * x[idx - j] * x[idx - j]).sum();
        (f / p * s).sqrt()
    }

    /// `0x426df0`: the signed mean `(f/p)·Σ w_j·x[idx−j]` (no √2).
    pub fn a0(&self, x: &[f64], idx: usize, p: f64) -> f64 {
        let f = self.f_at(idx).unwrap_or(F_ZERO);
        let Some((k, tau, dt)) = self.window(f, p) else { return 0.0 };
        if idx <= k || idx >= x.len() {
            return 0.0;
        }
        let s: f64 = (0..=k + 1).map(|j| Self::weight(j, k, tau, dt) * x[idx - j]).sum();
        f / p * s
    }

    /// `0x427830`: the signed second difference against the samples one and
    /// two measured periods back, each interpolated linearly:
    /// `x[idx] − 2·x̃(1/f) + x̃(2/f)`; 0 while `idx ≤ ⌊2fs/f⌋`.
    pub fn fault(&self, x: &[f64], idx: usize) -> f64 {
        let f = self.f_at(idx).unwrap_or(F_ZERO);
        let dt = 1.0 / self.fs;
        let (m1, m2) = (self.fs / f, 2.0 * self.fs / f);
        if !m1.is_finite() || m1 < 1.0 || idx >= x.len() {
            return 0.0;
        }
        let (k1, k2) = (m1.floor() as usize, m2.floor() as usize);
        if idx <= k2 {
            return 0.0;
        }
        let back = |periods: f64, k: usize| {
            let tau = periods / f - k as f64 / self.fs;
            x[idx - k] + tau / dt * (x[idx - k - 1] - x[idx - k])
        };
        x[idx] - 2.0 * back(1.0, k1) + back(2.0, k2)
    }
}

/// The phasor series of analog channel `ch` (the cache `ch+0x38`): the
/// corrected fundamental when `corr` applies to it, else the normal DFT.
pub fn sliding_phasors(corr: Option<FreqCorr<'_>>, ch: usize, x: &[f64], n: usize) -> crate::signal::Phasors {
    match corr.and_then(|c| c.for_chan(ch)) {
        Some(c) => (0..x.len()).map(|i| c.phasor(x, i)).collect(),
        None => crate::signal::sliding_phasors(x, n),
    }
}

/// The RMS series of analog channel `ch` over one period (`rms(i, 1)`).
pub fn sliding_rms(corr: Option<FreqCorr<'_>>, ch: usize, x: &[f64], n: usize) -> Vec<f64> {
    match corr.and_then(|c| c.for_chan(ch)) {
        Some(c) => (0..x.len()).map(|i| c.rms(x, i, 1.0)).collect(),
        None => crate::signal::sliding_rms(x, n),
    }
}

/// A channel that is never the frequency one (a computed channel).
pub const OTHER: usize = usize::MAX;

/// The phasor of channel `ch` at `i`.
pub fn phasor_at(corr: Option<FreqCorr<'_>>, ch: usize, x: &[f64], i: usize, n: usize) -> num_complex::Complex64 {
    match corr.and_then(|c| c.for_chan(ch)) {
        Some(c) => c.phasor(x, i),
        None => crate::signal::phasor_at(x, i, n),
    }
}

/// `rms(i, periods)` of channel `ch`.
pub fn rms_at(corr: Option<FreqCorr<'_>>, ch: usize, x: &[f64], i: usize, n: usize, periods: usize) -> f64 {
    match corr.and_then(|c| c.for_chan(ch)) {
        Some(c) => c.rms(x, i, periods as f64),
        None => crate::signal::rms_at(x, i, n * periods),
    }
}

/// `fourie(1, i)` of channel `ch` without the record gain (the stats
/// panel's A1).
pub fn fundamental_at(corr: Option<FreqCorr<'_>>, ch: usize, x: &[f64], i: usize, n: usize) -> f64 {
    match corr.and_then(|c| c.for_chan(ch)) {
        Some(c) => c.phasor(x, i).norm(),
        None => crate::signal::harmonic_rms(x, i, n, 1),
    }
}

/// The frequency diagram's bins (`0x426510(k, i, P)`): corrected, bin `k`
/// is harmonic `k/P` of the measured frequency over `P` measured periods,
/// bin 0 included, with no harmonic-1 gain (`0x42657b`). `corr` is the
/// channel's own ([`FreqCorr::for_chan`]).
pub fn spectrum(corr: Option<FreqCorr<'_>>, x: &[f64], i: usize, n: usize, periods: usize, k_max: usize, gain1: f64) -> Vec<f64> {
    match corr {
        Some(c) => (0..=k_max).map(|k| c.harmonic(x, i, k as f64, true, periods.max(1) as f64).norm()).collect(),
        None => crate::signal::spectrum(x, i, n, periods, k_max, gain1),
    }
}
