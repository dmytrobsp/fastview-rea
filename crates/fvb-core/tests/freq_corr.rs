//! Correction of computed values by a measured frequency (ledger L9.65):
//! `0x4267e0` / `0x427450` / `0x426df0` / `0x427830`, the candidates and
//! the selection of `0x42ebd0`.

use fvb_core::analysis::Analysis;
use fvb_core::comtrade::{self, Oscillogram};
use fvb_core::freqcorr::{self, FreqCorr};
use std::f64::consts::PI;

const CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
const DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

/// The demo record with Ub's unit changed to `unit` (its samples kept).
fn with_unit(unit: &str) -> Oscillogram {
    let cfg = CFG.replacen("2,Ub,B,,V,", &format!("2,Ub,B,,{unit},"), 1);
    comtrade::parse(&cfg, DAT).unwrap()
}

fn corr(freq: &[f64], fs: f64) -> FreqCorr<'_> {
    FreqCorr { chan: 99, freq, fs }
}

/// The weights add up to exactly `P/f`: a constant's mean is the constant.
#[test]
fn a_constant_integrates_exactly() {
    let x = vec![3.25; 400];
    for f in [49.5, 50.0, 51.3, 47.07] {
        let freq = vec![f; 400];
        let c = corr(&freq, 2000.0);
        for p in [1.0, 2.0, 3.0] {
            assert!((c.a0(&x, 399, p) - 3.25).abs() < 1e-12, "a0 f={f} p={p}");
            assert!((c.rms(&x, 399, p) - 3.25).abs() < 1e-12, "rms f={f} p={p}");
        }
        // Harmonic 0 is √2 times the mean, on the imaginary axis.
        let h0 = c.harmonic(&x, 399, 0.0, false, 1.0);
        assert!(h0.re.abs() < 1e-12 && (h0.im - 2f64.sqrt() * 3.25).abs() < 1e-12);
    }
}

/// An off-nominal sine: the corrected phasor and RMS follow the measured
/// period where the fixed 40-sample DFT leaks; the phase reference is the
/// newest sample as in the normal path.
#[test]
fn an_off_nominal_sine() {
    let (fs, f, amp, psi) = (2000.0, 48.7, 10.0, 0.4);
    let x: Vec<f64> = (0..600).map(|i| 2f64.sqrt() * amp * (2.0 * PI * f * i as f64 / fs + psi).sin()).collect();
    let freq = vec![f; 600];
    let c = corr(&freq, fs);
    let i = 500;
    let z = c.phasor(&x, i);
    let normal = fvb_core::signal::phasor_at(&x, i, 40);
    assert!((z.norm() - amp).abs() < 0.02, "corrected {}", z.norm());
    assert!((normal.norm() - amp).abs() > 3.0 * (z.norm() - amp).abs(), "the fixed window leaks more");
    // x[i] = √2·A·sin(ωi + ψ) gives A∠(ωi + ψ) at the newest sample.
    let want = (2.0 * PI * f * i as f64 / fs + psi).rem_euclid(2.0 * PI);
    let got = z.arg().rem_euclid(2.0 * PI);
    assert!((got - want).abs() < 0.01, "angle {got} vs {want}");
    assert!((c.rms(&x, i, 1.0) - amp).abs() < 0.02);
    // A steady sine has no fault component.
    assert!(c.fault(&x, i).abs() < 0.05, "fault {}", c.fault(&x, i));
}

/// 0 while `idx ≤ K`; f = 0 reads as 50; past the frequency channel's end
/// the DFT takes 50 and rms gives 0.
#[test]
fn warm_up_and_fallbacks() {
    let x = vec![1.0; 200];
    let freq = vec![50.0; 200];
    let c = corr(&freq, 1000.0);
    // K = 20 at 50 Hz and 1 kHz.
    assert_eq!(c.rms(&x, 20, 1.0), 0.0);
    assert!(c.rms(&x, 21, 1.0) > 0.0);
    let zero = vec![0.0; 200];
    let z = corr(&zero, 1000.0);
    assert!((z.a0(&x, 100, 1.0) - c.a0(&x, 100, 1.0)).abs() < 1e-15, "0 Hz reads as 50");
    let short = vec![50.0; 50];
    let s = corr(&short, 1000.0);
    assert_eq!(s.rms(&x, 100, 1.0), 0.0);
    assert!(s.harmonic(&x, 100, 0.0, false, 1.0).norm() > 0.0);
}

/// The candidates: a frequency unit (canonical Hz from «Гц») or ph / ccbm
/// «system»; the `.cfg` loader selects the first; the gate needs one rate
/// segment and spares the frequency channel itself.
#[test]
fn candidates_selection_and_gate() {
    let plain = comtrade::parse(CFG, DAT).unwrap();
    assert!(freqcorr::candidates(&plain).is_empty());
    assert_eq!(plain.freq_sel, 0);
    assert!(freqcorr::of(&plain).is_none());

    let rec = with_unit("Гц");
    assert_eq!(freqcorr::candidates(&rec), [1]);
    assert_eq!(rec.freq_sel, 1, "selected on load");
    let c = freqcorr::of(&rec).expect("corrected");
    assert_eq!(c.chan, 1);
    assert!(c.for_chan(1).is_none() && c.for_chan(3).is_some());
    let sys = comtrade::parse(&CFG.replacen("2,Ub,B,,V,", "2,Ub,B,system,V,", 1), DAT).unwrap();
    assert_eq!(freqcorr::candidates(&sys), [1]);

    // «Не корректировать» survives until the next rebuild.
    let mut off = rec.clone();
    off.freq_sel = 0;
    assert!(freqcorr::of(&off).is_none());
    freqcorr::refresh_selection(&mut off);
    assert_eq!(off.freq_sel, 1);

    let mut two = rec.clone();
    two.segments = 2;
    assert!(freqcorr::of(&two).is_none(), "one rate segment only");

    // The record's phasors: the frequency channel normal, the others not.
    let (a, b) = (Analysis::compute(&rec), Analysis::compute(&plain));
    let k = 300;
    assert_eq!(a.phasors[1][k], b.phasors[1][k]);
    assert_ne!(a.phasors[3][k], b.phasors[3][k]);
}

/// The list follows the vector (`0x42ec10`): an extra candidate (a pasted
/// channel) in front of a file one comes first, a formula one after them;
/// the selection is a number, so it now names another channel.
#[test]
fn candidates_in_vector_order() {
    let mut rec = with_unit("Гц");
    let ex = |name: &str, pos: usize, id: usize| comtrade::ExtraFreq { name: name.into(), samples: vec![50.0; 10], pos, id };
    rec.extra_freq = vec![ex("F", rec.analog.len(), 7), ex("P", 0, 3)];
    let ub = rec.analog[1].id.clone();
    assert_eq!(freqcorr::candidate_names(&rec), ["P".to_string(), ub, "F".into()]);
    assert_eq!(rec.freq_sel, 1);
    assert_eq!(freqcorr::selected_extra(&rec), Some(3), "the pasted channel is now the frequency channel");
    assert_eq!(freqcorr::of(&rec).unwrap().chan, usize::MAX);
    rec.freq_sel = 2;
    assert_eq!(freqcorr::selected_extra(&rec), None);
    rec.freq_sel = 3;
    assert_eq!(freqcorr::selected_extra(&rec), Some(7));
}
