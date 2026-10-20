//! Diagram views (круговая/дифференциальная/векторная/частотная/быстрый анализ/
//! ОМП) over the active document. The oscillogram (lane) view lives in
//! `lanes.rs`.

use crate::app::{AnalysisTab, App, OmpMethod, View};
use eframe::egui;
use fvb_core::omp::NeutralMode;
use egui_plot::{Bar, BarChart, Line, LineStyle, Plot, PlotBounds, PlotPoints, Points, Polygon};
use fvb_core::geom::Shape;
use fvb_core::model::Dimension;
use num_complex::Complex64;

impl App {
    pub fn render_tab(&mut self, ctx: &egui::Context) {
        match self.view {
            View::Oscillogram => self.view_oscillogram(ctx),
            View::Frequency => self.view_frequency(ctx),
            View::Vector => self.view_vector(ctx),
            View::Circle | View::Differential => self.char_view(ctx),
            View::Analysis => self.view_analysis(ctx),
            View::Omp => self.view_omp(ctx),
        }
    }

    fn view_oscillogram(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| self.lanes_ui(ui));
    }

    // ---- Круговая / Дифференциальная: characteristics + impedance overlay ----
    fn char_view(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("char_side")
            .default_width(330.0)
            .show(ctx, |ui| self.char_side_panel(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.char_plot(ui));
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
                        if ui.selectable_label(self.selected_char == Some(*ci), name).clicked() {
                            self.selected_char = Some(*ci);
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

    fn char_plot(&mut self, ui: &mut egui::Ui) {
        let dim = self.current_dimension();
        let b = self.block_idx;
        struct D {
            name: String,
            color: egui::Color32,
            op: Vec<Shape>,
            rs: Vec<Shape>,
        }
        let mut draws = Vec::new();
        // Autoscale extent from the view-independent geometry, as the
        // original's autoscale pass builds regions without a clip box.
        let mut extent: Option<(f64, f64, f64, f64)> = None;
        let view = self.char_view;
        if let Some(block) = self.library.blocks.get(b) {
            for (ci, ch) in block.characteristics.iter().enumerate() {
                let Some(st) = self.chars.get(&(b, ci)) else { continue };
                if !st.visible {
                    continue;
                }
                let Some(geo) = &st.geo else { continue };
                if let Some((x0, y0, x1, y1)) = geo.bounds {
                    extent = Some(match extent {
                        None => (x0, y0, x1, y1),
                        Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
                    });
                }
                // Open polypoints are closed against the visible area.
                let in_view = match view {
                    Some(v) if fvb_core::geom::depends_on_view(ch) => {
                        Some(fvb_core::geom::build_geometry(ch, &st.figs, Some(v)))
                    }
                    _ => None,
                };
                let geo = in_view.as_ref().unwrap_or(geo);
                draws.push(D {
                    name: ch.name.clone(),
                    color: st.color,
                    op: geo.operate.shapes.clone(),
                    rs: if self.show_reset { geo.reset.shapes.clone() } else { vec![] },
                });
            }
        }
        let overlay = if dim == Dimension::Ohm {
            self.active_doc().map(|d| {
                let (a, m) = (d.aux_cursor.min(d.main_cursor), d.aux_cursor.max(d.main_cursor));
                let locus = d.analysis.phase_a_locus(a, m);
                let point = d.analysis.phase_impedance(0, d.main_cursor).map(|z| [z.re, z.im]);
                (locus, point)
            })
        } else {
            None
        };
        let fill = self.fill_regions;
        let bg = ui.visuals().extreme_bg_color;
        let shown = Plot::new("charplot")
            .data_aspect(1.0)
            .x_axis_label(dim.x_label())
            .y_axis_label(dim.y_label())
            .legend(egui_plot::Legend::default())
            .show(ui, |pu| {
                if pu.auto_bounds().any() {
                    if let Some((x0, y0, x1, y1)) = extent {
                        let (mx, my) = ((x1 - x0) * 0.05 + 1e-9, (y1 - y0) * 0.05 + 1e-9);
                        pu.set_plot_bounds(PlotBounds::from_min_max([x0 - mx, y0 - my], [x1 + mx, y1 + my]));
                    }
                }
                for d in &draws {
                    draw_region(pu, &d.op, d.color, fill, false, bg, &d.name);
                    draw_region(pu, &d.rs, d.color.gamma_multiply(0.6f32), false, true, bg, "");
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
                            Points::new(vec![*p]).radius(5.0_f32).color(egui::Color32::RED).name("Z текущая"),
                        );
                    }
                }
            });
        let pb = shown.transform.bounds();
        let next = Some((pb.min()[0], pb.min()[1], pb.max()[0], pb.max()[1]));
        if next != self.char_view {
            self.char_view = next;
            ui.ctx().request_repaint();
        }
    }

    // ---- Векторная диаграмма ----
    fn view_vector(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Options bar (MENU 355).
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.vec_show_values, "Показывать значения");
                ui.checkbox(&mut self.vec_radians, "Углы в радианах");
                ui.checkbox(&mut self.vec_direction_only, "Только направление");
                ui.checkbox(&mut self.vec_abs_angle, "Абсолютный угол");
            });
            ui.separator();
            let radians = self.vec_radians;
            let dir_only = self.vec_direction_only;
            let show_values = self.vec_show_values;
            let abs_angle = self.vec_abs_angle;
            let Some(d) = self.active_doc() else {
                ui.centered_and_justified(|ui| ui.label("Загрузите осциллограмму для векторной диаграммы."));
                return;
            };
            ui.label(format!("Векторы при t = {:.4} с (осн. визир)", d.osc.time.get(d.main_cursor).copied().unwrap_or(0.0)));
            let k = d.main_cursor;
            let is_voltage = |ch: &fvb_core::comtrade::AnalogChannel| ch.unit.to_lowercase().starts_with('v') || ch.unit.to_lowercase().contains('в');
            // Reference angle: when not absolute, measure relative to the first
            // visible voltage phasor.
            let ref_ang = if abs_angle {
                0.0
            } else {
                d.osc.analog.iter().enumerate()
                    .find(|(idx, ch)| d.chans[*idx].on_vector && is_voltage(ch))
                    .and_then(|(idx, _)| d.analysis.phasors.get(idx).and_then(|p| p.get(k)))
                    .map(|z| z.arg())
                    .unwrap_or(0.0)
            };
            let mut u_max = 1e-9;
            let mut i_max = 1e-9;
            for (idx, ch) in d.osc.analog.iter().enumerate() {
                if !d.chans[idx].on_vector {
                    continue;
                }
                let m = d.analysis.phasors.get(idx).and_then(|p| p.get(k)).map(|z| z.norm()).unwrap_or(0.0);
                if is_voltage(ch) {
                    u_max = f64::max(u_max, m);
                } else {
                    i_max = f64::max(i_max, m);
                }
            }
            struct Arrow { name: String, color: egui::Color32, v: [f64; 2], mag: f64, ang_deg: f64 }
            let mut arrows: Vec<Arrow> = Vec::new();
            for (idx, ch) in d.osc.analog.iter().enumerate() {
                if !d.chans[idx].on_vector {
                    continue;
                }
                let z = d.analysis.phasors.get(idx).and_then(|p| p.get(k)).copied().unwrap_or(Complex64::new(0.0, 0.0));
                let mag = z.norm();
                let ang = z.arg() - ref_ang;
                let draw_len = if dir_only { 1.0 } else if is_voltage(ch) { mag / u_max } else { mag / i_max };
                arrows.push(Arrow {
                    name: ch.id.clone(),
                    color: d.chans[idx].color,
                    v: [draw_len * ang.cos(), draw_len * ang.sin()],
                    mag,
                    ang_deg: ang.to_degrees(),
                });
            }
            Plot::new("vectorplot").data_aspect(1.0).show(ui, |pu| {
                let circle: Vec<[f64; 2]> = (0..=180)
                    .map(|dd| {
                        let a = (dd as f64) * std::f64::consts::TAU / 180.0;
                        [a.cos(), a.sin()]
                    })
                    .collect();
                pu.line(Line::new(PlotPoints::from(circle)).color(egui::Color32::DARK_GRAY));
                for a in &arrows {
                    let label = if show_values {
                        if radians {
                            format!("{}  {:.3} ∠{:.2} рад", a.name, a.mag, a.ang_deg.to_radians())
                        } else {
                            format!("{}  {:.3} ∠{:+.1}°", a.name, a.mag, a.ang_deg)
                        }
                    } else {
                        a.name.clone()
                    };
                    pu.line(Line::new(PlotPoints::from(vec![[0.0, 0.0], a.v])).color(a.color).width(2.5_f32).name(label));
                    pu.points(Points::new(vec![a.v]).radius(4.0_f32).color(a.color));
                }
            });
        });
    }

    // ---- Частотная диаграмма: harmonic spectrum at the cursor ----
    fn view_frequency(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(d) = self.active_doc() else {
                ui.centered_and_justified(|ui| ui.label("Загрузите осциллограмму для частотной диаграммы."));
                return;
            };
            ui.label("Гармонический состав в основном визире (доля от 1-й гармоники)");
            let n = d.osc.samples_per_cycle().max(2);
            let k = d.main_cursor;
            const HMAX: usize = 15;
            let mut charts: Vec<BarChart> = Vec::new();
            let mut slot = 0usize;
            let visible: Vec<usize> = (0..d.osc.analog.len()).filter(|i| d.chans[*i].on_freq && d.chans[*i].visible).collect();
            let width = 0.8 / visible.len().max(1) as f64;
            for (si, &i) in visible.iter().enumerate() {
                let mags = harmonics(&d.osc.analog[i].samples, k, n, HMAX);
                let base = mags.get(1).copied().unwrap_or(0.0).max(1e-9);
                let bars: Vec<Bar> = (1..=HMAX)
                    .map(|h| Bar::new(h as f64 + si as f64 * width, mags.get(h).copied().unwrap_or(0.0) / base).width(width))
                    .collect();
                charts.push(BarChart::new(bars).color(d.chans[i].color).name(&d.osc.analog[i].id));
                slot += 1;
            }
            let _ = slot;
            Plot::new("freqplot")
                .legend(egui_plot::Legend::default())
                .x_axis_label("№ гармоники")
                .show(ui, |pu| {
                    for c in charts {
                        pu.bar_chart(c);
                    }
                });
        });
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
            let Some(d) = self.active_doc() else {
                ui.centered_and_justified(|ui| ui.label("Загрузите осциллограмму для анализа."));
                return;
            };
            let k = d.main_cursor;
            let t = d.osc.time.get(k).copied().unwrap_or(0.0);
            ui.label(egui::RichText::new(format!("Основной визир: t = {:.4} с (отсчёт {k})", t)).weak());
            ui.add_space(4.0);
            match self.analysis_tab {
                AnalysisTab::Symmetric => {
                    let (u0, u1, u2) = d.analysis.u_symmetric(k);
                    let (i0, i1, i2) = d.analysis.i_symmetric(k);
                    phasor_table(ui, "symgrid", "Величина", &[
                        ("3U0", u0), ("U1", u1), ("U2", u2), ("3I0", i0), ("I1", i1), ("I2", i2),
                    ]);
                }
                AnalysisTab::Zero => {
                    let (u0x3, _, _) = d.analysis.u_symmetric(k);
                    let (i0x3, _, _) = d.analysis.i_symmetric(k);
                    ui.label("Составляющие нулевой последовательности (3U0, 3I0 — суммарные остаточные величины):");
                    phasor_table(ui, "zerogrid", "Величина", &[
                        ("3U0", u0x3), ("U0", u0x3 / 3.0), ("3I0", i0x3), ("I0", i0x3 / 3.0),
                    ]);
                }
                AnalysisTab::PhasePhase => {
                    let z = d.analysis.phase_phase_impedances(k);
                    impedance_table(ui, "ppgrid", "Контур", &[("Zab", z[0]), ("Zbc", z[1]), ("Zca", z[2])]);
                }
                AnalysisTab::Phase => {
                    let z = [
                        ("Zа", d.analysis.phase_impedance(0, k)),
                        ("Zb", d.analysis.phase_impedance(1, k)),
                        ("Zc", d.analysis.phase_impedance(2, k)),
                    ];
                    impedance_table(ui, "phgrid", "Контур", &z);
                }
                AnalysisTab::Differential => {
                    let (ia, ib, ic) = d.analysis.i_phase(k);
                    let (id, ir) = d.analysis.differential_currents(k);
                    phasor_table(ui, "dif_i", "Фазный ток", &[("Ia", ia), ("Ib", ib), ("Ic", ic)]);
                    ui.separator();
                    egui::Grid::new("difgrid").striped(true).num_columns(2).show(ui, |ui| {
                        ui.label("Дифференциальный ток Iд = |Ia+Ib+Ic|");
                        ui.label(format!("{id:.3}"));
                        ui.end_row();
                        ui.label("Тормозной ток Iт = (|Ia|+|Ib|+|Ic|)/2");
                        ui.label(format!("{ir:.3}"));
                        ui.end_row();
                        ui.label("Отношение Iд/Iт");
                        ui.label(if ir > 1e-9 { format!("{:.3}", id / ir) } else { "—".into() });
                        ui.end_row();
                    });
                }
                AnalysisTab::Formula => {}
            }
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
    fn view_omp(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Определение места повреждения (ОМП)");
            ui.horizontal_wrapped(|ui| {
                ui.label("Метод:");
                ui.selectable_value(&mut self.omp_method, OmpMethod::SingleEnded, "Односторонний замер");
                ui.selectable_value(&mut self.omp_method, OmpMethod::DoubleEnded, "Двусторонний замер");
                ui.separator();
                ui.label("Нейтраль:");
                ui.selectable_value(&mut self.neutral, NeutralMode::Grounded, "Глухозаземлённая");
                ui.selectable_value(&mut self.neutral, NeutralMode::Isolated, "Изолированная");
            });
            ui.separator();
            egui::Grid::new("ompinputs").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("R1 уд., Ом/км");
                ui.add(egui::DragValue::new(&mut self.line.r1_per_km).speed(0.001));
                ui.label("X1 уд., Ом/км");
                ui.add(egui::DragValue::new(&mut self.line.x1_per_km).speed(0.001));
                ui.end_row();
                if self.neutral == NeutralMode::Grounded {
                    ui.label("R0 уд., Ом/км");
                    ui.add(egui::DragValue::new(&mut self.line.r0_per_km).speed(0.001));
                    ui.label("X0 уд., Ом/км");
                    ui.add(egui::DragValue::new(&mut self.line.x0_per_km).speed(0.001));
                    ui.end_row();
                }
                ui.label("Длина линии, км");
                ui.add(egui::DragValue::new(&mut self.line.length_km).speed(0.1));
                ui.end_row();
            });
            // Remote-end selection for the double-ended method.
            if self.omp_method == OmpMethod::DoubleEnded {
                let names: Vec<(usize, String)> = self
                    .docs
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| Some(*i) != self.active)
                    .map(|(i, d)| (i, d.name.clone()))
                    .collect();
                ui.horizontal(|ui| {
                    ui.label("Дальний конец (вторая осциллограмма):");
                    let cur = self.omp_remote.and_then(|i| self.docs.get(i)).map(|d| d.name.clone()).unwrap_or_else(|| "— не выбрано —".into());
                    egui::ComboBox::from_id_salt("ompremote").selected_text(cur).show_ui(ui, |ui| {
                        for (i, name) in &names {
                            ui.selectable_value(&mut self.omp_remote, Some(*i), name);
                        }
                    });
                });
            }
            ui.separator();

            let line = self.line;
            let neutral = self.neutral;
            let method = self.omp_method;
            let remote = self.omp_remote;
            let Some(d) = self.active_doc() else {
                ui.label("Загрузите осциллограмму и установите основной визир в аварийный режим.");
                return;
            };
            let k = d.main_cursor;
            let ua = phase_ph(d, d.analysis.u_abc[0], k);
            let ub = phase_ph(d, d.analysis.u_abc[1], k);
            let ia = phase_ph(d, d.analysis.i_abc[0], k);
            let ib = phase_ph(d, d.analysis.i_abc[1], k);
            if d.analysis.u_abc[0].is_none() || d.analysis.i_abc[0].is_none() {
                ui.label("Не удалось определить фазные каналы Ua/Ia в осциллограмме.");
                return;
            }
            let (pa, pb, pc) = d.analysis.i_phase(k);
            let i3i0 = pa + pb + pc;

            let result = match method {
                OmpMethod::SingleEnded => match neutral {
                    NeutralMode::Grounded => {
                        fvb_core::omp::single_ended_grounded(ua, ia, i3i0, &line).map(|dist| (dist, format!("Zпетли ф.A = {:.3}{:+.3}j Ом", (ua / ia).re, (ua / ia).im)))
                    }
                    NeutralMode::Isolated => {
                        let uab = ua - ub;
                        let iab = ia - ib;
                        fvb_core::omp::single_ended_distance(uab, iab, &line).map(|dist| (dist, format!("Zпетли AB = {:.3}{:+.3}j Ом", (uab / iab).re, (uab / iab).im)))
                    }
                },
                OmpMethod::DoubleEnded => {
                    match remote.and_then(|i| self.docs.get(i)) {
                        Some(r) => {
                            let rk = r.main_cursor;
                            let un = phase_ph(r, r.analysis.u_abc[0], rk);
                            let in_ = phase_ph(r, r.analysis.i_abc[0], rk);
                            if r.analysis.u_abc[0].is_none() || r.analysis.i_abc[0].is_none() {
                                None
                            } else {
                                fvb_core::omp::double_ended_distance(ua, ia, un, in_, &line).map(|dist| (dist, format!("дальний конец: {}", r.name)))
                            }
                        }
                        None => {
                            ui.label("Выберите вторую осциллограмму (дальний конец) для двустороннего замера.");
                            return;
                        }
                    }
                }
            };
            match result {
                Some((dist, detail)) => {
                    ui.label(egui::RichText::new(format!("Расстояние до места повреждения: {dist:.2} км")).strong().size(18.0));
                    if line.length_km > 0.0 {
                        ui.label(format!("({:.1}% длины линии)", 100.0 * dist / line.length_km));
                    }
                    ui.label(egui::RichText::new(detail).weak());
                }
                None => {
                    ui.label("Недостаточно данных для расчёта (проверьте каналы/ток/дальний конец).");
                }
            }
        });
    }
}

/// Phasor of an optional channel index at sample `k`.
fn phase_ph(d: &crate::app::Doc, idx: Option<usize>, k: usize) -> Complex64 {
    idx.and_then(|i| d.analysis.phasors.get(i)).and_then(|p| p.get(k)).copied().unwrap_or(Complex64::new(0.0, 0.0))
}

fn phasor_table(ui: &mut egui::Ui, id: &str, first_col: &str, rows: &[(&str, Complex64)]) {
    egui::Grid::new(id).striped(true).num_columns(3).show(ui, |ui| {
        ui.label(first_col);
        ui.label("Модуль");
        ui.label("Угол");
        ui.end_row();
        for (name, z) in rows {
            ui.label(*name);
            ui.label(format!("{:.3}", z.norm()));
            ui.label(format!("{:+.1}°", z.arg().to_degrees()));
            ui.end_row();
        }
    });
}

fn impedance_table(ui: &mut egui::Ui, id: &str, first_col: &str, rows: &[(&str, Option<Complex64>)]) {
    egui::Grid::new(id).striped(true).num_columns(4).show(ui, |ui| {
        ui.label(first_col);
        ui.label("R, Ом");
        ui.label("X, Ом");
        ui.label("|Z|, Ом");
        ui.end_row();
        for (name, z) in rows {
            ui.label(*name);
            if let Some(z) = z {
                ui.label(format!("{:.3}", z.re));
                ui.label(format!("{:.3}", z.im));
                ui.label(format!("{:.3}", z.norm()));
            } else {
                ui.label("—");
                ui.label("—");
                ui.label("—");
            }
            ui.end_row();
        }
    });
}

/// Magnitudes of harmonics 0..=hmax from the one-cycle window ending at `center`.
/// RMS magnitudes of harmonics 0..=hmax at sample `center`
/// ([`fvb_core::signal::harmonic_rms`], the original's `fourie:`).
fn harmonics(samples: &[f64], center: usize, n: usize, hmax: usize) -> Vec<f64> {
    (0..=hmax).map(|h| fvb_core::signal::harmonic_rms(samples, center, n, h)).collect()
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
                                .fill_color(color.gamma_multiply(0.12f32))
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
                    } else if fill && !reset {
                        pu.polygon(
                            Polygon::new(PlotPoints::from(ring.clone()))
                                .fill_color(bg)
                                .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.7f32))),
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
    }
}

#[cfg(test)]
mod tests {
    use super::harmonics;

    #[test]
    fn harmonics_finds_the_fundamental_of_a_sine() {
        let n = 40usize;
        let cycles = 6;
        let s: Vec<f64> = (0..n * cycles)
            .map(|i| 100.0 * (2.0 * std::f64::consts::PI * (i as f64) / n as f64).sin())
            .collect();
        let m = harmonics(&s, n * cycles - 1, n, 10);
        // fundamental (h=1) dominates; harmonics 2..=10 are ~0.
        assert!(m[1] > 60.0, "fundamental magnitude {}", m[1]);
        for h in 2..=10 {
            assert!(m[h] < m[1] * 0.05, "harmonic {h} should be small: {}", m[h]);
        }
    }
}
