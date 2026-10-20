use approx::assert_relative_eq;
use fvb_core::expr::{eval_str, Context};
use fvb_core::geom::{line_intersection, polygon_from_lines, EvalLine};
use fvb_core::model::LineKind;
use fvb_core::*;

// Context keys are stored lowercased (case-insensitive resolution).
fn ctx(pairs: &[(&str, f64)]) -> Context {
    pairs.iter().map(|(k, v)| (k.to_lowercase(), *v)).collect()
}

#[test]
fn arithmetic_and_precedence() {
    let c = Context::new();
    assert_relative_eq!(eval_str("2+3*4", &c).unwrap(), 14.0);
    assert_relative_eq!(eval_str("(2+3)*4", &c).unwrap(), 20.0);
    assert_relative_eq!(eval_str("-0.5*4+1.5", &c).unwrap(), -0.5);
    assert_relative_eq!(eval_str("10/2/5", &c).unwrap(), 1.0);
    assert_relative_eq!(eval_str("-2* 3 + 1", &c).unwrap(), -5.0);
}

#[test]
fn trig_is_in_degrees() {
    let c = Context::new();
    assert_relative_eq!(eval_str("sin:(30)", &c).unwrap(), 0.5, epsilon = 1e-9);
    assert_relative_eq!(eval_str("cos:(60)", &c).unwrap(), 0.5, epsilon = 1e-9);
    assert_relative_eq!(eval_str("tg:(45)", &c).unwrap(), 1.0, epsilon = 1e-9);
    // The library idiom tg:(90-F2): a reactance line with F2=50 -> tan(40deg)
    assert_relative_eq!(
        eval_str("tg:(90-F2)", &ctx(&[("F2", 50.0)])).unwrap(),
        40f64.to_radians().tan(),
        epsilon = 1e-9
    );
}

#[test]
fn power_operator() {
    let c = Context::new();
    assert_relative_eq!(eval_str("3^2", &c).unwrap(), 9.0);
    assert_relative_eq!(eval_str("2*3^2", &c).unwrap(), 18.0); // ^ tighter than *
    // Unary minus binds tighter than ^ in the original (0x419f20/0x419fc0).
    assert_relative_eq!(eval_str("-2^2", &c).unwrap(), 4.0);
    // The library idiom: radius = sqrt(xc^2 + p^2)
    assert_relative_eq!(
        eval_str("sqrt:(xc^2+p^2)", &ctx(&[("xc", 3.0), ("p", 4.0)])).unwrap(),
        5.0,
        epsilon = 1e-9
    );
}

#[test]
fn mod_and_sqrt() {
    let c = Context::new();
    assert_relative_eq!(eval_str("mod:(0-7)", &c).unwrap(), 7.0);
    assert_relative_eq!(eval_str("sqrt:(9)", &c).unwrap(), 3.0);
}

#[test]
fn cyrillic_identifiers() {
    let c = ctx(&[("Направленность", 1.0)]);
    assert_relative_eq!(eval_str("-2*Направленность + 1", &c).unwrap(), -1.0);
}

#[test]
fn real_library_expression() {
    // From ДЗ 1 ступень: KZ = Kcm*Zy + (1.05-1)*mod:(Kcm*Zy)
    let c = ctx(&[("Kcm", 0.2), ("Zy", 10.0)]);
    let got = eval_str("Kcm * Zy + (1.05 - 1) * mod:(Kcm * Zy)", &c).unwrap();
    assert_relative_eq!(got, 2.0 + 0.05 * 2.0, epsilon = 1e-9);
}

#[test]
fn line_intersection_mixed_types() {
    // y = x (type1) and x = 0 (type2: x = 0*y + 0) -> intersect at origin.
    let a = EvalLine { k: 1.0, b: 0.0, kind: LineKind::Slope };
    let b = EvalLine { k: 0.0, b: 0.0, kind: LineKind::InverseSlope };
    let p = line_intersection(&a, &b).unwrap();
    assert_relative_eq!(p[0], 0.0, epsilon = 1e-9);
    assert_relative_eq!(p[1], 0.0, epsilon = 1e-9);
}

#[test]
fn polygon_from_four_lines_is_a_quad() {
    // Unit square: top y=1, right x=1, bottom y=0, left x=0, listed in order.
    let lines = vec![
        EvalLine { k: 0.0, b: 1.0, kind: LineKind::Slope },         // y = 1
        EvalLine { k: 0.0, b: 1.0, kind: LineKind::InverseSlope },  // x = 1
        EvalLine { k: 0.0, b: 0.0, kind: LineKind::Slope },         // y = 0
        EvalLine { k: 0.0, b: 0.0, kind: LineKind::InverseSlope },  // x = 0
    ];
    let poly = polygon_from_lines(&lines).unwrap();
    assert_eq!(poly.len(), 4);
    // Corners present (order may wrap): (1,1),(1,0),(0,0),(0,1)
    for want in [[1.0, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]] {
        assert!(
            poly.iter().any(|p| (p[0] - want[0]).abs() < 1e-9 && (p[1] - want[1]).abs() < 1e-9),
            "missing corner {want:?} in {poly:?}"
        );
    }
}

const LIB: &[u8] = include_bytes!("../../../assets/FastView.fvb");

#[test]
fn parses_the_shipped_library() {
    let lib = load_fvb_bytes(LIB).expect("parse library");
    assert_eq!(lib.blocks.len(), 17, "17 device blocks");
    let total: usize = lib.blocks.iter().map(|b| b.characteristics.len()).sum();
    assert_eq!(total, 62, "62 characteristics");
}

#[test]
fn every_characteristic_evaluates_with_defaults() {
    let lib = load_fvb_bytes(LIB).expect("parse");
    let mut evaluated = 0;
    for b in &lib.blocks {
        for ch in &b.characteristics {
            let settings = ch.default_settings();
            let cx = ch.context(&settings).unwrap_or_else(|e| {
                panic!("context for {:?}/{:?}: {e}", b.name, ch.name)
            });
            let geo = evaluate(ch, &cx)
                .unwrap_or_else(|e| panic!("evaluate {:?}/{:?}: {e}", b.name, ch.name));
            // Every characteristic must produce at least one drawable shape.
            let shapes = geo.operate.shapes.len() + geo.reset.shapes.len();
            assert!(shapes > 0, "no shapes for {:?}/{:?}", b.name, ch.name);
            evaluated += 1;
        }
    }
    assert_eq!(evaluated, 62);
}

#[test]
fn distance_stage1_operate_region_is_bounded_and_nonempty() {
    let lib = load_fvb_bytes(LIB).expect("parse");
    let block = lib.block("БМРЗ - Защита линий").expect("line protection block");
    let ch = block
        .characteristics
        .iter()
        .find(|c| c.name == "ДЗ 1 ступень")
        .expect("distance stage 1");
    let settings = ch.default_settings();
    let cx = ch.context(&settings).unwrap();
    let geo = evaluate(ch, &cx).unwrap();
    assert!(geo.operate.shapes.iter().any(|s| matches!(s, Shape::Region(_))));
    let (minx, miny, maxx, maxy) = geo.bounds.expect("bounds");
    assert!(maxx - minx > 0.0 && maxy - miny > 0.0);
    // Reach is on the order of the setpoint impedance (Zcp=10) in secondary ohms.
    assert!(maxy < 1000.0 && maxx < 1000.0, "region should be bounded near the reach");
    let _ = (minx, miny);
}
