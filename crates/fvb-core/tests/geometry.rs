//! Geometry/region behaviour pinned to the decompiled original renderer
//! (ledger L4): `Figure` region builders `0x409c00/0x40a170/0x40a5d0/0x40ae40`
//! and the sequential CombineRgn fold in paint `0x43a4a0`.

use fvb_core::geom::{close_open_polypoint, evaluate, evaluate_in_view, polygon_from_lines, EvalLine, Shape};
use fvb_core::model::LineKind;
use fvb_core::*;
use std::collections::HashMap;

fn one_char(body: &str) -> Characteristic {
    let xml = format!(
        r#"<blocks><block name="b"><characteristic name="c" dimension="OHM">{body}</characteristic></block></blocks>"#
    );
    parse_library(&xml).unwrap().blocks.remove(0).characteristics.remove(0)
}

fn geo(ch: &Characteristic, view: Option<fvb_core::geom::View>) -> CharGeometry {
    let cx = ch.context(&HashMap::new()).unwrap();
    evaluate_in_view(ch, &cx, view).unwrap()
}

/// Even-odd containment over all rings of all shapes.
fn contains(shapes: &[Shape], p: [f64; 2]) -> bool {
    let mut inside = false;
    for Shape::Region(rings) in shapes {
        for ring in rings {
            let n = ring.len();
            for i in 0..n {
                let (a, b) = (ring[i], ring[(i + 1) % n]);
                if (a[1] > p[1]) != (b[1] > p[1]) {
                    let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                    if p[0] < x {
                        inside = !inside;
                    }
                }
            }
        }
    }
    inside
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64, extra: &str) -> String {
    format!(
        r#"<polypoint mode="on" {extra}><line x="{x0}" y="{y0}"/><line x="{x1}" y="{y0}"/><line x="{x1}" y="{y1}"/><line x="{x0}" y="{y1}"/></polypoint>"#
    )
}

/// Figures fold in document order: A, then `not` B, then `or` C gives
/// (A − B) ∪ C — C survives inside the cut-out. Grouping all unions before
/// the differences would erase it.
#[test]
fn combination_is_sequential_in_document_order() {
    let ch = one_char(&format!(
        "{}{}{}",
        square(0.0, 0.0, 10.0, 10.0, ""),
        square(2.0, 2.0, 8.0, 8.0, r#"type="not""#),
        square(4.0, 4.0, 6.0, 6.0, r#"type="or""#),
    ));
    let g = geo(&ch, None);
    assert!(contains(&g.operate.shapes, [1.0, 1.0]));
    assert!(!contains(&g.operate.shapes, [3.0, 3.0]));
    assert!(contains(&g.operate.shapes, [5.0, 5.0]), "later `or` re-adds inside the cut-out");
}

/// The first figure of a mode starts the region; its `type` is ignored.
#[test]
fn first_figure_type_is_ignored() {
    let ch = one_char(&square(0.0, 0.0, 1.0, 1.0, r#"type="and""#));
    assert!(contains(&geo(&ch, None).operate.shapes, [0.5, 0.5]));
    let ch = one_char(&square(0.0, 0.0, 1.0, 1.0, r#"type="not""#));
    assert!(contains(&geo(&ch, None).operate.shapes, [0.5, 0.5]));
}

/// `xor` is accepted (RGN_XOR).
#[test]
fn xor_combines() {
    let ch = one_char(&format!(
        "{}{}",
        square(0.0, 0.0, 2.0, 2.0, ""),
        square(1.0, 1.0, 3.0, 3.0, r#"type="xor""#)
    ));
    let s = geo(&ch, None).operate.shapes;
    assert!(contains(&s, [0.5, 0.5]));
    assert!(!contains(&s, [1.5, 1.5]));
    assert!(contains(&s, [2.5, 2.5]));
}

/// A degenerate first figure still starts the region, so a later `and`
/// leaves nothing (the original's 1x1-pixel stand-in).
#[test]
fn degenerate_first_figure_starts_the_region() {
    let ch = one_char(&format!(
        r#"<circle mode="on" radius="0" xcenter="0" ycenter="0"/>{}"#,
        square(0.0, 0.0, 1.0, 1.0, r#"type="and""#)
    ));
    assert!(geo(&ch, None).operate.shapes.is_empty());
}

/// `mode` is "on" only when spelled so; missing => reset. `closed` is false
/// only for "off"; missing => closed.
#[test]
fn attribute_defaults() {
    let ch = one_char(r#"<circle radius="1" xcenter="0" ycenter="0"/>"#);
    let g = geo(&ch, None);
    assert!(g.operate.shapes.is_empty());
    assert!(contains(&g.reset.shapes, [0.0, 0.0]));
    let ch = one_char(&square(0.0, 0.0, 1.0, 1.0, "").replace(r#"mode="on" "#, r#"mode="on""#));
    assert!(contains(&geo(&ch, Some((-5.0, -5.0, 5.0, 5.0))).operate.shapes, [0.5, 0.5]));
}

/// Polygon vertex i = line[i-1] ∩ line[i]; vertex 0 = last ∩ first.
#[test]
fn polygon_vertex_order() {
    let lines = [
        EvalLine { k: 0.0, b: 1.0, kind: LineKind::Slope },        // y = 1
        EvalLine { k: 0.0, b: 1.0, kind: LineKind::InverseSlope }, // x = 1
        EvalLine { k: 0.0, b: 0.0, kind: LineKind::Slope },        // y = 0
        EvalLine { k: 0.0, b: 0.0, kind: LineKind::InverseSlope }, // x = 0
    ];
    let p = polygon_from_lines(&lines).unwrap();
    assert_eq!(p, vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
}

/// A line whose type is neither 1 nor 2 puts its vertices at the origin.
#[test]
fn invalid_line_type_vertices_at_origin() {
    let lines = [
        EvalLine { k: 0.0, b: 1.0, kind: LineKind::Slope },
        EvalLine { k: 0.0, b: 5.0, kind: LineKind::Invalid },
        EvalLine { k: 0.0, b: 0.0, kind: LineKind::InverseSlope },
    ];
    let p = polygon_from_lines(&lines).unwrap();
    assert_eq!(p, vec![[0.0, 1.0], [0.0, 0.0], [0.0, 0.0]]);
}

/// Ellipse width/height are full axes; positive angle rotates CCW.
#[test]
fn ellipse_axes_and_rotation() {
    let ch = one_char(r#"<ellipse mode="on" xcenter="0" ycenter="0" width="4" height="2" angle="90"/>"#);
    let s = geo(&ch, None).operate.shapes;
    assert!(contains(&s, [0.0, 1.9]), "rotated 90°: the 4-wide axis is vertical");
    assert!(!contains(&s, [1.9, 0.0]));
    let ch = one_char(r#"<ellipse mode="on" xcenter="0" ycenter="0" width="4" height="2" angle="45"/>"#);
    let s = geo(&ch, None).operate.shapes;
    assert!(contains(&s, [1.3, 1.3]) && !contains(&s, [1.3, -1.3]), "CCW in world coordinates");
}

/// The ДТО idiom: an open curve (0,R+1)→(0.0001,R)→(1,R). Painted, it
/// closes through the far box so the region is everything above the curve
/// (left of its direction of travel), extending past the view.
#[test]
fn open_polypoint_closes_through_the_view_box() {
    let ch = one_char(
        r#"<polypoint mode="on" closed="off"><line x="0" y="3"/><line x="0.0001" y="2"/><line x="1" y="2"/></polypoint>"#,
    );
    let view = (-1.0, -1.0, 5.0, 5.0);
    let s = geo(&ch, Some(view)).operate.shapes;
    assert!(contains(&s, [3.0, 4.0]));
    assert!(contains(&s, [4.9, 2.1]), "the last segment is extended to the right");
    assert!(!contains(&s, [3.0, 1.0]));
    assert!(!contains(&s, [-0.5, 4.0]), "left of the extended first segment");
    // Without a view (autoscale pass) it closes straight back: a thin sliver.
    let s = geo(&ch, None).operate.shapes;
    assert!(!contains(&s, [3.0, 4.0]));
}

/// Box hits on the same edge: corners are walked all the way round only
/// when the first point's hit is behind the last point's in the walk.
#[test]
fn open_polypoint_same_edge_cases() {
    let view = (0.0, 0.0, 1.0, 1.0); // box x∈[-2,3], y∈[-2,3]
    // Both ends exit through the top edge. Down-right-up travel: walking
    // back along the top edge from the last hit reaches the first hit
    // directly — the region is the inside of the "cup" (left of travel).
    let cup = [[0.2, 0.8], [0.2, 0.2], [0.8, 0.2], [0.8, 0.8]];
    let ring = close_open_polypoint(&cup, view);
    assert_eq!(ring.len(), 4 + 2);
    assert_eq!(ring[4], [0.8, 3.0]);
    assert_eq!(ring[5], [0.2, 3.0]);
    // Reversed travel: the first hit is behind the last -> all four corners,
    // the region is the outside of the cup.
    let rev: Vec<_> = cup.iter().rev().copied().collect();
    let ring = close_open_polypoint(&rev, view);
    assert_eq!(ring.len(), 4 + 2 + 4);
    assert_eq!(&ring[5..9], &[[-2.0, 3.0], [-2.0, -2.0], [3.0, -2.0], [3.0, 3.0]]);
}

/// End segments whose lines meet beyond both ends close with that point.
#[test]
fn open_polypoint_closes_at_end_line_intersection() {
    let pts = [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 2.0]];
    let ring = close_open_polypoint(&pts, (-10.0, -10.0, 10.0, 10.0));
    assert_eq!(ring.len(), 6);
    let last = ring[5];
    assert!((last[0] - 0.0).abs() < 1e-12 && (last[1] - 3.0).abs() < 1e-12, "{last:?}");
}

/// Every shipped DIF characteristic now paints a filled operate region.
#[test]
fn shipped_dif_characteristics_fill() {
    let lib = load_fvb_bytes(include_bytes!("../../../assets/FastView.fvb")).unwrap();
    let mut n = 0;
    for ch in lib.blocks.iter().flat_map(|b| &b.characteristics) {
        if ch.dimension != Dimension::Dif {
            continue;
        }
        let cx = ch.context(&ch.default_settings()).unwrap();
        let g = evaluate(ch, &cx).unwrap();
        let (x0, y0, x1, y1) = g.bounds.unwrap();
        let view = (x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0);
        let g = evaluate_in_view(ch, &cx, Some(view)).unwrap();
        assert!(!g.operate.shapes.is_empty(), "{} has no operate region", ch.name);
        n += 1;
    }
    assert!(n > 10, "{n} DIF characteristics");
}
