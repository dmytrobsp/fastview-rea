//! Diagram views (круговая/дифференциальная/векторная/частотная/быстрый анализ/
//! ОМП) over the active document. The oscillogram (lane) view lives in
//! `lanes.rs`.

use crate::app::{AnalysisTab, App, Command, OmpMethod, Pane, Panes, View};
use crate::diagram_ui::Window;
use eframe::egui;

impl App {
    pub fn render_tab(&mut self, ctx: &egui::Context) {
        match self.view {
            View::Oscillogram => self.view_oscillogram(ctx),
            View::Analysis => self.view_analysis(ctx),
            View::Omp => self.view_omp(ctx),
        }
    }

    /// The diagram docking panes (`0x474e10`): one right-hand column with
    /// the Векторная | Круговая | Дифференциальная tab group (tabs at the
    /// bottom) and Частотная below it, each with a caption and a close
    /// button that hides it. Call before the central view.
    pub fn diagram_panes(&mut self, ctx: &egui::Context) {
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
                        app.frequency_ui(ui);
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
                        if pane_caption(ui, pane_title(active)) {
                            self.dispatch(Command::TogglePane(active));
                            return;
                        }
                        // Each pane: its vertical toolbar on the right edge
                        // (550 / 552 / 553), then the window.
                        match active {
                            Pane::Vector => {
                                self.pane_toolbar(ui, Window::Vector);
                                egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| self.vector_ui(ui));
                            }
                            Pane::Circle | Pane::Differential => {
                                let w = if active == Pane::Circle { Window::Circle } else { Window::Diff };
                                self.pane_toolbar(ui, w);
                                self.char_ui(ui);
                            }
                            Pane::Frequency => {}
                        }
                    });
                }
            });
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
        egui::TopBottomPanel::top("char_grid")
            .resizable(true)
            .default_height(200.0)
            .height_range(50.0..=(h - 150.0).max(50.0))
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.char_side_panel(ui));
            });
        egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| self.plane_ui(ui));
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
    fn view_analysis(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (tab, label) in [
                    (AnalysisTab::Symmetric, "Симметричные составляющие"),
                    (AnalysisTab::Zero, "Нулевая последовательность"),
                    (AnalysisTab::PhasePhase, "Междуфазные сопротивления"),
                    (AnalysisTab::Phase, "Фазные сопротивления"),
                    (AnalysisTab::Differential, "Дифференциальные токи"),
                    (AnalysisTab::Formula, "Редактор формул"),
                ] {
                    ui.selectable_value(&mut self.analysis_tab, tab, label);
                }
                ui.separator();
                if ui.button("Каналы…").on_hover_text("Назначить Ua/Ia… каналам").clicked() {
                    self.show_channel_map = true;
                }
            });
            ui.separator();
            if self.analysis_tab == AnalysisTab::Formula {
                self.formula_editor_ui(ui);
                return;
            }
            // Like the original's property sheet, a tab generates computed
            // channels; their values are read on the oscillogram and diagrams.
            self.quick_tab_ui(ui);
        });
    }

    fn formula_editor_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Расчётный канал по отсчётам (добавляется на осциллограмму):");
        ui.horizontal(|ui| {
            ui.label("имя");
            ui.text_edit_singleline(&mut self.formula_name);
            ui.label("формула");
            ui.text_edit_singleline(&mut self.formula_text);
            if ui.button("Добавить канал").clicked() {
                self.add_computed();
            }
        });
        ui.label(egui::RichText::new("Доступно: имена каналов, + - * / ^, mod:() sqrt:() sin:() cos:() tg:() ctg:()").weak());
        ui.separator();
        let mut remove = None;
        if let Some(d) = self.active_doc() {
            if d.computed.is_empty() {
                ui.label("Нет расчётных каналов.");
            } else {
                ui.label("Расчётные каналы:");
                for (i, c) in d.computed.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⬤").color(c.color));
                        ui.label(format!("{} = {}", c.name, c.formula));
                        if ui.small_button("✕").clicked() {
                            remove = Some(i);
                        }
                    });
                }
            }
        }
        if let (Some(i), Some(d)) = (remove, self.active_doc_mut()) {
            if i < d.computed.len() {
                d.computed.remove(i);
            }
        }
    }

    // ---- ОМП ----
    /// ОМП: parameters (method, substation currents, line sections with
    /// taps), the calculation, and the result text over the cursor interval.
    fn view_omp(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Определение места повреждения (ОМП)");
            ui.horizontal_wrapped(|ui| {
                ui.label("Метод:");
                ui.selectable_value(&mut self.omp_method, OmpMethod::Isolated, "Изолированная нейтраль");
                ui.selectable_value(&mut self.omp_method, OmpMethod::Grounded, "Глухозаземлённая нейтраль");
                ui.selectable_value(&mut self.omp_method, OmpMethod::DoubleEnded, "Двусторонний замер");
            });
            ui.horizontal(|ui| {
                ui.label("Номинальный ток прямой последовательности, A:");
                ui.add(egui::DragValue::new(&mut self.omp_inom[0]).speed(1.0));
                if self.omp_method == OmpMethod::DoubleEnded {
                    ui.label("подстанция №2:");
                    ui.add(egui::DragValue::new(&mut self.omp_inom[1]).speed(1.0));
                }
            });
            if self.omp_method == OmpMethod::DoubleEnded {
                let names: Vec<(usize, String)> = self
                    .docs
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| Some(*i) != self.active)
                    .map(|(i, d)| (i, d.name.clone()))
                    .collect();
                ui.horizontal(|ui| {
                    ui.label("Осциллограмма 2 (подстанция №2):");
                    let cur = self
                        .omp_remote
                        .and_then(|i| self.docs.get(i))
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| "— не выбрано —".into());
                    egui::ComboBox::from_id_salt("ompremote").selected_text(cur).show_ui(ui, |ui| {
                        for (i, name) in &names {
                            ui.selectable_value(&mut self.omp_remote, Some(*i), name);
                        }
                    });
                });
            }
            ui.separator();
            ui.label("Участки линии:");
            let isolated = self.omp_method == OmpMethod::Isolated;
            let mut remove = None;
            egui::Grid::new("ompsections").striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("№");
                ui.label("Длина, км");
                if isolated {
                    ui.label("R уд., Ом/км");
                    ui.label("X уд., Ом/км");
                } else {
                    ui.label("R1 уд.");
                    ui.label("X1 уд.");
                    ui.label("R0 уд.");
                    ui.label("X0 уд.");
                }
                ui.label("Число опор");
                ui.label("Отпайки (R0, X0, Ом)");
                ui.end_row();
                for (n, s) in self.omp_sections.iter_mut().enumerate() {
                    ui.label(format!("{}", n + 1));
                    ui.add(egui::DragValue::new(&mut s.length_km).speed(0.1));
                    if isolated {
                        ui.add(egui::DragValue::new(&mut s.r_per_km).speed(0.001));
                        ui.add(egui::DragValue::new(&mut s.x_per_km).speed(0.001));
                    } else {
                        ui.add(egui::DragValue::new(&mut s.r1_per_km).speed(0.001));
                        ui.add(egui::DragValue::new(&mut s.x1_per_km).speed(0.001));
                        ui.add(egui::DragValue::new(&mut s.r0_per_km).speed(0.001));
                        ui.add(egui::DragValue::new(&mut s.x0_per_km).speed(0.001));
                    }
                    ui.add(egui::DragValue::new(&mut s.supports));
                    ui.horizontal(|ui| {
                        let mut drop_tap = None;
                        for (ti, t) in s.taps.iter_mut().enumerate() {
                            ui.add(egui::DragValue::new(&mut t.r0).speed(0.1));
                            ui.add(egui::DragValue::new(&mut t.x0).speed(0.1));
                            if ui.small_button("×").clicked() {
                                drop_tap = Some(ti);
                            }
                        }
                        if let Some(ti) = drop_tap {
                            s.taps.remove(ti);
                        }
                        if ui.small_button("+").clicked() {
                            s.taps.push(fvb_core::omp::Tap::default());
                        }
                    });
                    if ui.small_button("Удалить").clicked() {
                        remove = Some(n);
                    }
                    ui.end_row();
                }
            });
            if let Some(n) = remove {
                self.omp_sections.remove(n);
            }
            ui.horizontal(|ui| {
                if ui.button("Добавить участок").clicked() {
                    self.omp_sections.push(fvb_core::omp::Section::default());
                }
                if ui.button("Рассчитать").clicked() {
                    self.run_omp();
                }
            });
            ui.separator();
            match self.omp_result() {
                Some(r) => {
                    ui.label(egui::RichText::new(r.text.trim()).strong().size(16.0));
                    ui.label(
                        egui::RichText::new(format!("Окно расчёта: отсчёты {} – {}", r.window.0.min(r.window.1), r.window.0.max(r.window.1)))
                            .weak(),
                    );
                }
                None => {
                    ui.label("Введите параметры линии и нажмите «Рассчитать».");
                }
            }
        });
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
