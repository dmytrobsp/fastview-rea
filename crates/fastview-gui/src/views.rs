//! Diagram views (круговая/дифференциальная/векторная/частотная/быстрый анализ)
//! over the active document (ОМП lives in `omp_ui.rs`). The oscillogram (lane) view lives in
//! `lanes.rs`.

use crate::app::{App, Command, Pane, Panes, View};
use crate::diagram_ui::Window;
use eframe::egui;

impl App {
    pub fn render_tab(&mut self, ctx: &egui::Context) {
        match self.view {
            View::Oscillogram => self.view_oscillogram(ctx),
        }
    }

    /// The diagram docking panes (`0x474e10`, L9.92 / L9.101): the
    /// Векторная | Круговая | Дифференциальная tab group (tabs at the
    /// bottom) and Частотная — each docked in a container on a frame edge
    /// (or beside / tabbed into the other), floating in its own window or
    /// pinned to an auto-hide strip on its edge; captions carry the pin
    /// and a close button that hides. Call before the central view.
    pub fn diagram_panes(&mut self, ctx: &egui::Context) {
        use crate::docking::{Place, Spot};
        self.pane_canvas.clear();
        self.pane_tab_rects.clear();
        self.pane_grid.clear();
        self.dock_client = ctx.available_rect();
        let views = self.dock_views();
        // The strips sit outermost, then the containers (edge-marker docks
        // outside the document area's ones).
        self.auto_hide_strips(ctx, &views);
        self.dock_rects = [None, None];
        let mut hosts: Vec<DockView> = views
            .iter()
            .copied()
            .filter(|v| self.dock_ref(v.unit).place == Place::Docked)
            .filter(|v| match self.dock_ref(v.unit).spot {
                Spot::Edge { .. } => true,
                // Beside a unit that does not show as a docked container:
                // alone on that unit's edge.
                _ => !views.iter().any(|w| w.unit == v.unit.other() && self.dock_ref(w.unit).place == Place::Docked),
            })
            .collect();
        hosts.sort_by_key(|v| !matches!(self.dock_ref(v.unit).spot, Spot::Edge { outer: true, .. }));
        for h in hosts {
            let beside = views.iter().copied().find(|w| w.unit == h.unit.other() && self.dock_ref(w.unit).place == Place::Docked && matches!(self.dock_ref(w.unit).spot, Spot::Beside(_)) && matches!(self.dock_ref(h.unit).spot, Spot::Edge { .. }));
            self.dock_container(ctx, h, beside);
        }
        let floating: Vec<DockView> = views.iter().copied().filter(|v| self.dock_ref(v.unit).place == Place::Floating).collect();
        for v in floating {
            self.floating_unit(ctx, v);
        }
        self.slid_out_units(ctx, &views);
        self.dock_drag_frame(ctx);
        self.diagram_overlays(ctx);
    }

    /// How many units show on their own (a tabbed-in one shows in its host).
    #[cfg(test)]
    pub fn dock_views_shown(&self) -> usize {
        self.dock_views().len()
    }

    /// What shows, per governing unit: a unit tabbed into the other shows
    /// in that one's group, wherever it is.
    fn dock_views(&self) -> Vec<DockView> {
        use crate::docking::{Place, Spot, Unit};
        let tab = self.panes.shown_tab();
        let freq = self.panes.frequency;
        let tabbed = |u: Unit| {
            let d = self.dock_ref(u);
            d.place == Place::Docked && d.spot == Spot::Tab
        };
        let mut v = Vec::new();
        if !tabbed(Unit::Group) {
            let with_freq = freq && tabbed(Unit::Freq);
            if tab.is_some() || with_freq {
                v.push(DockView { unit: Unit::Group, tab, freq: with_freq });
            }
        }
        if !tabbed(Unit::Freq) {
            let group = if tabbed(Unit::Group) { tab } else { None };
            if freq || group.is_some() {
                v.push(DockView { unit: Unit::Freq, tab: group, freq });
            }
        }
        v
    }

    fn dock(&mut self, unit: crate::docking::Unit) -> &mut crate::docking::Dockable {
        match unit {
            crate::docking::Unit::Group => &mut self.dock_group,
            crate::docking::Unit::Freq => &mut self.dock_freq,
        }
    }

    pub fn dock_ref(&self, unit: crate::docking::Unit) -> &crate::docking::Dockable {
        match unit {
            crate::docking::Unit::Group => &self.dock_group,
            crate::docking::Unit::Freq => &self.dock_freq,
        }
    }

    /// The frame edge a unit's container is on (its own, or the unit it
    /// sits beside / in).
    pub fn dock_edge(&self, unit: crate::docking::Unit) -> crate::docking::Edge {
        use crate::docking::Spot;
        match self.dock_ref(unit).spot {
            Spot::Edge { edge, .. } => edge,
            _ => match self.dock_ref(unit.other()).spot {
                Spot::Edge { edge, .. } => edge,
                _ => crate::docking::Edge::Right,
            },
        }
    }

    /// `unit` leaves its container: a unit beside it takes the container.
    fn dock_leave(&mut self, unit: crate::docking::Unit) {
        let (a, b) = match unit {
            crate::docking::Unit::Group => (&mut self.dock_group, &mut self.dock_freq),
            crate::docking::Unit::Freq => (&mut self.dock_freq, &mut self.dock_group),
        };
        crate::docking::leave(a, b);
    }

    /// The frame client's top-left, where a first float opens.
    fn float_origin(ctx: &egui::Context) -> egui::Pos2 {
        ctx.available_rect().min
    }

    /// A caption's actions on `unit` whose pane spans `pane`: close hides
    /// the shown pane, the pin toggles auto-hide, a double-click floats /
    /// redocks, a drag detaches a docked unit (or moves a floating one).
    fn caption_acts(&mut self, ctx: &egui::Context, unit: crate::docking::Unit, shown: Pane, act: CaptionAct, pane: egui::Rect) {
        use crate::docking::Place;
        if act.close {
            self.dispatch(Command::TogglePane(shown));
            return;
        }
        let origin = Self::float_origin(ctx);
        if act.pin {
            if self.dock(unit).place == Place::Docked {
                self.dock_leave(unit);
            }
            self.dock(unit).pin();
        }
        if act.double {
            if self.dock(unit).place == Place::Docked {
                self.dock_leave(unit);
            }
            self.dock(unit).double_click(origin);
            return;
        }
        if act.dragging {
            let (press, now, delta) = ctx.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos(), i.pointer.delta()));
            let (Some(press), Some(now)) = (press, now) else { return };
            let dragging = self.dock_drag.is_some();
            match self.dock(unit).place {
                Place::Docked => {
                    if self.dock(unit).drag_detach(press, now, pane, origin) {
                        // A unit beside it takes its container.
                        self.dock_leave(unit);
                        let r = self.dock(unit).float_at(origin);
                        self.dock_drag = Some((unit, now - r.min));
                    }
                }
                // A floating pane's drag shows the markers too.
                Place::Floating if !dragging => {
                    let d = self.dock(unit);
                    d.float_rect = d.float_rect.map(|r| r.translate(delta));
                    let r = self.dock(unit).float_at(origin);
                    self.dock_drag = Some((unit, now - r.min));
                }
                _ => {}
            }
        }
    }

    /// A unit's body: the group's tabs (with Частотная among them when it
    /// is tabbed in) or Частотная alone.
    fn unit_body(&mut self, ui: &mut egui::Ui, v: DockView) {
        match v.tab {
            Some(active) => self.group_body(ui, v, active),
            None if v.freq => self.freq_body(ui, v.unit),
            None => {}
        }
    }

    /// The tab group's body: the tab strip at the bottom, then the shown
    /// pane with its caption, toolbar and window.
    fn group_body(&mut self, ui: &mut egui::Ui, v: DockView, active: Pane) {
        let pinned = self.dock_ref(v.unit).place != crate::docking::Place::Floating;
        let outer = ui.max_rect();
        let front_freq = v.freq && self.freq_in_front;
        // A tab pulled out of the strip (`0x10068d0b` / `0x100690b4`).
        let mut pulled: Option<bool> = None;
        let strip = egui::TopBottomPanel::bottom(egui::Id::new(("pane_tabs", v.unit))).show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let panes = self.panes;
                for p in Panes::GROUP.into_iter().filter(|&p| panes.visible(p)) {
                    let r = ui.selectable_label(p == active && !front_freq, pane_title(p)).interact(egui::Sense::click_and_drag());
                    self.pane_tab_rects.push((p, r.rect));
                    if r.clicked() {
                        self.freq_in_front = false;
                        if p != active {
                            self.dispatch(Command::ActivatePane(p));
                        }
                    }
                    if r.dragged() {
                        pulled = Some(false);
                    }
                }
                if v.freq {
                    let r = ui.selectable_label(front_freq, pane_title(Pane::Frequency)).interact(egui::Sense::click_and_drag());
                    self.pane_tab_rects.push((Pane::Frequency, r.rect));
                    if r.clicked() {
                        self.freq_in_front = true;
                    }
                    if r.dragged() {
                        pulled = Some(true);
                    }
                }
            });
        });
        if let (Some(freq_tab), true) = (pulled, v.freq && v.tab.is_some()) {
            let ctx = ui.ctx().clone();
            self.tab_drag_out(&ctx, v.unit, freq_tab, strip.response.rect);
        }
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
            if front_freq {
                self.freq_body(ui, v.unit);
                return;
            }
            let act = pane_caption(ui, pane_title(active), pinned.then_some(self.dock_ref(v.unit).place == crate::docking::Place::AutoHide));
            let ctx = ui.ctx().clone();
            self.caption_acts(&ctx, v.unit, active, act, outer);
            if act.close {
                return;
            }
            // Each pane: its vertical toolbar on the right edge
            // (550 / 552 / 553), then the window.
            match active {
                Pane::Vector => {
                    self.pane_toolbar(ui, Window::Vector);
                    egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
                        self.pane_canvas.push((Pane::Vector, ui.available_rect_before_wrap()));
                        self.vector_ui(ui);
                        self.pane_drop(ui, Pane::Vector);
                    });
                }
                Pane::Circle | Pane::Differential => {
                    let w = if active == Pane::Circle { Window::Circle } else { Window::Diff };
                    self.pane_toolbar(ui, w);
                    self.char_ui(ui);
                    // The differential diagram has no drop target.
                    if active == Pane::Circle {
                        self.pane_drop(ui, Pane::Circle);
                    }
                }
                Pane::Frequency => {}
            }
        });
    }

    /// A tab dragged out of a combined group's strip: past the system drag
    /// distance (SM_CXDRAG / SM_CYDRAG, 4 px, `0x10068657`) and outside the
    /// strip, `CTabbedPane::FloatTab` (`0x1013a259`) floats that pane under
    /// the pointer, the other unit keeping the container; the drag then
    /// goes on as a floating caption drag (the markers). Our group's three
    /// panes are one unit: a group tab pulled out floats the group.
    fn tab_drag_out(&mut self, ctx: &egui::Context, host: crate::docking::Unit, freq_tab: bool, strip: egui::Rect) {
        use crate::docking::{Place, Spot, Unit};
        let (press, now) = ctx.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos()));
        let (Some(press), Some(now)) = (press, now) else { return };
        let d = now - press;
        if (d.x.abs() < crate::docking::DRAG_PX && d.y.abs() < crate::docking::DRAG_PX) || strip.contains(now) || self.dock_drag.is_some() {
            return;
        }
        let pulled = if freq_tab { Unit::Freq } else { Unit::Group };
        if pulled == host {
            // The host's own tab: the other unit takes the container.
            let h = *self.dock_ref(host);
            let o = self.dock(host.other());
            (o.spot, o.extent, o.gen, o.place) = (h.spot, h.extent, o.gen + 1, Place::Docked);
            self.dock(host).spot = Spot::Tab;
        }
        let origin = Self::float_origin(ctx);
        let grab = egui::vec2(40.0, 10.0);
        let size = self.dock(pulled).float_at(origin).size();
        let u = self.dock(pulled);
        u.place = Place::Floating;
        u.float_rect = Some(egui::Rect::from_min_size(now - grab, size));
        self.freq_in_front = false;
        self.dock_drag = Some((pulled, grab));
    }

    /// Частотная's body: caption and window; its caption acts on `unit`
    /// (the group it is tabbed into, or itself).
    fn freq_body(&mut self, ui: &mut egui::Ui, unit: crate::docking::Unit) {
        let pinned = self.dock_ref(unit).place != crate::docking::Place::Floating;
        let outer = ui.max_rect();
        let act = pane_caption(ui, pane_title(Pane::Frequency), pinned.then_some(self.dock_ref(unit).place == crate::docking::Place::AutoHide));
        let ctx = ui.ctx().clone();
        self.caption_acts(&ctx, unit, Pane::Frequency, act, outer);
        if act.close {
            return;
        }
        self.pane_canvas.push((Pane::Frequency, ui.available_rect_before_wrap()));
        self.frequency_ui(ui);
        self.pane_drop(ui, Pane::Frequency);
    }

    /// A docked container on its frame edge holding `h` — and `beside`, the
    /// other unit on its side of it.
    fn dock_container(&mut self, ctx: &egui::Context, h: DockView, beside: Option<DockView>) {
        use crate::docking::{Edge, Spot};
        let edge = self.dock_edge(h.unit);
        let hd = *self.dock_ref(h.unit);
        // The create sizes (500 wide on the right, Частотная 200 tall) or
        // the last drop's; a new drop is a new panel.
        let id = egui::Id::new(("fv_dock", edge as u8, h.unit, hd.gen));
        let extent = |v: DockView| -> f32 { if v.tab.is_some() { 500.0 } else { 200.0 } };
        let beside_dock = beside.map(|w| *self.dock_ref(w.unit));
        let body = |app: &mut App, ui: &mut egui::Ui| {
            if let Some(w) = beside {
                let Spot::Beside(side) = app.dock_ref(w.unit).spot else { return };
                let full = ui.available_size();
                let wd = beside_dock.unwrap_or(hd);
                let inner = egui::Id::new(("fv_dock_beside", edge as u8, h.unit, wd.gen));
                let r = match side {
                    Edge::Top | Edge::Bottom => {
                        let p = if side == Edge::Top { egui::TopBottomPanel::top(inner) } else { egui::TopBottomPanel::bottom(inner) };
                        let d = wd.extent.unwrap_or(extent(w)).min(full.y / 2.0);
                        p.resizable(true).default_height(d).height_range(80.0..=(full.y - 120.0).max(80.0)).show_inside(ui, |ui| app.unit_body(ui, w)).response.rect
                    }
                    Edge::Left | Edge::Right => {
                        let p = if side == Edge::Left { egui::SidePanel::left(inner) } else { egui::SidePanel::right(inner) };
                        let d = wd.extent.unwrap_or(full.x / 2.0).min(full.x / 2.0);
                        p.resizable(true).default_width(d).width_range(120.0..=(full.x - 120.0).max(120.0)).show_inside(ui, |ui| app.unit_body(ui, w)).response.rect
                    }
                };
                app.dock_rects[w.unit as usize] = Some(r);
            }
            let r = ui.available_rect_before_wrap();
            app.dock_rects[h.unit as usize] = Some(r);
            app.unit_body(ui, h);
        };
        let ext = hd.extent.unwrap_or_else(|| extent(h).max(beside.map_or(0.0, extent)));
        match edge {
            Edge::Left | Edge::Right => {
                let p = if edge == Edge::Left { egui::SidePanel::left(id) } else { egui::SidePanel::right(id) };
                p.resizable(true).default_width(ext).width_range(260.0..=1200.0).show(ctx, |ui| body(self, ui));
            }
            Edge::Top | Edge::Bottom => {
                let p = if edge == Edge::Top { egui::TopBottomPanel::top(id) } else { egui::TopBottomPanel::bottom(id) };
                let ext = if hd.extent.is_none() && (beside.is_some() || h.tab.is_some()) { 300.0 } else { ext };
                p.resizable(true).default_height(ext).height_range(80.0..=900.0).show(ctx, |ui| body(self, ui));
            }
        }
    }

    /// A floating unit: its own window at its recent floating rect; the
    /// caption moves it, a double-click redocks it.
    fn floating_unit(&mut self, ctx: &egui::Context, v: DockView) {
        let origin = Self::float_origin(ctx);
        let unit = v.unit;
        let r = self.dock(unit).float_at(origin);
        let title = v.tab.unwrap_or(Pane::Frequency);
        let resp = egui::Window::new(pane_title(title))
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
            let d = self.dock(unit);
            if d.place == crate::docking::Place::Floating {
                let min = d.float_rect.map_or(r.min, |f| f.min);
                d.float_rect = Some(egui::Rect::from_min_size(min, resp.response.rect.size()));
            }
        }
    }

    /// The auto-hide strips: a button per pinned pane on its unit's edge;
    /// hovering 400 ms or a click slides the unit out.
    fn auto_hide_strips(&mut self, ctx: &egui::Context, views: &[DockView]) {
        use crate::docking::{Edge, Place};
        let now = ctx.input(|i| i.time);
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let menu_up = ctx.memory(|m| m.any_popup_open());
        let mut moving = false;
        self.dock_strips = [None, None];
        let pinned: Vec<DockView> = views.iter().copied().filter(|v| self.dock_ref(v.unit).place == Place::AutoHide).collect();
        for v in pinned {
            let edge = self.dock_edge(v.unit);
            let panes = self.panes;
            let mut titles: Vec<Option<Pane>> = Vec::new();
            if v.tab.is_some() {
                titles.extend(Panes::GROUP.into_iter().filter(|&p| panes.visible(p)).map(Some));
            }
            if v.freq {
                titles.push(None);
            }
            let mut hit = (false, false, None);
            let id = egui::Id::new(("fv_strip", edge as u8, v.unit));
            let mut draw = |ui: &mut egui::Ui| {
                for t in &titles {
                    let r = strip_button(ui, pane_title(t.unwrap_or(Pane::Frequency)), edge.vertical());
                    if r.hovered() || r.clicked() {
                        hit = (true, hit.1 || r.clicked(), Some(*t));
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
            match hit.2 {
                Some(Some(p)) => {
                    self.freq_in_front = false;
                    if p != self.panes.active {
                        self.dispatch(Command::ActivatePane(p));
                    }
                }
                Some(None) => self.freq_in_front = true,
                None => {}
            }
            let in_pane = pointer.is_some_and(|q| self.dock_areas[v.unit as usize].is_some_and(|a| a.contains(q)));
            moving |= self.dock(v.unit).auto_hide_frame(now, hit.0, hit.1, in_pane, menu_up);
        }
        if moving || [&self.dock_group, &self.dock_freq].iter().any(|d| d.out) {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Slid-out auto-hide units, over the content beside their strip.
    fn slid_out_units(&mut self, ctx: &egui::Context, views: &[DockView]) {
        use crate::docking::{Edge, Place};
        // They slide out of their strip over everything else.
        let content = ctx.available_rect();
        self.dock_areas = [None, None];
        for v in views.iter().copied() {
            let d = self.dock_ref(v.unit);
            if d.place != Place::AutoHide || d.slide <= 0.0 {
                continue;
            }
            let edge = self.dock_edge(v.unit);
            let ext: f32 = if v.tab.is_some() { 500.0 } else { 200.0 };
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
            self.slide_area(ctx, if v.unit == crate::docking::Unit::Group { "fv_slide_group" } else { "fv_slide_freq" }, rect, |app, ui| app.unit_body(ui, v));
        }
    }

    /// A slid-out pane: drawn over the content in `rect`, clipped to it.
    fn slide_area(&mut self, ctx: &egui::Context, id: &str, rect: egui::Rect, body: impl FnOnce(&mut App, &mut egui::Ui)) {
        egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
            ui.set_clip_rect(rect);
            let v = ui.visuals();
            ui.painter().rect(rect, 0.0, v.panel_fill, v.window_stroke());
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect.shrink(3.0)), |ui| body(self, ui));
        });
    }

    /// A dragged floating unit follows the pointer while smart docking
    /// (DT_SMART, VS2008 look) shows the four frame-edge markers and, over
    /// the other unit's docked pane or the document area, the five-way
    /// group; the marker under the pointer is hot and shows the drop
    /// preview; released on it the unit docks there, elsewhere it stays
    /// floating (`0x100d7313`–`0x100d7445`, `0x10136b50`).
    fn dock_drag_frame(&mut self, ctx: &egui::Context) {
        use crate::docking::{Drop, Edge, Place};
        let Some((unit, grab)) = self.dock_drag else { return };
        let (down, pos) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.latest_pos()));
        let down = down || self.dock_drag_hold;
        let client = self.dock_client;
        let docs = ctx.available_rect();
        let mut markers: Vec<(egui::Rect, Drop)> = crate::docking::edge_markers(client).to_vec();
        let other = unit.other();
        let host = self.dock_rects[other as usize].filter(|_| self.dock_ref(other).place == Place::Docked && matches!(self.dock_ref(other).spot, crate::docking::Spot::Edge { .. }));
        match (host, pos) {
            (Some(h), Some(p)) if h.contains(p) => markers.extend(crate::docking::five_way(h, true)),
            (_, Some(p)) if docs.contains(p) => markers.extend(crate::docking::five_way(docs, false)),
            _ => {}
        }
        let hot = pos.and_then(|p| crate::docking::hit_marker(&markers, p));
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("fv_dock_markers")));
        // The drop preview (`CalcExpectedDockedRect` `0x1008c45d`): along a
        // frame edge (outer) or the document area (inner) as thick as the
        // floating window, beside a pane min(that, half the pane), into a
        // pane the pane less a tab strip.
        let fsize = self.dock_ref(unit).float_rect.map_or(egui::vec2(200.0, 200.0), |r| r.size());
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
                let d = self.dock(unit);
                d.float_rect = d.float_rect.map(|r| egui::Rect::from_min_size(p - grab, r.size()));
            }
            ctx.request_repaint();
        } else {
            if let Some(d) = hot {
                let other_dock = *self.dock_ref(other);
                let area = match (d, host) {
                    (Drop::Edge(_, true), _) => client,
                    (Drop::Beside(_), Some(h)) => h,
                    _ => docs,
                };
                crate::docking::drop_on(self.dock(unit), &other_dock, d, crate::docking::dock_extent(d, fsize, area));
                // A tab dropped in comes to the front (`AttachToTabWnd`,
                // bSetActive).
                if d == Drop::Tab {
                    self.freq_in_front = unit == crate::docking::Unit::Freq;
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

    fn view_oscillogram(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| self.lanes_ui(ui));
    }

    // ---- Круговая / Дифференциальная: characteristics + impedance overlay ----
    /// Circle / differential pane: the block and settings grid above the
    /// plot behind a splitter (200 px, 50 px .. height − 150), as the
    /// original's pane.
    fn char_ui(&mut self, ui: &mut egui::Ui) {
        let h = ui.available_height();
        let grid = egui::TopBottomPanel::top("char_grid")
            .resizable(true)
            .default_height(200.0)
            .height_range(50.0..=(h - 150.0).max(50.0))
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.char_side_panel(ui));
            });
        self.pane_grid.push(grid.response.rect);
        egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
            let pane = if self.panes.active == Pane::Differential { Pane::Differential } else { Pane::Circle };
            self.pane_canvas.push((pane, ui.available_rect_before_wrap()));
            self.plane_ui(ui);
        });
    }

    fn char_side_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Блок:");
            let prev = self.block_idx;
            let names: Vec<String> = self.library.blocks.iter().map(|b| b.name.clone()).collect();
            egui::ComboBox::from_id_salt("blockcombo")
                .width(230.0)
                .selected_text(names.get(self.block_idx).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (i, name) in names.iter().enumerate() {
                        ui.selectable_value(&mut self.block_idx, i, name);
                    }
                });
            if self.block_idx != prev {
                self.selected_char = None;
                self.ensure_block_state();
            }
        });
        ui.separator();
        ui.heading("Характеристики");
        let b = self.block_idx;
        let names: Vec<(usize, String)> = self
            .library
            .blocks
            .get(b)
            .map(|blk| {
                blk.characteristics
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let mut vis_changed = false;
        egui::ScrollArea::vertical()
            .id_salt("charlist")
            .max_height(ui.available_height() * 0.4)
            .show(ui, |ui| {
                for (ci, name) in &names {
                    ui.horizontal(|ui| {
                        if let Some(st) = self.chars.get_mut(&(b, *ci)) {
                            ui.label(egui::RichText::new("⬤").color(st.color));
                            if ui.checkbox(&mut st.visible, "").changed() {
                                vis_changed = true;
                            }
                        }
                        if ui.selectable_label(self.selected_char == Some(*ci), name).clicked() {
                            self.selected_char = Some(*ci);
                        }
                    });
        if vis_changed {
            self.persist_block();
        }
                }
            });
        ui.separator();
        ui.heading("Уставки");
        self.settings_editor(ui);
    }

    /// The settings grid (`BlockPropertyGrid`): one group per characteristic of
    /// the block with its parameters in document order. Values are plain text,
    /// evaluated as formulas; committing an edit re-evaluates the whole block.
    fn settings_editor(&mut self, ui: &mut egui::Ui) {
        let b = self.block_idx;
        let groups: Vec<(usize, String, Vec<String>)> = self
            .library
            .blocks
            .get(b)
            .map(|blk| {
                blk.characteristics
                    .iter()
                    .enumerate()
                    .map(|(ci, ch)| (ci, ch.name.clone(), ch.parameters.iter().map(|p| p.name.clone()).collect()))
                    .collect()
            })
            .unwrap_or_default();
        let mut changed = false;
        egui::ScrollArea::vertical().id_salt("paramscroll").show(ui, |ui| {
            for (ci, name, params) in &groups {
                egui::CollapsingHeader::new(name)
                    .id_salt(("paramgroup", b, *ci))
                    .default_open(self.selected_char == Some(*ci))
                    .show(ui, |ui| {
                        egui::Grid::new(("paramgrid", b, *ci)).num_columns(2).striped(true).show(ui, |ui| {
                            for p in params {
                                ui.label(p);
                                if let Some(st) = self.chars.get_mut(&(b, *ci)) {
                                    let text = st.settings.entry(p.clone()).or_default();
                                    let r = ui.add(egui::TextEdit::singleline(text).desired_width(140.0));
                                    changed |= r.lost_focus();
                                }
                                ui.end_row();
                            }
                        });
                    });
            }
            if let Some(ci) = self.selected_char {
                if ui.button("Сбросить уставки").clicked() {
                    if let (Some(blk), Some(st)) = (self.library.blocks.get(b), self.chars.get_mut(&(b, ci))) {
                        if let Some(ch) = blk.characteristics.get(ci) {
                            st.settings = ch.default_settings();
                            changed = true;
                        }
                    }
                }
            }
        });
        if changed {
            self.recompute_block();
        }
    }

    // ---- Быстрый анализ ----
}

/// A unit as it shows: its group's shown tab (if the group shows in it)
/// and whether Частотная shows in it.
#[derive(Debug, Clone, Copy)]
struct DockView {
    unit: crate::docking::Unit,
    tab: Option<Pane>,
    freq: bool,
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
            for active in Panes::GROUP {
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
