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
    g.channels.iter().map(|(n, _)| n.as_str()).collect()
}

/// Every generated formula evaluates on a real record, in order.
fn evaluates(g: &Generated) -> Vec<ComputedChannel> {
    let osc = comtrade::parse(CFG, DAT).unwrap();
    evaluate_batch(&osc, &[], &g.channels, false).unwrap_or_else(|e| panic!("{e}"))
}

/// The record's channel names (what `uniq` checks against).
fn taken() -> Vec<String> {
    let osc = comtrade::parse(CFG, DAT).unwrap();
    osc.analog.iter().map(|c| c.id.clone()).collect()
}

#[test]
fn symmetric_channels() {
    let t = taken();
    let g = symmetric(Some(&cur(false)), Some(&phase_u()), &t);
    assert_eq!(names(&g), ["I1", "I2", "3I0", "U1", "U2", "3U0"]);
    assert_eq!(g.channels[0].1, "I1: (Ia, Ib, Ic)");
    assert_eq!(g.channels[5].1, "3U0: (Ua, Ub, Uc)");
    // Computing Ib: form 401, then I1 / I2 name it as it will be appended —
    // the record has an «Ib», so "Ib_2" (forms 800 / 801); no 3I0.
    let g = symmetric(Some(&cur(true)), None, &t);
    assert_eq!(names(&g), ["Ib", "I1", "I2"]);
    assert_eq!(g.channels[0].1, "Ib: (Ia, Ic, 0)");
    assert_eq!(g.channels[1].1, r#"I1: (Ia, "Ib_2", Ic)"#);
    assert_eq!(evaluates(&g).len(), 3);
    let line = Voltages::Line { uab: "Ua".into(), ubc: "Ub".into() };
    let g = symmetric(None, Some(&line), &t);
    assert_eq!(g.channels, [("U1".into(), "U1l: (Ua, Ub)".into()), ("U2".into(), "U2l: (Ua, Ub)".into())]);
}

#[test]
fn zero_sequence_flags() {
    let i = ZeroCurrent::Calculated { ia: "Ia".into(), ib: "Ib".into(), ic: "Ic".into() };
    let u = ZeroVoltage::FromPhases { ua: "Ua".into(), ub: "Ub".into(), uc: "Uc".into() };
    let g = zero(Some(&i), Some(&u), &taken());
    assert_eq!(names(&g), ["3I0", "3U0", "S0"]);
    assert_eq!(g.channels[2], ("S0".into(), r#"Mod:(Pf: ("3I0", "3U0"))[VA]"#.into()));
    assert_eq!((g.vector.clone(), g.circle.clone()), (vec!["3I0".into(), "3U0".into()], vec!["S0".into()]));
    assert_eq!((g.angle_ref.as_deref(), g.show_values, g.circle_readout), (Some("3U0"), true, Some(1)));
    let out = evaluates(&g);
    assert!(out[2].samples.iter().all(|v| *v >= 0.0), "S0 is a magnitude");
    assert_eq!(out[2].unit, "VA");
    let g = zero(Some(&ZeroCurrent::Measured("Ia".into())), Some(&ZeroVoltage::FromUniUik { uni: "Ua".into(), uik: "Ub".into() }), &taken());
    assert_eq!((g.channels[0].1.as_str(), g.channels[1].1.as_str()), ("Chan: (Ia)", "Uniik: (Ua, Ub)"));
}

#[test]
fn phase_phase_impedances() {
    let g = phase_phase(&cur(false), &phase_u(), "0,5", &taken());
    assert_eq!(names(&g), ["Uab", "Ubc", "Uca", "Zab", "Zbc", "Zca"]);
    assert_eq!(g.channels[1].1, "Ulf: (Ub, Uc)");
    assert_eq!(g.channels[3].1, "Mod:(Zl: (Uab, Ia, Ib, 0.5))");
    assert_eq!(g.channels[5].1, "Mod:(Zl: (Uca, Ic, Ia, 0.5))");
    assert_eq!(g.circle, ["Zab", "Zbc", "Zca"]);
    // Line-voltage mode: only Uca generated, no circle flags.
    let line = Voltages::Line { uab: "Ua".into(), ubc: "Ub".into() };
    let g2 = phase_phase(&cur(true), &line, "0", &taken());
    assert_eq!(names(&g2), ["Ib", "Uca", "Zab", "Zbc", "Zca"]);
    assert_eq!(g2.channels[2].1, r#"Mod:(Zl: (Ua, Ia, "Ib_2", 0))"#);
    assert!(g2.circle.is_empty());
    let out = evaluates(&g);
    // Zab's stored complex series is Uab/(Ia − Ib).
    assert!(out[3].phasors.is_some());
    assert_eq!(out[3].unit, "Ohm");
    evaluates(&g2);
}

#[test]
fn phase_impedances_use_k0() {
    let comp = PhaseComp::PerUnit { r0: "0.3".into(), r1: "0,1".into(), rm: "0".into(), x0: "1.2".into(), x1: "0.4".into(), xm: "0".into() };
    let g = phase_z(&cur(false), ["Ua", "Ub", "Uc"], "", &comp, "0", &taken());
    assert_eq!(names(&g), ["3I0", "Za", "Zb", "Zc"]);
    assert_eq!(g.channels[1].1, r#"Mod:(Zf: (Ua, Ia, "3I0", 0, 0.3, 0.1, 0, 1.2, 0.4, 0, 0))"#);
    evaluates(&g);
}

#[test]
fn differential_currents() {
    let arm = |a: &str, b: &str, c: &str| Arm {
        currents: vec![a.into(), b.into(), c.into()],
        reductions: vec![Reduction::Priv { k: "1".into() }],
        group: "11".into(),
        remove_i0: false,
    };
    let arms = [arm("Ia", "Ib", "Ic"), arm("Ib", "Ic", "Ia")];
    let g = differential(3, &arms, &GroupComp::None, &Braking::Sum { k: "0.5".into() }, &taken());
    assert_eq!(g.dif_pairs[0], ("Ia диф.".into(), "Ia торм.".into()));
    // Every reduction, then every диф., then every торм.
    assert_eq!(
        names(&g),
        ["Ia плечо 1 прив.", "Ib плечо 1 прив.", "Ic плечо 1 прив.", "Ia плечо 2 прив.", "Ib плечо 2 прив.", "Ic плечо 2 прив.", "Ia диф.", "Ib диф.", "Ic диф.", "Ia торм.", "Ib торм.", "Ic торм."]
    );
    assert_eq!(g.channels[0].1, "CoeffPriv: (Ia, 1)");
    let zeros = ", 0".repeat(22);
    assert_eq!(g.channels[6].1, format!(r#"Mod:(GrsSumCur: ("Ia плечо 1 прив.", "Ia плечо 2 прив."{zeros}))"#));
    assert_eq!(g.channels[9].1, format!(r#"TormSumCur: (0.5, "Ia плечо 1 прив.", "Ia плечо 2 прив."{zeros})"#));
    assert_eq!(evaluates(&g).len(), 12);
    // The page's group method: every arm's turned currents after every
    // reduction, `Mod:(GrsPoslI?: …)`.
    let g = differential(3, &arms, &GroupComp::Posl, &Braking::Max, &taken());
    assert_eq!(g.channels.len(), 6 + 6 + 3 + 3);
    assert_eq!(g.channels[6], ("Ia плечо 1 прив. пов.".into(), r#"Mod:(GrsPoslIa: ("Ia плечо 1 прив.", "Ib плечо 1 прив.", "Ic плечо 1 прив.", 11, 1))"#.into()));
    assert!(g.channels[12].1.starts_with(r#"Mod:(GrsSumCur: ("Ia плечо 1 прив. пов.""#));
    assert_eq!(evaluates(&g).len(), 18);
    let g = differential(3, &arms, &GroupComp::Triangle, &Braking::Sub { k: "0.5".into() }, &taken());
    assert_eq!(g.channels[7].1, r#"Mod:(GrsTriangleIb: ("Ia плечо 1 прив.", "Ib плечо 1 прив.", "Ic плечо 1 прив.", 11))"#);
    assert_eq!(g.channels[15].1, r#"TormSubCur: ("Ia плечо 1 прив. пов.", "Ia плечо 2 прив. пов.", 0.5)"#);
    assert_eq!(evaluates(&g).len(), 18);
    // One phase is named «Ia».
    assert_eq!(names(&differential(1, &arms, &GroupComp::None, &Braking::Max, &taken()))[0], "Ia плечо 1 прив.");
}

/// A taken name gets "_2", "_3", … (0x48bff0).
#[test]
fn unique_names() {
    let names_taken = vec!["I1".to_string(), "I1_2".to_string()];
    assert_eq!(unique_name("I1", &names_taken), "I1_3");
    assert_eq!(unique_name("I2", &names_taken), "I2");
    // Case is ignored (`_wcsicmp`, Russian locale): Cyrillic folds too.
    assert_eq!(unique_name("i1", &names_taken), "i1_3");
    let cyr = vec!["Ia плечо 1 прив.".to_string()];
    assert_eq!(unique_name("IA ПЛЕЧО 1 ПРИВ.", &cyr), "IA ПЛЕЧО 1 ПРИВ._2");
    // Evaluating a batch twice renames the second set.
    let osc = comtrade::parse(CFG, DAT).unwrap();
    let g = symmetric(Some(&cur(false)), None, &taken());
    let first = evaluate_batch(&osc, &[], &g.channels, false).unwrap();
    let second = evaluate_batch(&osc, &first, &g.channels, false).unwrap();
    assert_eq!(second[0].name, "I1_2");
    // Within one batch a repeated name (two reduction methods) takes the
    // next suffix; a record channel's name — analog or digital — is taken
    // ignoring case.
    let batch: Vec<(String, String)> = [("X", "1"), ("x", "2"), ("ia", "3"), ("TRIP", "4")].iter().map(|(n, f)| (n.to_string(), f.to_string())).collect();
    let out = evaluate_batch(&osc, &[], &batch, false).unwrap();
    let names: Vec<&str> = out.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["X", "x_2", "ia_2", "TRIP_2"]);
}
