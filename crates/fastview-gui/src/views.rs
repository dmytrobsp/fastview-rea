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

    /// The diagram docking panes (`0x474e10`, L9.92): the Векторная |
    /// Круговая | Дифференциальная tab group (tabs at the bottom) and
    /// Частотная, each docked in the right-hand column, floating in its own
    /// window or pinned to an auto-hide strip; captions carry the pin and a
    /// close button that hides. Call before the central view.
    pub fn diagram_panes(&mut self, ctx: &egui::Context) {
        use crate::docking::{Place, Unit};
        self.pane_canvas.clear();
        self.pane_grid.clear();
        let tab = self.panes.shown_tab();
        let freq = self.panes.frequency;
        // The strips sit outermost, then the docked column.
        self.auto_hide_strips(ctx, tab, freq);
        let col_group = tab.filter(|_| self.dock_group.place == Place::Docked);
        let col_freq = freq && self.dock_freq.place == Place::Docked;
        if col_group.is_some() || col_freq {
            self.diagram_column(ctx, col_group, col_freq);
        }
        if let (Some(t), Place::Floating) = (tab, self.dock_group.place) {
            self.floating_unit(ctx, Unit::Group, t);
        }
        if freq && self.dock_freq.place == Place::Floating {
            self.floating_unit(ctx, Unit::Freq, Pane::Frequency);
        }
        self.slid_out_units(ctx, tab, freq);
        self.dock_drag_frame(ctx);
        self.diagram_overlays(ctx);
    }

    fn dock(&mut self, unit: crate::docking::Unit) -> &mut crate::docking::Dockable {
        match unit {
            crate::docking::Unit::Group => &mut self.dock_group,
            crate::docking::Unit::Freq => &mut self.dock_freq,
        }
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
            self.dock(unit).pin();
        }
        if act.double {
            self.dock(unit).double_click(origin);
            return;
        }
        if act.dragging {
            let (press, now, delta) = ctx.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos(), i.pointer.delta()));
            let (Some(press), Some(now)) = (press, now) else { return };
            match self.dock(unit).place {
                Place::Docked => {
                    if self.dock(unit).drag_detach(press, now, pane, origin) {
                        let r = self.dock(unit).float_at(origin);
                        self.dock_drag = Some((unit, now - r.min));
                    }
                }
                Place::Floating => {
                    let d = self.dock(unit);
                    d.float_rect = d.float_rect.map(|r| r.translate(delta));
                }
                Place::AutoHide => {}
            }
        }
    }

    /// The tab group's body: the tab strip at the bottom, then the shown
    /// pane with its caption, toolbar and window.
    fn group_body(&mut self, ui: &mut egui::Ui, active: Pane) {
        let pinned = self.dock_group.place != crate::docking::Place::Floating;
        let outer = ui.max_rect();
        egui::TopBottomPanel::bottom("pane_tabs").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let panes = self.panes;
                for p in Panes::GROUP.into_iter().filter(|&p| panes.visible(p)) {
                    if ui.selectable_label(p == active, pane_title(p)).clicked() && p != active {
                        self.dispatch(Command::ActivatePane(p));
                    }
                }
            });
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
            let act = pane_caption(ui, pane_title(active), pinned.then_some(self.dock_group.place == crate::docking::Place::AutoHide));
            let ctx = ui.ctx().clone();
            self.caption_acts(&ctx, crate::docking::Unit::Group, active, act, outer);
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

    /// Частотная's body: caption and window.
    fn freq_body(&mut self, ui: &mut egui::Ui) {
        let pinned = self.dock_freq.place != crate::docking::Place::Floating;
        let outer = ui.max_rect();
        let act = pane_caption(ui, pane_title(Pane::Frequency), pinned.then_some(self.dock_freq.place == crate::docking::Place::AutoHide));
        let ctx = ui.ctx().clone();
        self.caption_acts(&ctx, crate::docking::Unit::Freq, Pane::Frequency, act, outer);
        if act.close {
            return;
        }
        self.pane_canvas.push((Pane::Frequency, ui.available_rect_before_wrap()));
        self.frequency_ui(ui);
        self.pane_drop(ui, Pane::Frequency);
    }

    fn diagram_column(&mut self, ctx: &egui::Context, tab: Option<Pane>, freq: bool) {
        egui::SidePanel::right("diagram_panes")
            .resizable(true)
            .default_width(500.0)
            .width_range(260.0..=1200.0)
            .show(ctx, |ui| {
                let full_h = ui.available_height();
                if freq {
                    if tab.is_some() {
                        egui::TopBottomPanel::bottom("pane_frequency")
                            .resizable(true)
                            .default_height(200.0)
                            .height_range(80.0..=(full_h - 120.0).max(80.0))
                            .show_inside(ui, |ui| self.freq_body(ui));
                    } else {
                        self.freq_body(ui);
                    }
                }
                if let Some(active) = tab {
                    self.group_body(ui, active);
                }
            });
    }

    /// A floating unit: its own window at its recent floating rect; the
    /// caption moves it, a double-click redocks it.
    fn floating_unit(&mut self, ctx: &egui::Context, unit: crate::docking::Unit, shown: Pane) {
        let origin = Self::float_origin(ctx);
        let r = self.dock(unit).float_at(origin);
        let resp = egui::Window::new(pane_title(shown))
            .id(egui::Id::new(("fv_floating_pane", unit)))
            .title_bar(false)
            .resizable(true)
            .collapsible(false)
            .current_pos(r.min)
            .default_size(r.size())
            .min_size(egui::vec2(200.0, 150.0))
            .show(ctx, |ui| {
                ui.set_min_size(ui.available_size());
                match unit {
                    crate::docking::Unit::Group => self.group_body(ui, shown),
                    crate::docking::Unit::Freq => self.freq_body(ui),
                }
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

    /// The auto-hide strips: a button per pinned pane on its edge — the
    /// group's on the right, Частотная's at the bottom; hovering 400 ms or
    /// a click slides the pane out.
    fn auto_hide_strips(&mut self, ctx: &egui::Context, tab: Option<Pane>, freq: bool) {
        use crate::docking::Place;
        let now = ctx.input(|i| i.time);
        let pointer = ctx.input(|i| i.pointer.latest_pos());
        let menu_up = ctx.memory(|m| m.any_popup_open());
        let mut moving = false;
        self.dock_strips = [None, None];
        if tab.is_some() && self.dock_group.place == Place::AutoHide {
            let panes = self.panes;
            let mut hit = (false, false, None);
            let strip = egui::SidePanel::right("fv_strip_right").resizable(false).exact_width(24.0).show(ctx, |ui| {
                for p in Panes::GROUP.into_iter().filter(|&p| panes.visible(p)) {
                    let r = strip_button(ui, pane_title(p), true);
                    if r.hovered() || r.clicked() {
                        hit = (true, hit.1 || r.clicked(), Some(p));
                    }
                }
            });
            self.dock_strips[0] = Some(strip.response.rect);
            if let Some(p) = hit.2 {
                if p != self.panes.active {
                    self.dispatch(Command::ActivatePane(p));
                }
            }
            let in_pane = pointer.is_some_and(|q| self.dock_areas[0].is_some_and(|a| a.contains(q)));
            moving |= self.dock_group.auto_hide_frame(now, hit.0, hit.1, in_pane, menu_up);
        }
        if freq && self.dock_freq.place == Place::AutoHide {
            let mut hit = (false, false);
            let strip = egui::TopBottomPanel::bottom("fv_strip_bottom").resizable(false).exact_height(24.0).show(ctx, |ui| {
                let r = strip_button(ui, pane_title(Pane::Frequency), false);
                hit = (r.hovered() || r.clicked(), r.clicked());
            });
            self.dock_strips[1] = Some(strip.response.rect);
            let in_pane = pointer.is_some_and(|q| self.dock_areas[1].is_some_and(|a| a.contains(q)));
            moving |= self.dock_freq.auto_hide_frame(now, hit.0, hit.1, in_pane, menu_up);
        }
        if moving || [&self.dock_group, &self.dock_freq].iter().any(|d| d.out) {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Slid-out auto-hide panes, over the content beside their strip.
    fn slid_out_units(&mut self, ctx: &egui::Context, tab: Option<Pane>, freq: bool) {
        // They slide out of their strip over everything else.
        let content = ctx.available_rect();
        self.dock_areas = [None, None];
        if let (Some(active), true) = (tab, self.dock_group.place == crate::docking::Place::AutoHide && self.dock_group.slide > 0.0) {
            let right = self.dock_strips[0].map_or(content.right(), |s| s.left());
            let w = 500.0f32.min(right - ctx.screen_rect().left()) * self.dock_group.slide;
            let rect = egui::Rect::from_min_max(egui::pos2(right - w, content.top()), egui::pos2(right, content.bottom()));
            self.dock_areas[0] = Some(rect);
            self.slide_area(ctx, "fv_slide_group", rect, |app, ui| app.group_body(ui, active));
        }
        if freq && self.dock_freq.place == crate::docking::Place::AutoHide && self.dock_freq.slide > 0.0 {
            let bottom = self.dock_strips[1].map_or(content.bottom(), |s| s.top());
            let h = 200.0f32.min(bottom - content.top()) * self.dock_freq.slide;
            let rect = egui::Rect::from_min_max(egui::pos2(content.left(), bottom - h), egui::pos2(content.right(), bottom));
            self.dock_areas[1] = Some(rect);
            self.slide_area(ctx, "fv_slide_freq", rect, |app, ui| app.freq_body(ui));
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

    /// A just-detached unit follows the pointer; released over its home
    /// marker it docks again, elsewhere it stays floating (the original's
    /// smart docking offers every edge and the panes' sides — ours the
    /// unit's own place).
    fn dock_drag_frame(&mut self, ctx: &egui::Context) {
        use crate::docking::{Place, Unit};
        let Some((unit, grab)) = self.dock_drag else { return };
        let (down, pos) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.latest_pos()));
        let screen = ctx.screen_rect();
        let marker = match unit {
            Unit::Group => egui::Rect::from_center_size(egui::pos2(screen.right() - 40.0, screen.center().y), egui::vec2(32.0, 32.0)),
            Unit::Freq => egui::Rect::from_center_size(egui::pos2(screen.right() - 200.0, screen.bottom() - 40.0), egui::vec2(32.0, 32.0)),
        };
        let over = pos.is_some_and(|p| marker.contains(p));
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("fv_dock_markers")));
        painter.rect(marker, 4.0, if over { egui::Color32::from_rgb(120, 160, 230) } else { egui::Color32::from_rgb(230, 236, 248) }, egui::Stroke::new(1.0f32, egui::Color32::from_rgb(70, 100, 170)));
        painter.text(marker.center(), egui::Align2::CENTER_CENTER, if unit == Unit::Group { "▶" } else { "▼" }, egui::FontId::proportional(14.0), egui::Color32::from_rgb(40, 60, 120));
        if let (true, Some(p)) = (down, pos) {
            let d = self.dock(unit);
            d.float_rect = d.float_rect.map(|r| egui::Rect::from_min_size(p - grab, r.size()));
            ctx.request_repaint();
        } else {
            if over {
                self.dock(unit).place = Place::Docked;
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
