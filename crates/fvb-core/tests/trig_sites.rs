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
    let mut differ = 0;
    for n in 8..70usize {
        let x = samples(3 * n, n as u64 + 11);
        let i = 3 * n - 1;
        let (a, b) = (phasor_ref(&x, i, n, TRIG), phasor_ref(&x, i, n, STD));
        differ += usize::from(a != b);
        assert_eq!(phasor_at(&x, i, n), a, "phasor_at n={n}");
        assert_eq!(sliding_phasors(&x, n)[i], a, "sliding_phasors n={n}");
    }
    assert!(differ > 0, "the std-built reference differs at some n ({differ})");
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
    let mut differ = 0;
    for (fs, f) in [(1000.0, 50.0), (1600.0, 50.0), (2000.0, 49.7), (3200.0, 50.3), (4000.0, 50.0), (950.0, 47.0)] {
        let x = samples(400, fs as u64);
        let freq = vec![f; 400];
        let c = FreqCorr { chan: usize::MAX, freq: &freq, fs };
        for idx in [150usize, 211, 399] {
            let (a, b) = (reference(&x, idx, fs, f, TRIG), reference(&x, idx, fs, f, STD));
            differ += usize::from(a != b);
            assert_eq!(c.harmonic(&x, idx, 1.0, false, 1.0), a, "fs={fs} f={f} idx={idx}");
        }
    }
    assert!(differ > 0, "the std-built reference differs somewhere ({differ})");
}

#[test]
fn the_omp_phasor_uses_the_runtime_sin_cos() {
    let mut differ = 0;
    for p in [19usize, 27, 29, 34, 40, 80] {
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
            differ += usize::from(reference(k, TRIG) != reference(k, STD));
            assert_eq!(got[k], reference(k, TRIG), "p={p} k={k}");
        }
    }
    assert!(differ > 0, "the std-built reference differs somewhere ({differ})");
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
