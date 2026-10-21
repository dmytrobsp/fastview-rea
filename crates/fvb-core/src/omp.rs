//! Fault location (ОМП — определение места повреждения).
//!
//! Single-ended (одностороннего замера) uses the reactance method: the fault
//! loop reactance seen from one terminal, divided by the line reactance per unit
//! length, gives the distance. Following the help's derivation, the arc (fault)
//! resistance is purely resistive and carried by a current co-directional with
//! the fault current, so taking the imaginary part of `U/I` cancels it.
//!
//! Double-ended (двустороннего замера) uses measurements from both terminals and
//! the total line impedance, removing the infeed/arc dependence entirely.

use num_complex::Complex64;

/// Per-unit-length line parameters (secondary ohms per km): positive-sequence
/// `(r1,x1)` and zero-sequence `(r0,x0)` used for ground-fault compensation.
#[derive(Debug, Clone, Copy)]
pub struct LineParams {
    pub r1_per_km: f64,
    pub x1_per_km: f64,
    pub r0_per_km: f64,
    pub x0_per_km: f64,
    pub length_km: f64,
}

impl LineParams {
    /// Zero-sequence compensation factor `k0 = (Z0 − Z1) / (3·Z1)`.
    pub fn k0(&self) -> Complex64 {
        let z1 = Complex64::new(self.r1_per_km, self.x1_per_km);
        let z0 = Complex64::new(self.r0_per_km, self.x0_per_km);
        if z1.norm() < 1e-12 {
            Complex64::new(0.0, 0.0)
        } else {
            (z0 - z1) / (3.0 * z1)
        }
    }
}

/// How the network's neutral is treated, selecting the ОМП loop/compensation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutralMode {
    /// Глухозаземлённая нейтраль — phase-ground loop with zero-sequence
    /// compensation.
    Grounded,
    /// Изолированная нейтраль — interphase (phase-phase) loop, no zero-seq
    /// compensation.
    Isolated,
}

/// Single-ended reactance-method distance to fault, in km.
///
/// `u_loop`, `i_loop` are the faulted-loop phasors at the measuring terminal.
/// The reactance to the fault is `Im(U/I)`; distance = that / `x1_per_km`.
pub fn single_ended_distance(u_loop: Complex64, i_loop: Complex64, line: &LineParams) -> Option<f64> {
    if i_loop.norm() < 1e-9 || line.x1_per_km.abs() < 1e-12 {
        return None;
    }
    let z = u_loop / i_loop;
    let x_fault = z.im;
    Some(x_fault / line.x1_per_km)
}

/// Single-ended distance for a solidly-grounded network, with zero-sequence
/// compensation: `distance = Im( U_A / (I_A + k0·3I0) ) / x1`. `u_a`/`i_a` are the
/// faulted phase's voltage/current, `i_3i0` the residual (`Ia+Ib+Ic`).
pub fn single_ended_grounded(
    u_a: Complex64,
    i_a: Complex64,
    i_3i0: Complex64,
    line: &LineParams,
) -> Option<f64> {
    if line.x1_per_km.abs() < 1e-12 {
        return None;
    }
    let denom = i_a + line.k0() * i_3i0;
    if denom.norm() < 1e-9 {
        return None;
    }
    Some((u_a / denom).im / line.x1_per_km)
}

/// Fraction (0..1) of the line length to the fault, single-ended.
pub fn single_ended_fraction(u_loop: Complex64, i_loop: Complex64, line: &LineParams) -> Option<f64> {
    single_ended_distance(u_loop, i_loop, line).map(|d| {
        if line.length_km > 0.0 {
            d / line.length_km
        } else {
            d
        }
    })
}

/// Double-ended distance from the local terminal, in km.
///
/// Given the faulted-loop voltage/current phasors at both line ends (`m` = local,
/// `n` = remote) and the total line impedance `z_line`, the per-unit distance `m`
/// from the local end solves the two-terminal voltage-balance relation. A common
/// synchronised-measurement estimator is:
///
/// ```text
///   m = Im( (U_m − U_n + Z_line·I_n) / (Z_line·(I_m + I_n)) )
/// ```
///
/// which is independent of fault resistance. Returns distance in km.
pub fn double_ended_distance(
    u_m: Complex64,
    i_m: Complex64,
    u_n: Complex64,
    i_n: Complex64,
    line: &LineParams,
) -> Option<f64> {
    let z_line = Complex64::new(
        line.r1_per_km * line.length_km,
        line.x1_per_km * line.length_km,
    );
    let denom = z_line * (i_m + i_n);
    if denom.norm() < 1e-12 {
        return None;
    }
    let m = ((u_m - u_n + z_line * i_n) / denom).re;
    Some(m.clamp(0.0, 1.0) * line.length_km)
}

// ---------------------------------------------------------------------------
// The original's per-sample engine (ledger L7, `notes/OMP_SPEC.md`). The
// functions above are the earlier single-instant approximation, kept until
// the GUI moves to this engine.
// ---------------------------------------------------------------------------

/// A parallel line coupled to a section: mutual impedance per km and that
/// line's current phasor per sample.
#[derive(Debug, Clone, Default)]
pub struct ParallelLine {
    pub rm_per_km: f64,
    pub xm_per_km: f64,
    pub current: Vec<Complex64>,
}

/// A tap (отпайка) off a section: its zero-sequence impedance in total ohms.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tap {
    pub r0: f64,
    pub x0: f64,
}

/// One line section (участок) — the original's 0x58-byte record.
#[derive(Debug, Clone, Default)]
pub struct Section {
    /// R, X per km used by the isolated-neutral method (`+0x0/+0x8`).
    pub r_per_km: f64,
    pub x_per_km: f64,
    pub length_km: f64,
    pub r0_per_km: f64,
    pub x0_per_km: f64,
    pub r1_per_km: f64,
    pub x1_per_km: f64,
    pub supports: u32,
    pub parallel: Vec<ParallelLine>,
    pub taps: Vec<Tap>,
}

/// Per-sample state of one single-ended analysis.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    /// Earth-fault, 2-phase and 3-phase flags (after debounce when grounded).
    pub e: Vec<bool>,
    pub f2: Vec<bool>,
    pub f3: Vec<bool>,
    /// Faulted-phase flags.
    pub pa: Vec<bool>,
    pub pb: Vec<bool>,
    pub pc: Vec<bool>,
    pub uloop: Vec<Complex64>,
    pub iloop: Vec<Complex64>,
    pub iref: Vec<Complex64>,
    /// Instantaneous loop-current combination of the raw samples (window choice).
    pub inst: Vec<f64>,
    /// Distance from the line start, km, per sample.
    pub dist: Vec<f64>,
}

/// The ОМП phasor (`0x477280`): peak-scaled one-period DFT over the `p`
/// samples *before* `k` (window `[k−p, k−1]`), sine reference; 0 for `k < p`.
pub fn omp_phasors(x: &[f64], p: usize) -> Vec<Complex64> {
    let mut out = vec![Complex64::default(); x.len()];
    if p == 0 {
        return out;
    }
    let tau = 2.0 * std::f64::consts::PI;
    for (k, slot) in out.iter_mut().enumerate().skip(p) {
        let (mut re, mut im) = (0.0, 0.0);
        for j in 0..p {
            let th = j as f64 * tau / p as f64;
            let v = x[k - p + j];
            re += th.sin() * v;
            im += th.cos() * v;
        }
        let scale = 2.0 / p as f64;
        *slot = Complex64::new(re * scale, im * scale);
    }
    out
}

/// Hysteresis `H(set, reset, prev)`: set → true, then reset → false (reset
/// wins), else hold.
fn hyst(prev: bool, set: bool, reset: bool) -> bool {
    let mut f = prev;
    if set {
        f = true;
    }
    if reset {
        f = false;
    }
    f
}

/// Debounce (`0x499930`): a flag rises after `trunc(p/4)` consecutive raw
/// samples disagree with it, falls on the first one; `x[0]` is ignored.
pub fn debounce(x: &[bool], p: usize) -> Vec<bool> {
    let t = (p / 4) as i64;
    let mut y = vec![false; x.len()];
    let mut c: i64 = 0;
    for i in 1..x.len() {
        let prev = y[i - 1];
        y[i] = prev;
        c = if x[i] != prev { c + 1 } else { 0 };
        if prev && !x[i] && c >= 0 {
            y[i] = false;
            c = -1;
        } else if !prev && x[i] && c >= t {
            y[i] = true;
            c = -1;
        }
    }
    y
}

/// Sequence components of three phasors: `(I1, I2, I0)`, each divided by 3.
fn components(a: Complex64, b: Complex64, c: Complex64) -> (Complex64, Complex64, Complex64) {
    let op = Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI / 3.0);
    let op2 = op * op;
    ((a + op * b + op2 * c) / 3.0, (a + op2 * b + op * c) / 3.0, (a + b + c) / 3.0)
}

/// MSVC `std::complex` division as the original uses it (`0x420ca0`): an
/// exactly-zero or NaN divisor gives NaN.
fn cdiv(a: Complex64, b: Complex64) -> Complex64 {
    if (b.re == 0.0 && b.im == 0.0) || b.re.is_nan() || b.im.is_nan() {
        return Complex64::new(f64::NAN, f64::NAN);
    }
    a / b
}

const ONE: Complex64 = Complex64::new(1.0, 0.0);

/// Inputs of a single-ended analysis: phase-current samples, the loop
/// voltages (phase voltages when grounded, line voltages Uab/Ubc/Uca when
/// isolated), samples per period and the nominal current.
pub struct SingleEnded<'a> {
    pub neutral: NeutralMode,
    pub u: [&'a [f64]; 3],
    pub i: [&'a [f64]; 3],
    pub p: usize,
    pub inom: f64,
    pub sections: &'a [Section],
}

/// Run detection, phase selection, loop choice and the per-sample distance
/// (θ = 0; the tap θ re-estimation of `0x49adc0` is not reproduced yet).
// Sample-index loops over several parallel per-sample arrays, as in the original.
#[allow(clippy::needless_range_loop)]
pub fn single_ended(inp: &SingleEnded) -> Trace {
    let nk = inp.i.iter().map(|s| s.len()).min().unwrap_or(0);
    let p = inp.p;
    let ip: Vec<Vec<Complex64>> = inp.i.iter().map(|s| omp_phasors(&s[..nk], p)).collect();
    let up: Vec<Vec<Complex64>> = inp.u.iter().map(|s| omp_phasors(&s[..nk.min(s.len())], p)).collect();
    let uph = |c: usize, k: usize| up[c].get(k).copied().unwrap_or_default();
    let comps: Vec<_> = (0..nk).map(|k| components(ip[0][k], ip[1][k], ip[2][k])).collect();
    let m = |z: Complex64| z.norm();
    let g = 0.1 * 2f64.sqrt() * inp.inom;
    let mut t = Trace {
        e: vec![false; nk],
        f2: vec![false; nk],
        f3: vec![false; nk],
        pa: vec![false; nk],
        pb: vec![false; nk],
        pc: vec![false; nk],
        uloop: vec![Complex64::default(); nk],
        iloop: vec![Complex64::default(); nk],
        iref: vec![Complex64::default(); nk],
        inst: vec![0.0; nk],
        dist: vec![0.0; nk],
    };
    let raw = |c: usize, k: usize| inp.i[c][k];

    match inp.neutral {
        NeutralMode::Grounded => {
            // 0x498580: two hysteresis passes, then debounce.
            let (mut s, mut h, mut a, mut n) = (false, false, false, false);
            let (mut e, mut f2, mut f3) = (vec![false; nk], vec![false; nk], vec![false; nk]);
            for k in 1..nk {
                let (i1, i2, _) = comps[k];
                s = hyst(s, m(i1) > 4.0 * m(i2), m(i1) < 0.9 * 4.0 * m(i2));
                h = hyst(h, m(i1) > 2f64.sqrt() * inp.inom, m(i1) < 0.9 * 2f64.sqrt() * inp.inom);
                f3[k] = s && h;
            }
            for k in 1..nk {
                let (i1, i2, i0) = comps[k];
                a = hyst(a, 4.0 * m(i2) > m(i1), 0.9 * m(i1) > 4.0 * m(i2));
                n = hyst(n, m(i2) > 6.0 * m(i0), 0.9 * 6.0 * m(i0) > m(i2));
                let gate = m(i1) > g;
                f2[k] = a && n && gate;
                e[k] = a && !n && gate;
            }
            t.e = debounce(&e, p);
            t.f2 = debounce(&f2, p);
            t.f3 = debounce(&f3, p);
            // Phase selection with hysteretic magnitude comparisons.
            let (mut ab, mut ac, mut bc) = (false, false, false);
            for k in 1..nk {
                let (a_, b_, c_) = (m(ip[0][k]), m(ip[1][k]), m(ip[2][k]));
                ab = hyst(ab, a_ > b_, 0.9 * b_ > a_);
                ac = hyst(ac, a_ > c_, 0.9 * c_ > a_);
                bc = hyst(bc, b_ > c_, 0.9 * c_ > b_);
                let (e, f2, f3) = (t.e[k], t.f2[k], t.f3[k]);
                t.pa[k] = f3 || f2 && (ab || ac) || e && ab && ac;
                t.pb[k] = f3 || f2 && (!ab || bc) || e && !ab && bc;
                t.pc[k] = f3 || f2 && (!ac || !bc) || e && !ac && !bc;
            }
            // 0x499e90: loop choice.
            for k in p..nk {
                let ph = |c: usize| ip[c][k];
                let (e, f2, f3) = (t.e[k], t.f2[k], t.f3[k]);
                let (mut u, mut i, mut inst) = (ONE, ONE, 1.0);
                if f3 {
                    (u, i, inst) = (uph(0, k) - uph(1, k), ph(0) - ph(1), raw(0, k) - raw(1, k));
                }
                if f2 {
                    (u, i, inst) = if t.pa[k] && t.pb[k] {
                        (uph(0, k) - uph(1, k), ph(0) - ph(1), raw(0, k) - raw(1, k))
                    } else if t.pb[k] && t.pc[k] {
                        (uph(1, k) - uph(2, k), ph(1) - ph(2), raw(1, k) - raw(2, k))
                    } else if t.pa[k] && t.pc[k] {
                        (uph(2, k) - uph(0, k), ph(2) - ph(0), raw(2, k) - raw(0, k))
                    } else {
                        (ONE, ONE, 1.0)
                    };
                }
                if e {
                    (u, i) = if t.pa[k] {
                        (uph(0, k), ph(0))
                    } else if t.pb[k] {
                        (uph(1, k), ph(1))
                    } else if t.pc[k] {
                        (uph(2, k), ph(2))
                    } else {
                        (ONE, ONE)
                    };
                    inst = raw(0, k) + raw(1, k) + raw(2, k);
                }
                let iref = if f2 || f3 {
                    i
                } else if e {
                    comps[k].2
                } else {
                    (u, i, inst) = (ONE, ONE, 1.0);
                    ONE
                };
                t.uloop[k] = u;
                t.iloop[k] = i;
                t.iref[k] = iref;
                t.inst[k] = inst;
            }
            for k in p..nk {
                let u0 = (uph(0, k) + uph(1, k) + uph(2, k)) / 3.0;
                t.dist[k] = grounded_distance(&t, k, u0, comps[k].2, inp.sections, 0.0);
            }
        }
        NeutralMode::Isolated => {
            // 0x497f10: no hysteresis/debounce.
            for k in 0..nk {
                let (i1, i2, _) = comps[k];
                if m(i1) > g {
                    t.f2[k] = 3.0 * m(i2) > m(i1);
                    t.f3[k] = m(i1) >= 3.0 * m(i2) && m(i1) > 1.15 * 2f64.sqrt() * inp.inom;
                }
                let (a_, b_, c_) = (m(ip[0][k]), m(ip[1][k]), m(ip[2][k]));
                let (f2, f3) = (t.f2[k], t.f3[k]);
                t.pa[k] = f3 || f2 && (a_ > b_ || a_ > c_);
                t.pb[k] = f3 || f2 && (b_ > a_ || b_ > c_);
                t.pc[k] = f3 || f2 && (c_ > a_ || c_ > b_);
            }
            // 0x499c80: AB, BC, CA tested in turn; the last match wins.
            for k in p..nk {
                let ph = |c: usize| ip[c][k];
                let (mut u, mut i, mut inst) = (ONE, ONE, 1.0);
                if t.f2[k] || t.f3[k] {
                    if t.pa[k] && t.pb[k] {
                        (u, i, inst) = (uph(0, k), ph(0) - ph(1), raw(0, k) - raw(1, k));
                    }
                    if t.pb[k] && t.pc[k] {
                        (u, i, inst) = (uph(1, k), ph(1) - ph(2), raw(1, k) - raw(2, k));
                    }
                    if t.pa[k] && t.pc[k] {
                        (u, i, inst) = (uph(2, k), ph(2) - ph(0), raw(2, k) - raw(0, k));
                    }
                }
                t.uloop[k] = u;
                t.iloop[k] = i;
                t.inst[k] = inst;
            }
            for k in 0..nk {
                t.dist[k] = if t.f2[k] || t.f3[k] {
                    isolated_distance(t.uloop[k], t.iloop[k], inp.sections)
                } else {
                    0.0
                };
            }
        }
    }
    t
}

/// `0x49a670` for one sample: walk the sections, dropping the voltage across
/// every section the fault lies beyond (§6 of `notes/OMP_SPEC.md`).
fn grounded_distance(t: &Trace, k: usize, mut u0: Complex64, mut i0: Complex64, secs: &[Section], theta: f64) -> f64 {
    let (mut u, mut iph, mut iref) = (t.uloop[k], t.iloop[k], t.iref[k]);
    let earth = t.e[k];
    let mut acc = 0.0;
    let mut d = 0.0;
    for sec in secs {
        let z1 = Complex64::new(sec.r1_per_km, sec.x1_per_km);
        let z0 = Complex64::new(sec.r0_per_km, sec.x0_per_km);
        let (kk, pp) = if earth {
            let pp = sec.parallel.iter().fold(Complex64::default(), |s, pl| {
                let ip = pl.current.get(k).copied().unwrap_or_default();
                s + cdiv(Complex64::new(pl.rm_per_km, pl.xm_per_km) * ip, z1)
            });
            (cdiv(z0, z1) - ONE, pp)
        } else {
            (Complex64::default(), Complex64::default())
        };
        let iref_r = iref * Complex64::from_polar(1.0, theta);
        let ii = iph + kk * i0 + pp;
        d = cdiv(u, iref_r).im / cdiv(z1 * ii, iref_r).im;
        if sec.length_km >= d {
            break;
        }
        acc += sec.length_km;
        u -= z1 * sec.length_km * ii;
        u0 -= z0 * sec.length_km * (i0 + pp);
        if earth && !sec.taps.is_empty() {
            let y = sec.taps.iter().fold(Complex64::default(), |s, tp| s + cdiv(ONE, Complex64::new(tp.r0, tp.x0)));
            let it = cdiv(u0, cdiv(ONE, y));
            i0 -= it;
            iph += it;
            iref = i0;
        }
    }
    if 0.0 > d {
        d = 0.0;
    }
    d + acc
}

/// `0x49a430` for one sample (§7 of `notes/OMP_SPEC.md`).
fn isolated_distance(uloop: Complex64, iloop: Complex64, secs: &[Section]) -> f64 {
    let mut zacc = Complex64::default();
    let mut acc = 0.0;
    let mut d = 0.0;
    for sec in secs {
        let zp = cdiv(uloop, iloop) - zacc;
        let delta = sec.x_per_km.atan2(sec.r_per_km) - zp.im.atan2(zp.re);
        d = if delta > 0.0 { zp.im / sec.x_per_km } else { zp.re / sec.r_per_km };
        if sec.length_km >= d {
            break;
        }
        zacc += Complex64::new(sec.r_per_km * sec.length_km, sec.x_per_km * sec.length_km);
        acc += sec.length_km;
    }
    if 0.0 > d {
        d = 0.0;
    }
    d + acc
}

/// Median as the original computes it (`0x477810`): sort a copy, average the
/// elements between floor and ceil of the middle position.
pub fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = (s.len() - 1) as f64 / 2.0;
    (s[mid.floor() as usize] + s[mid.ceil() as usize]) / 2.0
}
