//! #72: each site of the signal maths that calls the original's runtime sin / cos does so through
//! `fvb_core::trig`. Each test builds its reference from `trig` and a second one from std, asserts the
//! site equals the first bit for bit, and asserts that the two references differ somewhere in the
//! scanned range (so a site reverted to std fails).

use fvb_core::expr::{eval_str, Context};
use fvb_core::freqcorr::FreqCorr;
use fvb_core::signal::{phasor_at, sliding_phasors};
use fvb_core::trig;
use num_complex::Complex64;

type Fns = (fn(f64) -> f64, fn(f64) -> f64);
const TRIG: Fns = (trig::sin, trig::cos);
const STD: Fns = (f64::sin, f64::cos);

/// Synthetic, non-flat samples.
fn samples(len: usize, seed: u64) -> Vec<f64> {
    let mut z = seed;
    (0..len)
        .map(|i| {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            100.0 * (i as f64 * 0.31).sin() + (z % 10_000) as f64 * 0.017
        })
        .collect()
}

fn phasor_ref(x: &[f64], i: usize, n: usize, f: Fns) -> Complex64 {
    let tau = 2.0 * std::f64::consts::PI;
    let (mut re, mut im) = (0.0, 0.0);
    for k in 1..=n {
        let v = x[i + k - n];
        let th = tau * k as f64 / n as f64;
        re += (f.0)(th) * v;
        im += (f.1)(th) * v;
    }
    let scale = 2f64.sqrt() / n as f64;
    Complex64::new(re * scale, im * scale)
}

#[test]
fn phasors_use_the_runtime_sin_cos() {
    let (mut dre, mut dim) = (0, 0);
    for n in 8..260usize {
        let x = samples(3 * n, n as u64 + 11);
        let i = 3 * n - 1;
        let (a, b) = (phasor_ref(&x, i, n, TRIG), phasor_ref(&x, i, n, STD));
        dre += usize::from(a.re != b.re);
        dim += usize::from(a.im != b.im);
        assert_eq!(phasor_at(&x, i, n), a, "phasor_at n={n}");
        assert_eq!(sliding_phasors(&x, n)[i], a, "sliding_phasors n={n}");
    }
    // A site reverted to std must fail somewhere: the sin part (re) and the cos part (im) each need a
    // grid angle where std's result differs from the runtime's. The phasor sums of the 8..260 scan
    // above show it in the sin part only on some libms (mingw's cos agrees with the port at every
    // one of those angles, though not on the grid at large), so the guard scans the DFT's angle grid
    // for n up to 1500 directly, not through the sums, and asks the sums only for one part.
    let tau = 2.0 * std::f64::consts::PI;
    let (mut gs, mut gc) = (0, 0);
    for n in 8..1500usize {
        for k in 1..=n {
            let th = tau * k as f64 / n as f64;
            gs += usize::from(trig::sin(th).to_bits() != th.sin().to_bits());
            gc += usize::from(trig::cos(th).to_bits() != th.cos().to_bits());
        }
    }
    assert!(gs > 0 && gc > 0, "std differs from the runtime's sin and cos on the DFT's angle grid ({gs}, {gc}); phasor sums ({dre}, {dim})");
    assert!(dre + dim > 0, "the std-built reference differs in at least one part at some n ({dre}, {dim})");
}

#[test]
fn the_corrected_harmonic_uses_the_runtime_sin_cos() {
    // 0x4267e0: weights dt/2, dt, dt/2 + τ − τ²/2dt, τ²/2dt; angle j·(((n/p)·2π)·f·dt).
    fn reference(x: &[f64], idx: usize, fs: f64, f: f64, fns: Fns) -> Complex64 {
        let (dt, p) = (1.0 / fs, 1.0);
        let m = p * fs / f;
        let k = m.floor() as usize;
        let tau = p / f - k as f64 / fs;
        let th = 2.0 * std::f64::consts::PI * 1.0 * f * dt;
        let mut s = Complex64::default();
        for j in 0..=k + 1 {
            let tail = tau * tau / (2.0 * dt);
            let w = if j == 0 {
                dt / 2.0
            } else if j < k {
                dt
            } else if j == k {
                dt / 2.0 + tau - tail
            } else {
                tail
            };
            let v = w * x[idx - j];
            let a = j as f64 * th;
            s += Complex64::new(-(fns.0)(a), (fns.1)(a)) * v;
        }
        s * (2f64.sqrt() * f / p)
    }
    let (mut dre, mut dim) = (0, 0);
    let pairs = (0..120).map(|i| (900.0 + 31.0 * i as f64, 47.0 + 0.043 * i as f64));
    for (fs, f) in pairs {
        let x = samples(400, fs as u64);
        let freq = vec![f; 400];
        let c = FreqCorr { chan: usize::MAX, freq: &freq, fs };
        for idx in [150usize, 211, 399] {
            let (a, b) = (reference(&x, idx, fs, f, TRIG), reference(&x, idx, fs, f, STD));
            dre += usize::from(a.re != b.re);
            dim += usize::from(a.im != b.im);
            assert_eq!(c.harmonic(&x, idx, 1.0, false, 1.0), a, "fs={fs} f={f} idx={idx}");
        }
    }
    assert!(dre > 0 && dim > 0, "the std-built reference differs in both parts ({dre}, {dim})");
}

#[test]
fn the_omp_phasor_uses_the_runtime_sin_cos() {
    let (mut dre, mut dim) = (0, 0);
    for p in (8..140usize).chain([160, 240]) {
        let x = samples(4 * p, p as u64 + 5);
        let tau = 2.0 * std::f64::consts::PI;
        let reference = |k: usize, f: Fns| {
            let (mut re, mut im) = (0.0, 0.0);
            for j in 0..p {
                let th = j as f64 * tau / p as f64;
                let v = x[k - p + j];
                re += (f.0)(th) * v;
                im += (f.1)(th) * v;
            }
            let scale = 2.0 / p as f64;
            Complex64::new(re * scale, im * scale)
        };
        let got = fvb_core::omp::omp_phasors(&x, p);
        for k in [p, 2 * p, 4 * p - 1] {
            let (a, b) = (reference(k, TRIG), reference(k, STD));
            dre += usize::from(a.re != b.re);
            dim += usize::from(a.im != b.im);
            assert_eq!(got[k], reference(k, TRIG), "p={p} k={k}");
        }
    }
    assert!(dre > 0 && dim > 0, "the std-built reference differs in both parts ({dre}, {dim})");
}

#[test]
fn formula_sin_cos_use_the_runtime_sin_cos() {
    let ctx = Context::new();
    let rad = |d: f64| d * std::f64::consts::PI / 180.0;
    let (mut ds, mut dc) = (0, 0);
    for d in 1..=720 {
        let d = d as f64 * 0.5;
        let (s, c) = (eval_str(&format!("sin:{d}"), &ctx).unwrap(), eval_str(&format!("cos:{d}"), &ctx).unwrap());
        assert_eq!(s.to_bits(), trig::sin(rad(d)).to_bits(), "sin:{d}");
        assert_eq!(c.to_bits(), trig::cos(rad(d)).to_bits(), "cos:{d}");
        ds += usize::from(trig::sin(rad(d)).to_bits() != rad(d).sin().to_bits());
        dc += usize::from(trig::cos(rad(d)).to_bits() != rad(d).cos().to_bits());
    }
    assert!(ds > 0 && dc > 0, "std differs from the runtime's at some angle ({ds}, {dc})");
}

// ---------------------------------------------------------------------------------------------
// #106 / #121: each site's sin half and cos half on their own, on every libm.
//
// The scans above ask std to differ from the port somewhere in a range, which depends on the
// libm (mingw's cos agrees with the port on much of what glibc's does not). These tests instead
// search, per site, for a parameter where a reference with ONLY the sin (or ONLY the cos) put
// back to std already differs from the port-built reference, take impulse samples so a single
// term carries the angle, and assert the site equals the port-built reference there. A site
// reverted to std in either half, or both, then fails on every platform; a libm that never
// differs in a half over the whole candidate range fails the search, loudly.

use fvb_core::expr::{Channels, Expr};

const SIN_STD: Fns = (f64::sin, trig::cos);
const COS_STD: Fns = (trig::sin, f64::cos);

fn halves<P: Copy + std::fmt::Debug, R: PartialEq + std::fmt::Debug>(what: &str, cands: impl Iterator<Item = P> + Clone, reference: impl Fn(P, Fns) -> R, site: impl Fn(P) -> R) {
    let find = |mixed: Fns| cands.clone().find(|&p| reference(p, mixed) != reference(p, TRIG));
    let s = find(SIN_STD).unwrap_or_else(|| panic!("{what}: no candidate tells the sin half from std's"));
    let c = find(COS_STD).unwrap_or_else(|| panic!("{what}: no candidate tells the cos half from std's"));
    for p in [s, c] {
        assert_eq!(site(p), reference(p, TRIG), "{what} at {p:?}");
    }
}

/// Zeros with one sample `v` at `at`.
fn impulse(len: usize, at: usize, v: f64) -> Vec<f64> {
    let mut x = vec![0.0; len];
    x[at] = v;
    x
}

/// All (n, k) of the DFT's angle grid, k = 1..=n.
fn grid(ns: std::ops::Range<usize>) -> impl Iterator<Item = (usize, usize)> + Clone {
    ns.flat_map(|n| (1..=n).map(move |k| (n, k)))
}

#[test]
fn phasor_at_and_sliding_phasors_use_both_halves() {
    let site_x = |(n, k): (usize, usize)| (impulse(2 * n, 2 * n - 1 + k - n, 1.3), 2 * n - 1);
    halves(
        "phasor_at",
        grid(8..160),
        |p, f| {
            let (x, i) = site_x(p);
            phasor_ref(&x, i, p.0, f)
        },
        |p| {
            let (x, i) = site_x(p);
            phasor_at(&x, i, p.0)
        },
    );
    halves(
        "sliding_phasors",
        grid(8..160),
        |p, f| {
            let (x, i) = site_x(p);
            phasor_ref(&x, i, p.0, f)
        },
        |p| {
            let (x, i) = site_x(p);
            sliding_phasors(&x, p.0)[i]
        },
    );
}

#[test]
fn the_omp_phasor_uses_both_halves() {
    let tau = 2.0 * std::f64::consts::PI;
    // Window [k−p, k−1] at k = p, the impulse at j.
    halves(
        "omp_phasors",
        grid(8..160),
        |(p, j), f| {
            let th = j as f64 * tau / p as f64;
            let (re, im) = ((f.0)(th) * 1.3, (f.1)(th) * 1.3);
            let scale = 2.0 / p as f64;
            Complex64::new(re * scale, im * scale)
        },
        |(p, j)| fvb_core::omp::omp_phasors(&impulse(2 * p, j, 1.3), p)[p],
    );
}

/// The tracked ОМП phasor (`0x477280` with a mode): 1 ms steps, the own frequency vector `f`, the
/// impulse `m` steps back from `i`; its term is at 2π·(phase after m steps), and it is also the
/// last sample's corrected term when the phase reaches 1 there.
#[test]
fn the_tracked_omp_phasor_uses_both_halves() {
    let tau = 2.0 * std::f64::consts::PI;
    let (len, i, n) = (160usize, 100usize, 20usize); // m back: the impulse also lands on the last sample
    let t_us: Vec<i64> = (0..len as i64).map(|k| k * 1000).collect();
    // `lp` computes the loop's terms, `fix` the last sample's corrected one.
    let reference = |(f, m): (f64, usize), lp: Fns, fix: Fns| {
        let x = impulse(len, i - m, 1.3);
        let (mut sum, mut inv, mut count) = (Complex64::default(), 0.0, 0usize);
        let (mut s, mut phase) = (i, 0.0f64);
        let mut last = None;
        while phase < 1.0 && s > 0 {
            let dt = (t_us[s] - t_us[s - 1]) as f64 * 1e-6;
            let th = tau * phase;
            sum += Complex64::new(-(lp.0)(th) * x[s], (lp.1)(th) * x[s]) * dt;
            last = Some((s, th, phase, dt));
            phase += dt * f;
            inv += 1.0 / f;
            count += 1;
            s -= 1;
        }
        if let Some((sl, th, before, dt)) = last {
            sum += Complex64::new(-(fix.0)(th) * x[sl], (fix.1)(th) * x[sl]) * ((1.0 - before) / f - dt);
        }
        sum * (2.0 / (inv / count as f64))
    };
    // The impulse on the loop's last sample (the one the phase passes 1 at): its corrected weight
    // (1 − phase before)/f − Δt is a fraction of −Δt, so the correction's sin / cos show in the sum.
    let spread = (0..400).flat_map(|a| (1..=26usize).map(move |m| (40.0 + 0.0371 * a as f64, m)));
    let last_step = |f: f64| {
        let (mut phase, mut m) = (0.0f64, 0usize);
        while phase < 1.0 {
            phase += 1000.0 * 1e-6 * f;
            m += 1;
        }
        (f, m - 1)
    };
    let last = (0..40_000).map(move |a| last_step(40.0 + 0.0173 * a as f64));
    let site = |(f, m): (f64, usize)| fvb_core::omp::omp_phasors_tracked(&impulse(len, i - m, 1.3), n, &t_us, &vec![f; len], &[])[i];
    halves("omp_phasors_tracked, the loop", spread, |p, f| reference(p, f, TRIG), site);
    halves("omp_phasors_tracked, the last sample's correction", last, |p, f| reference(p, TRIG, f), site);
}

/// Two channels whose phasor angles move with the index: a: 0, b: 0.0137·idx.
struct Rot;

impl Rot {
    fn z(ch: usize, idx: usize) -> Complex64 {
        Complex64::from_polar(1.0 + ch as f64, if ch == 0 { 0.4 } else { 0.0137 * idx as f64 })
    }
}

impl Channels for Rot {
    fn resolve(&self, name: &str) -> Option<usize> {
        ["ia", "ib"].iter().position(|n| *n == name.to_lowercase())
    }
    fn sample(&self, _: usize, _: usize) -> f64 {
        0.0
    }
    fn phasor(&self, ch: usize, idx: usize) -> Complex64 {
        Rot::z(ch, idx)
    }
    fn rms(&self, _: usize, _: usize) -> f64 {
        1.0
    }
    fn a0(&self, _: usize, _: usize) -> f64 {
        0.0
    }
    fn fourie(&self, _: usize, _: i32, _: usize) -> f64 {
        0.0
    }
    fn fault(&self, _: usize, _: usize) -> f64 {
        0.0
    }
    fn frequency(&self, _: usize) -> f64 {
        50.0
    }
    fn warmup(&self) -> usize {
        4
    }
}

#[test]
fn cosui_uses_the_runtime_cos() {
    // cosui:(x, y) = cos(arg y − arg x): one cos, no sin.
    let reference = |idx: usize, f: Fns| {
        let (x, y) = (Rot::z(0, idx), Rot::z(1, idx));
        (f.1)(y.im.atan2(y.re) - x.im.atan2(x.re))
    };
    let site = |idx: usize| Expr::parse_channels("cosui:(ia, ib)", &Rot).unwrap().eval_at(&Context::new(), &Rot, idx).unwrap();
    let find = |mixed: Fns| (0..40_000usize).find(|&k| reference(k, mixed).to_bits() != reference(k, TRIG).to_bits());
    let k = find(STD).expect("a cos differing from std's among the angle differences");
    assert_eq!(site(k).to_bits(), reference(k, TRIG).to_bits(), "cosui at {k}");
}

/// The polar pair of the loop impedance `zk:` (`0x41a110`): `r·cos(deg)`, `r·sin(deg)` for each of the
/// two compensation terms; Z = U / (I + Ia·Za + Ib·Zb), divided as `0x420ca0`.
#[test]
fn the_polar_impedance_pair_uses_both_halves() {
    fn cdiv(a: Complex64, b: Complex64) -> Complex64 {
        if b.re.abs() < b.im.abs() {
            let w = b.re / b.im;
            let d = b.im + w * b.re;
            Complex64::new((a.re * w + a.im) / d, (a.im * w - a.re) / d)
        } else {
            let w = b.im / b.re;
            let d = b.re + w * b.im;
            Complex64::new((a.re + a.im * w) / d, (a.im - a.re * w) / d)
        }
    }
    let rad = |d: f64| d * std::f64::consts::PI / 180.0;
    let (ia, ib) = (Rot::z(0, 0), Rot::z(1, 0));
    for second in [false, true] {
        let reference = |deg: f64, f: Fns| {
            let polar = |r: f64, d: f64| Complex64::new(r * (f.1)(rad(d)), r * (f.0)(rad(d)));
            let (za, zb) = if second { (polar(0.0, 0.0), polar(1.0, deg)) } else { (polar(1.0, deg), polar(0.0, 0.0)) };
            cdiv(ia, ib + ia * za + ib * zb)
        };
        let site = |deg: f64| {
            let src = if second { format!("zk:(ia, ib, ia, ib, 0, 1, 0, {deg}, 0.5)") } else { format!("zk:(ia, ib, ia, ib, 1, 0, {deg}, 0, 0.5)") };
            Expr::parse_channels(&src, &Rot).unwrap().eval_sample(&Context::new(), &Rot, 0).unwrap().1.unwrap()
        };
        halves(if second { "zk: second pair" } else { "zk: first pair" }, (1..60_000).map(|j| j as f64 * 0.0123), |p, f| (reference(p, f).re.to_bits(), reference(p, f).im.to_bits()), |p| (site(p).re.to_bits(), site(p).im.to_bits()));
    }
}

/// The corrected harmonic (`0x4267e0`) with the impulse at `x[idx − j]`: its term is `−sin(j·θ)`,
/// `cos(j·θ)` times the weight of step j.
#[test]
fn the_corrected_harmonic_uses_both_halves() {
    let idx = 300usize;
    let reference = |(fs, f, j): (f64, f64, usize), fns: Fns| {
        let (dt, p) = (1.0 / fs, 1.0);
        let k = (p * fs / f).floor() as usize;
        let tau = p / f - k as f64 / fs;
        let th = 2.0 * std::f64::consts::PI * 1.0 * f * dt;
        let tail = tau * tau / (2.0 * dt);
        let w = if j < k {
            dt
        } else if j == k {
            dt / 2.0 + tau - tail
        } else {
            tail
        };
        let v = w * 1.3;
        let a = j as f64 * th;
        Complex64::new(-(fns.0)(a), (fns.1)(a)) * v * (2f64.sqrt() * f / p)
    };
    let cands = (0..120).flat_map(|a| {
        let (fs, f) = (900.0 + 31.0 * a as f64, 47.0 + 0.043 * a as f64);
        let k = (fs / f).floor() as usize;
        (1..=k + 1).map(move |j| (fs, f, j))
    });
    halves("harmonic", cands, reference, |(fs, f, j)| {
        let freq = vec![f; 400];
        let c = FreqCorr { chan: usize::MAX, freq: &freq, fs };
        c.harmonic(&impulse(400, idx - j, 1.3), idx, 1.0, false, 1.0)
    });
}

/// `sin:` and `cos:` of the formula language, each half on its own.
#[test]
fn formula_sin_and_cos_use_both_halves() {
    let rad = |d: f64| d * std::f64::consts::PI / 180.0;
    let ctx = Context::new();
    let find = |mixed: Fns| (1..20_000).map(|d| d as f64 * 0.037).find(|&d| (mixed.0)(rad(d)).to_bits() != trig::sin(rad(d)).to_bits() || (mixed.1)(rad(d)).to_bits() != trig::cos(rad(d)).to_bits());
    let (s, c) = (find(SIN_STD).expect("sin differs"), find(COS_STD).expect("cos differs"));
    for d in [s, c] {
        assert_eq!(eval_str(&format!("sin:{d}"), &ctx).unwrap().to_bits(), trig::sin(rad(d)).to_bits(), "sin:{d}");
        assert_eq!(eval_str(&format!("cos:{d}"), &ctx).unwrap().to_bits(), trig::cos(rad(d)).to_bits(), "cos:{d}");
    }
}

/// `tg:` and `ctg:` (#107) call the runtime's tan: `tan(v·π/180)` and `1 / tan(v·π/180)`.
#[test]
fn formula_tg_and_ctg_use_the_runtime_tan() {
    let rad = |d: f64| d * std::f64::consts::PI / 180.0;
    let ctx = Context::new();
    let (mut dt, mut dc) = (0, 0);
    for d in 1..=14_400 {
        let d = d as f64 * 0.05;
        let (a, b) = (eval_str(&format!("tg:{d}"), &ctx).unwrap(), eval_str(&format!("ctg:{d}"), &ctx).unwrap());
        assert_eq!(a.to_bits(), trig::tan(rad(d)).to_bits(), "tg:{d}");
        assert_eq!(b.to_bits(), (1.0 / trig::tan(rad(d))).to_bits(), "ctg:{d}");
        dt += usize::from(trig::tan(rad(d)).to_bits() != rad(d).tan().to_bits());
        dc += usize::from((1.0 / trig::tan(rad(d))).to_bits() != (1.0 / rad(d).tan()).to_bits());
    }
    assert!(dt > 0 && dc > 0, "std's tan differs from the runtime's at some angle ({dt}, {dc})");
}
