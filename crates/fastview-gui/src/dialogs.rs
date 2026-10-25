//! Modal-ish dialog windows: About, channel properties, and the comment editor.

use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

/// A dialog window as MFC places one: centred on the main window at its
/// first showing (`_AfxPostInitDialog` centres a dialog whose template
/// leaves it at 0, 0 — seen on the VM), movable afterwards, in the
/// system's dialog frame (`Dlg::show`).
pub fn dlg<'o>(ctx: &egui::Context, title: impl Into<String>) -> Dlg<'o> {
    let title = title.into();
    Dlg {
        // The first frame (egui's sizing pass, size unknown): about the
        // centre; `show` then places it exactly.
        win: egui::Window::new(title.clone()).pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.screen_rect().center()),
        id: egui::Id::new(&title),
        title,
        nudge: Vec2::ZERO,
        owner: None,
        sysmenu: true,
        open: None,
        tool: false,
        margin: 11.0,
    }
}

/// A dialog window drawn in the frame Windows gives the original's dialogs
/// (VM, Windows 11): a 1 px border, a 25 px (243) caption with the title
/// 5 px in, the client on COLOR_3DFACE (240) with black text. The close
/// button is a thin 10 px × 11 px from the right edge — or, for a
/// WS_EX_TOOLWINDOW template (DIALOG 463 only), a 19 px (199, 80, 80) box
/// with a small white ×.
pub struct Dlg<'o> {
    win: egui::Window<'static>,
    id: egui::Id,
    title: String,
    nudge: Vec2,
    owner: Option<egui::Id>,
    sysmenu: bool,
    open: Option<&'o mut bool>,
    tool: bool,
    margin: f32,
}

/// The frame's colours (VM): the caption, the client, the border.
const DLG_CAPTION: Color32 = Color32::from_rgb(243, 243, 243);
const DLG_FACE: Color32 = Color32::from_rgb(240, 240, 240);
const DLG_BORDER: Color32 = Color32::from_rgba_premultiplied(45, 45, 45, 102);
const DLG_CAPTION_H: f32 = 25.0;
/// The controls' font: the templates' MS Shell Dlg 8 (Tahoma 8 pt, 11 px);
/// the caption is the system's 9 pt (12 px).
pub const DLG_FONT: egui::FontId = egui::FontId::new(11.0, egui::FontFamily::Proportional);

impl<'o> Dlg<'o> {
    pub fn collapsible(self, _: bool) -> Self {
        self
    }
    pub fn resizable(mut self, on: bool) -> Self {
        self.win = self.win.resizable(on);
        self
    }
    pub fn open(mut self, open: &'o mut bool) -> Self {
        self.open = Some(open);
        self
    }
    pub fn id(mut self, id: egui::Id) -> Self {
        self.win = self.win.id(id);
        self.id = id;
        self
    }
    pub fn anchor(mut self, align: egui::Align2, offset: impl Into<Vec2>) -> Self {
        self.win = self.win.anchor(align, offset);
        self
    }
    pub fn order(mut self, order: egui::Order) -> Self {
        self.win = self.win.order(order);
        self
    }
    pub fn interactable(mut self, on: bool) -> Self {
        self.win = self.win.interactable(on);
        self
    }
    /// An offset from the CenterWindow place (a window that grows after
    /// it was centred keeps its left-top).
    pub fn nudge(mut self, d: Vec2) -> Self {
        self.nudge = d;
        self
    }
    /// The dialog's owner, another of our windows (by its id): it is
    /// centred on that window instead of the main one.
    pub fn owner(mut self, id: egui::Id) -> Self {
        self.owner = Some(id);
        self
    }
    /// A template without WS_SYSMENU: no close button in the caption.
    pub fn no_close(mut self) -> Self {
        self.sysmenu = false;
        self
    }
    /// A WS_EX_TOOLWINDOW template's frame.
    pub fn tool(mut self) -> Self {
        self.tool = true;
        self
    }
    /// The client's margin (default 11 px, the templates' 7 dialog units).
    pub fn margin(mut self, m: f32) -> Self {
        self.margin = m;
        self
    }

    pub fn show<R>(self, ctx: &egui::Context, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> Option<egui::InnerResponse<Option<R>>> {
        let Dlg { mut win, id, title, nudge, owner, sysmenu, open, tool, margin } = self;
        if open.as_deref() == Some(&false) {
            return None;
        }
        // CenterWindow at every opening (each DoModal creates the dialog
        // anew), from the window's measured size — for its first frames
        // after it appears (egui's own pivot placement rounds half-way
        // positions by the size it had a frame before); movable after.
        let key = id.with("fv_dlg_placed");
        let frame = ctx.cumulative_pass_nr();
        let size = ctx.memory(|m| m.area_rect(id)).map(|r| r.size());
        // (the last frame shown, its size, whether the size has settled)
        let prev: Option<(u64, Option<Vec2>, bool)> = ctx.data(|d| d.get_temp(key));
        let settled = match prev {
            Some((last, prev_size, settled)) if last + 1 == frame => settled || (size.is_some() && size == prev_size),
            _ => false,
        };
        if let Some(size) = size {
            // egui keeps the pivot's position: once placed, the left-top.
            win = win.pivot(egui::Align2::LEFT_TOP);
            if !settled {
                // CenterWindow centres the window rect on its owner's, both
                // holding Windows 11's invisible 2 px frame left, right and
                // below the visible one: the owner's centre is
                // (⌊(l + r)/2⌋, ⌊(t + b + 2)/2⌋) of its visible rect (r, b
                // exclusive), the maximised main window's the screen's
                // centre; the visible left-top (cx − ⌊(w + 4)/2⌋ + 2,
                // cy − ⌊(h + 2)/2⌋). VM: DIALOGs 463, 477, 392 at (1101,
                // 306), (1023, 475), (1019, 457) about (1280, 696); 359 on
                // «Быстрый анализ» (1016–1688 × 317–1036) at (1038, 411).
                let o = owner.and_then(|o| ctx.memory(|m| m.area_rect(o)));
                let (cx, cy) = match o {
                    Some(o) => (((o.left() + o.right()) / 2.0).floor(), ((o.top() + o.bottom() + 2.0) / 2.0).floor()),
                    None => {
                        let s = ctx.screen_rect();
                        (((s.left() + s.right()) / 2.0).floor(), ((s.top() + s.bottom()) / 2.0).floor())
                    }
                };
                let at = Pos2::new(cx - ((size.x + 4.0) / 2.0).floor() + 2.0, cy - ((size.y + 2.0) / 2.0).floor());
                win = win.current_pos(at + nudge);
            }
        }
        ctx.data_mut(|d| d.insert_temp(key, (frame, size, settled)));
        // A dialog moves by its caption only (Windows): egui's own dragging
        // of a window by its background is off; a caption drag keeps the
        // place it reached until the dialog is opened anew.
        let moved_key = id.with("fv_dlg_moved");
        if prev.is_none_or(|(last, _, _)| last + 1 != frame) {
            ctx.data_mut(|d| d.remove::<Pos2>(moved_key));
        }
        if let Some(at) = ctx.data(|d| d.get_temp::<Pos2>(moved_key)) {
            win = win.current_pos(at);
        }
        let win = win.movable(false);
        // The DWM shadow, its look approximated.
        let shadow = egui::epaint::Shadow { offset: Vec2::new(0.0, 6.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(60) };
        // The border blends over what lies behind the window: the fill is
        // painted inside it.
        let frame = egui::Frame::none().inner_margin(1.0).shadow(shadow);
        let mut close = false;
        let r = win.title_bar(false).collapsible(false).frame(frame).show(ctx, |ui| {
            // The client's fill, under the content (set once its size is known).
            let face = ui.painter().add(egui::Shape::Noop);
            let font = egui::FontId::proportional(12.0);
            let tw = ui.fonts(|f| f.layout_no_wrap(title.clone(), font.clone(), Color32::BLACK).size().x);
            let spacing = ui.spacing().item_spacing;
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (cap, _) = ui.allocate_exact_size(Vec2::new(5.0 + tw + if tool { 30.0 } else { 56.0 }, DLG_CAPTION_H), egui::Sense::hover());
            ui.spacing_mut().item_spacing = spacing;
            let inner = egui::Frame::none()
                .inner_margin(margin)
                .show(ui, |ui| {
                    ui.visuals_mut().override_text_color = Some(Color32::BLACK);
                    // A client laid out on its template places every control
                    // itself: no spacing after any of them.
                    if margin == 0.0 {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                    }
                    // The templates' MS Shell Dlg 8 (Tahoma 8 pt: 11 px).
                    for st in [egui::TextStyle::Body, egui::TextStyle::Button] {
                        ui.style_mut().text_styles.insert(st, DLG_FONT);
                    }
                    add_contents(ui)
                })
                .inner;
            let full = ui.min_rect();
            let cap = Rect::from_min_max(Pos2::new(full.left(), cap.top()), Pos2::new(full.right(), cap.bottom()));
            let p = ui.painter();
            p.set(face, egui::Shape::rect_filled(Rect::from_min_max(Pos2::new(full.left(), cap.bottom()), full.max), 0.0, DLG_FACE));
            p.rect_filled(cap, 0.0, DLG_CAPTION);
            p.text(Pos2::new(cap.left() + 4.0, cap.center().y), egui::Align2::LEFT_CENTER, &title, font, Color32::BLACK);
            // The border: 1 px around the client and caption, as lines on
            // the pixel centres (a 1 px filled rect is feathered twice).
            let o = full.expand(1.0);
            let b = Stroke::new(1.0f32, DLG_BORDER);
            p.hline(o.left()..=o.right(), o.top() + 0.5, b);
            p.hline(o.left()..=o.right(), o.bottom() - 0.5, b);
            p.vline(o.left() + 0.5, o.top() + 1.0..=o.bottom() - 1.0, b);
            p.vline(o.right() - 0.5, o.top() + 1.0..=o.bottom() - 1.0, b);
            // The close button (the right border is at o.right() − 1).
            let rb = o.right() - 1.0;
            let (btn, glyph) = if tool {
                let b = Rect::from_min_size(Pos2::new(rb - 20.0, cap.top() + 2.0), Vec2::splat(19.0));
                (b, Rect::from_min_size(b.min + Vec2::splat(7.0), Vec2::splat(5.0)))
            } else {
                let g = Rect::from_min_size(Pos2::new(rb - 21.0, cap.top() + 7.0), Vec2::splat(10.0));
                (Rect::from_min_max(Pos2::new(rb - 46.0, cap.top()), Pos2::new(rb, cap.bottom())), g)
            };
            let drag = ui.interact(Rect::from_min_max(cap.min, Pos2::new(btn.left(), cap.bottom())), ui.id().with("fv_dlg_caption"), egui::Sense::drag());
            if drag.dragged() {
                let to = o.min + drag.drag_delta();
                ui.ctx().data_mut(|d| d.insert_temp(moved_key, to));
            }
            if !sysmenu {
                return inner;
            }
            let resp = ui.interact(btn, ui.id().with("fv_dlg_close"), egui::Sense::click());
            close = resp.clicked();
            // The hover look is Windows' (red, white glyph) — not measured.
            let (bg, ink) = match (tool, resp.hovered()) {
                (true, false) => (Some(Color32::from_rgb(199, 80, 80)), Color32::WHITE),
                (true, true) => (Some(Color32::from_rgb(224, 67, 67)), Color32::WHITE),
                (false, false) => (None, Color32::from_rgb(23, 24, 24)),
                (false, true) => (Some(Color32::from_rgb(196, 43, 28)), Color32::WHITE),
            };
            if let Some(bg) = bg {
                p.rect_filled(btn, 0.0, bg);
            }
            if tool {
                // The 5 × 5 bold × of the bitmap: two pixels wide but at the
                // centre.
                for (dx, dy) in [(0, 0), (1, 0), (3, 0), (4, 0), (1, 1), (2, 1), (3, 1), (2, 2), (1, 3), (2, 3), (3, 3), (0, 4), (1, 4), (3, 4), (4, 4)] {
                    p.rect_filled(Rect::from_min_size(glyph.min + Vec2::new(dx as f32, dy as f32), Vec2::splat(1.0)), 0.0, ink);
                }
            } else {
                let s = Stroke::new(1.0f32, ink);
                p.line_segment([glyph.left_top() + Vec2::splat(0.5), glyph.right_bottom() - Vec2::splat(0.5)], s);
                p.line_segment([glyph.right_top() + Vec2::new(-0.5, 0.5), glyph.left_bottom() + Vec2::new(0.5, -0.5)], s);
            }
            inner
        });
        if close {
            if let Some(o) = open {
                *o = false;
            }
        }
        r
    }
}


/// A dialog's push button at its template rectangle (VM, Windows 11
/// theme): the face inset 1 px, corners rounded 4 px, (253) with a (208)
/// border darkening to (186) along the bottom; the default button's border
/// (0, 120, 212) to (0, 108, 191). Hover and pressed are the theme's
/// (approximated); the text centred, black.
pub fn win_button(ui: &mut egui::Ui, r: Rect, text: &str, default: bool) -> egui::Response {
    win_button_en(ui, r, text, default, true)
}

/// `win_button`, disabled: a paler face and grey text, deaf to clicks
/// (the disabled look approximated).
pub fn win_button_en(ui: &mut egui::Ui, r: Rect, text: &str, default: bool, enabled: bool) -> egui::Response {
    let resp = ui.interact(r, ui.id().with(("fv_btn", text, r.min.x as i32, r.min.y as i32)), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let face = r.shrink(1.0);
    if !enabled {
        let p = ui.painter();
        p.rect_filled(face, 4.0, Color32::from_gray(229));
        p.rect_filled(face.shrink(1.0), 3.0, Color32::from_gray(245));
        p.text(face.center(), egui::Align2::CENTER_CENTER, text, DLG_FONT, Color32::from_gray(160));
        return resp;
    }
    let (fill, edge, bottom) = match (resp.is_pointer_button_down_on(), resp.hovered(), default) {
        (true, _, _) => (Color32::from_rgb(204, 228, 247), Color32::from_rgb(0, 84, 153), Color32::from_rgb(0, 84, 153)),
        (false, true, _) => (Color32::from_rgb(224, 238, 249), Color32::from_rgb(0, 120, 212), Color32::from_rgb(0, 108, 191)),
        (false, false, true) => (Color32::from_gray(253), Color32::from_rgb(0, 120, 212), Color32::from_rgb(0, 108, 191)),
        (false, false, false) => (Color32::from_gray(253), Color32::from_gray(208), Color32::from_gray(186)),
    };
    let p = ui.painter();
    // The bottom edge's colour under the face, the border over the rest.
    p.rect_filled(face, 4.0, bottom);
    p.rect_filled(Rect::from_min_max(face.min, Pos2::new(face.right(), face.bottom() - 1.0)), 4.0, edge);
    p.rect_filled(face.shrink(1.0), 3.0, fill);
    p.text(face.center(), egui::Align2::CENTER_CENTER, text, DLG_FONT, Color32::BLACK);
    resp
}

/// A template rectangle in the client's pixels: each field through MulDiv
/// at MS Shell Dlg 8's base units (6 × 13), as the dialog manager converts
/// a template's controls (VM: DIALOG 477's list at 7, 7, 327 × 221 DLU is
/// 11, 11, 491 × 359 px).
pub fn dlu(client: Rect, x: i32, y: i32, w: i32, h: i32) -> Rect {
    let md = |a: i32, b: i32, c: i32| ((a * b * 2 + c) / (2 * c)) as f32;
    Rect::from_min_size(client.min + Vec2::new(md(x, 6, 4), md(y, 13, 8)), Vec2::new(md(w, 6, 4), md(h, 13, 8)))
}

/// A static control's text: from its rectangle's top, word-wrapped to its
/// width, SS_LEFT or SS_RIGHT; a disabled one in the theme's (109). A
/// one-line control keeps its text on one line and unclipped: our font is
/// wider than Tahoma, and wrapping there would drop words the original
/// shows (W6).
pub fn win_label(ui: &egui::Ui, r: Rect, text: &str, right: bool, enabled: bool) {
    let ink = if enabled { Color32::BLACK } else { Color32::from_gray(109) };
    let one_line = r.height() < 26.0;
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), DLG_FONT, ink, if one_line { f32::INFINITY } else { r.width() });
    job.halign = if right { egui::Align::RIGHT } else { egui::Align::LEFT };
    let g = ui.fonts(|f| f.layout_job(job));
    let at = if right { Pos2::new(r.right(), r.top()) } else { r.left_top() };
    // A multi-line one is clipped at its sides only: a line more in our
    // font stays readable below it.
    let clip = if one_line { ui.clip_rect() } else { Rect::from_min_max(r.min, Pos2::new(r.right(), ui.clip_rect().bottom())).intersect(ui.clip_rect()) };
    ui.painter().with_clip_rect(clip).galley(at, g, ink);
}

/// A group box at its template rectangle (VM, Windows 11): a square (220)
/// frame at the left and right − 1, top + 6 and bottom − 2; the title from
/// left + 9, the top line broken from left + 7 to 2 px past it.
pub fn win_group(ui: &egui::Ui, r: Rect, title: &str) {
    win_group_en(ui, r, title, true);
}

/// `win_group` with its title greyed when disabled.
pub fn win_group_en(ui: &egui::Ui, r: Rect, title: &str, enabled: bool) {
    let p = ui.painter();
    let line = Stroke::new(1.0f32, Color32::from_gray(220));
    let (x0, x1, y0, y1) = (r.left() + 0.5, r.right() - 0.5, r.top() + 6.5, r.bottom() - 1.5);
    p.vline(x0, y0..=y1, line);
    p.vline(x1, y0..=y1, line);
    p.hline(x0..=x1, y1, line);
    if title.is_empty() {
        p.hline(x0..=x1, y0, line);
    } else {
        let ink = if enabled { Color32::BLACK } else { Color32::from_gray(109) };
        let t = p.text(Pos2::new(r.left() + 9.0, r.top()), egui::Align2::LEFT_TOP, title, DLG_FONT, ink);
        p.hline(x0..=r.left() + 7.0, y0, line);
        p.hline(t.right() + 2.0..=x1, y0, line);
    }
}

/// An auto radio button at its template rectangle (VM, Windows 11): a
/// 13 px disc centred in the row — checked (0, 95, 184) with a 5 px white
/// dot, else a (98) ring on (243) — the text from 16 px in. Returns the
/// click.
pub fn win_radio(ui: &mut egui::Ui, r: Rect, text: &str, checked: bool) -> bool {
    win_radio_en(ui, r, text, checked, true)
}

/// `win_radio`, disabled: paler, its text (109), deaf to clicks (the
/// disabled look approximated).
pub fn win_radio_en(ui: &mut egui::Ui, r: Rect, text: &str, checked: bool, enabled: bool) -> bool {
    let resp = ui.interact(r, ui.id().with(("fv_radio", text, r.min.x as i32, r.min.y as i32)), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let c = Pos2::new(r.left() + 6.5, r.top() + (r.height() / 2.0).floor());
    let p = ui.painter();
    match (checked, enabled) {
        (true, true) => {
            p.circle_filled(c, 6.5, Color32::from_rgb(0, 95, 184));
            p.circle_filled(c, 3.0, Color32::WHITE);
        }
        (true, false) => {
            p.circle_filled(c, 6.5, Color32::from_gray(160));
            p.circle_filled(c, 3.0, Color32::WHITE);
        }
        (false, _) => {
            p.circle_filled(c, 6.5, Color32::from_gray(if enabled { 98 } else { 191 }));
            p.circle_filled(c, 5.5, if resp.hovered() && enabled { Color32::from_gray(233) } else { Color32::from_gray(243) });
        }
    }
    let ink = if enabled { Color32::BLACK } else { Color32::from_gray(109) };
    p.text(Pos2::new(r.left() + 16.0, c.y), egui::Align2::LEFT_CENTER, text, DLG_FONT, ink);
    resp.clicked()
}

/// The edit took the focus from the keyboard (Tab / Shift+Tab) this pass.
fn tabbed_in(ui: &egui::Ui, r: &egui::Response) -> bool {
    r.gained_focus() && ui.input(|i| i.key_pressed(egui::Key::Tab))
}

/// An edit's selected text in the highlight's ink, white (Win32's
/// COLOR_HIGHLIGHTTEXT; egui fills only the selection's background).
fn selected_white(p: &egui::Painter, out: &egui::text_edit::TextEditOutput) {
    let Some(cr) = out.cursor_range.filter(|c| !c.is_empty()) else { return };
    let [a, b] = cr.sorted_cursors();
    let (a, b) = (a.rcursor, b.rcursor);
    for ri in a.row..=b.row.min(out.galley.rows.len().saturating_sub(1)) {
        let row = &out.galley.rows[ri];
        let l = if ri == a.row { row.x_offset(a.column) } else { row.rect.left() };
        let r = if ri == b.row { row.x_offset(b.column) } else { row.rect.right() };
        let rect = Rect::from_min_max(Pos2::new(l, row.min_y()), Pos2::new(r, row.max_y())).translate(out.galley_pos.to_vec2());
        p.with_clip_rect(rect.intersect(out.text_clip_rect)).galley_with_override_text_color(out.galley_pos, out.galley.clone(), Color32::WHITE);
    }
}

/// A single-line edit at its template rectangle (VM, Windows 11): white,
/// a (236) line along its top and left, the bottom 2 px of (0, 103, 192)
/// while it has the focus, else 1 px (131); the selection white on
/// (0, 120, 212) from 6 px in. `focus` focuses it with its text selected
/// (as a dialog's first edit opens, or after a refused value's message).
pub fn win_edit(ui: &mut egui::Ui, r: Rect, id: egui::Id, text: &mut String, focus: bool) -> egui::Response {
    let painter = ui.painter().clone();
    let frame = painter.add(egui::Shape::Noop);
    let inner = Rect::from_min_max(r.min + Vec2::new(5.0, 2.0), r.max - Vec2::new(2.0, 2.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    child.visuals_mut().selection.bg_fill = Color32::from_rgb(0, 120, 212);
    child.visuals_mut().selection.stroke = Stroke::new(1.0f32, Color32::WHITE);
    let mut out = egui::TextEdit::singleline(text).id(id).frame(false).font(DLG_FONT).margin(egui::Margin::symmetric(1.0, 1.0)).desired_width(inner.width() - 2.0).show(&mut child);
    selected_white(child.painter(), &out);
    // Focused by the dialog, or reached with Tab (the dialog manager's
    // EM_SETSEL for DLGC_HASSETSEL; VM: Tab into «Число гармоник» then a
    // digit replaces its text): the text selected.
    if focus || tabbed_in(ui, &out.response) {
        out.response.request_focus();
        let n = text.chars().count();
        out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
        out.state.store(ui.ctx(), out.response.id);
    }
    let focused = out.response.has_focus() || focus;
    let line = |y: f32, h: f32, c: Color32| egui::Shape::rect_filled(Rect::from_min_max(Pos2::new(r.left(), y), Pos2::new(r.right(), y + h)), 0.0, c);
    painter.set(
        frame,
        egui::Shape::Vec(vec![
            egui::Shape::rect_filled(r, 0.0, Color32::WHITE),
            line(r.top(), 1.0, Color32::from_gray(236)),
            egui::Shape::rect_filled(Rect::from_min_max(r.min, Pos2::new(r.left() + 1.0, r.bottom())), 0.0, Color32::from_gray(236)),
            if focused { line(r.bottom() - 2.0, 2.0, Color32::from_rgb(0, 103, 192)) } else { line(r.bottom() - 1.0, 1.0, Color32::from_gray(131)) },
        ]),
    );
    out.response
}

/// `win_edit`, or when disabled its text in (109) on (250) in a (234)
/// frame, not editable (the disabled look approximated).
pub fn win_edit_en(ui: &mut egui::Ui, r: Rect, id: egui::Id, text: &mut String, focus: bool, enabled: bool) {
    if enabled {
        win_edit(ui, r, id, text, focus);
        return;
    }
    let p = ui.painter();
    p.rect_filled(r, 0.0, Color32::from_gray(234));
    p.rect_filled(r.shrink(1.0), 0.0, Color32::from_gray(250));
    p.with_clip_rect(r.shrink(1.0)).text(Pos2::new(r.left() + 6.0, r.center().y), egui::Align2::LEFT_CENTER, text.as_str(), DLG_FONT, Color32::from_gray(109));
}

/// A multi-line edit at its template rectangle (VM, Windows 11): white in a
/// 1 px (236) frame, the bottom line 2 px of (0, 103, 192) while it has the
/// focus, else 1 px (134). `focus` gives it the focus.
pub fn win_edit_multiline(ui: &mut egui::Ui, r: Rect, id: egui::Id, text: &mut String, focus: bool) -> egui::Response {
    let painter = ui.painter().clone();
    let frame = painter.add(egui::Shape::Noop);
    let inner = Rect::from_min_max(r.min + Vec2::new(4.0, 3.0), r.max - Vec2::new(2.0, 3.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    let out = egui::ScrollArea::vertical()
        .id_salt(id.with("scroll"))
        .max_height(inner.height())
        .show(&mut child, |ui| {
            ui.visuals_mut().selection.bg_fill = Color32::from_rgb(0, 120, 212);
            ui.visuals_mut().selection.stroke = Stroke::new(1.0f32, Color32::WHITE);
            let mut out = egui::TextEdit::multiline(text).id(id).frame(false).font(DLG_FONT).margin(egui::Margin::ZERO).desired_width(inner.width() - 16.0).desired_rows(1).min_size(inner.size() - Vec2::new(16.0, 0.0)).show(ui);
            selected_white(ui.painter(), &out);
            // Focused as the dialog's first tab stop, its text selected
            // (VM: the comment reopened shows all of it selected).
            if focus || tabbed_in(ui, &out.response) {
                out.response.request_focus();
                let n = text.chars().count();
                out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
                out.state.store(ui.ctx(), id);
            }
            out.response
        });
    let resp = out.inner;
    let focused = resp.has_focus();
    painter.set(
        frame,
        egui::Shape::Vec(vec![
            egui::Shape::rect_filled(r, 0.0, Color32::from_gray(236)),
            egui::Shape::rect_filled(r.shrink(1.0), 0.0, Color32::WHITE),
            if focused {
                egui::Shape::rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 2.0), r.max), 0.0, Color32::from_rgb(0, 103, 192))
            } else {
                egui::Shape::rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 1.0), r.max), 0.0, Color32::from_gray(134))
            },
        ]),
    );
    resp
}

/// DIALOG 332 «Комментарии к диаграмме»'s body on its 379 × 307 DLU
/// template (569 × 499 px): the edit at 6, 6 (366 × 276), OK (the default)
/// and «Отмена» at 244 and 310, 288 (62 × 14) — the code-page combo
/// (1087, at 6, 288) stays hidden. The edit, the first tab stop, has the
/// focus whenever no other control has (a dialog always has one).
/// Returns the button pressed.
pub fn comment_dialog_body(ui: &mut egui::Ui, id: egui::Id, text: &mut String) -> Option<bool> {
    let (client, _) = ui.allocate_exact_size(Vec2::new(569.0, 499.0), egui::Sense::hover());
    let focus = ui.memory(|m| m.focused().is_none());
    win_edit_multiline(ui, dlu(client, 6, 6, 366, 276), id, text, focus);
    if win_button(ui, dlu(client, 244, 288, 62, 14), "OK", true).clicked() {
        return Some(true);
    }
    // Esc is Отмена (VM); Enter stays the edit's new line (ES_WANTRETURN).
    if win_button(ui, dlu(client, 310, 288, 62, 14), "Отмена", false).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Some(false);
    }
    None
}

/// `text` cut to `width` with «...» at its end (a grid cell's
/// DT_END_ELLIPSIS).
pub fn ellipsize(ui: &egui::Ui, text: &str, font: egui::FontId, width: f32) -> String {
    let w = |s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_owned(), font.clone(), Color32::BLACK).size().x);
    let chars: Vec<char> = text.chars().collect();
    let mut n = chars.len();
    while n > 0 && w(&format!("{}...", chars[..n].iter().collect::<String>())) > width {
        n -= 1;
    }
    format!("{}...", chars[..n].iter().collect::<String>())
}

/// An auto check box at its template rectangle (VM, Windows 11): a 13 px
/// box rounded 2 px, centred in the row — checked (0, 95, 184) with a thin
/// white tick, else a (98) frame on (243) — the text from 16 px in.
/// Returns the click.
pub fn win_check(ui: &mut egui::Ui, r: Rect, text: &str, checked: bool) -> bool {
    win_check_ex(ui, r, text, checked, true, false)
}

/// `win_check`, greyed when disabled (the look approximated) and, with
/// BS_LEFTTEXT (`left`), the box at the control's right end and the text
/// right-aligned before it.
pub fn win_check_ex(ui: &mut egui::Ui, r: Rect, text: &str, checked: bool, enabled: bool, left: bool) -> bool {
    let resp = ui.interact(r, ui.id().with(("fv_check", text, r.min.x as i32, r.min.y as i32)), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let bx = if left { r.right() - 13.0 } else { r.left() };
    let b = Rect::from_min_size(Pos2::new(bx, r.top() + ((r.height() - 13.0) / 2.0).floor()), Vec2::splat(13.0));
    let p = ui.painter();
    match (checked, enabled) {
        (true, _) => {
            p.rect_filled(b, 2.5, if enabled { Color32::from_rgb(0, 95, 184) } else { Color32::from_gray(160) });
            let o = b.min;
            p.add(egui::Shape::line(vec![o + Vec2::new(3.5, 6.5), o + Vec2::new(5.5, 8.5), o + Vec2::new(9.5, 4.5)], Stroke::new(1.0f32, Color32::WHITE)));
        }
        (false, _) => {
            p.rect_filled(b, 2.5, Color32::from_gray(if enabled { 98 } else { 191 }));
            p.rect_filled(b.shrink(1.0), 1.5, if resp.hovered() && enabled { Color32::from_gray(233) } else { Color32::from_gray(243) });
        }
    }
    let ink = if enabled { Color32::BLACK } else { Color32::from_gray(109) };
    if left {
        // BS_LEFTTEXT: the text right-aligned 3 px before the box (VM).
        p.text(Pos2::new(b.left() - 3.0, b.center().y), egui::Align2::RIGHT_CENTER, text, DLG_FONT, ink);
    } else {
        p.text(Pos2::new(r.left() + 16.0, b.center().y), egui::Align2::LEFT_CENTER, text, DLG_FONT, ink);
    }
    resp.clicked() && enabled
}

/// One visible row of a TreeView.
pub struct TreeRow {
    pub depth: usize,
    pub label: String,
    pub has_children: bool,
    pub expanded: bool,
    pub selected: bool,
}

/// What the pointer did to a TreeView's rows this frame.
#[derive(Default)]
pub struct TreeEvents {
    pub clicked: Option<usize>,
    pub double_clicked: Option<usize>,
    pub toggled: Option<usize>,
    /// Each row's image-and-label rect.
    pub rects: Vec<Rect>,
}

/// A TreeView (VM, Windows 11): 16 px rows indented 19 px a level; with
/// TVS_LINESATROOT a row's line at 11 + 19·depth from the inner left, its
/// image's 16 px cell at 22 + 19·depth and its label 19 px after it (one
/// level less without it: the roots then have no line nor box); dotted
/// (109) lines on every other pixel — the vertical ones on even rows, a
/// row's branch from its line to its image along its middle — continued
/// for the ancestors with later siblings; the ± box 9 px on the line, a
/// (145) frame with (186) corners on (250) shading to (227), the sign 5 px
/// of (75, 99, 167); the selected label white on (0, 120, 212). `icon`
/// draws row k's image into its cell; `top_row` scrolls row k to the top
/// this frame. Without the focus (`start_focused` until a press inside,
/// lost to a press outside) the selection is (240) behind black text, as
/// TVS_SHOWSELALWAYS draws it (VM: the finder's tree on opening).
/// `sel_always` (TVS_SHOWSELALWAYS): the selection stays marked, grey,
/// without the focus; else it is not marked then.
#[allow(clippy::too_many_arguments)]
pub fn win_tree(ui: &mut egui::Ui, r: Rect, id: egui::Id, border: Color32, rows: &[TreeRow], lines_at_root: bool, top_row: Option<usize>, start_focused: bool, sel_always: bool, icon: &mut dyn FnMut(&egui::Painter, Rect, usize)) -> TreeEvents {
    let mut ev = TreeEvents::default();
    let focus_id = id.with("fv_tree_focus");
    let stored = ui.data(|d| d.get_temp::<bool>(focus_id));
    let mut focused = stored.unwrap_or(start_focused);
    // A press elsewhere takes the focus — not the one that opened the
    // dialog, in the frame the tree first shows.
    if let Some(p) = ui.input(|i| i.pointer.any_pressed().then(|| crate::lanes::press_pos(i)).flatten()).filter(|_| stored.is_some()) {
        focused = r.contains(p);
    }
    ui.data_mut(|d| d.insert_temp(focus_id, focused));
    let p = ui.painter().clone();
    p.rect_filled(r, 0.0, border);
    let inner = r.shrink(1.0);
    p.rect_filled(inner, 0.0, Color32::WHITE);
    // Whether row i has a later sibling (a row of its depth before any
    // shallower one).
    let more: Vec<bool> = (0..rows.len())
        .map(|i| {
            let d = rows[i].depth;
            rows[i + 1..].iter().take_while(|r| r.depth >= d).any(|r| r.depth == d)
        })
        .collect();
    let dot = Color32::from_gray(109);
    // Scrolled by whole rows: the window scroll bar (Windows 11's, as the
    // view's) whenever the rows overflow, the wheel three rows a notch.
    let scroll_id = id.with("fv_tree_scroll");
    let content_h = 16.0 * rows.len() as f32;
    let page = (inner.height() / 16.0).floor() * 16.0;
    let max_off = (content_h - page).max(0.0);
    let mut off: f32 = ui.data(|d| d.get_temp(scroll_id)).unwrap_or(0.0);
    if let Some(k) = top_row {
        off = 16.0 * k as f32;
    }
    if ui.rect_contains_pointer(inner) {
        // A notch each (egui-winit merges a frame's notches).
        for dy in ui.input(|i| crate::lanes::wheel_deltas(&i.events)) {
            off -= dy.signum() * 48.0;
        }
    }
    let content = if max_off > 0.0 { Rect::from_min_max(inner.min, Pos2::new(inner.right() - 17.0, inner.bottom())) } else { inner };
    if max_off > 0.0 {
        let bar = Rect::from_min_max(Pos2::new(content.right(), inner.top()), inner.max);
        if let Some(v) = crate::lanes::scrollbar(ui, bar, false, off, max_off, page, 16.0, page) {
            off = v;
        }
    }
    off = ((off / 16.0).round() * 16.0).clamp(0.0, max_off);
    ui.data_mut(|d| d.insert_temp(scroll_id, off));
    {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(content));
        child.set_clip_rect(content.intersect(ui.clip_rect()));
        let ui = &mut child;
        {
            let all = Rect::from_min_size(Pos2::new(content.left(), content.top() - off), Vec2::new(content.width(), content_h));
            let p = ui.painter().clone();
            let shift = if lines_at_root { 0.0 } else { -19.0 };
            let line_x = |d: usize| all.left() + shift + 11.0 + 19.0 * d as f32;
            let vdots = |x: f32, y0: f32, y1: f32| {
                let mut y = y0;
                while y < y1 {
                    p.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::splat(1.0)), 0.0, dot);
                    y += 2.0;
                }
            };
            let mut stack: Vec<usize> = Vec::new();
            for (i, row) in rows.iter().enumerate() {
                let top = all.top() + 16.0 * i as f32;
                let mid = top + 8.0;
                while stack.last().is_some_and(|&a| rows[a].depth >= row.depth) {
                    stack.pop();
                }
                let lined = lines_at_root || row.depth > 0;
                // The ancestors' lines through this row.
                for &a in &stack {
                    if more[a] && (lines_at_root || rows[a].depth > 0) {
                        vdots(line_x(rows[a].depth), top, top + 16.0);
                    }
                }
                let x = line_x(row.depth);
                if lined {
                    let first_root = i == 0;
                    vdots(x, if first_root { mid } else { top }, if more[i] { top + 16.0 } else { mid + 1.0 });
                    let mut hx = x + 2.0;
                    while hx < x + 11.0 {
                        p.rect_filled(Rect::from_min_size(Pos2::new(hx, mid), Vec2::splat(1.0)), 0.0, dot);
                        hx += 2.0;
                    }
                    if row.has_children {
                        let b = Rect::from_min_size(Pos2::new(x - 4.0, mid - 4.0), Vec2::splat(9.0));
                        p.rect_filled(b, 0.0, Color32::from_gray(145));
                        for (y0, n, c) in [(1.0, 3.0, Color32::from_rgb(250, 251, 251)), (4.0, 2.0, Color32::from_rgb(237, 237, 236)), (6.0, 2.0, Color32::from_gray(227))] {
                            p.rect_filled(Rect::from_min_size(b.min + Vec2::new(1.0, y0), Vec2::new(7.0, n)), 0.0, c);
                        }
                        for c in [b.left_top(), b.right_top() - Vec2::new(1.0, 0.0), b.left_bottom() - Vec2::new(0.0, 1.0), b.right_bottom() - Vec2::splat(1.0)] {
                            p.rect_filled(Rect::from_min_size(c, Vec2::splat(1.0)), 0.0, Color32::from_rgb(186, 187, 188));
                        }
                        let sign = Color32::from_rgb(75, 99, 167);
                        p.rect_filled(Rect::from_min_size(b.min + Vec2::new(2.0, 4.0), Vec2::new(5.0, 1.0)), 0.0, sign);
                        if !row.expanded {
                            p.rect_filled(Rect::from_min_size(b.min + Vec2::new(4.0, 2.0), Vec2::new(1.0, 5.0)), 0.0, sign);
                        }
                        if ui.interact(b.expand(2.0), id.with(("box", i)), egui::Sense::click()).clicked() {
                            ev.toggled = Some(i);
                        }
                    }
                }
                let cell = Rect::from_min_size(Pos2::new(x + 11.0, top), Vec2::splat(16.0));
                icon(&p, cell, i);
                let g = ui.fonts(|f| f.layout_no_wrap(row.label.clone(), DLG_FONT, Color32::BLACK));
                let lr = Rect::from_min_size(Pos2::new(cell.left() + 19.0, top), Vec2::new(g.size().x + 5.0, 16.0));
                if row.selected && (focused || sel_always) {
                    p.rect_filled(lr, 0.0, if focused { Color32::from_rgb(0, 120, 212) } else { Color32::from_gray(240) });
                }
                p.galley(Pos2::new(lr.left() + 2.0, (mid - g.size().y / 2.0).round()), g, if row.selected && focused { Color32::WHITE } else { Color32::BLACK });
                ev.rects.push(Rect::from_min_max(cell.min, lr.max));
                let resp = ui.interact(Rect::from_min_max(cell.min, lr.max), id.with(("item", i)), egui::Sense::click());
                if resp.clicked() {
                    ev.clicked = Some(i);
                }
                if resp.double_clicked() {
                    ev.double_clicked = Some(i);
                }
                stack.push(i);
            }
        }
    }
    ev
}

/// A progress bar (msctls_progress32, Windows 11 theme; not measured —
/// none stayed up long enough on the VM): a (188) frame on (230), the done
/// part (6, 176, 37).
pub fn win_progress(ui: &egui::Ui, r: Rect, fraction: f32) {
    let p = ui.painter();
    p.rect_filled(r, 0.0, Color32::from_gray(188));
    let inner = r.shrink(1.0);
    p.rect_filled(inner, 0.0, Color32::from_gray(230));
    let w = (inner.width() * fraction.clamp(0.0, 1.0)).round();
    p.rect_filled(Rect::from_min_size(inner.min, Vec2::new(w, inner.height())), 0.0, Color32::from_rgb(6, 176, 37));
}

/// A combo box's closed height (Windows for the template font, VM).
pub const COMBO_H: f32 = 21.0;

/// The look of a closed combo box (VM, Windows 11): a drop-down list is a
/// push-button face, (253) in a rounded (210) → (188) border, square (250)
/// in (234) when disabled; an editable one (CBS_DROPDOWN) is a white field
/// in a square (141) border. While its list is open: (204, 228, 247) in a
/// square (0, 95, 183) border. The chevron: a 1 px (111) V from 13 to 6 px
/// left of the right edge, rows 9–12.
fn combo_face(ui: &egui::Ui, r: Rect, edit: bool, enabled: bool, open: bool, hover: bool) {
    let p = ui.painter();
    if open {
        p.rect_filled(r, 0.0, Color32::from_rgb(0, 95, 183));
        p.rect_filled(r.shrink(1.0), 0.0, Color32::from_rgb(204, 228, 247));
    } else if !enabled {
        p.rect_filled(r, 0.0, Color32::from_gray(234));
        p.rect_filled(r.shrink(1.0), 0.0, Color32::from_gray(250));
    } else if edit {
        p.rect_filled(r, 0.0, if hover { Color32::from_rgb(0, 120, 215) } else { Color32::from_gray(141) });
        p.rect_filled(r.shrink(1.0), 0.0, Color32::WHITE);
    } else {
        let (edge, bottom, fill) = if hover {
            (Color32::from_rgb(0, 120, 212), Color32::from_rgb(0, 108, 191), Color32::from_rgb(224, 238, 249))
        } else {
            (Color32::from_gray(210), Color32::from_gray(188), Color32::from_gray(253))
        };
        p.rect_filled(r, 4.0, bottom);
        p.rect_filled(Rect::from_min_max(r.min, Pos2::new(r.right(), r.bottom() - 1.0)), 4.0, edge);
        p.rect_filled(r.shrink(1.0), 3.0, fill);
    }
    let ink = if enabled { Color32::from_gray(111) } else { Color32::from_gray(191) };
    let (x, y) = (r.right(), r.top());
    p.add(egui::Shape::line(vec![Pos2::new(x - 12.5, y + 9.0), Pos2::new(x - 9.0, y + 12.5), Pos2::new(x - 5.5, y + 9.0)], Stroke::new(1.0f32, ink)));
}

/// An open drop-down list owns Enter and Esc (a Win32 list's keys, VM:
/// Enter picks the highlighted item and the dialog stays): at a frame's
/// start, while a list was open in the last one, they are taken out of the
/// input before any dialog reads them, and kept for the list.
pub fn route_combo_keys(ctx: &egui::Context) {
    let id = egui::Id::new("fv_combo_popup");
    let pass = ctx.cumulative_pass_nr();
    let open = ctx.data(|d| d.get_temp::<u64>(id)).is_some_and(|f| f + 1 >= pass);
    let keys = if open { ctx.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::Enter), i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))) } else { (false, false) };
    ctx.data_mut(|d| d.insert_temp(id.with("keys"), keys));
}

/// Enter and Esc for an open list: pressed, or routed to it.
fn combo_keys(ctx: &egui::Context) -> (bool, bool) {
    let (enter, esc) = ctx.data(|d| d.get_temp::<(bool, bool)>(egui::Id::new("fv_combo_popup").with("keys"))).unwrap_or_default();
    ctx.input(|i| (enter || i.key_pressed(egui::Key::Enter), esc || i.key_pressed(egui::Key::Escape)))
}

/// A combo box's drop-down list under `r` (VM): its width, a 1 px
/// (0, 120, 215) border, 13 px items, the one under the pointer (at first
/// the current one) white on (0, 120, 212), a soft shadow at the right.
/// Returns the item picked; closes on a pick or a press elsewhere.
pub fn combo_popup(ui: &egui::Ui, r: Rect, id: egui::Id, items: &[String], current: Option<usize>) -> Option<usize> {
    let ctx = ui.ctx().clone();
    let open_id = id.with("open");
    let mut picked = None;
    let list = Rect::from_min_size(Pos2::new(r.left(), r.bottom()), Vec2::new(r.width(), 2.0 + 13.0 * items.len() as f32));
    let area = egui::Area::new(id.with("popup")).order(egui::Order::Foreground).fixed_pos(list.min).show(&ctx, |ui| {
        ui.allocate_exact_size(list.size(), egui::Sense::hover());
        let p = ui.painter();
        for k in 1..=5 {
            let a = [134u8, 161, 200, 227, 237][k - 1];
            p.vline(list.right() + k as f32 - 0.5, list.top() + 3.0..=list.bottom(), Stroke::new(1.0f32, Color32::from_black_alpha(255 - a)));
        }
        p.rect_filled(list, 0.0, Color32::from_rgb(0, 120, 215));
        p.rect_filled(list.shrink(1.0), 0.0, Color32::WHITE);
        let hover = ui.input(|i| i.pointer.interact_pos()).filter(|q| list.contains(*q)).and_then(|q| {
            let k = ((q.y - list.top() - 1.0) / 13.0).floor();
            (k >= 0.0 && (k as usize) < items.len()).then_some(k as usize)
        });
        // The keyboard's item: ↑ / ↓ move it from the current one, Enter
        // picks it (a Win32 list's keys); the pointer's item wins.
        let key_id = id.with("key_hot");
        let mut key_hot: Option<usize> = ui.data(|d| d.get_temp(key_id)).unwrap_or(current);
        let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
        let enter = combo_keys(ui.ctx()).0;
        if !items.is_empty() && (up || down) {
            let k = key_hot.map_or(0, |k| if down { (k + 1).min(items.len() - 1) } else { k.saturating_sub(1) });
            key_hot = Some(k);
        }
        if hover.is_some() {
            key_hot = hover;
        }
        ui.data_mut(|d| d.insert_temp(key_id, key_hot));
        if enter {
            picked = key_hot;
        }
        let hot = key_hot;
        // Long items are cut at the list's edge.
        let text_p = p.with_clip_rect(list.shrink(1.0));
        for (k, t) in items.iter().enumerate() {
            let row = Rect::from_min_size(Pos2::new(list.left() + 1.0, list.top() + 1.0 + 13.0 * k as f32), Vec2::new(list.width() - 2.0, 13.0));
            let ink = if hot == Some(k) {
                p.rect_filled(row, 0.0, Color32::from_rgb(0, 120, 212));
                Color32::WHITE
            } else {
                Color32::BLACK
            };
            text_p.text(row.left_center() + Vec2::new(2.0, 0.0), egui::Align2::LEFT_CENTER, t, DLG_FONT, ink);
        }
        if ui.input(|i| i.pointer.primary_released()) && hover.is_some() {
            picked = hover;
        }
    });
    let outside = ctx.input(|i| i.pointer.any_pressed()) && !area.response.rect.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default()) && !r.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default());
    if picked.is_some() || outside || combo_keys(&ctx).1 {
        ctx.data_mut(|d| {
            d.insert_temp(open_id, false);
            d.remove::<Option<usize>>(id.with("key_hot"));
        });
    } else {
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("fv_combo_popup"), pass));
    }
    picked
}

/// A drop-down list combo (CBS_DROPDOWNLIST) at its closed rectangle:
/// returns the item picked from its list.
pub fn win_combo_list(ui: &mut egui::Ui, r: Rect, id: egui::Id, items: &[String], current: usize, enabled: bool) -> Option<usize> {
    win_combo_list_in(ui, r, id, items, current, enabled, DLG_FONT)
}

/// [`win_combo_list`] with its text in `font` (a control bar's 9 pt UI
/// font); `current` past the items shows an empty field.
pub fn win_combo_list_in(ui: &mut egui::Ui, r: Rect, id: egui::Id, items: &[String], current: usize, enabled: bool, font: egui::FontId) -> Option<usize> {
    let r = Rect::from_min_size(r.min, Vec2::new(r.width(), COMBO_H));
    let resp = ui.interact(r, id, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let open_id = id.with("open");
    let mut open = enabled && ui.data(|d| d.get_temp(open_id)).unwrap_or(false);
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(open_id, open));
    }
    combo_face(ui, r, false, enabled, open, resp.hovered() && enabled);
    let ink = if enabled { Color32::BLACK } else { Color32::from_gray(109) };
    let text_r = Rect::from_min_max(r.min + Vec2::new(4.0, 0.0), Pos2::new(r.right() - 17.0, r.bottom()));
    if let Some(t) = items.get(current) {
        ui.painter().with_clip_rect(text_r).text(text_r.left_center(), egui::Align2::LEFT_CENTER, t, font, ink);
    }
    if open {
        return combo_popup(ui, r, id, items, (current < items.len()).then_some(current));
    }
    None
}

/// An editable combo (CBS_DROPDOWN) at its closed rectangle: the edit over
/// the field (`focus` selects its text and focuses it), the button at the
/// right opening the list; returns the item picked.
pub fn win_combo_edit(ui: &mut egui::Ui, r: Rect, id: egui::Id, text: &mut String, items: &[String], focus: bool) -> Option<usize> {
    let r = Rect::from_min_size(r.min, Vec2::new(r.width(), COMBO_H));
    let button = Rect::from_min_max(Pos2::new(r.right() - 17.0, r.top()), r.max);
    let resp = ui.interact(button, id, egui::Sense::click());
    let open_id = id.with("open");
    let mut open = ui.data(|d| d.get_temp(open_id)).unwrap_or(false);
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(open_id, open));
    }
    let hover = ui.rect_contains_pointer(r);
    combo_face(ui, r, true, true, open, hover);
    let field = Rect::from_min_max(r.min + Vec2::new(3.0, 3.0), Pos2::new(button.left(), r.bottom() - 3.0));
    let mut out = egui::TextEdit::singleline(text).frame(false).font(DLG_FONT).margin(egui::Margin::symmetric(1.0, 0.0)).desired_width(field.width()).show(&mut ui.new_child(egui::UiBuilder::new().max_rect(field)));
    if focus {
        out.response.request_focus();
        let n = text.chars().count();
        out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
        out.state.store(ui.ctx(), out.response.id);
    }
    if open {
        let cur = items.iter().position(|t| t == text);
        if let Some(k) = combo_popup(ui, r, id, items, cur) {
            *text = items[k].clone();
            return Some(k);
        }
    }
    None
}

/// A caption bar's close button (CMFCCaptionBar in message-bar mode,
/// mfc `0x1006fd4c`): 17 × 17 at (the bar's right − 20, the box's top), its
/// cross, the hot look only under the pointer. Returns a click.
pub fn caption_close(ui: &mut egui::Ui, bar: Rect, id: egui::Id) -> bool {
    let r = Rect::from_min_size(Pos2::new(bar.right() - 20.0, bar.top()), Vec2::splat(17.0));
    let resp = ui.interact(r, id, egui::Sense::click());
    if resp.hovered() {
        crate::popmenu::hot_bar(ui.painter(), r);
    }
    // The glyph (VM, Office 2007 Blue): an 8 × 7 cross of 2 px steps in
    // (86, 125, 177), 4 px in and 5 down.
    let o = r.min + Vec2::new(4.0, 5.0);
    let ink = Color32::from_rgb(86, 125, 177);
    let run = |x0: f32, x1: f32, y: f32| ui.painter().rect_filled(Rect::from_min_max(o + Vec2::new(x0, y), o + Vec2::new(x1 + 1.0, y + 1.0)), 0.0, ink);
    for (y, a, b) in [(0.0, 0.0, 6.0), (1.0, 1.0, 5.0), (5.0, 1.0, 5.0), (6.0, 0.0, 6.0)] {
        run(a, a + 1.0, y);
        run(b, b + 1.0, y);
    }
    run(2.0, 5.0, 2.0);
    run(3.0, 4.0, 3.0);
    run(2.0, 5.0, 4.0);
    resp.clicked()
}

/// A transparent full-screen layer at `order` that swallows the pointer
/// below a modal window (the windows of a higher order stay live).
pub fn modal_backdrop(ctx: &egui::Context, id: &str, order: egui::Order) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("fv_modal_pass"), pass));
    backdrop(ctx, id, order);
}

/// The layer of [`modal_backdrop`] alone (the frame's own, while a modal
/// window is up: not itself a reason to be modal).
pub fn backdrop(ctx: &egui::Context, id: &str, order: egui::Order) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new(id)).order(order).fixed_pos(screen.min).show(ctx, |ui| {
        ui.allocate_exact_size(screen.size(), egui::Sense::click_and_drag());
    });
}

/// The icon of a message box: `AfxMessageBox` adds the exclamation to
/// MB_OK / MB_OKCANCEL and the question to MB_YESNO(CANCEL) when the call
/// gives none; MB_ICONHAND (0x10) is the error mark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MsgIcon {
    #[default]
    Exclamation,
    Error,
    Question,
}

/// A message box's client (VM, Windows 11's MessageBox): a white area —
/// the 32 px icon at (21, 23) (our drawing of the system icons), the text
/// from x 61 centred on the icon's middle, 31 px clear at the right, at
/// least 23 px above and below — over a 42 px (240) strip with the buttons
/// (75 × 23, 10 px down, 10 px apart, the last 15 px from the right edge);
/// the first is the default one, with the focus rectangle. Returns the
/// button pressed.
pub fn message_box(ui: &mut egui::Ui, icon: MsgIcon, text: &str, buttons: &[&str]) -> Option<usize> {
    let font = egui::FontId::proportional(12.0);
    let g = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font.clone(), Color32::BLACK));
    let n = buttons.len() as f32;
    let w = (61.0 + g.size().x + 31.0).max(15.0 + 75.0 * n + 10.0 * (n - 1.0) + 15.0).ceil();
    let white_h = (23.0 + 32.0 + 23.0f32).max(g.size().y + 46.0).ceil();
    let (client, _) = ui.allocate_exact_size(Vec2::new(w, white_h + 42.0), egui::Sense::hover());
    let white = Rect::from_min_size(client.min, Vec2::new(w, white_h));
    ui.painter().rect_filled(white, 0.0, Color32::WHITE);
    let cell = Rect::from_min_size(white.min + Vec2::new(21.0, 23.0), Vec2::splat(32.0));
    message_icon(ui.painter(), cell, icon);
    let mid = if g.size().y > 32.0 { white.center().y } else { cell.center().y };
    ui.painter().galley(Pos2::new(white.left() + 61.0, (mid - g.size().y / 2.0).round()), g, Color32::BLACK);
    let mut pressed = None;
    for (k, label) in buttons.iter().enumerate() {
        let x = client.right() - 15.0 - 75.0 * (n - k as f32) - 10.0 * (n - 1.0 - k as f32);
        let r = Rect::from_min_size(Pos2::new(x, white.bottom() + 10.0), Vec2::new(75.0, 23.0));
        if win_button(ui, r, label, k == 0).clicked() {
            pressed = Some(k);
        }
        if k == 0 {
            focus_rect(ui.painter(), Rect::from_min_max(r.min + Vec2::new(4.0, 3.0), r.max - Vec2::new(4.0, 3.0)));
        }
    }
    pressed
}

/// The focus rectangle: a dotted (13) frame, every other pixel (VM).
pub fn focus_rect(p: &egui::Painter, r: Rect) {
    let ink = Color32::from_gray(13);
    let dot = |x: f32, y: f32| p.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::splat(1.0)), 0.0, ink);
    let mut x = r.left();
    while x < r.right() {
        dot(x, r.top());
        dot(x, r.bottom() - 1.0);
        x += 2.0;
    }
    let mut y = r.top() + 2.0;
    while y < r.bottom() - 1.0 {
        dot(r.left(), y);
        dot(r.right() - 1.0, y);
        y += 2.0;
    }
}

/// Our drawing of a system message icon in its 32 px cell.
fn message_icon(p: &egui::Painter, r: Rect, icon: MsgIcon) {
    let c = r.center();
    let (mark, bg) = match icon {
        MsgIcon::Exclamation => {
            let tri = vec![Pos2::new(c.x, r.top() + 2.0), Pos2::new(r.right() - 2.0, r.bottom() - 3.0), Pos2::new(r.left() + 2.0, r.bottom() - 3.0)];
            p.add(egui::Shape::convex_polygon(tri, Color32::from_rgb(252, 204, 34), Stroke::new(1.0f32, Color32::from_rgb(190, 140, 0))));
            ("!", None)
        }
        MsgIcon::Error => ("✕", Some(Color32::from_rgb(214, 40, 40))),
        MsgIcon::Question => ("?", Some(Color32::from_rgb(40, 100, 200))),
    };
    if let Some(bg) = bg {
        p.circle_filled(c, 15.0, bg);
    }
    let color = if bg.is_some() { Color32::WHITE } else { Color32::BLACK };
    let at = if bg.is_some() { c } else { c + Vec2::new(0.0, 4.0) };
    p.text(at, egui::Align2::CENTER_CENTER, mark, egui::FontId::proportional(20.0), color);
}

/// Whether a dialog window (egui's `Order::Middle` layer, where every
/// `egui::Window` of ours lives or puts its backdrop) was shown last frame.
pub fn window_up(ctx: &egui::Context) -> bool {
    // A backdrop shown in the last pass counts too, whatever its order (a
    // message box over a dialog sits above the Middle windows).
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<u64>(egui::Id::new("fv_modal_pass"))).is_some_and(|p| p + 1 >= pass)
        || ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Middle))
}

impl App {
    pub fn dialogs_ui(&mut self, ctx: &egui::Context) {
        self.about_window(ctx);
        self.windows_dialog(ctx);
        self.comment_window(ctx);
        self.channel_map_window(ctx);
        self.color_window(ctx);
        self.finder_window(ctx);
        self.loading_window(ctx);
        self.omp_windows(ctx);
        self.record_dialogs_ui(ctx);
        self.quick_sheet_window(ctx);
        self.formula_dlg_window(ctx);
        self.save_prompt_window(ctx);
    }

    /// «Загрузка» (DIALOG 492): an indeterminate progress bar per large file
    /// being read, and «Отмена», which abandons that open.
    fn loading_window(&mut self, ctx: &egui::Context) {
        self.poll_loading();
        if self.loading.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        let now = ctx.input(|i| i.time);
        let mut cancel = None;
        // DIALOG 492: a progress bar 0..100 and «Отмена», no close box, no
        // text; the bar is set every 500 ms (`0x4717d0`, `0x471870`).
        for (i, l) in self.loading.iter_mut().enumerate().filter(|(_, l)| l.big) {
            if now - l.shown.1 >= 0.5 {
                l.shown = (l.progress.percent(), now);
            }
            // DIALOG 492: 200 × 25 DLU (300 × 41 px), no WS_SYSMENU — no
            // close button; the bar at 5, 7 and «Отмена» at 135, 6.
            crate::dialogs::dlg(ctx, "Загрузка осциллограммы").id(egui::Id::new(("fv_loading", i))).margin(0.0).no_close().collapsible(false).resizable(false).show(ctx, |ui| {
                let (client, _) = ui.allocate_exact_size(Vec2::new(300.0, 41.0), egui::Sense::hover());
                win_progress(ui, dlu(client, 5, 7, 120, 11), l.shown.0 as f32 / 100.0);
                if win_button(ui, dlu(client, 135, 6, 60, 12), "Отмена", false).clicked() {
                    cancel = Some(i);
                }
            });
        }
        // «Отмена» sets the flag the readers poll; the open is abandoned.
        if let Some(i) = cancel {
            self.loading[i].progress.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            self.loading.remove(i);
        }
    }

    /// Channel-role assignment — **our extension**, not a dialog of the
    /// original (whose «Отображение каналов» is the visibility grid, DIALOG
    /// 463, `chandlg`): pick which analog channel plays Ua/Ub/Uc and Ia/Ib/Ic
    /// for our analyses, diagrams and ОМП. Pre-filled from the names.
    fn channel_map_window(&mut self, ctx: &egui::Context) {
        if !self.show_channel_map {
            return;
        }
        let mut open = true;
        crate::dialogs::dlg(ctx, "Роли каналов Ua…Ic (дополнение FastView-REA)")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let Some(d) = self.active.and_then(|a| self.docs.get_mut(a)) else {
                    ui.label("Нет активной осциллограммы.");
                    return;
                };
                let ids: Vec<(usize, String)> = d
                    .osc
                    .analog
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, format!("{} [{}]", c.id, c.unit)))
                    .collect();
                ui.label("Токи");
                let calc_ib = d.analysis.calc_ib;
                egui::Grid::new("imap").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    role_combo(ui, "Ia:", &mut d.analysis.i_abc[0], &ids, true);
                    ui.end_row();
                    role_combo(ui, "Ib:", &mut d.analysis.i_abc[1], &ids, !calc_ib);
                    ui.end_row();
                    role_combo(ui, "Ic:", &mut d.analysis.i_abc[2], &ids, true);
                    ui.end_row();
                });
                ui.checkbox(&mut d.analysis.calc_ib, "Рассчитать Ib по Ia и Iс (сети с изолированной нейтралью)");
                ui.separator();
                ui.label("Напряжения (фазные)");
                egui::Grid::new("umap").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    role_combo(ui, "Ua:", &mut d.analysis.u_abc[0], &ids, true);
                    ui.end_row();
                    role_combo(ui, "Ub:", &mut d.analysis.u_abc[1], &ids, true);
                    ui.end_row();
                    role_combo(ui, "Uc:", &mut d.analysis.u_abc[2], &ids, true);
                    ui.end_row();
                });
                ui.separator();
                ui.label(
                    egui::RichText::new(
                        "Назначение используется во всех расчётах: симметричные составляющие, \
                         сопротивления, векторная/круговая диаграммы, ОМП.",
                    )
                    .small()
                    .weak(),
                );
            });
        self.show_channel_map = open;
    }

    /// 57664 «О программе» (`0x447dc0`; AboutWnd `0x4471e0`, DIALOG 100):
    /// a borderless 630 × 400 modal centred on the main window over its own
    /// picture (BITMAP 479 — the original's asset; ours draws its own title
    /// on the same (51,104,154) ground); white Verdana 14 text at (35,300)
    /// — the version, then the site and support lines —, the two links
    /// underlined in (0,172,134) with a hand cursor, the bottom-right corner
    /// (530,350)–(630,400) closing it; Esc / Enter close it too. The texts
    /// are this program's, not the original's version and contacts.
    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        const BG: egui::Color32 = egui::Color32::from_rgb(51, 104, 154);
        const LINK: egui::Color32 = egui::Color32::from_rgb(0, 172, 134);
        const SITE: &str = "https://github.com/dmytrobsp/fastview-rea";
        const ISSUES: &str = "https://github.com/dmytrobsp/fastview-rea/issues";
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter));
        modal_backdrop(ctx, "fv_about_block", egui::Order::Middle);
        let size = Vec2::new(630.0, 400.0);
        egui::Area::new(egui::Id::new("fv_about"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                let p = ui.painter();
                let o = rect.min;
                p.rect_filled(rect, 0.0, BG);
                // Our own picture: the mark and the title.
                chevron(p, Rect::from_min_size(o + Vec2::new(36.0, 40.0), Vec2::splat(72.0)));
                p.text(o + Vec2::new(128.0, 44.0), egui::Align2::LEFT_TOP, "FastView-REA", egui::FontId::proportional(34.0), egui::Color32::WHITE);
                p.text(o + Vec2::new(130.0, 92.0), egui::Align2::LEFT_TOP, "Просмотр и анализ осциллограмм COMTRADE", egui::FontId::proportional(15.0), egui::Color32::WHITE);
                p.text(
                    o + Vec2::new(130.0, 116.0),
                    egui::Align2::LEFT_TOP,
                    "Независимая совместимая реконструкция Fast View 5 (Rust / egui).\nОригинальный код не используется.",
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_gray(220),
                );
                // The text block at (35,300), 14 px lines.
                let font = egui::FontId::proportional(12.5);
                let line = |k: f32| o + Vec2::new(35.0, 300.0 + 14.0 * k);
                p.text(line(0.0), egui::Align2::LEFT_TOP, format!("Версия {}", env!("CARGO_PKG_VERSION")), font.clone(), egui::Color32::WHITE);
                let link = |k: f32, label: &str, text: &str, url: &str| {
                    let g = p.text(line(k), egui::Align2::LEFT_TOP, label, font.clone(), egui::Color32::WHITE);
                    let at = egui::pos2(g.right() + 4.0, g.top());
                    let r = p.text(at, egui::Align2::LEFT_TOP, text, font.clone(), LINK);
                    p.hline(r.x_range(), r.bottom(), egui::Stroke::new(1.0f32, LINK));
                    let resp = ui.interact(r, ui.id().with(url), egui::Sense::click());
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                    }
                };
                link(1.0, "Сайт проекта :", "github.com/dmytrobsp/fastview-rea", SITE);
                link(2.0, "Обратная связь :", "github.com/dmytrobsp/fastview-rea/issues", ISSUES);
                // The close corner.
                let corner = Rect::from_min_max(o + Vec2::new(530.0, 350.0), o + Vec2::new(630.0, 400.0));
                p.text(corner.center(), egui::Align2::CENTER_CENTER, "Закрыть ×", egui::FontId::proportional(14.0), egui::Color32::WHITE);
                let resp = ui.interact(corner, ui.id().with("fv_about_close"), egui::Sense::click());
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                close |= resp.clicked();
            });
        if close {
            self.show_about = false;
        }
    }

}

/// A labelled channel-role combo bound to an `Option<usize>` slot.
fn role_combo(ui: &mut egui::Ui, label: &str, slot: &mut Option<usize>, ids: &[(usize, String)], enabled: bool) {
    ui.label(label);
    let cur = slot
        .and_then(|i| ids.iter().find(|(j, _)| *j == i))
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| "— нет —".to_string());
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(label).selected_text(cur).width(170.0).show_ui(ui, |ui| {
            ui.selectable_value(slot, None, "— нет —");
            for (i, n) in ids {
                ui.selectable_value(slot, Some(*i), n);
            }
        });
    });
}

/// Draw the Механотроника chevron mark in `rect`.
fn chevron(painter: &egui::Painter, rect: Rect) {
    let h = rect.height();
    let blue = Color32::from_rgb(0x2E, 0x5B, 0x9E);
    let green = Color32::from_rgb(0x4C, 0xAF, 0x50);
    for (yoff, col) in [(0.0_f32, blue), (h * 0.34, green)] {
        let a = Pos2::new(rect.left(), rect.bottom() - yoff - h * 0.12);
        let b = Pos2::new(rect.center().x, rect.top() + yoff + h * 0.06);
        let c = Pos2::new(rect.right(), rect.bottom() - yoff - h * 0.12);
        let w = h * 0.12;
        painter.line_segment([a, b], Stroke::new(w, col));
        painter.line_segment([b, c], Stroke::new(w, col));
    }
}

/// MFC 12's DDX_Text of an int (`0x101e0e79`): up to 64 characters of the
/// window text through `_snwscanf_s` "%d" — leading blanks and a sign,
/// then at least one digit; anything after the number is ignored ("50.5"
/// → 50, "12 Гц" → 12); `None` = «Введите целое число.» (61712).
pub fn ddx_int(s: &str) -> Option<i32> {
    let s: String = s.chars().take(63).collect();
    let t = s.trim_start();
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits: Vec<u32> = rest.chars().map_while(|c| c.to_digit(10)).collect();
    if digits.is_empty() {
        return None;
    }
    // Overflow wraps (INFERRED from the CRT's conversion).
    let v = digits.iter().fold(0i32, |a, &d| a.wrapping_mul(10).wrapping_add(d as i32));
    Some(if neg { v.wrapping_neg() } else { v })
}

#[cfg(test)]
mod ddx_tests {
    #[test]
    fn ddx_int_as_scanf() {
        use super::ddx_int;
        assert_eq!(ddx_int(" 50.5"), Some(50));
        assert_eq!(ddx_int("12 Гц"), Some(12));
        assert_eq!(ddx_int("-7x"), Some(-7));
        assert_eq!(ddx_int("0x10"), Some(0));
        assert_eq!((ddx_int(""), ddx_int("-"), ddx_int("abc")), (None, None, None));
    }
}

#[cfg(test)]
mod combo_tests {
    use super::*;

    /// A dialog moves by its caption only: a drag on the caption moves it
    /// by the drag, a press and drag on its body does not.
    #[test]
    fn dialog_moves_by_its_caption() {
        let ctx = egui::Context::default();
        let mut t = 0.0;
        let mut frame = |events: Vec<egui::Event>| {
            t += 0.05;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time: Some(t), events, ..Default::default() };
            let _ = ctx.run(raw, |ctx| {
                dlg(ctx, "Тест").margin(0.0).show(ctx, |ui| {
                    ui.allocate_exact_size(Vec2::new(200.0, 100.0), egui::Sense::hover());
                });
            });
        };
        for _ in 0..4 {
            frame(vec![]);
        }
        let rect = || ctx.memory(|m| m.area_rect(egui::Id::new("Тест"))).unwrap();
        let r0 = rect();
        let drag = |from: Pos2, by: Vec2| {
            let b = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            vec![vec![egui::Event::PointerMoved(from)], vec![b(from, true)], vec![egui::Event::PointerMoved(from + by * 0.5)], vec![egui::Event::PointerMoved(from + by)], vec![b(from + by, false)], vec![]]
        };
        for evs in drag(r0.center() + Vec2::new(0.0, 20.0), Vec2::new(40.0, 30.0)) {
            frame(evs);
        }
        assert_eq!(rect().min, r0.min, "the body does not drag the window");
        for evs in drag(Pos2::new(r0.left() + 30.0, r0.top() + 12.0), Vec2::new(40.0, 30.0)) {
            frame(evs);
        }
        assert_eq!(rect().min, r0.min + Vec2::new(40.0, 30.0));
    }

    /// A click opens the list under the combo; a click on an item picks it
    /// and closes the list.
    #[test]
    fn drop_down_list_opens_and_picks() {
        let ctx = egui::Context::default();
        let items = vec!["866 MS-DOS".to_string(), "Windows-1251".to_string()];
        let r = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(120.0, COMBO_H));
        let mut picked = None;
        let mut t = 0.0;
        let mut frame = |events: Vec<egui::Event>, picked: &mut Option<usize>| {
            t += 0.05;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time: Some(t), events, ..Default::default() };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(k) = win_combo_list(ui, r, egui::Id::new("cp"), &items, 1, true) {
                        *picked = Some(k);
                    }
                });
            });
        };
        let click = |pos: Pos2| {
            let ev = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            vec![vec![egui::Event::PointerMoved(pos)], vec![ev(true)], vec![ev(false)]]
        };
        frame(vec![], &mut picked);
        for evs in click(r.center()) {
            frame(evs, &mut picked);
        }
        frame(vec![], &mut picked);
        // Item 0: the list's first 13 px row under the combo.
        for evs in click(Pos2::new(r.left() + 20.0, r.bottom() + 1.0 + 6.0)) {
            frame(evs, &mut picked);
        }
        assert_eq!(picked, Some(0));
        frame(vec![], &mut picked);
        assert_eq!(ctx.data(|d| d.get_temp::<bool>(egui::Id::new("cp").with("open"))), Some(false));
    }
}
