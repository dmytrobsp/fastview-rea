//! «Цвет канала» (33030): MFC's colour dialog (CMFCColorDialog, the
//! «Цвета» template of mfc120rus — tabs «Обычные» / «Спектр»), as the VM
//! shows it. The colours of the hexagon are the dialog's own (sampled);
//! the drawing is ours.

use crate::app::App;
use crate::dialogs::{dlu, win_button, win_button_en, win_edit, DLG_FONT};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

/// The 127 cells of the colour hexagon, row by row from the top (7, 8,
/// … 13 … 7 cells), as the VM shows them (sampled).
const HEX_COLORS: [u32; 127] = [
    0x000080, 0x120080, 0x270080, 0x3D0080, 0x560080, 0x6B0080, 0x800080, 0x001280, 0x0000C0, 0x2400C0,
    0x4E00C0, 0x8000C0, 0xA300C0, 0xC000C0, 0x800067, 0x002780, 0x0024C0, 0x0000FF, 0x4600FF, 0x8000FF,
    0xC000FF, 0xFF00FF, 0xC0009D, 0x800052, 0x003D80, 0x004BC0, 0x0046FF, 0x5757FF, 0x8057FF, 0xC057FF,
    0xFF57FF, 0xFF00C0, 0xC00080, 0x80003B, 0x005480, 0x0080C0, 0x0080FF, 0x5780FF, 0xA0A0FF, 0xC0A0FF,
    0xFFA0FF, 0xFF57C0, 0xFF0080, 0xC0004B, 0x800025, 0x006980, 0x00A0C0, 0x00C0FF, 0x57C0FF, 0xA0C0FF,
    0xE9E9FF, 0xFFE9FF, 0xFFA0C0, 0xFF5780, 0xFF0042, 0xC00020, 0x800010, 0x008080, 0x00C0C0, 0x00FFFF,
    0x57FFFF, 0xA0FFFF, 0xE9FFFF, 0xFFFFFF, 0xFFE9E9, 0xFFA0A0, 0xFF5757, 0xFF0000, 0xC00000, 0x800000,
    0x008069, 0x00C0A0, 0x00FFC0, 0x57FFC0, 0xA0FFC0, 0xE9FFE9, 0xFFFFE9, 0xFFC0A0, 0xFF8057, 0xFF4200,
    0xC02000, 0x801000, 0x008054, 0x00C080, 0x00FF80, 0x57FF80, 0xA0FFA0, 0xC0FFA0, 0xFFFFA0, 0xFFC057,
    0xFF8000, 0xC04B00, 0x802500, 0x00803D, 0x00C04B, 0x00FF46, 0x57FF57, 0x80FF57, 0xC0FF57, 0xFFFF57,
    0xFFC000, 0xC08000, 0x803B00, 0x008027, 0x00C024, 0x00FF00, 0x46FF00, 0x80FF00, 0xC0FF00, 0xFFFF00,
    0xC09D00, 0x805200, 0x008012, 0x00C000, 0x24C000, 0x4EC000, 0x80C000, 0xA3C000, 0xC0C000, 0x806700,
    0x008000, 0x128000, 0x278000, 0x3D8000, 0x568000, 0x6B8000, 0x808000,
];

/// The grey section (VM, in the client): a large white and a large black
/// hexagon, and two rows of small greys between them.
const GREYS_TOP: [u8; 8] = [0xF0, 0xD2, 0xB4, 0x96, 0x78, 0x5A, 0x3C, 0x1E];
const GREYS_BOTTOM: [u8; 7] = [0xE1, 0xC0, 0xA5, 0x87, 0x69, 0x4B, 0x2D];

fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// The selected cell's ring (VM: the original's pixels, `#` black, `o`
/// white).
const SEL_RING: [&str; 16] = [
    "......oooo......",
    "....oo####oo....",
    "..oo###oo###oo..",
    "oo###o####o###oo",
    "###o###..###o###",
    "#o###......###o#",
    "#o#..........#o#",
    "#o#..........#o#",
    "#o#..........#o#",
    "#o#..........#o#",
    "#o#o........o#o#",
    "#o###o....o###o#",
    "###o###oo###o###",
    "..###o####o###..",
    "....###oo###....",
    "......####......",
];

/// A pointy-top hexagon over [l, r) × [t, b) with tips `tip` px deep.
fn hexagon(l: f32, t: f32, r: f32, b: f32, tip: f32) -> Vec<Pos2> {
    let cx = (l + r) / 2.0;
    vec![Pos2::new(cx, t), Pos2::new(r, t + tip), Pos2::new(r, b - tip), Pos2::new(cx, b), Pos2::new(l, b - tip), Pos2::new(l, t + tip)]
}

/// The dialog's state while open: the new colour and the tab.
#[derive(Clone, Copy)]
struct ColorState {
    new: Color32,
    spectrum: bool,
}

impl App {
    /// «Цвет канала» (33030): the colour dialog, started from the first
    /// selected channel's colour (the «Текущий» half of the swatch); OK
    /// gives the new colour to every selected channel.
    pub fn color_window(&mut self, ctx: &egui::Context) {
        let Some(current) = self.color_dialog else { return };
        let key = egui::Id::new("fv_color_state");
        let mut st: ColorState = ctx.data(|d| d.get_temp(key)).unwrap_or(ColorState { new: current, spectrum: false });
        let mut open = true;
        let mut done: Option<bool> = None;
        // The «Цвета» template: 252 × 180 DLU (378 × 293 px).
        crate::dialogs::dlg(ctx, "Цвета").margin(0.0).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            let (client, _) = ui.allocate_exact_size(Vec2::new(378.0, 293.0), egui::Sense::hover());
            let at = |x: f32, y: f32| client.min + Vec2::new(x, y);
            let p = ui.painter().clone();
            // The tab control (4, 4, 182 × 172 DLU) (VM): the page (249) in
            // a (229) frame from y 27; the selected tab (249) from y 7, open
            // into the page, the other (243) from y 9, both framed (229).
            let tabs = Rect::from_min_size(at(6.0, 6.0), Vec2::new(273.0, 280.0));
            let page = Rect::from_min_max(Pos2::new(tabs.left(), tabs.top() + 21.0), tabs.max);
            let line = Color32::from_gray(229);
            p.rect_filled(page, 0.0, line);
            p.rect_filled(page.shrink(1.0), 0.0, Color32::from_gray(249));
            let mut x = tabs.left();
            for (k, title) in ["Обычные", "Спектр"].into_iter().enumerate() {
                let sel = st.spectrum == (k == 1);
                let w = ui.fonts(|f| f.layout_no_wrap(title.to_owned(), DLG_FONT, Color32::BLACK).size().x).round() + 14.0;
                let top = tabs.top() + if sel { 1.0 } else { 3.0 };
                let r = Rect::from_min_max(Pos2::new(x, top), Pos2::new(x + w + 1.0, page.top() + 1.0));
                p.rect_filled(r, 0.0, line);
                let fill = Rect::from_min_max(r.min + Vec2::splat(1.0), Pos2::new(r.right() - 1.0, if sel { page.top() + 1.0 } else { page.top() }));
                p.rect_filled(fill, 0.0, Color32::from_gray(if sel { 249 } else { 243 }));
                p.text(Pos2::new(r.center().x, r.top() + 10.0), egui::Align2::CENTER_CENTER, title, DLG_FONT, Color32::BLACK);
                if ui.interact(r, ui.id().with(("fv_color_tab", k)), egui::Sense::click()).clicked() {
                    st.spectrum = k == 1;
                }
                x += w;
            }
            p.text(at(13.0, 36.0), egui::Align2::LEFT_TOP, "Цвета:", DLG_FONT, Color32::BLACK);
            if st.spectrum {
                spectrum_page(ui, &p, client, &mut st.new);
            } else {
                // The hexagon (VM): 13 × 15 px cells 14 px apart in rows
                // 12 px apart, centred at (143.5, 144.5); the cell of the new
                // colour outlined.
                let mut k = 0;
                let mut hot = None;
                for r in 0..13 {
                    let n = 7 + r.min(12 - r);
                    let cy = 144.5 + 12.0 * (r as f32 - 6.0);
                    for c in 0..n {
                        let cx = 143.5 - 7.0 * (n as f32 - 1.0) + 14.0 * c as f32;
                        // Tiling the plane as GDI's cells do (no seams): 14
                        // wide, 16 tall, 4 px tips on 12 px rows.
                        // (0.6 px over: egui feathers every edge, which would
                        // show the page through between the cells.)
                        let pts = hexagon(client.left() + cx - 7.6, client.top() + cy - 8.6, client.left() + cx + 7.6, client.top() + cy + 8.6, 4.3);
                        let col = rgb(HEX_COLORS[k]);
                        p.add(egui::Shape::convex_polygon(pts.clone(), col, Stroke::NONE));
                        if col == st.new {
                            hot = Some((Rect::from_center_size(Pos2::new(client.left() + cx, client.top() + cy), Vec2::new(14.0, 16.0)), 4.0));
                        }
                        let cell = Rect::from_center_size(Pos2::new(client.left() + cx, client.top() + cy), Vec2::new(13.0, 12.0));
                        if ui.interact(cell, ui.id().with(("fv_hex", k)), egui::Sense::click()).clicked() {
                            st.new = col;
                        }
                        k += 1;
                    }
                }
                // The greys: the large white and black hexagons, the two rows
                // of small ones between them (VM).
                let mut greys: Vec<(Rect, f32, Color32)> = vec![
                    (Rect::from_min_max(at(13.0, 244.0), at(54.0, 283.0)), 10.0, Color32::WHITE),
                    (Rect::from_min_max(at(232.0, 244.0), at(273.0, 283.0)), 10.0, Color32::BLACK),
                ];
                for (i, g) in GREYS_TOP.iter().enumerate() {
                    greys.push((Rect::from_min_size(at(63.0 + 20.0 * i as f32, 244.0), Vec2::new(20.0, 22.0)), 5.5, Color32::from_gray(*g)));
                }
                for (i, g) in GREYS_BOTTOM.iter().enumerate() {
                    greys.push((Rect::from_min_size(at(73.0 + 20.0 * i as f32, 261.0), Vec2::new(20.0, 22.0)), 5.5, Color32::from_gray(*g)));
                }
                for (i, (r, tip, col)) in greys.into_iter().enumerate() {
                    let pts = hexagon(r.left() - 0.6, r.top() - 0.6, r.right() + 0.6, r.bottom() + 0.6, tip);
                    p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
                    if col == st.new {
                        hot = Some((r, tip));
                    }
                    if ui.interact(r.shrink2(Vec2::new(0.0, tip / 2.0)), ui.id().with(("fv_grey", i)), egui::Sense::click()).clicked() {
                        st.new = col;
                    }
                }
                // The selected cell's ring as CMFCColorPickerCtrl draws it
                // (VM pixels, 16 × 16 about the cell's centre: black and
                // white steps along its outline).
                // The greys' larger cells keep a three-line outline.
                if let Some((r, tip)) = hot.filter(|(r, _)| r.width() > 15.0) {
                    for (i, ink) in [Color32::BLACK, Color32::WHITE, Color32::BLACK].into_iter().enumerate() {
                        let d = 0.5 + i as f32;
                        p.add(egui::Shape::closed_line(hexagon(r.left() + d, r.top() + d, r.right() - d, r.bottom() - d, (tip - i as f32 * 0.5).max(1.0)), Stroke::new(1.0f32, ink)));
                    }
                } else if let Some((r, _)) = hot {
                    let o = Pos2::new((r.center().x - 8.5).round(), (r.center().y - 8.5).round());
                    for (y, row) in SEL_RING.iter().enumerate() {
                        for (x, ch) in row.chars().enumerate() {
                            let ink = match ch {
                                '#' => Color32::BLACK,
                                'o' => Color32::WHITE,
                                _ => continue,
                            };
                            p.rect_filled(Rect::from_min_size(o + Vec2::new(x as f32, y as f32), Vec2::splat(1.0)), 0.0, ink);
                        }
                    }
                }
            }
            // The right column (template): ОК, Отмена, «Выбор...», «Новый»,
            // the swatch — the new colour above the current one, in a (105)
            // frame — and «Текущий».
            if win_button(ui, dlu(client, 194, 16, 53, 14), "ОК", true).clicked() {
                done = Some(true);
            }
            if win_button(ui, dlu(client, 194, 34, 53, 14), "Отмена", false).clicked() {
                done = Some(false);
            }
            // «Выбор...» picks a colour off the screen in the original; ours
            // cannot read the screen: greyed.
            win_button_en(ui, dlu(client, 194, 65, 53, 14), "Выбор...", false, false);
            let new_l = dlu(client, 194, 110, 53, 9);
            p.text(Pos2::new(new_l.center().x, new_l.top()), egui::Align2::CENTER_TOP, "Новый", DLG_FONT, Color32::BLACK);
            let sw = dlu(client, 194, 122, 53, 41);
            p.rect_filled(sw, 0.0, Color32::from_gray(105));
            let inner = sw.shrink(1.0);
            let mid = inner.top() + (inner.height() / 2.0).floor();
            p.rect_filled(Rect::from_min_max(inner.min, Pos2::new(inner.right(), mid)), 0.0, st.new);
            p.rect_filled(Rect::from_min_max(Pos2::new(inner.left(), mid), inner.max), 0.0, current);
            let cur_l = dlu(client, 194, 166, 53, 10);
            p.text(Pos2::new(cur_l.center().x, cur_l.top()), egui::Align2::CENTER_TOP, "Текущий", DLG_FONT, Color32::BLACK);
        });
        let done = done.or_else(|| self.dialog_keys(ctx));
        match done {
            Some(true) => {
                if let Some(d) = self.active_doc_mut() {
                    d.set_color_selected(st.new);
                }
                self.color_dialog = None;
                ctx.data_mut(|d| d.remove::<ColorState>(key));
            }
            Some(false) => {
                self.color_dialog = None;
                ctx.data_mut(|d| d.remove::<ColorState>(key));
            }
            None if open => ctx.data_mut(|d| d.insert_temp(key, st)),
            None => {
                self.color_dialog = None;
                ctx.data_mut(|d| d.remove::<ColorState>(key));
            }
        }
    }
}

/// «Спектр» (the template's page): the hue × saturation spectrum (2, 14,
/// 120 × 80 DLU) at full lightness ½, the lightness bar beside it (126, 14,
/// 18 × 80) and the six fields «Оттенок», «Насыщение», «Яркость»,
/// «Красный», «Зеленый», «Синий» (0…255) — our drawing of MFC's.
fn spectrum_page(ui: &mut egui::Ui, p: &egui::Painter, client: Rect, new: &mut Color32) {
    let page = Rect::from_min_size(client.min + Vec2::new(10.0, 30.0), Vec2::ZERO);
    let spec = dlu(page, 2, 14, 120, 80);
    let n = 24;
    let mut mesh = egui::Mesh::default();
    for i in 0..n {
        for j in 0..2 {
            let (h0, h1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            let (s0, s1) = (1.0 - j as f32 / 2.0, 1.0 - (j + 1) as f32 / 2.0);
            let x0 = spec.left() + spec.width() * h0;
            let x1 = spec.left() + spec.width() * h1;
            let y0 = spec.top() + spec.height() * (1.0 - s0);
            let y1 = spec.top() + spec.height() * (1.0 - s1);
            let k = mesh.vertices.len() as u32;
            for (x, y, h, s) in [(x0, y0, h0, s0), (x1, y0, h1, s0), (x1, y1, h1, s1), (x0, y1, h0, s1)] {
                mesh.colored_vertex(Pos2::new(x, y), hsl(h, s, 0.5));
            }
            mesh.add_triangle(k, k + 1, k + 2);
            mesh.add_triangle(k, k + 2, k + 3);
        }
    }
    p.add(mesh);
    let (h, s, l) = to_hsl(*new);
    let resp = ui.interact(spec, ui.id().with("fv_spec"), egui::Sense::click_and_drag());
    if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.is_pointer_button_down_on()) {
        let hh = ((q.x - spec.left()) / spec.width()).clamp(0.0, 1.0);
        let ss = 1.0 - ((q.y - spec.top()) / spec.height()).clamp(0.0, 1.0);
        *new = hsl(hh, ss, l);
    }
    let mark = Pos2::new(spec.left() + spec.width() * h, spec.top() + spec.height() * (1.0 - s));
    p.circle_stroke(mark, 4.0, Stroke::new(1.0f32, Color32::BLACK));
    let bar = dlu(page, 126, 14, 18, 80);
    let mut mesh = egui::Mesh::default();
    for i in 0..16 {
        let (l0, l1) = (1.0 - i as f32 / 16.0, 1.0 - (i + 1) as f32 / 16.0);
        let (y0, y1) = (bar.top() + bar.height() * i as f32 / 16.0, bar.top() + bar.height() * (i + 1) as f32 / 16.0);
        let k = mesh.vertices.len() as u32;
        for (x, y, ll) in [(bar.left(), y0, l0), (bar.right(), y0, l0), (bar.right(), y1, l1), (bar.left(), y1, l1)] {
            mesh.colored_vertex(Pos2::new(x, y), hsl(h, s, ll));
        }
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k, k + 2, k + 3);
    }
    p.add(mesh);
    let resp = ui.interact(bar, ui.id().with("fv_lum"), egui::Sense::click_and_drag());
    if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.is_pointer_button_down_on()) {
        *new = hsl(h, s, 1.0 - ((q.y - bar.top()) / bar.height()).clamp(0.0, 1.0));
    }
    let [r, g, b, _] = new.to_array();
    let vals = [(h * 255.0).round() as u8, (s * 255.0).round() as u8, (l * 255.0).round() as u8, r, g, b];
    let fields = [((2, 103, 38), (45, 102), "Оттенок:"), ((2, 120, 44), (45, 119), "Насыщение:"), ((2, 137, 38), (45, 136), "Яркость:"), ((82, 103, 32), (115, 102), "Красный:"), ((82, 120, 33), (115, 119), "Зеленый:"), ((82, 137, 28), (115, 136), "Синий:")];
    for (k, ((lx, ly, lw), (ex, ey), label)) in fields.into_iter().enumerate() {
        crate::dialogs::win_label(ui, dlu(page, lx, ly, lw, 8), label, false, true);
        let id = egui::Id::new(("fv_color_field", k));
        let mut text = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| vals[k].to_string());
        let resp = win_edit(ui, dlu(page, ex, ey, 30, 13), id, &mut text, false);
        if resp.changed() {
            if let Ok(v) = text.trim().parse::<u8>() {
                let mut v6 = vals;
                v6[k] = v;
                *new = if k < 3 { hsl(v6[0] as f32 / 255.0, v6[1] as f32 / 255.0, v6[2] as f32 / 255.0) } else { Color32::from_rgb(v6[3], v6[4], v6[5]) };
            }
            ui.data_mut(|d| d.insert_temp(id, text));
        } else if !resp.has_focus() {
            ui.data_mut(|d| d.remove::<String>(id));
        }
    }
}

/// HSL (0…1 each) to a colour.
fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h * 6.0) % 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let q = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(q(r), q(g), q(b))
}

/// A colour to HSL (0…1 each).
fn to_hsl(c: Color32) -> (f32, f32, f32) {
    let [r, g, b, _] = c.to_array().map(|v| v as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let sector = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    ((sector / 6.0).clamp(0.0, 1.0), s, l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_round_trips() {
        for c in [Color32::from_rgb(192, 192, 0), Color32::from_rgb(0, 128, 128), Color32::from_rgb(255, 87, 87), Color32::from_gray(150)] {
            let (h, s, l) = to_hsl(c);
            let back = hsl(h, s, l).to_array();
            for (a, b) in back.iter().zip(c.to_array()) {
                assert!((*a as i32 - b as i32).abs() <= 1, "{c:?} → {back:?}");
            }
        }
    }
}
