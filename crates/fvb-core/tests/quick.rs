//! Quick analysis as a channel generator (ledger L8, `notes/VIEWS_SPEC.md`).

use fvb_core::comtrade;
use fvb_core::quick::*;

const CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
const DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

fn cur(calc_ib: bool) -> Currents {
    Currents { ia: "Ia".into(), ib: "Ib".into(), ic: "Ic".into(), calc_ib }
}

fn phase_u() -> Voltages {
    Voltages::Phase { ua: "Ua".into(), ub: "Ub".into(), uc: "Uc".into() }
}

fn names(g: &Generated) -> Vec<&str> {
    g.channels.iter().map(|(n, _, _)| n.as_str()).collect()
}

/// Every generated formula evaluates on a real record, in order.
fn evaluates(g: &Generated) -> Vec<ComputedChannel> {
    let osc = comtrade::parse(CFG, DAT).unwrap();
    evaluate_batch(&osc, &[], &g.channels, false).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn symmetric_channels() {
    let g = symmetric(Some(&cur(false)), Some(&phase_u()));
    assert_eq!(names(&g), ["I1", "I2", "3I0", "U1", "U2", "3U0"]);
    assert_eq!(g.channels[0].1, r#"I1:("Ia","Ib","Ic")"#);
    // Computing Ib: an Ib channel first, no 3I0.
    let g = symmetric(Some(&cur(true)), None);
    assert_eq!(names(&g), ["Ib", "I1", "I2"]);
    assert_eq!(g.channels[0].1, r#"Ib:("Ia","Ic",0)"#);
    assert_eq!(evaluates(&g).len(), 3);
    let line = Voltages::Line { uab: "Ua".into(), ubc: "Ub".into() };
    assert_eq!(names(&symmetric(None, Some(&line))), ["U1", "U2"]);
}

#[test]
fn zero_sequence_flags() {
    let i = ZeroCurrent::Calculated { ia: "Ia".into(), ib: "Ib".into(), ic: "Ic".into() };
    let u = ZeroVoltage::FromPhases { ua: "Ua".into(), ub: "Ub".into(), uc: "Uc".into() };
    let g = zero(Some(&i), Some(&u));
    assert_eq!(names(&g), ["3I0", "3U0", "S0"]);
    assert_eq!(g.channels[2], ("S0".into(), r#"Mod:(Pf:("3I0","3U0"))"#.into(), "VA".into()));
    assert_eq!((g.vector.clone(), g.circle.clone()), (vec!["3I0".into(), "3U0".into()], vec!["S0".into()]));
    assert_eq!((g.angle_ref.as_deref(), g.show_values, g.circle_readout), (Some("3U0"), true, Some(1)));
    let out = evaluates(&g);
    assert!(out[2].samples.iter().all(|v| *v >= 0.0), "S0 is a magnitude");
}

#[test]
fn phase_phase_impedances() {
    let g = phase_phase(&cur(false), &phase_u(), "0,5");
    assert_eq!(names(&g), ["Uab", "Ubc", "Uca", "Zab", "Zbc", "Zca"]);
    assert_eq!(g.channels[3].1, r#"Mod:(Zl:("Uab","Ia","Ib",0.5))"#);
    assert_eq!(g.circle, ["Zab", "Zbc", "Zca"]);
    // Line-voltage mode: only Uca generated, no circle flags.
    let line = Voltages::Line { uab: "Ua".into(), ubc: "Ub".into() };
    let g2 = phase_phase(&cur(false), &line, "0");
    assert_eq!(names(&g2), ["Uca", "Zab", "Zbc", "Zca"]);
    assert!(g2.circle.is_empty());
    let out = evaluates(&g);
    // Zab's stored complex series is Uab/(Ia − Ib).
    assert!(out[3].phasors.is_some());
}

#[test]
fn phase_impedances_use_k0() {
    let comp = PhaseComp::PerUnit { r0: "0.3".into(), r1: "0.1".into(), rm: "0".into(), x0: "1.2".into(), x1: "0.4".into(), xm: "0".into() };
    let g = phase_z(&cur(false), ["Ua", "Ub", "Uc"], "", &comp, "0");
    assert_eq!(names(&g), ["3I0", "Za", "Zb", "Zc"]);
    assert_eq!(g.channels[1].1, r#"Mod:(Zf:("Ua","Ia","3I0",0,0.3,0.1,0,1.2,0.4,0,0))"#);
    evaluates(&g);
}

#[test]
fn differential_currents() {
    let arm = |a: &str, b: &str, c: &str| Arm {
        currents: vec![a.into(), b.into(), c.into()],
        reduction: Reduction::Priv { k: "1".into() },
        group: GroupComp::None,
    };
    let g = differential(3, &[arm("Ia", "Ib", "Ic"), arm("Ib", "Ic", "Ia")], &Braking::Sum { k: "0.5".into() });
    assert_eq!(g.dif_pairs[0], ("Ia диф.".into(), "Ia торм.".into()));
    assert_eq!(g.channels.len(), 2 * 3 + 2 * 3);
    let out = evaluates(&g);
    assert_eq!(out.len(), 12);
}

/// A taken name gets "_2", "_3", … (0x48bff0).
#[test]
fn unique_names() {
    let taken = vec!["I1".to_string(), "I1_2".to_string()];
    assert_eq!(unique_name("I1", &taken), "I1_3");
    assert_eq!(unique_name("I2", &taken), "I2");
    // Evaluating a batch twice renames the second set.
    let osc = comtrade::parse(CFG, DAT).unwrap();
    let g = symmetric(Some(&cur(false)), None);
    let first = evaluate_batch(&osc, &[], &g.channels, false).unwrap();
    let second = evaluate_batch(&osc, &first, &g.channels, false).unwrap();
    assert_eq!(second[0].name, "I1_2");
}
