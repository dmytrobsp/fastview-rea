//! Headless validation of the oscillogram + impedance-overlay chain: load the
//! demo COMTRADE fault record, compute the phase-A impedance locus, and plot it
//! over the distance-protection characteristics (block 0).
//!
//! Usage: `cargo run -p fvb-core --example render_overlay -- <out.png>`

use fvb_core::analysis::Analysis;
use fvb_core::geom::Shape;
use fvb_core::*;
use tiny_skia::*;

const W: u32 = 900;
const H: u32 = 900;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/overlay.png".into());
    let lib = load_fvb_bytes(include_bytes!("../../../assets/FastView.fvb")).unwrap();
    let block = &lib.blocks[0];

    let osc = comtrade::parse(
        include_str!("../../../assets/demo/demo_fault.cfg"),
        include_bytes!("../../../assets/demo/demo_fault.dat"),
    )
    .expect("parse demo comtrade");
    let an = Analysis::compute(&osc);
    eprintln!(
        "demo: {} analog ch, {} samples, Ua idx {:?}, Ia idx {:?}",
        osc.analog.len(),
        osc.sample_count(),
        an.u_abc[0],
        an.i_abc[0]
    );
    let locus = an.phase_a_locus(0, osc.sample_count() - 1);
    eprintln!("phase-A locus points: {}", locus.len());

    // Characteristic geometry (ДЗ 1/2/3) with defaults.
    let mut chars: Vec<(usize, CharGeometry)> = Vec::new();
    let mut bounds = (-2.0f64, -2.0f64, 12.0f64, 12.0f64);
    let mut acc = |x: f64, y: f64| {
        bounds.0 = bounds.0.min(x);
        bounds.1 = bounds.1.min(y);
        bounds.2 = bounds.2.max(x);
        bounds.3 = bounds.3.max(y);
    };
    for (ci, ch) in block.characteristics.iter().enumerate().take(3) {
        let cx = ch.context(&ch.default_settings()).unwrap();
        let geo = evaluate(ch, &cx).unwrap();
        if let Some((a, b, c, d)) = geo.bounds {
            acc(a, b);
            acc(c, d);
        }
        chars.push((ci, geo));
    }
    for p in &locus {
        acc(p[0], p[1]);
    }

    let (mut minx, mut miny, mut maxx, mut maxy) = bounds;
    let span = (maxx - minx).max(maxy - miny) * 1.1;
    let cx0 = (minx + maxx) / 2.0;
    let cy0 = (miny + maxy) / 2.0;
    minx = cx0 - span / 2.0;
    maxx = cx0 + span / 2.0;
    miny = cy0 - span / 2.0;
    maxy = cy0 + span / 2.0;

    let margin = 40.0f32;
    let plot = W as f32 - 2.0 * margin;
    let to_px = |p: &[f64; 2]| -> (f32, f32) {
        let x = margin + ((p[0] - minx) / (maxx - minx)) as f32 * plot;
        let y = margin + (1.0 - ((p[1] - miny) / (maxy - miny)) as f32) * plot;
        (x, y)
    };

    let mut pm = Pixmap::new(W, H).unwrap();
    pm.fill(Color::WHITE);
    // axes
    let mut axis = Paint::default();
    axis.set_color_rgba8(150, 150, 150, 255);
    let (ox, _) = to_px(&[0.0, 0.0]);
    let (_, oy) = to_px(&[0.0, 0.0]);
    stroke_line(&mut pm, ox, margin, ox, margin + plot, &axis, 1.0);
    stroke_line(&mut pm, margin, oy, margin + plot, oy, &axis, 1.0);

    const PAL: &[(u8, u8, u8)] = &[(31, 119, 180), (214, 39, 40), (44, 160, 44)];
    for (ci, geo) in &chars {
        let (r, g, b) = PAL[ci % PAL.len()];
        draw_region(&mut pm, &geo.operate.shapes, (r, g, b), true, &to_px);
        draw_region(&mut pm, &geo.reset.shapes, (r, g, b), false, &to_px);
    }

    // impedance locus (gold)
    if locus.len() >= 2 {
        let mut pb = PathBuilder::new();
        let (x, y) = to_px(&locus[0]);
        pb.move_to(x, y);
        for p in &locus[1..] {
            let (x, y) = to_px(p);
            pb.line_to(x, y);
        }
        if let Some(path) = pb.finish() {
            let mut gp = Paint::default();
            gp.set_color_rgba8(255, 180, 0, 255);
            gp.anti_alias = true;
            pm.stroke_path(&path, &gp, &Stroke { width: 2.5, ..Default::default() }, Transform::identity(), None);
        }
        // mark the fault-time point (sample ~ 60% in, mid-fault)
        let kf = osc.sample_count() * 5 / 10;
        if let Some(z) = an.phase_impedance(0, kf) {
            let (x, y) = to_px(&[z.re, z.im]);
            let mut rp = Paint::default();
            rp.set_color_rgba8(200, 0, 0, 255);
            rp.anti_alias = true;
            if let Some(c) = PathBuilder::from_circle(x, y, 5.0) {
                pm.fill_path(&c, &rp, FillRule::Winding, Transform::identity(), None);
            }
        }
    }

    pm.save_png(&out).unwrap();
    eprintln!("wrote {out}");
}

fn stroke_line(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, paint: &Paint, w: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y1);
    if let Some(p) = pb.finish() {
        pm.stroke_path(&p, paint, &Stroke { width: w, ..Default::default() }, Transform::identity(), None);
    }
}

fn draw_region(pm: &mut Pixmap, shapes: &[Shape], (r, g, b): (u8, u8, u8), fill: bool, to_px: &dyn Fn(&[f64; 2]) -> (f32, f32)) {
    for Shape::Region(rings) in shapes {
        for (idx, ring) in rings.iter().enumerate() {
            if ring.len() < 3 {
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
                sp.set_color_rgba8(r, g, b, if fill { 255 } else { 130 });
                sp.anti_alias = true;
                pm.stroke_path(&path, &sp, &Stroke { width: if fill { 2.0 } else { 1.0 }, ..Default::default() }, Transform::identity(), None);
            }
        }
    }
}
