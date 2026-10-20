//! Signal analysis over an oscillogram: DFT phasors, RMS, symmetric components
//! and loop impedances.
//!
//! Formulas follow the help's "Математический аппарат" appendix (standard
//! power-system definitions): the phasor of a channel at a point is the first
//! DFT harmonic over a one-period window lying in the past (to the left) of that
//! point; symmetric components use the Fortescue transform with
//! `a = 1∠120°`; loop impedances are `U_loop / I_loop`.

use num_complex::Complex64;

/// A phasor series: the fundamental-frequency complex value at each sample point
/// (for points before a full window is available the value is zero).
pub type Phasors = Vec<Complex64>;

/// The 120° rotation operator used by the symmetric-component transform.
pub fn operator_a() -> Complex64 {
    Complex64::from_polar(1.0, 120f64.to_radians())
}

/// Compute the fundamental phasor at every sample using a sliding one-cycle DFT.
///
/// The window of `n` samples ending at (and including) index `i` is transformed;
/// the returned complex value is scaled so that `|phasor|` is the **RMS**
/// magnitude of the fundamental (matching the "действующее значение" the tool
/// displays). Phase is in radians (`arg`).
pub fn sliding_phasors(samples: &[f64], n: usize) -> Phasors {
    let len = samples.len();
    let mut out = vec![Complex64::new(0.0, 0.0); len];
    if n < 2 || len < n {
        return out;
    }
    // Precompute the twiddle factors for the fundamental (k = 1).
    let mut cs = Vec::with_capacity(n);
    for m in 0..n {
        let ang = -2.0 * std::f64::consts::PI * (m as f64) / (n as f64);
        cs.push(Complex64::from_polar(1.0, ang));
    }
    // Peak-normalised DFT bin: X = (2/N) Σ x[m] e^{-j2π m/N}; RMS = |X|/√2.
    let scale = 2.0 / (n as f64) / std::f64::consts::SQRT_2;
    for i in (n - 1)..len {
        let start = i + 1 - n;
        let mut acc = Complex64::new(0.0, 0.0);
        for m in 0..n {
            acc += cs[m] * samples[start + m];
        }
        out[i] = acc * scale;
    }
    out
}

/// RMS of the full signal over a one-cycle sliding window (true RMS, all
/// harmonics + DC), returned per sample.
pub fn sliding_rms(samples: &[f64], n: usize) -> Vec<f64> {
    let len = samples.len();
    let mut out = vec![0.0; len];
    if n == 0 || len < n {
        return out;
    }
    for i in (n - 1)..len {
        let start = i + 1 - n;
        let mut s = 0.0;
        for m in 0..n {
            let v = samples[start + m];
            s += v * v;
        }
        out[i] = (s / n as f64).sqrt();
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
    for i in (n - 1)..len {
        let start = i + 1 - n;
        let s: f64 = samples[start..start + n].iter().sum();
        out[i] = s / n as f64;
    }
    out
}

/// Emergency component (аварийная составляющая): RMS of the second derivative
/// over a one-cycle window — a stationarity-break indicator. Needs ≥ 2 cycles.
pub fn sliding_emergency(samples: &[f64], n: usize, dt: f64) -> Vec<f64> {
    let len = samples.len();
    let mut out = vec![0.0; len];
    if n == 0 || len < 2 * n || dt <= 0.0 {
        return out;
    }
    let mut d2 = vec![0.0; len];
    for i in 1..len - 1 {
        d2[i] = (samples[i + 1] - 2.0 * samples[i] + samples[i - 1]) / (dt * dt);
    }
    for i in (n - 1)..len {
        let start = i + 1 - n;
        let mut s = 0.0;
        for m in 0..n {
            let v = d2[start + m];
            s += v * v;
        }
        out[i] = (s / n as f64).sqrt();
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
