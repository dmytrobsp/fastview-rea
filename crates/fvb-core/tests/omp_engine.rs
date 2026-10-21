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

fn grounded_trace(sec: &[Section], ia: Complex64, d: f64) -> Trace {
    let i0 = ia / 3.0;
    let z1 = Complex64::new(sec[0].r1_per_km, sec[0].x1_per_km);
    let k = Complex64::new(sec[0].r0_per_km, sec[0].x0_per_km) / z1 - 1.0;
    let ua = z1 * d * (ia + k * i0);
    let healthy = Complex64::from_polar(57_000.0, -2.0);
    let u = [wave(ua), wave(healthy), wave(healthy * Complex64::from_polar(1.0, 2.1))];
    let i = [wave(ia), vec![0.0; LEN], vec![0.0; LEN]];
    single_ended(&SingleEnded {
        neutral: NeutralMode::Grounded,
        u: [&u[0], &u[1], &u[2]],
        i: [&i[0], &i[1], &i[2]],
        p: P,
        inom: 600.0,
        sections: sec,
    })
}

/// SI prefix table of 0x43d960 (strict comparisons, Cyrillic prefixes).
#[test]
fn si_prefixes() {
    assert_eq!(si(23_000.0), (23.0, "к"));
    assert_eq!(si(1000.0), (1000.0, ""));
    assert_eq!(si(0.5), (500.0, "м"));
    assert_eq!(si(0.0), (0.0, ""));
    assert_eq!(si(-2.5e6), (-2.5, "М"));
}

/// The whole single-ended pipeline: interval, window, median, location and the
/// original's result text (strings 3006/3026/3020).
#[test]
fn located_earth_fault_text() {
    let mut sec = section(60.0, 0.12, 0.40, 0.30, 1.20);
    sec.supports = 120;
    let sec = [sec];
    let t = grounded_trace(&sec, Complex64::new(800.0, -300.0), 23.0);
    let r = locate(&t, NeutralMode::Grounded, &sec, P);
    assert_eq!(r.window.1, r.window.0 + P - 1);
    assert!((r.distance - 23.0).abs() < 1e-6);
    assert!(r.e && r.phases == [true, false, false]);
    assert_eq!((r.section, r.support), (0, 47));
    assert!(
        r.text.starts_with(
            "    1-х фазное КЗ: фазы A на участке № 1, опоры 46 - 47.    Расстояние = 23.00 кm.     Расстояние от нач. участка = 23.00 кm.    Погрешность расчета = "
        ),
        "{}",
        r.text
    );
}

/// Beyond the line (or at 0) the text says the fault is on an adjacent line.
#[test]
fn adjacent_line_text() {
    let sec = [section(10.0, 0.12, 0.40, 0.30, 1.20)];
    let t = grounded_trace(&sec, Complex64::new(800.0, -300.0), 25.0);
    let r = locate(&t, NeutralMode::Grounded, &sec, P);
    assert_eq!(r.text, "    1-х фазное КЗ: фазы A на смежной линии.");
    let quiet = Trace { dist: vec![0.0; 50], e: vec![false; 50], f2: vec![false; 50], f3: vec![false; 50],
        pa: vec![false; 50], pb: vec![false; 50], pc: vec![false; 50], inst: vec![0.0; 50], ..Default::default() };
    assert_eq!(result_for_window(&quiet, &sec, 0, 9).text, "    КЗ отсутствует.");
}

/// With equal scores the window chooser's bias picks the latest candidate.
#[test]
fn window_chooser_prefers_later_windows() {
    let dist = vec![5.0; 100];
    let inst = vec![0.0; 100];
    let (c1, c2) = choose_window(&dist, &inst, 10, 60, P);
    assert_eq!((c1, c2), (40, 40 + P - 1));
}

/// Phase quantities from positive- and negative-sequence components.
fn phases_of(p1: Complex64, p2: Complex64) -> [Vec<f64>; 3] {
    let a = Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI / 3.0);
    [wave(p1 + p2), wave(a * a * p1 + a * p2), wave(a * p1 + a * a * p2)]
}

/// Two-phase fault 18 km out on a 50 km line, sources behind both ends
/// (Zs1, Zs2). The search matches |U2| at the fault from both ends.
#[test]
fn double_ended_internal_fault() {
    let sec = [section(50.0, 0.1, 0.4, 0.3, 1.2)];
    let z1 = Complex64::new(0.1, 0.4);
    let (zs1, zs2) = (Complex64::new(1.0, 6.0), Complex64::new(2.0, 9.0));
    let x = 18.0;
    let i2a = Complex64::from_polar(700.0, -1.2);
    // −(Zs1 + Z1·x)·I2₁ = (Zs2 + Z1·(L−x))·I2₂  (end-2 current measured out of the line)
    let i2b = -(zs1 + z1 * x) * i2a / (zs2 + z1 * (50.0 - x));
    let (u2a, u2b) = (-zs1 * i2a, zs2 * i2b);
    let u1 = Complex64::from_polar(60_000.0, 0.0);
    let (ua, ia) = (phases_of(u1, u2a), phases_of(i2a, i2a));
    let (ub, ib) = (phases_of(u1, u2b), phases_of(i2b, i2b));
    let t = double_ended(&DoubleEnded {
        u1: [&ua[0], &ua[1], &ua[2]],
        i1: [&ia[0], &ia[1], &ia[2]],
        u2: [&ub[0], &ub[1], &ub[2]],
        i2: [&ib[0], &ib[1], &ib[2]],
        p: P,
        inom1: 300.0,
        inom2: 300.0,
        sections: &sec,
    });
    let k = LEN - 1;
    assert!(t.f2[k]);
    assert!((t.dist[k] - x).abs() < 0.02, "{}", t.dist[k]);
    let r = locate_double(&t, &sec, P);
    assert!((r.distance - x).abs() < 0.02 && r.f2, "{r:?}");

    // Same magnitudes but end 2's current reversed: the angle difference is
    // within ±90°, so every distance is zeroed (adjacent-line fault).
    let (ub2, ib2) = (phases_of(u1, -u2b), phases_of(i2b, i2b));
    let t = double_ended(&DoubleEnded {
        u1: [&ua[0], &ua[1], &ua[2]],
        i1: [&ia[0], &ia[1], &ia[2]],
        u2: [&ub2[0], &ub2[1], &ub2[2]],
        i2: [&ib2[0], &ib2[1], &ib2[2]],
        p: P,
        inom1: 300.0,
        inom2: 300.0,
        sections: &sec,
    });
    let r = locate_double(&t, &sec, P);
    assert_eq!(r.distance, 0.0);
    assert!(r.text.ends_with(" на смежной линии."), "{}", r.text);
}
