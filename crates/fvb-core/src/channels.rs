//! The oscillogram as the formula language sees it in channel mode
//! ([`crate::expr::Channels`], the original's `CalcChannels`).

use crate::comtrade::Oscillogram;
use crate::expr::Channels;
use crate::signal::{self, Phasors};
use num_complex::Complex64;

/// Channels of one oscillogram plus already-computed channels, addressed by
/// name, with the per-channel methods of ledger L6.
pub struct OscChannels {
    names: Vec<String>,
    /// Per channel (k, r, s) ratios; 1 for computed channels.
    ratios: Vec<(f64, f64, f64)>,
    samples: Vec<Vec<f64>>,
    phasors: Vec<Phasors>,
    n: usize,
    freq: f64,
    /// The record's harmonic-1 gain (`rec+0x278`, set by OSC2 files).
    gain1: f64,
}

impl OscChannels {
    /// The analog channels of `osc` followed by the computed channels added
    /// earlier (their stored complex series, if any, is their phasor).
    /// `show_primary` is the display-in-primary-units switch (`+0x89`).
    pub fn new(osc: &Oscillogram, extra: &[crate::quick::ComputedChannel], show_primary: bool) -> OscChannels {
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
        let mut phasors: Vec<Phasors> = samples.iter().map(|s| signal::sliding_phasors(s, n)).collect();
        for c in extra {
            names.push(c.name.clone());
            phasors.push(c.phasors.clone().unwrap_or_else(|| signal::sliding_phasors(&c.samples, n)));
            samples.push(c.samples.clone());
            ratios.push((1.0, 1.0, 1.0));
        }
        OscChannels {
            names,
            ratios,
            samples,
            phasors,
            n,
            freq: osc.line_freq,
            gain1: osc.harmonic1_gain,
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
        signal::rms_at(&self.samples[ch], idx, self.n)
    }
    fn a0(&self, ch: usize, idx: usize) -> f64 {
        signal::a0_at(&self.samples[ch], idx, self.n)
    }
    fn fourie(&self, ch: usize, n: i32, idx: usize) -> f64 {
        let h = signal::harmonic_rms(&self.samples[ch], idx, self.n, n.unsigned_abs() as usize);
        if n == 1 {
            h * self.gain1
        } else {
            h
        }
    }
    fn fault(&self, ch: usize, idx: usize) -> f64 {
        signal::fault_at(&self.samples[ch], idx, self.n)
    }
    fn frequency(&self, _idx: usize) -> f64 {
        self.freq
    }
    fn warmup(&self) -> usize {
        self.n
    }
}
