//! Clean-room vector icons for the ribbon, drawn with the egui painter.
//!
//! These are original line-art icons that evoke the roles of the corresponding
//! original commands; the proprietary toolbar bitmaps are *not* embedded (they
//! are kept local, never redistributed — see `.gitignore`).

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Open,
    Save,
    Print,
    Copy,
    Paste,
    Undo,
    Redo,
    Freq,
    Vector,
    Circle,
    Diff,
    Channels,
    Analysis,
    Wave,
    Invert,
    RemoveDc,
    ShowAll,
    HideAll,
    Primary,
    Params,
    OmpRun,
}

/// Paint `icon` centred in `rect`, scaled to it, using `color` as the primary
/// stroke (a few icons add fixed accent colours).
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    // Work in a padded unit square mapped into `rect`.
    let pad = rect.width().min(rect.height()) * 0.12;
    let r = Rect::from_min_max(
        rect.min + Vec2::splat(pad),
        rect.max - Vec2::splat(pad),
    );
    let s = r.width().min(r.height());
    let p = |x: f32, y: f32| Pos2::new(r.left() + x * r.width(), r.top() + y * r.height());
    let w = (s * 0.11).max(1.2);
    let stroke = Stroke::new(w, color);
    let line = |painter: &egui::Painter, a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);
    let poly = |painter: &egui::Painter, pts: Vec<Pos2>| {
        painter.add(Shape::closed_line(pts, stroke));
    };
    let path = |painter: &egui::Painter, pts: Vec<Pos2>| {
        painter.add(Shape::line(pts, stroke));
    };

    match icon {
        Icon::Open => {
            // folder
            poly(painter, vec![p(0.08, 0.30), p(0.40, 0.30), p(0.50, 0.42), p(0.92, 0.42), p(0.92, 0.82), p(0.08, 0.82)]);
            line(painter, p(0.08, 0.50), p(0.92, 0.50));
        }
        Icon::Save => {
            // floppy disk
            poly(painter, vec![p(0.15, 0.15), p(0.78, 0.15), p(0.88, 0.25), p(0.88, 0.85), p(0.15, 0.85)]);
            painter.rect_stroke(Rect::from_min_max(p(0.32, 0.15), p(0.70, 0.40)), 0.0, stroke);
            painter.rect_stroke(Rect::from_min_max(p(0.28, 0.55), p(0.75, 0.82)), 0.0, stroke);
        }
        Icon::Print => {
            painter.rect_stroke(Rect::from_min_max(p(0.30, 0.12), p(0.70, 0.34)), 0.0, stroke);
            painter.rect_stroke(Rect::from_min_max(p(0.14, 0.34), p(0.86, 0.66)), 0.0, stroke);
            painter.rect_stroke(Rect::from_min_max(p(0.28, 0.62), p(0.72, 0.88)), 0.0, stroke);
        }
        Icon::Copy => {
            painter.rect_stroke(Rect::from_min_max(p(0.14, 0.14), p(0.62, 0.70)), 0.0, stroke);
            painter.rect_stroke(Rect::from_min_max(p(0.38, 0.32), p(0.86, 0.88)), 0.0, stroke);
        }
        Icon::Paste => {
            painter.rect_stroke(Rect::from_min_max(p(0.18, 0.18), p(0.82, 0.88)), 0.0, stroke);
            painter.rect_stroke(Rect::from_min_max(p(0.36, 0.10), p(0.64, 0.24)), 0.0, stroke);
            line(painter, p(0.30, 0.42), p(0.70, 0.42));
            line(painter, p(0.30, 0.56), p(0.70, 0.56));
            line(painter, p(0.30, 0.70), p(0.56, 0.70));
        }
        Icon::Undo | Icon::Redo => {
            let flip = icon == Icon::Redo;
            let fx = |x: f32| if flip { 1.0 - x } else { x };
            // arc
            let arc: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let a = std::f32::consts::PI * (0.15 + 0.7 * i as f32 / 10.0);
                    p(fx(0.5 + 0.34 * a.cos()), 0.56 - 0.30 * a.sin())
                })
                .collect();
            path(painter, arc);
            // arrowhead at the left end
            let tip = p(fx(0.18), 0.52);
            line(painter, tip, p(fx(0.30), 0.40));
            line(painter, tip, p(fx(0.30), 0.64));
        }
        Icon::Freq => {
            // spectrum bars
            for (i, h) in [0.75f32, 0.45, 0.60, 0.30].iter().enumerate() {
                let x = 0.18 + i as f32 * 0.22;
                let col = [Color32::from_rgb(0x20, 0x4A, 0xA0), Color32::from_rgb(0xCC, 0x22, 0x22), Color32::from_rgb(0x18, 0x8A, 0x42), Color32::from_rgb(0xD4, 0xA0, 0x17)][i];
                painter.rect_filled(Rect::from_min_max(p(x, 0.88 - h), p(x + 0.13, 0.88)), 0.0, col);
            }
        }
        Icon::Vector => {
            let red = Stroke::new(w, Color32::from_rgb(0xCC, 0x22, 0x22));
            let grn = Stroke::new(w, Color32::from_rgb(0x18, 0x8A, 0x42));
            let o = p(0.20, 0.82);
            painter.line_segment([o, p(0.86, 0.26)], red);
            painter.line_segment([o, p(0.80, 0.70)], grn);
            painter.line_segment([o, p(0.40, 0.18)], stroke);
        }
        Icon::Circle => {
            painter.circle_stroke(p(0.5, 0.5), s * 0.38, stroke);
            line(painter, p(0.5, 0.08), p(0.5, 0.92));
            line(painter, p(0.08, 0.5), p(0.92, 0.5));
        }
        Icon::Diff => {
            path(painter, vec![p(0.10, 0.80), p(0.30, 0.78), p(0.45, 0.40), p(0.55, 0.38), p(0.70, 0.74), p(0.90, 0.72)]);
            line(painter, p(0.08, 0.88), p(0.92, 0.88));
        }
        Icon::Channels | Icon::ShowAll | Icon::HideAll => {
            for (i, on) in [true, true, icon != Icon::HideAll].iter().enumerate() {
                let y = 0.24 + i as f32 * 0.26;
                // checkbox
                painter.rect_stroke(Rect::from_min_max(p(0.10, y - 0.08), p(0.26, y + 0.08)), 0.0, stroke);
                if *on {
                    line(painter, p(0.12, y), p(0.17, y + 0.06));
                    line(painter, p(0.17, y + 0.06), p(0.24, y - 0.06));
                }
                line(painter, p(0.34, y), p(0.90, y));
            }
        }
        Icon::Analysis => {
            painter.circle_stroke(p(0.42, 0.42), s * 0.26, stroke);
            line(painter, p(0.60, 0.60), p(0.86, 0.86));
        }
        Icon::Wave => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let t = i as f32 / 24.0;
                    p(0.08 + t * 0.84, 0.5 - 0.34 * (t * std::f32::consts::TAU).sin())
                })
                .collect();
            path(painter, pts);
        }
        Icon::Invert => {
            let up: Vec<Pos2> = (0..=12).map(|i| { let t = i as f32 / 12.0; p(0.08 + t * 0.84, 0.36 - 0.22 * (t * std::f32::consts::TAU).sin()) }).collect();
            let dn: Vec<Pos2> = (0..=12).map(|i| { let t = i as f32 / 12.0; p(0.08 + t * 0.84, 0.64 + 0.22 * (t * std::f32::consts::TAU).sin()) }).collect();
            path(painter, up);
            painter.add(Shape::line(dn, Stroke::new(w, color.gamma_multiply(0.55))));
        }
        Icon::RemoveDc => {
            let pts: Vec<Pos2> = (0..=24).map(|i| { let t = i as f32 / 24.0; p(0.08 + t * 0.84, 0.36 - 0.22 * (t * std::f32::consts::TAU).sin()) }).collect();
            path(painter, pts);
            painter.line_segment([p(0.08, 0.74), p(0.92, 0.74)], Stroke::new(w, Color32::from_rgb(0xCC, 0x22, 0x22)));
        }
        Icon::Primary => {
            // "1°/2°" primary-vs-secondary: a transformer-ish double circle
            painter.circle_stroke(p(0.36, 0.5), s * 0.24, stroke);
            painter.circle_stroke(p(0.62, 0.5), s * 0.24, stroke);
        }
        Icon::Params => {
            // three sliders
            for (i, kx) in [0.30f32, 0.60, 0.45].iter().enumerate() {
                let y = 0.26 + i as f32 * 0.24;
                line(painter, p(0.12, y), p(0.88, y));
                painter.rect_filled(Rect::from_min_max(p(kx - 0.05, y - 0.07), p(kx + 0.05, y + 0.07)), 0.0, color);
            }
        }
        Icon::OmpRun => {
            painter.circle_stroke(p(0.5, 0.5), s * 0.34, stroke);
            // lightning bolt
            let bolt = Stroke::new(w, Color32::from_rgb(0xE0, 0xA8, 0x1E));
            painter.add(Shape::line(
                vec![p(0.56, 0.20), p(0.42, 0.52), p(0.54, 0.52), p(0.44, 0.80)],
                bolt,
            ));
        }
    }
}
