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
    samples: Vec<Vec<f64>>,
    phasors: Vec<Phasors>,
    n: usize,
    freq: f64,
}

impl OscChannels {
    /// The analog channels of `osc` followed by `extra` (name, samples)
    /// channels, e.g. computed channels added earlier.
    pub fn new(osc: &Oscillogram, extra: &[(String, Vec<f64>)]) -> OscChannels {
        let n = osc.samples_per_cycle().max(2);
        let mut names: Vec<String> = osc.analog.iter().map(|c| c.id.clone()).collect();
        let mut samples: Vec<Vec<f64>> = osc.analog.iter().map(|c| c.samples.clone()).collect();
        for (name, s) in extra {
            names.push(name.clone());
            samples.push(s.clone());
        }
        let phasors = samples.iter().map(|s| signal::sliding_phasors(s, n)).collect();
        OscChannels {
            names,
            samples,
            phasors,
            n,
            freq: osc.line_freq,
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
    /// Case-insensitive match on the channel name (the comparison the
    /// original's channel mode uses, `_wcsicmp`). Which channel string the
    /// original matches against is not yet verified (ledger L6).
    fn resolve(&self, name: &str) -> Option<usize> {
        let name = name.to_lowercase();
        self.names.iter().position(|n| n.to_lowercase() == name)
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
        signal::harmonic_rms(&self.samples[ch], idx, self.n, n.unsigned_abs() as usize)
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
