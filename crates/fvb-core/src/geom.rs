//! Geometry engine: evaluate a characteristic's figures into operate/reset
//! regions, reproducing the original renderer (ledger L4).
//!
//! Each figure (`PolygonF`, `PolypointF`, `Circle`, `EllipseF` in the original)
//! builds its own region through a GDI path (`BeginPath` / `Polygon|Ellipse` /
//! `EndPath` / `PathToRegion`, ALTERNATE fill). Regions are then folded
//! **sequentially in document order, separately for each mode**: the first
//! figure of a mode starts the region (its `type` is ignored) and every later
//! one is combined into it with `CombineRgn(rgn, rgn, fig, type)` —
//! `and`/`or`/`xor`/`not` = RGN_AND/OR/XOR/DIFF (paint `0x43a4a0`).
//!
//! An open polypoint (`closed="off"`) is still a filled region: while painting,
//! the original closes it through a box five view-widths wide around the window
//! (`0x40ae40`, see [`close_open_polypoint`]); without a view (its autoscale
//! pass) it closes straight back to the first point.
//!
//! The original works in integer pixels (round-half-up world→device); we keep
//! world coordinates, so edges can differ from the original by under a pixel.

use crate::expr::{Context, Expr, FormulaError};
use crate::model::*;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;

/// A point in the plane.
pub type Pt = [f64; 2];
/// A closed contour (auto-closed by `i_overlay`).
type Contour = Vec<Pt>;
/// A shape = outer contour followed by hole contours.
type IShape = Vec<Contour>;
/// A set of shapes.
type IShapes = Vec<IShape>;

/// The visible world rectangle `(minx, miny, maxx, maxy)` of the plot.
pub type View = (f64, f64, f64, f64);

/// A filled region: `[0]` is the outer ring, the rest are holes.
#[derive(Debug, Clone)]
pub enum Shape {
    Region(Vec<Vec<Pt>>),
}

/// The evaluated geometry of a characteristic for one [`Mode`].
#[derive(Debug, Clone, Default)]
pub struct ModeGeometry {
    pub shapes: Vec<Shape>,
}

/// Evaluated geometry of a whole characteristic.
#[derive(Debug, Clone, Default)]
pub struct CharGeometry {
    pub operate: ModeGeometry,
    pub reset: ModeGeometry,
    /// Overall data bounds `(minx, miny, maxx, maxy)` across all shapes.
    pub bounds: Option<(f64, f64, f64, f64)>,
}

/// Half-extent beyond which a consecutive-line intersection is treated as a
/// runaway (near-parallel edges) and dropped. The original has no guard: the
/// inf/NaN becomes `0x80000000` in device space and GDI fails.
const BIG: f64 = 1.0e6;

/// Determinant tolerance used by the polypoint closing code (`0x40ae40`).
const DET_EPS: f64 = 1e-6;

fn simplify_ring(ring: Contour) -> IShapes {
    if ring.len() < 3 {
        return Vec::new();
    }
    let subj: IShape = vec![ring];
    subj.overlay(&Vec::<IShape>::new(), OverlayRule::Subject, FillRule::EvenOdd)
}

/// `CombineRgn(rgn, rgn, fig, op)`.
fn combine(rgn: IShapes, fig: &IShapes, op: Combine) -> IShapes {
    let rule = match op {
        Combine::And => OverlayRule::Intersect,
        Combine::Or | Combine::Base => OverlayRule::Union,
        Combine::Xor => OverlayRule::Xor,
        Combine::Not => OverlayRule::Difference,
    };
    match (rgn.is_empty(), fig.is_empty()) {
        (true, true) => Vec::new(),
        (true, false) => match rule {
            OverlayRule::Union | OverlayRule::Xor => fig.clone(),
            _ => Vec::new(),
        },
        (false, true) => match rule {
            OverlayRule::Intersect => Vec::new(),
            _ => rgn,
        },
        (false, false) => rgn.overlay(fig, rule, FillRule::NonZero),
    }
}

/// A figure's evaluated numbers — the output of the original's figure vtable
/// slot 2, kept between evaluations.
#[derive(Debug, Clone, PartialEq)]
pub enum FigureValues {
    Polygon(Vec<EvalLine>),
    PolyPoint(Vec<Pt>),
    Circle { center: Pt, radius: f64 },
    Ellipse { center: Pt, width: f64, height: f64, angle: f64 },
}

/// Evaluate one figure's expressions.
pub fn evaluate_figure(p: &Primitive, ctx: &Context) -> Result<FigureValues, FormulaError> {
    Ok(match p {
        Primitive::Polygon { lines, .. } => FigureValues::Polygon(eval_lines(lines, ctx)?),
        Primitive::PolyPoint { points, .. } => FigureValues::PolyPoint(eval_points(points, ctx)?),
        Primitive::Circle {
            radius, xcenter, ycenter, ..
        } => FigureValues::Circle {
            center: [xcenter.eval_formula(ctx)?, ycenter.eval_formula(ctx)?],
            radius: radius.eval_formula(ctx)?,
        },
        Primitive::Ellipse {
            xcenter, ycenter, width, height, angle, ..
        } => FigureValues::Ellipse {
            center: [xcenter.eval_formula(ctx)?, ycenter.eval_formula(ctx)?],
            width: width.eval_formula(ctx)?,
            height: height.eval_formula(ctx)?,
            angle: angle.eval_formula(ctx)?,
        },
    })
}

/// Evaluate a characteristic's figures in document order into `figs` (one slot
/// per figure; `None` = never evaluated). On the first error evaluation stops:
/// earlier figures hold their new values, the failing and later ones keep
/// their previous ones — there is no per-figure fallback in the original.
pub fn evaluate_figures(
    ch: &Characteristic,
    ctx: &Context,
    figs: &mut Vec<Option<FigureValues>>,
) -> Result<(), FormulaError> {
    figs.resize(ch.primitives.len(), None);
    for (p, slot) in ch.primitives.iter().zip(figs.iter_mut()) {
        *slot = Some(evaluate_figure(p, ctx)?);
    }
    Ok(())
}

/// Evaluate a whole block as `FUN_00409260` does after any settings change:
/// for each characteristic in order, rebuild its variable map from its
/// settings and vars, then evaluate its figures. One `try` covers the loop, so
/// the first error abandons the rest of the block — characteristics after it
/// keep their previous figure values.
pub fn evaluate_block<'a>(
    items: impl IntoIterator<Item = (&'a Characteristic, &'a Settings, &'a mut Vec<Option<FigureValues>>)>,
) -> Result<(), FormulaError> {
    for (ch, settings, figs) in items {
        let ctx = ch.context(settings)?;
        evaluate_figures(ch, &ctx, figs)?;
    }
    Ok(())
}

/// Evaluate every figure of `ch` with no view: open polypoints close straight
/// back to their first point (the original's autoscale pass).
pub fn evaluate(ch: &Characteristic, ctx: &Context) -> Result<CharGeometry, FormulaError> {
    evaluate_in_view(ch, ctx, None)
}

/// Evaluate every figure of `ch` as painted in a plot showing `view`: open
/// polypoints are closed through the box around that view.
pub fn evaluate_in_view(
    ch: &Characteristic,
    ctx: &Context,
    view: Option<View>,
) -> Result<CharGeometry, FormulaError> {
    let mut figs = Vec::new();
    evaluate_figures(ch, ctx, &mut figs)?;
    Ok(build_geometry(ch, &figs, view))
}

/// Build the operate and reset regions from evaluated figure values (the
/// original's vtable slot 3 + the CombineRgn fold of paint `0x43a4a0`).
pub fn build_geometry(ch: &Characteristic, figs: &[Option<FigureValues>], view: Option<View>) -> CharGeometry {
    let operate = build_mode(ch, figs, Mode::On, view);
    let reset = build_mode(ch, figs, Mode::Off, view);
    let mut geo = CharGeometry {
        operate,
        reset,
        bounds: None,
    };
    geo.bounds = compute_bounds(&geo);
    geo
}

/// True if any figure of `ch` is an open polypoint, i.e. its geometry depends
/// on the view.
pub fn depends_on_view(ch: &Characteristic) -> bool {
    ch.primitives
        .iter()
        .any(|p| matches!(p, Primitive::PolyPoint { closed: false, .. }))
}

fn build_mode(ch: &Characteristic, figs: &[Option<FigureValues>], mode: Mode, view: Option<View>) -> ModeGeometry {
    // `None` until the first figure of this mode starts the region. A figure
    // whose region is degenerate (zero area) still starts it: the original
    // substitutes a 1x1-pixel rectangle, which we treat as empty. A figure
    // that was never evaluated is treated as degenerate.
    let mut region: Option<IShapes> = None;
    for (i, p) in ch.primitives.iter().enumerate().filter(|(_, p)| p.mode() == mode) {
        let fig = match figs.get(i) {
            Some(Some(v)) => figure_region(p, v, view),
            _ => Vec::new(),
        };
        region = Some(match region {
            None => fig,
            Some(r) => combine(r, &fig, p.combine()),
        });
    }
    let shapes = region
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !s.is_empty() && s[0].len() >= 3)
        .map(Shape::Region)
        .collect();
    ModeGeometry { shapes }
}

/// The region one figure contributes (its vtable slot 3).
fn figure_region(p: &Primitive, v: &FigureValues, view: Option<View>) -> IShapes {
    match v {
        FigureValues::PolyPoint(pts) => {
            let closed = !matches!(p, Primitive::PolyPoint { closed: false, .. });
            match (closed, view) {
                (false, Some(view)) => simplify_ring(close_open_polypoint(pts, view)),
                _ => simplify_ring(pts.clone()),
            }
        }
        FigureValues::Polygon(lines) => polygon_from_lines(lines).map(simplify_ring).unwrap_or_default(),
        FigureValues::Circle { center, radius } => {
            if radius.abs() < 1e-9 {
                return Vec::new();
            }
            simplify_ring(ellipse_points(*center, radius.abs(), radius.abs(), 0.0))
        }
        FigureValues::Ellipse {
            center,
            width,
            height,
            angle,
        } => {
            // width/height are full axes (the original halves them).
            if width.abs() < 1e-9 || height.abs() < 1e-9 {
                return Vec::new();
            }
            simplify_ring(ellipse_points(*center, width.abs() / 2.0, height.abs() / 2.0, *angle))
        }
    }
}

/// GDI's world → device rounding at scale 1: nearest, ties up.
fn rnd(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// The bounding box of a set of shapes.
fn shapes_box(s: &IShapes) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    for p in s.iter().flatten().flatten() {
        b = Some(match b {
            None => (p[0], p[1], p[0], p[1]),
            Some((l, t, r, bt)) => (l.min(p[0]), t.min(p[1]), r.max(p[0]), bt.max(p[1])),
        });
    }
    b
}

/// A 1 × 1 rectangle region at a device point.
fn unit_rect(x: f64, y: f64) -> IShapes {
    vec![vec![vec![[x, y], [x + 1.0, y], [x + 1.0, y + 1.0], [x, y + 1.0]]]]
}

/// A characteristic's extent for the plane scale (`0x4387d0`): for each
/// mode every figure builds its own integer region at scale 1, origin 0
/// (device y = −world y), the regions fold by their ops as when painting,
/// and the extent is the largest |edge| of the fold's box (`GetRgnBox`).
/// Per figure: polygon / polypoint vertices round to the nearest unit,
/// ties up (`0x409c00`, `0x40ae40`); a circle is the ellipse of its box
/// rounded outward (`0x40a170`); an ellipse is the axis-aligned one of
/// ⌊−a/2⌋…⌈a/2⌉ × ⌊−b/2⌋…⌈b/2⌉ turned, rounded and moved to (R(cx),
/// R(−cy)) (`0x40a5d0`). A figure whose region is empty — or whose box
/// has no width or no height — is a 1 × 1 rectangle instead: a polygon's
/// at its first point (⌈x⌉, −⌈y⌉), (0, 0) without one; a circle's at
/// (⌈cx⌉, ⌈cy⌉) and an ellipse's at (R(cx), R(cy)), neither flipped.
pub fn integer_extent(ch: &Characteristic, figs: &[Option<FigureValues>]) -> f64 {
    let mut ext = 0.0f64;
    for mode in [Mode::On, Mode::Off] {
        let mut region: Option<IShapes> = None;
        for (i, p) in ch.primitives.iter().enumerate().filter(|(_, p)| p.mode() == mode) {
            let fig = match figs.get(i) {
                Some(Some(v)) => device_region(v),
                _ => unit_rect(0.0, 0.0),
            };
            region = Some(match region {
                None => fig,
                Some(r) => combine(r, &fig, p.combine()),
            });
        }
        if let Some((l, t, r, b)) = region.as_ref().and_then(shapes_box) {
            ext = ext.max(l.abs()).max(t.abs()).max(r.abs()).max(b.abs());
        }
    }
    ext
}

/// One figure's integer region at scale 1 (see [`integer_extent`]).
fn device_region(v: &FigureValues) -> IShapes {
    let dev = |p: &Pt| [rnd(p[0]), -rnd(p[1])];
    let (ring, fallback): (Contour, Pt) = match v {
        FigureValues::PolyPoint(pts) => (pts.iter().map(dev).collect(), pts.first().map_or([0.0, 0.0], |p| [p[0].ceil(), -p[1].ceil()])),
        FigureValues::Polygon(lines) => {
            let first = lines.last().zip(lines.first()).and_then(|(a, b)| line_intersection(a, b));
            let ring = polygon_from_lines(lines).map(|pts| pts.iter().map(dev).collect()).unwrap_or_default();
            (ring, first.map_or([0.0, 0.0], |p| [p[0].ceil(), -p[1].ceil()]))
        }
        FigureValues::Circle { center: [cx, cy], radius } => {
            let r = radius.abs();
            let (l, t, rt, b) = ((cx - r).floor(), (-(cy + r)).floor(), (cx + r).ceil(), (r - cy).ceil());
            let ring = ellipse_points([(l + rt) / 2.0, (t + b) / 2.0], (rt - l) / 2.0, (b - t) / 2.0, 0.0);
            (ring, [cx.ceil(), cy.ceil()])
        }
        FigureValues::Ellipse { center: [cx, cy], width, height, angle } => {
            let (a, b) = (width.abs(), height.abs());
            let (l, t, r, bt) = ((-a / 2.0).floor(), (-b / 2.0).floor(), (a / 2.0).ceil(), (b / 2.0).ceil());
            let (ox, oy) = (rnd(*cx), rnd(-cy));
            // Device y points down: the turn is mirrored (the box is the
            // same either way).
            let ring = ellipse_points([(l + r) / 2.0, (t + bt) / 2.0], (r - l) / 2.0, (bt - t) / 2.0, -angle)
                .iter()
                .map(|p| [rnd(p[0]) + ox, rnd(p[1]) + oy])
                .collect();
            (ring, [rnd(*cx), rnd(*cy)])
        }
    };
    let region = simplify_ring(ring);
    match shapes_box(&region) {
        Some((l, t, r, b)) if r > l && b > t => region,
        _ => unit_rect(fallback[0], fallback[1]),
    }
}

/// Close an open polypoint the way `0x40ae40` does while painting.
///
/// 1. If the lines of the first segment (p1→p0) and the last segment
///    (p[n-2]→p[n-1]) meet beyond both ends, that point closes the polygon.
/// 2. Otherwise each end segment is extended past its end point onto the
///    lines of a box spanning the view plus two view-sizes on every side
///    (corners TL, TR, BR, BL; edge i runs corner i → i+1). The nearest hit
///    on the outward side wins. The polygon continues from the last point to
///    its hit, walks the box corners backwards (counter-clockwise in world
///    coordinates) to the first point's hit and closes. If either end misses,
///    nothing is added.
///
/// The region therefore lies to the left of the path p0 → p[n-1].
pub fn close_open_polypoint(pts: &[Pt], view: View) -> Vec<Pt> {
    let mut out = pts.to_vec();
    let n = pts.len();
    if n < 2 {
        return out;
    }
    let (p0, p1, pl, pm) = (pts[0], pts[1], pts[n - 1], pts[n - 2]);

    // 1. End-segment lines meeting beyond both ends.
    let (dx1, dy1) = (p1[0] - p0[0], p1[1] - p0[1]);
    let (fx, fy) = (pm[0] - pl[0], pm[1] - pl[1]);
    let det = dx1 * fy - fx * dy1;
    if det.abs() >= DET_EPS {
        let d22 = p0[1] * p1[0] - p1[1] * p0[0];
        let d1 = pm[0] * pl[1] - pl[0] * pm[1];
        let y = (d22 * fy - d1 * dy1) / det;
        let x = (d22 * fx - d1 * dx1) / det;
        if (x - p0[0]).abs() <= (x - p1[0]).abs()
            && (y - p0[1]).abs() <= (y - p1[1]).abs()
            && (x - pl[0]).abs() <= (x - pm[0]).abs()
            && (y - pl[1]).abs() <= (y - pm[1]).abs()
        {
            out.push([x, y]);
            return out;
        }
    }

    // 2. Rays onto the box around the view.
    let (vx0, vy0, vx1, vy1) = view;
    let (w, h) = (vx1 - vx0, vy1 - vy0);
    let (xl, xr, yt, yb) = (vx0 - 2.0 * w, vx1 + 2.0 * w, vy1 + 2.0 * h, vy0 - 2.0 * h);
    let corners: [Pt; 4] = [[xl, yt], [xr, yt], [xr, yb], [xl, yb]];
    // Per end: (edge index, hit). End 0 extends p1→p0, end 1 extends p[n-2]→p[n-1].
    let mut hits: [Option<(usize, Pt)>; 2] = [None, None];
    for (end, (tip, inner)) in [(p0, p1), (pl, pm)].into_iter().enumerate() {
        let (dx, dy) = (inner[0] - tip[0], inner[1] - tip[1]);
        for i in 0..4 {
            let (ci, cj) = (corners[i], corners[(i + 1) % 4]);
            let (ex, ey) = (ci[0] - cj[0], ci[1] - cj[1]);
            let det = ey * dx - ex * dy;
            if det.abs() < DET_EPS {
                continue;
            }
            let d1 = tip[1] * inner[0] - inner[1] * tip[0];
            let d22 = ci[0] * cj[1] - ci[1] * cj[0];
            let y = (ey * d1 - d22 * dy) / det;
            let x = (ex * d1 - d22 * dx) / det;
            if (x - tip[0]).abs() > (x - inner[0]).abs() || (y - tip[1]).abs() > (y - inner[1]).abs() {
                continue;
            }
            let better = match hits[end] {
                None => true,
                Some((_, prev)) => {
                    (x - inner[0]).abs() < (prev[0] - inner[0]).abs()
                        || (y - inner[1]).abs() < (prev[1] - inner[1]).abs()
                }
            };
            if better {
                hits[end] = Some((i, [x, y]));
            }
        }
    }
    let (Some((e0, h0)), Some((e1, h1))) = (hits[0], hits[1]) else {
        return out;
    };
    out.push(h1);
    let walk: Vec<usize> = if e0 < e1 {
        (e0 + 1..=e1).rev().collect()
    } else if e1 < e0 {
        (0..=e1).rev().chain((e0 + 1..=3).rev()).collect()
    } else {
        // Both on one edge: go all the way round only when the first point's
        // hit lies behind the last point's hit in the walking direction.
        let around = match e0 {
            0 => h1[0] < h0[0],
            1 => h0[1] < h1[1],
            2 => h0[0] < h1[0],
            _ => h1[1] < h0[1],
        };
        if around {
            (0..4).map(|k| (e0 + 4 - k) % 4).collect()
        } else {
            Vec::new()
        }
    };
    out.extend(walk.into_iter().map(|k| corners[k]));
    out.push(h0);
    out
}

/// Evaluated bounding line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvalLine {
    pub k: f64,
    pub b: f64,
    pub kind: LineKind,
}

fn eval_lines(lines: &[PolyLine], ctx: &Context) -> Result<Vec<EvalLine>, FormulaError> {
    lines
        .iter()
        .map(|l| {
            Ok(EvalLine {
                k: l.k.eval_formula(ctx)?,
                b: l.b.eval_formula(ctx)?,
                kind: l.kind,
            })
        })
        .collect()
}

fn eval_points(points: &[(Expr, Expr)], ctx: &Context) -> Result<Vec<Pt>, FormulaError> {
    points
        .iter()
        .map(|(x, y)| Ok([x.eval_formula(ctx)?, y.eval_formula(ctx)?]))
        .collect()
}

/// Intersection of a polygon's previous line `a` with the current line `b`,
/// with the original's per-type formulas (`0x409860`); `None` if not finite
/// (parallel lines). A line of invalid type puts the vertex at the origin.
pub fn line_intersection(a: &EvalLine, b: &EvalLine) -> Option<Pt> {
    let (k1, b1, k2, b2) = (a.k, a.b, b.k, b.b);
    let p = match (a.kind, b.kind) {
        (LineKind::Invalid, _) | (_, LineKind::Invalid) => [0.0, 0.0],
        (LineKind::Slope, LineKind::Slope) => {
            let x = (b2 - b1) / (k1 - k2);
            [x, k1 * x + b1]
        }
        (LineKind::InverseSlope, LineKind::InverseSlope) => {
            let y = (b2 - b1) / (k1 - k2);
            [k1 * y + b1, y]
        }
        (LineKind::Slope, LineKind::InverseSlope) => {
            let y = (k1 * b2 + b1) / (1.0 - k2 * k1);
            [k2 * y + b2, y]
        }
        (LineKind::InverseSlope, LineKind::Slope) => {
            let x = (k1 * b2 + b1) / (1.0 - k2 * k1);
            [x, k2 * x + b2]
        }
    };
    (p[0].is_finite() && p[1].is_finite()).then_some(p)
}

/// Build the vertex loop of a `<polygon>`: vertex i = line[i-1] ∩ line[i]
/// (vertex 0 = last ∩ first), as fed to GDI `Polygon` by `0x409c00`.
/// Self-intersections are resolved by the even-odd fill in [`simplify_ring`].
pub fn polygon_from_lines(lines: &[EvalLine]) -> Option<Vec<Pt>> {
    let n = lines.len();
    if n < 3 {
        return None;
    }
    let pts: Vec<Pt> = (0..n)
        .filter_map(|i| line_intersection(&lines[(i + n - 1) % n], &lines[i]))
        .filter(|p| p[0].abs() < BIG && p[1].abs() < BIG)
        .collect();
    (pts.len() >= 3).then_some(pts)
}

/// Sample an (optionally rotated) ellipse into a polygon ring. `angle` degrees.
pub fn ellipse_points(center: Pt, rx: f64, ry: f64, angle_deg: f64) -> Vec<Pt> {
    const SEG: usize = 180;
    let a = angle_deg.to_radians();
    let (sa, ca) = a.sin_cos();
    (0..SEG)
        .map(|i| {
            let t = (i as f64) * std::f64::consts::TAU / (SEG as f64);
            let (st, ct) = t.sin_cos();
            let (ex, ey) = (rx * ct, ry * st);
            [center[0] + ex * ca - ey * sa, center[1] + ex * sa + ey * ca]
        })
        .collect()
}

fn compute_bounds(geo: &CharGeometry) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    let mut acc = |p: &Pt| {
        b = Some(match b {
            None => (p[0], p[1], p[0], p[1]),
            Some((minx, miny, maxx, maxy)) => (
                minx.min(p[0]),
                miny.min(p[1]),
                maxx.max(p[0]),
                maxy.max(p[1]),
            ),
        });
    };
    for mg in [&geo.operate, &geo.reset] {
        for s in &mg.shapes {
            match s {
                Shape::Region(rings) => rings.iter().flatten().for_each(&mut acc),
            }
        }
    }
    b
}
