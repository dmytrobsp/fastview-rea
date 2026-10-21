//! Fault location (ОМП — определение места повреждения), reproducing the
//! original's per-sample engine (`0x4a00b0`; ledger L7, `notes/OMP_SPEC.md`):
//! phasors over the period before each sample, fault-type detection with
//! hysteresis and debounce, phase selection and loop choice, a distance per
//! sample walking the line's sections, then a one-period window chosen by the
//! original's σ / |mean| search, the median distance over it and the result
//! text. Grounded neutral uses an I0-polarised (Takagi-type) formula, isolated
//! neutral the loop impedance against R, X per km; the double-ended method
//! matches the fault-point sequence voltage magnitudes seen from both ends.

use num_complex::Complex64;

/// Network neutral treatment for the single-ended methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutralMode {
    /// Глухозаземлённая нейтраль — phase-ground loop with zero-sequence
    /// compensation.
    Grounded,
    /// Изолированная нейтраль — interphase (phase-phase) loop, no zero-seq
    /// compensation.
    Isolated,
}

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
    /// Double-ended: `arg(U/I)` at end 1 minus end 2, radians, per sample.
    pub angle: Vec<f64>,
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
        angle: vec![0.0; nk],
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

/// The fault interval the window is chosen in (`0x499b50`, isolated
/// `0x499aa0`): the **first** run of one phase code (grounded/double-ended) or
/// of F2 ∨ F3 (isolated) at least one period long; `(P, M−1)` if none.
pub fn fault_interval(t: &Trace, neutral: NeutralMode, p: usize) -> (usize, usize) {
    let m = t.dist.len();
    if m < 2 || p == 0 {
        return (p, m.saturating_sub(1));
    }
    let code = |k: usize| u8::from(t.pa[k]) + 2 * u8::from(t.pb[k]) + 4 * u8::from(t.pc[k]);
    let fault = |k: usize| match neutral {
        NeutralMode::Isolated => t.f2[k] || t.f3[k],
        NeutralMode::Grounded => t.e[k] || t.f2[k] || t.f3[k],
    };
    let mut e = p - 1;
    let mut s;
    loop {
        s = (e + 1..=m - 2).find(|&k| fault(k)).unwrap_or(m - 1);
        e = match neutral {
            NeutralMode::Isolated => (s + 1..m).find(|&k| !fault(k)).unwrap_or(m),
            NeutralMode::Grounded => (s + 1..m).find(|&k| code(k) != code(s)).unwrap_or(m),
        };
        if s + p >= m - 1 || e - s >= p {
            break;
        }
    }
    if e - s >= p {
        (s, e)
    } else {
        (p, m - 1)
    }
}

fn window_sigma(x: &[f64], t: usize, p: usize) -> f64 {
    let w = &x[t..t + p];
    let mean = w.iter().sum::<f64>() / p as f64;
    (w.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / p as f64).sqrt()
}

fn window_mean_abs(x: &[f64], t: usize, p: usize) -> f64 {
    (x[t..t + p].iter().sum::<f64>() / p as f64).abs()
}

/// The one-period window inside `[lo, hi]` (`0x477db0`): alternate a
/// minimum-σ-of-distance search and a minimum-|mean loop current| search, each
/// restricted to scores above the previous pick's raw value, until the two
/// picks are within half a period; then shift the σ pick half-way away from
/// the current pick. Returns `(cursor1, cursor2)`.
pub fn choose_window(dist: &[f64], inst: &[f64], lo: usize, hi: usize, p: usize) -> (usize, usize) {
    let fits = p > 0 && hi >= lo + p && hi < dist.len().min(inst.len());
    let search = |score: &dyn Fn(usize) -> f64, tau: f64| -> usize {
        if !fits {
            return lo;
        }
        let (mut best, mut bt) = (1e6, lo);
        for t in lo..=hi - p {
            let sc = score(t);
            if sc > tau && sc < best {
                best = sc;
                bt = t;
            }
        }
        bt
    };
    let bias = |t: usize, eps: f64| eps - (t - lo) as f64 * eps / (10.0 * p as f64);
    let sigma_score = |t: usize| window_sigma(dist, t, p) + bias(t, 0.001);
    let mean_score = |t: usize| window_mean_abs(inst, t, p) + bias(t, 1e-6);
    let (mut ta, mut tb) = (0.0, 0.0);
    for i in 0..1000 {
        let a = search(&sigma_score, ta);
        let b = search(&mean_score, tb);
        if (a as f64 - b as f64).abs() <= 0.5 * p as f64 {
            if i == 999 {
                break;
            }
            let c1 = (a as f64 + ((a as f64 - b as f64) / 2.0).ceil()).trunc() as usize;
            return (c1, c1 + p.saturating_sub(1));
        }
        if fits {
            ta = window_sigma(dist, a, p);
            tb = window_mean_abs(inst, b, p);
        }
    }
    let a = search(&sigma_score, -1.0);
    (a, a + p.saturating_sub(1))
}

/// The outcome shown to the user (`0x49d330`, `0x49d200`, `0x49d700`).
#[derive(Debug, Clone, PartialEq)]
pub struct OmpResult {
    pub window: (usize, usize),
    /// Median distance from the line start, km.
    pub distance: f64,
    /// 2σ/mean·100 over the window, %.
    pub error: f64,
    pub e: bool,
    pub f2: bool,
    pub f3: bool,
    pub phases: [bool; 3],
    /// Section index (0-based) and the distance into it, km.
    pub section: usize,
    pub in_section_km: f64,
    pub support: u32,
    pub text: String,
}

/// Upper median of 0/1 flags: set when at least half of them are.
fn flag_median(v: &[bool]) -> bool {
    let ones = v.iter().filter(|b| **b).count();
    let n = v.len();
    n > 0 && ones >= n - n / 2
}

/// SI-prefixed value (`0x43d960`) with the Cyrillic prefixes of the Russian UI.
pub fn si(v: f64) -> (f64, &'static str) {
    let a = v.abs();
    if v == 0.0 {
        (v, "")
    } else if a < 1e-6 {
        (v * 1e9, "н")
    } else if a < 1e-3 {
        (v * 1e6, "мк")
    } else if a < 1.0 {
        (v * 1e3, "м")
    } else if a > 1e15 {
        (v / 1e15, "П")
    } else if a > 1e12 {
        (v / 1e12, "Т")
    } else if a > 1e9 {
        (v / 1e9, "Г")
    } else if a > 1e6 {
        (v / 1e6, "М")
    } else if a > 1e3 {
        (v / 1e3, "к")
    } else {
        (v, "")
    }
}

/// Statistics, location and text for the window `(c1, c2)`.
pub fn result_for_window(t: &Trace, sections: &[Section], c1: usize, c2: usize) -> OmpResult {
    let last = t.dist.len().saturating_sub(1);
    let lo = c1.min(c2);
    let hi = c1.max(c2).min(last);
    let w = if lo <= hi && !t.dist.is_empty() { lo..=hi } else { 0..=0 };
    let d_win: Vec<f64> = t.dist.get(w.clone()).map(|s| s.to_vec()).unwrap_or_default();
    let distance = median(&d_win);
    let n = d_win.len().max(1) as f64;
    let mean = d_win.iter().sum::<f64>() / n;
    let sigma = (d_win.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n).sqrt();
    let error = 2.0 * sigma / mean * 100.0;
    let fm = |v: &Vec<bool>| flag_median(v.get(w.clone()).unwrap_or(&[]));
    let (e, f2, f3) = (fm(&t.e), fm(&t.f2), fm(&t.f3));
    let phases = [fm(&t.pa), fm(&t.pb), fm(&t.pc)];
    // Section and support (0x49d200).
    let mut r = distance;
    let mut j = 0;
    while j < sections.len() && r > sections[j].length_km {
        r -= sections[j].length_km;
        j += 1;
    }
    let support = if j >= sections.len() {
        1
    } else {
        (f64::from(sections[j].supports) * r / sections[j].length_km).trunc() as u32 + 1
    };
    // Text (0x49d700).
    let total: f64 = sections.iter().map(|s| s.length_km).sum();
    let mut text = "    КЗ отсутствует.".to_string();
    if e || f2 || f3 {
        text = if f3 {
            "    3-х фазное КЗ: фазы "
        } else if f2 {
            "    2-х фазное КЗ: фазы "
        } else {
            "    1-х фазное КЗ: фазы "
        }
        .to_string();
        for (on, name) in phases.iter().zip(["A", "B", "C"]) {
            if *on {
                text.push_str(name);
            }
        }
        if distance > total || distance <= 0.0 {
            text.push_str(" на смежной линии.");
        } else {
            let (v1, p1) = si(distance * 1000.0);
            let (v2, p2) = si(r * 1000.0);
            text.push_str(&format!(
                " на участке № {}, опоры {} - {}.    Расстояние = {v1:.2} {p1}m.     Расстояние от нач. участка = {v2:.2} {p2}m.    Погрешность расчета = {error:.2} %.",
                j + 1,
                support as i64 - 1,
                support
            ));
        }
    }
    OmpResult {
        window: (c1, c2),
        distance,
        error,
        e,
        f2,
        f3,
        phases,
        section: j,
        in_section_km: r,
        support,
        text,
    }
}

/// Interval, window and result for a finished single-ended trace.
pub fn locate(t: &Trace, neutral: NeutralMode, sections: &[Section], p: usize) -> OmpResult {
    let (lo, hi) = fault_interval(t, neutral, p);
    let (c1, c2) = choose_window(&t.dist, &t.inst, lo, hi, p);
    result_for_window(t, sections, c1, c2)
}

/// Inputs of a double-ended analysis: phase voltages and currents at both
/// ends (end 2 from the second oscillogram), and each end's nominal current.
pub struct DoubleEnded<'a> {
    pub u1: [&'a [f64]; 3],
    pub i1: [&'a [f64]; 3],
    pub u2: [&'a [f64]; 3],
    pub i2: [&'a [f64]; 3],
    pub p: usize,
    pub inom1: f64,
    pub inom2: f64,
    pub sections: &'a [Section],
}

/// Negative-sequence component `(A + a²B + aC)/3`.
fn negative(a: Complex64, b: Complex64, c: Complex64) -> Complex64 {
    components(a, b, c).1
}

fn taps_zeq(sec: &Section) -> Option<Complex64> {
    if sec.taps.is_empty() {
        return None;
    }
    let y = sec.taps.iter().fold(Complex64::default(), |s, tp| s + cdiv(ONE, Complex64::new(tp.r0, tp.x0)));
    let z = cdiv(ONE, y);
    (z.re != 0.0 || z.im != 0.0).then_some(z)
}

fn mutual(sec: &Section, k: usize) -> Complex64 {
    let za = Complex64::new(sec.r0_per_km, sec.x0_per_km);
    sec.parallel
        .iter()
        .filter(|pl| pl.rm_per_km != 0.0 || pl.xm_per_km != 0.0)
        .fold(Complex64::default(), |s, pl| {
            let im = pl.current.get(k).copied().unwrap_or_default();
            s + cdiv(Complex64::new(pl.rm_per_km, pl.xm_per_km) * im, za)
        })
}

/// Fault-point voltage seen from end 1 at distance `x` (`0x49b210`).
fn from_end1(secs: &[Section], mut u: Complex64, mut i: Complex64, x: f64, earth: bool, k: usize) -> Complex64 {
    let mut covered = 0.0;
    for sec in secs {
        if covered >= x {
            break;
        }
        let l = sec.length_km.min(x - covered);
        if earth {
            let za = Complex64::new(sec.r0_per_km, sec.x0_per_km) * l;
            u -= za * (i + mutual(sec, k));
            if sec.length_km < x - covered {
                if let Some(zeq) = taps_zeq(sec) {
                    i -= cdiv(u, zeq);
                }
            }
        } else {
            u -= Complex64::new(sec.r1_per_km, sec.x1_per_km) * l * i;
        }
        covered += l;
    }
    u
}

/// Fault-point voltage seen from end 2 at distance `y` from it (`0x49b600`).
fn from_end2(secs: &[Section], mut u: Complex64, mut i: Complex64, y: f64, earth: bool, k: usize) -> Complex64 {
    let mut covered = 0.0;
    for sec in secs.iter().rev() {
        if covered >= y {
            break;
        }
        let l = sec.length_km.min(y - covered);
        if earth {
            if let Some(zeq) = taps_zeq(sec) {
                i += cdiv(u, zeq);
            }
            let za = Complex64::new(sec.r0_per_km, sec.x0_per_km) * l;
            u += za * (i + mutual(sec, k));
        } else {
            u += Complex64::new(sec.r1_per_km, sec.x1_per_km) * l * i;
        }
        covered += l;
    }
    u
}

/// Double-ended analysis (`0x499000`, `0x49a2f0`, `0x49cf00`).
// Sample-index loops over several parallel per-sample arrays, as in the original.
#[allow(clippy::needless_range_loop)]
pub fn double_ended(inp: &DoubleEnded) -> Trace {
    let all = inp.i1.iter().chain(inp.i2.iter()).chain(inp.u1.iter()).chain(inp.u2.iter());
    let nk = all.map(|s| s.len()).min().unwrap_or(0);
    let p = inp.p;
    let ph = |s: &[&[f64]; 3]| -> Vec<Vec<Complex64>> { s.iter().map(|x| omp_phasors(&x[..nk], p)).collect() };
    let (i1, i2, u1, u2) = (ph(&inp.i1), ph(&inp.i2), ph(&inp.u1), ph(&inp.u2));
    let c1: Vec<_> = (0..nk).map(|k| components(i1[0][k], i1[1][k], i1[2][k])).collect();
    let c2: Vec<_> = (0..nk).map(|k| components(i2[0][k], i2[1][k], i2[2][k])).collect();
    let m = |z: Complex64| z.norm();
    let rt2 = 2f64.sqrt();
    let (in1, in2) = (inp.inom1, inp.inom2.abs());
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
        angle: vec![0.0; nk],
    };
    // Detection (0x499000).
    let (mut a1, mut a2, mut n1, mut n2) = (false, false, false, false);
    let (mut e, mut f2, mut f3) = (vec![false; nk], vec![false; nk], vec![false; nk]);
    for k in 1..nk {
        let ((p1, q1, z1), (p2, q2, z2)) = (c1[k], c2[k]);
        a1 = hyst(a1, 4.0 * m(q1) > m(p1), 0.9 * m(p1) > 4.0 * m(q1));
        a2 = hyst(a2, 4.0 * m(q2) > m(p2), 0.9 * m(p2) > 4.0 * m(q2));
        if a1 && a2 {
            if m(p1) <= 0.1 * rt2 * in1 || m(p2) <= 0.1 * rt2 * in2 {
                continue;
            }
            n1 = hyst(n1, m(q1) > 6.0 * m(z1), 5.4 * m(z1) > m(q1));
            n2 = hyst(n2, m(q2) > 6.0 * m(z2), 5.4 * m(z2) > m(q2));
            if n1 && n2 {
                f2[k] = true;
            } else {
                e[k] = true;
            }
        } else {
            f3[k] = m(p1) > 1.1 * rt2 * in1 && m(p2) > 1.1 * rt2 * in2;
        }
    }
    t.e = debounce(&e, p);
    t.f2 = debounce(&f2, p);
    t.f3 = debounce(&f3, p);
    // Phase selection on end-1 currents (as 0x498580).
    let (mut ab, mut ac, mut bc) = (false, false, false);
    for k in 1..nk {
        let (x, y, z) = (m(i1[0][k]), m(i1[1][k]), m(i1[2][k]));
        ab = hyst(ab, x > y, 0.9 * y > x);
        ac = hyst(ac, x > z, 0.9 * z > x);
        bc = hyst(bc, y > z, 0.9 * z > y);
        let (e, f2, f3) = (t.e[k], t.f2[k], t.f3[k]);
        t.pa[k] = f3 || f2 && (ab || ac) || e && ab && ac;
        t.pb[k] = f3 || f2 && (!ab || bc) || e && !ab && bc;
        t.pc[k] = f3 || f2 && (!ac || !bc) || e && !ac && !bc;
    }
    // Loop current (0x49a2f0).
    let raw = |c: usize, k: usize| inp.i1[c][k];
    for k in p..nk {
        if t.f2[k] || t.f3[k] {
            t.inst[k] = if t.pa[k] && t.pb[k] {
                raw(0, k) - raw(1, k)
            } else if t.pb[k] && t.pc[k] {
                raw(1, k) - raw(2, k)
            } else if t.pa[k] && t.pc[k] {
                raw(2, k) - raw(0, k)
            } else {
                1.0
            };
        } else if t.e[k] {
            t.inst[k] = if t.pa[k] {
                raw(0, k)
            } else if t.pb[k] && t.pc[k] {
                raw(1, k) - raw(2, k)
            } else {
                1.0
            };
        }
    }
    // Distance search (0x49cf00).
    let total: f64 = inp.sections.iter().map(|s| s.length_km).sum();
    for k in 0..nk {
        let earth_only = t.e[k] && !(t.f2[k] || t.f3[k]);
        if !(t.e[k] || t.f2[k] || t.f3[k]) {
            continue;
        }
        let (ua, ia, ub, ib) = if earth_only {
            (
                (u1[0][k] + u1[1][k] + u1[2][k]) / 3.0,
                c1[k].2,
                (u2[0][k] + u2[1][k] + u2[2][k]) / 3.0,
                c2[k].2,
            )
        } else {
            (negative(u1[0][k], u1[1][k], u1[2][k]), c1[k].1, negative(u2[0][k], u2[1][k], u2[2][k]), c2[k].1)
        };
        let f = |x: f64| {
            (from_end1(inp.sections, ua, ia, x, earth_only, k).norm()
                - from_end2(inp.sections, ub, ib, total - x, earth_only, k).norm())
            .abs()
        };
        let (mut x, mut h) = (0.0, total / 2.0);
        t.dist[k] = if h <= 0.01 {
            0.0
        } else {
            loop {
                let sgn = if f(x) > f(x + 0.01) { 1.0 } else { -1.0 };
                let xn = x + sgn * h;
                h = (xn - x).abs() / 2.0;
                if xn < 0.0 {
                    break 0.0;
                }
                if h > 0.01 {
                    x = xn;
                } else {
                    break xn;
                }
            }
        };
        let arg = |u: Complex64, i: Complex64| cdiv(u, i).im.atan2(cdiv(u, i).re);
        t.angle[k] = arg(ua, ia) - arg(ub, ib);
    }
    t
}

/// Interval, window, direction check and result for a double-ended trace.
pub fn locate_double(t: &Trace, sections: &[Section], p: usize) -> OmpResult {
    let (lo, hi) = fault_interval(t, NeutralMode::Grounded, p);
    let (c1, c2) = choose_window(&t.dist, &t.inst, lo, hi, p);
    result_double_for_window(t, sections, c1, c2)
}

/// Double-ended result for a window: if the median angle difference over it
/// lies within ±90° (strict) every distance is zeroed first (`0x49d3a0`).
pub fn result_double_for_window(t: &Trace, sections: &[Section], c1: usize, c2: usize) -> OmpResult {
    let (a, b) = (c1.min(c2), c1.max(c2).min(t.angle.len().saturating_sub(1)));
    let ang = median(t.angle.get(a..=b).unwrap_or(&[]));
    let half_pi = std::f64::consts::FRAC_PI_2;
    if -half_pi < ang && ang < half_pi {
        let mut zeroed = t.clone();
        zeroed.dist.iter_mut().for_each(|d| *d = 0.0);
        return result_for_window(&zeroed, sections, c1, c2);
    }
    result_for_window(t, sections, c1, c2)
}
