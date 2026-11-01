//! Diagram views (круговая/дифференциальная/векторная/частотная/быстрый анализ)
//! over the active document (ОМП lives in `omp_ui.rs`). The oscillogram (lane) view lives in
//! `lanes.rs`.

use crate::app::{App, ChanKey, Command, Pane, View};
use crate::diagram_ui::Window;
use crate::oscgeom::PixelLines;
use eframe::egui;

/// Remember the offset from the frame's corner of the spot a first float
/// opens at ([`App::float_origin`]).
pub fn set_float_corner(ctx: &egui::Context, corner: egui::Vec2) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("fv_float_corner"), corner));
}

impl App {
    pub fn render_tab(&mut self, ctx: &egui::Context) {
        match self.view {
            View::Oscillogram => self.view_oscillogram(ctx),
        }
    }

    /// The diagram docking panes (`0x474e10`, L9.92 / L9.101): each pane a
    /// unit — docked in a container on a frame edge, beside another
    /// (splitting its slot) or as a tab of another's tabbed group (tabs at
    /// the bottom), floating in its own window or pinned to an auto-hide
    /// strip on its edge; captions carry the pin and a close button that
    /// hides. Call before the central view.
    pub fn diagram_panes(&mut self, ctx: &egui::Context) {
        use crate::docking::Place;
        self.pane_canvas.clear();
        self.drop_targets.clear();
        self.pane_tab_rects.clear();
        self.pane_grid.clear();
        self.dock_client = ctx.available_rect();
        let dim = self.current_dimension();
        let views = self.dock_views();
        // The strips sit outermost (shown with the frame's bars), then the
        // containers.
        self.auto_hide_strips(ctx, &views);
        self.dock_rects = [None; 4];
        let shown = self.groups_shown();
        let mut roots: Vec<DockView> = views.iter().copied().filter(|v| self.docks[v.unit].place == Place::Docked && self.docks.parent(v.unit, |u| shown[u as usize]).is_none()).collect();
        // In the docking manager's list order: each block spans the space
        // the earlier ones left (`AdjustDockingLayout` `0x1008cd55`).
        roots.sort_by_key(|v| self.docks[v.unit].order);
        for r in roots {
            self.dock_container(ctx, r, &views);
        }
        let floating: Vec<DockView> = views.iter().copied().filter(|v| self.docks[v.unit].place == Place::Floating).collect();
        for v in floating {
            self.floating_unit(ctx, v);
        }
        self.slid_out_units(ctx, &views);
        self.dock_drag_frame(ctx);
        self.pane_menu_frame(ctx);
        self.diagram_overlays(ctx);
        for &(p, r) in &self.pane_canvas {
            match self.canvas_memo.iter_mut().find(|(q, _)| *q == p) {
                Some(m) => m.1 = r,
                None => self.canvas_memo.push((p, r)),
            }
        }
        // The circle / differential state stays the last activated one's.
        self.switch_diagram(match self.panes.active {
            Pane::Circle | Pane::Differential => pane_dim(self.panes.active),
            _ => dim,
        });
    }

    /// How many groups show on their own (a tabbed-in pane shows in its
    /// host's group).
    #[cfg(test)]
    pub fn dock_views_shown(&self) -> usize {
        self.dock_views().len()
    }

    /// What shows: per group (its own unit and the panes tabbed into it,
    /// wherever it is) the visible tabs and the front one.
    fn dock_views(&self) -> Vec<DockView> {
        crate::docking::UNITS
            .into_iter()
            .filter(|&u| self.docks.host(u) == u)
            .filter_map(|u| {
                let mut mask = [false; 4];
                for m in self.docks.members(u) {
                    mask[m as usize] = self.panes.visible(m);
                }
                let front = self.panes.front(self.docks.members(u))?;
                // One visible tab: no strip (the tab control's HideSingleTab,
                // `0x1013a212`).
                Some(DockView { unit: u, mask, front, strip: mask.iter().filter(|&&m| m).count() > 1 })
            })
            .collect()
    }

    /// Which units head a group with a visible pane.
    fn groups_shown(&self) -> [bool; 4] {
        crate::docking::UNITS.map(|u| self.docks.host(u) == u && self.panes.front(self.docks.members(u)).is_some())
    }

    /// The group `unit` leaves its slot: a group beside it takes it.
    fn dock_leave(&mut self, unit: crate::docking::Unit) {
        let shown = self.groups_shown();
        let own = self.dock_rects[unit as usize].map(|r| r.size());
        self.docks.leave_sized(unit, |u| shown[u as usize], own);
    }

    /// Where a first float opens: the frame's corner plus the offset
    /// [`set_float_corner`] left (clone: the mini-frame's corner is on the
    /// spot MFC took at the pane's creation, in screen coordinates — the
    /// frame's corner when the frame was shown where Windows first put it,
    /// the default frame's spot when a saved or maximised placement moved it
    /// afterwards, #161).
    fn float_origin(ctx: &egui::Context) -> egui::Pos2 {
        ctx.screen_rect().min + ctx.data(|d| d.get_temp::<egui::Vec2>(egui::Id::new("fv_float_corner"))).unwrap_or(egui::Vec2::ZERO)
    }

    /// A caption's actions on the group of `unit` whose pane spans `pane`:
    /// close hides the shown pane, the pin toggles auto-hide, a
    /// double-click floats / redocks, a drag detaches a docked group (or
    /// moves a floating one).
    /// MFC's pane menu (`0x100ce72c`; the strings 17039 / 17040 / 17088 /
    /// 17041 / 17042): Плавающее (greyed auto-hidden), Закрепленное
    /// (greyed auto-hidden), В закладке (always greyed: CanBeTabbedDocument
    /// is never set), Автоматически скрывать (greyed floating), Скрыть
    /// (greyed auto-hidden); the checks follow the place.
    fn pane_menu_frame(&mut self, ctx: &egui::Context) {
        use crate::docking::Place;
        use crate::popmenu::{Entry, Mark};
        #[derive(Clone, Copy)]
        enum Cmd {
            Float,
            Dock,
            Tab,
            AutoHide,
            Hide,
        }
        let Some((unit, pane, next, mut pm)) = self.pane_menu else { return };
        let place = self.docks[unit].place;
        let ah = place == Place::AutoHide;
        let entries = vec![
            // mfc120rus's texts, each with its mnemonic.
            Entry::item("&Плавающее", !ah, Mark::Check(place == Place::Floating), Cmd::Float),
            Entry::item("&Закрепленное", !ah, Mark::Check(place == Place::Docked), Cmd::Dock),
            Entry::item("&В закладке", false, Mark::None, Cmd::Tab),
            Entry::item("&Автоматически скрывать", place != Place::Floating, Mark::Check(ah), Cmd::AutoHide),
            Entry::item("&Скрыть", !ah, Mark::None, Cmd::Hide),
        ];
        let out = crate::popmenu::show(ctx, "fv_pane_menu", &mut pm, &entries);
        self.pane_menu = (!out.close).then_some((unit, pane, next, pm));
        let origin = Self::float_origin(ctx);
        match out.chosen {
            // Docked: a group's active tab floats alone at its recent rect
            // (`0x100ceb39`–`0x100cec00`), a single pane floats.
            Some(Cmd::Float) if place == Place::Docked => match next {
                Some(n) => {
                    let rect = self.docks[pane].float_at(origin);
                    self.docks.float_tab(pane, unit, n, rect);
                }
                None => {
                    self.dock_leave(unit);
                    self.docks[unit].double_click(origin);
                }
            },
            Some(Cmd::Dock) if place == Place::Floating => self.docks[unit].double_click(origin),
            Some(Cmd::AutoHide) => {
                self.pin_prepare(unit);
                self.docks[unit].pin();
            }
            Some(Cmd::Hide) => self.dispatch(Command::TogglePane(pane)),
            _ => {}
        }
    }

    /// What every way of auto-hiding a docked group does first (the caption
    /// pin, Ctrl + pin, the pane menu's «Автоматически скрывать»; MFC has one
    /// toggle): the slid-out pane's window is as large as its docked one
    /// (#126, VM): a lone pane (the bottom Частотная, the right
    /// Дифференциальная) the same size, its divider outside it, over the
    /// view; a tabbed group without the frame its tab control drew (3 px)
    /// and without a place for the divider (4 px; L9.410); then the group
    /// leaves its slot.
    fn pin_prepare(&mut self, u: crate::docking::Unit) {
        if self.docks[u].place != crate::docking::Place::Docked {
            return;
        }
        if let Some(r) = self.dock_rects[u as usize] {
            let across = if self.docks.edge(u).vertical() { r.width() } else { r.height() };
            let grouped = self.dock_views().iter().any(|g| g.unit == u && g.strip);
            // On a top / bottom edge the group's tab strip lies across the
            // extent: VM, a bottom group slid out is 48 px lower than docked
            // at two docked heights (100 %; 125 / 150 % not run: the strip's
            // height twice, a reading of the one constant).
            let lost = if !grouped {
                0.0
            } else if self.docks.edge(u).vertical() {
                crate::dpi::s(crate::docking::GROUP_FRAME_PX + crate::docking::DIVIDER_PX)
            } else {
                2.0 * crate::dpi::measured(24.0, 30.0, 34.0)
            };
            self.docks[u].slide_extent = Some((across - lost).max(crate::docking::SLIDE_MIN_PX));
        }
        self.dock_leave(u);
    }

    fn caption_acts(&mut self, ctx: &egui::Context, unit: crate::docking::Unit, shown: Pane, act: CaptionAct, pane: egui::Rect) {
        use crate::docking::Place;
        if act.close {
            self.dispatch(Command::HidePane(shown));
            return;
        }
        let origin = Self::float_origin(ctx);
        if act.pin {
            // Ctrl held at the release (mfc120u `0x100871c7`, GetAsyncKeyState
            // at `0x10087337`): every shown group on the same edge in the
            // same state goes at once — auto-hidden with no slide, or
            // restored; a plain pin slides the pane shut in 12 steps.
            let ctrl = ctx.input(|i| i.modifiers.ctrl);
            let (place, edge) = (self.docks[unit].place, self.docks.edge(unit));
            let units: Vec<crate::docking::Unit> = if ctrl {
                crate::docking::UNITS
                    .into_iter()
                    .filter(|&u| self.docks.host(u) == u && self.docks[u].place == place && self.docks.edge(u) == edge && self.panes.front(self.docks.members(u)).is_some())
                    .collect()
            } else {
                vec![unit]
            };
            for u in units {
                self.pin_prepare(u);
                self.docks[u].pin();
                if !ctrl && self.docks[u].place == Place::AutoHide {
                    self.docks[u].slide = 1.0;
                }
            }
        }
        if act.double {
            if self.docks[unit].place == Place::Docked {
                self.dock_leave(unit);
            }
            self.docks[unit].double_click(origin);
            return;
        }
        // The four-way move cursor from the press on the caption of a pane
        // that a drag moves (docked: out of its dock; floating: its window),
        // and while it is dragged. An auto-hidden one's caption drag does
        // nothing: the arrow stays.
        let place = self.docks[unit].place;
        if (act.armed || act.dragging) && matches!(place, Place::Docked | Place::Floating) {
            ctx.set_cursor_icon(egui::CursorIcon::Move);
        }
        if act.armed || act.dragging {
            let (press, now, delta) = ctx.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos(), i.pointer.delta()));
            let (Some(press), Some(now)) = (press, now) else { return };
            let dragging = self.dock_drag.is_some();
            match place {
                // Past the drag distance from the press, wherever the
                // pointer is (the drag has not yet reached egui's own
                // click-or-drag decision).
                Place::Docked if act.armed => {
                    if self.docks[unit].drag_detach(press, now, pane, origin) {
                        // A group beside it takes its slot.
                        self.dock_leave(unit);
                        let r = self.docks[unit].float_at(origin);
                        self.dock_drag = Some((unit, now - r.min));
                    }
                }
                // A floating pane's drag shows the markers too.
                Place::Floating if act.dragging && !dragging => {
                    let d = &mut self.docks[unit];
                    d.float_rect = d.float_rect.map(|r| r.translate(delta));
                    let r = self.docks[unit].float_at(origin);
                    self.dock_drag = Some((unit, now - r.min));
                }
                _ => {}
            }
        }
    }

    /// A group's body: with several tabs the tab strip at the bottom, then
    /// the front pane with its caption, toolbar and window.
    fn unit_body(&mut self, ui: &mut egui::Ui, v: DockView) {
        if !v.strip {
            // In a panel of its own: the content cannot widen a dock slot.
            let outer = ui.max_rect();
            egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                self.pane_body(ui, v, v.front, outer);
            });
            return;
        }
        let outer = ui.max_rect();
        // A tab pulled out of the strip (`0x10068d0b` / `0x100690b4`).
        let mut dragged: Option<Pane> = None;
        // The tab a left press is held on (the detach's arming: the original
        // tracks the press itself, not egui's drag decision).
        let mut armed: Option<Pane> = None;
        // The tab a double-click landed on (`0x10068d99`: the detach, mode 2).
        let mut double: Option<Pane> = None;
        let mut tabs: Vec<(Pane, egui::Rect)> = Vec::new();
        // The tab strip (VM, Office 2007 Blue): a (157, 189, 216) line over
        // the brush (191, 219, 255), 20 px tabs from 4 px in; the front tab
        // framed (141, 178, 227) on a (224, 233, 246) → (247, 251, 255)
        // gradient, its bottom edge (154, 187, 230); every label (21, 66,
        // 139), 6 px in.
        let brush = self.skin.splitter();
        // 24 px, 30 / 34 at 125 % / 150 % (VM), its tabs 4 px less.
        let strip_h = crate::dpi::measured(24.0, 30.0, 34.0);
        let strip = egui::TopBottomPanel::bottom(egui::Id::new(("pane_tabs", v.unit))).show_separator_line(false).exact_height(strip_h).frame(egui::Frame::none().fill(brush)).show_inside(ui, |ui| {
            let band = ui.max_rect();
            ui.painter().hline(band.x_range(), band.top() + 0.5, egui::Stroke::new(1.0f32, egui::Color32::from_rgb(157, 189, 216)));
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            ui.add_space(1.0);
            ui.horizontal(|ui| {
                ui.add_space(crate::dpi::s(4.0));
                let order = self.tab_order;
                for p in order.into_iter().filter(|&p| v.mask[p as usize]) {
                    let on = p == v.front;
                    let font = egui::FontId::proportional(crate::dpi::font(12.0));
                    let ink = egui::Color32::from_rgb(21, 66, 139);
                    let g = ui.painter().layout_no_wrap(pane_title(p).to_string(), font, ink);
                    // An id per pane: the drag follows the tab when it moves.
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(g.size().x + crate::dpi::s(12.0), strip_h - 4.0), egui::Sense::hover());
                    let r = ui.interact(rect, ui.id().with(("fv_pane_tab", p as u8)), egui::Sense::click_and_drag());
                    // A press on a tab (either button, the front one too)
                    // gives the group's pane the focus (clone: the tab
                    // control focuses the pane it shows).
                    if r.hovered() && ui.input(|i| i.pointer.primary_pressed() || i.pointer.secondary_pressed()) {
                        self.copy_focus = crate::app::CopyFocus::Pane(v.unit);
                    }
                    let pnt = ui.painter();
                    if on {
                        crate::lanes::gradient(pnt, rect, egui::Color32::from_rgb(224, 233, 246), egui::Color32::from_rgb(247, 251, 255));
                        let frame = egui::Stroke::new(1.0f32, egui::Color32::from_rgb(141, 178, 227));
                        pnt.vline(rect.left() + 0.5, rect.y_range(), frame);
                        pnt.vline(rect.right() - 0.5, rect.y_range(), frame);
                        pnt.hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0f32, egui::Color32::from_rgb(154, 187, 230)));
                    } else if r.hovered() {
                        // Hot (VM, «Круговая диаграмма» under the pointer):
                        // the ribbon's hot tab upside down — warm (225, 211,
                        // 165) at the strip's line to (208, 225, 243) at
                        // mid-height, then (200, 224, 255) → (197, 222, 254),
                        // a light last row; a (152, 187, 233) frame at the
                        // sides and bottom, (239, 246, 255) inside it.
                        use crate::oscgeom::PixelLines;
                        let c = |r, g, b| egui::Color32::from_rgb(r, g, b);
                        let face = rect.shrink2(egui::vec2(1.0, 0.0)).with_max_y(rect.bottom() - 1.0);
                        let (h, mid) = (face.height(), (face.height() / 2.0).round());
                        let (hi, frame) = (c(226, 239, 255), egui::Stroke::new(1.0f32, c(152, 187, 233)));
                        pnt.add(crate::ribbon::gradient_mesh(face, &[(0.0, c(225, 211, 165)), (mid, c(208, 225, 243)), (mid, c(200, 224, 255)), (h - 1.0, c(197, 222, 254)), (h - 1.0, hi), (h, hi)]));
                        let inner = egui::Stroke::new(1.0f32, c(239, 246, 255));
                        pnt.px_vline(face.left(), face.top()..=face.bottom() - 2.0, inner);
                        pnt.px_vline(face.right() - 1.0, face.top()..=face.bottom() - 2.0, inner);
                        pnt.px_vline(rect.left(), rect.top()..=rect.bottom() - 2.0, frame);
                        pnt.px_vline(rect.right() - 1.0, rect.top()..=rect.bottom() - 2.0, frame);
                        pnt.px_hline(rect.left() + 1.0..=rect.right() - 2.0, rect.bottom() - 1.0, frame);
                    }
                    pnt.galley(egui::pos2(rect.left() + crate::dpi::s(6.0), rect.center().y - g.size().y / 2.0), g, ink);
                    tabs.push((p, r.rect));
                    // A press (or a drag) brings the tab to the front.
                    if (r.clicked() || r.drag_started()) && !on {
                        self.dispatch(Command::ActivatePane(p));
                    }
                    // A right-click too, and opens the pane's menu there
                    // (VM: the caption's five items, for that pane).
                    if r.secondary_clicked() {
                        if !on {
                            self.dispatch(Command::ActivatePane(p));
                        }
                        let at = ui.ctx().pointer_interact_pos().unwrap_or(rect.center());
                        let next = self.tab_order.into_iter().find(|&q| q != p && v.mask[q as usize]);
                        self.pane_menu = Some((v.unit, p, next, crate::popmenu::PopMenu::new(ui.ctx().cumulative_pass_nr(), at)));
                    }
                    if r.dragged() {
                        dragged = Some(p);
                    }
                    if r.double_clicked() {
                        double = Some(p);
                    }
                    if r.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down()) {
                        armed = Some(p);
                    }
                }
            });
        });
        self.pane_tab_rects.extend(&tabs);
        // Inside the strip a dragged tab passing over another swaps with it
        // (`0x100690b4`, tab swapping on, more than one tab).
        if let (Some(p), Some(at)) = (dragged, ui.ctx().input(|i| i.pointer.latest_pos())) {
            if strip.response.rect.contains(at) {
                // Past the other tab's middle, so tabs of unequal widths do
                // not swap back and forth.
                let own = tabs.iter().find(|(q, _)| *q == p).map(|(_, r)| r.center().x);
                if let Some(&(q, _)) = tabs.iter().find(|(q, r)| *q != p && r.contains(at) && own.is_some_and(|x| (r.center().x > x) == (at.x > r.center().x))) {
                    crate::docking::swap_tabs(&mut self.tab_order, p, q);
                }
            }
        }
        if let (Some(p), true) = (armed, tabs.len() > 1) {
            let ctx = ui.ctx().clone();
            // The four-way move cursor from the press on and while it is
            // dragged, as a caption's (L9.411; an improvement: the original
            // keeps the arrow).
            ctx.set_cursor_icon(egui::CursorIcon::Move);
            self.tab_drag_out(&ctx, v, p, strip.response.rect);
        }
        // A double-click on a tab of a docked group with more than one tab
        // floats that pane alone (clone, #124), at its recent floating rect
        // as a caption double-click does.
        if let (Some(p), true) = (double, tabs.len() > 1) {
            if self.docks[v.unit].place == crate::docking::Place::Docked && self.dock_drag.is_none() {
                let next = self.tab_order.into_iter().find(|&q| q != p && v.mask[q as usize]).unwrap_or(v.unit);
                let rect = self.docks[p].float_at(Self::float_origin(ui.ctx()));
                self.docks.float_tab(p, v.unit, next, rect);
                // Clone (FOCUS): the keyboard focus is the floated pane's own
                // window, in its mini-frame, whichever tab it was.
                self.copy_focus = crate::app::CopyFocus::Pane(p);
            } else if self.docks[v.unit].place == crate::docking::Place::Floating && self.dock_drag.is_none() {
                // In a floating group the same double-click docks that pane
                // alone where the group was last docked (clone, #162).
                let next = self.tab_order.into_iter().find(|&q| q != p && v.mask[q as usize]).unwrap_or(v.unit);
                self.docks.dock_tab_back(p, v.unit, next);
                // Clone (FOCUS): the keyboard focus is the docked pane's own
                // window.
                self.copy_focus = crate::app::CopyFocus::Pane(p);
            }
        }
        egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
            self.pane_body(ui, v, v.front, outer);
        });
    }

    /// One pane of group `v`: its caption (acting on the group), then the
    /// pane's vertical toolbar on the right edge (550 / 552 / 553;
    /// Частотная has none) and its window.
    fn pane_body(&mut self, ui: &mut egui::Ui, v: DockView, p: Pane, outer: egui::Rect) {
        use crate::docking::Place;
        let place = self.docks[v.unit].place;
        let act = pane_caption(ui, &self.skin.palette(), pane_title(p), (place != Place::Floating).then_some(place == Place::AutoHide));
        // A press on the caption gives the pane the focus (its
        // WM_LBUTTONDOWN `0x10086d23`: SetFocus on the pane or its one
        // child window; WM_RBUTTONDOWN `0x10086f01`: on the pane).
        // (Not in a floating window under a modal dialog: it is disabled.)
        if act.pressed && !(place == Place::Floating && self.modal_up) {
            self.copy_focus = crate::app::CopyFocus::Pane(v.unit);
        }
        let ctx = ui.ctx().clone();
        if let Some(at) = act.menu {
            let next = self.tab_order.into_iter().find(|&q| q != p && v.mask[q as usize]);
            self.pane_menu = Some((v.unit, p, next, crate::popmenu::PopMenu::new(ctx.cumulative_pass_nr(), at)));
        }
        self.caption_acts(&ctx, v.unit, p, act, outer);
        if act.close {
            return;
        }
        // A tabbed group frames its panes under the caption (VM: a (185,
        // 208, 237) line, then 2 px of (213, 228, 242), at the top and
        // sides; 1 px of it at the bottom, the tab strip below). The
        // vector diagram's window stops a row short: white under it (VM;
        // the circle diagram's reaches the frame).
        if v.strip {
            let r = ui.available_rect_before_wrap();
            let pnt = ui.painter();
            pnt.rect_filled(r, 0.0, egui::Color32::from_rgb(213, 228, 242));
            let b = egui::Stroke::new(1.0f32, egui::Color32::from_rgb(185, 208, 237));
            pnt.px_hline(r.left()..=r.right() - 1.0, r.top(), b);
            pnt.px_vline(r.left(), r.top()..=r.bottom() - 1.0, b);
            pnt.px_vline(r.right() - 1.0, r.top()..=r.bottom() - 1.0, b);
            let short = if p == Pane::Vector { 1.0 } else { 0.0 };
            let inner = egui::Rect::from_min_max(r.min + egui::vec2(3.0, 3.0), r.max - egui::vec2(3.0, 1.0 + short));
            pnt.rect_filled(egui::Rect::from_min_max(inner.left_bottom(), egui::pos2(inner.right(), inner.bottom() + short)), 0.0, egui::Color32::WHITE);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(*ui.layout()));
            let menu = self.pane_window(&mut child, p);
            ui.advance_cursor_after_rect(r);
            self.grid_pane_menu(ui.ctx(), v, p, menu);
            return;
        }
        let menu = self.pane_window(ui, p);
        self.grid_pane_menu(ui.ctx(), v, p, menu);
    }

    /// A right-click on a property grid (`at`): its pane's menu there, as
    /// on the caption (VM: «Floating / Docking / …» over «Тип блока»).
    fn grid_pane_menu(&mut self, ctx: &egui::Context, v: DockView, p: Pane, at: Option<egui::Pos2>) {
        if let Some(at) = at {
            let next = self.tab_order.into_iter().find(|&q| q != p && v.mask[q as usize]);
            self.pane_menu = Some((v.unit, p, next, crate::popmenu::PopMenu::new(ctx.cumulative_pass_nr(), at)));
        }
    }

    /// The pane's window under its caption: the toolbar on the right and
    /// the diagram.
    /// Returns where its property grid was right-clicked.
    fn pane_window(&mut self, ui: &mut egui::Ui, p: Pane) -> Option<egui::Pos2> {
        let mut grid_menu = None;
        match p {
            Pane::Vector => {
                self.pane_toolbar(ui, Window::Vector);
                egui::CentralPanel::default().frame(egui::Frame::none().inner_margin(egui::Margin { left: 1.0, right: 2.0, top: 1.0, bottom: 1.0 })).show_inside(ui, |ui| {
                    self.pane_canvas.push((Pane::Vector, ui.available_rect_before_wrap()));
                    self.vector_ui(ui);
                    self.pane_drop(ui, ui.max_rect(), Pane::Vector);
                    // A white column before the toolbar, as the plane's.
                    let m = ui.max_rect();
                    let w = egui::Rect::from_min_max(m.min - egui::vec2(1.0, 1.0), m.max + egui::vec2(2.0, 1.0));
                    ui.painter().px_vline(w.right() - 1.0, w.y_range(), egui::Stroke::new(1.0f32, egui::Color32::WHITE));
                    window_border(ui, w.with_max_x(w.right() - 1.0), [true; 4]);
                });
            }
            Pane::Circle | Pane::Differential => {
                // The two planes share the characteristics state: this
                // pane's is brought in.
                self.switch_diagram(pane_dim(p));
                self.pane_toolbar(ui, if p == Pane::Circle { Window::Circle } else { Window::Diff });
                // The circle window (grid and diagram, `0x437560`) is the drop
                // target, not the toolbar of the pane around it.
                let window = ui.available_rect_before_wrap();
                grid_menu = self.char_ui(ui, p);
                // The differential diagram has no drop target.
                if p == Pane::Circle {
                    self.pane_drop(ui, window, Pane::Circle);
                }
            }
            Pane::Frequency => {
                self.pane_canvas.push((Pane::Frequency, ui.available_rect_before_wrap()));
                self.frequency_ui(ui);
                self.pane_drop(ui, ui.max_rect(), Pane::Frequency);
            }
        }
        grid_menu
    }

    /// A tab dragged out of a group's strip: at the system drag distance
    /// (SM_CXDRAG / SM_CYDRAG, 4 px on either axis, `>=`, `0x10068657`; grown
    /// with the display scale as the caption's, L9.411) and outside the strip
    /// (`0x100690b4` tries the detach only with the pointer off the tab area),
    /// `CTabbedPane::FloatTab` (`0x1013a259`) floats that pane alone under
    /// the pointer (the group's own pane hands the group to the next tab);
    /// the drag then goes on as a floating caption drag (the markers). The
    /// mini-frame comes up at the create size with the pane's own corner
    /// under the pointer: its outer corner 4 px left of and 27 px above
    /// it (VM: Круговая pulled out of the group to (1500, 700) floats with
    /// its frame at (1496, 673)).
    fn tab_drag_out(&mut self, ctx: &egui::Context, v: DockView, pulled: Pane, strip: egui::Rect) {
        let (press, now) = ctx.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos()));
        let (Some(press), Some(now)) = (press, now) else { return };
        let d = now - press;
        let reach = crate::dpi::s(crate::docking::DRAG_PX);
        if (d.x.abs() < reach && d.y.abs() < reach) || strip.contains(now) || self.dock_drag.is_some() {
            return;
        }
        let next = self.tab_order.into_iter().find(|&q| q != pulled && v.mask[q as usize]).unwrap_or(v.unit);
        let origin = Self::float_origin(ctx);
        let grab = egui::vec2(4.0, 27.0);
        let size = self.docks[pulled].float_at(origin).size();
        self.docks.float_tab(pulled, v.unit, next, egui::Rect::from_min_size(now - grab, size));
        self.dock_drag = Some((pulled, grab));
    }

    /// A docked container on its frame edge holding the group `h` and the
    /// groups beside it.
    fn dock_container(&mut self, ctx: &egui::Context, h: DockView, views: &[DockView]) {
        use crate::docking::Edge;
        let edge = self.docks.edge(h.unit);
        let hd = self.docks[h.unit];
        let shown = self.groups_shown();
        // Each shown docked group's parent slot, once.
        let parents: Vec<(DockView, Option<crate::docking::Unit>)> = views.iter().map(|&v| (v, (self.docks[v.unit].place == crate::docking::Place::Docked).then(|| self.docks.parent(v.unit, |u| shown[u as usize])).flatten())).collect();
        let mut inside = vec![h];
        let mut k = 0;
        while k < inside.len() {
            let u = inside[k].unit;
            inside.extend(parents.iter().filter(|(_, p)| *p == Some(u)).map(|(v, _)| *v));
            k += 1;
        }
        // The create sizes (500 wide on the right, Частотная 200 tall) or
        // the last drop's; a new drop is a new panel.
        let id = egui::Id::new(("fv_dock", edge as u8, h.unit, hd.gen));
        let natural = inside.iter().map(|v| v.natural()).fold(0.0, f32::max);
        let across = matches!(edge, Edge::Left | Edge::Right);
        let mut ext = match hd.extent {
            Some(e) => e,
            None if !across && (inside.len() > 1 || natural > 200.0) => 300.0,
            None => natural,
        };
        // Docked at its create size (no saved one), a block is capped once
        // at half the documents' area the earlier blocks leave
        // (`DockToFrameWindow` `0x10088f61`, `AdjustRectToClientArea`
        // `0x1008c750`), then keeps that size however the frame changes
        // (VM, L9.394).
        if hd.extent.is_none() {
            let avail = ctx.available_rect();
            ext = crate::docking::start_extent(ext, if across { avail.width() } else { avail.height() });
            self.docks[h.unit].extent = Some(ext);
        }
        let body = |app: &mut App, ui: &mut egui::Ui| app.dock_slot(ui, h, edge, &parents);
        let strip = self.skin.splitter();
        match edge {
            Edge::Left | Edge::Right => {
                let p = if edge == Edge::Left { egui::SidePanel::left(id) } else { egui::SidePanel::right(id) };
                // The pane's size plus the 4 px strip (VM: 500 + 4). The
                // divider goes as far as the pointer takes it (VM): the
                // block down to its strip and 2 px, the document area down
                // to 4 px.
                let range = 6.0..=(ctx.available_rect().width() - 4.0).max(6.0);
                let size = grip_size(ctx, id, ext + 4.0, range.clone());
                let mut dragged = None;
                p.resizable(false).show_separator_line(false).frame(dock_frame(strip, edge)).exact_width(size).show(ctx, |ui| {
                    dragged = grip(ui, id, edge, size, range);
                    body(self, ui)
                });
                // Only what a drag of the divider gives the block is kept in
                // the group's extent (its size lives under this panel's id,
                // which changes when the neighbour takes the block over:
                // #127) — never a clamp to the room the frame leaves.
                if let Some(n) = dragged {
                    self.docks[h.unit].extent = Some(n - 4.0);
                }
            }
            Edge::Top | Edge::Bottom => {
                let p = if edge == Edge::Top { egui::TopBottomPanel::top(id) } else { egui::TopBottomPanel::bottom(id) };
                let range = 6.0..=(ctx.available_rect().height() - 4.0).max(6.0);
                let size = grip_size(ctx, id, ext + 4.0, range.clone());
                let mut dragged = None;
                p.resizable(false).show_separator_line(false).frame(dock_frame(strip, edge)).exact_height(size).show(ctx, |ui| {
                    dragged = grip(ui, id, edge, size, range);
                    body(self, ui)
                });
                // Only what a drag of the divider gives the block is kept in
                // the group's extent (its size lives under this panel's id,
                // which changes when the neighbour takes the block over:
                // #127) — never a clamp to the room the frame leaves.
                if let Some(n) = dragged {
                    self.docks[h.unit].extent = Some(n - 4.0);
                }
            }
        }
    }

    /// A group's slot in its container: the groups beside it split it
    /// (each a panel on its side, holding its own slot), the rest holds the
    /// group.
    fn dock_slot(&mut self, ui: &mut egui::Ui, v: DockView, edge: crate::docking::Edge, parents: &[(DockView, Option<crate::docking::Unit>)]) {
        use crate::docking::{Edge, Spot};
        for w in parents.iter().filter(|(_, p)| *p == Some(v.unit)).map(|(w, _)| *w) {
            let wd = self.docks[w.unit];
            let Spot::Beside(_, side) = wd.spot else { continue };
            let full = ui.available_size();
            let inner = egui::Id::new(("fv_dock_beside", edge as u8, w.unit, wd.gen));
            match side {
                Edge::Top | Edge::Bottom => {
                    let p = if side == Edge::Top { egui::TopBottomPanel::top(inner) } else { egui::TopBottomPanel::bottom(inner) };
                    let d = wd.extent.unwrap_or_else(|| w.natural().min(full.y / 2.0));
                    let gap = if side == Edge::Top { egui::Margin { bottom: 4.0, ..Default::default() } } else { egui::Margin { top: 4.0, ..Default::default() } };
                    let range = 80.0..=(full.y - 120.0).max(80.0);
                    let size = grip_size(ui.ctx(), inner, d, range.clone());
                    p.resizable(false).show_separator_line(false).frame(egui::Frame::none().fill(self.skin.splitter()).inner_margin(gap)).exact_height(size).show_inside(ui, |ui| {
                        grip(ui, inner, side, size, range);
                        self.dock_slot(ui, w, edge, parents)
                    });
                }
                Edge::Left | Edge::Right => {
                    let p = if side == Edge::Left { egui::SidePanel::left(inner) } else { egui::SidePanel::right(inner) };
                    let d = wd.extent.unwrap_or(full.x / 2.0);
                    // The panes beside each other: 4 px of the splitter brush
                    // between them (VM).
                    let gap = if side == Edge::Left { egui::Margin { right: 4.0, ..Default::default() } } else { egui::Margin { left: 4.0, ..Default::default() } };
                    let range = 120.0..=(full.x - 120.0).max(120.0);
                    let size = grip_size(ui.ctx(), inner, d, range.clone());
                    p.resizable(false).show_separator_line(false).frame(egui::Frame::none().fill(self.skin.splitter()).inner_margin(gap)).exact_width(size).show_inside(ui, |ui| {
                        grip(ui, inner, side, size, range);
                        self.dock_slot(ui, w, edge, parents)
                    });
                }
            }
        }
        let rect = ui.available_rect_before_wrap();
        self.dock_rects[v.unit as usize] = Some(rect);
        self.unit_body(ui, v);
        // The slot keeps its whole rect (a central panel inside claims
        // nothing of its parent, so the side panel would shrink to its
        // minimum).
        ui.expand_to_include_rect(rect);
    }

    /// A floating group: its own window at its recent floating rect; the
    /// caption moves it, a double-click redocks it.
    fn floating_unit(&mut self, ctx: &egui::Context, v: DockView) {
        let origin = Self::float_origin(ctx);
        let unit = v.unit;
        let r = self.docks[unit].float_at(origin);
        // The rect is the mini-frame's outer one (clone: 200 x 200 and
        // 500 x 500 outside); egui sizes the content, its frame comes on top.
        let chrome = egui::Frame::window(&ctx.style()).total_margin().sum();
        let resp = egui::Window::new(pane_title(v.front))
            .id(floating_pane_id(unit))
            .title_bar(false)
            .interactable(!self.modal_up)
            .resizable(!self.modal_up)
            .collapsible(false)
            .current_pos(r.min)
            .default_size(r.size() - chrome)
            .min_size(egui::vec2(200.0, 150.0) - chrome)
            .show(ctx, |ui| {
                ui.set_min_size(ui.available_size());
                self.unit_body(ui, v);
                // MFC's frame disables its floating windows while a modal
                // dialog is up (clone: «Каналы» open, the mini-frame reads
                // as disabled): a widget over the whole window, the last
                // of its layer, takes every click and drag (an egui
                // disabled window would fade).
                if self.modal_up {
                    ui.interact(ui.max_rect(), ui.id().with("fv_float_disabled"), egui::Sense::click_and_drag());
                }
            });
        if let Some(resp) = resp {
            // Its size as the user leaves it; the position the caption sets.
            let d = &mut self.docks[unit];
            if d.place == crate::docking::Place::Floating {
                let min = d.float_rect.map_or(r.min, |f| f.min);
                d.float_rect = Some(egui::Rect::from_min_size(min, resp.response.rect.size()));
            }
        }
    }

    /// The auto-hide strips of the left, right and top edges, shown before
    /// the ribbon and the bars so that they take the frame's outermost
    /// space (VM, maximised): a side strip runs from under the caption row
    /// to the window's bottom, beside the ribbon, the bars and the status
    /// row; the top strip is the first row of the client, under the
    /// caption's glass (so not seen) with the ribbon pushed down by it.
    pub fn strips_outer(&mut self, ctx: &egui::Context) {
        use crate::docking::Edge;
        self.dock_strips = [None; 4];
        self.strip_hits = Default::default();
        self.strip_inset = [0.0; 4];
        let views = self.dock_views();
        for e in [Edge::Top, Edge::Left, Edge::Right] {
            self.strip_edge(ctx, e, &views);
        }
    }

    /// The bottom edge's strip, shown before the status row: the last row
    /// of the client, below it (VM).
    pub fn strip_bottom(&mut self, ctx: &egui::Context) {
        let views = self.dock_views();
        self.strip_edge(ctx, crate::docking::Edge::Bottom, &views);
    }

    /// One edge's strip: the buttons of every pinned group on it, one per
    /// visible pane in the tab order, the group's front pane highlighted;
    /// hovering one 400 ms or a click slides its group out with that pane
    /// in front (the group's other panes are not shown with it).
    fn strip_edge(&mut self, ctx: &egui::Context, edge: crate::docking::Edge, views: &[DockView]) {
        use crate::docking::{Edge, Place};
        let groups: Vec<DockView> = views.iter().copied().filter(|v| self.docks[v.unit].place == Place::AutoHide && self.docks.edge(v.unit) == edge).collect();
        if groups.is_empty() {
            return;
        }
        let vertical = edge.vertical();
        let screen = ctx.screen_rect();
        let brush = self.skin.splitter();
        // The strip: 23 px across a side edge, 21 along the top / bottom
        // one (VM); the buttons 2 px less across, on its outer side, 4 px
        // in along it.
        let across = crate::dpi::s(if vertical { 23.0 } else { 21.0 });
        let cap = if vertical { crate::ribbon::caption_h() } else { 0.0 };
        let front: Vec<(DockView, Vec<Pane>)> = groups.iter().map(|&v| (v, self.tab_order.into_iter().filter(|&p| v.mask[p as usize]).collect())).collect();
        let id = egui::Id::new(("fv_strip", edge as u8));
        let mut hits: Vec<(crate::docking::Unit, StripHit)> = Vec::new();
        let draw = |ui: &mut egui::Ui| -> egui::Rect {
            let full = ui.max_rect();
            let strip = egui::Rect::from_min_max(full.min + egui::vec2(0.0, cap), full.max);
            if vertical {
                // Beside the ribbon's caption row: the window's glass.
                ui.painter().rect_filled(egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, strip.top())), 0.0, egui::Color32::WHITE);
            }
            // The top strip lies under the caption's glass: not seen.
            let seen = edge != Edge::Top;
            if seen {
                ui.painter().rect_filled(strip, 0.0, brush);
            } else {
                ui.painter().rect_filled(full, 0.0, egui::Color32::WHITE);
            }
            // Along a side strip the buttons start 4 px under the window's
            // top, behind the caption row's glass (VM: the first button's
            // title is cut by it); along the others 4 px in.
            let maximised = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            let origin = if vertical { screen.top() - if maximised { 8.0 } else { 0.0 } - strip.top() } else { 0.0 };
            let mut at = origin + crate::dpi::s(4.0);
            let paint = ui.painter().with_clip_rect(strip);
            for (v, titles) in &front {
                let mut hit = StripHit::default();
                for &t in titles {
                    let title = pane_title(t);
                    let is_front = t == v.front;
                    let len = crate::dpi::s(crate::docking::strip_button_len(title.chars().count(), is_front, vertical));
                    let rect = match edge {
                        Edge::Left => egui::Rect::from_min_size(egui::pos2(strip.left(), strip.top() + at), egui::vec2(across - crate::dpi::s(2.0), len)),
                        Edge::Right => egui::Rect::from_min_size(egui::pos2(strip.left() + crate::dpi::s(2.0), strip.top() + at), egui::vec2(across - crate::dpi::s(2.0), len)),
                        Edge::Top => egui::Rect::from_min_size(egui::pos2(strip.left() + at, strip.top()), egui::vec2(len, across - crate::dpi::s(2.0))),
                        Edge::Bottom => egui::Rect::from_min_size(egui::pos2(strip.left() + at, strip.top() + crate::dpi::s(2.0)), egui::vec2(len, across - crate::dpi::s(2.0))),
                    };
                    at += len;
                    // A button is hovered where it lies, under the caption's
                    // glass too (VM: the stub of a left strip's first tab, 4 px
                    // under the window's top, slides its pane out); the window's
                    // own buttons, above the strip, take theirs.
                    let vis = rect.intersect(full);
                    if vis.width() <= 0.0 || vis.height() <= 0.0 {
                        continue;
                    }
                    let r = ui.interact(vis, id.with((v.unit as u8, t as u8)), egui::Sense::click());
                    if seen {
                        strip_button(&paint, rect, title, vertical, is_front, r.hovered());
                    }
                    // The strip acts on the press (WM_LBUTTONDOWN).
                    let pressed = r.hovered() && ui.input(|i| i.pointer.primary_pressed());
                    if r.hovered() || pressed {
                        hit.on = true;
                        hit.pressed |= pressed;
                        hit.pane = Some(t);
                    }
                    hit.buttons.push(vis);
                }
                hits.push((v.unit, hit));
                // A group's buttons are set apart from the next group's.
                at += crate::dpi::s(8.0);
            }
            strip
        };
        let frame = egui::Frame::none();
        let rect = match edge {
            Edge::Left => egui::SidePanel::left(id).resizable(false).exact_width(across).show_separator_line(false).frame(frame).show(ctx, draw).inner,
            Edge::Right => egui::SidePanel::right(id).resizable(false).exact_width(across).show_separator_line(false).frame(frame).show(ctx, draw).inner,
            Edge::Top => egui::TopBottomPanel::top(id).resizable(false).exact_height(across).show_separator_line(false).frame(frame).show(ctx, draw).inner,
            Edge::Bottom => egui::TopBottomPanel::bottom(id).resizable(false).exact_height(across).show_separator_line(false).frame(frame).show(ctx, draw).inner,
        };
        self.strip_inset[edge_ix(edge)] = across;
        for (u, h) in hits {
            self.dock_strips[u as usize] = Some(rect);
            self.strip_hits[u as usize] = h;
        }
    }

    /// The hover, press and hide checks of the pinned groups, after their
    /// strips are drawn: hovering a button 400 ms or a press slides the
    /// group out with that pane in front; every 700 ms a slid-out group
    /// slides back unless the pointer is on it.
    fn auto_hide_strips(&mut self, ctx: &egui::Context, views: &[DockView]) {
        use crate::docking::Place;
        let now = ctx.input(|i| i.time);
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let menu_up = ctx.memory(|m| m.any_popup_open()) || self.pane_menu.is_some();
        // The next frame any of them needs (the slide, the hover's open
        // time, the hide check's timer).
        let mut wake: Option<f64> = None;
        let pinned: Vec<DockView> = views.iter().copied().filter(|v| self.docks[v.unit].place == Place::AutoHide).collect();
        // A unit that no longer shows (its panes hidden) is back in its
        // strip when it shows again.
        for u in crate::docking::UNITS {
            if !views.iter().any(|v| v.unit == u) {
                self.docks.0[u as usize].hide();
            }
        }
        // The focus leaves a window that hides — its pane closed, its
        // group closed or auto-hidden and in its strip (Windows hands a
        // hidden window's focus to its parent, the frame passes it on to
        // the view).
        // A pane of a tabbed group keeps the focus window it was when it
        // or its group hides (clone: GetFocus still names the hidden pane
        // after the ribbon toggle or the caption «×»); only a group slid
        // back into its strip takes it off.
        if let Some(g) = self.focus_group() {
            let d = self.docks[g];
            let tabbed = self.docks.members(g).count() > 1;
            let hidden = !views.iter().any(|v| v.unit == g);
            if (hidden && !tabbed) || (d.place == Place::AutoHide && !d.out && d.slide == 0.0) {
                self.copy_focus = crate::app::CopyFocus::View;
            }
        }
        for v in pinned {
            let hit = self.strip_hits[v.unit as usize].clone();
            if let Some(p) = hit.pane.filter(|&p| p != v.front) {
                self.dispatch(Command::ActivatePane(p));
            }
            // A press on its button gives the pane the focus (the strip's
            // WM_LBUTTONDOWN, `0x1005f612`: SetFocus on the button's pane;
            // VM: clicked out, it stays out with the pointer over the view).
            if hit.pressed {
                self.copy_focus = crate::app::CopyFocus::Pane(v.unit);
            }
            // The hide check tests the cursor against the pane's rect
            // joined with its divider's and its button's by position
            // (`0x10087a4a`: GetCursorPos, PtInRect), not by which window
            // is on top there.
            let (pane, divider) = (self.dock_areas[v.unit as usize], self.dock_dividers[v.unit as usize]);
            // The original's pane starts at the window's top, so its check
            // covers that too: a side or top pane's region reaches up there
            // although ours is drawn from the caption row's bottom. With a
            // top strip beside a side pane the original's starts at the
            // caption row's bottom, not the window's top (#128, VM: the pointer
            // resting in the caption row above the pane lets it slide in).
            let edge = self.docks.edge(v.unit);
            let top_strip = self.strip_inset[crate::docking::Edge::Top as usize] > 0.0;
            let from_top = !(edge == crate::docking::Edge::Bottom || (top_strip && edge.vertical()));
            let pane = pane.map(|a| if from_top { egui::Rect::from_min_max(egui::pos2(a.left(), ctx.screen_rect().top().min(a.top())), a.max) } else { a });
            let inside = pointer.is_some_and(|q| pane.is_some_and(|a| a.contains(q)) || divider.is_some_and(|a| a.contains(q)) || hit.buttons.iter().any(|b| b.contains(q)));
            // Nor while the pane holds the focus: the check returns at once
            // while its active flag (+0x2c8) is set — at each idle update
            // the focus window is the pane or a child of it (`0x10089d7d`,
            // GetFocus / IsChild); a press in its diagram or grid gives it.
            let focused = self.focus_group() == Some(v.unit);
            // Nor while its dock-bar tooltip is visible, nor a tooltip of
            // its property grid (`0x10087a4a`); the divider being dragged
            // holds it too (the cursor stays on the divider).
            let tip = crate::tips::visible_anchor(ctx).is_some_and(|a| pane.is_some_and(|p| p.contains_rect(a)));
            let held = menu_up || focused || tip || self.divider_drag[v.unit as usize];
            self.docks[v.unit].auto_hide_frame(now, hit.on, hit.pressed, inside, held);
            if let Some(dt) = self.docks[v.unit].wake_in(now) {
                wake = Some(wake.map_or(dt, |w| w.min(dt)));
            }
        }
        // egui starts a requested frame a frame's time early: the frame
        // after a timer comes at its time, not just before it.
        if let Some(dt) = wake {
            let lead = ctx.input(|i| f64::from(i.predicted_dt));
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(dt + lead));
        }
    }

    /// Slid-out auto-hide groups, over the content beside their strip: a
    /// side pane spans the frame's height below the caption row (the
    /// original's starts under its glass), the top and bottom ones its whole
    /// width; one pane of the group, with no tab strip, and a divider on the
    /// side facing the content (VM).
    fn slid_out_units(&mut self, ctx: &egui::Context, views: &[DockView]) {
        use crate::docking::{Edge, Place, DIVIDER_PX};
        let screen = ctx.screen_rect();
        // A side or top pane starts below the caption row, not under its glass
        // as the original's does (its caption's pin and close would lie under
        // the window's own buttons): an improvement, docs/IMPROVEMENTS.md.
        let cap = screen.top() + crate::ribbon::caption_h();
        let divider = crate::dpi::s(DIVIDER_PX);
        self.dock_areas = [None; 4];
        self.dock_dividers = [None; 4];
        self.slid_ranges = [None; 4];
        for v in views.iter().copied() {
            let d = self.docks[v.unit];
            if d.place != Place::AutoHide || d.slide <= 0.0 {
                self.divider_drag[v.unit as usize] = false;
                // egui lays a new area out once unseen (its sizing pass):
                // that pass goes to an empty one off the screen, so the
                // pane's first frame out is drawn.
                let id = egui::Id::new(("fv_slide", v.unit));
                if d.place == Place::AutoHide && ctx.memory(|m| m.area_rect(id)).is_none() {
                    egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(egui::pos2(-1.0e4, -1.0e4)).constrain(false).default_size(egui::Vec2::ZERO).fade_in(false).show(ctx, |_| {});
                }
                continue;
            }
            let edge = self.docks.edge(v.unit);
            let ext = d.slide_extent.unwrap_or_else(|| v.natural());
            let s = self.dock_strips[v.unit as usize];
            let k = d.slide;
            // Along the pane's edge it spans the frame; across it, up to
            // its strip, as far as the frame leaves room.
            let rect = match edge {
                Edge::Right => {
                    let right = s.map_or(screen.right(), |s| s.left());
                    let w = ext.min(right - screen.left() - divider) * k;
                    egui::Rect::from_min_max(egui::pos2(right - w, cap), egui::pos2(right, s.map_or(screen.bottom(), |s| s.bottom())))
                }
                Edge::Left => {
                    let left = s.map_or(screen.left(), |s| s.right());
                    let w = ext.min(screen.right() - left - divider) * k;
                    egui::Rect::from_min_max(egui::pos2(left, cap), egui::pos2(left + w, s.map_or(screen.bottom(), |s| s.bottom())))
                }
                Edge::Bottom => {
                    let bottom = s.map_or(screen.bottom(), |s| s.top());
                    let h = ext.min(bottom - cap - divider) * k;
                    egui::Rect::from_min_max(egui::pos2(screen.left(), bottom - h), egui::pos2(screen.right(), bottom))
                }
                Edge::Top => {
                    let h = ext.min(screen.bottom() - cap - divider) * k;
                    egui::Rect::from_min_max(egui::pos2(screen.left(), cap), egui::pos2(screen.right(), cap + h))
                }
            };
            // On a pixel, as egui places an area.
            let ppp = ctx.pixels_per_point();
            let rect = egui::Rect::from_min_size((rect.min.to_vec2() * ppp).round().to_pos2() / ppp, rect.size());
            let div = match edge {
                Edge::Right => egui::Rect::from_min_max(egui::pos2(rect.left() - divider, rect.top()), egui::pos2(rect.left(), rect.bottom())),
                Edge::Left => egui::Rect::from_min_max(egui::pos2(rect.right(), rect.top()), egui::pos2(rect.right() + divider, rect.bottom())),
                Edge::Bottom => egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() - divider), egui::pos2(rect.right(), rect.top())),
                Edge::Top => egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom()), egui::pos2(rect.right(), rect.bottom() + divider)),
            };
            self.dock_areas[v.unit as usize] = Some(rect);
            self.dock_dividers[v.unit as usize] = Some(div);
            let (c0, g0) = (self.pane_canvas.len(), self.pane_grid.len());
            let one = DockView { strip: false, ..v };
            self.slide_area(ctx, egui::Id::new(("fv_slide", v.unit)), rect, |app, ui| app.unit_body(ui, one));
            self.slid_ranges[v.unit as usize] = Some([c0, self.pane_canvas.len(), g0, self.pane_grid.len()]);
            // The divider: dragged, it resizes the pane (VM: the new size
            // stays when the pane is shown again).
            let r = self.slide_divider(ctx, egui::Id::new(("fv_slide_div", v.unit)), div, edge.vertical());
            self.divider_drag[v.unit as usize] = r.dragged();
            if r.dragged() {
                let delta = r.drag_delta();
                let grow = match edge {
                    Edge::Right => -delta.x,
                    Edge::Left => delta.x,
                    Edge::Bottom => -delta.y,
                    Edge::Top => delta.y,
                };
                let room = match edge {
                    Edge::Right | Edge::Left => screen.width(),
                    Edge::Bottom => screen.height() - crate::ribbon::caption_h(),
                    Edge::Top => screen.height(),
                } - divider - 2.0 * crate::dpi::s(23.0);
                let now = d.slide_extent.unwrap_or_else(|| v.natural());
                self.docks[v.unit].slide_extent = Some((now + grow).clamp(crate::docking::SLIDE_MIN_PX, room.max(crate::docking::SLIDE_MIN_PX)));
                ctx.request_repaint();
            }
        }
        // The window's own buttons stay at the window's edge (VM) beside a
        // right strip and on the caption row under a top strip: drawn here,
        // where no panel clips them.
        if self.own_caption && (self.strip_inset[1] > 0.0 || self.strip_inset[2] > 0.0) {
            let row = self.caption_buttons_row(ctx);
            let modal = self.modal_up;
            let area = egui::Area::new(egui::Id::new("fv_caption_buttons")).order(egui::Order::Foreground).fixed_pos(row.min).constrain(false).fade_in(false).show(ctx, |ui| {
                ui.allocate_ui_with_layout(row.size(), egui::Layout::right_to_left(egui::Align::Center), |ui| ui.add_enabled_ui(!modal, crate::ribbon::caption_buttons));
            });
            ctx.move_to_top(area.response.layer_id);
        }
    }

    /// The window's three buttons' row: the caption row at the right edge.
    fn caption_buttons_row(&self, ctx: &egui::Context) -> egui::Rect {
        let screen = ctx.screen_rect();
        egui::Rect::from_min_max(egui::pos2(screen.right() - 3.0 * crate::dpi::s(46.0), screen.top()), egui::pos2(screen.right(), screen.top() + crate::ribbon::caption_h()))
    }

    /// A slid-out pane: drawn over the content in `rect`. The area is exactly `rect`: a pane part-way out lays out
    /// wider than its rect, and an area sized by its content (pushed back
    /// into the screen as it grows) would cover the strip and take its
    /// hover.
    fn slide_area(&mut self, ctx: &egui::Context, id: egui::Id, rect: egui::Rect, body: impl FnOnce(&mut App, &mut egui::Ui)) {
        // `rect` on a pixel (as egui places an area); no fade-in (the pane
        // only slides).
        let modal = self.modal_up;
        egui::Area::new(id).order(slid_order(modal)).fixed_pos(rect.min).constrain(false).default_size(rect.size()).fade_in(false).show(ctx, |ui| {
            // The area claims its rect for clicks before its content runs,
            // with the last frame's size: sliding in, a size that still
            // reaches over the strip — claimed again with this frame's rect
            // (the last claim of an id wins; from the same corner, one rect
            // holds the other, so egui sees no clash).
            ui.interact(rect, id.with("move"), egui::Sense::click());
            // The pane's window fills the rect (VM: no margin, no outline
            // around it; #126).
            let fill = ui.visuals().panel_fill;
            ui.painter().rect_filled(rect, 0.0, fill);
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            body(self, &mut inner);
            ui.expand_to_include_rect(rect);
            // MFC's frame, which a modal dialog disables, is the pane's
            // parent: a widget over the whole pane, the last of its layer,
            // takes every click and drag while the dialog is up (as the
            // floating window's).
            if modal {
                ui.interact(rect, id.with("fv_slide_disabled"), egui::Sense::click_and_drag());
            }
        });
    }

    /// The divider beside a slid-out pane: the splitter's brush, a drag
    /// along the strip's axis across it.
    fn slide_divider(&mut self, ctx: &egui::Context, id: egui::Id, rect: egui::Rect, vertical: bool) -> egui::Response {
        let brush = self.skin.splitter();
        let modal = self.modal_up;
        egui::Area::new(id)
            .order(slid_order(modal))
            .fixed_pos(rect.min)
            .constrain(false)
            .default_size(rect.size())
            .fade_in(false)
            .show(ctx, |ui| {
                ui.painter().rect_filled(rect, 0.0, brush);
                let r = ui.interact(rect, id.with("drag"), if modal { egui::Sense::hover() } else { egui::Sense::drag() });
                if r.hovered() || r.dragged() {
                    ctx.set_cursor_icon(if vertical { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
                }
                ui.expand_to_include_rect(rect);
                r
            })
            .inner
    }

    /// A dragged floating group follows the pointer while smart docking
    /// (DT_SMART, VS2008 look) shows the four frame-edge markers and, over
    /// another docked group's pane or the document area, the five-way
    /// group; the marker under the pointer is hot and shows the drop
    /// preview; released on it the group docks there, elsewhere it stays
    /// floating (`0x100d7313`–`0x100d7445`, `0x10136b50`).
    fn dock_drag_frame(&mut self, ctx: &egui::Context) {
        use crate::docking::{Drop, Edge, Place, UNITS};
        let Some((unit, grab)) = self.dock_drag else { return };
        let (down, pos) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.latest_pos()));
        let down = down || self.dock_drag_hold;
        let client = self.dock_client;
        let docs = ctx.available_rect();
        // The edge markers on the frame's client — the whole window with
        // its 8 px invisible frame: the ribbon draws the caption (VM,
        // maximised: the top one over the caption row 8 px from the
        // screen's top, the side ones centred on the window's height).
        let mut markers: Vec<(egui::Rect, Drop)> = crate::docking::edge_markers(ctx.screen_rect().expand(8.0)).to_vec();
        // The docked group under the pointer (its slot's pane rect).
        let target = pos.and_then(|p| UNITS.into_iter().filter(|&t| t != unit && self.docks[t].place == Place::Docked).find_map(|t| self.dock_rects[t as usize].filter(|r| r.contains(p)).map(|r| (t, r))));
        let host = target.map(|(_, r)| r);
        match (host, pos) {
            (Some(h), _) => markers.extend(crate::docking::five_way(h, true)),
            (_, Some(p)) if docs.contains(p) => markers.extend(crate::docking::five_way(docs, false)),
            _ => {}
        }
        // With Ctrl held nothing docks: the move handler returns at once
        // (`OnMoveMiniFrame` `0x1008c860`), no expected rect (`0x1008c49b`),
        // no drop (`0x1008a2cd`) — the window just moves.
        let ctrl = ctx.input(|i| i.modifiers.ctrl);
        let hot = pos.filter(|_| !ctrl).and_then(|p| crate::docking::hit_marker(&markers, p));
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("fv_dock_markers")));
        // The drop preview (`CalcExpectedDockedRect` `0x1008c45d`): along a
        // frame edge (outer) or the document area (inner) as thick as the
        // floating window, beside a pane min(that, half the pane), into a
        // pane the pane less a tab strip.
        let fsize = self.docks[unit].float_rect.map_or(egui::vec2(200.0, 200.0), |r| r.size());
        if let Some(d) = hot {
            let along = |r: egui::Rect, e: Edge, x: f32| match e {
                Edge::Left => egui::Rect::from_min_max(r.min, egui::pos2(r.left() + x, r.bottom())),
                Edge::Right => egui::Rect::from_min_max(egui::pos2(r.right() - x, r.top()), r.max),
                Edge::Top => egui::Rect::from_min_max(r.min, egui::pos2(r.right(), r.top() + x)),
                Edge::Bottom => egui::Rect::from_min_max(egui::pos2(r.left(), r.bottom() - x), r.max),
            };
            let fill = crate::theme::alpha(60, 110, 200, 60);
            let stroke = egui::Stroke::new(2.0f32, egui::Color32::from_rgb(60, 110, 200));
            match (d, host) {
                (Drop::Edge(e, outer), _) => {
                    let base = if outer { client } else { docs };
                    painter.rect(along(base, e, if e.vertical() { fsize.x } else { fsize.y }), 0.0, fill, stroke);
                }
                (Drop::Beside(e), Some(h)) => {
                    painter.rect(along(h, e, crate::docking::dock_extent(d, fsize, h)), 0.0, fill, stroke);
                }
                (Drop::Tab, Some(h)) => {
                    // The target less a bottom tab strip with a 40 px tab
                    // 10 px in (`0x100962d3`).
                    let strip = 20.0;
                    painter.rect(egui::Rect::from_min_max(h.min, egui::pos2(h.right(), h.bottom() - strip)), 0.0, fill, stroke);
                    let tab = egui::Rect::from_min_size(egui::pos2(h.left() + 10.0, h.bottom() - strip), egui::vec2(40.0f32.min(h.width() - 14.0), strip));
                    painter.rect(tab, 0.0, fill, stroke);
                }
                _ => {}
            }
        }
        for (r, d) in &markers {
            let on = hot == Some(*d) && pos.is_some_and(|p| r.contains(p));
            dock_marker(&painter, *r, *d, on);
        }
        if down {
            ctx.set_cursor_icon(egui::CursorIcon::Move);
            // While a marker is hot only the preview moves.
            if let (Some(p), None) = (pos, hot) {
                let d = &mut self.docks[unit];
                d.float_rect = d.float_rect.map(|r| egui::Rect::from_min_size(p - grab, r.size()));
            }
            ctx.request_repaint();
        } else {
            if let Some(d) = hot {
                let area = match (d, host) {
                    (Drop::Edge(_, true), _) => client,
                    (Drop::Beside(_), Some(h)) => h,
                    _ => docs,
                };
                let front = self.panes.front(self.docks.members(unit));
                self.docks.drop_on(unit, d, target.map(|(t, _)| t), crate::docking::dock_extent(d, fsize, area));
                // A tab dropped in comes to the front (`AttachToTabWnd`,
                // bSetActive).
                if let (Drop::Tab, Some(p)) = (d, front) {
                    self.dispatch(Command::ActivatePane(p));
                }
            }
            self.dock_drag = None;
        }
    }

    /// A stats-panel drag released over a diagram pane (`0x4415b0`, kinds
    /// 2 / 4 / 8): each dragged channel not on the diagram is put on it
    /// (35000+i; the circle diagram only takes Ohm / VA channels,
    /// CompareNoCase) and the document is marked modified.
    pub fn pane_drop(&mut self, ui: &egui::Ui, rect: egui::Rect, pane: Pane) {
        let ctx = ui.ctx();
        self.drop_targets.push((pane, rect));
        let (released, pos) = ctx.input(|i| (i.pointer.primary_released(), i.pointer.latest_pos()));
        let Some(drag) = egui::DragAndDrop::payload::<crate::lanes::ChanDrag>(ctx) else { return };
        // The window under the pointer takes the drop: not a diagram that a
        // floating or slid-out window lies over (clone).
        let over = pos.filter(|p| rect.contains(*p) && !ctx.layer_id_at(*p).is_some_and(|l| l != ui.layer_id() && is_pane_window_layer(l)));
        let Some(_) = over else { return };
        // Another record's channel numbers mean nothing here (#74).
        let mine = self.active_doc().is_some_and(|d| d.serial == drag.doc);
        self.drag_cursor = Some(if mine { crate::lanes::MOVE_DROP_CURSOR } else { egui::CursorIcon::NoDrop });
        if !released || !mine {
            return;
        }
        let Some(drag) = egui::DragAndDrop::take_payload::<crate::lanes::ChanDrag>(ctx) else { return };
        self.drop_channels(&drag.keys(), pane);
    }

    /// The drop's work on a pane (`0x4415b0`): every dragged channel whose
    /// bit for the pane is clear gets the pane's command 35000+i, which sets
    /// it (`0x46f100`, `0x48f590`; recompute, modified mark) — file and
    /// formula channels alike, the vector and frequency panes without any
    /// test of the channel (#103). The circle diagram only takes channels of
    /// an Ohm / VA unit that are not pending (CompareNoCase on the canonical
    /// unit, `0x414c30`; none in ADC mode). A digital channel never reaches a
    /// pane: the drag payload stops at `doc+0x1d8` (analog-kind records).
    pub fn drop_channels(&mut self, keys: &[ChanKey], pane: Pane) {
        let Some(d) = self.active_doc_mut() else { return };
        let mut changed = false;
        for &k in keys {
            let (pending, unit, flag) = match k {
                ChanKey::Analog(i) if i < d.chans.len() => {
                    let unit = if d.chans[i].adc_counts { String::new() } else { d.osc.analog[i].unit.to_lowercase() };
                    let c = &mut d.chans[i];
                    (false, unit, [&mut c.on_freq, &mut c.on_vector, &mut c.on_circle])
                }
                ChanKey::Computed(j) if j < d.computed.len() && !d.pending_digital(j) => {
                    let pending = d.computed[j].formula == crate::merge_ui::PENDING;
                    let c = &mut d.computed[j];
                    (pending, c.unit.trim().to_lowercase(), [&mut c.on_freq, &mut c.on_vector, &mut c.on_circle])
                }
                _ => continue,
            };
            let [freq, vector, circle] = flag;
            let flag = match pane {
                Pane::Frequency => freq,
                Pane::Vector => vector,
                Pane::Circle if !pending && (unit == "ohm" || unit == "va") => circle,
                _ => continue,
            };
            if !*flag {
                *flag = true;
                changed = true;
            }
        }
        if changed {
            d.set.modified = true;
        }
    }

    /// The MDI area: what the docking panes and bars leave (under the
    /// parameters bar, beside the diagrams — seen on the VM). The tab strip
    /// (20 px on a light band, 1 px edge) sits inside it, and the active
    /// document is framed 2 px in its tab's colour.
    fn view_oscillogram(&mut self, ctx: &egui::Context) {
        let pal = self.skin.palette();
        // With no document the MDI client shows its own colour, nothing
        // else (MAINWND/MainClientArea; (164, 195, 235) in blue, seen on
        // the VM).
        if self.docs.is_empty() {
            egui::CentralPanel::default().frame(egui::Frame::none().fill(pal.work_bg)).show(ctx, |ui| self.lanes_ui(ui));
            return;
        }
        if let Some((serial, at)) = self.mdi_drop.take() {
            if let Some(i) = self.docs.iter().position(|d| d.serial == serial) {
                self.drop_doc_tab(i, at);
            }
        }
        self.touch_front();
        // A press in another group activates its document first (MFC's
        // mouse activation), and the view takes the click as well (VM: the
        // визир moves).
        let press = ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.interact_pos()).flatten());
        // Not on a group's ▾ button: that press activates nothing (clone —
        // the title stays, the button holds the focus).
        let on_list_button = |p: egui::Pos2| self.list_buttons.iter().any(|r| r.contains(p));
        if let Some(p) = press.filter(|&p| ctx.layer_id_at(p).is_some_and(|l| l.order == egui::Order::Background) && !on_list_button(p)) {
            if let Some(h) = self.mdi_rects.iter().position(|r| r.contains(p)) {
                if self.active_doc().map(|d| d.mdi_group) != Some(h) {
                    if let Some(f) = self.group_front(h) {
                        self.active = Some(f);
                        self.touch_front();
                    }
                }
            }
        }
        self.doc_tab_rects = vec![egui::Rect::NOTHING; self.docs.len()];
        // Between the groups each one's resize edge: the document area's
        // border line, then 4 px of the face (VM, Blue: (213, 228, 242);
        // the other looks not measured).
        let face = pal.bands.map_or(pal.ribbon_top, |b| b.bar_bottom);
        egui::CentralPanel::default().frame(egui::Frame::none().fill(face)).show(ctx, |ui| {
            self.layout_groups(ui.max_rect());
            let n = self.group_count();
            self.mdi_strips = vec![egui::Rect::NOTHING; n];
            self.list_buttons = vec![egui::Rect::NOTHING; n];
            self.doc_list_open = false;
            // The active group last: what the frame keeps of a view this
            // pass is the active document's.
            let bold = self.active;
            let active_g = self.active_doc().map_or(0, |d| d.mdi_group);
            for g in (0..n).filter(|&g| g != active_g).chain([active_g]) {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(self.mdi_rects[g]).id_salt(("fv_mdi_group", g)));
                let front = self.group_front(g);
                let real = std::mem::replace(&mut self.active, front);
                self.doc_group_ui(&mut child, g, bold, g == active_g);
                // Unless the group's own strip changed it.
                if self.active == front {
                    self.active = real;
                }
            }
            for g in 0..n.saturating_sub(1) {
                self.group_edge(ui, g);
            }
        });
    }

    /// Tab group `g` in its part of the MDI area, its front document taken
    /// as the active one: the tab strip, the child frame in the tab's
    /// colour, the view (or the preview, in the active group), the border.
    fn doc_group_ui(&mut self, ui: &mut egui::Ui, g: usize, bold: Option<usize>, front_is_active: bool) {
        let pal = self.skin.palette();
        let edge = egui::Stroke::new(1.0f32, pal.ribbon_top.gamma_multiply(0.92));
        egui::Frame::none().fill(pal.ribbon_top).stroke(edge).inner_margin(egui::Margin::same(1.0)).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        // 20 px, 29 / 33 at 125 % / 150 % (VM: MFC's tab height from the
        // system font).
        egui::TopBottomPanel::top(if g == 0 { egui::Id::new("fv_doc_tabs") } else { egui::Id::new(("fv_doc_tabs", g)) })
            .exact_height(crate::dpi::measured(20.0, 29.0, 33.0))
            .show_separator_line(false)
            .frame(egui::Frame::none())
            .show_inside(ui, |ui| {
                let r = ui.max_rect();
                let band = pal.mdi_tabs.unwrap_or([pal.ribbon_top.gamma_multiply(0.95), pal.ribbon_top.lerp_to_gamma(egui::Color32::WHITE, 0.5)]);
                let mut mesh = egui::Mesh::default();
                for (p, c) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().zip([band[0], band[0], band[1], band[1]]) {
                    mesh.colored_vertex(p, c);
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(mesh));
                // The row's bottom line (VM, Blue: (185, 208, 237)).
                if let Some(bands) = pal.bands {
                    ui.painter().hline(r.x_range(), r.bottom() - 0.5, egui::Stroke::new(1.0f32, bands.mdi_border));
                }
                self.doc_tabs(ui, g, bold);
            });
        let (r, g, b) = crate::app::TAB_COLORS[self.active_doc().and_then(|d| d.tab_color).unwrap_or(0)];
        egui::CentralPanel::default()
            // The child frame in the tab's colour — 2 px, 1 px at the
            // top — round the view's sunken client edge (VM: (160,
            // 160, 160) / (105, 105, 105) at the top and left, white /
            // (227, 227, 227) at the right and bottom).
            .frame(egui::Frame::none().inner_margin(egui::Margin { left: 2.0, right: 2.0, top: 1.0, bottom: 2.0 }))
            .show_inside(ui, |ui| {
                // The frame's colour runs along the diagonal from
                // the tab's colour at the top left to 35 lighter per
                // channel at the bottom right (VM: (255, 220, 120) →
                // (255, 254, 155)).
                let m = ui.max_rect();
                let fr = egui::Rect::from_min_max(m.min - egui::vec2(2.0, 1.0), m.max + egui::vec2(2.0, 2.0));
                let base = egui::Color32::from_rgb(r, g, b);
                let shade = |t: f32| {
                    let up = |c: u8| (c as f32 + 35.0 * t).round().min(255.0) as u8;
                    egui::Color32::from_rgb(up(r), up(g), up(b))
                };
                let (w, h) = (fr.width(), fr.height());
                let d2 = (w * w + h * h).max(1.0);
                let mut mesh = egui::Mesh::default();
                for (pt, c) in [(fr.left_top(), base), (fr.right_top(), shade(w * w / d2)), (fr.right_bottom(), shade(1.0)), (fr.left_bottom(), shade(h * h / d2))] {
                    mesh.colored_vertex(pt, c);
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(mesh));
                let edge = ui.available_rect_before_wrap();
                egui::Frame::none().fill(ui.visuals().panel_fill).inner_margin(egui::Margin::same(2.0)).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    if self.print.preview.is_some() && front_is_active {
                        self.preview_ui(ui);
                    } else {
                        self.lanes_ui(ui);
                    }
                });
                let p = ui.painter();
                let line = |c: u8| egui::Stroke::new(1.0f32, egui::Color32::from_gray(c));
                let (l, t, r_, b_) = (edge.left(), edge.top(), edge.right(), edge.bottom());
                p.px_hline(l..=r_ - 1.0, t, line(160));
                p.px_vline(l, t..=b_ - 1.0, line(160));
                p.px_hline(l + 1.0..=r_ - 2.0, t + 1.0, line(105));
                p.px_vline(l + 1.0, t + 1.0..=b_ - 2.0, line(105));
                p.px_hline(l..=r_ - 1.0, b_ - 1.0, line(255));
                p.px_vline(r_ - 1.0, t..=b_ - 1.0, line(255));
                p.px_hline(l + 1.0..=r_ - 2.0, b_ - 2.0, line(227));
                p.px_vline(r_ - 2.0, t + 1.0..=b_ - 2.0, line(227));
            });
        // The document area's 1 px border, inside (VM, Office 2007
        // Blue: (185, 208, 237); the other looks keep the band's
        // shade, not measured).
        if let Some(bands) = pal.bands {
            let r = ui.max_rect().expand(1.0);
            let b = egui::Stroke::new(1.0f32, bands.mdi_border);
            let p = ui.painter();
            p.px_hline(r.left()..=r.right() - 1.0, r.top(), b);
            p.px_hline(r.left()..=r.right() - 1.0, r.bottom() - 1.0, b);
            p.px_vline(r.left(), r.top()..=r.bottom() - 1.0, b);
            p.px_vline(r.right() - 1.0, r.top()..=r.bottom() - 1.0, b);
        }
        });
    }

    /// The resize edge after group `g`: dragged, a bar follows the pointer
    /// and the two neighbours take the new split when it is let go.
    fn group_edge(&mut self, ui: &mut egui::Ui, g: usize) {
        let win = self.mdi_wins[g];
        let v = self.mdi_vertical;
        let strip = if v { egui::Rect::from_min_max(egui::pos2(win.right() - 5.0, win.top()), win.max) } else { egui::Rect::from_min_max(egui::pos2(win.left(), win.bottom() - 5.0), win.max) };
        // The document area's border colour for the line (VM, Blue: (185,
        // 208, 237)), the face after it; the other looks not measured.
        let pal = self.skin.palette();
        let line = pal.bands.map_or(pal.ribbon_top.gamma_multiply(0.92), |b| b.mdi_border);
        let face = pal.bands.map_or(pal.ribbon_top, |b| b.bar_bottom);
        ui.painter().rect_filled(strip, 0.0, face);
        if v {
            ui.painter().px_vline(strip.left(), strip.y_range(), egui::Stroke::new(1.0f32, line));
        } else {
            ui.painter().px_hline(strip.x_range(), strip.top(), egui::Stroke::new(1.0f32, line));
        }
        let resp = ui.interact(strip, egui::Id::new(("fv_mdi_edge", g)), egui::Sense::drag());
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(if v { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
        }
        let moved = |r: &egui::Response| r.interact_pointer_pos().zip(ui.ctx().input(|i| i.pointer.press_origin())).map(|(p, o)| if v { p.x - o.x } else { p.y - o.y });
        if resp.dragged() {
            if let Some(d) = moved(&resp) {
                // The halftone bar (`0x1013f622`), kept within 40 px of the
                // neighbours' far ends.
                let span = win.union(self.mdi_wins[g + 1]);
                let bar = if v {
                    let x = (strip.left() + d).clamp(span.left() + 40.0, span.right() - 40.0 - 5.0);
                    egui::Rect::from_min_size(egui::pos2(x, strip.top()), strip.size())
                } else {
                    let y = (strip.top() + d).clamp(span.top() + 40.0, span.bottom() - 40.0 - 5.0);
                    egui::Rect::from_min_size(egui::pos2(strip.left(), y), strip.size())
                };
                halftone(ui.ctx(), bar, None);
            }
        }
        if resp.drag_stopped() {
            if let Some(d) = moved(&resp) {
                self.drag_split(g, d);
            }
        }
    }
    // ---- Круговая / Дифференциальная: characteristics + impedance overlay ----
    /// Circle / differential pane: the block and settings grid above the
    /// plot behind a splitter (200 px, 50 px .. height − 150), as the
    /// original's pane.
    /// Returns where the property grid was right-clicked: the grid leaves
    /// WM_CONTEXTMENU to its pane, which shows its own menu (VM).
    fn char_ui(&mut self, ui: &mut egui::Ui, pane: Pane) -> Option<egui::Pos2> {
        let h = ui.available_height();
        // `Properties Height` of «Circle Diagram» / «Diff Diagram»
        // (default 200, persisted on every re-layout — `0x4376e0`). Only a
        // drag clamps it to [50, pane − 150] (`0x437900` / `0x443390`); the
        // layout takes it as it is, however small the pane (VM, L9.399).
        let key = format!("{}\\Properties Height", if pane == Pane::Differential { "Diff Diagram" } else { "Circle Diagram" });
        let stored = self.store.view.get(&key).and_then(|v| v.parse::<f32>().ok()).unwrap_or(200.0);
        let grid_id = egui::Id::new(("char_grid", pane));
        let dragging = ui.ctx().dragged_id() == Some(grid_id.with("__resize"));
        let top = (h - 150.0).max(50.0);
        let grid = egui::TopBottomPanel::top(grid_id)
            .resizable(true)
            .show_separator_line(false)
            .default_height(stored)
            .height_range(50.0..=if dragging { top } else { top.max(stored) })
            // The property grid is a white list (seen on the VM), from the
            // pane's client corner, a white column and row after it.
            .frame(egui::Frame::none().fill(egui::Color32::WHITE).inner_margin(egui::Margin { left: 0.0, right: 1.0, top: 0.0, bottom: 1.0 }))
            .show_inside(ui, |ui| {
                // The grid fills its splitter part however few its rows. It
                // scrolls by whole 17 px rows with the window scroll bar
                // (Windows 11's, as the tree's) taking 17 px of its width
                // whenever the rows overflow (VM: the value column — and
                // «Тип блока»'s ▾ — narrower by it), the wheel three rows a
                // notch. The rows' height is last frame's.
                let inner = ui.max_rect();
                ui.allocate_rect(inner, egui::Sense::hover());
                let sid = egui::Id::new(("char_grid_scroll", pane));
                let (mut off, content_h): (f32, f32) = ui.data(|d| d.get_temp(sid)).unwrap_or((0.0, 0.0));
                // (the rows and the scroll bar at the system DPI: `dpi`)
                let (row, bar_w) = (crate::omp_ui::grid_row_h(), crate::dpi::s(17.0));
                let page = (inner.height() / row).floor() * row;
                // The window scroll bar's range as MFC sets it (clone: with
                // P rows in view and N in all, the wheel and the bar reach
                // N - P + 2 rows down, a page of P - 2 sizing the thumb);
                // it is there when the rows overflow the window.
                let bar_page = (page - 2.0 * row).max(row);
                let max_off = if content_h > page { (content_h - bar_page).max(0.0) } else { 0.0 };
                ui.data_mut(|d| d.insert_temp(grid_rows_id(pane), (page / row) as i32));
                // PgUp / PgDn scroll a page (P - 1 rows) with the selection.
                if let Some(rows) = ui.data_mut(|d| d.remove_temp::<i32>(grid_page_id(pane))) {
                    off = (off + rows as f32 * row).clamp(0.0, max_off);
                }
                if ui.rect_contains_pointer(inner) {
                    for dy in ui.input(|i| crate::lanes::wheel_deltas(&i.events)) {
                        off -= dy.signum() * 3.0 * row;
                    }
                }
                // A row selected by a key comes into view (EnsureVisible).
                if let Some(j) = ui.data_mut(|d| d.remove_temp::<usize>(ui_grid_ensure_id(pane))) {
                    off = ensure_row_visible(off, page, row, j);
                }
                let content = if max_off > 0.0 { egui::Rect::from_min_max(inner.min, egui::pos2(inner.right() - bar_w, inner.bottom())) } else { inner };
                if max_off > 0.0 {
                    let bar = egui::Rect::from_min_max(egui::pos2(content.right(), inner.top()), inner.max);
                    if let Some(v) = crate::lanes::scrollbar(ui, bar, false, off, max_off, bar_page, row, page) {
                        off = v;
                    }
                }
                off = ((off / row).round() * row).clamp(0.0, max_off);
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(content.min - egui::vec2(0.0, off), egui::vec2(content.width(), f32::INFINITY))));
                child.set_clip_rect(content.intersect(ui.clip_rect()));
                self.char_side_panel(&mut child, pane);
                let h = child.min_rect().height();
                // The name / value divider on down the empty list (VM).
                let below = content.top() - off + h;
                if below < content.bottom() {
                    use crate::oscgeom::PixelLines;
                    let x = content.left() + (content.width() / 2.0).floor();
                    ui.painter().with_clip_rect(content).px_vline(x, below..=content.bottom(), egui::Stroke::new(1.0f32, egui::Color32::from_rgb(185, 208, 237)));
                }
                ui.data_mut(|d| d.insert_temp(sid, (off, h)));
            });
        let now = grid.response.rect.height().round();
        if (now - stored).abs() >= 1.0 && self.persist {
            self.store.view.insert(key, format!("{now}"));
            self.store.save();
        }
        self.pane_grid.push(grid.response.rect);
        let grid_menu = (ui.rect_contains_pointer(grid.response.rect) && ui.input(|i| i.pointer.secondary_clicked())).then(|| ui.ctx().pointer_interact_pos()).flatten();
        let strip_col = self.skin.splitter();
        egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
            // The splitter (VM): a 1 px (100, 100, 100) line under the grid,
            // then 5 px of the frame's splitter brush (`0x438110`).
            ui.spacing_mut().item_spacing.y = 0.0;
            let (strip, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
            ui.painter().rect_filled(strip, 0.0, strip_col);
            ui.painter().px_hline(strip.x_range(), strip.top(), egui::Stroke::new(1.0f32, egui::Color32::from_gray(100)));
            // The plane window: its border at the sides and bottom (its top
            // is the line above the strip), a white column before the
            // toolbar (VM); the plane drawn in its client area.
            // The plane window begins under the line: its top 5 rows are
            // the splitter's brush, which its layout counts (VM: the cells
            // from line + 11, not + 16).
            let outer = ui.available_rect_before_wrap();
            let client = egui::Rect::from_min_max(egui::pos2(outer.left() + 1.0, strip.top() + 1.0), egui::pos2(outer.right() - 2.0, outer.bottom() - 1.0));
            self.pane_canvas.push((pane, client));
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(client).layout(*ui.layout()));
            self.plane_ui(&mut inner);
            ui.advance_cursor_after_rect(outer);
            let border = outer.with_max_x(outer.right() - 1.0);
            ui.painter().px_vline(border.right(), border.y_range(), egui::Stroke::new(1.0f32, egui::Color32::WHITE));
            window_border(ui, border, [false, true, true, true]);
        });
        grid_menu
    }

    /// The block property grid (`BlockPropertyGrid`, an MFC property grid
    /// with no header, text (40, 40, 40), the name column half the width):
    /// «Тип блока» (string 3483) choosing the block (`0x407cc0`),
    /// then one group per characteristic — its row carries, at the right, a
    /// 12 × 12 box framed in the characteristic's colour with a tick while
    /// it is shown (`0x407dc0`; a click there toggles it, `0x4075c0`) — and
    /// its parameters as text rows; an edit re-evaluates the whole block.
    fn char_side_panel(&mut self, ui: &mut egui::Ui, pane: Pane) {
        const TEXT: egui::Color32 = egui::Color32::from_rgb(40, 40, 40);
        // CMFCPropertyGridCtrl as the VM shows it (Office 2007 Blue): 17 px
        // rows (16 + a (185, 208, 237) line), the name column half the
        // width, names 6 px in (a group's and its parameters' 22 px, past
        // the expand box), the selected row's name cell filled (180, 201,
        // 233) with the focus, (213, 228, 242) without.
        let w = ui.available_width();
        let name_w = (w / 2.0).floor();
        // The rows follow the grid's font, the same MFC global font as
        // «Параметры ОМП»'s grid: 17 / 23 / 27 px at 100 / 125 / 150 %
        // (clone, #173; the dialogs' base units gave 21 / 26).
        let row_h = crate::omp_ui::grid_row_h();
        // Rows edge to edge (VM: a 17 px pitch).
        ui.spacing_mut().item_spacing.y = 0.0;
        let line = egui::Stroke::new(1.0f32, egui::Color32::from_rgb(185, 208, 237));
        let sel_id = ui.id().with("fv_grid_sel");
        let focus_id = ui.id().with("fv_grid_focus");
        // The rebuild selects «Тип блока» (`0x407750`).
        let selected: egui::Id = ui.data(|d| d.get_temp(sel_id)).unwrap_or(sel_id.with("type"));
        let focused = ui.data(|d| d.get_temp::<bool>(focus_id)).unwrap_or(false);
        let sel_fill = if focused { egui::Color32::from_rgb(180, 201, 233) } else { egui::Color32::from_rgb(213, 228, 242) };
        let select: std::cell::Cell<Option<egui::Id>> = std::cell::Cell::new(None);
        // One grid row: the name cell and the rect left for the value.
        let row = |ui: &mut egui::Ui, key: egui::Id, name: &str, bold: bool, indent: f32, group: bool| -> (egui::Rect, egui::Rect) {
            let (r, _) = ui.allocate_exact_size(egui::vec2(w, row_h), egui::Sense::hover());
            let body = egui::Rect::from_min_max(r.min, egui::pos2(r.right(), r.bottom() - 1.0));
            ui.painter().px_hline(r.x_range(), r.bottom() - 1.0, line);
            let nr = egui::Rect::from_min_max(body.min, egui::pos2(body.left() + name_w, body.bottom()));
            if key == selected {
                ui.painter().rect_filled(if group { body } else { nr }, 0.0, sel_fill);
            }
            if ui.interact(egui::Rect::from_min_max(egui::pos2(nr.left() + indent, nr.top()), nr.max), key.with("name"), egui::Sense::click()).clicked() {
                select.set(Some(key));
            }
            let font = egui::FontId::proportional(crate::dpi::font(12.0));
            let g = ui.painter().layout_no_wrap(name.to_string(), font, TEXT);
            let at = egui::pos2(nr.left() + indent, nr.center().y - g.size().y / 2.0);
            let clip = if group { body } else { nr };
            ui.painter().with_clip_rect(clip).galley(at, g.clone(), TEXT);
            if bold {
                ui.painter().with_clip_rect(clip).galley(at + egui::vec2(0.5, 0.0), g, TEXT);
            }
            if !group {
                ui.painter().px_vline(nr.right(), body.y_range(), line);
            }
            (r, egui::Rect::from_min_max(egui::pos2(nr.right() + 1.0, body.top()), body.max))
        };
        ui.spacing_mut().item_spacing.y = 0.0;
        // «Тип блока»: its text, and on the selected row the list's button
        // (15 px at the value's right); the list drops from the cell.
        let type_key = sel_id.with("type");
        // The names' ink where the original's is (VM, «БМРЗ - Защита
        // трансформаторов (с 2025 г.в.)»): «Тип блока» 6 px in, the groups'
        // and parameters' 22–23 (our «Т» starts 2.5 px into its cell).
        let (_, vr) = row(ui, type_key, "Тип блока", false, 3.0, false);
        // The rows in order, for the keys.
        let mut order: Vec<(egui::Id, GridRow)> = vec![(type_key, GridRow::Type)];
        let prev = self.block_idx;
        let names: Vec<String> = self.library.blocks.iter().map(|b| b.name.clone()).collect();
        let shown = self.block_text.clone().or_else(|| names.get(self.block_idx).cloned()).unwrap_or_default();
        let combo = ui.interact(vr, ui.id().with("fv_grid_type_value"), egui::Sense::click());
        // «Тип блока» stepped by ↓ / ↑ (below): its in-place edit, the text
        // selected, until Enter, Esc or a click; holds the text before.
        let type_edit_id = ui.id().with("fv_grid_type_edit");
        let type_edit: Option<Option<String>> = ui.data(|d| d.get_temp(type_edit_id));
        let at_text = egui::pos2(vr.left() + 4.0, vr.center().y);
        let font12 = egui::FontId::proportional(crate::dpi::font(12.0));
        if type_edit.is_some() {
            let gal = ui.painter().layout_no_wrap(shown.clone(), font12.clone(), egui::Color32::WHITE);
            let r = egui::Align2::LEFT_CENTER.anchor_size(at_text, gal.size());
            ui.painter().with_clip_rect(vr).rect_filled(r, 0.0, egui::Color32::from_rgb(0, 120, 215));
            ui.painter().with_clip_rect(vr).galley(r.min, gal, egui::Color32::WHITE);
        } else {
            ui.painter().with_clip_rect(vr).text(at_text, egui::Align2::LEFT_CENTER, &shown, font12, TEXT);
        }
        let list_id = ui.id().with("fv_grid_type_list");
        if selected == type_key {
            // Office 2007 Blue (VM): a 14 px button shading (226, 238, 255)
            // → (196, 221, 255) down the row, a 5 / 3 / 1 px (86, 125, 177)
            // arrow 6 rows down; the other looks a grey box.
            let blue = self.skin.palette().bands.is_some_and(|b| b.panel_sep.is_some());
            let bt = egui::Rect::from_min_max(egui::pos2(vr.right() - crate::dpi::s(if blue { 14.0 } else { 15.0 }), vr.top()), vr.max);
            let hot = combo.hovered() && ui.ctx().pointer_hover_pos().is_some_and(|q| bt.contains(q));
            // Pressed (VM): while its list is down and after the list
            // closes by a key, until the next press — a (251, 140, 61)
            // left edge, the face (252, 153, 61) → (254, 181, 89).
            let down_id = list_id.with("down");
            let open = ui.data(|d| d.get_temp::<bool>(list_id.with("open")).unwrap_or(false));
            if !open && ui.input(|i| i.pointer.any_pressed()) {
                ui.data_mut(|d| d.remove::<bool>(down_id));
            }
            let down = open || ui.data(|d| d.get_temp::<bool>(down_id).unwrap_or(false));
            if blue {
                let (top, bottom) = if hot || down { (egui::Color32::from_rgb(252, 153, 61), egui::Color32::from_rgb(254, 181, 89)) } else { (egui::Color32::from_rgb(226, 238, 255), egui::Color32::from_rgb(196, 221, 255)) };
                ui.painter().add(crate::ribbon::gradient_mesh(bt, &[(0.5, top), (bt.height() - 0.5, bottom)]));
                if hot || down {
                    ui.painter().rect_filled(egui::Rect::from_min_size(bt.min, egui::vec2(1.0, bt.height())), 0.0, egui::Color32::from_rgb(251, 140, 61));
                }
                let ink = egui::Color32::from_rgb(86, 125, 177);
                for (k, (dx, w)) in [(5.0, 5.0), (6.0, 3.0), (7.0, 1.0)].into_iter().enumerate() {
                    let y = bt.top() + 6.0 + k as f32;
                    ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(bt.left() + dx, y), egui::vec2(w, 1.0)), 0.0, ink);
                }
            } else {
                ui.painter().rect_filled(bt, 0.0, if hot { egui::Color32::from_rgb(253, 170, 78) } else { egui::Color32::from_gray(240) });
                let c = bt.center();
                ui.painter().add(egui::Shape::convex_polygon(vec![egui::pos2(c.x - 3.0, c.y - 1.0), egui::pos2(c.x + 3.0, c.y - 1.0), egui::pos2(c.x, c.y + 2.0)], TEXT, egui::Stroke::NONE));
            }
            if combo.clicked() && ui.ctx().pointer_interact_pos().is_some_and(|q| bt.contains(q)) {
                let open = !ui.data(|d| d.get_temp::<bool>(list_id.with("open")).unwrap_or(false));
                ui.data_mut(|d| d.insert_temp(list_id.with("open"), open));
                if open {
                    ui.data_mut(|d| d.insert_temp(down_id, true));
                }
            }
        }
        if combo.clicked() {
            select.set(Some(type_key));
        }
        // The in-place combo's drop-down list: a Win32 list the value
        // cell's width under it, long names cut at its edge (VM).
        if ui.data(|d| d.get_temp::<bool>(list_id.with("open")).unwrap_or(false)) {
            // Under the row's line, from 3 px left of the value (VM: x 1791
            // for a value from 1794, row 258 under a 241–256 row).
            let under = egui::Rect::from_min_max(egui::pos2(vr.left() - 3.0, vr.top()), egui::pos2(vr.right(), vr.bottom() + 1.0));
            if let Some(i) = crate::dialogs::combo_popup_in(ui, under, list_id, &names, Some(self.block_idx), crate::omp_ui::grid_row_h() - 4.0, egui::FontId::proportional(crate::dpi::font(12.0))) {
                self.block_idx = i;
            }
        }
        // A click in any value area marks the document modified, «Тип
        // блока»'s included (`0x4075c0` → `0x407c40`).
        let mut touched = combo.clicked();
        if self.block_idx != prev {
            self.selected_char = None;
            // The rebuild evaluates, then saves (`0x407cc0` → `0x407750`).
            self.ensure_block_state();
            self.save_block();
            // The rebuild (`0x407750`) ends with ExpandAll: every group open.
            if let Some(blk) = self.library.blocks.get(self.block_idx) {
                let b = self.block_idx;
                ui.data_mut(|d| (0..blk.characteristics.len()).for_each(|ci| d.remove::<bool>(ui.id().with(("fv_grid_open", b, ci)))));
            }
        }
        let b = self.block_idx;
        let groups: Vec<(usize, String, Vec<String>)> = self
            .library
            .blocks
            .get(b)
            .map(|blk| blk.characteristics.iter().enumerate().map(|(ci, ch)| (ci, ch.name.clone(), ch.parameters.iter().map(|p| p.name.clone()).collect())).collect())
            .unwrap_or_default();
        let (mut changed, mut vis_changed) = (false, false);
        for (ci, name, params) in &groups {
            let open_id = ui.id().with(("fv_grid_open", b, *ci));
            let open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(true);
            let (r, _) = row(ui, sel_id.with(("group", *ci)), name, true, crate::dpi::s(22.0), true);
            order.push((sel_id.with(("group", *ci)), GridRow::Group(open_id, open, *ci)));
            // The expand box (9 × 9 at +4, top + 3: a (145, 145, 145)
            // frame round a light gradient, the sign in (75, 99, 167)) and
            // the visibility box.
            // (centred in a taller row at > 100 %)
            let toggle = egui::Rect::from_min_size(egui::pos2(r.left() + 4.0, r.top() + 3.0 + ((r.height() - 17.0) / 2.0).floor()), egui::vec2(9.0, 9.0));
            crate::lanes::gradient(ui.painter(), toggle, egui::Color32::from_rgb(250, 251, 251), egui::Color32::from_gray(227));
            ui.painter().px_rect_stroke(toggle, 0.0, egui::Stroke::new(1.0f32, egui::Color32::from_gray(145)));
            let sign = egui::Stroke::new(1.0f32, egui::Color32::from_rgb(75, 99, 167));
            let c = toggle.center();
            ui.painter().px_hline(c.x - 2.0..=c.x + 2.0, c.y, sign);
            if !open {
                ui.painter().px_vline(c.x, c.y - 2.0..=c.y + 2.0, sign);
            }
            // The grid drops a double-click on a group row (`0x407640`): the
            // second click of a pair does nothing.
            let once = |r: egui::Response| r.clicked() && !r.double_clicked();
            if once(ui.interact(toggle.expand(2.0), ui.id().with(("fv_grid_tg", b, *ci)), egui::Sense::click())) {
                ui.data_mut(|d| d.insert_temp(open_id, !open));
            }
            if select.get() == Some(sel_id.with(("group", *ci))) {
                self.selected_char = Some(*ci);
            }
            if let Some(st) = self.chars.get_mut(&(b, *ci)) {
                // `0x407dc0`: Rectangle(right − 14, top + 3, right − 2,
                // top + 15) with a pen in the operate colour and a brush in
                // the palette's fourth column; when visible, image 0 of
                // bitmap 470 (13 × 13) — a dark slate tick, ours drawn.
                let (ir, ig, ib) = crate::diagram::CHAR_PALETTE[*ci % 9][3];
                let bx = egui::Rect::from_min_size(egui::pos2(r.right() - 15.0, r.top() + 2.0 + ((r.height() - 17.0) / 2.0).floor()), egui::vec2(12.0, 12.0));
                ui.painter().rect_filled(bx, 0.0, egui::Color32::from_rgb(ir, ig, ib));
                ui.painter().px_rect_stroke(bx, 0.0, egui::Stroke::new(1.0f32, st.color));
                if st.visible {
                    let tick = egui::Stroke::new(1.5f32, egui::Color32::from_rgb(71, 92, 148));
                    ui.painter().line_segment([bx.left_center() + egui::vec2(2.5, 0.0), egui::pos2(bx.left() + 5.0, bx.bottom() - 3.0)], tick);
                    ui.painter().line_segment([egui::pos2(bx.left() + 5.0, bx.bottom() - 3.0), egui::pos2(bx.right() - 2.5, bx.top() + 2.5)], tick);
                }
                let value_area = egui::Rect::from_min_max(egui::pos2(r.left() + name_w, r.top()), r.max);
                if once(ui.interact(value_area, ui.id().with(("fv_grid_vis", b, *ci)), egui::Sense::click())) {
                    st.visible = !st.visible;
                    vis_changed = true;
                    touched = true;
                }
            }
            if !open {
                continue;
            }
            for (pi, p) in params.iter().enumerate() {
                let key = sel_id.with(("param", *ci, pi));
                let (_, vr) = row(ui, key, p, false, crate::dpi::s(22.0), false);
                if let Some(st) = self.chars.get_mut(&(b, *ci)) {
                    let text = st.settings.entry(p.clone()).or_default();
                    // The edit in its cell, its line centred 4 px in as
                    // «Тип блока»'s, on a child that leaves the row's
                    // 17 px allocation alone (`put` moved the next row to
                    // the edit's bottom: 16–19 px pitches, VM 17).
                    let font = egui::FontId::proportional(crate::dpi::font(12.0));
                    let top = ((vr.height() - ui.fonts(|f| f.row_height(&font))) / 2.0).max(0.0);
                    let margin = egui::Margin { left: 4.0, right: 4.0, top, bottom: 0.0 };
                    let mut cell = ui.new_child(egui::UiBuilder::new().max_rect(vr));
                    let resp = cell.add_sized(vr.size(), egui::TextEdit::singleline(text).frame(false).text_color(TEXT).font(font).margin(margin));
                    order.push((key, GridRow::Param(resp.id, *ci, pi)));
                    // The stock in-place edit (CMFCPropertyGridCtrl, INFERRED):
                    // Esc restores the text, a commit (Enter, focus lost)
                    // reaches OnPropertyChanged (`0x407580`) only when the
                    // text changed.
                    let before_id = resp.id.with("fv_grid_before");
                    if resp.clicked() || resp.gained_focus() {
                        select.set(Some(key));
                    }
                    if resp.gained_focus() {
                        // Kept when a typed character opened the edit.
                        let t = text.clone();
                        ui.data_mut(|d| {
                            if d.get_temp::<String>(before_id).is_none() {
                                d.insert_temp(before_id, t);
                            }
                        });
                    }
                    if resp.lost_focus() {
                        let before: Option<String> = ui.data_mut(|d| d.remove_temp(before_id));
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            if let Some(t) = before {
                                *text = t;
                            }
                        } else {
                            changed |= before.as_ref() != Some(text);
                        }
                    }
                }
            }
        }
        // CMFCPropertyGridCtrl's keys while it has the focus and no edit is
        // open (INFERRED from MFC): ↑ / ↓ / Home / End move the selection;
        // on a group Enter toggles it, + / → open and − / ← close it; on a
        // parameter F4 or Enter opens its edit, a typed character opens it
        // with that character in place of the text.
        let editing = ui.memory(|m| m.focused().is_some() || m.any_popup_open()) || ui.data(|d| d.get_temp::<bool>(list_id.with("open")).unwrap_or(false));
        if focused && !editing && !self.modal_up {
            let cur = select.get().unwrap_or(selected);
            let at = order.iter().position(|(k, _)| *k == cur).unwrap_or(0);
            let (keys, typed) = ui.input(|i| {
                let typed: Option<String> = i.events.iter().find_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None });
                let k = |key| i.key_pressed(key);
                ([k(egui::Key::ArrowUp), k(egui::Key::ArrowDown), k(egui::Key::Home), k(egui::Key::End), k(egui::Key::Enter), k(egui::Key::F4), k(egui::Key::ArrowRight) || k(egui::Key::Plus), k(egui::Key::ArrowLeft) || k(egui::Key::Minus)], typed)
            });
            let [up, down, home, end, enter, f4, open_key, close_key] = keys;
            let last = order.len().saturating_sub(1);
            // PgUp / PgDn (clone): the selection and the window both move
            // by a page, P - 1 rows (P: the rows in view), kept inside the
            // rows and the scroll range.
            let (pg_up, pg_down) = ui.input(|i| (i.key_pressed(egui::Key::PageUp), i.key_pressed(egui::Key::PageDown)));
            let page_rows = ui.data(|d| d.get_temp::<i32>(grid_rows_id(pane))).unwrap_or(2).saturating_sub(1).max(1) as usize;
            // «Тип блока» alone (no block's rows): ↓ / ↑ step its value in
            // an in-place edit and Enter keeps the text — the block is not
            // loaded (VM: «Нет» → «БМРЗ - Защита линий» → «… генераторов»,
            // after Enter no rows and no characteristics); Esc restores it.
            // With rows below, the keys move the selection.
            let sole_type = order.len() == 1 && matches!(order.get(at), Some((_, GridRow::Type)));
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if let Some(before) = type_edit.clone() {
                if enter || esc {
                    if esc {
                        self.block_text = before;
                    }
                    ui.data_mut(|d| d.remove::<Option<String>>(type_edit_id));
                }
            }
            let to = if sole_type && (up || down) {
                let cur = self.block_text.as_ref().and_then(|t| names.iter().position(|n| n == t)).unwrap_or(self.block_idx);
                let next = if down { (cur + 1).min(names.len().saturating_sub(1)) } else { cur.saturating_sub(1) };
                if type_edit.is_none() {
                    let before = self.block_text.clone();
                    ui.data_mut(|d| d.insert_temp(type_edit_id, before));
                }
                self.block_text = (next != self.block_idx).then(|| names[next].clone());
                None
            } else if pg_up || pg_down {
                ui.data_mut(|d| d.insert_temp(grid_page_id(pane), if pg_down { page_rows as i32 } else { -(page_rows as i32) }));
                Some(if pg_down { (at + page_rows).min(last) } else { at.saturating_sub(page_rows) })
            } else if up {
                Some(at.saturating_sub(1))
            } else if down {
                Some((at + 1).min(last))
            } else if home {
                Some(0)
            } else if end {
                Some(last)
            } else {
                None
            };
            if let Some(j) = to {
                select.set(Some(order[j].0));
                // MFC scrolls a key-selected row into view (EnsureVisible):
                // the grid's scroll reads the row's place.
                ui.data_mut(|d| d.insert_temp(ui_grid_ensure_id(pane), j));
                // As a click on the group's row does.
                if let GridRow::Group(.., ci) = order[j].1 {
                    self.selected_char = Some(ci);
                }
            } else if let Some((_, r)) = order.get(at) {
                match *r {
                    GridRow::Group(open_id, open, _) if enter || open_key || close_key => {
                        let want = if enter { !open } else { open_key };
                        ui.data_mut(|d| d.insert_temp(open_id, want));
                    }
                    GridRow::Param(edit, ..) if enter || f4 => ui.memory_mut(|m| m.request_focus(edit)),
                    GridRow::Param(edit, ci, pi) => {
                        let name = groups.iter().find(|g| g.0 == ci).and_then(|g| g.2.get(pi)).cloned();
                        if let (Some(t), Some(name), Some(st)) = (typed.filter(|t| !t.trim().is_empty()), name, self.chars.get_mut(&(b, ci))) {
                            // The text before the edit, for Esc and the
                            // commit's comparison.
                            let before = st.settings.insert(name, t).unwrap_or_default();
                            ui.data_mut(|d| d.insert_temp(edit.with("fv_grid_before"), before));
                            ui.memory_mut(|m| m.request_focus(edit));
                        }
                    }
                    // F4 on «Тип блока» does nothing (VM; MFC drops the list
                    // only from an open in-place edit).
                    _ => {}
                }
            }
        }
        if let Some(k) = select.get() {
            ui.data_mut(|d| d.insert_temp(sel_id, k));
        }
        // The grid has the focus after a press inside it, until one outside.
        // A press ends «Тип блока»'s in-place edit, its text kept.
        let area = ui.min_rect();
        if let Some(q) = ui.input(|i| i.pointer.any_pressed().then_some(i.pointer.interact_pos()).flatten()) {
            ui.data_mut(|d| d.insert_temp(focus_id, area.contains(q)));
            ui.data_mut(|d| d.remove::<Option<String>>(type_edit_id));
        }
        if touched {
            if let Some(d) = self.active_doc_mut() {
                d.set.modified = true;
            }
        }
        // «Тип блока»'s value click saves the active block too (slot 94),
        // over a saved name the library lacks; the cell keeps its text (VM:
        // stepped to «… линии», Enter, a click and ↓ give «… генераторов»).
        if self.block_idx != prev {
            self.block_text = None;
        } else if combo.clicked() {
            self.save_block();
        }
        // A parameter commit (`0x407580`) re-evaluates, then the block is
        // saved into the document (`0x408cf0`), as after a visibility click.
        if changed {
            self.recompute_block();
        }
        if changed || vis_changed {
            self.save_block();
        }
    }

    // ---- Быстрый анализ ----
}

/// The rows in view of the grid's window, passed to its keys.
fn grid_rows_id(pane: Pane) -> egui::Id {
    egui::Id::new(("fv_grid_rows_in_view", pane))
}

/// A PgUp / PgDn's page (rows, signed), passed from the grid's keys to its
/// scroll.
fn grid_page_id(pane: Pane) -> egui::Id {
    egui::Id::new(("fv_grid_page_scroll", pane))
}

/// Where the key-selected row is passed from the grid's rows to its scroll.
fn ui_grid_ensure_id(pane: Pane) -> egui::Id {
    egui::Id::new(("fv_grid_ensure_visible", pane))
}

/// The scroll offset that shows grid row `j` (`row` px each) in a window of
/// `page` px, moved as little as possible (CMFCPropertyGridCtrl's
/// EnsureVisible): up to the row's top, or down to its bottom.
fn ensure_row_visible(off: f32, page: f32, row: f32, j: usize) -> f32 {
    let (top, bottom) = (j as f32 * row, (j as f32 + 1.0) * row);
    if top < off {
        top
    } else if bottom > off + page {
        bottom - page
    } else {
        off
    }
}

/// What an auto-hide strip's buttons of one pinned group saw this frame.
#[derive(Debug, Clone, Default)]
pub struct StripHit {
    /// The pointer is on one of the group's buttons.
    pub on: bool,
    /// A press on one.
    pub pressed: bool,
    /// The pane whose button the pointer is on.
    pub pane: Option<Pane>,
    pub buttons: Vec<egui::Rect>,
}

/// A group as it shows: its own unit, its visible tabs (by pane), the
/// front one, and whether it has a tab strip (more than one visible).
#[derive(Debug, Clone, Copy)]
pub(crate) struct DockView {
    unit: crate::docking::Unit,
    mask: [bool; 4],
    front: Pane,
    strip: bool,
}

impl DockView {
    /// Its create size across its edge: 500, Частотная alone 200.
    fn natural(&self) -> f32 {
        if self.mask == [false, false, false, true] {
            200.0
        } else {
            500.0
        }
    }
}

/// The egui window id of a floating group's mini-frame.
fn floating_pane_id(unit: crate::docking::Unit) -> egui::Id {
    egui::Id::new(("fv_floating_pane", unit))
}

/// The layer a floating group's window is on.
pub fn floating_pane_layer(unit: crate::docking::Unit) -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, floating_pane_id(unit))
}

/// Whether `l` is a floating pane's window (an ordinary tool window, not a
/// dialog).
pub fn is_floating_pane_layer(l: egui::LayerId) -> bool {
    crate::docking::UNITS.into_iter().any(|u| l == floating_pane_layer(u))
}

/// The group whose slid-out window `l` is (it lies over the view like a
/// floating one).
pub fn slid_pane_unit(l: egui::LayerId) -> Option<crate::docking::Unit> {
    crate::docking::UNITS.into_iter().find(|&u| [slid_order(false), slid_order(true)].into_iter().any(|o| l == egui::LayerId::new(o, egui::Id::new(("fv_slide", u)))))
}

/// The order a slid-out pane's window is drawn in: over the view; under a
/// modal dialog (the Middle order) while one is up, so the dialog lies above
/// it.
fn slid_order(modal: bool) -> egui::Order {
    if modal {
        egui::Order::PanelResizeLine
    } else {
        egui::Order::Foreground
    }
}

/// Whether `l` is a floating group's window or a slid-out one's (a pane's own
/// window, over the view).
pub fn is_pane_window_layer(l: egui::LayerId) -> bool {
    is_floating_pane_layer(l) || is_slid_pane_layer(l)
}

/// Whether `l` is a slid-out auto-hidden group's window.
pub fn is_slid_pane_layer(l: egui::LayerId) -> bool {
    slid_pane_unit(l).is_some()
}

fn pane_dim(p: Pane) -> fvb_core::model::Dimension {
    if p == Pane::Differential {
        fvb_core::model::Dimension::Dif
    } else {
        fvb_core::model::Dimension::Ohm
    }
}

pub(crate) fn pane_title(p: Pane) -> &'static str {
    match p {
        Pane::Vector => "Векторная диаграмма",
        Pane::Circle => "Круговая диаграмма",
        Pane::Differential => "Дифференциальная диаграмма",
        Pane::Frequency => "Частотная диаграмма",
    }
}

/// What a caption did this frame.
/// A docking container: the panes sit 2 px inside it, against the
/// splitters (MFC's docking bars), not egui's 8 px panel margin; a group
/// beside another inside the container adds none.
/// A diagram window's WS_BORDER: 1 px of COLOR_WINDOWFRAME (VM: (100, 100,
/// 100)) on the sides asked for — top, left, right, bottom.
fn window_border(ui: &egui::Ui, r: egui::Rect, [top, left, right, bottom]: [bool; 4]) {
    let st = egui::Stroke::new(1.0f32, egui::Color32::from_gray(100));
    let p = ui.painter();
    if top {
        p.px_hline(r.left()..=r.right() - 1.0, r.top(), st);
    }
    if bottom {
        p.px_hline(r.left()..=r.right() - 1.0, r.bottom() - 1.0, st);
    }
    if left {
        p.px_vline(r.left(), r.top()..=r.bottom() - 1.0, st);
    }
    if right {
        p.px_vline(r.right() - 1.0, r.top()..=r.bottom() - 1.0, st);
    }
}

/// A docked container: 4 px of the frame's splitter brush on the side
/// facing the document area (VM, Office 2007 Blue: (191, 219, 255) between
/// the document's border and the pane), nothing on the others.
/// A docked panel's size: as last dragged by its [`grip`], else `default`.
fn grip_size(ctx: &egui::Context, id: egui::Id, default: f32, range: std::ops::RangeInclusive<f32>) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(id.with("size"))).unwrap_or(default).clamp(*range.start(), *range.end())
}

/// A docked panel's 4 px divider strip — on the inner side of the panel
/// on `edge` of its slot — as its only resize grip: MFC's pane divider is
/// that strip (L9.109); egui's own grab reaches 5 px past the edge, into
/// the neighbour (VM: a press 2 px inside the oscillogram view starts a
/// lane drag in the original, not a resize). Call first in the panel;
/// returns the size a drag just gave the panel.
fn grip(ui: &mut egui::Ui, id: egui::Id, edge: crate::docking::Edge, size: f32, range: std::ops::RangeInclusive<f32>) -> Option<f32> {
    use crate::docking::Edge;
    let c = ui.max_rect();
    let (strip, sign) = match edge {
        Edge::Right => (egui::Rect::from_min_max(egui::pos2(c.left() - 4.0, c.top()), egui::pos2(c.left(), c.bottom())), egui::vec2(-1.0, 0.0)),
        Edge::Left => (egui::Rect::from_min_max(egui::pos2(c.right(), c.top()), egui::pos2(c.right() + 4.0, c.bottom())), egui::vec2(1.0, 0.0)),
        Edge::Bottom => (egui::Rect::from_min_max(egui::pos2(c.left(), c.top() - 4.0), egui::pos2(c.right(), c.top())), egui::vec2(0.0, -1.0)),
        Edge::Top => (egui::Rect::from_min_max(egui::pos2(c.left(), c.bottom()), egui::pos2(c.right(), c.bottom() + 4.0)), egui::vec2(0.0, 1.0)),
    };
    let resp = ui.interact(strip, id.with("grip"), egui::Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(if sign.x != 0.0 { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
    }
    if resp.dragged() {
        let d = resp.drag_delta() * sign;
        let new = (size + d.x + d.y).clamp(*range.start(), *range.end());
        ui.ctx().data_mut(|m| m.insert_temp(id.with("size"), new));
        return Some(new);
    }
    None
}

/// A smart-docking marker in the VS2008 look (VM): a rounded box, white
/// at the top to light grey, a grey edge, and a small blue-grey triangle on
/// the box's inner side pointing to its edge; the centre one a small pane.
/// Hot: a light blue face.
fn dock_marker(p: &egui::Painter, r: egui::Rect, d: crate::docking::Drop, on: bool) {
    use crate::docking::{Drop, Edge};
    use egui::{pos2, Color32};
    let r = r.shrink(2.0);
    let (top, bottom) = if on { (Color32::from_rgb(232, 240, 252), Color32::from_rgb(196, 214, 240)) } else { (Color32::from_rgb(252, 252, 252), Color32::from_rgb(222, 224, 228)) };
    let mut mesh = egui::Mesh::default();
    for (q, c) in [(r.left_top(), top), (r.right_top(), top), (r.right_bottom(), bottom), (r.left_bottom(), bottom)] {
        mesh.colored_vertex(q, c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
    p.rect_stroke(r, 4.0, egui::Stroke::new(1.0f32, Color32::from_rgb(166, 170, 178)));
    let ink = Color32::from_rgb(112, 132, 188);
    let (c, h) = (r.center(), 5.0);
    let edge = match d {
        Drop::Edge(e, _) | Drop::Beside(e) => Some(e),
        Drop::Tab => None,
    };
    let tri = match edge {
        Some(Edge::Top) => [pos2(c.x - h, r.bottom() - 7.0), pos2(c.x + h, r.bottom() - 7.0), pos2(c.x, r.bottom() - 7.0 - h)],
        Some(Edge::Bottom) => [pos2(c.x - h, r.top() + 7.0), pos2(c.x + h, r.top() + 7.0), pos2(c.x, r.top() + 7.0 + h)],
        Some(Edge::Left) => [pos2(r.right() - 7.0, c.y - h), pos2(r.right() - 7.0, c.y + h), pos2(r.right() - 7.0 - h, c.y)],
        Some(Edge::Right) => [pos2(r.left() + 7.0, c.y - h), pos2(r.left() + 7.0, c.y + h), pos2(r.left() + 7.0 + h, c.y)],
        None => {
            let w = egui::Rect::from_center_size(c, egui::vec2(16.0, 14.0));
            p.rect(w, 0.0, Color32::WHITE, egui::Stroke::new(1.0f32, ink));
            p.rect_filled(egui::Rect::from_min_size(w.min, egui::vec2(16.0, 3.0)), 0.0, ink);
            return;
        }
    };
    p.add(egui::Shape::convex_polygon(tri.to_vec(), ink, egui::Stroke::NONE));
}

fn dock_frame(strip: egui::Color32, edge: crate::docking::Edge) -> egui::Frame {
    use crate::docking::Edge;
    let m = |e: Edge| if e == edge { 4.0 } else { 0.0 };
    // The strip is on the inner side: a right-docked container's left.
    let margin = egui::Margin { left: m(Edge::Right), right: m(Edge::Left), top: m(Edge::Bottom), bottom: m(Edge::Top) };
    egui::Frame::none().fill(strip).inner_margin(margin)
}

#[derive(Debug, Clone, Copy, Default)]
struct CaptionAct {
    close: bool,
    pin: bool,
    double: bool,
    dragging: bool,
    /// The primary button is held, pressed on the caption itself (not on
    /// one of its buttons): a drag that has not moved far yet counts.
    armed: bool,
    /// A right-click: the pane menu at that point.
    menu: Option<egui::Pos2>,
    /// A press on the caption or its buttons (any button).
    pressed: bool,
}

/// A docking-pane caption: the title, the pin (`pinned` None when the pane
/// floats — a mini-frame has none) and a close button that only hides the
/// pane; the caption itself takes double-clicks and drags. As the VM shows
/// it (Office 2007 blue): 20 px with a 1 px light top edge on a flat band,
/// the title 4 px in in the 9 pt UI font, the close and pin glyphs flat
/// 1 px drawings 11 / 26 px from the right edge.
fn pane_caption(ui: &mut egui::Ui, pal: &crate::theme::Palette, title: &str, pinned: Option<bool>) -> CaptionAct {
    use crate::dpi::s;
    // 29 / 33 px at 125 % / 150 % (VM: MFC's caption from the system
    // font), its font, inset and buttons grown with it.
    let h = crate::dpi::measured(20.0, 29.0, 33.0);
    let mut act = CaptionAct::default();
    // The caption takes double-clicks and drags; registered first, so its
    // buttons, drawn over it, keep their clicks.
    // The pane's window right under it (VM: no gap).
    ui.spacing_mut().item_spacing.y = 0.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
    let resp = ui.interact(rect, ui.id().with("fv_pane_caption"), egui::Sense::click_and_drag());
    // A double-click: two clicks within the system double-click time
    // (Windows' default 500 ms; WM_LBUTTONDBLCLK on the caption).
    let id = ui.id().with("fv_pane_caption_last");
    if resp.clicked() {
        let now = ui.input(|i| i.time);
        let last = ui.data(|d| d.get_temp::<f64>(id));
        act.double = last.is_some_and(|t| now - t <= 0.5);
        ui.data_mut(|d| if act.double { d.remove::<f64>(id) } else { d.insert_temp(id, now) });
    }
    // The buttons (the close at 11 px from the right edge, the pin at 26)
    // never start a drag: a press that began on one is no caption press.
    let spots = [Some(rect.right() - s(11.0)), pinned.map(|_| rect.right() - s(26.0))];
    let cy = rect.center().y.floor() + 0.5;
    let on_button = ui.input(|i| i.pointer.press_origin()).is_some_and(|p| spots.iter().flatten().any(|&x| egui::Rect::from_center_size(egui::pos2(x, cy), egui::vec2(s(15.0), s(15.0))).contains(p)));
    act.armed = resp.is_pointer_button_down_on() && !on_button && ui.input(|i| i.pointer.primary_down());
    act.dragging = resp.dragged_by(egui::PointerButton::Primary) && !on_button;
    act.pressed = resp.contains_pointer() && ui.input(|i| i.pointer.any_pressed());
    if resp.secondary_clicked() {
        act.menu = resp.interact_pointer_pos();
    }
    let band = pal.pane_caption();
    let p = ui.painter();
    let ink = match pal.pane_cap {
        // The older looks' captions (VM): a gradient or flat, their text.
        Some([top, bottom, text]) => {
            crate::lanes::gradient(p, rect, top, bottom);
            text
        }
        None => {
            p.rect_filled(rect, 0.0, band);
            // The line over it: the bar gradient's bottom colour (VM).
            let top = pal.bands.map_or(band.lerp_to_gamma(egui::Color32::WHITE, 0.4), |b| b.bar_bottom);
            p.px_hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0f32, top));
            pal.text
        }
    };
    let font = egui::FontId::proportional(crate::dpi::font(12.0));
    p.with_clip_rect(rect.shrink2(egui::vec2(0.0, 0.0)).with_max_x(rect.right() - s(34.0)))
        .text(egui::pos2(rect.left() + s(4.0), rect.center().y), egui::Align2::LEFT_CENTER, title, font, ink);
    let glyph = egui::Color32::from_rgb(86, 125, 177);
    let button = |ui: &mut egui::Ui, centre: egui::Pos2, salt: &str| {
        let r = egui::Rect::from_center_size(centre, egui::vec2(s(15.0), s(15.0)));
        let b = ui.interact(r, ui.id().with(("fv_pane_btn", salt)), egui::Sense::click());
        if b.hovered() {
            ui.painter().rect(r, 0.0, pal.accent_soft, egui::Stroke::new(1.0f32, pal.accent_border));
        }
        b
    };
    // ✕: two 1 px diagonals in a 7 px box.
    let c = egui::pos2(rect.right() - s(11.0), cy);
    let close = button(ui, c, "close");
    let st = egui::Stroke::new(1.0f32, glyph);
    let d = crate::dpi::half(3.5);
    ui.painter().line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], st);
    ui.painter().line_segment([c + egui::vec2(-d, d), c + egui::vec2(d, -d)], st);
    // MFC's plain tips under the pointer (mfc120rus; VM «Close» / «Auto Hide»).
    crate::tips::pointer_tip(ui, &close, crate::tips::TipImage::None, "", "Закрыть", false);
    act.close = close.clicked();
    if let Some(auto_hide) = pinned {
        // The pin: upright while docked, on its side while auto-hidden.
        let c = egui::pos2(rect.right() - s(26.0), cy);
        let pin = button(ui, c, "pin");
        let pt = ui.painter();
        if auto_hide {
            pt.px_rect_stroke(egui::Rect::from_min_max(c + egui::vec2(-s(1.0), -s(2.0)), c + egui::vec2(s(3.0), s(2.0))), 0.0, st);
            pt.px_vline(c.x - s(1.0), (c.y - s(3.0))..=(c.y + s(3.0)), st);
            pt.px_hline((c.x - s(4.0))..=(c.x - s(1.0)), c.y, st);
        } else {
            pt.px_rect_stroke(egui::Rect::from_min_max(c + egui::vec2(-s(2.0), -s(4.0)), c + egui::vec2(s(2.0), s(1.0))), 0.0, st);
            pt.px_hline((c.x - s(3.0))..=(c.x + s(3.0)), c.y + s(1.0), st);
            pt.px_vline(c.x, (c.y + s(1.0))..=(c.y + s(4.0)), st);
        }
        let tip = if auto_hide { "Закрепить" } else { "Автоматически скрывать" };
        crate::tips::pointer_tip(ui, &pin, crate::tips::TipImage::None, "", tip, false);
        act.pin = pin.clicked();
    }
    act
}

/// An auto-hide strip button in `rect`: the pane's title, written down a
/// side strip or along a top / bottom one. The front pane of its group is
/// the warm gradient (VM: (255, 243, 199) to (255, 220, 122), across a side
/// strip left to right, down a bottom one bottom to top), the others the
/// strip's own colour; every one framed (185, 208, 237).
fn strip_button(p: &egui::Painter, rect: egui::Rect, title: &str, vertical: bool, front: bool, hot: bool) {
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    let font = egui::FontId::proportional(crate::dpi::font(11.0));
    let ink = egui::Color32::from_rgb(30, 30, 30);
    let galley = p.layout_no_wrap(title.to_owned(), font, ink);
    let frame = egui::Color32::from_rgb(185, 208, 237);
    let (light, dark) = if front || hot { (egui::Color32::from_rgb(255, 243, 199), egui::Color32::from_rgb(255, 220, 122)) } else { (egui::Color32::from_rgb(213, 228, 242), egui::Color32::from_rgb(196, 220, 250)) };
    let inner = rect.shrink(1.0);
    let mut mesh = Mesh::default();
    // The light end first: the left of a side strip, the bottom of the
    // others.
    let (c, pts) = if vertical {
        ([light, dark, dark, light], [inner.left_top(), inner.right_top(), inner.right_bottom(), inner.left_bottom()])
    } else {
        ([dark, dark, light, light], [inner.left_top(), inner.right_top(), inner.right_bottom(), inner.left_bottom()])
    };
    for (pos, color) in pts.into_iter().zip(c) {
        mesh.vertices.push(Vertex { pos, uv: WHITE_UV, color });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    p.add(mesh);
    p.rect_stroke(rect, 0.0, egui::Stroke::new(1.0f32, frame));
    // A stub's title starts at its edge and is cut by it.
    let text = p.with_clip_rect(rect);
    let lead = if front { 8.0 } else { 3.0 };
    if vertical {
        let at = egui::pos2(rect.center().x + galley.size().y / 2.0, rect.top() + lead);
        text.add(egui::epaint::TextShape::new(at, galley, ink).with_angle(std::f32::consts::FRAC_PI_2));
    } else {
        text.galley(rect.left_center() + egui::vec2(lead, -galley.size().y / 2.0), galley, ink);
    }
}

/// The index of an edge in the per-edge arrays: left, right, top, bottom.
fn edge_ix(e: crate::docking::Edge) -> usize {
    e as usize
}

/// A row of the parameter grid, for its keys.
#[derive(Clone, Copy)]
enum GridRow {
    /// «Тип блока».
    Type,
    /// A characteristic: its open state's id, whether it is open, its index.
    Group(egui::Id, bool, usize),
    /// A parameter: its edit's id, its group and its place there.
    Param(egui::Id, usize, usize),
}

/// MFC's drag feedback (`DrawDragRect` with the halftone brush, XOR): a
/// checkerboard of dark pixels over `r`, or only a `frame`-wide outline of it.
pub fn halftone(ctx: &egui::Context, r: egui::Rect, frame: Option<f32>) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("fv_halftone")));
    let ink = egui::Color32::from_black_alpha(150);
    let r = egui::Rect::from_min_max(r.min.round(), r.max.round());
    let parts = match frame {
        None => vec![r],
        Some(t) => vec![
            egui::Rect::from_min_max(r.min, egui::pos2(r.right(), r.top() + t)),
            egui::Rect::from_min_max(egui::pos2(r.left(), r.bottom() - t), r.max),
            egui::Rect::from_min_max(egui::pos2(r.left(), r.top() + t), egui::pos2(r.left() + t, r.bottom() - t)),
            egui::Rect::from_min_max(egui::pos2(r.right() - t, r.top() + t), egui::pos2(r.right(), r.bottom() - t)),
        ],
    };
    let mut mesh = egui::Mesh::default();
    for p in parts {
        let (x0, y0, x1, y1) = (p.left() as i32, p.top() as i32, p.right() as i32, p.bottom() as i32);
        for y in y0..y1 {
            let mut x = x0 + (x0 + y).rem_euclid(2);
            while x < x1 {
                mesh.add_colored_rect(egui::Rect::from_min_size(egui::pos2(x as f32, y as f32), egui::vec2(1.0, 1.0)), ink);
                x += 2;
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod pane_tests {
    use super::*;

    /// Every pane arrangement renders beside the oscillogram without panicking,
    /// at a normal and at the frame's minimum size (866×500).
    #[test]
    fn panes_render_in_every_arrangement() {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        let ctx = egui::Context::default();
        let frame = |app: &mut App, size: egui::Vec2| {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                app.diagram_panes(ctx);
                app.render_tab(ctx);
            });
        };
        for size in [egui::vec2(1320.0, 860.0), egui::vec2(866.0, 500.0)] {
            for active in crate::app::Panes::GROUP {
                app.dispatch(Command::ActivatePane(active));
                for _ in 0..3 {
                    frame(&mut app, size);
                }
            }
            for p in [Pane::Frequency, Pane::Vector, Pane::Circle, Pane::Differential] {
                app.dispatch(Command::TogglePane(p));
                frame(&mut app, size);
                frame(&mut app, size);
            }
            assert!(app.panes.shown_tab().is_none() && !app.panes.frequency);
            frame(&mut app, size);
            for p in [Pane::Frequency, Pane::Vector, Pane::Circle, Pane::Differential] {
                app.dispatch(Command::TogglePane(p));
            }
        }
    }
}
