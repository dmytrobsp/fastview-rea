//! Channel signal methods pinned to the decompiled original (ledger L6).

use fvb_core::signal::*;
use std::f64::consts::{PI, SQRT_2};

fn sine(n: usize, len: usize, amp_rms: f64, phase_deg: f64) -> Vec<f64> {
    (0..len)
        .map(|i| SQRT_2 * amp_rms * (2.0 * PI * i as f64 / n as f64 + phase_deg.to_radians()).sin())
        .collect()
}

/// `N = rate / trunc(f)` with integer division (0x427062).
#[test]
fn samples_per_period_rule() {
    assert_eq!(samples_per_period(1000.0, 50.0), Some(20));
    assert_eq!(samples_per_period(4800.0, 50.0), Some(96));
    // Both operands are truncated before an integer division: rounding the
    // quotient would give 24 here.
    assert_eq!(samples_per_period(1199.9, 50.0), Some(23));
    assert_eq!(samples_per_period(1000.0, 49.9), Some(20));
    assert_eq!(samples_per_period(1100.0, 49.9), Some(22), "1100/49 = 22.4 -> 22 (1100/49.9 ≈ 22.04)");
    assert_eq!(samples_per_period(1000.0, 0.5), None);
}

/// Phasors are RMS-scaled and sine-referenced at the newest sample: a sine
/// of phase ψ at sample i reads X∠ψ(i) (FUN_00426fc0).
#[test]
fn phasor_is_sine_referenced_at_the_newest_sample() {
    let n = 20;
    let x = sine(n, 100, 10.0, 30.0);
    let p = sliding_phasors(&x, n);
    assert_eq!(p[n - 2], num_complex::Complex64::new(0.0, 0.0), "zero before a full window");
    for i in [n - 1, 37, 99] {
        let want = 30.0 + 360.0 * i as f64 / n as f64;
        let got = p[i].arg().to_degrees();
        let diff = (got - want).rem_euclid(360.0);
        assert!(diff < 1e-9 || diff > 360.0 - 1e-9, "i={i}: {got} vs {want}");
        assert!((p[i].norm() - 10.0).abs() < 1e-9);
    }
}

/// `fault` = RMS of the one-period-lag second difference (FUN_00427670): zero
/// for a steady sine, non-zero after an amplitude step, zero before 2N.
#[test]
fn fault_component_uses_a_one_period_lag() {
    let n = 20;
    let mut x = sine(n, 200, 10.0, 0.0);
    let f = sliding_fault(&x, n);
    assert!(f.iter().all(|v| v.abs() < 1e-9));
    for v in x.iter_mut().skip(120) {
        *v *= 3.0;
    }
    let f = sliding_fault(&x, n);
    assert!(f[119].abs() < 1e-9 && f[125] > 1.0);
    // Terms with s < 2N are 0, so nothing shows before i = 2N; a spike at
    // s = 5 reappears through the x[s−2N] term at s = 2N + 5.
    let mut y = vec![0.0; 100];
    y[5] = 100.0;
    let f = sliding_fault(&y, n);
    assert!(f[..2 * n].iter().all(|v| *v == 0.0));
    assert!(f[2 * n + 5] > 0.0);
}

/// `fourie:` = RMS magnitude of harmonic h over the phasor window; h=0 is |a0|.
#[test]
fn harmonic_magnitudes() {
    let n = 32;
    let mut x = sine(n, 64, 10.0, 0.0);
    for (i, v) in x.iter_mut().enumerate() {
        *v += SQRT_2 * 2.0 * (2.0 * PI * 3.0 * i as f64 / n as f64).cos() + 1.5;
    }
    let i = 63;
    assert!((harmonic_rms(&x, i, n, 1) - 10.0).abs() < 1e-9);
    assert!((harmonic_rms(&x, i, n, 3) - 2.0).abs() < 1e-9);
    assert!((harmonic_rms(&x, i, n, 0) - 1.5).abs() < 1e-9);
    assert_eq!(harmonic_rms(&x, n - 2, n, 1), 0.0, "zero before a full window");
}

/// Frequency-diagram spectrum over P periods: bin P is the fundamental, the
/// harmonic-1 gain lands on bin 1 even then (0x426510 quirk).
#[test]
fn spectrum_over_periods() {
    let n = 20;
    let x = sine(n, 100, 10.0, 0.0);
    let sp = spectrum(&x, 99, n, 1, 10, 1.0);
    assert!((sp[1] - 10.0).abs() < 1e-9 && sp[2].abs() < 1e-9);
    let sp = spectrum(&x, 99, n, 2, 10, 1.5);
    assert!((sp[2] - 10.0).abs() < 1e-9, "bin P = fundamental");
    assert!(sp[1].abs() < 1e-9, "gain applied to an empty bin 1");
    let sp1 = spectrum(&x, 99, n, 1, 3, 1.5);
    assert!((sp1[1] - 15.0).abs() < 1e-9);
    assert_eq!(spectrum(&x, 10, n, 1, 3, 1.0), vec![0.0; 4]);
    // phasor_at matches the sliding series.
    let ph = sliding_phasors(&x, n);
    assert_eq!(phasor_at(&x, 57, n), ph[57]);
}

/// #72: every sin / cos of the DFT, the phasor and the harmonic is the original's runtime's, and the
/// angle is formed as the original does (`0x426510`, `0x426240`: `j·(k·2π)/w`, the harmonic's
/// `k·2π` first, so it differs from `(j·k)·2π/w` for some k above 10). A flat window makes every
/// harmonic rounding noise, so a single last-bit difference in an angle's sine shows in the result.
#[test]
fn dft_angles_and_trig_are_the_originals() {
    use fvb_core::trig::{cos, sin};
    let tau = 2.0 * std::f64::consts::PI;
    let mut checked = 0;
    for w in [20usize, 40, 80, 100, 120, 160] {
        let x = vec![1234.5678_f64; w];
        for k in 1..=30usize {
            let (mut c, mut s) = (0.0, 0.0);
            for (j, v) in (1..=w).zip(&x) {
                let th = j as f64 * (k as f64 * tau) / w as f64;
                c += v * cos(th);
                s += v * sin(th);
            }
            let want = 2f64.sqrt() * (c * c + s * s).sqrt() / w as f64;
            let got = spectrum(&x, w - 1, w, 1, 30, 1.0)[k];
            assert_eq!(got.to_bits(), want.to_bits(), "spectrum w={w} k={k}");
            let rms = harmonic_rms(&x, w - 1, w, k);
            let want = (c * c + s * s).sqrt() * 2f64.sqrt() / w as f64;
            assert_eq!(rms.to_bits(), want.to_bits(), "harmonic w={w} k={k}");
            checked += 1;
        }
    }
    assert_eq!(checked, 180);
    // The sliding fundamental: (k·2π)/n, k = 1..=n.
    let x = vec![1234.5678_f64; 80];
    let (mut re, mut im) = (0.0, 0.0);
    for (k, v) in (1..=40usize).zip(&x[40..]) {
        let th = tau * k as f64 / 40.0;
        re += sin(th) * v;
        im += cos(th) * v;
    }
    let want = num_complex::Complex64::new(re * (2f64.sqrt() / 40.0), im * (2f64.sqrt() / 40.0));
    assert_eq!(phasor_at(&x, 79, 40), want);
}

/// #107: the operator `a` is (−0.5, √3·0.5) with the root taken by `sqrt` (`0x41d800`, `0x420650`), not the polar
/// `1∠120°`, whose cosine and sine round differently.
#[test]
fn the_operator_a_is_the_originals_pair() {
    let a = fvb_core::signal::operator_a();
    assert_eq!((a.re.to_bits(), a.im.to_bits()), ((-0.5f64).to_bits(), (3f64.sqrt() * 0.5).to_bits()));
    let polar = num_complex::Complex64::from_polar(1.0, 120f64.to_radians());
    assert_ne!((a.re.to_bits(), a.im.to_bits()), (polar.re.to_bits(), polar.im.to_bits()), "the test tells the two forms apart");
}
