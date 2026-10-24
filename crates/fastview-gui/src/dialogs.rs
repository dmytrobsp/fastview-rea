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
    pub fn fixed_size(mut self, size: impl Into<Vec2>) -> Self {
        self.win = self.win.fixed_size(size);
        self
    }
    pub fn default_size(mut self, size: impl Into<Vec2>) -> Self {
        self.win = self.win.default_size(size);
        self
    }
    pub fn interactable(mut self, on: bool) -> Self {
        self.win = self.win.interactable(on);
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
        let Dlg { mut win, id, title, open, tool, margin } = self;
        if open.as_deref() == Some(&false) {
            return None;
        }
        // CenterWindow at every opening (each DoModal creates the dialog
        // anew): the left-top at the screen's (W − w) / 2, (H − h) / 2
        // floored, from the window's measured size — for its first frames
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
                let screen = ctx.screen_rect();
                win = win.current_pos(screen.min + ((screen.size() - size) / 2.0).floor());
            }
        }
        ctx.data_mut(|d| d.insert_temp(key, (frame, size, settled)));
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
    let resp = ui.interact(r, ui.id().with(("fv_btn", text)), egui::Sense::click());
    let face = r.shrink(1.0);
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

/// A transparent full-screen layer at `order` that swallows the pointer
/// below a modal window (the windows of a higher order stay live).
pub fn modal_backdrop(ctx: &egui::Context, id: &str, order: egui::Order) {
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

/// A message box's body: its icon left of the text (our drawing of the
/// system icons).
pub fn message_body(ui: &mut egui::Ui, icon: MsgIcon, text: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::hover());
        let p = ui.painter();
        let c = r.center();
        let (mark, bg) = match icon {
            MsgIcon::Exclamation => {
                let tri = vec![Pos2::new(c.x, r.top() + 2.0), Pos2::new(r.right() - 1.0, r.bottom() - 3.0), Pos2::new(r.left() + 1.0, r.bottom() - 3.0)];
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
        ui.add_space(8.0);
        ui.label(text);
    });
}

/// Whether a dialog window (egui's `Order::Middle` layer, where every
/// `egui::Window` of ours lives or puts its backdrop) was shown last frame.
pub fn window_up(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Middle))
}

impl App {
    pub fn dialogs_ui(&mut self, ctx: &egui::Context) {
        self.about_window(ctx);
        self.windows_dialog(ctx);
        self.channel_props_window(ctx);
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
            crate::dialogs::dlg(ctx, "Загрузка осциллограммы").id(egui::Id::new(("fv_loading", i))).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::ProgressBar::new(l.shown.0 as f32 / 100.0).desired_width(240.0));
                    if ui.button("Отмена").clicked() {
                        cancel = Some(i);
                    }
                });
            });
        }
        // «Отмена» sets the flag the readers poll; the open is abandoned.
        if let Some(i) = cancel {
            self.loading[i].progress.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            self.loading.remove(i);
        }
    }

    /// «Цвет канала» (33030): a colour dialog started from the first selected
    /// channel's colour; OK gives the colour to every selected channel.
    fn color_window(&mut self, ctx: &egui::Context) {
        let Some(mut col) = self.color_dialog else { return };
        let mut open = true;
        let mut done: Option<bool> = None;
        crate::dialogs::dlg(ctx, "Цвет")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::color_picker::color_picker_color32(ui, &mut col, egui::color_picker::Alpha::Opaque);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        done = Some(true);
                    }
                    if ui.button("Отмена").clicked() {
                        done = Some(false);
                    }
                });
            });
        self.color_dialog = match done {
            Some(true) => {
                if let Some(d) = self.active_doc_mut() {
                    d.set_color_selected(col);
                }
                None
            }
            Some(false) => None,
            None => open.then_some(col),
        };
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

    fn channel_props_window(&mut self, ctx: &egui::Context) {
        let Some(i) = self.props_channel else {
            return;
        };
        let mut open = true;
        let mut recolor_default = false;
        let mut close_btn = false;
        crate::dialogs::dlg(ctx, "Свойства канала")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let Some(doc) = self.active.and_then(|a| self.docs.get_mut(a)) else {
                    ui.label("Нет активной осциллограммы.");
                    return;
                };
                let Some(ch) = doc.osc.analog.get(i) else {
                    ui.label("Канал недоступен.");
                    return;
                };
                let (id, unit, phase) = (ch.id.clone(), ch.unit.clone(), ch.phase.clone());
                egui::Grid::new("chprops").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label("Имя:");
                    ui.label(egui::RichText::new(&id).strong());
                    ui.end_row();
                    ui.label("Фаза:");
                    ui.label(if phase.is_empty() { "—" } else { &phase });
                    ui.end_row();
                    ui.label("Единица:");
                    ui.label(if unit.is_empty() { "—" } else { &unit });
                    ui.end_row();
                    if let Some(cv) = doc.chans.get_mut(i) {
                        ui.label("Цвет:");
                        ui.color_edit_button_srgba(&mut cv.color);
                        ui.end_row();
                        ui.label("Инвертировать:");
                        ui.checkbox(&mut cv.invert, "");
                        ui.end_row();
                        ui.label("Удалить постоянную:");
                        ui.checkbox(&mut cv.remove_dc, "");
                        ui.end_row();
                        ui.label("Видимый:");
                        ui.checkbox(&mut cv.visible, "");
                        ui.end_row();
                        ui.label("На диаграммах:");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut cv.on_freq, "частотн.");
                            // Eligibility by unit as CMDUI 32850 / 32851 (`0x430650`).
                            ui.add_enabled(crate::diagram::vector_unit_ok(unit.trim()), egui::Checkbox::new(&mut cv.on_vector, "вект."));
                            ui.add_enabled(crate::diagram::circle_flag_unit_ok(unit.trim()), egui::Checkbox::new(&mut cv.on_circle, "круг."));
                        });
                        ui.end_row();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Цвет по умолчанию").clicked() {
                        recolor_default = true;
                    }
                    if ui.button("Закрыть").clicked() {
                        close_btn = true;
                    }
                });
            });
        if recolor_default {
            if let Some(doc) = self.active.and_then(|a| self.docs.get_mut(a)) {
                if let (Some(ch), Some(cv)) = (doc.osc.analog.get(i), doc.chans.get_mut(i)) {
                    cv.color = crate::app::default_channel_color(ch, i);
                }
            }
        }
        if !open || close_btn {
            self.props_channel = None;
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

/// A DDX edit: single-line; with `focus` it takes the keyboard focus with
/// its whole text selected (CDataExchange::Fail after a refused value).
pub fn ddx_edit(ui: &mut egui::Ui, text: &mut String, width: f32, focus: bool) -> egui::Response {
    let mut out = egui::TextEdit::singleline(text).desired_width(width).show(ui);
    if focus {
        out.response.request_focus();
        let n = text.chars().count();
        out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
        out.state.store(ui.ctx(), out.response.id);
    }
    out.response
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
