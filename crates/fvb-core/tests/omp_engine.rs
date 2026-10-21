//! The per-sample ОМП engine pinned to the original's behaviour
//! (`notes/OMP_SPEC.md`, ledger L7).

use fvb_core::omp::*;
use num_complex::Complex64;

const P: usize = 20;
const LEN: usize = 200;

/// Samples whose ОМП phasor is proportional to `c` (sine reference).
fn wave(c: Complex64) -> Vec<f64> {
    (0..LEN)
        .map(|k| (c * Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI * k as f64 / P as f64)).im)
        .collect()
}

fn section(len: f64, r1: f64, x1: f64, r0: f64, x0: f64) -> Section {
    Section {
        r_per_km: r1,
        x_per_km: x1,
        length_km: len,
        r0_per_km: r0,
        x0_per_km: x0,
        r1_per_km: r1,
        x1_per_km: x1,
        ..Default::default()
    }
}

fn settled(t: &Trace) -> f64 {
    *t.dist.last().unwrap()
}

/// A bolted A-to-earth fault 23 km out: Ua = Z1·d·(Ia + K·I0). Detected as an
/// earth fault on phase A; the I0-polarised formula recovers d exactly.
#[test]
fn grounded_single_phase_fault() {
    let sec = [section(60.0, 0.12, 0.40, 0.30, 1.20)];
    let ia = Complex64::new(800.0, -300.0);
    let i0 = ia / 3.0;
    let z1 = Complex64::new(0.12, 0.40);
    let k = Complex64::new(0.30, 1.20) / z1 - 1.0;
    let ua = z1 * 23.0 * (ia + k * i0);
    let healthy = Complex64::from_polar(57_000.0, -2.0);
    let (u, i) = (
        [wave(ua), wave(healthy), wave(healthy * Complex64::from_polar(1.0, 2.1))],
        [wave(ia), vec![0.0; LEN], vec![0.0; LEN]],
    );
    let t = single_ended(&SingleEnded {
        neutral: NeutralMode::Grounded,
        u: [&u[0], &u[1], &u[2]],
        i: [&i[0], &i[1], &i[2]],
        p: P,
        inom: 600.0,
        sections: &sec,
    });
    let k = LEN - 1;
    assert!(t.e[k] && !t.f2[k] && !t.f3[k]);
    assert!(t.pa[k] && !t.pb[k] && !t.pc[k]);
    assert!((settled(&t) - 23.0).abs() < 1e-6, "{}", settled(&t));
}

/// A–B fault 15 km out across two sections (10 km + 30 km of a different
/// conductor): the voltage across section 1 is dropped before section 2.
#[test]
fn grounded_two_phase_fault_walks_sections() {
    let s1 = section(10.0, 0.10, 0.35, 0.3, 1.0);
    let s2 = section(30.0, 0.20, 0.42, 0.4, 1.3);
    let sec = [s1.clone(), s2.clone()];
    let iab = Complex64::new(1500.0, -900.0);
    let (z1a, z1b) = (Complex64::new(0.10, 0.35), Complex64::new(0.20, 0.42));
    let uab = (z1a * 10.0 + z1b * 5.0) * iab;
    let ua = Complex64::from_polar(30_000.0, 0.0);
    let ub = ua - uab;
    let (u, i) = (
        [wave(ua), wave(ub), wave(Complex64::from_polar(57_000.0, 2.0))],
        [wave(iab / 2.0), wave(-iab / 2.0), vec![0.0; LEN]],
    );
    let t = single_ended(&SingleEnded {
        neutral: NeutralMode::Grounded,
        u: [&u[0], &u[1], &u[2]],
        i: [&i[0], &i[1], &i[2]],
        p: P,
        inom: 600.0,
        sections: &sec,
    });
    let k = LEN - 1;
    assert!(t.f2[k] && t.pa[k] && t.pb[k] && !t.pc[k]);
    assert!((settled(&t) - 15.0).abs() < 1e-6, "{}", settled(&t));
}

/// When the fault lies beyond the last section, the original adds the last
/// section's length twice (acc includes it and d is measured from its start).
#[test]
fn fault_beyond_last_section_over_counts() {
    let sec = [section(10.0, 0.1, 0.4, 0.3, 1.2)];
    let iab = Complex64::new(1000.0, -500.0);
    let uab = Complex64::new(0.1, 0.4) * 25.0 * iab;
    let ua = Complex64::from_polar(30_000.0, 0.0);
    let (u, i) = (
        [wave(ua), wave(ua - uab), wave(Complex64::from_polar(57_000.0, 2.0))],
        [wave(iab / 2.0), wave(-iab / 2.0), vec![0.0; LEN]],
    );
    let t = single_ended(&SingleEnded {
        neutral: NeutralMode::Grounded,
        u: [&u[0], &u[1], &u[2]],
        i: [&i[0], &i[1], &i[2]],
        p: P,
        inom: 600.0,
        sections: &sec,
    });
    // d measured on the only section = 25, plus acc = 10.
    assert!((settled(&t) - 35.0).abs() < 1e-6, "{}", settled(&t));
}

/// Isolated neutral: A–B fault located by Uab/(Ia−Ib) against R, X per km.
#[test]
fn isolated_two_phase_fault() {
    let sec = [section(40.0, 0.15, 0.38, 0.0, 0.0)];
    let iab = Complex64::new(900.0, -700.0);
    let uab = Complex64::new(0.15, 0.38) * 12.5 * iab;
    let (u, i) = (
        [wave(uab), wave(Complex64::from_polar(10_000.0, -2.0)), wave(Complex64::from_polar(10_000.0, 2.0))],
        [wave(iab / 2.0), wave(-iab / 2.0), vec![0.0; LEN]],
    );
    let t = single_ended(&SingleEnded {
        neutral: NeutralMode::Isolated,
        u: [&u[0], &u[1], &u[2]],
        i: [&i[0], &i[1], &i[2]],
        p: P,
        inom: 300.0,
        sections: &sec,
    });
    let k = LEN - 1;
    assert!(t.f2[k] && t.pa[k] && t.pb[k] && !t.pc[k]);
    assert!((settled(&t) - 12.5).abs() < 1e-6, "{}", settled(&t));
    assert_eq!(t.dist[0], 0.0, "no fault before a full window");
}

/// Debounce: rises on the trunc(P/4)-th consecutive raw sample, falls at once.
#[test]
fn debounce_timing() {
    let x = [false, true, true, true, true, false, true];
    assert_eq!(debounce(&x, 8), vec![false, false, true, true, true, false, false]);
    assert_eq!(debounce(&x, 3), vec![false, true, true, true, true, false, true]);
}

/// Median: an even count averages the two middle values.
#[test]
fn median_rule() {
    assert_eq!(median(&[3.0, 1.0, 2.0, 4.0]), 2.5);
    assert_eq!(median(&[5.0, 1.0, 3.0]), 3.0);
}
