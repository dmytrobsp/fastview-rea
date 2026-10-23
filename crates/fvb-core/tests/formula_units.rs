//! The unit of a formula channel (`0x415bd0`, ledger L9.70): a `[...]`
//! where the evaluation stops, else the dimension walk.

use fvb_core::comtrade;
use fvb_core::quick::{evaluate_batch, ComputedChannel};

const CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
const DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

fn unit_after(existing: &[ComputedChannel], formula: &str) -> String {
    let osc = comtrade::parse(CFG, DAT).unwrap();
    let out = evaluate_batch(&osc, existing, &[("X".into(), formula.into())], false).unwrap_or_else(|e| panic!("{e}"));
    out[0].unit.clone()
}

fn unit(formula: &str) -> String {
    unit_after(&[], formula)
}

#[test]
fn operators_keep_the_left_unit() {
    assert_eq!(unit("Ua-Ub"), "V");
    assert_eq!(unit("Ia*Ua"), "A");
    assert_eq!(unit("2*Ua"), "", "a number on the left");
    assert_eq!(unit("-Ua"), "V");
    assert_eq!(unit("Ua^2"), "", "^ clears it");
    assert_eq!(unit("(Ua+Ub)/3"), "V");
}

#[test]
fn a_bracket_where_the_walk_stops_wins() {
    // The evaluator skips a [...] after an atom; the walk stops on it.
    assert_eq!(unit("Ua*Ia [W]"), "W");
    assert_eq!(unit("Ua[кВ]+Ub"), "кВ");
    assert_eq!(unit("2[Гц]*Ua"), "Гц");
    // After a group the evaluator itself stops on it.
    assert_eq!(unit("(Ua+Ub)[кВ]"), "кВ");
    assert_eq!(unit("Mod:(Pf: (Ia, Ua))[VA]"), "VA");
    // Inside a group the closing step swallows it.
    assert_eq!(unit("(Ua[кВ])*Ib"), "V");
}

#[test]
fn functions_set_or_keep_it() {
    assert_eq!(unit("Rms:(Ia)"), "A");
    assert_eq!(unit("Mod:(Ua)"), "V");
    assert_eq!(unit("Sin:(Ua)"), "");
    assert_eq!(unit("Fourie:(Ua, 2)"), "");
    assert_eq!(unit("3I0:(Ia, Ib, Ic)"), "A");
    assert_eq!(unit("3U0:(Ua, Ub, Uc)"), "V");
    assert_eq!(unit("Pf:(Ia, Ua)"), "W");
    assert_eq!(unit("Re:(Pf:(Ia, Ua))"), "W");
    assert_eq!(unit("Zl:(Ua, Ia, Ib, 1)"), "Ohm");
    assert_eq!(unit("Frequency: ()"), "Hz");
    // The braking coefficient then 24 currents (form 525).
    assert_eq!(unit(&format!("TormSumCur:(0.5, Ia, Ib{})", ", 0".repeat(22))), "A", "the second argument's");
    assert_eq!(unit("Chan:(Ua)"), "V");
}

#[test]
fn an_earlier_formula_channel_brings_its_own() {
    let osc = comtrade::parse(CFG, DAT).unwrap();
    let first = evaluate_batch(&osc, &[], &[("F".into(), "Frequency: ()".into())], false).unwrap();
    assert_eq!(first[0].unit, "Hz");
    assert_eq!(unit_after(&first, "F*2"), "Hz");
    assert_eq!(unit_after(&first, "2*F"), "");
}
