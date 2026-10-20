//! `.fvb` loading and characteristic evaluation pinned to the decompiled
//! original (ledger L5): loader `FUN_0040c7e0`, attribute lookup
//! `FUN_00408ab0`, block evaluation `FUN_00409260` / `SetVariable`
//! `FUN_004083b0`.

use fvb_core::geom::{build_geometry, evaluate_figures};
use fvb_core::*;
use std::collections::HashMap;

fn lib(body: &str) -> Library {
    parse_library(&format!("<blocks><block name=\"B\">{body}</block></blocks>")).unwrap()
}

fn settings(pairs: &[(&str, &str)]) -> Settings {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// A missing expression attribute is stored as "" and only fails when
/// evaluated, with the original's syntax-error message.
#[test]
fn missing_expression_fails_at_evaluation_not_load() {
    let l = lib(r#"<characteristic name="c"><circle mode="on" xcenter="0" ycenter="0"/></characteristic>"#);
    let ch = &l.blocks[0].characteristics[0];
    let cx = ch.context(&HashMap::new()).unwrap();
    let err = evaluate(ch, &cx).unwrap_err();
    assert_eq!(err.formula, "");
    assert_eq!(err.to_string(), ": Синтаксическая ошибка в формуле");
}

/// Value compares are exact and case-sensitive (`CString::Compare`).
#[test]
fn attribute_values_are_case_sensitive() {
    let l = lib(
        r#"<characteristic name="a" dimension="dif"/><characteristic name="b" dimension="DIF"/>
           <characteristic name="c"><circle mode="On" type="AND" radius="1" xcenter="0" ycenter="0"/></characteristic>"#,
    );
    let cs = &l.blocks[0].characteristics;
    assert_eq!(cs[0].dimension, Dimension::Ohm);
    assert_eq!(cs[1].dimension, Dimension::Dif);
    assert_eq!(cs[2].primitives[0].mode(), Mode::Off);
    assert_eq!(cs[2].primitives[0].combine(), Combine::Base);
}

/// The document is walked by structure: block/characteristic/line tags are
/// never checked, unknown figure-level tags are ignored.
#[test]
fn loader_walks_by_structure() {
    let l = parse_library(
        r#"<a><b name="Блок"><c name="Х" dimension="DIF">
             <parameter name="p" default="2"/><note text="ignored"/>
             <polygon mode="on"><q k="0" b="1" type="1"/><q k="0" b="1" type="2"/><q k="0" b="0" type="1"/></polygon>
           </c></b></a>"#,
    )
    .unwrap();
    let ch = &l.blocks[0].characteristics[0];
    assert_eq!((l.blocks[0].name.as_str(), ch.name.as_str()), ("Блок", "Х"));
    assert_eq!(ch.parameters.len(), 1);
    match &ch.primitives[..] {
        [Primitive::Polygon { lines, .. }] => assert_eq!(lines.len(), 3),
        other => panic!("{other:?}"),
    }
}

/// Parameter values are formulas (default text, or the grid's text);
/// defaults see earlier parameters; vars see parameters and earlier vars; a
/// later assignment of the same name wins.
#[test]
fn settings_are_formulas_and_scoping_is_single_pass() {
    let l = lib(
        r#"<characteristic name="c">
             <parameter name="A" default="2"/><parameter name="B" default="a*3"/>
             <var name="v" expression="b+1"/><var name="A" expression="100"/>
           </characteristic>"#,
    );
    let ch = &l.blocks[0].characteristics[0];
    assert_eq!(ch.default_settings()["B"], "a*3");
    let cx = ch.context(&HashMap::new()).unwrap();
    assert_eq!((cx["a"], cx["b"], cx["v"]), (100.0, 6.0, 7.0));
    let cx = ch.context(&settings(&[("A", "2+3")])).unwrap();
    assert_eq!(cx["b"], 15.0);
    // A default cannot see a later parameter.
    let l = lib(r#"<characteristic name="c"><parameter name="A" default="b"/><parameter name="B" default="1"/></characteristic>"#);
    let err = l.blocks[0].characteristics[0].context(&HashMap::new()).unwrap_err();
    assert_eq!(err.to_string(), "b: Переменная не найдена");
}

/// One failing formula abandons the rest of the block: earlier
/// characteristics/figures are updated, later ones keep previous values.
#[test]
fn block_evaluation_stops_at_the_first_error() {
    let circle = |r: &str| format!(r#"<circle mode="on" radius="{r}" xcenter="0" ycenter="0"/>"#);
    let l = lib(&format!(
        r#"<characteristic name="1"><parameter name="r" default="1"/>{}</characteristic>
           <characteristic name="2"><parameter name="r" default="1"/>{}{}</characteristic>
           <characteristic name="3"><parameter name="r" default="1"/>{}</characteristic>"#,
        circle("r"),
        circle("r"),
        circle("r/q"),
        circle("r"),
    ));
    let chars = &l.blocks[0].characteristics;
    let stale = Some(FigureValues::Circle { center: [9.0, 9.0], radius: 9.0 });
    let mut figs: Vec<Vec<Option<FigureValues>>> = vec![vec![stale.clone()], vec![stale.clone(), stale.clone()], vec![stale.clone()]];
    let set: Vec<Settings> = chars.iter().map(|_| settings(&[("r", "2")])).collect();
    let err = evaluate_block(chars.iter().zip(set.iter()).zip(figs.iter_mut()).map(|((c, s), f)| (c, s, f))).unwrap_err();
    assert_eq!(err.to_string(), "r/q: Переменная не найдена");
    let fresh = Some(FigureValues::Circle { center: [0.0, 0.0], radius: 2.0 });
    assert_eq!(figs[0], vec![fresh.clone()]);
    assert_eq!(figs[1], vec![fresh, stale.clone()]);
    assert_eq!(figs[2], vec![stale]);
}

/// Figures never evaluated draw nothing but still start their mode's region.
#[test]
fn unevaluated_figures_are_degenerate() {
    let l = lib(r#"<characteristic name="c"><circle mode="on" radius="1" xcenter="0" ycenter="0"/></characteristic>"#);
    let ch = &l.blocks[0].characteristics[0];
    assert!(build_geometry(ch, &[], None).operate.shapes.is_empty());
    let mut figs = Vec::new();
    evaluate_figures(ch, &ch.context(&HashMap::new()).unwrap(), &mut figs).unwrap();
    assert_eq!(build_geometry(ch, &figs, None).operate.shapes.len(), 1);
}

/// Each diagram loads its own library: the circle diagram keeps non-DIF
/// characteristics, the differential one only DIF; empty blocks are dropped.
/// The shipped library has two mixed blocks that appear in both.
#[test]
fn per_diagram_libraries() {
    let full = load_fvb_bytes(include_bytes!("../../../assets/FastView.fvb")).unwrap();
    let ohm = full.for_dimension(Dimension::Ohm);
    let dif = full.for_dimension(Dimension::Dif);
    assert_eq!((ohm.blocks.len(), dif.blocks.len()), (10, 9));
    let mixed = "БМРЗ - Защита электродвигателей (с 2025 г.в.)";
    assert_eq!(ohm.block(mixed).unwrap().characteristics.len(), 3);
    assert_eq!(dif.block(mixed).unwrap().characteristics.len(), 4);
    for b in &ohm.blocks {
        assert!(b.characteristics.iter().all(|c| c.dimension == Dimension::Ohm));
    }
}
