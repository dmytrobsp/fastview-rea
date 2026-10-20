//! The lane-based oscillogram view — the original's primary document window.
//!
//! Each analog/computed/digital channel is a horizontal lane with a left stats
//! panel (name badge · value@cursor · Max/Min/СКЗ/A1) and a waveform. Two
//! vertical визиры (red main, blue aux) with a shaded selection between them,
//! a shared time axis, on-waveform comments, and per-channel / time-axis context
//! menus (cmd ids from `notes/ui/menus.txt`). Lanes have a minimum height and the
//! view scrolls vertically when there are many channels.

use crate::app::{App, Command, CommentEdit, ValueMode};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

const LEFT_W: f32 = 158.0;
const AXIS_H: f32 = 26.0;
const MIN_LANE: f32 = 60.0;
const COMMENT_COLOR: Color32 = Color32::from_rgb(0x00, 0x80, 0x80);

enum Lane {
    Analog(usize),
    Computed(usize),
    Digital(usize),
}

enum Act {
    SetMain(usize),
    SetAux(usize),
    SetView(f64, f64),
    SetTimeUnit(crate::app::TimeUnit),
    ToggleGrid,
    Channel(usize, ChanAct),
    AddComment(usize),
    EditComment(usize),
    PinCursor(usize),
    ToggleCommentHidden(usize),
    OpenProps(usize),
    Cmd(Command),
}

enum ChanAct {
    Invert,
    RemoveDc,
    Hide,
    Color(Color32),
    OnFreq(bool),
    OnVector(bool),
    OnCircle(bool),
    Overlay(bool),
    Adc(bool),
    VZoomMul(f64),
    VZoomReset,
}

impl App {
    pub fn lanes_ui(&mut self, ui: &mut egui::Ui) {
        let Some(ai) = self.active else {
            ui.centered_and_justified(|ui| {
                ui.label("Осциллограмма не загружена. Откройте COMTRADE (Ctrl+O) или «Демо».");
            });
            return;
        };
        let value_mode = self.value_mode;
        let time_unit = self.time_unit;
        let show_grid = self.show_grid;
        let primary = self.primary_values;
        let avail = ui.available_size();

        let mut actions: Vec<Act> = Vec::new();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let doc = &self.docs[ai];
            // Build lanes: visible analog (overlaid ones merge into the preceding
            // lane — Совместить каналы), then computed, then digital. `overlays[li]`
            // holds the extra analog channels drawn in lane `li`.
            let mut lanes: Vec<Lane> = Vec::new();
            let mut overlays: Vec<Vec<usize>> = Vec::new();
            for (i, c) in doc.chans.iter().enumerate() {
                if !c.visible {
                    continue;
                }
                if c.overlay && matches!(lanes.last(), Some(Lane::Analog(_))) {
                    overlays.last_mut().unwrap().push(i);
                } else {
                    lanes.push(Lane::Analog(i));
                    overlays.push(Vec::new());
                }
            }
            for i in 0..doc.computed.len() {
                lanes.push(Lane::Computed(i));
                overlays.push(Vec::new());
            }
            for i in 0..doc.osc.digital.len() {
                lanes.push(Lane::Digital(i));
                overlays.push(Vec::new());
            }
            let n_lanes = lanes.len().max(1);

            // Lane height: fill the viewport, but never below MIN_LANE (then scroll).
            let fit = (avail.y - AXIS_H) / n_lanes as f32;
            let lane_h = fit.max(MIN_LANE);
            let total_h = if fit >= MIN_LANE { avail.y } else { n_lanes as f32 * lane_h + AXIS_H };

            let (resp, painter) =
                ui.allocate_painter(Vec2::new(avail.x, total_h), Sense::click_and_drag());
            let rect = resp.rect;
            let plot_x0 = rect.left() + LEFT_W;
            let plot_x1 = rect.right() - 6.0;
            let plot_w = (plot_x1 - plot_x0).max(1.0);
            let plot_y1 = rect.bottom() - AXIS_H;

            let (t0, t1) = (doc.view_t0, doc.view_t1.max(doc.view_t0 + 1e-9));
            let x_of = |t: f64| plot_x0 + ((t - t0) / (t1 - t0)) as f32 * plot_w;
            let t_of = |x: f32| t0 + ((x - plot_x0) / plot_w) as f64 * (t1 - t0);

            // Plot background.
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(plot_x0, rect.top()), Pos2::new(plot_x1, plot_y1)),
                0.0f32,
                ui.visuals().extreme_bg_color,
            );

            // Time grid + axis ticks.
            let ticks = nice_ticks(t0, t1, 7);
            for &tt in &ticks {
                let x = x_of(tt);
                if x < plot_x0 - 1.0 || x > plot_x1 + 1.0 {
                    continue;
                }
                if show_grid {
                    painter.line_segment(
                        [Pos2::new(x, rect.top()), Pos2::new(x, plot_y1)],
                        Stroke::new(1.0f32, Color32::from_gray(225)),
                    );
                }
                painter.line_segment(
                    [Pos2::new(x, plot_y1), Pos2::new(x, plot_y1 + 4.0)],
                    Stroke::new(1.0f32, Color32::from_gray(120)),
                );
                let label = if time_unit == crate::app::TimeUnit::Samples {
                    format!("{}", nearest_sample(&doc.osc.time, tt))
                } else {
                    format!("{:.0}", time_unit.scale_secs(tt))
                };
                painter.text(
                    Pos2::new(x, plot_y1 + 6.0),
                    Align2::CENTER_TOP,
                    label,
                    FontId::proportional(10.0),
                    Color32::from_gray(90),
                );
            }
            painter.text(
                Pos2::new(plot_x1, plot_y1 + 6.0),
                Align2::RIGHT_TOP,
                time_unit.label(),
                FontId::proportional(10.0),
                Color32::from_gray(90),
            );

            // Lanes.
            let k_main = doc.main_cursor;
            for (li, lane) in lanes.iter().enumerate() {
                let lane_top = rect.top() + li as f32 * lane_h;
                let lane_bg = if li % 2 == 0 {
                    Color32::from_rgb(0xEA, 0xF4, 0xFB)
                } else {
                    Color32::WHITE
                };
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(rect.left(), lane_top), Pos2::new(plot_x0, lane_top + lane_h)),
                    0.0f32,
                    lane_bg,
                );
                painter.line_segment(
                    [Pos2::new(rect.left(), lane_top + lane_h), Pos2::new(plot_x1, lane_top + lane_h)],
                    Stroke::new(1.0f32, Color32::from_gray(210)),
                );
                let baseline = lane_top + lane_h * 0.5;
                painter.line_segment(
                    [Pos2::new(plot_x0, baseline), Pos2::new(plot_x1, baseline)],
                    Stroke::new(1.0f32, Color32::from_gray(235)),
                );

                let (name, unit, color, maxv, minv, rms, a1, val) = lane_stats(doc, lane, k_main, value_mode, primary);

                // Stats panel.
                let badge = Rect::from_min_size(Pos2::new(rect.left() + 3.0, lane_top + 3.0), Vec2::new(10.0, 10.0));
                painter.rect_filled(badge, 1.0f32, color);
                painter.text(
                    Pos2::new(rect.left() + 18.0, lane_top + 3.0),
                    Align2::LEFT_TOP,
                    &name,
                    FontId::proportional(11.5),
                    Color32::from_gray(30),
                );
                painter.text(
                    Pos2::new(plot_x0 - 6.0, lane_top + 3.0),
                    Align2::RIGHT_TOP,
                    format!("{}  {}", fmt(val), unit),
                    FontId::monospace(11.0),
                    color.gamma_multiply(0.9),
                );
                let base_vzoom = if let Lane::Analog(i) = lane { doc.chans[*i].vzoom } else { 1.0 };
                if overlays[li].is_empty() {
                    if lane_h > 44.0 {
                        let mut yy = lane_top + 18.0;
                        for (lbl, v) in [("Max", maxv), ("Min", minv), ("СКЗ", rms), ("A1", a1)] {
                            if (lbl == "СКЗ" || lbl == "A1") && matches!(lane, Lane::Computed(_) | Lane::Digital(_)) {
                                continue;
                            }
                            painter.text(
                                Pos2::new(rect.left() + 6.0, yy),
                                Align2::LEFT_TOP,
                                format!("{lbl}:"),
                                FontId::proportional(9.5),
                                Color32::from_gray(110),
                            );
                            painter.text(
                                Pos2::new(plot_x0 - 6.0, yy),
                                Align2::RIGHT_TOP,
                                fmt(v),
                                FontId::monospace(9.5),
                                Color32::from_gray(80),
                            );
                            yy += 10.5;
                            if yy > lane_top + lane_h - 6.0 {
                                break;
                            }
                        }
                    }
                } else {
                    // Overlaid lane: compact badge+id+value per additional member.
                    let mut yy = lane_top + 17.0;
                    for &oi in &overlays[li] {
                        let (on, ou, oc, .., ov) = lane_stats(doc, &Lane::Analog(oi), k_main, value_mode, primary);
                        painter.rect_filled(Rect::from_min_size(Pos2::new(rect.left() + 3.0, yy + 1.0), Vec2::new(8.0, 8.0)), 1.0f32, oc);
                        painter.text(Pos2::new(rect.left() + 15.0, yy), Align2::LEFT_TOP, &on, FontId::proportional(10.0), Color32::from_gray(40));
                        painter.text(Pos2::new(plot_x0 - 6.0, yy), Align2::RIGHT_TOP, format!("{} {}", fmt(ov), ou), FontId::monospace(9.5), oc.gamma_multiply(0.9));
                        yy += 11.0;
                        if yy > lane_top + lane_h - 4.0 {
                            break;
                        }
                    }
                }

                // Waveform(s): shared full-scale across base + overlays, scaled by
                // the base channel's vertical zoom.
                let amp = lane_h * 0.42;
                let mut lane_max = maxv.abs().max(minv.abs());
                for &oi in &overlays[li] {
                    lane_max = lane_max.max(analog_maxabs(doc, oi, primary));
                }
                let max_abs = (lane_max / base_vzoom).max(1e-9);
                let y_of = |v: f64| baseline - (v / max_abs) as f32 * amp;
                let draw_wave = |pts: &dyn Fn(usize) -> f64, n: usize, times: &[f64]| -> Vec<Pos2> {
                    let i0 = times.partition_point(|&t| t < t0).saturating_sub(1);
                    let i1 = (times.partition_point(|&t| t <= t1) + 1).min(n);
                    let span = i1.saturating_sub(i0).max(1);
                    let step = (span as f32 / (plot_w * 2.0)).ceil().max(1.0) as usize;
                    let mut v = Vec::with_capacity(span / step + 2);
                    let mut k = i0;
                    while k < i1 {
                        let t = times.get(k).copied().unwrap_or(0.0);
                        v.push(Pos2::new(x_of(t), y_of(pts(k)).clamp(lane_top + 1.0, lane_top + lane_h - 1.0)));
                        k += step;
                    }
                    v
                };
                match lane {
                    Lane::Analog(i) => {
                        let pts = draw_wave(&|k| analog_disp(doc, *i, k, primary), doc.osc.sample_count(), &doc.osc.time);
                        painter.add(Shape::line(pts, Stroke::new(1.3f32, color)));
                        for &oi in &overlays[li] {
                            let oc = doc.chans[oi].color;
                            let op = draw_wave(&|k| analog_disp(doc, oi, k, primary), doc.osc.sample_count(), &doc.osc.time);
                            painter.add(Shape::line(op, Stroke::new(1.2f32, oc)));
                        }
                    }
                    Lane::Computed(i) => {
                        let s = &doc.computed[*i].samples;
                        let pts = draw_wave(&|k| s.get(k).copied().unwrap_or(0.0), s.len(), &doc.osc.time);
                        painter.add(Shape::line(pts, Stroke::new(1.3f32, color)));
                    }
                    Lane::Digital(i) => {
                        let s = &doc.osc.digital[*i].samples;
                        let y0 = baseline + amp * 0.6;
                        let y1b = baseline - amp * 0.6;
                        let pts = draw_wave(
                            &|k| if s.get(k).copied().unwrap_or(false) { 1.0 } else { 0.0 },
                            s.len(),
                            &doc.osc.time,
                        );
                        let step_pts: Vec<Pos2> = pts
                            .iter()
                            .map(|p| Pos2::new(p.x, if p.y < baseline { y1b } else { y0 }))
                            .collect();
                        painter.add(Shape::line(step_pts, Stroke::new(1.4f32, color)));
                    }
                }
            }

            // Selection shading between the two cursors.
            let tm = doc.osc.time.get(doc.main_cursor).copied().unwrap_or(0.0);
            let ta = doc.osc.time.get(doc.aux_cursor).copied().unwrap_or(0.0);
            let (xs, xe) = (x_of(tm.min(ta)), x_of(tm.max(ta)));
            if (xe - xs).abs() > 1.0 {
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(xs, rect.top()), Pos2::new(xe, plot_y1)),
                    0.0f32,
                    Color32::from_rgba_unmultiplied(0xE0, 0x30, 0x30, 24),
                );
            }

            // Comments (anchored markers + labels).
            for (ci, c) in doc.comments.iter().enumerate() {
                let t = doc.osc.time.get(c.sample).copied().unwrap_or(0.0);
                let x = x_of(t);
                if x < plot_x0 - 1.0 || x > plot_x1 + 1.0 {
                    continue;
                }
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, plot_y1)],
                    Stroke::new(1.2f32, COMMENT_COLOR),
                );
                // flag
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, rect.top() + 1.0), Vec2::new(12.0, 12.0)),
                    0.0f32,
                    COMMENT_COLOR,
                );
                painter.text(
                    Pos2::new(x + 6.0, rect.top() + 7.0),
                    Align2::CENTER_CENTER,
                    format!("{}", ci + 1),
                    FontId::proportional(9.0),
                    Color32::WHITE,
                );
                if !c.hidden {
                    let mut label: String = c.text.lines().next().unwrap_or("").chars().take(24).collect();
                    if let Some(lv) = c.level {
                        label = if label.is_empty() {
                            format!("Уровень: {lv:.3}")
                        } else {
                            format!("{label}  (Уровень: {lv:.3})")
                        };
                    }
                    painter.text(
                        Pos2::new(x + 15.0, rect.top() + 2.0),
                        Align2::LEFT_TOP,
                        label,
                        FontId::proportional(10.0),
                        COMMENT_COLOR,
                    );
                }
                // Amplitude-level line (drawn in the first lane's scale).
                if let (Some(lv), Some(first)) = (c.level, lanes.first()) {
                    if let Lane::Analog(fi) = first {
                        let base = rect.top();
                        let lane_h2 = lane_h;
                        let maxabs = analog_maxabs(doc, *fi, primary).max(1e-9);
                        let yy = base + lane_h2 * 0.5 - (lv / maxabs) as f32 * (lane_h2 * 0.42);
                        if yy > base && yy < base + lane_h2 {
                            painter.line_segment(
                                [Pos2::new(x - 10.0, yy), Pos2::new(x + 10.0, yy)],
                                Stroke::new(1.2f32, COMMENT_COLOR),
                            );
                        }
                    }
                }
            }

            // Cursors.
            painter.line_segment(
                [Pos2::new(x_of(ta), rect.top()), Pos2::new(x_of(ta), plot_y1)],
                Stroke::new(1.5f32, Color32::from_rgb(0x20, 0x60, 0xD0)),
            );
            painter.line_segment(
                [Pos2::new(x_of(tm), rect.top()), Pos2::new(x_of(tm), plot_y1)],
                Stroke::new(1.8f32, Color32::from_rgb(0xD0, 0x20, 0x20)),
            );

            // ---- interaction ----
            let shift = ui.input(|i| i.modifiers.shift);
            // Double-click a stats panel -> channel properties.
            if resp.double_clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if pos.x < plot_x0 {
                        let li = ((pos.y - rect.top()) / lane_h).floor() as usize;
                        if let Some(Lane::Analog(i)) = lanes.get(li) {
                            actions.push(Act::OpenProps(*i));
                        }
                    }
                }
            } else if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if pos.x >= plot_x0 {
                        // Click near a comment marker edits it; otherwise set a cursor.
                        let hit = doc.comments.iter().position(|c| {
                            let x = x_of(doc.osc.time.get(c.sample).copied().unwrap_or(0.0));
                            (x - pos.x).abs() < 5.0
                        });
                        if let Some(idx) = hit {
                            actions.push(Act::EditComment(idx));
                        } else {
                            let k = nearest_sample(&doc.osc.time, t_of(pos.x));
                            actions.push(if shift { Act::SetAux(k) } else { Act::SetMain(k) });
                        }
                    }
                }
            }
            if resp.dragged() {
                let dx = resp.drag_delta().x;
                if dx != 0.0 {
                    let dt = -(dx as f64) / plot_w as f64 * (t1 - t0);
                    actions.push(Act::SetView(t0 + dt, t1 + dt));
                }
            }
            // Ctrl+wheel zooms time (plain wheel scrolls the view vertically).
            if let Some(pos) = resp.hover_pos() {
                if pos.x >= plot_x0 {
                    let (zoom, scroll) = ui.input(|i| (i.modifiers.command || i.modifiers.ctrl, i.raw_scroll_delta.y + i.smooth_scroll_delta.y));
                    if zoom && scroll.abs() > 0.1 {
                        let factor = (1.0 - scroll as f64 * 0.0015).clamp(0.2, 5.0);
                        let tc = t_of(pos.x);
                        actions.push(Act::SetView(tc - (tc - t0) * factor, tc + (t1 - tc) * factor));
                    }
                }
            }

            // Context menu.
            let menu_pos = resp.hover_pos();
            resp.context_menu(|ui| {
                let pos = menu_pos.unwrap_or(rect.center());
                if pos.y > plot_y1 {
                    ui.label("Ось времени");
                    for (lbl, unit) in [
                        ("Микросекунды", crate::app::TimeUnit::Micro),
                        ("Миллисекунды", crate::app::TimeUnit::Milli),
                        ("Секунды", crate::app::TimeUnit::Sec),
                        ("Отсчёты", crate::app::TimeUnit::Samples),
                    ] {
                        if ui.radio(time_unit == unit, lbl).clicked() {
                            actions.push(Act::SetTimeUnit(unit));
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.checkbox(&mut { show_grid }, "Сетка").clicked() {
                        actions.push(Act::ToggleGrid);
                        ui.close_menu();
                    }
                    return;
                }
                if pos.x >= plot_x0 {
                    // Right-click on a comment marker -> comment actions (MENU 563).
                    let near = doc.comments.iter().position(|c| {
                        let cx = x_of(doc.osc.time.get(c.sample).copied().unwrap_or(0.0));
                        (cx - pos.x).abs() < 6.0
                    });
                    if let Some(idx) = near {
                        ui.label("Комментарий");
                        if ui.button("Изменить комментарий / уровень…").clicked() {
                            actions.push(Act::EditComment(idx));
                            ui.close_menu();
                        }
                        if ui.button("Зафиксировать визир").clicked() {
                            actions.push(Act::PinCursor(doc.comments[idx].sample));
                            ui.close_menu();
                        }
                        let hidden = doc.comments[idx].hidden;
                        if ui.button(if hidden { "Показать комментарий" } else { "Скрыть комментарий" }).clicked() {
                            actions.push(Act::ToggleCommentHidden(idx));
                            ui.close_menu();
                        }
                        if ui.button("Удалить комментарий").clicked() {
                            actions.push(Act::EditComment(idx)); // opens editor; delete there
                            ui.close_menu();
                        }
                        return;
                    }
                    let samp = nearest_sample(&doc.osc.time, t_of(pos.x));
                    if ui.button("Добавить комментарий здесь").clicked() {
                        actions.push(Act::AddComment(samp));
                        ui.close_menu();
                    }
                    ui.separator();
                }
                let li = ((pos.y - rect.top()) / lane_h).floor() as usize;
                if let Some(Lane::Analog(i)) = lanes.get(li) {
                    let i = *i;
                    ui.label(&doc.osc.analog[i].id);
                    if ui.button("Свойства…").clicked() {
                        actions.push(Act::OpenProps(i));
                        ui.close_menu();
                    }
                    if ui.button("Скрыть").clicked() {
                        actions.push(Act::Channel(i, ChanAct::Hide));
                        ui.close_menu();
                    }
                    ui.menu_button("Цвет канала", |ui| {
                        for (lbl, c) in [
                            ("Жёлтый", Color32::from_rgb(0xD4, 0xA0, 0x17)),
                            ("Зелёный", Color32::from_rgb(0x18, 0x8A, 0x42)),
                            ("Красный", Color32::from_rgb(0xCC, 0x22, 0x22)),
                            ("Синий", Color32::from_rgb(0x20, 0x4A, 0xA0)),
                            ("Чёрный", Color32::from_rgb(0x20, 0x20, 0x20)),
                        ] {
                            if ui.button(lbl).clicked() {
                                actions.push(Act::Channel(i, ChanAct::Color(c)));
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    let ch = &doc.chans[i];
                    if ui.checkbox(&mut { ch.invert }, "Инвертировать").clicked() {
                        actions.push(Act::Channel(i, ChanAct::Invert));
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut { ch.remove_dc }, "Удалить постоянную").clicked() {
                        actions.push(Act::Channel(i, ChanAct::RemoveDc));
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut { ch.adc_counts }, "Отображать в отсчётах АЦП").clicked() {
                        actions.push(Act::Channel(i, ChanAct::Adc(!ch.adc_counts)));
                        ui.close_menu();
                    }
                    if ch.overlay {
                        if ui.button("Разъединить каналы").clicked() {
                            actions.push(Act::Channel(i, ChanAct::Overlay(false)));
                            ui.close_menu();
                        }
                    } else if li > 0 && ui.button("Совместить каналы (с предыдущим)").clicked() {
                        actions.push(Act::Channel(i, ChanAct::Overlay(true)));
                        ui.close_menu();
                    }
                    ui.menu_button("Масштаб по вертикали", |ui| {
                        if ui.button("Увеличить").clicked() {
                            actions.push(Act::Channel(i, ChanAct::VZoomMul(1.25)));
                            ui.close_menu();
                        }
                        if ui.button("Уменьшить").clicked() {
                            actions.push(Act::Channel(i, ChanAct::VZoomMul(0.8)));
                            ui.close_menu();
                        }
                        if ui.button("Вернуть к исходному размеру").clicked() {
                            actions.push(Act::Channel(i, ChanAct::VZoomReset));
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    if ui.radio(value_mode == ValueMode::Instant, "Мгновенные значения").clicked() {
                        actions.push(Act::Cmd(Command::SetValueMode(ValueMode::Instant)));
                        ui.close_menu();
                    }
                    if ui.radio(value_mode == ValueMode::Rms, "Среднеквадратичные значения").clicked() {
                        actions.push(Act::Cmd(Command::SetValueMode(ValueMode::Rms)));
                        ui.close_menu();
                    }
                    if ui.radio(value_mode == ValueMode::FirstHarmonic, "Первая гармоника").clicked() {
                        actions.push(Act::Cmd(Command::SetValueMode(ValueMode::FirstHarmonic)));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.checkbox(&mut { ch.on_circle }, "Отображать на круговой диаграмме").clicked() {
                        actions.push(Act::Channel(i, ChanAct::OnCircle(!ch.on_circle)));
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut { ch.on_vector }, "Отображать на векторной диаграмме").clicked() {
                        actions.push(Act::Channel(i, ChanAct::OnVector(!ch.on_vector)));
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut { ch.on_freq }, "Отображать на частотной диаграмме").clicked() {
                        actions.push(Act::Channel(i, ChanAct::OnFreq(!ch.on_freq)));
                        ui.close_menu();
                    }
                }
            });
        });

        // Apply deferred actions.
        for a in actions {
            match a {
                Act::SetTimeUnit(u) => self.time_unit = u,
                Act::ToggleGrid => self.show_grid = !self.show_grid,
                Act::SetMain(k) => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        d.main_cursor = k;
                    }
                }
                Act::SetAux(k) => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        d.aux_cursor = k;
                    }
                }
                Act::SetView(a, b) => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        if b - a > 1e-9 {
                            d.view_t0 = a;
                            d.view_t1 = b;
                        }
                    }
                }
                Act::AddComment(sample) => {
                    self.comment_edit = Some(CommentEdit { sample, idx: None, text: String::new(), level_str: String::new() });
                }
                Act::EditComment(idx) => {
                    if let Some(d) = self.docs.get(ai) {
                        if let Some(c) = d.comments.get(idx) {
                            self.comment_edit = Some(CommentEdit {
                                sample: c.sample,
                                idx: Some(idx),
                                text: c.text.clone(),
                                level_str: c.level.map(|l| format!("{l}")).unwrap_or_default(),
                            });
                        }
                    }
                }
                Act::PinCursor(sample) => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        d.main_cursor = sample;
                    }
                }
                Act::ToggleCommentHidden(idx) => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        if let Some(c) = d.comments.get_mut(idx) {
                            c.hidden = !c.hidden;
                        }
                    }
                }
                Act::OpenProps(i) => self.props_channel = Some(i),
                Act::Channel(i, act) => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        if let Some(c) = d.chans.get_mut(i) {
                            match act {
                                ChanAct::Invert => c.invert = !c.invert,
                                ChanAct::RemoveDc => c.remove_dc = !c.remove_dc,
                                ChanAct::Hide => c.visible = false,
                                ChanAct::Color(col) => c.color = col,
                                ChanAct::OnFreq(v) => c.on_freq = v,
                                ChanAct::OnVector(v) => c.on_vector = v,
                                ChanAct::OnCircle(v) => c.on_circle = v,
                                ChanAct::Overlay(v) => c.overlay = v,
                                ChanAct::Adc(v) => c.adc_counts = v,
                                ChanAct::VZoomMul(m) => c.vzoom = (c.vzoom * m).clamp(0.1, 50.0),
                                ChanAct::VZoomReset => c.vzoom = 1.0,
                            }
                        }
                    }
                }
                Act::Cmd(c) => self.dispatch(c),
            }
        }
    }

    /// The six status-bar cursor readouts (визир 1/2 and their differences).
    pub fn cursor_status(&self) -> [String; 6] {
        let empty = || "—".to_string();
        let Some(d) = self.active_doc() else {
            return [empty(), empty(), empty(), empty(), empty(), empty()];
        };
        let u = self.time_unit;
        let t1 = d.osc.time.get(d.main_cursor).copied().unwrap_or(0.0);
        let t2 = d.osc.time.get(d.aux_cursor).copied().unwrap_or(0.0);
        let f = |t: f64| {
            if u == crate::app::TimeUnit::Samples {
                format!("{}", nearest_sample(&d.osc.time, t))
            } else {
                format!("{:.1} {}", u.scale_secs(t), u.label())
            }
        };
        let first = d.chans.iter().position(|c| c.visible);
        let hv = |k: usize| match first {
            Some(i) => format!("{} {}", fmt(d.sample(i, k)), d.osc.analog[i].unit),
            None => empty(),
        };
        [
            f(t1),
            f(t2),
            if u == crate::app::TimeUnit::Samples {
                empty()
            } else {
                format!("{:.1} {}", u.scale_secs((t1 - t2).abs()), u.label())
            },
            hv(d.main_cursor),
            hv(d.aux_cursor),
            empty(),
        ]
    }
}

fn primary_scale(doc: &crate::app::Doc, i: usize, primary: bool) -> f64 {
    let ch = &doc.osc.analog[i];
    if primary && !ch.is_primary && ch.secondary != 0.0 {
        ch.primary / ch.secondary
    } else {
        1.0
    }
}

/// Displayed sample of analog channel `i` at `k`: raw ADC counts when the channel
/// is in «отсчёты АЦП» mode, otherwise the (optionally primary-scaled) value with
/// invert / remove-DC applied.
fn analog_disp(doc: &crate::app::Doc, i: usize, k: usize, primary: bool) -> f64 {
    let ch = &doc.osc.analog[i];
    if doc.chans[i].adc_counts {
        let raw = ch.samples.get(k).copied().unwrap_or(0.0);
        if ch.a.abs() > 1e-12 {
            (raw - ch.b) / ch.a
        } else {
            raw
        }
    } else {
        doc.sample(i, k) * primary_scale(doc, i, primary)
    }
}

fn analog_maxabs(doc: &crate::app::Doc, i: usize, primary: bool) -> f64 {
    (0..doc.osc.sample_count())
        .map(|k| analog_disp(doc, i, k, primary).abs())
        .fold(1e-9, f64::max)
}

fn analog_unit(doc: &crate::app::Doc, i: usize) -> String {
    if doc.chans[i].adc_counts {
        "отсч.".to_string()
    } else {
        doc.osc.analog[i].unit.clone()
    }
}

/// Compute (name, unit, color, max, min, rms, a1, value@cursor) for a lane.
fn lane_stats(
    doc: &crate::app::Doc,
    lane: &Lane,
    k_main: usize,
    value_mode: ValueMode,
    primary: bool,
) -> (String, String, Color32, f64, f64, f64, f64, f64) {
    match lane {
        Lane::Analog(i) => {
            let ch = &doc.osc.analog[*i];
            let adc = doc.chans[*i].adc_counts;
            // RMS/1st-harmonic are computed in scaled units; divide by |a| for ADC.
            let sdiv = if adc && ch.a.abs() > 1e-12 { ch.a.abs() } else { 1.0 };
            let scale = primary_scale(doc, *i, primary);
            let n = doc.osc.sample_count();
            let mut mx = f64::MIN;
            let mut mn = f64::MAX;
            for k in 0..n {
                let v = analog_disp(doc, *i, k, primary);
                mx = mx.max(v);
                mn = mn.min(v);
            }
            let rms = doc.rms_at(*i, k_main) * scale / sdiv;
            let a1 = doc.harmonic_at(*i, k_main) * scale / sdiv;
            let val = match value_mode {
                ValueMode::Instant => analog_disp(doc, *i, k_main, primary),
                ValueMode::Rms => rms,
                ValueMode::FirstHarmonic => a1,
            };
            (ch.id.clone(), analog_unit(doc, *i), doc.chans[*i].color, mx, mn, rms, a1, val)
        }
        Lane::Computed(i) => {
            let c = &doc.computed[*i];
            let mx = c.samples.iter().copied().fold(f64::MIN, f64::max);
            let mn = c.samples.iter().copied().fold(f64::MAX, f64::min);
            let val = c.samples.get(k_main).copied().unwrap_or(0.0);
            (c.name.clone(), String::new(), c.color, mx, mn, 0.0, 0.0, val)
        }
        Lane::Digital(i) => {
            let d = &doc.osc.digital[*i];
            let val = if d.samples.get(k_main).copied().unwrap_or(false) { 1.0 } else { 0.0 };
            (d.id.clone(), String::new(), Color32::from_rgb(0x44, 0x44, 0xAA), 1.0, 0.0, 0.0, 0.0, val)
        }
    }
}

fn fmt(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else if v.abs() >= 1000.0 || v.abs() < 0.01 {
        format!("{v:.3e}")
    } else {
        format!("{v:.3}")
    }
}

fn nearest_sample(times: &[f64], t: f64) -> usize {
    if times.is_empty() {
        return 0;
    }
    let p = times.partition_point(|&x| x < t);
    if p == 0 {
        0
    } else if p >= times.len() {
        times.len() - 1
    } else if (times[p] - t).abs() < (t - times[p - 1]).abs() {
        p
    } else {
        p - 1
    }
}

/// Produce ~`want` round tick positions spanning `[a, b]`.
fn nice_ticks(a: f64, b: f64, want: usize) -> Vec<f64> {
    let span = (b - a).abs();
    if span <= 0.0 {
        return vec![a];
    }
    let raw = span / want as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = if norm < 1.5 {
        1.0
    } else if norm < 3.0 {
        2.0
    } else if norm < 7.0 {
        5.0
    } else {
        10.0
    } * mag;
    let start = (a / step).ceil() * step;
    let mut out = Vec::new();
    let mut t = start;
    while t <= b + step * 0.01 && out.len() < want * 3 {
        out.push(t);
        t += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{nearest_sample, nice_ticks};

    #[test]
    fn nearest_sample_rounds_to_closest() {
        let t = [0.0, 0.1, 0.2, 0.3];
        assert_eq!(nearest_sample(&t, 0.0), 0);
        assert_eq!(nearest_sample(&t, 0.14), 1);
        assert_eq!(nearest_sample(&t, 0.16), 2);
        assert_eq!(nearest_sample(&t, 9.0), 3);
        assert_eq!(nearest_sample(&t, -1.0), 0);
    }

    #[test]
    fn nice_ticks_are_round_and_in_range() {
        let ticks = nice_ticks(0.0, 1.0, 5);
        assert!(ticks.len() >= 3);
        assert!(ticks.iter().all(|&t| t >= -1e-9 && t <= 1.0 + 0.3));
    }
}
