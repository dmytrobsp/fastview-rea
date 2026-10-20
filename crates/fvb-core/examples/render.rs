//! Headless PNG renderer for a block's characteristics — used to visually
//! validate the geometry engine against known relay-protection shapes.
//!
//! Usage: `cargo run -p fvb-core --example render -- <block_index> <out.png>`

use fvb_core::geom::{evaluate_in_view, Shape};
use fvb_core::*;
use tiny_skia::*;

const W: u32 = 900;
const H: u32 = 900;
const PAL: &[(u8, u8, u8)] = &[
    (31, 119, 180),
    (214, 39, 40),
    (44, 160, 44),
    (148, 103, 189),
    (255, 127, 14),
    (23, 190, 207),
    (227, 119, 194),
    (188, 189, 34),
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let block_idx: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let out = args.get(2).cloned().unwrap_or_else(|| "/tmp/fv.png".into());

    let lib = load_fvb_bytes(include_bytes!("../../../assets/FastView.fvb")).unwrap();
    let block = &lib.blocks[block_idx];
    eprintln!("block {block_idx}: {}", block.name);

    // Evaluate all characteristics with default settings, collect bounds.
    let mut items: Vec<(usize, CharGeometry)> = Vec::new();
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for (ci, ch) in block.characteristics.iter().enumerate() {
        let cx = ch.context(&ch.default_settings()).unwrap();
        let geo = evaluate(ch, &cx).unwrap();
        if let Some((a, b, c, d)) = geo.bounds {
            bounds = Some(match bounds {
                None => (a, b, c, d),
                Some((e, f, g, h)) => (e.min(a), f.min(b), g.max(c), h.max(d)),
            });
        }
        eprintln!(
            "  [{ci}] {} — operate {} shapes, reset {} shapes",
            ch.name,
            geo.operate.shapes.len(),
            geo.reset.shapes.len()
        );
        items.push((ci, geo));
    }

    let (mut minx, mut miny, mut maxx, mut maxy) = bounds.unwrap_or((-10.0, -10.0, 10.0, 10.0));
    // pad and make square around the data
    let padx = (maxx - minx) * 0.1 + 1e-3;
    let pady = (maxy - miny) * 0.1 + 1e-3;
    minx -= padx;
    maxx += padx;
    miny -= pady;
    maxy += pady;
    let span = (maxx - minx).max(maxy - miny);
    let cx0 = (minx + maxx) / 2.0;
    let cy0 = (miny + maxy) / 2.0;
    minx = cx0 - span / 2.0;
    maxx = cx0 + span / 2.0;
    miny = cy0 - span / 2.0;
    maxy = cy0 + span / 2.0;

    // Paint pass: like the original, open polypoints are closed against the
    // view that the (view-independent) autoscale pass chose.
    for (ci, geo) in items.iter_mut() {
        let ch = &block.characteristics[*ci];
        let cx = ch.context(&ch.default_settings()).unwrap();
        *geo = evaluate_in_view(ch, &cx, Some((minx, miny, maxx, maxy))).unwrap();
    }

    let margin = 40.0_f32;
    let plot = W as f32 - 2.0 * margin;
    let to_px = |p: &[f64; 2]| -> (f32, f32) {
        let x = margin + ((p[0] - minx) / (maxx - minx)) as f32 * plot;
        let y = margin + (1.0 - ((p[1] - miny) / (maxy - miny)) as f32) * plot;
        (x, y)
    };

    let mut pm = Pixmap::new(W, H).unwrap();
    pm.fill(Color::WHITE);

    // axes through origin
    let mut axis = Paint::default();
    axis.set_color_rgba8(140, 140, 140, 255);
    let (ox, _) = to_px(&[0.0, miny]);
    let (_, oy) = to_px(&[minx, 0.0]);
    if let Some(p) = line_path(ox, margin, ox, margin + plot) {
        pm.stroke_path(&p, &axis, &Stroke { width: 1.0, ..Default::default() }, Transform::identity(), None);
    }
    if let Some(p) = line_path(margin, oy, margin + plot, oy) {
        pm.stroke_path(&p, &axis, &Stroke { width: 1.0, ..Default::default() }, Transform::identity(), None);
    }

    for (ci, geo) in &items {
        let (r, g, b) = PAL[ci % PAL.len()];
        // operate: fill translucent + stroke solid
        draw_shapes(&mut pm, &geo.operate.shapes, (r, g, b), true, &to_px);
        // reset: stroke only, lighter
        draw_shapes(&mut pm, &geo.reset.shapes, (r, g, b), false, &to_px);
    }

    pm.save_png(&out).unwrap();
    eprintln!(
        "wrote {out}  (view range R∈[{minx:.1},{maxx:.1}] X∈[{miny:.1},{maxy:.1}])"
    );
}

fn line_path(x0: f32, y0: f32, x1: f32, y1: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y1);
    pb.finish()
}

fn draw_shapes(
    pm: &mut Pixmap,
    shapes: &[Shape],
    (r, g, b): (u8, u8, u8),
    fill: bool,
    to_px: &dyn Fn(&[f64; 2]) -> (f32, f32),
) {
    for Shape::Region(rings) in shapes {
        for (idx, ring) in rings.iter().enumerate() {
            if ring.len() < 2 {
                continue;
            }
            let mut pb = PathBuilder::new();
            let (x, y) = to_px(&ring[0]);
            pb.move_to(x, y);
            for p in &ring[1..] {
                let (x, y) = to_px(p);
                pb.line_to(x, y);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                if idx == 0 && fill {
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(r, g, b, 40);
                    paint.anti_alias = true;
                    pm.fill_path(&path, &paint, FillRule::EvenOdd, Transform::identity(), None);
                }
                let mut sp = Paint::default();
                let a = if fill { 255 } else { 150 };
                sp.set_color_rgba8(r, g, b, a);
                sp.anti_alias = true;
                pm.stroke_path(
                    &path,
                    &sp,
                    &Stroke { width: if fill { 2.0 } else { 1.2 }, ..Default::default() },
                    Transform::identity(),
                    None,
                );
            }
        }
    }
}
