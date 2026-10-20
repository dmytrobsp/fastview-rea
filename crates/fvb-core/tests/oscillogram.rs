use approx::assert_relative_eq;
use fvb_core::comtrade;
use fvb_core::omp::{single_ended_distance, LineParams};
use fvb_core::signal::*;
use num_complex::Complex64;
use std::f64::consts::PI;

const FS: f64 = 2000.0; // sampling rate
const F0: f64 = 50.0; // line frequency
const N: usize = (FS / F0) as usize; // 40 samples/cycle

fn sine(peak: f64, phase_deg: f64, cycles: usize) -> Vec<f64> {
    let total = N * cycles;
    (0..total)
        .map(|i| {
            let t = i as f64 / FS;
            peak * (2.0 * PI * F0 * t + phase_deg.to_radians()).sin()
        })
        .collect()
}

#[test]
fn phasor_magnitude_is_rms_and_angle_matches() {
    let s = sine(100.0, 30.0, 10);
    let ph = sliding_phasors(&s, N);
    // Use the last fully-windowed sample.
    let last = ph[ph.len() - 1];
    assert_relative_eq!(last.norm(), 100.0 / 2f64.sqrt(), epsilon = 1e-6);
    // A pure sine's fundamental angle is reported consistently; check two points
    // one cycle apart agree.
    let a = ph[ph.len() - 1];
    let b = ph[ph.len() - 1 - N];
    assert_relative_eq!(a.arg().to_degrees(), b.arg().to_degrees(), epsilon = 1e-6);
}

#[test]
fn balanced_set_has_only_positive_sequence() {
    let a = sliding_phasors(&sine(100.0, 0.0, 12), N);
    let b = sliding_phasors(&sine(100.0, -120.0, 12), N);
    let c = sliding_phasors(&sine(100.0, 120.0, 12), N);
    let i = a.len() - 1;
    let (z3, p, n) = symmetric_components(a[i], b[i], c[i]);
    let mag = 100.0 / 2f64.sqrt();
    assert_relative_eq!(p.norm(), mag, epsilon = 1e-6);
    assert!(n.norm() < 1e-6, "negative seq should vanish, got {}", n.norm());
    assert!(z3.norm() < 1e-6, "zero seq should vanish, got {}", z3.norm());
}

#[test]
fn single_phase_fault_produces_negative_and_zero_sequence() {
    // Only phase A energised -> strong 3I0 and I2.
    let a = sliding_phasors(&sine(100.0, 0.0, 12), N);
    let zero = vec![Complex64::new(0.0, 0.0); a.len()];
    let i = a.len() - 1;
    let (z3, _p, n) = symmetric_components(a[i], zero[i], zero[i]);
    assert!(z3.norm() > 1.0);
    assert!(n.norm() > 1.0);
}

#[test]
fn impedance_angle_equals_voltage_current_phase_difference() {
    // U at 0°, I at -60° (lagging): Z = U/I has angle +60°, |Z| = |U|/|I|.
    let u = sliding_phasors(&sine(100.0, 0.0, 12), N);
    let cur = sliding_phasors(&sine(2.0, -60.0, 12), N);
    let z = impedance_series(&u, &cur);
    let last = z[z.len() - 1];
    assert_relative_eq!(last.norm(), 50.0, epsilon = 1e-4); // 100/2
    assert_relative_eq!(last.arg().to_degrees(), 60.0, epsilon = 1e-4);
}

#[test]
fn single_ended_omp_recovers_fault_distance() {
    // Fault loop: U/I reactance = 4 Ω; line x1 = 0.4 Ω/km -> 10 km.
    // Build U = Z*I with Z = 1 + j4 (some arc resistance + reactance).
    let i_loop = sliding_phasors(&sine(5.0, -80.0, 12), N);
    let idx = i_loop.len() - 1;
    let z_true = Complex64::new(1.0, 4.0);
    // Construct a matching U phasor directly for the measured point.
    let u_meas = z_true * i_loop[idx];
    let line = LineParams { r1_per_km: 0.1, x1_per_km: 0.4, r0_per_km: 0.3, x0_per_km: 1.2, length_km: 50.0 };
    let d = single_ended_distance(u_meas, i_loop[idx], &line).unwrap();
    assert_relative_eq!(d, 10.0, epsilon = 1e-6);
}

#[test]
fn comtrade_ascii_round_trip() {
    // Minimal 1-analog, 1-digital ASCII record, 1999 revision.
    let cfg = "Station,Dev,1999\n\
        2,1A,1D\n\
        1,Ia,A,,A,0.1,0.0,0.0,-32768,32767,100.0,1.0,P\n\
        1,Trip,,,0\n\
        50\n\
        1\n\
        2000,4\n\
        01/01/2024,00:00:00.000000\n\
        01/01/2024,00:00:00.001000\n\
        ASCII\n\
        1\n";
    // raw values 10,20,30,40 -> scaled a*raw+b = 1.0,2.0,3.0,4.0; digital toggles
    let dat = "1,0,10,0\n2,500,20,1\n3,1000,30,0\n4,1500,40,1\n";
    let rec = comtrade::parse(cfg, dat.as_bytes()).expect("parse comtrade");
    assert_eq!(rec.line_freq, 50.0);
    assert_eq!(rec.analog.len(), 1);
    assert_eq!(rec.digital.len(), 1);
    assert_eq!(rec.analog[0].samples, vec![1.0, 2.0, 3.0, 4.0]);
    assert_eq!(rec.digital[0].samples, vec![false, true, false, true]);
    assert_eq!(rec.sample_count(), 4);
    assert_eq!(rec.samples_per_cycle(), 40); // 2000/50
}

#[test]
fn comtrade_binary_round_trip() {
    let cfg = "S,D,1999\n\
        2,2A,0D\n\
        1,Ua,A,,V,1.0,0.0,0.0,-32768,32767,1.0,1.0,S\n\
        2,Ia,A,,A,1.0,0.0,0.0,-32768,32767,1.0,1.0,S\n\
        50\n1\n2000,2\n\
        01/01/2024,00:00:00.000000\n01/01/2024,00:00:00.001000\n\
        BINARY\n1\n";
    // 2 samples, each: u32 sample#, u32 timestamp, 2x i16 analog
    let mut dat = Vec::new();
    for (s, (a0, a1)) in [(1u32, (111i16, -222i16)), (2u32, (333i16, 444i16))] {
        dat.extend_from_slice(&s.to_le_bytes());
        dat.extend_from_slice(&(s * 500).to_le_bytes());
        dat.extend_from_slice(&a0.to_le_bytes());
        dat.extend_from_slice(&a1.to_le_bytes());
    }
    let rec = comtrade::parse(cfg, &dat).expect("parse binary");
    assert_eq!(rec.analog[0].samples, vec![111.0, 333.0]);
    assert_eq!(rec.analog[1].samples, vec![-222.0, 444.0]);
}

#[test]
fn line_voltages_from_phase() {
    use fvb_core::signal::line_from_phase;
    let ua = Complex64::new(1.0, 0.0);
    let ub = Complex64::new(0.0, 1.0);
    let uc = Complex64::new(-1.0, 0.0);
    let (ab, bc, ca) = line_from_phase(ua, ub, uc);
    assert_relative_eq!(ab.re, 1.0);
    assert_relative_eq!(ab.im, -1.0);
    assert_relative_eq!(bc.re, 1.0);
    assert_relative_eq!(bc.im, 1.0);
    assert_relative_eq!(ca.re, -2.0);
    assert_relative_eq!(ca.im, 0.0);
}

#[test]
fn double_ended_midline_fault() {
    use fvb_core::omp::{double_ended_distance, LineParams};
    // Symmetric infeed, fault at the midpoint: distance ~= half the line.
    let line = LineParams { r1_per_km: 0.0, x1_per_km: 0.4, r0_per_km: 0.0, x0_per_km: 1.2, length_km: 100.0 };
    let z = Complex64::new(0.0, 0.4 * 100.0);
    let i_m = Complex64::new(1.0, 0.0);
    let i_n = Complex64::new(1.0, 0.0);
    // U_m = m*Z*I_m (to fault); U_n = (1-m)*Z*I_n; with m=0.5 both = 0.5*Z*I
    let u_m = Complex64::new(0.5, 0.0) * z * i_m;
    let u_n = Complex64::new(0.5, 0.0) * z * i_n;
    let d = double_ended_distance(u_m, i_m, u_n, i_n, &line).unwrap();
    assert!((d - 50.0).abs() < 5.0, "expected ~50 km, got {d}");
}

#[test]
fn analysis_detects_phases_and_computes_symmetric() {
    use fvb_core::analysis::Analysis;
    let rec = comtrade::parse(
        include_str!("../../../assets/demo/demo_fault.cfg"),
        include_bytes!("../../../assets/demo/demo_fault.dat"),
    )
    .expect("demo");
    let an = Analysis::compute(&rec);
    assert!(an.u_abc[0].is_some() && an.u_abc[1].is_some() && an.u_abc[2].is_some());
    assert!(an.i_abc[0].is_some() && an.i_abc[1].is_some() && an.i_abc[2].is_some());
    // Pre-fault (early, balanced) -> small negative/zero sequence.
    let pre = rec.samples_per_cycle() * 2;
    let (_i0, i1, i2) = an.i_symmetric(pre);
    assert!(i1.norm() > i2.norm() * 3.0, "pre-fault should be mostly positive sequence");
    // During fault (A-phase) -> significant 3I0 and I2.
    let mid = rec.sample_count() / 2;
    let (i0f, _i1f, i2f) = an.i_symmetric(mid);
    assert!(i0f.norm() > 0.5 && i2f.norm() > 0.5, "fault should show zero/neg sequence");
    // Phase-A impedance locus should reach near the origin (into the zone).
    let locus = an.phase_a_locus(0, rec.sample_count() - 1);
    let min_r = locus.iter().map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt()).fold(f64::MAX, f64::min);
    assert!(min_r < 10.0, "fault impedance should approach the zone, min |Z|={min_r}");
}

#[test]
fn k0_factor_is_correct() {
    use fvb_core::omp::LineParams;
    // z0 = 3·z1 -> k0 = (z0-z1)/(3z1) = 2/3 (real).
    let line = LineParams { r1_per_km: 0.1, x1_per_km: 0.4, r0_per_km: 0.3, x0_per_km: 1.2, length_km: 50.0 };
    let k0 = line.k0();
    assert_relative_eq!(k0.re, 2.0 / 3.0, epsilon = 1e-9);
    assert_relative_eq!(k0.im, 0.0, epsilon = 1e-9);
}

#[test]
fn grounded_omp_recovers_distance_with_compensation() {
    use fvb_core::omp::{single_ended_grounded, LineParams};
    let line = LineParams { r1_per_km: 0.1, x1_per_km: 0.4, r0_per_km: 0.3, x0_per_km: 1.2, length_km: 50.0 };
    let i_a = Complex64::from_polar(5.0, (-80f64).to_radians());
    let i3i0 = Complex64::from_polar(3.0, (-75f64).to_radians());
    let denom = i_a + line.k0() * i3i0;
    // fault reactance x = 4.8 -> distance 12 km at x1 = 0.4
    let zf = Complex64::new(0.5, 4.8);
    let u_a = zf * denom;
    let d = single_ended_grounded(u_a, i_a, i3i0, &line).unwrap();
    assert_relative_eq!(d, 12.0, epsilon = 1e-6);
}

#[test]
fn analysis_phase_phase_and_differential() {
    use fvb_core::analysis::Analysis;
    let rec = comtrade::parse(
        include_str!("../../../assets/demo/demo_fault.cfg"),
        include_bytes!("../../../assets/demo/demo_fault.dat"),
    )
    .expect("demo");
    let an = Analysis::compute(&rec);
    let mid = rec.sample_count() / 2;
    let zpp = an.phase_phase_impedances(mid);
    assert!(zpp.iter().any(|z| z.is_some()), "phase-phase impedance should resolve");
    let (id, ir) = an.differential_currents(mid);
    assert!(id >= 0.0 && ir >= 0.0);
    // During an A-ground fault the residual (differential) current is significant.
    assert!(id > 0.1, "fault should produce residual current, got {id}");
}

#[test]
fn comtrade_write_round_trips() {
    let rec = comtrade::parse(
        include_str!("../../../assets/demo/demo_fault.cfg"),
        include_bytes!("../../../assets/demo/demo_fault.dat"),
    )
    .expect("demo");
    let (cfg, dat) = comtrade::write(&rec);
    let rec2 = comtrade::parse(&cfg, dat.as_bytes()).expect("reparse written");
    assert_eq!(rec2.analog.len(), rec.analog.len());
    assert_eq!(rec2.digital.len(), rec.digital.len());
    assert_eq!(rec2.sample_count(), rec.sample_count());
    // Values survive the integer-count round trip within one quantisation step.
    for (a, b) in rec.analog.iter().zip(rec2.analog.iter()) {
        assert_eq!(a.id, b.id);
        let step = a.a.abs().max(1e-9);
        for (x, y) in a.samples.iter().zip(b.samples.iter()) {
            assert!((x - y).abs() <= step * 1.5 + 1e-6, "sample {x} vs {y} (step {step})");
        }
    }
}

#[test]
fn calc_ib_from_ia_ic_when_isolated() {
    use fvb_core::analysis::Analysis;
    let rec = comtrade::parse(
        include_str!("../../../assets/demo/demo_fault.cfg"),
        include_bytes!("../../../assets/demo/demo_fault.dat"),
    )
    .expect("demo");
    let mut an = Analysis::compute(&rec);
    let k = rec.sample_count() / 2;
    // With calc_ib, Ib must equal -(Ia+Ic) exactly.
    an.calc_ib = true;
    let (ia, ib, ic) = an.i_phase(k);
    let expect = -(ia + ic);
    assert_relative_eq!(ib.re, expect.re, epsilon = 1e-9);
    assert_relative_eq!(ib.im, expect.im, epsilon = 1e-9);
    // Residual (differential) current then vanishes.
    let (id, _) = an.differential_currents(k);
    assert!(id < 1e-6, "with computed Ib the residual should be ~0, got {id}");
}

#[test]
fn cfg_encoding_decoders() {
    // "Ua" + Cyrillic "фаза" in three single-byte code pages must decode right.
    // CP1251 'фаза' = F4 E0 E7 E0 ; CP866 'фаза' = A4 A0 A7 A0 ; KOI8-R = C6 C1 DA C1
    let cfg_tail = ",A,,V,1.0,0.0,0.0,-32768,32767,1.0,1.0,S\n50\n1\n1000,2\n\
        01/01/2024,00:00:00.000000\n01/01/2024,00:00:00.001000\nASCII\n1\n";
    let dat = b"1,0,1,1\n2,500,2,2\n";
    for (bytes, enc) in [
        (vec![0xF4u8, 0xE0, 0xE7, 0xE0], comtrade::Encoding::Cp1251),
        (vec![0xE4u8, 0xA0, 0xA7, 0xA0], comtrade::Encoding::Cp866),
        (vec![0xC6u8, 0xC1, 0xDA, 0xC1], comtrade::Encoding::Koi8r),
    ] {
        // Build a cfg with the channel id being the encoded word.
        let mut cfg = Vec::new();
        cfg.extend_from_slice(b"S,D,1999\n2,2A,0D\n1,");
        cfg.extend_from_slice(&bytes);
        cfg.extend_from_slice(cfg_tail.as_bytes());
        // decode_text_enc is private; exercise it through a temp file + load_with.
        let dir = std::env::temp_dir().join(format!("fvtest_{:?}", enc));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("x.cfg"), &cfg).unwrap();
        std::fs::write(dir.join("x.dat"), dat).unwrap();
        let rec = comtrade::load_with(dir.join("x.cfg"), enc).expect("load");
        assert_eq!(rec.analog[0].id, "фаза", "encoding {:?}", enc);
    }
}
