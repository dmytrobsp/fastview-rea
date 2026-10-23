//! Review elements of an oscillogram («Рецензирование осциллограммы», ledger
//! L9.6, `notes/UI_BEHAVIOUR_SPEC.md` §5): moments, level lines and areas,
//! each with a text, a lock and a "text hidden" flag. Placement is a one-shot
//! ribbon tool (155 / 156 / 157, `0x43eb40`), dragging moves the grabbed part
//! (`0x43efa0`), painting follows `0x43f630` and the info box `0x43df90`.

use crate::app::{App, ChanKey};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};

/// The three kinds (type() slot 1: 1 moment, 2 level, 3 area).
#[derive(Debug, Clone, PartialEq)]
pub enum CommentKind {
    /// A moment of time (визир) at a sample.
    Moment { pos: usize },
    /// A level line on an analog channel: a value in its units over a sample
    /// range.
    Level { chan: usize, value: f64, from: usize, to: usize },
    /// An area: a lane range × a sample range.
    Area { chan_from: ChanKey, chan_to: ChanKey, from: usize, to: usize },
}

/// One review element.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub kind: CommentKind,
    pub text: String,
    /// «Зафиксировать» (33039): a locked element does not move.
    pub locked: bool,
    /// «Скрыть комментарий» (33041): the text is hidden, the mark stays.
    pub hidden: bool,
}

/// Drag parts (`doc+0x85c`): 1 the "to" end while creating, 2 the moment /
/// left end / left edge, 3 the right end / right edge, 4 a level line's body;
/// 0 = hit but nothing to drag (an area's top or bottom edge).
pub type Part = u8;

/// Hit-test reach around an element, in pixels.
const REACH: f32 = 10.0;
const NORMAL: Color32 = Color32::from_rgb(255, 100, 100);
const SELECTED: Color32 = Color32::from_rgb(192, 0, 0);
const TEXT: Color32 = Color32::from_rgb(128, 0, 0);

impl Comment {
    pub fn new(kind: CommentKind) -> Comment {
        Comment { kind, text: String::new(), locked: false, hidden: false }
    }

    /// The kind number used for the menu labels (1 moment, 2 level, 3 area).
    pub fn kind_no(&self) -> u8 {
        match self.kind {
            CommentKind::Moment { .. } => 1,
            CommentKind::Level { .. } => 2,
            CommentKind::Area { .. } => 3,
        }
    }

    /// The moment's sample, for moments.
    pub fn moment(&self) -> Option<usize> {
        match self.kind {
            CommentKind::Moment { pos } => Some(pos),
            _ => None,
        }
    }

    /// Release after creating (`0x43ef50`): `from ≤ to`.
    pub fn normalise(&mut self) {
        match &mut self.kind {
            CommentKind::Level { from, to, .. } | CommentKind::Area { from, to, .. } if *from > *to => std::mem::swap(from, to),
            _ => {}
        }
    }

    /// Move part `part` to `sample` / the lane under the pointer
    /// (`0x43efa0`). `lane` is the lane under the pointer and, when it is
    /// the level line's own channel, the value there.
    pub fn drag(&mut self, part: Part, sample: usize, lane: Option<(ChanKey, Option<f64>)>) {
        if self.locked && part != 1 {
            return;
        }
        match &mut self.kind {
            CommentKind::Moment { pos } => *pos = sample,
            CommentKind::Level { chan, value, from, to } => match part {
                1 | 3 => *to = sample,
                2 => *from = sample,
                // The body moves onto the analog lane under the pointer,
                // taking its value there (`0x43f46e`, `0x4716d0`); over a
                // digital lane or none nothing changes.
                4 => {
                    if let Some((ChanKey::Analog(c), Some(v))) = lane {
                        (*chan, *value) = (c, v);
                    }
                }
                _ => {}
            },
            CommentKind::Area { chan_to, from, to, .. } => match part {
                1 => {
                    *to = sample;
                    if let Some((k, _)) = lane {
                        *chan_to = k;
                    }
                }
                2 => *from = sample,
                3 => *to = sample,
                _ => {}
            },
        }
    }

    /// Slot 9 — the info box text and the level / area label: the text,
    /// a blank line, then «Уровень: %.2f» (`0x470f30`), «Длительность:» of
    /// |Δt| (`0x403fa0`) or, for a moment (`0x47a4e0`), «Время:» with the
    /// absolute time and «От начала ОСЦ:» — the whole text empty when the
    /// record's start stamp does not parse. `us(k)`: sample k's time in µs
    /// from the first sample; `abs`: [`fvb_core::comtrade::moment_time_text`].
    pub fn info(&self, us: &dyn Fn(usize) -> i64, abs: &dyn Fn(i64) -> Option<String>) -> String {
        let text = &self.text;
        match self.kind {
            CommentKind::Moment { pos } => {
                let dt = us(pos);
                abs(dt).map_or_else(String::new, |a| format!("{text}\n\nВремя: {a}\nОт начала ОСЦ: {}", seconds_ms(dt)))
            }
            CommentKind::Level { value, .. } => format!("{text}\n\nУровень: {value:.2}"),
            CommentKind::Area { from, to, .. } => format!("{text}\n\nДлительность: {}", seconds_ms((us(to) - us(from)).abs())),
        }
    }
}

/// `%d.%03d с` of whole µs: seconds and milliseconds truncated.
pub fn seconds_ms(us: i64) -> String {
    format!("{}.{:03} с", us / 1_000_000, us % 1_000_000 / 1000)
}

/// What painting and hit-testing need from the view.
pub struct Geo<'a> {
    pub x_of: &'a dyn Fn(f64) -> f32,
    /// Top and height of a channel's lane, when it is laid out.
    pub lane: &'a dyn Fn(ChanKey) -> Option<(f32, f32)>,
    /// y of a value on an analog channel's scale.
    pub value_y: &'a dyn Fn(usize, f64) -> Option<f32>,
    /// Top of the plot and the bottom of the визир lines.
    pub top: f32,
    pub v_bottom: f32,
    /// Sample k's time in µs from the first sample and a moment's absolute
    /// time (see [`Comment::info`]).
    pub us: &'a dyn Fn(usize) -> i64,
    pub abs: &'a dyn Fn(i64) -> Option<String>,
}

/// An area's rectangle: its sample range × the span of its two lanes.
fn area_rect(from: usize, to: usize, a: ChanKey, b: ChanKey, geo: &Geo) -> Option<Rect> {
    let (ta, ha) = (geo.lane)(a)?;
    let (tb, hb) = (geo.lane)(b)?;
    let (x1, x2) = ((geo.x_of)(from.min(to) as f64), (geo.x_of)(from.max(to) as f64));
    Some(Rect::from_min_max(Pos2::new(x1, ta.min(tb)), Pos2::new(x2, (ta + ha).max(tb + hb))))
}

/// The grabbed part of `c` at `pos`, if it is hit (±10 px; an area only on
/// its border band).
pub fn hit(c: &Comment, pos: Pos2, geo: &Geo) -> Option<Part> {
    match c.kind {
        CommentKind::Moment { pos: k } => (((geo.x_of)(k as f64) - pos.x).abs() <= REACH).then_some(2),
        CommentKind::Level { chan, value, from, to } => {
            let y = (geo.value_y)(chan, value)?;
            let (x1, x2) = ((geo.x_of)(from.min(to) as f64), (geo.x_of)(from.max(to) as f64));
            if pos.x < x1 - REACH || pos.x > x2 + REACH || (pos.y - y).abs() > REACH {
                return None;
            }
            Some(if (pos.x - x1).abs() < REACH {
                2
            } else if (pos.x - x2).abs() < REACH {
                3
            } else {
                4
            })
        }
        CommentKind::Area { chan_from, chan_to, from, to } => {
            let r = area_rect(from, to, chan_from, chan_to, geo)?;
            if !r.expand(REACH).contains(pos) || r.shrink(REACH).contains(pos) {
                return None;
            }
            Some(if (pos.x - r.left()).abs() <= REACH {
                2
            } else if (pos.x - r.right()).abs() <= REACH {
                3
            } else {
                0
            })
        }
    }
}

/// A small speech-bubble mark (the original draws a bitmap).
fn bubble(p: &egui::Painter, at: Pos2, color: Color32) {
    let body = Rect::from_min_size(at, Vec2::new(14.0, 10.0));
    p.rect_filled(body, 3.0, Color32::WHITE);
    p.rect_stroke(body, 3.0, Stroke::new(1.0f32, color));
    p.add(Shape::convex_polygon(
        vec![Pos2::new(at.x + 3.0, at.y + 10.0), Pos2::new(at.x + 8.0, at.y + 10.0), Pos2::new(at.x + 3.0, at.y + 14.0)],
        color,
        Stroke::NONE,
    ));
}

/// Paint every element, then the selected one (`hover`, the element under
/// the pointer) once more on top with its info box — as `0x43f630` does, so
/// a selected area's translucent fill is laid twice.
pub fn paint(p: &egui::Painter, comments: &[Comment], hover: Option<usize>, geo: &Geo) {
    let font = FontId::proportional(11.0);
    for (i, c) in comments.iter().enumerate() {
        draw(p, c, hover == Some(i), geo, &font);
    }
    let Some(c) = hover.and_then(|i| comments.get(i)) else { return };
    let Some(at) = draw(p, c, true, geo, &font) else { return };
    let galley = p.layout_no_wrap(c.info(geo.us, geo.abs), font, Color32::from_rgb(0, 0, 192));
    let r = Rect::from_min_size(at, galley.size() + Vec2::splat(10.0));
    p.rect_filled(r, 10.0, Color32::from_rgb(255, 230, 230));
    p.rect_stroke(r, 10.0, Stroke::new(1.0f32, Color32::from_rgb(192, 175, 175)));
    p.galley(r.min + Vec2::splat(5.0), galley, Color32::from_rgb(0, 0, 192));
}

/// One element (its slot +0x18); the info box's top-left, or `None` when it
/// has no place in the view.
fn draw(p: &egui::Painter, c: &Comment, sel: bool, geo: &Geo, font: &FontId) -> Option<Pos2> {
    let col = if sel { SELECTED } else { NORMAL };
    let right_top = match c.kind {
        CommentKind::Moment { pos } => {
            let x = (geo.x_of)(pos as f64);
            bubble(p, Pos2::new(x - 3.0, geo.top + 16.0), col);
            p.vline(x, geo.top + 30.0..=geo.v_bottom, Stroke::new(if sel { 3.0f32 } else { 1.0 }, col));
            Pos2::new(x + 3.0, geo.top + 30.0)
        }
        CommentKind::Level { chan, value, from, to } => {
            let y = (geo.value_y)(chan, value)?;
            let (x1, x2) = ((geo.x_of)(from.min(to) as f64), (geo.x_of)(from.max(to) as f64));
            p.hline(x1..=x2, y, Stroke::new(3.0f32, col));
            for x in [x1, x2] {
                p.circle(Pos2::new(x, y), 5.0, Color32::from_rgb(255, 0, 0), Stroke::new(1.0f32, col));
            }
            bubble(p, Pos2::new(x2 + 10.0, y - 7.0), col);
            if !c.hidden {
                p.text(Pos2::new(x1 + 5.0, y + 5.0), Align2::LEFT_TOP, c.info(geo.us, geo.abs), font.clone(), TEXT);
            }
            Pos2::new(x2 + 3.0, y)
        }
        CommentKind::Area { chan_from, chan_to, from, to } => {
            let r = area_rect(from, to, chan_from, chan_to, geo)?;
            p.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(255, 0, 0, 64));
            p.rect_stroke(r, 0.0, Stroke::new(3.0f32, col));
            if !c.hidden {
                p.text(Pos2::new(r.left() + 5.0, r.bottom() - 5.0), Align2::LEFT_BOTTOM, c.info(geo.us, geo.abs), font.clone(), TEXT);
            }
            r.right_top() + Vec2::new(3.0, 0.0)
        }
    };
    Some(right_top)
}

/// State of the comment text dialog (DIALOG 332 in item mode).
#[derive(Debug, Clone)]
pub struct CommentEdit {
    pub idx: usize,
    pub text: String,
}

impl App {
    /// «Комментарии к диаграмме»: the element's text; OK sets it.
    pub fn comment_window(&mut self, ctx: &egui::Context) {
        let Some(mut ed) = self.comment_edit.take() else { return };
        let mut open = true;
        let mut done: Option<bool> = None;
        egui::Window::new("Комментарии к диаграмме")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.add(egui::TextEdit::multiline(&mut ed.text).desired_rows(8).desired_width(360.0));
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        done = Some(true);
                    }
                    if ui.button("Отмена").clicked() {
                        done = Some(false);
                    }
                });
            });
        match done {
            Some(true) => {
                if let Some(c) = self.active_doc_mut().and_then(|d| d.comments.get_mut(ed.idx)) {
                    c.text = ed.text;
                }
            }
            Some(false) => {}
            None if open => self.comment_edit = Some(ed),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo_test<'a>(x_of: &'a dyn Fn(f64) -> f32, lane: &'a dyn Fn(ChanKey) -> Option<(f32, f32)>, vy: &'a dyn Fn(usize, f64) -> Option<f32>) -> Geo<'a> {
        fn us(k: usize) -> i64 {
            k as i64 * 1000
        }
        fn abs(_: i64) -> Option<String> {
            None
        }
        Geo { x_of, lane, value_y: vy, top: 0.0, v_bottom: 400.0, us: &us, abs: &abs }
    }

    #[test]
    fn hit_parts_per_kind() {
        let x_of = |k: f64| (k * 10.0) as f32;
        let lane = |k: ChanKey| match k {
            ChanKey::Analog(0) => Some((0.0, 100.0)),
            ChanKey::Analog(1) => Some((100.0, 100.0)),
            _ => None,
        };
        let vy = |_: usize, v: f64| Some(100.0 - v as f32);
        let g = geo_test(&x_of, &lane, &vy);
        let m = Comment::new(CommentKind::Moment { pos: 20 });
        assert_eq!(hit(&m, Pos2::new(208.0, 50.0), &g), Some(2));
        assert_eq!(hit(&m, Pos2::new(215.0, 50.0), &g), None);
        let l = Comment::new(CommentKind::Level { chan: 0, value: 40.0, from: 10, to: 30 });
        assert_eq!(hit(&l, Pos2::new(101.0, 62.0), &g), Some(2), "left end");
        assert_eq!(hit(&l, Pos2::new(298.0, 58.0), &g), Some(3), "right end");
        assert_eq!(hit(&l, Pos2::new(200.0, 55.0), &g), Some(4), "body");
        assert_eq!(hit(&l, Pos2::new(200.0, 80.0), &g), None);
        let a = Comment::new(CommentKind::Area { chan_from: ChanKey::Analog(0), chan_to: ChanKey::Analog(1), from: 10, to: 30 });
        assert_eq!(hit(&a, Pos2::new(104.0, 120.0), &g), Some(2), "left edge");
        assert_eq!(hit(&a, Pos2::new(296.0, 50.0), &g), Some(3), "right edge");
        assert_eq!(hit(&a, Pos2::new(200.0, 3.0), &g), Some(0), "top edge: hit, nothing to drag");
        assert_eq!(hit(&a, Pos2::new(200.0, 100.0), &g), None, "only the border band");
    }

    #[test]
    fn drag_and_lock() {
        let mut l = Comment::new(CommentKind::Level { chan: 0, value: 1.0, from: 10, to: 10 });
        l.drag(1, 5, None);
        l.normalise();
        assert_eq!(l.kind, CommentKind::Level { chan: 0, value: 1.0, from: 5, to: 10 });
        l.drag(4, 0, Some((ChanKey::Analog(1), Some(9.0))));
        assert_eq!(l.kind, CommentKind::Level { chan: 1, value: 9.0, from: 5, to: 10 }, "another analog lane: the level moves onto it");
        l.drag(4, 0, Some((ChanKey::Digital(0), None)));
        assert_eq!(l.kind, CommentKind::Level { chan: 1, value: 9.0, from: 5, to: 10 }, "a digital lane: nothing");
        l.drag(4, 0, Some((ChanKey::Analog(0), Some(2.5))));
        assert!(matches!(l.kind, CommentKind::Level { chan: 0, value, .. } if value == 2.5));
        l.locked = true;
        l.drag(2, 0, None);
        assert!(matches!(l.kind, CommentKind::Level { from: 5, .. }), "locked elements do not move");
        assert_eq!(seconds_ms(1_234_999), "1.234 с", "truncated");
    }

    #[test]
    fn info_texts_as_slot_9() {
        let us = |k: usize| k as i64 * 250_500;
        let abs = |dt: i64| Some(format!("T+{dt}"));
        let none = |_: i64| None;
        let mut m = Comment::new(CommentKind::Moment { pos: 4 });
        assert_eq!(m.info(&us, &abs), "\n\nВремя: T+1002000\nОт начала ОСЦ: 1.002 с");
        m.text = "КЗ".into();
        assert_eq!(m.info(&us, &abs), "КЗ\n\nВремя: T+1002000\nОт начала ОСЦ: 1.002 с");
        assert_eq!(m.info(&us, &none), "", "an unparsable start stamp empties the text");
        let l = Comment::new(CommentKind::Level { chan: 0, value: 2.345, from: 0, to: 1 });
        assert_eq!(l.info(&us, &none), "\n\nУровень: 2.35");
        let a = Comment::new(CommentKind::Area { chan_from: ChanKey::Analog(0), chan_to: ChanKey::Analog(0), from: 7, to: 3 });
        assert_eq!(a.info(&us, &none), "\n\nДлительность: 1.002 с");
    }
}
