//! The formula dialog's templates (DIALOG 359, `fvb_core::addformula`): every
//! leaf's formula, built from its form's initial state, evaluates on a real
//! record as a computed channel.

use fvb_core::addformula::*;
use fvb_core::comtrade;
use fvb_core::quick::evaluate_batch;

const CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
const DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

#[test]
fn every_template_evaluates() {
    let osc = comtrade::parse(CFG, DAT).unwrap();
    let chans: Vec<String> = osc.analog.iter().map(|c| c.id.clone()).collect();
    for (label, dialog) in leaves() {
        let mut v = initial_values(dialog, &chans, osc.sample_rate, osc.line_freq);
        // The demo record has no line voltages: pick phase channels instead.
        for t in v.values_mut() {
            if t == NO_CHANNEL {
                *t = "Ua".into();
            }
        }
        let f = build(dialog, &v).unwrap();
        let batch = [("X".to_string(), f.clone())];
        let out = evaluate_batch(&osc, &[], &batch, false).unwrap_or_else(|e| panic!("{label} ({dialog}) {f}: {e}"));
        assert_eq!(out[0].samples.len(), osc.time.len(), "{label}");
        // A template's trailing `[...]` is its unit (`0x415bd0`, step 1).
        if let Some(u) = f.strip_suffix(']').and_then(|s| s.rfind('[').map(|i| &s[i + 1..])) {
            assert_eq!(out[0].unit, u, "{label}");
        }
        if dialog == 488 {
            // "Frequency: ()" — the record's line frequency.
            assert!(out[0].samples.iter().all(|&v| v == 50.0));
        }
    }
}
