//! Diagram views (круговая/дифференциальная/векторная/частотная/быстрый анализ)
//! over the active document (ОМП lives in `omp_ui.rs`). The oscillogram (lane) view lives in
//! `lanes.rs`.

use crate::app::{App, Command, Pane, View};
use crate::diagram_ui::Window;
use crate::oscgeom::PixelLines;
use eframe::egui;

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
        self.pane_tab_rects.clear();
        self.pane_grid.clear();
        self.dock_client = ctx.available_rect();
        let dim = self.current_dimension();
        let views = self.dock_views();
        // The strips sit outermost, then the containers.
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
        self.docks.leave(unit, |u| shown[u as usize]);
    }

    /// The frame client's top-left, where a first float opens.
    fn float_origin(ctx: &egui::Context) -> egui::Pos2 {
        ctx.available_rect().min
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
            Entry::item("Плавающее", !ah, Mark::Check(place == Place::Floating), Cmd::Float),
            Entry::item("Закрепленное", !ah, Mark::Check(place == Place::Docked), Cmd::Dock),
            Entry::item("В закладке", false, Mark::None, Cmd::Tab),
            Entry::item("Автоматически скрывать", place != Place::Floating, Mark::Check(ah), Cmd::AutoHide),
            Entry::item("Скрыть", !ah, Mark::None, Cmd::Hide),
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
                if place == Place::Docked {
                    self.dock_leave(unit);
                }
                self.docks[unit].pin();
            }
            Some(Cmd::Hide) => self.dispatch(Command::TogglePane(pane)),
            _ => {}
        }
    }

    fn caption_acts(&mut self, ctx: &egui::Context, unit: crate::docking::Unit, shown: Pane, act: CaptionAct, pane: egui::Rect) {
        use crate::docking::Place;
        if act.close {
            self.dispatch(Command::TogglePane(shown));
            return;
        }
        let origin = Self::float_origin(ctx);
        if act.pin {
            if self.docks[unit].place == Place::Docked {
                self.dock_leave(unit);
            }
            self.docks[unit].pin();
        }
        if act.double {
            if self.docks[unit].place == Place::Docked {
                self.dock_leave(unit);
            }
            self.docks[unit].double_click(origin);
            return;
        }
        if act.dragging {
            let (press, now, delta) = ctx.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos(), i.pointer.delta()));
            let (Some(press), Some(now)) = (press, now) else { return };
            let dragging = self.dock_drag.is_some();
            match self.docks[unit].place {
                Place::Docked => {
                    if self.docks[unit].drag_detach(press, now, pane, origin) {
                        // A group beside it takes its slot.
                        self.dock_leave(unit);
                        let r = self.docks[unit].float_at(origin);
                        self.dock_drag = Some((unit, now - r.min));
                    }
                }
                // A floating pane's drag shows the markers too.
                Place::Floating if !dragging => {
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
        let mut tabs: Vec<(Pane, egui::Rect)> = Vec::new();
        let strip = egui::TopBottomPanel::bottom(egui::Id::new(("pane_tabs", v.unit))).show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let order = self.tab_order;
                for p in order.into_iter().filter(|&p| v.mask[p as usize]) {
                    let on = p == v.front;
                    // An id per pane: the drag follows the tab when it moves.
                    let r = ui.push_id(("fv_pane_tab", p as u8), |ui| ui.selectable_label(on, pane_title(p)).interact(egui::Sense::click_and_drag())).inner;
                    tabs.push((p, r.rect));
                    // A press (or a drag) brings the tab to the front.
                    if (r.clicked() || r.drag_started()) && !on {
                        self.dispatch(Command::ActivatePane(p));
                    }
                    if r.dragged() {
                        dragged = Some(p);
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
        if let (Some(p), true) = (dragged, tabs.len() > 1) {
            let ctx = ui.ctx().clone();
            self.tab_drag_out(&ctx, v, p, strip.response.rect);
        }
        egui::CentralPanel::default().show_inside(ui, |ui| {
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
        let act = pane_caption(ui, pane_title(p), (place != Place::Floating).then_some(place == Place::AutoHide));
        let ctx = ui.ctx().clone();
        if let Some(at) = act.menu {
            let next = self.tab_order.into_iter().find(|&q| q != p && v.mask[q as usize]);
            self.pane_menu = Some((v.unit, p, next, crate::popmenu::PopMenu::new(ctx.cumulative_pass_nr(), at)));
        }
        self.caption_acts(&ctx, v.unit, p, act, outer);
        if act.close {
            return;
        }
        match p {
            Pane::Vector => {
                self.pane_toolbar(ui, Window::Vector);
                egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
                    self.pane_canvas.push((Pane::Vector, ui.available_rect_before_wrap()));
                    self.vector_ui(ui);
                    self.pane_drop(ui, Pane::Vector);
                });
            }
            Pane::Circle | Pane::Differential => {
                // The two planes share the characteristics state: this
                // pane's is brought in.
                self.switch_diagram(pane_dim(p));
                self.pane_toolbar(ui, if p == Pane::Circle { Window::Circle } else { Window::Diff });
                self.char_ui(ui, p);
                // The differential diagram has no drop target.
                if p == Pane::Circle {
                    self.pane_drop(ui, Pane::Circle);
                }
            }
            Pane::Frequency => {
                self.pane_canvas.push((Pane::Frequency, ui.available_rect_before_wrap()));
                self.frequency_ui(ui);
                self.pane_drop(ui, Pane::Frequency);
            }
        }
    }

    /// A tab dragged out of a group's strip: past the system drag distance
    /// (SM_CXDRAG / SM_CYDRAG, 4 px, `0x10068657`) and outside the strip,
    /// `CTabbedPane::FloatTab` (`0x1013a259`) floats that pane alone under
    /// the pointer (the group's own pane hands the group to the next tab);
    /// the drag then goes on as a floating caption drag (the markers).
    fn tab_drag_out(&mut self, ctx: &egui::Context, v: DockView, pulled: Pane, strip: egui::Rect) {
        let (press, now) = ctx.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos()));
        let (Some(press), Some(now)) = (press, now) else { return };
        let d = now - press;
        if (d.x.abs() < crate::docking::DRAG_PX && d.y.abs() < crate::docking::DRAG_PX) || strip.contains(now) || self.dock_drag.is_some() {
            return;
        }
        let next = self.tab_order.into_iter().find(|&q| q != pulled && v.mask[q as usize]).unwrap_or(v.unit);
        let origin = Self::float_origin(ctx);
        let grab = egui::vec2(40.0, 10.0);
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
        let ext = hd.extent.unwrap_or(natural);
        let body = |app: &mut App, ui: &mut egui::Ui| app.dock_slot(ui, h, edge, &parents);
        match edge {
            Edge::Left | Edge::Right => {
                let p = if edge == Edge::Left { egui::SidePanel::left(id) } else { egui::SidePanel::right(id) };
                p.resizable(true).default_width(ext).width_range(260.0..=1200.0).show(ctx, |ui| body(self, ui));
            }
            Edge::Top | Edge::Bottom => {
                let p = if edge == Edge::Top { egui::TopBottomPanel::top(id) } else { egui::TopBottomPanel::bottom(id) };
                let ext = if hd.extent.is_none() && (inside.len() > 1 || natural > 200.0) { 300.0 } else { ext };
                p.resizable(true).default_height(ext).height_range(80.0..=900.0).show(ctx, |ui| body(self, ui));
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
                    let d = wd.extent.unwrap_or(w.natural()).min(full.y / 2.0);
                    p.resizable(true).default_height(d).height_range(80.0..=(full.y - 120.0).max(80.0)).show_inside(ui, |ui| self.dock_slot(ui, w, edge, parents));
                }
                Edge::Left | Edge::Right => {
                    let p = if side == Edge::Left { egui::SidePanel::left(inner) } else { egui::SidePanel::right(inner) };
                    let d = wd.extent.unwrap_or(full.x / 2.0).min(full.x / 2.0);
                    p.resizable(true).default_width(d).width_range(120.0..=(full.x - 120.0).max(120.0)).show_inside(ui, |ui| self.dock_slot(ui, w, edge, parents));
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
        let resp = egui::Window::new(pane_title(v.front))
            .id(egui::Id::new(("fv_floating_pane", unit)))
            .title_bar(false)
            .resizable(true)
            .collapsible(false)
            .current_pos(r.min)
            .default_size(r.size())
            .min_size(egui::vec2(200.0, 150.0))
            .show(ctx, |ui| {
                ui.set_min_size(ui.available_size());
                self.unit_body(ui, v);
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

    /// The auto-hide strips: a button per visible pane of each pinned
    /// group on its edge; hovering 400 ms or a click slides the group out
    /// with that pane in front.
    fn auto_hide_strips(&mut self, ctx: &egui::Context, views: &[DockView]) {
        use crate::docking::{Edge, Place};
        let now = ctx.input(|i| i.time);
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let menu_up = ctx.memory(|m| m.any_popup_open()) || self.pane_menu.is_some();
        let mut moving = false;
        self.dock_strips = [None; 4];
        let pinned: Vec<DockView> = views.iter().copied().filter(|v| self.docks[v.unit].place == Place::AutoHide).collect();
        for v in pinned {
            let edge = self.docks.edge(v.unit);
            let titles: Vec<Pane> = self.tab_order.into_iter().filter(|&p| v.mask[p as usize]).collect();
            let mut hit = (false, false, None);
            let id = egui::Id::new(("fv_strip", edge as u8, v.unit));
            let mut draw = |ui: &mut egui::Ui| {
                for &t in &titles {
                    let r = strip_button(ui, pane_title(t), edge.vertical());
                    if r.hovered() || r.clicked() {
                        hit = (true, hit.1 || r.clicked(), Some(t));
                    }
                }
            };
            let strip = match edge {
                Edge::Left => egui::SidePanel::left(id).resizable(false).exact_width(24.0).show(ctx, |ui| draw(ui)),
                Edge::Right => egui::SidePanel::right(id).resizable(false).exact_width(24.0).show(ctx, |ui| draw(ui)),
                Edge::Top => egui::TopBottomPanel::top(id).resizable(false).exact_height(24.0).show(ctx, |ui| ui.horizontal(|ui| draw(ui)).inner),
                Edge::Bottom => egui::TopBottomPanel::bottom(id).resizable(false).exact_height(24.0).show(ctx, |ui| ui.horizontal(|ui| draw(ui)).inner),
            };
            self.dock_strips[v.unit as usize] = Some(strip.response.rect);
            if let Some(p) = hit.2.filter(|&p| p != v.front) {
                self.dispatch(Command::ActivatePane(p));
            }
            let in_pane = pointer.is_some_and(|q| self.dock_areas[v.unit as usize].is_some_and(|a| a.contains(q)));
            moving |= self.docks[v.unit].auto_hide_frame(now, hit.0, hit.1, in_pane, menu_up);
        }
        if moving || self.docks.0.iter().any(|d| d.out) {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Slid-out auto-hide groups, over the content beside their strip.
    fn slid_out_units(&mut self, ctx: &egui::Context, views: &[DockView]) {
        use crate::docking::{Edge, Place};
        // They slide out of their strip over everything else.
        let content = ctx.available_rect();
        self.dock_areas = [None; 4];
        for v in views.iter().copied() {
            let d = self.docks[v.unit];
            if d.place != Place::AutoHide || d.slide <= 0.0 {
                continue;
            }
            let edge = self.docks.edge(v.unit);
            let ext = v.natural();
            let s = self.dock_strips[v.unit as usize];
            let k = d.slide;
            let rect = match edge {
                Edge::Right => {
                    let right = s.map_or(content.right(), |s| s.left());
                    let w = ext.min(right - ctx.screen_rect().left()) * k;
                    egui::Rect::from_min_max(egui::pos2(right - w, content.top()), egui::pos2(right, content.bottom()))
                }
                Edge::Left => {
                    let left = s.map_or(content.left(), |s| s.right());
                    let w = ext.min(ctx.screen_rect().right() - left) * k;
                    egui::Rect::from_min_max(egui::pos2(left, content.top()), egui::pos2(left + w, content.bottom()))
                }
                Edge::Bottom => {
                    let bottom = s.map_or(content.bottom(), |s| s.top());
                    let h = ext.min(bottom - content.top()) * k;
                    egui::Rect::from_min_max(egui::pos2(content.left(), bottom - h), egui::pos2(content.right(), bottom))
                }
                Edge::Top => {
                    let top = s.map_or(content.top(), |s| s.bottom());
                    let h = ext.min(content.bottom() - top) * k;
                    egui::Rect::from_min_max(egui::pos2(content.left(), top), egui::pos2(content.right(), top + h))
                }
            };
            self.dock_areas[v.unit as usize] = Some(rect);
            self.slide_area(ctx, egui::Id::new(("fv_slide", v.unit)), rect, |app, ui| app.unit_body(ui, v));
        }
    }

    /// A slid-out pane: drawn over the content in `rect`, clipped to it.
    fn slide_area(&mut self, ctx: &egui::Context, id: egui::Id, rect: egui::Rect, body: impl FnOnce(&mut App, &mut egui::Ui)) {
        egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
            ui.set_clip_rect(rect);
            let v = ui.visuals();
            ui.painter().rect(rect, 0.0, v.panel_fill, v.window_stroke());
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect.shrink(3.0)), |ui| body(self, ui));
        });
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
        let mut markers: Vec<(egui::Rect, Drop)> = crate::docking::edge_markers(client).to_vec();
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
            let fill = egui::Color32::from_rgba_unmultiplied(60, 110, 200, 60);
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
            let fill = if on { egui::Color32::from_rgb(120, 160, 230) } else { egui::Color32::from_rgb(230, 236, 248) };
            painter.rect(*r, 3.0, fill, egui::Stroke::new(1.0f32, egui::Color32::from_rgb(70, 100, 170)));
            let glyph = match d {
                Drop::Edge(Edge::Left, _) | Drop::Beside(Edge::Left) => "◀",
                Drop::Edge(Edge::Right, _) | Drop::Beside(Edge::Right) => "▶",
                Drop::Edge(Edge::Top, _) | Drop::Beside(Edge::Top) => "▲",
                Drop::Edge(Edge::Bottom, _) | Drop::Beside(Edge::Bottom) => "▼",
                Drop::Tab => "▣",
            };
            painter.text(r.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(14.0), egui::Color32::from_rgb(40, 60, 120));
        }
        if down {
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
    pub fn pane_drop(&mut self, ui: &egui::Ui, pane: Pane) {
        let ctx = ui.ctx();
        let (released, pos) = ctx.input(|i| (i.pointer.primary_released(), i.pointer.latest_pos()));
        if !released || !pos.is_some_and(|p| ui.max_rect().contains(p)) || !egui::DragAndDrop::has_payload_of_type::<crate::lanes::ChanDrag>(ctx) {
            return;
        }
        let Some(drag) = egui::DragAndDrop::take_payload::<crate::lanes::ChanDrag>(ctx) else { return };
        self.drop_channels(&drag.chans, pane);
    }

    pub fn drop_channels(&mut self, chans: &[usize], pane: Pane) {
        let Some(d) = self.active_doc_mut() else { return };
        let (mut changed, n) = (false, d.chans.len());
        for &i in chans.iter().filter(|&&i| i < n) {
            // The canonical unit, none in ADC mode (`0x414c30`).
            let unit = if d.chans[i].adc_counts { String::new() } else { d.osc.analog[i].unit.to_lowercase() };
            let c = &mut d.chans[i];
            let flag = match pane {
                Pane::Frequency => &mut c.on_freq,
                Pane::Vector => &mut c.on_vector,
                Pane::Circle if unit == "ohm" || unit == "va" => &mut c.on_circle,
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
        if self.docs.is_empty() {
            egui::CentralPanel::default().show(ctx, |ui| self.lanes_ui(ui));
            return;
        }
        let pal = self.skin.palette();
        let edge = egui::Stroke::new(1.0f32, pal.ribbon_top.gamma_multiply(0.92));
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.ribbon_top).stroke(edge).inner_margin(egui::Margin::same(1.0)))
            .show(ctx, |ui| {
                egui::TopBottomPanel::top("fv_doc_tabs")
                    .exact_height(20.0)
                    .show_separator_line(false)
                    .frame(egui::Frame::none())
                    .show_inside(ui, |ui| {
                        let r = ui.max_rect();
                        let band = [pal.ribbon_top.gamma_multiply(0.95), pal.ribbon_top.lerp_to_gamma(egui::Color32::WHITE, 0.5)];
                        let mut mesh = egui::Mesh::default();
                        for (p, c) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().zip([band[0], band[0], band[1], band[1]]) {
                            mesh.colored_vertex(p, c);
                        }
                        mesh.add_triangle(0, 1, 2);
                        mesh.add_triangle(0, 2, 3);
                        ui.painter().add(egui::Shape::mesh(mesh));
                        self.doc_tabs(ui);
                    });
                let (r, g, b) = crate::app::TAB_COLORS[self.active_doc().and_then(|d| d.tab_color).unwrap_or(0)];
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(egui::Color32::from_rgb(r, g, b)).inner_margin(egui::Margin::same(2.0)))
                    .show_inside(ui, |ui| {
                        egui::Frame::none().fill(ui.visuals().panel_fill).show(ui, |ui| {
                            ui.set_min_size(ui.available_size());
                            self.lanes_ui(ui);
                        });
                    });
            });
    }

    // ---- Круговая / Дифференциальная: characteristics + impedance overlay ----
    /// Circle / differential pane: the block and settings grid above the
    /// plot behind a splitter (200 px, 50 px .. height − 150), as the
    /// original's pane.
    fn char_ui(&mut self, ui: &mut egui::Ui, pane: Pane) {
        let h = ui.available_height();
        // `Properties Height` of «Circle Diagram» / «Diff Diagram»
        // (default 200, clamped to [50, pane − 150], persisted on every
        // re-layout — `0x437900` / `0x443390`).
        let key = format!("{}\\Properties Height", if pane == Pane::Differential { "Diff Diagram" } else { "Circle Diagram" });
        let stored = self.store.view.get(&key).and_then(|v| v.parse::<f32>().ok()).unwrap_or(200.0);
        let grid = egui::TopBottomPanel::top(egui::Id::new(("char_grid", pane)))
            .resizable(true)
            .default_height(stored)
            .height_range(50.0..=(h - 150.0).max(50.0))
            // The property grid is a white list (seen on the VM).
            .frame(egui::Frame::none().fill(egui::Color32::WHITE).inner_margin(egui::Margin::same(1.0)))
            .show_inside(ui, |ui| {
                // The grid fills its splitter part however few its rows
                // (a panel would otherwise shrink to «Тип блока» alone).
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.char_side_panel(ui));
            });
        let now = grid.response.rect.height().round();
        if (now - stored).abs() >= 1.0 && self.persist {
            self.store.view.insert(key, format!("{now}"));
            self.store.save();
        }
        self.pane_grid.push(grid.response.rect);
        egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
            self.pane_canvas.push((pane, ui.available_rect_before_wrap()));
            self.plane_ui(ui);
        });
    }

    /// The block property grid (`BlockPropertyGrid`, an MFC property grid
    /// with no header, text (40, 40, 40), the name column a third of the
    /// width): «Тип блока» (string 3483) choosing the block (`0x407cc0`),
    /// then one group per characteristic — its row carries, at the right, a
    /// 12 × 12 box framed in the characteristic's colour with a tick while
    /// it is shown (`0x407dc0`; a click there toggles it, `0x4075c0`) — and
    /// its parameters as text rows; an edit re-evaluates the whole block.
    fn char_side_panel(&mut self, ui: &mut egui::Ui) {
        const TEXT: egui::Color32 = egui::Color32::from_rgb(40, 40, 40);
        let w = ui.available_width();
        let name_w = (w / 3.0).max(60.0);
        let row_h = 20.0;
        let line = egui::Stroke::new(1.0f32, egui::Color32::from_gray(220));
        // One grid row: the name cell and the rect left for the value.
        let row = |ui: &mut egui::Ui, name: &str, bold: bool, band: bool| -> (egui::Rect, egui::Rect) {
            let (r, _) = ui.allocate_exact_size(egui::vec2(w, row_h), egui::Sense::hover());
            if band {
                ui.painter().rect_filled(r, 0.0, egui::Color32::from_gray(232));
            }
            ui.painter().px_hline(r.x_range(), r.bottom(), line);
            let nr = egui::Rect::from_min_max(r.min, egui::pos2(r.left() + name_w, r.bottom()));
            let font = if bold { egui::FontId::new(12.0, egui::FontFamily::Proportional) } else { egui::FontId::proportional(12.0) };
            let g = ui.painter().layout_no_wrap(name.to_string(), font, TEXT);
            let at = egui::pos2(nr.left() + 14.0, nr.center().y - g.size().y / 2.0);
            ui.painter().with_clip_rect(nr).galley(at, g.clone(), TEXT);
            if bold {
                ui.painter().with_clip_rect(nr).galley(at + egui::vec2(0.5, 0.0), g, TEXT);
            }
            if !band {
                ui.painter().px_vline(nr.right(), r.y_range(), line);
            }
            (r, egui::Rect::from_min_max(egui::pos2(nr.right() + 2.0, r.top() + 1.0), egui::pos2(r.right() - 2.0, r.bottom() - 1.0)))
        };
        ui.spacing_mut().item_spacing.y = 0.0;
        // «Тип блока».
        let (_, vr) = row(ui, "Тип блока", false, false);
        let prev = self.block_idx;
        let names: Vec<String> = self.library.blocks.iter().map(|b| b.name.clone()).collect();
        let combo = ui.put(vr, |ui: &mut egui::Ui| {
            egui::ComboBox::from_id_salt("blockcombo")
                .width(vr.width())
                .selected_text(names.get(self.block_idx).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (i, name) in names.iter().enumerate() {
                        ui.selectable_value(&mut self.block_idx, i, name);
                    }
                })
                .response
        });
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
            let (r, _) = row(ui, name, true, true);
            // The expand box and the visibility box.
            let toggle = egui::Rect::from_min_size(egui::pos2(r.left() + 2.0, r.center().y - 5.0), egui::vec2(10.0, 10.0));
            ui.painter().px_rect_stroke(toggle, 0.0, egui::Stroke::new(1.0f32, egui::Color32::from_gray(128)));
            ui.painter().text(toggle.center(), egui::Align2::CENTER_CENTER, if open { "−" } else { "+" }, egui::FontId::proportional(10.0), TEXT);
            // The grid drops a double-click on a group row (`0x407640`): the
            // second click of a pair does nothing.
            let once = |r: egui::Response| r.clicked() && !r.double_clicked();
            if once(ui.interact(toggle.expand(2.0), ui.id().with(("fv_grid_tg", b, *ci)), egui::Sense::click())) {
                ui.data_mut(|d| d.insert_temp(open_id, !open));
            }
            if ui.interact(egui::Rect::from_min_max(egui::pos2(toggle.right() + 2.0, r.top()), egui::pos2(r.left() + name_w, r.bottom())), ui.id().with(("fv_grid_name", b, *ci)), egui::Sense::click()).clicked() {
                self.selected_char = Some(*ci);
            }
            if let Some(st) = self.chars.get_mut(&(b, *ci)) {
                let bx = egui::Rect::from_min_max(egui::pos2(r.right() - 14.0, r.top() + 3.0), egui::pos2(r.right() - 2.0, r.top() + 15.0));
                ui.painter().rect(bx, 0.0, egui::Color32::WHITE, egui::Stroke::new(1.0f32, st.color));
                if st.visible {
                    ui.painter().text(bx.center(), egui::Align2::CENTER_CENTER, "✓", egui::FontId::proportional(11.0), egui::Color32::from_rgb(0, 0, 200));
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
            for p in params {
                let (_, vr) = row(ui, p, false, false);
                if let Some(st) = self.chars.get_mut(&(b, *ci)) {
                    let text = st.settings.entry(p.clone()).or_default();
                    let resp = ui.put(vr, egui::TextEdit::singleline(text).frame(false).text_color(TEXT));
                    changed |= resp.lost_focus();
                }
            }
        }
        if touched {
            if let Some(d) = self.active_doc_mut() {
                d.set.modified = true;
            }
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

/// A group as it shows: its own unit, its visible tabs (by pane), the
/// front one, and whether it has a tab strip (more than one visible).
#[derive(Debug, Clone, Copy)]
struct DockView {
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

fn pane_dim(p: Pane) -> fvb_core::model::Dimension {
    if p == Pane::Differential {
        fvb_core::model::Dimension::Dif
    } else {
        fvb_core::model::Dimension::Ohm
    }
}

fn pane_title(p: Pane) -> &'static str {
    match p {
        Pane::Vector => "Векторная диаграмма",
        Pane::Circle => "Круговая диаграмма",
        Pane::Differential => "Дифференциальная диаграмма",
        Pane::Frequency => "Частотная диаграмма",
    }
}

/// What a caption did this frame.
#[derive(Debug, Clone, Copy, Default)]
struct CaptionAct {
    close: bool,
    pin: bool,
    double: bool,
    dragging: bool,
    /// A right-click: the pane menu at that point.
    menu: Option<egui::Pos2>,
}

/// A docking-pane caption: the title, the pin (`pinned` None when the pane
/// floats — a mini-frame has none) and a close button that only hides the
/// pane; the caption itself takes double-clicks and drags.
fn pane_caption(ui: &mut egui::Ui, title: &str, pinned: Option<bool>) -> CaptionAct {
    let mut act = CaptionAct::default();
    // The caption takes double-clicks and drags; registered first, so its
    // buttons, drawn over it, keep their clicks.
    let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), ui.spacing().interact_size.y));
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
    act.dragging = resp.dragged();
    if resp.secondary_clicked() {
        act.menu = resp.interact_pointer_pos();
    }
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(title).strong()).selectable(false));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            act.close = ui.small_button("✕").on_hover_text("Закрыть").clicked();
            if let Some(pinned) = pinned {
                let tip = if pinned { "Закрепить" } else { "Автоматически скрывать" };
                act.pin = ui.small_button(if pinned { "⊣" } else { "📌" }).on_hover_text(tip).clicked();
            }
        });
    });
    ui.separator();
    act
}

/// An auto-hide strip button: the pane's title, written down the right
/// strip or along the bottom one.
fn strip_button(ui: &mut egui::Ui, title: &str, vertical: bool) -> egui::Response {
    let font = egui::FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(title.to_owned(), font, ui.visuals().text_color());
    let len = galley.size().x + 16.0;
    let size = if vertical { egui::vec2(20.0, len) } else { egui::vec2(len, 20.0) };
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if resp.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
    ui.painter().rect(rect, 2.0, fill, ui.visuals().widgets.inactive.bg_stroke);
    if vertical {
        let at = egui::pos2(rect.center().x + galley.size().y / 2.0, rect.top() + 8.0);
        ui.painter().add(egui::epaint::TextShape::new(at, galley, ui.visuals().text_color()).with_angle(std::f32::consts::FRAC_PI_2));
    } else {
        ui.painter().galley(rect.left_center() + egui::vec2(8.0, -galley.size().y / 2.0), galley, ui.visuals().text_color());
    }
    resp
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
