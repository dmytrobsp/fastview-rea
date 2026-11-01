//! #30 (ledger L6): after one of the analysis functions (tokens 0x30–0x39) every later channel reference is
//! retagged 0x46 and reads its scalar through `0x426120`; and the name guard (error 0x1d4).

use fvb_core::expr::{Channels, Context, Expr, ExprError, FormulaError};
use fvb_core::quick::{evaluate_batch_late, ComputedChannel};
use num_complex::Complex64;

/// Two channels; the accessor's value is the index (+ 100 per channel), the late one half a unit more.
struct Mock;

impl Channels for Mock {
    fn resolve(&self, name: &str) -> Option<usize> {
        ["ia", "ib"].iter().position(|n| *n == name.to_lowercase())
    }
    fn sample(&self, ch: usize, idx: usize) -> f64 {
        idx as f64 + ch as f64 * 100.0
    }
    fn sample_late(&self, ch: usize, idx: usize) -> f64 {
        self.sample(ch, idx) + 0.5
    }
    fn phasor(&self, _ch: usize, _idx: usize) -> Complex64 {
        Complex64::new(1.0, 0.0)
    }
    fn rms(&self, ch: usize, _idx: usize) -> f64 {
        10.0 + ch as f64
    }
    fn a0(&self, _ch: usize, _idx: usize) -> f64 {
        0.0
    }
    fn fourie(&self, _ch: usize, _n: i32, _idx: usize) -> f64 {
        0.0
    }
    fn fault(&self, _ch: usize, _idx: usize) -> f64 {
        0.0
    }
    fn frequency(&self, _idx: usize) -> f64 {
        50.0
    }
    fn warmup(&self) -> usize {
        4
    }
}

fn at(src: &str, idx: usize) -> f64 {
    Expr::parse_channels(src, &Mock).unwrap().eval_at(&Context::new(), &Mock, idx).unwrap()
}

#[test]
fn channel_references_after_an_analysis_function_read_the_late_value() {
    // No analysis function: the accessor everywhere.
    assert_eq!(at("ib", 3), 103.0);
    assert_eq!(at("ib + ia", 3), 106.0);
    // An argument of the function is after it (the retag runs when the function is reached).
    assert_eq!(at("coeffpriv:(ib, 4)", 3), 414.0);
    // A reference before the function stays; one after it is retagged.
    assert_eq!(at("ib + coeffpriv:(ia, 1)", 3), 103.0 + 3.5);
    assert_eq!(at("coeffpriv:(ia, 1) + ib", 3), 3.5 + 103.5);
    assert_eq!(at("coeffpriv:(ia, 1) + 2 * ib", 3), 3.5 + 207.0);
    // The other channel functions do not retag (0x30–0x39 only).
    assert_eq!(at("rms:(ia) + ib", 3), 10.0 + 103.0);
    // The grs* variants (0x34–0x39) do.
    assert_eq!(at("grsposlia:(ia, ib, ia, 0, 2) * 0 + ib", 3), 103.5);
}

const CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
const DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

#[test]
fn a_primary_stored_channel_reads_its_late_series() {
    let mut osc = fvb_core::comtrade::parse(CFG, DAT).unwrap();
    // Primary-stored, with a non-round ratio.
    osc.analog[0].is_primary = true;
    osc.analog[0].primary = 1234.0;
    osc.analog[0].secondary = 5.5;
    let name = osc.analog[0].id.clone();
    let raw = osc.analog[0].samples.clone();
    let late = |show_primary: bool| {
        let k = if show_primary { 1.0 } else { 5.5 / 1234.0 };
        let l: Vec<Option<Vec<f64>>> = (0..osc.analog.len()).map(|i| fvb_core::channels::late_series(&osc.analog[i], show_primary, false, false, false, 0.0, k)).collect();
        assert!(l[1..].iter().all(Option::is_none), "only primary-stored data differs");
        Some(l)
    };
    let batch = [("X".to_string(), format!("coeffpriv:({name}, 1)")), ("P".to_string(), format!("{name} * 1"))];
    for show_primary in [false, true] {
        let out = evaluate_batch_late(&osc, None, late(show_primary), None, &[], &batch, &[], show_primary).unwrap();
        let (x, p) = (&out[0].samples, &out[1].samples);
        // Shown secondary: both give secondary / primary · raw; shown primary: the accessor the stored value,
        // the late series secondary / primary of it (clone: `coeffpriv:(Ua,1)+Ua` read twice the shown value).
        let (acc, lat) = if show_primary { (1.0, 5.5 / 1234.0) } else { (5.5 / 1234.0, 5.5 / 1234.0) };
        for i in [10, 77, 150] {
            assert_eq!(p[i], raw[i] * acc, "accessor, shown primary {show_primary}");
            assert_eq!(x[i], raw[i] * lat, "late, shown primary {show_primary}");
        }
    }
    // Without the late series the retag is the accessor: what the code did before.
    let out = evaluate_batch_late(&osc, None, None, None, &[], &batch, &[], false).unwrap();
    assert_eq!(out[0].samples[77], out[1].samples[77]);
}

fn run(batch: &[(&str, &str)], tail: &[&str]) -> Result<Vec<ComputedChannel>, FormulaError> {
    let osc = fvb_core::comtrade::parse(CFG, DAT).unwrap();
    let batch: Vec<(String, String)> = batch.iter().map(|(n, f)| (n.to_string(), f.to_string())).collect();
    let tail: Vec<String> = tail.iter().map(|s| s.to_string()).collect();
    evaluate_batch_late(&osc, None, None, None, &[], &batch, &tail, false)
}

/// The name guard (`0x419074`): a formula naming its own definition or a later one fails with 0x1d4, whether or
/// not a channel of that name exists; the message is "<definition>: '<name>' -  не определен", shown as is.
#[test]
fn a_formula_naming_its_own_or_a_later_definition_is_an_error() {
    let ok = run(&[("A", "ia * 2"), ("B", "a + 1")], &[]).unwrap();
    assert_eq!(ok.len(), 2, "a later definition may read an earlier one");
    let own = run(&[("A", "ia + a")], &[]).unwrap_err();
    assert_eq!(own.error, ExprError::NotDefined { owner: "A".into(), ident: "a".into() });
    assert_eq!(own.to_string(), "A: 'a' -  не определен");
    let later = run(&[("A", "b + 1"), ("B", "ia")], &[]).unwrap_err();
    assert_eq!(later.to_string(), "A: 'b' -  не определен", "case-insensitive, the typed (lowercased) text");
    // The names that follow the batch in the list count too.
    assert_eq!(run(&[("A", "ia + z9")], &["Z9"]).unwrap_err().to_string(), "A: 'z9' -  не определен");
    // The guard is before the lookup: a name no definition has is still 0x162.
    assert_eq!(run(&[("A", "ia + qq")], &[]).unwrap_err().error, ExprError::BadChannelName("qq".into()));
    // …and a record channel's name that a definition also takes is the guard's, not the channel.
    let name = fvb_core::comtrade::parse(CFG, DAT).unwrap().analog[0].id.clone();
    assert!(run(&[("A", &format!("{name} + 1"))], &[&name]).is_err());
}
