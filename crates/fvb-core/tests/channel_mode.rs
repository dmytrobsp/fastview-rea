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

/// An identifier that resolves to no channel is dropped from the token
/// stream (0x419126), which can silently change the formula.
#[test]
fn unresolved_identifiers_are_dropped() {
    assert_eq!(at("2 zz + 1", 0).unwrap(), 3.0);
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
