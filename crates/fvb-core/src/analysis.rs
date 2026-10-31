//! High-level analysis of a loaded oscillogram: detect the three-phase voltage
//! and current channels, compute their phasors, symmetric components and the
//! impedance loci used by the "Быстрый анализ" tabs and the impedance-plane
//! overlay.

use crate::comtrade::Oscillogram;
use crate::signal::*;
use num_complex::Complex64;

/// Phase tag inferred from a channel's phase/id label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    A,
    B,
    C,
    Other,
}

fn classify_phase(label: &str, id: &str) -> Phase {
    let s = format!("{label}{id}").to_lowercase();
    // Latin A/B/C and Cyrillic А/В/С (and common а/в/с/n).
    let has = |cs: &[&str]| cs.iter().any(|c| s.contains(c));
    if has(&["a", "а", "l1"]) && !has(&["ab"]) {
        Phase::A
    } else if has(&["b", "в", "l2"]) {
        Phase::B
    } else if has(&["c", "с", "l3"]) {
        Phase::C
    } else {
        Phase::Other
    }
}

fn is_voltage(unit: &str) -> bool {
    let u = unit.to_lowercase();
    u.starts_with('v') || u.contains("кв") || u.contains("kv") || u == "в"
}

/// Derived per-channel phasor series plus three-phase groups.
pub struct Analysis {
    /// Phasor series aligned to the oscillogram samples, one per analog channel.
    pub phasors: Vec<Phasors>,
    /// Channel indices assigned to voltage phases (A,B,C). Initially auto-detected
    /// from channel names; the user can reassign them (the original's channel-role
    /// mapping, e.g. DIALOG 536).
    pub u_abc: [Option<usize>; 3],
    /// Channel indices assigned to current phases (A,B,C).
    pub i_abc: [Option<usize>; 3],
    /// Compute `Ib = −(Ia + Ic)` instead of reading a channel (isolated-neutral
    /// networks — «Рассчитать Ib по Ia и Iс»).
    pub calc_ib: bool,
    pub n: usize,
}

impl Analysis {
    pub fn compute(osc: &Oscillogram) -> Analysis {
        Analysis::compute_with(osc, crate::freqcorr::of(osc))
    }

    /// [`Analysis::compute`] with the frequency correction `corr` (taken
    /// from the record as stored when `osc` is a flagged copy).
    pub fn compute_with(osc: &Oscillogram, corr: Option<crate::freqcorr::FreqCorr<'_>>) -> Analysis {
        let series: Vec<&[f64]> = osc.analog.iter().map(|c| c.samples.as_slice()).collect();
        Analysis::compute_over(osc, &series, corr)
    }

    /// [`Analysis::compute_with`] with the phasors taken over `series` (one per
    /// analog channel) instead of the channels' own samples: the values as the
    /// accessor `0x426020` returns them, the display factor already applied
    /// per sample.
    pub fn compute_over(osc: &Oscillogram, series: &[&[f64]], corr: Option<crate::freqcorr::FreqCorr<'_>>) -> Analysis {
        let n = osc.samples_per_cycle().max(2);
        // Each channel at its own period (`ch+0x108`, else the record's).
        let own = |i: usize| osc.analog.get(i).map_or(0.0, |c| c.own_rate);
        let phasors: Vec<Phasors> = series
            .iter()
            .enumerate()
            .map(|(i, x)| crate::freqcorr::sliding_phasors(corr.map(|c| c.at_rate(own(i))), i, x, osc.period_of(own(i)).max(2)))
            .collect();

        let mut u_abc = [None, None, None];
        let mut i_abc = [None, None, None];
        for (idx, ch) in osc.analog.iter().enumerate() {
            let ph = classify_phase(&ch.phase, &ch.id);
            let slot = match ph {
                Phase::A => 0,
                Phase::B => 1,
                Phase::C => 2,
                Phase::Other => continue,
            };
            if is_voltage(&ch.unit) {
                if u_abc[slot].is_none() {
                    u_abc[slot] = Some(idx);
                }
            } else if i_abc[slot].is_none() {
                i_abc[slot] = Some(idx);
            }
        }
        Analysis {
            phasors,
            u_abc,
            i_abc,
            calc_ib: false,
            n,
        }
    }

    /// Current phasor for phase B, honouring the «Рассчитать Ib по Ia и Iс» option.
    fn ib(&self, k: usize) -> Complex64 {
        if self.calc_ib {
            -(self.ph(self.i_abc[0], k) + self.ph(self.i_abc[2], k))
        } else {
            self.ph(self.i_abc[1], k)
        }
    }

    fn ph(&self, idx: Option<usize>, k: usize) -> Complex64 {
        idx.and_then(|i| self.phasors.get(i))
            .and_then(|p| p.get(k).copied())
            .unwrap_or(Complex64::new(0.0, 0.0))
    }

    /// Voltage symmetric components `(3U0, U1, U2)` at sample `k`.
    pub fn u_symmetric(&self, k: usize) -> (Complex64, Complex64, Complex64) {
        symmetric_components(
            self.ph(self.u_abc[0], k),
            self.ph(self.u_abc[1], k),
            self.ph(self.u_abc[2], k),
        )
    }

    /// Current symmetric components `(3I0, I1, I2)` at sample `k`.
    pub fn i_symmetric(&self, k: usize) -> (Complex64, Complex64, Complex64) {
        symmetric_components(self.ph(self.i_abc[0], k), self.ib(k), self.ph(self.i_abc[2], k))
    }

    /// Phase-A/B/C loop impedance `Uφ/Iφ` at sample `k`.
    pub fn phase_impedance(&self, phase: usize, k: usize) -> Option<Complex64> {
        let u = self.ph(self.u_abc.get(phase).copied().flatten(), k);
        let i = if phase == 1 {
            self.ib(k)
        } else {
            self.ph(self.i_abc.get(phase).copied().flatten(), k)
        };
        if i.norm() > 1e-9 {
            Some(u / i)
        } else {
            None
        }
    }

    /// Phase-A impedance locus over `[start, end]` samples (for overlaying on the
    /// characteristic plane). Returns `(R, X)` points.
    pub fn phase_a_locus(&self, start: usize, end: usize) -> Vec<[f64; 2]> {
        let mut out = Vec::new();
        for k in start..=end.min(self.phasors.first().map(|p| p.len().saturating_sub(1)).unwrap_or(0)) {
            if let Some(z) = self.phase_impedance(0, k) {
                if z.norm() < 1e6 {
                    out.push([z.re, z.im]);
                }
            }
        }
        out
    }

    /// Phase voltages `(Ua, Ub, Uc)` at sample `k`.
    pub fn u_phase(&self, k: usize) -> (Complex64, Complex64, Complex64) {
        (self.ph(self.u_abc[0], k), self.ph(self.u_abc[1], k), self.ph(self.u_abc[2], k))
    }

    /// Phase currents `(Ia, Ib, Ic)` at sample `k`.
    pub fn i_phase(&self, k: usize) -> (Complex64, Complex64, Complex64) {
        (self.ph(self.i_abc[0], k), self.ib(k), self.ph(self.i_abc[2], k))
    }

    /// Line (phase-to-phase) voltages `(Uab, Ubc, Uca)` at sample `k`.
    pub fn line_voltages(&self, k: usize) -> (Complex64, Complex64, Complex64) {
        let (ua, ub, uc) = self.u_phase(k);
        line_from_phase(ua, ub, uc)
    }

    /// Phase-to-phase loop impedances `[Zab, Zbc, Zca]` at `k`
    /// (`Zab = Uab / (Ia − Ib)`, etc.). `None` where the loop current is ~0.
    pub fn phase_phase_impedances(&self, k: usize) -> [Option<Complex64>; 3] {
        let (ua, ub, uc) = self.u_phase(k);
        let (ia, ib, ic) = self.i_phase(k);
        let zz = |ull: Complex64, il: Complex64| {
            if il.norm() > 1e-9 {
                Some(ull / il)
            } else {
                None
            }
        };
        [
            zz(ua - ub, ia - ib),
            zz(ub - uc, ib - ic),
            zz(uc - ua, ic - ia),
        ]
    }

    /// Differential and restraint currents at `k` for a three-phase set:
    /// `(Id, Ir)` per phase where the differential is the residual sum
    /// `Id = |Ia+Ib+Ic|` (= `|3I0|`) and the restraint `Ir = (|Ia|+|Ib|+|Ic|)/2`.
    pub fn differential_currents(&self, k: usize) -> (f64, f64) {
        let (ia, ib, ic) = self.i_phase(k);
        let id = (ia + ib + ic).norm();
        let ir = (ia.norm() + ib.norm() + ic.norm()) / 2.0;
        (id, ir)
    }
}
