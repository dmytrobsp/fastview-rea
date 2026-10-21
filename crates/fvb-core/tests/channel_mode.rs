//! The formula language's channel mode pinned to the decompiled `Calculator`
//! (ledger L6): CHANNEL atoms, the channel flag, and the channel functions.

use fvb_core::expr::{Channels, Context, Expr, ExprError};
use num_complex::Complex64;

/// Three phases with fixed RMS phasors 1∠0°, 1∠−120°, 1∠120°; sample value =
/// index; rms = 10 + ch; one period = 4 samples.
struct Mock;

impl Channels for Mock {
    fn resolve(&self, name: &str) -> Option<usize> {
        ["ia", "ib", "ic"].iter().position(|n| *n == name.to_lowercase())
    }
    fn sample(&self, ch: usize, idx: usize) -> f64 {
        idx as f64 + ch as f64 * 100.0
    }
    fn phasor(&self, ch: usize, _idx: usize) -> Complex64 {
        Complex64::from_polar(1.0, -(ch as f64) * 120f64.to_radians())
    }
    fn rms(&self, ch: usize, _idx: usize) -> f64 {
        10.0 + ch as f64
    }
    fn a0(&self, _ch: usize, _idx: usize) -> f64 {
        0.5
    }
    fn fourie(&self, _ch: usize, n: i32, _idx: usize) -> f64 {
        f64::from(n)
    }
    fn fault(&self, _ch: usize, _idx: usize) -> f64 {
        7.0
    }
    fn frequency(&self, _idx: usize) -> f64 {
        50.0
    }
    fn warmup(&self) -> usize {
        4
    }
}

fn at(src: &str, idx: usize) -> Result<f64, ExprError> {
    Expr::parse_channels(src, &Mock)?.eval_at(&Context::new(), &Mock, idx)
}

/// A CHANNEL atom's scalar is the instantaneous value; its complex pair is
/// the phasor (0x41f960), so `mod:` reads the phasor magnitude.
#[test]
fn channel_atom_slots() {
    assert_eq!(at("ib", 7).unwrap(), 107.0);
    assert_eq!(at("IA*2", 3).unwrap(), 6.0);
    assert!((at("mod:(ib)", 0).unwrap() - 1.0).abs() < 1e-12);
}

/// An identifier that names no channel is error 0x162 (`CalcChannels`
/// never returns null).
#[test]
fn unknown_channel_name_is_an_error() {
    assert_eq!(at("2 zz + 1", 0), Err(ExprError::BadChannelName("zz".into())));
    assert_eq!(ExprError::BadChannelName("zz".into()).to_string(), "Имя канала задано неправильно.");
}

/// Channel functions need a bare channel argument (error 0x431).
#[test]
fn channel_functions_need_a_bare_channel() {
    assert_eq!(at("rms:(ib)", 0).unwrap(), 11.0);
    assert_eq!(at("a0:(ia)", 0).unwrap(), 0.5);
    assert_eq!(at("fault:(ia)", 0).unwrap(), 7.0);
    assert_eq!(at("fourie:(ia, 3)", 0).unwrap(), 3.0);
    assert_eq!(at("rms:(2)", 0), Err(ExprError::NeedsChannel("rms:")));
    assert_eq!(at("rms:(ia+ib)", 0), Err(ExprError::NeedsChannel("rms:")));
}

/// `arg:` is 0 before the warm-up (one period); `frequency:` is the record's
/// frequency (and 0.0 in the `.fvb` context).
#[test]
fn arg_warmup_and_frequency() {
    assert_eq!(at("arg:(ib)", 3).unwrap(), 0.0);
    assert!((at("arg:(ib)", 4).unwrap() + 120.0).abs() < 1e-9);
    assert_eq!(at("frequency:(ia)", 0).unwrap(), 50.0);
    assert_eq!(fvb_core::expr::eval_str("frequency:(1)", &Context::new()).unwrap(), 0.0);
}

/// Symmetric components (0x41ad96, 0x41a796, 0x41aa6e): a balanced positive
/// sequence has |I1| = 1, I2 = 0, 3I0 = 0.
#[test]
fn symmetric_components() {
    assert!((at("i1:(ia, ib, ic)", 0).unwrap() - 1.0).abs() < 1e-12);
    assert!(at("i2:(ia, ib, ic)", 0).unwrap().abs() < 1e-12);
    assert!(at("3i0:(ia, ib, ic)", 0).unwrap().abs() < 1e-12);
    // Negative sequence order: swap b and c.
    assert!((at("i2:(ia, ic, ib)", 0).unwrap() - 1.0).abs() < 1e-12);
}

fn z(src: &str) -> (Option<f64>, Option<Complex64>) {
    Expr::parse_channels(src, &Mock).unwrap().eval_sample(&Context::new(), &Mock, 0).unwrap()
}

fn close(a: Complex64, b: Complex64) -> bool {
    (a - b).norm() < 1e-9
}

/// Flag bits decide what a computed channel stores: scalar (bit 0) and/or
/// complex (bit 1); `pf:` is complex-only, `null` neither (0x415780).
#[test]
fn result_flags() {
    let (s, c) = z("ia");
    assert!(s.is_some() && c.is_some());
    let (s, c) = z("pf:(ia, ib)");
    assert!(s.is_none() && c.is_some());
    let (s, c) = z("rms:(ia)");
    assert!(s.is_some() && c.is_none());
    assert_eq!(z("null"), (None, None));
}

/// Line/phase helpers (0x41a611, 0x41b55d, 0x41b5f8, 0x41b712, 0x41c105).
#[test]
fn voltage_and_current_helpers() {
    let (a, b, c) = (Mock.phasor(0, 0), Mock.phasor(1, 0), Mock.phasor(2, 0));
    assert!(close(z("ib:(ia, ib, ic)").1.unwrap(), c - (a + b)));
    assert!(close(z("ulf:(ia, ib)").1.unwrap(), a - b));
    assert!(close(z("ull:(ia, ib)").1.unwrap(), -(a + b)));
    // Positive sequence from two line voltages: (2·Uab + Ubc + j√3·Ubc)/6.
    let (uab, ubc) = (a - b, b - c);
    let want = (2.0 * uab + ubc + Complex64::new(0.0, 3f64.sqrt()) * ubc) / 6.0;
    assert!(close(z("u1l:(ulf:(ia, ib), ulf:(ib, ic))").1.unwrap(), want));
    let want = b + c - a + (a - b) / 3f64.sqrt();
    assert!(close(z("uncn:(ia, ib, ic, ia, ib)").1.unwrap(), want));
}

/// Power and power factor (0x41c37b, 0x41c414, 0x41c756).
#[test]
fn power_functions() {
    let (a, b) = (Mock.phasor(0, 0), Mock.phasor(1, 0));
    assert!(close(z("pf:(ia, ib)").1.unwrap(), a.conj() * b));
    assert!(close(z("pf2wm:(ia, ib, ib, ia)").1.unwrap(), a * b.conj() - b * a.conj()));
    assert!((at("pk:(ib)", 0).unwrap() - (-0.5)).abs() < 1e-12);
    assert_eq!(at("pk:(0)", 0).unwrap(), 1.0);
}

/// Loop impedance with K0 / mutual compensation and the minimum-current gate.
#[test]
fn zf_and_zk() {
    let (a, b) = (Mock.phasor(0, 0), Mock.phasor(1, 0));
    // R0=3 X0=9, R1=1 X1=3, Rm=Xm=0 → K0 = (Z0/Z1 − 1)/3.
    let k0 = (Complex64::new(3.0, 9.0) / Complex64::new(1.0, 3.0) - 1.0) / 3.0;
    let want = a / (b + k0 * a);
    let got = z("zf:(ia, ib, ia, ib, 3, 1, 0, 9, 3, 0, 0.5)");
    assert!(close(got.1.unwrap(), want), "{got:?} vs {want}");
    // |Ib| = 1 < gate 2 → deliberate division by zero.
    let (s, c) = z("zf:(ia, ib, ia, ib, 3, 1, 0, 9, 3, 0, 2)");
    assert!(!s.unwrap().is_finite() && c.unwrap().re.is_nan());
    let za = Complex64::from_polar(2.0, 30f64.to_radians());
    let want = a / (b + a * za);
    assert!(close(z("zk:(ia, ib, ia, ib, 2, 0, 30, 0, 0.5)").1.unwrap(), want));
}

/// Channel pass-through, sums and the winding-group/restraint helpers.
#[test]
fn channel_helpers() {
    let (a, b, c) = (Mock.phasor(0, 0), Mock.phasor(1, 0), Mock.phasor(2, 0));
    assert_eq!(at("chan:(ib)", 5).unwrap(), 105.0);
    assert_eq!(at("chan:(2)", 0), Err(ExprError::NeedsChannel("chan:")));
    assert!(close(z("uniik:(ia, ib)").1.unwrap(), a + b));
    assert!((at("coeffpriv:(ib, 4)", 3).unwrap() - 412.0).abs() < 1e-12);
    assert!((at("coeffamppriv:(ib, 4, 2)", 3).unwrap() - 206.0).abs() < 1e-12);
    let rt3 = 3f64.sqrt();
    assert!(close(z("grstriangleib:(ia, ib, ic, 1)").1.unwrap(), (b - c) / rt3));
    assert!(close(z("grstriangleib:(ia, ib, ic, 11)").1.unwrap(), (b - a) / rt3));
    assert!(close(z("grstriangleic:(ia, ib, ic, 0)").1.unwrap(), c));
    assert!(close(z("grstriangleic:(ia, ib, ic, 5)").1.unwrap(), a));
    assert!((at("tormsubcur:(ia, ib, 2)", 0).unwrap() - (b - a).norm() / 2.0).abs() < 1e-12);
}
