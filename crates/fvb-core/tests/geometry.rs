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

/// `0x4387d0`: integer regions per figure at scale 1 — vertices to the
/// nearest unit (ties up), circles outward, a degenerate figure a 1 × 1
/// box at its first point — folded by their ops, then the box's largest
/// |edge|.
#[test]
fn characteristic_extent_in_whole_units() {
    let ext = |body: &str| {
        let ch = one_char(body);
        let cx = ch.context(&HashMap::new()).unwrap();
        let mut figs = Vec::new();
        fvb_core::geom::evaluate_figures(&ch, &cx, &mut figs).unwrap();
        fvb_core::geom::integer_extent(&ch, &figs)
    };
    // (−1.5, −0.2)…(4.2, 3.1): x −1…4, y 0…3 → 4.
    assert_eq!(ext(&square(-1.5, -0.2, 4.2, 3.1, "")), 4.0);
    // A circle r 2.3 at the origin: ⌊−2.3⌋ = −3 (nearest would give 2), but
    // GDI leaves out the box's right edge (⌈2.3⌉ − 1 = 2) and bottom edge,
    // and the path's scan conversion the top row and the left column (clone,
    // #194): −2…2 → 2.
    assert_eq!(ext(r#"<circle mode="on" radius="2.3" xcenter="0" ycenter="0"/>"#), 2.0);
    // The same on a clone's plane scale: a circle whose top is the extreme
    // edge, at 41 above the box's centre line 10 up (⌊−51⌋ = −51, tip row
    // lost → 50), one unit further → 51; the left edge alike.
    assert_eq!(ext(r#"<circle mode="on" radius="41" xcenter="0" ycenter="10"/>"#), 50.0);
    assert_eq!(ext(r#"<circle mode="on" radius="42" xcenter="0" ycenter="10"/>"#), 51.0);
    assert_eq!(ext(r#"<circle mode="on" radius="41" xcenter="-10" ycenter="0"/>"#), 50.0);
    assert_eq!(ext(r#"<circle mode="on" radius="42" xcenter="-10" ycenter="0"/>"#), 51.0);
    // …and the right / bottom edges' own (L9.285): 41 + 10 − 1 = 50 right.
    assert_eq!(ext(r#"<circle mode="on" radius="41" xcenter="10" ycenter="0"/>"#), 50.0);
    // A sub-unit square rounds to nothing: 1 × 1 at (⌈0.1⌉, −⌈0.1⌉) =
    // (1, −1) → 2.
    assert_eq!(ext(&square(0.1, 0.1, 0.4, 0.3, "")), 2.0);
    // Zero height alone is empty too: at (⌈0.6⌉, −⌈0.2⌉) = (1, −1) → its
    // box reaches 2 in x.
    assert_eq!(ext(&square(0.6, 0.2, 3.2, 0.4, "")), 2.0);
    // The fold: a big square cut by a bigger `not` leaves nothing → 0.
    assert_eq!(ext(&format!("{}{}", square(0.0, 0.0, 2.0, 2.0, ""), square(-5.0, -5.0, 5.0, 5.0, r#"type="not""#))), 0.0);
    // An ellipse 5 × 3 at (1, 0): x ⌊−2.5⌋ = −3 … ⌈2.5⌉ − 1 = 2, moved by 1 →
    // −2…3 → 3 (the right edge is the extreme).
    assert_eq!(ext(r#"<ellipse mode="on" xcenter="1" ycenter="0" width="5" height="3" angle="0"/>"#), 3.0);
    // At (−1, 0) the left edge is the extreme: ⌊−2.5⌋ = −3, moved by −1 → −4
    // → 4. The ellipse keeps its left column (clone, #194: 82 wide at x −10
    // scales to 51); the circle's rule would lose it and give 3.
    assert_eq!(ext(r#"<ellipse mode="on" xcenter="-1" ycenter="0" width="5" height="3" angle="0"/>"#), 4.0);
    assert_eq!(ext(r#"<ellipse mode="on" xcenter="-10" ycenter="0" width="82" height="20" angle="0"/>"#), 51.0);
    // The top row is lost: 82 high at y 10 up reaches ⌊−41⌋ + 1 − 10 = −50 →
    // 50 (the circle's top case, radius 41 at y 10, gives 50 too); the right
    // edge ⌈41⌉ − 1 = 40 and the bottom 40 − 10 = 30 stay inside.
    assert_eq!(ext(r#"<ellipse mode="on" xcenter="0" ycenter="10" width="20" height="82" angle="0"/>"#), 50.0);
    assert_eq!(ext(r#"<ellipse mode="on" xcenter="0" ycenter="11" width="20" height="82" angle="0"/>"#), 51.0);
}

/// The plane extent of one figure body (`geom::integer_extent`).
fn extent_of(body: &str) -> f64 {
    let ch = one_char(body);
    let cx = ch.context(&HashMap::new()).unwrap();
    let mut figs = Vec::new();
    fvb_core::geom::evaluate_figures(&ch, &cx, &mut figs).unwrap();
    fvb_core::geom::integer_extent(&ch, &figs)
}

fn polypoint(pts: &[(f64, f64)]) -> String {
    let lines: String = pts.iter().map(|(x, y)| format!(r#"<line x="{x}" y="{y}"/>"#)).collect();
    format!(r#"<polypoint mode="on">{lines}</polypoint>"#)
}

fn polygon(lines: &[(f64, f64, u8)]) -> String {
    let lines: String = lines.iter().map(|(k, b, t)| format!(r#"<line k="{k}" b="{b}" type="{t}"/>"#)).collect();
    format!(r#"<polygon mode="on">{lines}</polygon>"#)
}

/// #226 (clones, a plane scale that steps between a 50 and a 51 unit
/// extent): GDI's path-to-region conversion loses a polygon's lone top
/// vertex row, as the circle's. A flat top edge, a left, right or bottom apex
/// and a thin apex's second row stay; a slanting top edge's first row is its
/// lone vertex's too.
#[test]
fn a_polygons_lone_top_vertex_row_is_lost_in_the_plane_extent() {
    // The flat top edge keeps its row at 51 and at 50.
    assert_eq!(extent_of(&polypoint(&[(-5.0, 51.0), (5.0, 51.0), (5.0, 40.0), (-5.0, 40.0)])), 51.0);
    assert_eq!(extent_of(&polypoint(&[(-5.0, 50.0), (5.0, 50.0), (5.0, 40.0), (-5.0, 40.0)])), 50.0);
    // A diamond whose apex reaches 51 scales as 50; at 52 one row is lost.
    assert_eq!(extent_of(&polypoint(&[(0.0, 51.0), (6.0, 45.0), (0.0, 39.0), (-6.0, 45.0)])), 50.0);
    assert_eq!(extent_of(&polypoint(&[(0.0, 52.0), (6.0, 46.0), (0.0, 40.0), (-6.0, 46.0)])), 51.0);
    // A thin triangle (0.2 across per row) loses the apex row only: its next
    // row, 0.4 wide, still holds a lattice point.
    assert_eq!(extent_of(&polypoint(&[(0.0, 52.0), (2.0, 42.0), (-2.0, 42.0)])), 51.0);
    assert_eq!(extent_of(&polypoint(&[(0.0, 54.0), (2.0, 44.0), (-2.0, 44.0)])), 53.0);
    // A wide shallow apex, and a sector's: polygons of lines, an apex rounded
    // from 50.6.
    assert_eq!(extent_of(&polypoint(&[(0.0, 51.0), (30.0, 45.0), (-30.0, 45.0)])), 50.0);
    assert_eq!(extent_of(&polygon(&[(1.0, 51.0, 1), (-1.0, 51.0, 1), (0.0, 40.0, 1)])), 50.0);
    assert_eq!(extent_of(&polygon(&[(1.7, 50.6, 1), (-1.7, 50.6, 1), (0.0, 40.0, 1)])), 50.0);
    // A top edge that slants starts at its lone left vertex: its first row has
    // no pixel (one row, and with two rows' slant still one).
    assert_eq!(extent_of(&polypoint(&[(-5.0, 51.0), (5.0, 50.0), (5.0, 40.0), (-5.0, 40.0)])), 50.0);
    assert_eq!(extent_of(&polypoint(&[(-5.0, 51.0), (5.0, 49.0), (5.0, 40.0), (-5.0, 40.0)])), 50.0);
    // A left, right and bottom apex keep their pixels (the right and bottom
    // edges are the box's open sides), also a thin one.
    assert_eq!(extent_of(&polypoint(&[(-51.0, 0.0), (-45.0, 6.0), (-39.0, 0.0), (-45.0, -6.0)])), 51.0);
    assert_eq!(extent_of(&polypoint(&[(-51.0, 0.0), (-41.0, 2.0), (-41.0, -2.0)])), 51.0);
    assert_eq!(extent_of(&polypoint(&[(51.0, 0.0), (45.0, 6.0), (39.0, 0.0), (45.0, -6.0)])), 51.0);
    assert_eq!(extent_of(&polypoint(&[(0.0, -51.0), (6.0, -45.0), (0.0, -39.0), (-6.0, -45.0)])), 51.0);
    assert_eq!(extent_of(&polypoint(&[(0.0, -52.0), (2.0, -42.0), (-2.0, -42.0)])), 52.0);
    assert_eq!(extent_of(&polygon(&[(0.5, 25.5, 1), (0.0, -40.0, 2), (-0.5, -25.5, 1)])), 51.0);
}

/// #242 (clones, 94 turned ellipses at the edge of the 50 | 51 step): the
/// original draws the ellipse into a path, turns the points and converts the
/// turned path to a region, so the row GDI loses is the turned shape's top
/// row, in device space. At 90° the box's left, right and bottom edges stay
/// and its top row (the unturned left side) goes.
#[test]
fn a_turned_ellipses_top_row_is_lost_after_the_turn() {
    let e = |cx: f64, cy: f64, w: u32, h: u32, angle: i32| {
        extent_of(&format!(r#"<ellipse mode="on" xcenter="{cx}" ycenter="{cy}" width="{w}" height="{h}" angle="{angle}"/>"#))
    };
    // 90°: 82 × 20 stands up; its extreme edge is the extent's, at each side.
    assert_eq!((e(0.0, 11.0, 82, 20, 90), e(0.0, 12.0, 82, 20, 90)), (50.0, 51.0), "top");
    assert_eq!((e(0.0, -9.0, 82, 20, 90), e(0.0, -10.0, 82, 20, 90)), (50.0, 51.0), "bottom");
    assert_eq!((e(-9.0, 0.0, 20, 82, 90), e(-10.0, 0.0, 20, 82, 90)), (50.0, 51.0), "left");
    assert_eq!((e(10.0, 0.0, 20, 82, 90), e(11.0, 0.0, 20, 82, 90)), (50.0, 51.0), "right");
    // The same with a half-unit centre (rounded up after the sign flip).
    assert_eq!((e(0.0, 11.5, 82, 20, 90), e(0.0, 12.5, 82, 20, 90)), (50.0, 51.0), "top, half unit");
    // 45°.
    assert_eq!((e(0.0, 21.0, 82, 20, 45), e(0.0, 22.0, 82, 20, 45)), (50.0, 51.0), "45 top");
    assert_eq!((e(0.0, -20.0, 82, 20, 45), e(0.0, -21.0, 82, 20, 45)), (50.0, 51.0), "45 bottom");
    assert_eq!((e(21.0, 0.0, 82, 20, 45), e(22.0, 0.0, 82, 20, 45)), (50.0, 51.0), "45 right");
    assert_eq!((e(-21.0, 0.0, 82, 20, 45), e(-22.0, 0.0, 82, 20, 45)), (50.0, 51.0), "45 left");
    // Other angles and sizes, as measured.
    assert_eq!((e(0.0, 28.0, 80, 24, 30), e(0.0, 29.0, 80, 24, 30)), (50.0, 51.0), "30 top");
    assert_eq!((e(28.0, 0.0, 80, 24, 60), e(29.0, 0.0, 80, 24, 60)), (50.0, 51.0), "60 right");
    assert_eq!((e(0.0, 22.0, 82, 20, 135), e(0.0, 23.0, 82, 20, 135)), (50.0, 51.0), "135 top");
    assert_eq!((e(0.0, 28.0, 82, 20, -30), e(0.0, 29.0, 82, 20, -30)), (50.0, 51.0), "-30 top");
    assert_eq!((e(0.0, 36.0, 83, 19, 15), e(0.0, 37.0, 83, 19, 15)), (50.0, 51.0), "15 top");
    assert_eq!((e(-26.0, 0.0, 66, 26, 225), e(-27.0, 0.0, 66, 26, 225)), (50.0, 51.0), "225 left");
}

/// #242: the Bezier flattening's step count is not fixed. Four clone probes (a 50 extent in the original,
/// each at a different size and angle) that a fixed 8 or 16 steps per curve push to 51: the fitted law
/// (a power of two from the second differences) gives 50.
#[test]
fn the_flattening_of_a_turned_ellipse_is_not_a_fixed_step_count() {
    let e = |cx: f64, cy: f64, w: u32, h: u32, angle: i32| {
        extent_of(&format!(r#"<ellipse mode="on" xcenter="{cx}" ycenter="{cy}" width="{w}" height="{h}" angle="{angle}"/>"#))
    };
    assert_eq!(e(0.0, 33.0, 47, 10, 50), 50.0);
    assert_eq!(e(0.0, 31.0, 31, 48, 45), 50.0);
    assert_eq!(e(34.0, 0.0, 33, 21, 12), 50.0);
    assert_eq!(e(0.0, 31.0, 35, 41, 325), 50.0);
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
