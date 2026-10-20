//! Geometry engine: evaluate a characteristic's primitives into drawable
//! shapes and combine them into operate/reset regions.
//!
//! Mirrors the original renderer (`CircleDiagramWnd`): each `<polygon>` becomes
//! an ordered vertex loop (successive intersections of its bounding lines, as
//! fed to GDI `Polygon`), filled by the even-odd rule, and primitives combine
//! with region booleans (`CombineRgn`: `type=and|or|not` -> intersect / union /
//! difference). Here we use `i_overlay`, which — like the original GDI path —
//! tolerates self-intersecting loops (e.g. the directional РНМ characteristic).

use crate::expr::{Context, Expr, ExprError};
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

/// A drawn shape: either a filled region (outer ring + holes) or an open polyline.
#[derive(Debug, Clone)]
pub enum Shape {
    /// Filled region: `[0]` is the outer ring, the rest are holes.
    Region(Vec<Vec<Pt>>),
    /// Open polyline (e.g. a differential restraint curve).
    Polyline(Vec<Pt>),
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
/// runaway (near-parallel edges) and dropped.
const BIG: f64 = 1.0e6;

fn empty_clip() -> IShapes {
    Vec::new()
}

fn simplify_ring(ring: Contour) -> IShapes {
    if ring.len() < 3 {
        return Vec::new();
    }
    let subj: IShape = vec![ring];
    subj.overlay(&empty_clip(), OverlayRule::Subject, FillRule::EvenOdd)
}

fn union(a: &IShapes, b: &IShapes) -> IShapes {
    if a.is_empty() {
        return b.clone();
    }
    if b.is_empty() {
        return a.clone();
    }
    a.overlay(b, OverlayRule::Union, FillRule::NonZero)
}

fn intersect(a: &IShapes, b: &IShapes) -> IShapes {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    a.overlay(b, OverlayRule::Intersect, FillRule::NonZero)
}

fn difference(a: &IShapes, b: &IShapes) -> IShapes {
    if a.is_empty() || b.is_empty() {
        return a.clone();
    }
    a.overlay(b, OverlayRule::Difference, FillRule::NonZero)
}

/// Evaluate every primitive of `ch` with the given `ctx` and build the operate
/// and reset geometry.
pub fn evaluate(ch: &Characteristic, ctx: &Context) -> Result<CharGeometry, ExprError> {
    let operate = build_mode(ch, ctx, Mode::On)?;
    let reset = build_mode(ch, ctx, Mode::Off)?;
    let mut geo = CharGeometry {
        operate,
        reset,
        bounds: None,
    };
    geo.bounds = compute_bounds(&geo);
    Ok(geo)
}

fn build_mode(ch: &Characteristic, ctx: &Context, mode: Mode) -> Result<ModeGeometry, ExprError> {
    // Region = (base ∪ or) ∩ (∩ ands) ∖ (∪ nots). When there is no base/or,
    // the `and` primitives alone form the region (the directional РНМ element
    // uses only `and` primitives). Degenerate primitives drop out, which is how
    // a `тип`-selected primitive with zero size is ignored.
    let mut base_or: IShapes = Vec::new();
    let mut anded: Option<IShapes> = None;
    let mut nots: Vec<IShapes> = Vec::new();
    let mut polylines: Vec<Vec<Pt>> = Vec::new();

    for p in ch.primitives.iter().filter(|p| p.mode() == mode) {
        // Resolve the primitive into either a region shape-set or a polyline.
        let shapes: IShapes = match p {
            Primitive::PolyPoint { closed, points, .. } => {
                let pts = eval_points(points, ctx)?;
                if !*closed {
                    if pts.len() >= 2 {
                        polylines.push(pts);
                    }
                    continue;
                }
                simplify_ring(pts)
            }
            Primitive::Polygon { lines, .. } => {
                let evaled = eval_lines(lines, ctx)?;
                match polygon_from_lines(&evaled) {
                    Some(ring) => simplify_ring(ring),
                    None => continue,
                }
            }
            Primitive::Circle {
                radius, xcenter, ycenter, ..
            } => {
                let r = radius.eval(ctx)?;
                if r.abs() < 1e-9 {
                    continue;
                }
                let c = [xcenter.eval(ctx)?, ycenter.eval(ctx)?];
                simplify_ring(ellipse_points(c, r.abs(), r.abs(), 0.0))
            }
            Primitive::Ellipse {
                xcenter, ycenter, width, height, angle, ..
            } => {
                let w = width.eval(ctx)?;
                let h = height.eval(ctx)?;
                if w.abs() < 1e-9 || h.abs() < 1e-9 {
                    continue;
                }
                let c = [xcenter.eval(ctx)?, ycenter.eval(ctx)?];
                simplify_ring(ellipse_points(c, w.abs() / 2.0, h.abs() / 2.0, angle.eval(ctx)?))
            }
        };
        if shapes.is_empty() {
            continue;
        }
        match p.combine() {
            Combine::Base | Combine::Or => base_or = union(&base_or, &shapes),
            Combine::And => {
                anded = Some(match anded {
                    Some(a) => intersect(&a, &shapes),
                    None => shapes,
                });
            }
            Combine::Not => nots.push(shapes),
        }
    }

    let mut region = match (base_or.is_empty(), anded) {
        (false, Some(a)) => intersect(&base_or, &a),
        (false, None) => base_or,
        (true, Some(a)) => a,
        (true, None) => Vec::new(),
    };
    for n in &nots {
        region = difference(&region, n);
    }

    let mut shapes = Vec::new();
    for shape in region {
        if !shape.is_empty() && shape[0].len() >= 3 {
            shapes.push(Shape::Region(shape));
        }
    }
    for pl in polylines {
        shapes.push(Shape::Polyline(pl));
    }
    Ok(ModeGeometry { shapes })
}

/// Evaluated bounding line.
#[derive(Debug, Clone, Copy)]
pub struct EvalLine {
    pub k: f64,
    pub b: f64,
    pub kind: LineKind,
}

fn eval_lines(lines: &[PolyLine], ctx: &Context) -> Result<Vec<EvalLine>, ExprError> {
    lines
        .iter()
        .map(|l| {
            Ok(EvalLine {
                k: l.k.eval(ctx)?,
                b: l.b.eval(ctx)?,
                kind: l.kind,
            })
        })
        .collect()
}

fn eval_points(points: &[(Expr, Expr)], ctx: &Context) -> Result<Vec<Pt>, ExprError> {
    points
        .iter()
        .map(|(x, y)| Ok([x.eval(ctx)?, y.eval(ctx)?]))
        .collect()
}

/// General form `p*x + q*y = r` of an evaluated line.
fn general_form(l: &EvalLine) -> (f64, f64, f64) {
    match l.kind {
        // y = k*x + b  ->  -k*x + y = b
        LineKind::Slope => (-l.k, 1.0, l.b),
        // x = k*y + b  ->  x - k*y = b
        LineKind::InverseSlope => (1.0, -l.k, l.b),
    }
}

/// Intersection point of two evaluated lines, or `None` if (near) parallel.
pub fn line_intersection(a: &EvalLine, b: &EvalLine) -> Option<Pt> {
    let (pa, qa, ra) = general_form(a);
    let (pb, qb, rb) = general_form(b);
    let det = pa * qb - pb * qa;
    if det.abs() < 1e-12 {
        return None;
    }
    Some([
        (ra * qb - rb * qa) / det,
        (pa * rb - pb * ra) / det,
    ])
}

/// Build the ordered vertex loop of a `<polygon>` from its bounding lines by
/// intersecting successive lines in document order (wrapping last -> first),
/// exactly as the original feeds vertices to GDI `Polygon`. Self-intersections
/// are resolved later by the even-odd fill in [`simplify_ring`].
pub fn polygon_from_lines(lines: &[EvalLine]) -> Option<Vec<Pt>> {
    let n = lines.len();
    if n < 3 {
        return None;
    }
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        if let Some(p) = line_intersection(&lines[i], &lines[(i + 1) % n]) {
            if p[0].abs() < BIG && p[1].abs() < BIG {
                pts.push(p);
            }
        }
    }
    if pts.len() >= 3 {
        Some(pts)
    } else {
        None
    }
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
                Shape::Polyline(pl) => pl.iter().for_each(&mut acc),
            }
        }
    }
    b
}
