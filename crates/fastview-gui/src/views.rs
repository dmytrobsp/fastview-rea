//! Per-tab view rendering for the FastView reconstruction.

use crate::app::{App, Tab};
use eframe::egui;
use egui_plot::{Line, LineStyle, Plot, PlotPoints, Points, Polygon};
use fvb_core::geom::Shape;
use fvb_core::model::Dimension;
use num_complex::Complex64;

/// Inferred editor kind for a protection-setting parameter, from its name.
enum Widget {
    /// Enumerated type selector (e.g. `_тип`): integer 1..=max with a label.
    Enum(i64),
    /// Boolean "ввод"/enable flag (0/1).
    Bool,
    Angle,
    Ohm,
    Coeff,
    Number,
}

fn infer_widget(name: &str) -> (Widget, &'static str) {
    let n = name.to_lowercase();
    if n.contains("тип") {
        (Widget::Enum(3), "")
    } else if n.contains("ввод") || n.contains("направленност") {
        (Widget::Bool, "")
    } else if n.contains("_ф") || n.ends_with('ф') || n.contains("угол") || n.contains("фмч") || n.contains("_фн") {
        (Widget::Angle, "°")
    } else if n.contains('z') || n.contains('r') || n.contains('x') || n.contains("сопрот") {
        (Widget::Ohm, "Ом")
    } else if n.contains("кт") || n.contains("кс") || n.contains("коэф") {
        (Widget::Coeff, "")
    } else {
        (Widget::Number, "")
    }
}

impl App {
    pub fn render_tab(&mut self, ctx: &egui::Context) {
        match self.tab {
            Tab::Circle => self.view_circle(ctx),
            Tab::Oscillogram => self.view_oscillogram(ctx),
            Tab::Vector => self.view_vector(ctx),
            Tab::Analysis => self.view_analysis(ctx),
            Tab::Omp => self.view_omp(ctx),
        }
    }

    // ---- Круговая диаграмма: characteristics + impedance overlay ----
    fn view_circle(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("circle_side")
            .default_width(330.0)
            .show(ctx, |ui| self.circle_side_panel(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.circle_plot(ui));
    }

    fn circle_side_panel(&mut self, ui: &mut egui::Ui) {
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
                self.selected = None;
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
        egui::ScrollArea::vertical()
            .id_salt("charlist")
            .max_height(ui.available_height() * 0.4)
            .show(ui, |ui| {
                for (ci, name) in &names {
                    ui.horizontal(|ui| {
                        if let Some(st) = self.chars.get_mut(&(b, *ci)) {
                            ui.label(egui::RichText::new("⬤").color(st.color));
                            ui.checkbox(&mut st.visible, "");
                        }
                        if ui.selectable_label(self.selected == Some(*ci), name).clicked() {
                            self.selected = Some(*ci);
                        }
                    });
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Уставки");
            ui.checkbox(&mut self.show_reset, "возврат");
            ui.checkbox(&mut self.fill_regions, "заливка");
        });
        self.settings_editor(ui);
    }

    fn settings_editor(&mut self, ui: &mut egui::Ui) {
        let b = self.block_idx;
        let Some(ci) = self.selected else {
            ui.label("Выберите характеристику.");
            return;
        };
        let params: Vec<String> = self
            .library
            .blocks
            .get(b)
            .and_then(|blk| blk.characteristics.get(ci))
            .map(|ch| ch.parameters.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default();
        if params.is_empty() {
            ui.label("Нет настраиваемых уставок.");
            return;
        }
        let mut changed = false;
        egui::ScrollArea::vertical().id_salt("paramscroll").show(ui, |ui| {
            egui::Grid::new("paramgrid").num_columns(3).striped(true).show(ui, |ui| {
                for name in &params {
                    let (widget, unit) = infer_widget(name);
                    ui.label(name);
                    if let Some(st) = self.chars.get_mut(&(b, ci)) {
                        let v = st.settings.entry(name.clone()).or_insert(0.0);
                        let resp = match widget {
                            Widget::Bool => {
                                let mut on = *v != 0.0;
                                let r = ui.checkbox(&mut on, "");
                                if r.changed() {
                                    *v = if on { 1.0 } else { 0.0 };
                                }
                                r
                            }
                            Widget::Enum(max) => ui.add(
                                egui::DragValue::new(v)
                                    .range(1.0..=max as f64)
                                    .speed(0.05)
                                    .fixed_decimals(0),
                            ),
                            Widget::Angle => ui.add(egui::DragValue::new(v).speed(0.5).suffix("°")),
                            Widget::Ohm => ui.add(egui::DragValue::new(v).speed(0.05)),
                            Widget::Coeff => ui.add(egui::DragValue::new(v).speed(0.01)),
                            Widget::Number => ui.add(egui::DragValue::new(v).speed(0.05)),
                        };
                        changed |= resp.changed();
                    }
                    ui.label(unit);
                    ui.end_row();
                }
            });
            if ui.button("Сбросить уставки").clicked() {
                if let (Some(blk), Some(st)) =
                    (self.library.blocks.get(b), self.chars.get_mut(&(b, ci)))
                {
                    if let Some(ch) = blk.characteristics.get(ci) {
                        st.settings = ch.default_settings();
                        changed = true;
                    }
                }
            }
        });
        if changed {
            self.recompute_one(ci);
        }
    }

    fn circle_plot(&mut self, ui: &mut egui::Ui) {
        let dim = self.current_dimension();
        let b = self.block_idx;
        struct D {
            name: String,
            color: egui::Color32,
            op: Vec<Shape>,
            rs: Vec<Shape>,
        }
        let mut draws = Vec::new();
        if let Some(block) = self.library.blocks.get(b) {
            for ci in 0..block.characteristics.len() {
                if let Some(st) = self.chars.get(&(b, ci)) {
                    if !st.visible {
                        continue;
                    }
                    if let Some(geo) = &st.geo {
                        draws.push(D {
                            name: block.characteristics[ci].name.clone(),
                            color: st.color,
                            op: geo.operate.shapes.clone(),
                            rs: if self.show_reset {
                                geo.reset.shapes.clone()
                            } else {
                                vec![]
                            },
                        });
                    }
                }
            }
        }
        // Impedance overlay (phase-A locus between cursors) when an oscillogram is
        // loaded and we're in the OHM plane.
        let overlay = if dim == Dimension::Ohm {
            self.loaded.as_ref().map(|l| {
                let (a, m) = (l.aux_cursor.min(l.main_cursor), l.aux_cursor.max(l.main_cursor));
                let locus = l.analysis.phase_a_locus(a, m);
                let point = l.analysis.phase_impedance(0, l.main_cursor).map(|z| [z.re, z.im]);
                (locus, point)
            })
        } else {
            None
        };
        let fill = self.fill_regions;
        let bg = ui.visuals().extreme_bg_color;
        Plot::new("circleplot")
            .data_aspect(1.0)
            .x_axis_label(dim.x_label())
            .y_axis_label(dim.y_label())
            .legend(egui_plot::Legend::default())
            .show(ui, |pu| {
                for d in &draws {
                    draw_region(pu, &d.op, d.color, fill, false, bg, &d.name);
                    draw_region(pu, &d.rs, d.color.gamma_multiply(0.6), false, true, bg, "");
                }
                if let Some((locus, point)) = &overlay {
                    if locus.len() >= 2 {
                        pu.line(
                            Line::new(PlotPoints::from(locus.clone()))
                                .color(egui::Color32::from_rgb(255, 215, 0))
                                .width(2.0_f32)
                                .name("Траектория Z (ф.A)"),
                        );
                    }
                    if let Some(p) = point {
                        pu.points(
                            Points::new(vec![*p])
                                .radius(5.0_f32)
                                .color(egui::Color32::RED)
                                .name("Z текущая"),
                        );
                    }
                }
            });
    }

    // ---- Осциллограмма ----
    fn view_oscillogram(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("osc_side").default_width(300.0).show(ctx, |ui| {
            self.osc_side_panel(ui);
        });
        egui::CentralPanel::default().show(ctx, |ui| self.osc_plot(ui));
    }

    fn osc_side_panel(&mut self, ui: &mut egui::Ui) {
        let Some(l) = &mut self.loaded else {
            ui.add_space(8.0);
            ui.label("Осциллограмма не загружена.");
            ui.label("Файл → Открыть COMTRADE, либо «Демо-осциллограмма».");
            return;
        };
        ui.add_space(4.0);
        ui.label(egui::RichText::new(&l.name).strong());
        ui.heading("Каналы");
        egui::ScrollArea::vertical().id_salt("chanscroll").max_height(ui.available_height() * 0.5).show(ui, |ui| {
            for (i, ch) in l.osc.analog.iter().enumerate() {
                if let Some(v) = l.chan_visible.get_mut(i) {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⬤").color(App::palette(i)));
                        ui.checkbox(v, format!("{} [{}]", ch.id, ch.unit));
                    });
                }
            }
        });
        ui.separator();
        ui.heading("Визиры");
        let n = l.osc.sample_count().saturating_sub(1);
        ui.add(egui::Slider::new(&mut l.main_cursor, 0..=n).text("основной"));
        ui.add(egui::Slider::new(&mut l.aux_cursor, 0..=n).text("вспомог."));
        let t_main = l.osc.time.get(l.main_cursor).copied().unwrap_or(0.0);
        ui.label(format!("t(осн.) = {:.4} с", t_main));
        ui.separator();
        ui.heading("Действующие значения (осн. визир)");
        egui::Grid::new("rmsgrid").striped(true).show(ui, |ui| {
            for (i, ch) in l.osc.analog.iter().enumerate() {
                let ph = l.analysis.phasors.get(i).and_then(|p| p.get(l.main_cursor)).copied().unwrap_or(Complex64::new(0.0, 0.0));
                ui.label(&ch.id);
                ui.label(format!("{:.3}", ph.norm()));
                ui.label(format!("{:+.1}°", ph.arg().to_degrees()));
                ui.end_row();
            }
        });
    }

    fn osc_plot(&mut self, ui: &mut egui::Ui) {
        let Some(l) = &self.loaded else {
            ui.centered_and_justified(|ui| ui.label("Нет данных осциллограммы."));
            return;
        };
        let main_t = l.osc.time.get(l.main_cursor).copied().unwrap_or(0.0);
        let aux_t = l.osc.time.get(l.aux_cursor).copied().unwrap_or(0.0);
        let mut lines: Vec<(String, egui::Color32, Vec<[f64; 2]>)> = Vec::new();
        for (i, ch) in l.osc.analog.iter().enumerate() {
            if !l.chan_visible.get(i).copied().unwrap_or(false) {
                continue;
            }
            let pts: Vec<[f64; 2]> = l
                .osc
                .time
                .iter()
                .zip(ch.samples.iter())
                .map(|(t, v)| [*t, *v])
                .collect();
            lines.push((ch.id.clone(), App::palette(i), pts));
        }
        // computed channels
        for (ci, c) in self.computed.iter().enumerate() {
            let pts: Vec<[f64; 2]> = l
                .osc
                .time
                .iter()
                .zip(c.samples.iter())
                .map(|(t, v)| [*t, *v])
                .collect();
            lines.push((c.name.clone(), App::palette(ci + 7), pts));
        }
        Plot::new("oscplot")
            .legend(egui_plot::Legend::default())
            .x_axis_label("t, с")
            .show(ui, |pu| {
                for (name, color, pts) in &lines {
                    pu.line(Line::new(PlotPoints::from(pts.clone())).color(*color).name(name));
                }
                // cursors
                pu.line(
                    Line::new(PlotPoints::from(vec![[main_t, f64::NEG_INFINITY], [main_t, f64::INFINITY]]))
                        .color(egui::Color32::RED)
                        .style(LineStyle::dashed_dense())
                        .name("осн. визир"),
                );
                pu.line(
                    Line::new(PlotPoints::from(vec![[aux_t, f64::NEG_INFINITY], [aux_t, f64::INFINITY]]))
                        .color(egui::Color32::LIGHT_BLUE)
                        .style(LineStyle::dashed_dense())
                        .name("всп. визир"),
                );
            });
    }

    // ---- Векторная диаграмма ----
    fn view_vector(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(l) = &self.loaded else {
                ui.centered_and_justified(|ui| ui.label("Загрузите осциллограмму для векторной диаграммы."));
                return;
            };
            ui.label(format!("Векторы при t = {:.4} с (осн. визир)", l.osc.time.get(l.main_cursor).copied().unwrap_or(0.0)));
            let k = l.main_cursor;
            // normalise U and I separately to comparable arrow lengths
            let mut u_max = 1e-9;
            let mut i_max = 1e-9;
            for (idx, ch) in l.osc.analog.iter().enumerate() {
                let m = l.analysis.phasors.get(idx).and_then(|p| p.get(k)).map(|z| z.norm()).unwrap_or(0.0);
                if ch.unit.to_lowercase().starts_with('v') {
                    u_max = f64::max(u_max, m);
                } else {
                    i_max = f64::max(i_max, m);
                }
            }
            let mut arrows: Vec<(String, egui::Color32, [f64; 2])> = Vec::new();
            for (idx, ch) in l.osc.analog.iter().enumerate() {
                let z = l.analysis.phasors.get(idx).and_then(|p| p.get(k)).copied().unwrap_or(Complex64::new(0.0, 0.0));
                let is_v = ch.unit.to_lowercase().starts_with('v');
                let scale = if is_v { 1.0 / u_max } else { 1.0 / i_max };
                arrows.push((ch.id.clone(), App::palette(idx), [z.re * scale, z.im * scale]));
            }
            Plot::new("vectorplot").data_aspect(1.0).show(ui, |pu| {
                // unit circle reference
                let circle: Vec<[f64; 2]> = (0..=180).map(|d| {
                    let a = (d as f64) * std::f64::consts::TAU / 180.0;
                    [a.cos(), a.sin()]
                }).collect();
                pu.line(Line::new(PlotPoints::from(circle)).color(egui::Color32::DARK_GRAY));
                for (name, color, v) in &arrows {
                    pu.line(Line::new(PlotPoints::from(vec![[0.0, 0.0], *v])).color(*color).width(2.5_f32).name(name));
                    pu.points(Points::new(vec![*v]).radius(4.0_f32).color(*color));
                }
            });
        });
    }

    // ---- Быстрый анализ ----
    fn view_analysis(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("analysis_formula").default_width(320.0).show(ctx, |ui| {
            ui.heading("Редактор формул");
            ui.label("Расчётный канал по отсчётам:");
            ui.horizontal(|ui| {
                ui.label("имя");
                ui.text_edit_singleline(&mut self.formula_name);
            });
            ui.horizontal(|ui| {
                ui.label("формула");
                ui.text_edit_singleline(&mut self.formula_text);
            });
            if ui.button("Добавить канал").clicked() {
                self.add_computed();
            }
            ui.separator();
            ui.label("Доступно: имена каналов, + - * / ^, mod:() sqrt:() sin:() cos:() tg:() ctg:()");
            ui.separator();
            if !self.computed.is_empty() {
                ui.label("Расчётные каналы:");
                let mut remove = None;
                for (i, c) in self.computed.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} = {}", c.name, c.formula));
                        if ui.small_button("✕").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    self.computed.remove(i);
                }
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(l) = &self.loaded else {
                ui.centered_and_justified(|ui| ui.label("Загрузите осциллограмму для анализа."));
                return;
            };
            let k = l.main_cursor;
            ui.heading("Симметричные составляющие (осн. визир)");
            let (u0, u1, u2) = l.analysis.u_symmetric(k);
            let (i0, i1, i2) = l.analysis.i_symmetric(k);
            egui::Grid::new("symgrid").striped(true).num_columns(3).show(ui, |ui| {
                ui.label("Величина");
                ui.label("Модуль");
                ui.label("Угол");
                ui.end_row();
                for (name, z) in [
                    ("3U0", u0), ("U1", u1), ("U2", u2),
                    ("3I0", i0), ("I1", i1), ("I2", i2),
                ] {
                    ui.label(name);
                    ui.label(format!("{:.3}", z.norm()));
                    ui.label(format!("{:+.1}°", z.arg().to_degrees()));
                    ui.end_row();
                }
            });
            ui.separator();
            ui.heading("Сопротивления (осн. визир)");
            egui::Grid::new("zgrid").striped(true).num_columns(3).show(ui, |ui| {
                ui.label("Контур");
                ui.label("R, Ом");
                ui.label("X, Ом");
                ui.end_row();
                for (name, ph) in [("Zа", 0usize), ("Zb", 1), ("Zc", 2)] {
                    if let Some(z) = l.analysis.phase_impedance(ph, k) {
                        ui.label(name);
                        ui.label(format!("{:.3}", z.re));
                        ui.label(format!("{:.3}", z.im));
                        ui.end_row();
                    }
                }
            });
        });
    }

    // ---- ОМП ----
    fn view_omp(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Определение места повреждения (одностороннего замера)");
            ui.separator();
            egui::Grid::new("ompinputs").num_columns(2).show(ui, |ui| {
                ui.label("R1 уд., Ом/км");
                ui.add(egui::DragValue::new(&mut self.line.r1_per_km).speed(0.001));
                ui.end_row();
                ui.label("X1 уд., Ом/км");
                ui.add(egui::DragValue::new(&mut self.line.x1_per_km).speed(0.001));
                ui.end_row();
                ui.label("Длина линии, км");
                ui.add(egui::DragValue::new(&mut self.line.length_km).speed(0.1));
                ui.end_row();
            });
            ui.separator();
            let Some(l) = &self.loaded else {
                ui.label("Загрузите осциллограмму и установите основной визир в аварийный режим.");
                return;
            };
            let k = l.main_cursor;
            let ua = l.analysis.phasors.get(l.analysis.u_abc[0].unwrap_or(0)).and_then(|p| p.get(k)).copied();
            let ia = l.analysis.phasors.get(l.analysis.i_abc[0].unwrap_or(0)).and_then(|p| p.get(k)).copied();
            match (ua, ia, l.analysis.u_abc[0], l.analysis.i_abc[0]) {
                (Some(u), Some(i), Some(_), Some(_)) => {
                    if let Some(d) = fvb_core::omp::single_ended_distance(u, i, &self.line) {
                        ui.label(egui::RichText::new(format!("Расстояние до места повреждения: {:.2} км", d)).strong().size(18.0));
                        if self.line.length_km > 0.0 {
                            ui.label(format!("({:.1}% длины линии)", 100.0 * d / self.line.length_km));
                        }
                        let z = u / i;
                        ui.label(format!("Zпетли фазы A = {:.3} + j{:.3} Ом", z.re, z.im));
                    } else {
                        ui.label("Недостаточный ток для расчёта.");
                    }
                }
                _ => {
                    ui.label("Не удалось определить фазные каналы Ua/Ia в осциллограмме.");
                }
            }
        });
    }
}

/// Draw a region (operate or reset), punching holes out of the fill.
fn draw_region(
    pu: &mut egui_plot::PlotUi,
    shapes: &[Shape],
    color: egui::Color32,
    fill: bool,
    reset: bool,
    bg: egui::Color32,
    name: &str,
) {
    let style = if reset { LineStyle::dashed_dense() } else { LineStyle::Solid };
    for shape in shapes {
        match shape {
            Shape::Region(rings) => {
                for (idx, ring) in rings.iter().enumerate() {
                    if ring.len() < 3 {
                        continue;
                    }
                    if idx == 0 {
                        if fill && !reset {
                            let mut poly = Polygon::new(PlotPoints::from(ring.clone()))
                                .fill_color(color.gamma_multiply(0.12))
                                .stroke(egui::Stroke::new(1.6_f32, color))
                                .style(style);
                            if !name.is_empty() {
                                poly = poly.name(name);
                            }
                            pu.polygon(poly);
                        } else {
                            let mut lp = ring.clone();
                            if let Some(f) = lp.first().copied() {
                                lp.push(f);
                            }
                            let mut ln = Line::new(PlotPoints::from(lp)).color(color).width(1.6_f32).style(style);
                            if !name.is_empty() {
                                ln = ln.name(name);
                            }
                            pu.line(ln);
                        }
                    } else {
                        // hole: punch it out of the fill with the background colour
                        if fill && !reset {
                            pu.polygon(
                                Polygon::new(PlotPoints::from(ring.clone()))
                                    .fill_color(bg)
                                    .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.7))),
                            );
                        } else {
                            let mut lp = ring.clone();
                            if let Some(f) = lp.first().copied() {
                                lp.push(f);
                            }
                            pu.line(Line::new(PlotPoints::from(lp)).color(color).width(1.2_f32).style(style));
                        }
                    }
                }
            }
            Shape::Polyline(pl) => {
                let mut ln = Line::new(PlotPoints::from(pl.clone())).color(color).width(1.8_f32).style(style);
                if !name.is_empty() {
                    ln = ln.name(name);
                }
                pu.line(ln);
            }
        }
    }
}
