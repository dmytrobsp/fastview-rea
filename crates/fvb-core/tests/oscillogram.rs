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
    let line = LineParams { r1_per_km: 0.1, x1_per_km: 0.4, length_km: 50.0 };
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
