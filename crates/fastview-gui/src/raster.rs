//! Headless rendering of egui painting to pixels, for printing (ledger
//! L9.22): a fresh `egui::Context` runs the paint closure at a chosen
//! pixels-per-point, the output is tessellated by egui itself and the
//! triangle meshes are rasterised on the CPU (premultiplied colours blended
//! in gamma space, as egui's GPU back ends do; bilinear texture sampling).

use eframe::egui::{self, epaint, Color32, Pos2, Rect, Vec2};
use std::collections::HashMap;

/// An RGB pixel buffer (row-major, top to bottom).
#[derive(Clone)]
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

impl Canvas {
    /// Shrunk to `w` × `h` by area-weighted averaging: every line keeps
    /// its share of the pixels it covers.
    pub fn shrink(&self, w: usize, h: usize) -> Canvas {
        let c = self;
        let mut out = Canvas::white(w, h);
        let (sx, sy) = (c.w as f32 / w as f32, c.h as f32 / h as f32);
        for y in 0..h {
            let (y0, y1) = (y as f32 * sy, (y + 1) as f32 * sy);
            for x in 0..w {
                let (x0, x1) = (x as f32 * sx, (x + 1) as f32 * sx);
                let mut acc = [0f32; 3];
                let mut area = 0f32;
                for yy in (y0.floor() as usize)..(y1.ceil() as usize).min(c.h) {
                    let wy = (y1.min(yy as f32 + 1.0) - y0.max(yy as f32)).max(0.0);
                    for xx in (x0.floor() as usize)..(x1.ceil() as usize).min(c.w) {
                        let wx = (x1.min(xx as f32 + 1.0) - x0.max(xx as f32)).max(0.0);
                        let px = c.px[yy * c.w + xx];
                        for k in 0..3 {
                            acc[k] += px[k] as f32 * wx * wy;
                        }
                        area += wx * wy;
                    }
                }
                out.px[y * w + x] = acc.map(|v| (v / area.max(1e-6)).round() as u8);
            }
        }
        out
    }

    /// Shrunk to `w` × `h` keeping ink: each pixel the darkest of the
    /// pixels it covers (a 1-pixel pen's line keeps its colour at any
    /// scale, as GDI draws it at the scale shown).
    pub fn shrink_ink(&self, w: usize, h: usize) -> Canvas {
        let mut out = Canvas::white(w, h);
        let (sx, sy) = (self.w as f32 / w as f32, self.h as f32 / h as f32);
        for y in 0..h {
            // Blocks that partition the source: a 1-pixel line stays one.
            let (y0, y1) = ((y as f32 * sy) as usize, (((y + 1) as f32 * sy) as usize).min(self.h));
            for x in 0..w {
                let (x0, x1) = ((x as f32 * sx) as usize, (((x + 1) as f32 * sx) as usize).min(self.w));
                let mut best = [255u8; 3];
                let mut ink = u32::MAX;
                for yy in y0..y1.max(y0 + 1).min(self.h) {
                    for &px in &self.px[yy * self.w + x0..yy * self.w + x1.max(x0 + 1).min(self.w)] {
                        let v = px.iter().map(|&c| c as u32).sum::<u32>();
                        if v < ink {
                            (ink, best) = (v, px);
                        }
                    }
                }
                out.px[y * w + x] = best;
            }
        }
        out
    }

    /// Enlarged to `w` × `h`, each pixel repeated (a stretch without
    /// interpolation).
    pub fn enlarge_nearest(&self, w: usize, h: usize) -> Canvas {
        let mut out = Canvas::white(w, h);
        for y in 0..h {
            let sy = (y * self.h / h.max(1)).min(self.h - 1);
            for x in 0..w {
                out.px[y * w + x] = self.px[sy * self.w + (x * self.w / w.max(1)).min(self.w - 1)];
            }
        }
        out
    }

    /// The `w` × `h` region at (x, y), clipped.
    pub fn region(&self, x: usize, y: usize, w: usize, h: usize) -> Canvas {
        let (w, h) = (w.min(self.w.saturating_sub(x)), h.min(self.h.saturating_sub(y)));
        let mut out = Canvas::white(w.max(1), h.max(1));
        for yy in 0..h {
            out.px[yy * out.w..yy * out.w + w].copy_from_slice(&self.px[(y + yy) * self.w + x..(y + yy) * self.w + x + w]);
        }
        out
    }

    pub fn white(w: usize, h: usize) -> Canvas {
        Canvas { w, h, px: vec![[255; 3]; w * h] }
    }

    /// The page placed on the device's whole sheet at its printable area.
    pub fn on_sheet(self, dev: &crate::printout::Device) -> Canvas {
        let (sw, sh) = ((dev.sheet_mm.0 / 25.4 * dev.dpi).round() as usize, (dev.sheet_mm.1 / 25.4 * dev.dpi).round() as usize);
        if dev.offset == (0, 0) && (sw, sh) == (self.w, self.h) {
            return self;
        }
        let mut out = Canvas::white(sw.max(1), sh.max(1));
        for y in 0..self.h.min(sh.saturating_sub(dev.offset.1)) {
            let w = self.w.min(sw.saturating_sub(dev.offset.0));
            let at = (y + dev.offset.1) * out.w + dev.offset.0;
            out.px[at..at + w].copy_from_slice(&self.px[y * self.w..y * self.w + w]);
        }
        out
    }

    #[cfg(test)]
    pub fn get(&self, x: usize, y: usize) -> [u8; 3] {
        self.px[y * self.w + x]
    }

    /// Copy `src` with its top-left corner at (x, y), clipped.
    pub fn blit(&mut self, src: &Canvas, x: i64, y: i64) {
        for sy in 0..src.h {
            let ty = y + sy as i64;
            if ty < 0 || ty >= self.h as i64 {
                continue;
            }
            for sx in 0..src.w {
                let tx = x + sx as i64;
                if tx >= 0 && tx < self.w as i64 {
                    self.px[ty as usize * self.w + tx as usize] = src.px[sy * src.w + sx];
                }
            }
        }
    }
}

/// A texture as premultiplied sRGBA.
struct Tex {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

impl Tex {
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = (u * self.w as f32 - 0.5).clamp(0.0, (self.w - 1) as f32);
        let y = (v * self.h as f32 - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (tx, ty) = (x - x0 as f32, y - y0 as f32);
        let at = |x: usize, y: usize| self.px[y * self.w + x].to_array().map(f32::from);
        let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
        let mut out = [0.0; 4];
        for i in 0..4 {
            out[i] = (a[i] * (1.0 - tx) + b[i] * tx) * (1.0 - ty) + (c[i] * (1.0 - tx) + d[i] * tx) * ty;
        }
        out
    }
}

fn apply(textures: &mut HashMap<egui::TextureId, Tex>, delta: &egui::TexturesDelta) {
    for (id, d) in &delta.set {
        let (w, h, px): (usize, usize, Vec<Color32>) = match &d.image {
            egui::ImageData::Color(img) => (img.size[0], img.size[1], img.pixels.clone()),
            egui::ImageData::Font(img) => (img.size[0], img.size[1], img.srgba_pixels(None).collect()),
        };
        match d.pos {
            None => {
                textures.insert(*id, Tex { w, h, px });
            }
            Some([ox, oy]) => {
                if let Some(t) = textures.get_mut(id) {
                    for y in 0..h {
                        for x in 0..w {
                            if ox + x < t.w && oy + y < t.h {
                                t.px[(oy + y) * t.w + ox + x] = px[y * w + x];
                            }
                        }
                    }
                }
            }
        }
    }
    for id in &delta.free {
        textures.remove(id);
    }
}

thread_local! {
    /// Shapes without feathering: lines and edges cover the pixels whose
    /// centres they contain, in full colour, as GDI draws them.
    static CRISP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `f` with [`render`] drawing crisp (unfeathered) shapes.
pub fn crisp<R>(f: impl FnOnce() -> R) -> R {
    let old = CRISP.with(|c| c.replace(true));
    let r = f();
    CRISP.with(|c| c.set(old));
    r
}

/// Run `paint` headless on a `size` (points) screen at `ppp` pixels per
/// point and rasterise the result onto a white canvas. The pass runs twice
/// so widgets that size themselves on their first frame draw on the second.
pub fn render(size: Vec2, ppp: f32, fonts: Option<&egui::FontDefinitions>, paint: impl FnMut(&egui::Context)) -> Canvas {
    render_input(size, ppp, fonts, vec![vec![]; 3], paint)
}

/// [`render`] with one pass per batch of input events (the last pass is
/// the one rasterised).
pub fn render_input(size: Vec2, ppp: f32, fonts: Option<&egui::FontDefinitions>, passes: Vec<Vec<egui::Event>>, mut paint: impl FnMut(&egui::Context)) -> Canvas {
    let ctx = egui::Context::default();
    if let Some(f) = fonts {
        ctx.set_fonts(f.clone());
    }
    ctx.set_pixels_per_point(ppp);
    if CRISP.with(std::cell::Cell::get) {
        ctx.tessellation_options_mut(|o| o.feathering = false);
    }
    let mut textures = HashMap::new();
    let mut out = None;
    for (pass, events) in passes.into_iter().enumerate() {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            max_texture_side: Some(16384),
            time: Some(pass as f64 * 0.1),
            events,
            ..Default::default()
        };
        let full = ctx.run(raw, &mut paint);
        apply(&mut textures, &full.textures_delta);
        out = Some((full.shapes, full.pixels_per_point));
    }
    let (shapes, ppp) = out.expect("a pass ran");
    let (w, h) = ((size.x * ppp).round() as usize, (size.y * ppp).round() as usize);
    rasterise(&ctx, shapes, ppp, &textures, w, h)
}

/// [`render_input`] as a runner on a monitor at `native` pixels per point
/// shows it: the window `px` device pixels, so each pass's screen is
/// `px` / (zoom · native) points (egui-winit), the zoom what `setup` and
/// then the app left it at, through the app's raw input `hook`; the last
/// pass rasterised at its own pixels per
/// point onto `px`.
#[cfg(test)]
pub fn render_native(px: Vec2, native: f32, setup: impl FnOnce(&egui::Context), hook: impl Fn(&egui::Context, &mut egui::RawInput), fonts: Option<&egui::FontDefinitions>, passes: Vec<Vec<egui::Event>>, paint: impl FnMut(&egui::Context)) -> Canvas {
    render_natives(setup, hook, fonts, passes.into_iter().map(|events| (native, px, events)).collect(), paint)
}

/// [`render_native`] with the monitor's `native` scale and the window's
/// size in device pixels given per pass: the window dragged to another
/// monitor, or the scale changed live, between two passes (the screen of
/// a pass follows the zoom the app left); the last pass's size is the
/// canvas's.
#[cfg(test)]
pub fn render_natives(setup: impl FnOnce(&egui::Context), hook: impl Fn(&egui::Context, &mut egui::RawInput), fonts: Option<&egui::FontDefinitions>, passes: Vec<(f32, Vec2, Vec<egui::Event>)>, mut paint: impl FnMut(&egui::Context)) -> Canvas {
    let ctx = egui::Context::default();
    if let Some(f) = fonts {
        ctx.set_fonts(f.clone());
    }
    setup(&ctx);
    let mut textures = HashMap::new();
    let mut out = None;
    let mut size = Vec2::ZERO;
    for (pass, (native, px, events)) in passes.into_iter().enumerate() {
        size = px;
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, px / (ctx.zoom_factor() * native))),
            max_texture_side: Some(16384),
            time: Some(pass as f64 * 0.1),
            events,
            ..Default::default()
        };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(native);
        hook(&ctx, &mut raw);
        let full = ctx.run(raw, &mut paint);
        apply(&mut textures, &full.textures_delta);
        out = Some((full.shapes, full.pixels_per_point));
    }
    let (shapes, ppp) = out.expect("a pass ran");
    rasterise(&ctx, shapes, ppp, &textures, size.x.round() as usize, size.y.round() as usize)
}

/// egui's shapes tessellated at `ppp` and drawn onto a white `w` × `h`.
fn rasterise(ctx: &egui::Context, shapes: Vec<epaint::ClippedShape>, ppp: f32, textures: &HashMap<egui::TextureId, Tex>, w: usize, h: usize) -> Canvas {
    let prims = ctx.tessellate(shapes, ppp);
    let mut canvas = Canvas::white(w.max(1), h.max(1));
    let mut acc: Vec<[f32; 3]> = vec![[255.0; 3]; canvas.w * canvas.h];
    for p in &prims {
        if let epaint::Primitive::Mesh(m) = &p.primitive {
            draw_mesh(&mut acc, canvas.w, canvas.h, m, p.clip_rect, ppp, textures.get(&m.texture_id));
        }
    }
    for (o, a) in canvas.px.iter_mut().zip(&acc) {
        *o = a.map(|v| v.round().clamp(0.0, 255.0) as u8);
    }
    canvas
}

/// egui's output rasterised frame after frame (the software runner,
/// `crate::softrun`): the textures kept between frames.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Default)]
pub struct Painter {
    textures: HashMap<egui::TextureId, Tex>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Painter {
    /// Paint `prims` on a `w` × `h` pixel white ground: the new textures
    /// set first, the freed ones dropped after (egui's order).
    pub fn paint(&mut self, prims: &[epaint::ClippedPrimitive], delta: &egui::TexturesDelta, w: usize, h: usize, ppp: f32) -> Canvas {
        apply(&mut self.textures, &egui::TexturesDelta { set: delta.set.clone(), free: Vec::new() });
        let mut canvas = Canvas::white(w.max(1), h.max(1));
        let mut acc: Vec<[f32; 3]> = vec![[255.0; 3]; canvas.w * canvas.h];
        for p in prims {
            if let epaint::Primitive::Mesh(m) = &p.primitive {
                draw_mesh(&mut acc, canvas.w, canvas.h, m, p.clip_rect, ppp, self.textures.get(&m.texture_id));
            }
        }
        for (o, a) in canvas.px.iter_mut().zip(&acc) {
            *o = a.map(|v| v.round().clamp(0.0, 255.0) as u8);
        }
        for id in &delta.free {
            self.textures.remove(id);
        }
        canvas
    }
}

fn draw_mesh(acc: &mut [[f32; 3]], w: usize, h: usize, m: &epaint::Mesh, clip: Rect, ppp: f32, tex: Option<&Tex>) {
    let cx0 = (clip.min.x * ppp).max(0.0).floor() as i64;
    let cy0 = (clip.min.y * ppp).max(0.0).floor() as i64;
    let cx1 = ((clip.max.x * ppp).ceil() as i64).min(w as i64);
    let cy1 = ((clip.max.y * ppp).ceil() as i64).min(h as i64);
    for t in m.indices.as_chunks::<3>().0 {
        let v = [m.vertices[t[0] as usize], m.vertices[t[1] as usize], m.vertices[t[2] as usize]];
        let p = v.map(|v| v.pos.to_vec2() * ppp);
        let area = (p[1] - p[0]).x * (p[2] - p[0]).y - (p[1] - p[0]).y * (p[2] - p[0]).x;
        if area.abs() < 1e-9 {
            continue;
        }
        let minx = (p.iter().map(|q| q.x).fold(f32::MAX, f32::min).floor() as i64).max(cx0);
        let maxx = (p.iter().map(|q| q.x).fold(f32::MIN, f32::max).ceil() as i64).min(cx1);
        let miny = (p.iter().map(|q| q.y).fold(f32::MAX, f32::min).floor() as i64).max(cy0);
        let maxy = (p.iter().map(|q| q.y).fold(f32::MIN, f32::max).ceil() as i64).min(cy1);
        let col = v.map(|v| v.color.to_array().map(f32::from));
        for y in miny..maxy {
            for x in minx..maxx {
                // The pixel centre, nudged a fraction of a pixel: a fill
                // rule — a centre on an edge two triangles share falls in
                // exactly one (the GPU's top-left rule does the same; an
                // inclusive test drew it twice, doubling translucent ink).
                let q = Vec2::new(x as f32 + 0.5 + 1.0 / 512.0, y as f32 + 0.5 + 1.0 / 256.0);
                let e = |a: Vec2, b: Vec2| (b - a).x * (q - a).y - (b - a).y * (q - a).x;
                let (w0, w1, w2) = (e(p[1], p[2]) / area, e(p[2], p[0]) / area, e(p[0], p[1]) / area);
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let mut c = [0f32; 4];
                for i in 0..4 {
                    c[i] = (col[0][i] * w0 + col[1][i] * w1 + col[2][i] * w2) / 255.0;
                }
                if let Some(tex) = tex {
                    let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                    let vv = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                    let s = tex.sample(u, vv);
                    for i in 0..4 {
                        c[i] *= s[i] / 255.0;
                    }
                }
                let dst = &mut acc[y as usize * w + x as usize];
                for i in 0..3 {
                    dst[i] = c[i] * 255.0 + dst[i] * (1.0 - c[3]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shrunk_page_keeps_its_thin_lines() {
        // A 1-pixel red line every 5 columns, shrunk 4×: averaging pales
        // every one, keeping ink leaves each in its full colour.
        let mut c = Canvas::white(40, 8);
        for y in 0..8 {
            for x in (0..40).step_by(5) {
                c.px[y * 40 + x] = [255, 0, 0];
            }
        }
        let ink = c.shrink_ink(10, 2);
        let avg = c.shrink(10, 2);
        let reds = |c: &Canvas| (0..10).filter(|&x| c.get(x, 0) == [255, 0, 0]).count();
        assert_eq!(reds(&ink), 8);
        assert_eq!(reds(&avg), 0);
        assert!((0..10).all(|x| ink.get(x, 0) == [255, 0, 0] || ink.get(x, 0) == [255, 255, 255]));
    }

    #[test]
    fn a_one_pixel_line_stays_one_pixel_at_a_fractional_shrink() {
        // Shrunk by 1.5: every source column falls in one block, so a
        // 1-pixel line is one pixel wide, never two.
        for x0 in 0..30 {
            let mut c = Canvas::white(30, 3);
            for y in 0..3 {
                c.px[y * 30 + x0] = [100, 100, 100];
            }
            let ink = c.shrink_ink(20, 2);
            assert_eq!((0..20).filter(|&x| ink.get(x, 0) == [100, 100, 100]).count(), 1, "line at {x0}");
        }
    }

    #[test]
    fn crisp_lines_cover_whole_pixels() {
        let c = crisp(|| {
            render(Vec2::new(20.0, 10.0), 1.0, None, |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                    ui.painter().line_segment([Pos2::new(0.0, 5.3), Pos2::new(20.0, 5.3)], egui::Stroke::new(1.0f32, egui::Color32::from_gray(100)));
                });
            })
        });
        let col: Vec<[u8; 3]> = (0..10).map(|y| c.get(10, y)).collect();
        assert_eq!(col.iter().filter(|p| **p == [100, 100, 100]).count(), 1, "{col:?}");
        assert!(col.iter().all(|p| *p == [100, 100, 100] || *p == [255, 255, 255]), "{col:?}");
    }

    #[test]
    fn renders_shapes_and_text_at_the_requested_density() {
        let c = render(Vec2::new(100.0, 50.0), 2.0, None, |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            p.rect_filled(Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(40.0, 30.0)), 0.0, Color32::from_rgb(200, 0, 0));
            p.text(Pos2::new(50.0, 10.0), egui::Align2::LEFT_TOP, "Аб", egui::FontId::proportional(14.0), Color32::BLACK);
        });
        assert_eq!((c.w, c.h), (200, 100));
        assert_eq!(c.get(50, 40), [200, 0, 0], "inside the rectangle");
        assert_eq!(c.get(5, 5), [255, 255, 255], "background stays white");
        let dark = (100..200).flat_map(|x| (20..60).map(move |y| (x, y))).filter(|&(x, y)| c.get(x, y)[0] < 128).count();
        assert!(dark > 30, "the text is drawn ({dark} dark pixels)");
    }
}
