//! The oscillogram as the formula language sees it in channel mode
//! ([`crate::expr::Channels`], the original's `CalcChannels`).

use crate::comtrade::Oscillogram;
use crate::expr::Channels;
use crate::freqcorr::{self, FreqCorr};
use crate::signal::{self, Phasors};
use num_complex::Complex64;

/// Channels of one oscillogram plus already-computed channels, addressed by
/// name, with the per-channel methods of ledger L6.
pub struct OscChannels {
    names: Vec<String>,
    /// Canonical units (`+0x7c`): the record's, then the computed channels'.
    units: Vec<String>,
    /// Per channel (k, r, s) ratios; 1 for computed channels.
    ratios: Vec<(f64, f64, f64)>,
    samples: Vec<Vec<f64>>,
    phasors: Vec<Phasors>,
    n: usize,
    /// Per channel: its own rate (`ch+0x108`, 0 = the record's), which gives
    /// its window round(rate / 50) and the correction's rate (L9.142).
    rates: Vec<f64>,
    /// Channels from here on are digital (appended after the computed
    /// ones, the original's vector order): their RMS / a0 / harmonics /
    /// fault / phasor are 0 (`0x4272ae`, `0x426c9b`, `0x426274`, `0x42767e`,
    /// `0x428ac4`).
    digital_from: usize,
    /// The record's nominal-frequency track (`rec+0x1fc`).
    freq: Vec<f64>,
    /// The record's harmonic-1 gain (`rec+0x278`, set by OSC2 files).
    gain1: f64,
    /// The frequency correction: the frequency channel, its stored samples
    /// and the rate (ledger L9.65).
    corr: Option<(usize, Vec<f64>, f64)>,
}

impl OscChannels {
    /// The analog channels of `osc` followed by the computed channels added
    /// earlier (their stored complex series, if any, is their phasor).
    /// `show_primary` is the display-in-primary-units switch (`+0x89`).
    pub fn new(osc: &Oscillogram, extra: &[crate::quick::ComputedChannel], show_primary: bool) -> OscChannels {
        OscChannels::new_with(osc, extra, show_primary, crate::freqcorr::of(osc))
    }

    /// [`OscChannels::new`] with the frequency correction `corr`; every
    /// channel but the frequency one, computed ones included, is corrected.
    pub fn new_with(osc: &Oscillogram, extra: &[crate::quick::ComputedChannel], show_primary: bool, corr: Option<FreqCorr<'_>>) -> OscChannels {
        OscChannels::new_shown(osc, None, extra, show_primary, corr)
    }

    /// [`OscChannels::new_with`] over analog values the caller already read through the accessor
    /// `0x426020` (`shown[i]` is channel `i` as it returns it: x·k − dc·k, negated for «Инвертировать»,
    /// #105). Without them the stored values are multiplied by k here, which is `(x − dc)·k` once the
    /// DC is removed and differs from the accessor's order in the last bits.
    pub fn new_shown(osc: &Oscillogram, shown: Option<&[&[f64]]>, extra: &[crate::quick::ComputedChannel], show_primary: bool, corr: Option<FreqCorr<'_>>) -> OscChannels {
        let n = osc.samples_per_cycle().max(2);
        // Channel names are the COMTRADE ids with trailing spaces trimmed (+0x68).
        let mut names: Vec<String> = osc.analog.iter().map(|c| c.id.trim_end_matches(' ').to_string()).collect();
        let mut ratios: Vec<(f64, f64, f64)> = osc
            .analog
            .iter()
            .map(|c| {
                let (d0, d8, primary) = (c.primary, c.secondary, c.is_primary);
                let k = if show_primary && !primary {
                    d0 / d8
                } else if primary && !show_primary {
                    d8 / d0
                } else {
                    1.0
                };
                let r = if show_primary { 1.0 } else { d0 / d8 };
                let s = if primary && !show_primary { 1.0 } else { k };
                (k, r, s)
            })
            .collect();
        // Instantaneous values are the stored values times k (FUN_00426020).
        let mut samples: Vec<Vec<f64>> = osc
            .analog
            .iter()
            .zip(&ratios)
            .enumerate()
            .map(|(i, (c, (k, _, _)))| match shown.and_then(|s| s.get(i)) {
                Some(x) => x.to_vec(),
                None => c.samples.iter().map(|v| v * k).collect(),
            })
            .collect();
        let mut rates: Vec<f64> = osc.analog.iter().map(|c| c.own_rate).collect();
        let np = |r: f64| osc.period_of(r).max(2);
        let mut phasors: Vec<Phasors> = samples.iter().enumerate().map(|(i, s)| freqcorr::sliding_phasors(corr.map(|c| c.at_rate(rates[i])), i, s, np(rates[i]))).collect();
        for c in extra {
            names.push(c.name.clone());
            let ch = phasors.len();
            phasors.push(c.phasors.clone().unwrap_or_else(|| freqcorr::sliding_phasors(corr.map(|q| q.at_rate(c.own_rate)), ch, &c.samples, np(c.own_rate))));
            rates.push(c.own_rate);
            samples.push(c.samples.clone());
            ratios.push((1.0, 1.0, 1.0));
        }
        // Digital channels, 1 / 0 as the accessor reads them (`0x4260db`;
        // an inverted one comes inverted in `osc`).
        let digital_from = names.len();
        for d in &osc.digital {
            names.push(d.id.trim_end_matches(' ').to_string());
            samples.push(d.samples.iter().map(|&b| f64::from(u8::from(b))).collect());
            phasors.push(Vec::new());
            rates.push(0.0);
            ratios.push((1.0, 1.0, 1.0));
        }
        let units = osc.analog.iter().map(|c| c.unit.clone()).chain(extra.iter().map(|c| c.unit.clone())).chain(osc.digital.iter().map(|_| String::new())).collect();
        OscChannels {
            digital_from,
            names,
            units,
            ratios,
            samples,
            phasors,
            n,
            rates: rates.clone(),
            freq: osc.freq_track.clone(),
            gain1: osc.harmonic1_gain,
            corr: corr.map(|c| (c.chan, c.freq.to_vec(), c.fs)),
        }
    }

    /// The correction for channel `ch`, if any.
    fn fc(&self, ch: usize) -> Option<FreqCorr<'_>> {
        self.corr.as_ref().and_then(|(chan, freq, fs)| FreqCorr { chan: *chan, freq, fs: *fs }.at_rate(self.rates[ch]).for_chan(ch))
    }

    /// Channel `ch`'s window (`0x428490` and the signal functions).
    fn n_of(&self, ch: usize) -> usize {
        if self.rates[ch] != 0.0 {
            ((self.rates[ch] / 50.0).round() as usize).max(2)
        } else {
            self.n
        }
    }

    pub fn len(&self) -> usize {
        self.samples.first().map_or(0, Vec::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Channels for OscChannels {
    /// `0x42edf0`: the first exact-case match, else the first
    /// case-insensitive one.
    fn resolve(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name).or_else(|| {
            let name = name.to_lowercase();
            self.names.iter().position(|n| n.to_lowercase() == name)
        })
    }
    fn ratio_k(&self, ch: usize) -> f64 {
        self.ratios[ch].0
    }
    fn unit(&self, ch: usize) -> String {
        self.units.get(ch).cloned().unwrap_or_default()
    }
    fn ratio_r(&self, ch: usize) -> f64 {
        self.ratios[ch].1
    }
    fn ratio_s(&self, ch: usize) -> f64 {
        self.ratios[ch].2
    }
    fn sample(&self, ch: usize, idx: usize) -> f64 {
        self.samples[ch].get(idx).copied().unwrap_or(0.0)
    }
    fn phasor(&self, ch: usize, idx: usize) -> Complex64 {
        self.phasors[ch].get(idx).copied().unwrap_or_default()
    }
    fn rms(&self, ch: usize, idx: usize) -> f64 {
        if ch >= self.digital_from {
            return 0.0;
        }
        match self.fc(ch) {
            Some(c) => c.rms(&self.samples[ch], idx, 1.0),
            None => signal::rms_at(&self.samples[ch], idx, self.n_of(ch)),
        }
    }
    fn a0(&self, ch: usize, idx: usize) -> f64 {
        if ch >= self.digital_from {
            return 0.0;
        }
        match self.fc(ch) {
            Some(c) => c.a0(&self.samples[ch], idx, 1.0),
            None => signal::a0_at(&self.samples[ch], idx, self.n_of(ch)),
        }
    }
    fn fourie(&self, ch: usize, n: i32, idx: usize) -> f64 {
        if ch >= self.digital_from {
            return 0.0;
        }
        // Corrected: the gate comes before the n = 0 case and there is no
        // harmonic-1 gain (`0x4262ac`).
        if let Some(c) = self.fc(ch) {
            return c.harmonic(&self.samples[ch], idx, f64::from(n), false, 1.0).norm();
        }
        let h = signal::harmonic_rms(&self.samples[ch], idx, self.n_of(ch), n.unsigned_abs() as usize);
        if n == 1 {
            h * self.gain1
        } else {
            h
        }
    }
    fn fault(&self, ch: usize, idx: usize) -> f64 {
        if ch >= self.digital_from {
            return 0.0;
        }
        match self.fc(ch) {
            Some(c) => c.fault(&self.samples[ch], idx),
            None => signal::fault_at(&self.samples[ch], idx, self.n_of(ch)),
        }
    }
    /// `0x414e90`: the track at the sample, 0 past its end.
    fn frequency(&self, idx: usize) -> f64 {
        self.freq.get(idx).copied().unwrap_or(0.0)
    }
    fn warmup(&self) -> usize {
        self.n
    }
}
