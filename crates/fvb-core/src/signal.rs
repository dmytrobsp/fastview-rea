//! Signal analysis over an oscillogram: DFT phasors, RMS, symmetric components
//! and loop impedances.
//!
//! The per-channel methods reproduce the decompiled original (ledger L6):
//! phasor `FUN_00426fc0`, rms `FUN_004272a0`, a0 `FUN_00426c90`, harmonic
//! `FUN_00426240`, fault `FUN_00427670`. Every window is the `N` samples ending
//! at (and including) the sample index, and every method returns 0 until a
//! full window is available. Phasors are RMS-scaled and **sine-referenced at
//! the newest sample**: `x[i] = √2·X·sin(ωi + ψ)` gives the phasor `X∠ψ` at `i`.
//! Symmetric components use the Fortescue transform with `a = 1∠120°`; loop
//! impedances are `U_loop / I_loop`.

use num_complex::Complex64;

/// A phasor series: the fundamental-frequency complex value at each sample point
/// (for points before a full window is available the value is zero).
pub type Phasors = Vec<Complex64>;

/// Samples per nominal period as the original computes it: `rate / trunc(f)`
/// with unsigned integer division (`0x427062`), from the record's integer
/// working sample rate and nominal frequency. Never a measured frequency.
pub fn samples_per_period(sample_rate: f64, line_freq: f64) -> Option<usize> {
    let (rate, f) = (sample_rate.trunc(), line_freq.trunc());
    (rate >= 1.0 && f >= 1.0).then(|| (rate as u64 / f as u64) as usize)
}

/// The 120° rotation operator used by the symmetric-component transform.
pub fn operator_a() -> Complex64 {
    Complex64::from_polar(1.0, 120f64.to_radians())
}

/// Fundamental phasor at every sample (`FUN_00426fc0`):
/// `Re = (√2/N)·Σ sin(2πk/N)·x[i−N+k]`, `Im = (√2/N)·Σ cos(2πk/N)·x[i−N+k]`,
/// k = 1..N; zero while `i < N − 1`.
pub fn sliding_phasors(samples: &[f64], n: usize) -> Phasors {
    let len = samples.len();
    let mut out = vec![Complex64::new(0.0, 0.0); len];
    if n < 2 || len < n {
        return out;
    }
    let tau = 2.0 * std::f64::consts::PI;
    let sin_t: Vec<f64> = (1..=n).map(|k| (tau * k as f64 / n as f64).sin()).collect();
    let cos_t: Vec<f64> = (1..=n).map(|k| (tau * k as f64 / n as f64).cos()).collect();
    let scale = 2f64.sqrt() / n as f64;
    for (i, slot) in out.iter_mut().enumerate().skip(n - 1) {
        let (mut re, mut im) = (0.0, 0.0);
        for k in 1..=n {
            let x = samples[i + k - n];
            re += sin_t[k - 1] * x;
            im += cos_t[k - 1] * x;
        }
        *slot = Complex64::new(re * scale, im * scale);
    }
    out
}

/// RMS magnitude of harmonic `h` at sample `i` over the phasor window
/// (`fourie:`, `FUN_00426240`): `sqrt(C² + S²)·√2/N` with angles `k·h·2π/N`;
/// `h = 0` is `|a0|`. Zero before a full window. (The original also scales
/// `h = 1` by a per-record gain that only non-COMTRADE loaders set.)
pub fn harmonic_rms(samples: &[f64], i: usize, n: usize, h: usize) -> f64 {
    if n == 0 || i + 1 < n || i >= samples.len() {
        return 0.0;
    }
    let window = &samples[i + 1 - n..=i];
    if h == 0 {
        return (window.iter().sum::<f64>() / n as f64).abs();
    }
    let tau = 2.0 * std::f64::consts::PI;
    let (mut c, mut s) = (0.0, 0.0);
    for (k, x) in (1..=n).zip(window) {
        let th = k as f64 * h as f64 * tau / n as f64;
        c += x * th.cos();
        s += x * th.sin();
    }
    (c * c + s * s).sqrt() * 2f64.sqrt() / n as f64
}

/// RMS of the full signal over a one-cycle sliding window (true RMS, all
/// harmonics + DC), returned per sample.
pub fn sliding_rms(samples: &[f64], n: usize) -> Vec<f64> {
    let len = samples.len();
    let mut out = vec![0.0; len];
    if n == 0 || len < n {
        return out;
    }
    for (i, slot) in out.iter_mut().enumerate().skip(n - 1) {
        let start = i + 1 - n;
        let mut s = 0.0;
        for m in 0..n {
            let v = samples[start + m];
            s += v * v;
        }
        *slot = (s / n as f64).sqrt();
    }
    out
}

/// DC (average) component over a one-cycle sliding window.
pub fn sliding_dc(samples: &[f64], n: usize) -> Vec<f64> {
    let len = samples.len();
    let mut out = vec![0.0; len];
    if n == 0 || len < n {
        return out;
    }
    for (i, slot) in out.iter_mut().enumerate().skip(n - 1) {
        let start = i + 1 - n;
        let s: f64 = samples[start..start + n].iter().sum();
        *slot = s / n as f64;
    }
    out
}

/// Fault (emergency, аварийная) component per sample (`FUN_00427670`): the
/// RMS over one period of the one-period-lag second difference
/// `d[s] = x[s] − 2·x[s−N] + x[s−2N]`, where terms with `s < 2N` count as 0:
/// `sqrt(Σ d²) / sqrt(N)` over the N samples ending at `i`.
pub fn sliding_fault(samples: &[f64], n: usize) -> Vec<f64> {
    let len = samples.len();
    let mut out = vec![0.0; len];
    if n == 0 {
        return out;
    }
    let d = |s: usize| {
        if s < 2 * n {
            0.0
        } else {
            samples[s] - 2.0 * samples[s - n] + samples[s - 2 * n]
        }
    };
    for (i, slot) in out.iter_mut().enumerate().skip(n - 1) {
        let sum: f64 = (1..=n).map(|k| d(i + k - n).powi(2)).sum();
        *slot = sum.sqrt() / (n as f64).sqrt();
    }
    out
}

/// Symmetric components of a three-phase phasor set at one instant.
/// Returns `(zero*3, positive, negative)` — i.e. the zero entry is `3·I0`
/// (`3·U0`), matching the tool's `3I0`/`3U0` channels.
pub fn symmetric_components(a: Complex64, b: Complex64, c: Complex64) -> (Complex64, Complex64, Complex64) {
    let op = operator_a();
    let op2 = op * op;
    let three_zero = a + b + c;
    let positive = (a + op * b + op2 * c) / 3.0;
    let negative = (a + op2 * b + op * c) / 3.0;
    (three_zero, positive, negative)
}

/// Series form of [`symmetric_components`] over aligned phasor channels.
pub fn symmetric_series(
    a: &Phasors,
    b: &Phasors,
    c: &Phasors,
) -> (Phasors, Phasors, Phasors) {
    let len = a.len().min(b.len()).min(c.len());
    let mut z = vec![Complex64::new(0.0, 0.0); len];
    let mut p = z.clone();
    let mut n = z.clone();
    for i in 0..len {
        let (zz, pp, nn) = symmetric_components(a[i], b[i], c[i]);
        z[i] = zz;
        p[i] = pp;
        n[i] = nn;
    }
    (z, p, n)
}

/// Complex loop impedance `U/I` per sample (guards against tiny currents).
pub fn impedance_series(u: &Phasors, i: &Phasors) -> Phasors {
    let len = u.len().min(i.len());
    let mut out = vec![Complex64::new(0.0, 0.0); len];
    for k in 0..len {
        if i[k].norm() > 1e-9 {
            out[k] = u[k] / i[k];
        }
    }
    out
}

/// Phase-to-phase loop impedance `Z_ab = U_ab / (I_a − I_b)` etc.
pub fn phase_phase_impedance(u_ll: &Phasors, i_a: &Phasors, i_b: &Phasors) -> Phasors {
    let len = u_ll.len().min(i_a.len()).min(i_b.len());
    let mut out = vec![Complex64::new(0.0, 0.0); len];
    for k in 0..len {
        let il = i_a[k] - i_b[k];
        if il.norm() > 1e-9 {
            out[k] = u_ll[k] / il;
        }
    }
    out
}

/// Line voltages from phase voltages: `Uab = Ua − Ub`, etc.
pub fn line_from_phase(ua: Complex64, ub: Complex64, uc: Complex64) -> (Complex64, Complex64, Complex64) {
    (ua - ub, ub - uc, uc - ua)
}

/// Absolute angle of a phasor in degrees.
pub fn angle_deg(z: Complex64) -> f64 {
    z.arg().to_degrees()
}
