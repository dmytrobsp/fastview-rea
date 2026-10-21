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
    pub fn white(w: usize, h: usize) -> Canvas {
        Canvas { w, h, px: vec![[255; 3]; w * h] }
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
    let prims = ctx.tessellate(shapes, ppp);
    let (w, h) = ((size.x * ppp).round() as usize, (size.y * ppp).round() as usize);
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
                let q = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
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
