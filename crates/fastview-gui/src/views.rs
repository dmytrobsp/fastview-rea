//! Diagram views (круговая/дифференциальная/векторная/частотная/быстрый анализ/
//! ОМП) over the active document. The oscillogram (lane) view lives in
//! `lanes.rs`.

use crate::app::{AnalysisTab, App, Command, OmpMethod, Pane, Panes, View};
use eframe::egui;
use egui_plot::{Bar, BarChart, Line, LineStyle, Plot, PlotBounds, PlotPoints, Points, Polygon};
use fvb_core::geom::Shape;
use fvb_core::model::Dimension;
use num_complex::Complex64;

/// A locus on the circle / differential diagram: name, colour, points, and the
/// points at cursor 1 and cursor 2.
type Locus = (String, egui::Color32, Vec<[f64; 2]>, [f64; 2], [f64; 2]);

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
        if tab.is_none() && !self.panes.frequency {
            return;
        }
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
                        match active {
                            Pane::Vector => self.vector_ui(ui),
                            Pane::Circle | Pane::Differential => self.char_ui(ui),
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
        egui::CentralPanel::default().show_inside(ui, |ui| self.char_plot(ui));
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
        ui.horizontal(|ui| {
            ui.heading("Уставки");
            ui.checkbox(&mut self.show_reset, "возврат");
            ui.checkbox(&mut self.fill_regions, "заливка");
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.circle_between_cursors, "Диапазон между визирами");
            egui::ComboBox::from_id_salt("circreadout")
                .selected_text(["Без значений", "Re + jIm", "Модуль и угол"][self.circle_readout.min(2) as usize])
                .show_ui(ui, |ui| {
                    for (i, t) in ["Без значений", "Re + jIm", "Модуль и угол"].iter().enumerate() {
                        ui.selectable_value(&mut self.circle_readout, i as u8, *t);
                    }
                });
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
        // Loci (`0x43a880`): on the circle diagram every channel flagged for
        // it, as its per-sample phasor (Re, Im); on the differential diagram
        // the (Iторм, Iдиф) pairs. Samples N..end, or between the cursors.
        let between = self.circle_between_cursors;
        let readout = self.circle_readout;
        let loci: Vec<Locus> = self
            .active_doc()
            .map(|d| {
                let n = d.osc.samples_per_cycle().max(2);
                let len = d.osc.sample_count();
                let (c1, c2) = (d.main_cursor, d.aux_cursor);
                let range = if between { c1.min(c2).max(n)..=c1.max(c2) } else { n..=len.saturating_sub(1) };
                let mut out = Vec::new();
                let pick = |series: &dyn Fn(usize) -> [f64; 2]| -> (Vec<[f64; 2]>, [f64; 2], [f64; 2]) {
                    (range.clone().filter(|k| *k < len).map(series).collect(), series(c1.min(len.saturating_sub(1))), series(c2.min(len.saturating_sub(1))))
                };
                if dim == Dimension::Ohm {
                    for (i, ch) in d.osc.analog.iter().enumerate() {
                        if d.chans[i].on_circle {
                            let ph = &d.analysis.phasors[i];
                            let (pts, p1, p2) = pick(&|k| ph.get(k).map_or([0.0, 0.0], |z| [z.re, z.im]));
                            out.push((ch.id.clone(), d.chans[i].color, pts, p1, p2));
                        }
                    }
                    for c in d.computed.iter().filter(|c| c.on_circle) {
                        let series = |k: usize| {
                            let z = match &c.phasors {
                                Some(p) => p.get(k).copied().unwrap_or_default(),
                                None => fvb_core::signal::phasor_at(&c.samples, k, n),
                            };
                            [z.re, z.im]
                        };
                        let (pts, p1, p2) = pick(&series);
                        out.push((c.name.clone(), c.color, pts, p1, p2));
                    }
                } else {
                    for (dif, tor) in &d.dif_pairs {
                        let get = |name: &str| d.computed.iter().find(|c| c.name == name);
                        if let (Some(dc), Some(tc)) = (get(dif), get(tor)) {
                            let series = |k: usize| [tc.samples.get(k).copied().unwrap_or(0.0), dc.samples.get(k).copied().unwrap_or(0.0)];
                            let (pts, p1, p2) = pick(&series);
                            out.push((format!("{dif} / {tor}"), dc.color, pts, p1, p2));
                        }
                    }
                }
                out
            })
            .unwrap_or_default();
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
                for (name, color, pts, p1, p2) in &loci {
                    let label = match readout {
                        1 => format!("{name}: {:.3} {:+.3}j", p1[0], p1[1]),
                        2 => format!("{name}: {:.3} ∠{:.2}º", p1[0].hypot(p1[1]), p1[1].atan2(p1[0]).to_degrees()),
                        _ => name.clone(),
                    };
                    if pts.len() >= 2 {
                        pu.line(Line::new(PlotPoints::from(pts.clone())).color(*color).width(1.5_f32).name(label));
                    }
                    pu.points(Points::new(vec![*p1]).shape(egui_plot::MarkerShape::Plus).radius(6.0_f32).color(egui::Color32::RED));
                    pu.points(Points::new(vec![*p2]).shape(egui_plot::MarkerShape::Plus).radius(6.0_f32).color(egui::Color32::BLUE));
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
    /// Vector diagram (`0x490360`): channels flagged for it (analog and
    /// computed), their RMS phasors at the main cursor; angles absolute or
    /// relative to a chosen channel; lengths scaled per group (unit exactly
    /// "A" vs the rest) to a "nice" maximum.
    fn vector_ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let names: Vec<String> = self
                .active_doc()
                .map(|d| d.osc.analog.iter().map(|c| c.id.clone()).chain(d.computed.iter().map(|c| c.name.clone())).collect())
                .unwrap_or_default();
            // Options (MENU 355): the angle reference is "Абсолютный угол"
            // (36000) or a channel (36000 + i).
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.vec_show_values, "Показывать значения");
                ui.checkbox(&mut self.vec_radians, "Углы в радианах");
                ui.checkbox(&mut self.vec_direction_only, "Только направление");
                ui.label("Угол относительно:");
                let cur = self.vec_angle_ref.clone().unwrap_or_else(|| "Абсолютный угол".into());
                egui::ComboBox::from_id_salt("vecref").selected_text(cur).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.vec_angle_ref, None, "Абсолютный угол");
                    for n in &names {
                        ui.selectable_value(&mut self.vec_angle_ref, Some(n.clone()), n);
                    }
                });
            });
            ui.separator();
            let (radians, dir_only, show_values) = (self.vec_radians, self.vec_direction_only, self.vec_show_values);
            let angle_ref = self.vec_angle_ref.clone();
            let Some(d) = self.active_doc() else {
                ui.centered_and_justified(|ui| ui.label("Загрузите осциллограмму для векторной диаграммы."));
                return;
            };
            let k = d.main_cursor;
            let n = d.osc.samples_per_cycle().max(2);
            // (name, unit, phasor, colour) of every channel on the diagram.
            let phasor_of = |name: &str| -> Option<Complex64> {
                if let Some(i) = d.osc.analog.iter().position(|c| c.id == name) {
                    return d.analysis.phasors.get(i).and_then(|p| p.get(k)).copied();
                }
                d.computed.iter().find(|c| c.name == name).map(|c| match &c.phasors {
                    Some(p) => p.get(k).copied().unwrap_or_default(),
                    None => fvb_core::signal::phasor_at(&c.samples, k, n),
                })
            };
            let mut shown: Vec<(String, String, Complex64, egui::Color32)> = Vec::new();
            for (i, ch) in d.osc.analog.iter().enumerate() {
                if d.chans[i].on_vector {
                    shown.push((ch.id.clone(), ch.unit.clone(), phasor_of(&ch.id).unwrap_or_default(), d.chans[i].color));
                }
            }
            for c in d.computed.iter().filter(|c| c.on_vector) {
                shown.push((c.name.clone(), c.unit.clone(), phasor_of(&c.name).unwrap_or_default(), c.color));
            }
            let reference = angle_ref.as_deref().and_then(phasor_of).map(fvb_core::views::finite_or_zero);
            let is_current = |unit: &str| unit == "A";
            let group_max = |current: bool| {
                shown.iter().filter(|s| is_current(&s.1) == current).map(|s| fvb_core::views::finite_or_zero(s.2).norm()).fold(0.0, f64::max)
            };
            let (i_scale, u_scale) = (fvb_core::views::nice_scale(group_max(true)), fvb_core::views::nice_scale(group_max(false)));
            ui.label(format!(
                "t = {:.4} с (осн. визир)    U max = {:.1} {}V    I max = {:.1} {}A",
                d.osc.time.get(k).copied().unwrap_or(0.0),
                fvb_core::omp::si(u_scale).0,
                fvb_core::omp::si(u_scale).1,
                fvb_core::omp::si(i_scale).0,
                fvb_core::omp::si(i_scale).1,
            ));
            Plot::new("vectorplot").data_aspect(1.0).show(ui, |pu| {
                // Grid: circles and 12 radial lines every 30°.
                for r in [0.2, 0.4, 0.6, 0.8, 1.0] {
                    let c: Vec<[f64; 2]> = (0..=180)
                        .map(|dd| {
                            let a = dd as f64 * std::f64::consts::TAU / 180.0;
                            [r * a.cos(), r * a.sin()]
                        })
                        .collect();
                    pu.line(Line::new(PlotPoints::from(c)).color(egui::Color32::DARK_GRAY).width(0.5_f32));
                }
                for j in 0..12 {
                    let a = j as f64 * std::f64::consts::PI / 6.0;
                    pu.line(Line::new(PlotPoints::from(vec![[0.0, 0.0], [a.cos(), a.sin()]])).color(egui::Color32::DARK_GRAY).width(0.5_f32));
                }
                for (name, unit, z, color) in &shown {
                    let z = fvb_core::views::finite_or_zero(*z);
                    let ang = fvb_core::views::vector_angle(z, reference);
                    let scale = if is_current(unit) { i_scale } else { u_scale };
                    let len = if dir_only { 1.0 } else { z.norm() / scale };
                    let tip = [len * ang.cos(), len * ang.sin()];
                    let label = if show_values {
                        let (v, p) = fvb_core::omp::si(z.norm());
                        format!("{name}  {v:.2} {p}{unit}{}", fvb_core::views::angle_text(ang, radians))
                    } else {
                        name.clone()
                    };
                    let style = if unit == "V" { LineStyle::Solid } else { LineStyle::dotted_dense() };
                    pu.line(Line::new(PlotPoints::from(vec![[0.0, 0.0], tip])).color(*color).width(2.5_f32).style(style).name(label));
                    pu.points(Points::new(vec![tip]).radius(4.0_f32).color(*color));
                }
            });
        });
    }

    // ---- Частотная диаграмма: harmonic spectrum at the cursor ----
    /// Frequency diagram (`0x46f5a0`): for every channel with the frequency
    /// flag, the spectrum over `P` periods ending at the main cursor
    /// (`fvb_core::signal::spectrum`), bars at `k·f/P` Hz normalised to the
    /// channel's largest bin, labelled in absolute RMS or percent of the
    /// fundamental (or of the true RMS).
    fn frequency_ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Периодов:");
                ui.add(egui::DragValue::new(&mut self.freq_periods).range(1..=50));
                ui.label("Гармоник:");
                ui.add(egui::DragValue::new(&mut self.freq_harmonics).range(1..=200));
                ui.checkbox(&mut self.freq_percent, "В процентах");
                if self.freq_percent {
                    ui.checkbox(&mut self.freq_use_rms, "от действующего значения");
                }
            });
            let (periods, percent, use_rms) = (self.freq_periods.max(1), self.freq_percent, self.freq_use_rms);
            let Some(d) = self.active_doc() else {
                ui.centered_and_justified(|ui| ui.label("Загрузите осциллограмму для частотной диаграммы."));
                return;
            };
            let n = d.osc.samples_per_cycle().max(2);
            let k = d.main_cursor;
            // NumHarmonics is capped at the Nyquist bin of the window.
            let k_max = self.freq_harmonics.min(n * periods / 2);
            let f = d.osc.line_freq;
            let visible: Vec<usize> = (0..d.osc.analog.len()).filter(|i| d.chans[*i].on_freq && d.chans[*i].visible).collect();
            let width = 0.8 * f / periods as f64 / visible.len().max(1) as f64;
            let mut charts: Vec<BarChart> = Vec::new();
            for (si, &i) in visible.iter().enumerate() {
                let samples = &d.osc.analog[i].samples;
                let sp = fvb_core::signal::spectrum(samples, k, n, periods, k_max, d.osc.harmonic1_gain);
                let peak = sp.iter().copied().fold(0.0, f64::max).max(1e-300);
                let base = if use_rms {
                    fvb_core::signal::rms_at(samples, k, n * periods)
                } else {
                    sp.get(periods).copied().unwrap_or(0.0)
                };
                let bars: Vec<Bar> = sp
                    .iter()
                    .enumerate()
                    .map(|(h, v)| {
                        let shown = if percent { 100.0 * v / base.max(1e-300) } else { *v };
                        Bar::new(h as f64 * f / periods as f64 + si as f64 * width, v / peak)
                            .width(width)
                            .name(fvb_core::views::freq_label(shown))
                    })
                    .collect();
                let unit = if percent { ", %" } else { "" };
                charts.push(BarChart::new(bars).color(d.chans[i].color).name(format!("{}{unit}", d.osc.analog[i].id)));
            }
            Plot::new("freqplot")
                .legend(egui_plot::Legend::default())
                .x_axis_label("f, Гц")
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

/// Magnitudes of harmonics 0..=hmax from the one-cycle window ending at `center`.
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
