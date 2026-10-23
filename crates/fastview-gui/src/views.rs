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

    /// The diagram docking panes (`0x474e10`): one right-hand column with
    /// the Векторная | Круговая | Дифференциальная tab group (tabs at the
    /// bottom) and Частотная below it, each with a caption and a close
    /// button that hides it. Call before the central view.
    pub fn diagram_panes(&mut self, ctx: &egui::Context) {
        self.pane_canvas.clear();
        self.pane_grid.clear();
        let tab = self.panes.shown_tab();
        if tab.is_some() || self.panes.frequency {
            self.diagram_column(ctx, tab);
        }
        self.diagram_overlays(ctx);
    }

    fn diagram_column(&mut self, ctx: &egui::Context, tab: Option<Pane>) {
        egui::SidePanel::right("diagram_panes")
            .resizable(true)
            .default_width(500.0)
            .width_range(260.0..=1200.0)
            .show(ctx, |ui| {
                let full_h = ui.available_height();
                if self.panes.frequency {
                    let freq = |app: &mut App, ui: &mut egui::Ui| {
                        if pane_caption(ui, "Частотная диаграмма") {
                            app.dispatch(Command::TogglePane(Pane::Frequency));
                        }
                        app.pane_canvas.push((Pane::Frequency, ui.available_rect_before_wrap()));
                        app.frequency_ui(ui);
                        app.pane_drop(ui, Pane::Frequency);
                    };
                    if tab.is_some() {
                        egui::TopBottomPanel::bottom("pane_frequency")
                            .resizable(true)
                            .default_height(200.0)
                            .height_range(80.0..=(full_h - 120.0).max(80.0))
                            .show_inside(ui, |ui| freq(self, ui));
                    } else {
                        freq(self, ui);
                    }
                }
                if let Some(active) = tab {
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
                        if pane_caption(ui, pane_title(active)) {
                            self.dispatch(Command::TogglePane(active));
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
            });
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

/// A docking-pane caption: the title and a close button (which only hides
/// the pane). Returns true when the close button was clicked.
fn pane_caption(ui: &mut egui::Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close = ui.small_button("✕").on_hover_text("Закрыть").clicked();
        });
    });
    ui.separator();
    close
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
