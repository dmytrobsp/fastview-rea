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
            .map(|(c, (k, _, _))| c.samples.iter().map(|v| v * k).collect())
            .collect();
        let mut phasors: Vec<Phasors> = samples.iter().enumerate().map(|(i, s)| freqcorr::sliding_phasors(corr, i, s, n)).collect();
        for c in extra {
            names.push(c.name.clone());
            let ch = phasors.len();
            phasors.push(c.phasors.clone().unwrap_or_else(|| freqcorr::sliding_phasors(corr, ch, &c.samples, n)));
            samples.push(c.samples.clone());
            ratios.push((1.0, 1.0, 1.0));
        }
        let units = osc.analog.iter().map(|c| c.unit.clone()).chain(extra.iter().map(|c| c.unit.clone())).collect();
        OscChannels {
            names,
            units,
            ratios,
            samples,
            phasors,
            n,
            freq: osc.freq_track.clone(),
            gain1: osc.harmonic1_gain,
            corr: corr.map(|c| (c.chan, c.freq.to_vec(), c.fs)),
        }
    }

    /// The correction for channel `ch`, if any.
    fn fc(&self, ch: usize) -> Option<FreqCorr<'_>> {
        self.corr.as_ref().and_then(|(chan, freq, fs)| FreqCorr { chan: *chan, freq, fs: *fs }.for_chan(ch))
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
        match self.fc(ch) {
            Some(c) => c.rms(&self.samples[ch], idx, 1.0),
            None => signal::rms_at(&self.samples[ch], idx, self.n),
        }
    }
    fn a0(&self, ch: usize, idx: usize) -> f64 {
        match self.fc(ch) {
            Some(c) => c.a0(&self.samples[ch], idx, 1.0),
            None => signal::a0_at(&self.samples[ch], idx, self.n),
        }
    }
    fn fourie(&self, ch: usize, n: i32, idx: usize) -> f64 {
        // Corrected: the gate comes before the n = 0 case and there is no
        // harmonic-1 gain (`0x4262ac`).
        if let Some(c) = self.fc(ch) {
            return c.harmonic(&self.samples[ch], idx, f64::from(n), false, 1.0).norm();
        }
        let h = signal::harmonic_rms(&self.samples[ch], idx, self.n, n.unsigned_abs() as usize);
        if n == 1 {
            h * self.gain1
        } else {
            h
        }
    }
    fn fault(&self, ch: usize, idx: usize) -> f64 {
        match self.fc(ch) {
            Some(c) => c.fault(&self.samples[ch], idx),
            None => signal::fault_at(&self.samples[ch], idx, self.n),
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
