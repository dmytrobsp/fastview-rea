//! The lane-based oscillogram view — the original's primary document window.
//!
//! Each analog/computed/digital channel is a horizontal lane with a left stats
//! panel (name badge · value@cursor · Max/Min/СКЗ/A1) and a waveform. Two
//! vertical визиры (red main, blue aux) with a shaded selection between them,
//! a shared time axis, on-waveform comments, and the original's context menus
//! chosen by click region (MENU 560 axis band, 561 stats panel acting on the
//! selected channels, 563 on a comment — ledger L9.1). Lanes have a minimum
//! height and the view scrolls vertically when there are many channels.

use crate::app::{App, ChanKey, CommentEdit, TimeUnit, ValueMode};
use crate::chansel::{select_click, ChanFlag, ItemState, SelClick};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

const LEFT_W: f32 = 158.0;
const AXIS_H: f32 = 26.0;
const MIN_LANE: f32 = 60.0;
const COMMENT_COLOR: Color32 = Color32::from_rgb(0x00, 0x80, 0x80);
/// Stats-panel background of a selected channel row.
const SELECTED_BG: Color32 = Color32::from_rgb(0xFE, 0xEC, 0x9C);

type Lane = ChanKey;

/// Which context menu the right-click opened (`WM_RBUTTONUP` `0x45efa0`
/// chooses it by the click's region).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneMenuKind {
    /// Bottom time-axis band: MENU 560.
    TimeAxis,
    /// Stats panel: MENU 561, acting on the selected channels.
    Channels,
    /// A comment under the pointer in the plot: MENU 563.
    Comment(usize),
}

/// An open context menu. Its subject is fixed at the right-click; `serial`
/// gives each opening its own popup area so it is sized to its own items.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneMenu {
    pub serial: u64,
    pub pos: Pos2,
    pub kind: LaneMenuKind,
}

/// Where a pointer position falls in the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// The bottom time-axis band.
    Axis,
    /// The stats panel, on a channel row (None below the last row).
    Stats(Option<ChanKey>),
    /// The waveform area.
    Plot,
}

/// Classify `pos` (view geometry: `top` of the first row, `plot_x0` the
/// waveform's left edge, `plot_y1` the top of the axis band, uniform `lane_h`).
pub fn hit_test(pos: Pos2, top: f32, plot_x0: f32, plot_y1: f32, lane_h: f32, lanes: &[ChanKey]) -> Hit {
    if pos.y > plot_y1 {
        Hit::Axis
    } else if pos.x >= plot_x0 {
        Hit::Plot
    } else {
        let li = ((pos.y - top) / lane_h).floor();
        Hit::Stats(if li >= 0.0 { lanes.get(li as usize).copied() } else { None })
    }
}

enum Act {
    SetMain(usize),
    SetAux(usize),
    SetView(f64, f64),
    PlaceMoment(usize),
    EditComment(usize),
    OpenProps(usize),
    StartDragHcur(u8),
    SetHcur(u8, f64),
    EndDragHcur,
    Select(ChanKey, SelClick),
    ClearSelection,
    OpenMenu(Pos2, LaneMenuKind),
}

/// A command chosen in one of the context menus.
enum MenuCmd {
    Flag(ChanFlag),
    Hide,
    Mode(ValueMode),
    ResetZoom,
    CopyMerge,
    PasteMerge,
    Combine,
    Split,
    Color,
    TimeUnit(TimeUnit),
    Grid,
    EditComment(usize),
    LockComment(usize),
    DeleteComment(usize),
    HideComment(usize),
    EditLevel(usize),
}

impl App {
    pub fn lanes_ui(&mut self, ui: &mut egui::Ui) {
        let Some(ai) = self.active else {
            ui.centered_and_justified(|ui| {
                ui.label("Осциллограмма не загружена. Откройте COMTRADE (Ctrl+O) или «Демо».");
            });
            return;
        };
        let time_unit = self.time_unit;
        let show_grid = self.show_grid;
        let primary = self.primary_values;
        let hcur_dragging = self.dragging_hcur;
        let avail = ui.available_size();

        let mut actions: Vec<Act> = Vec::new();
        let mut rows: Vec<ChanKey> = Vec::new();
        let menu_busy = self.lane_menu.is_some() || self.lane_menu_eat;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let doc = &self.docs[ai];
            // Build lanes: visible analog (overlaid ones merge into the preceding
            // lane — Совместить каналы), then computed, then digital. `overlays[li]`
            // holds the extra analog channels drawn in lane `li`.
            let mut lanes: Vec<Lane> = Vec::new();
            let mut overlays: Vec<Vec<usize>> = Vec::new();
            let mut group_lane: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
            for (i, c) in doc.chans.iter().enumerate() {
                if !c.visible {
                    continue;
                }
                // A grouped channel joins the lane of its group's first visible member.
                if let Some(g) = c.group {
                    if let Some(&li) = group_lane.get(&g) {
                        overlays[li].push(i);
                        continue;
                    }
                    group_lane.insert(g, lanes.len());
                }
                lanes.push(Lane::Analog(i));
                overlays.push(Vec::new());
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
                let lane_bg = if doc.selection.contains(lane) {
                    SELECTED_BG
                } else if li % 2 == 0 {
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

                let (name, unit, color, maxv, minv, rms, a1, val) = lane_stats(doc, lane, k_main, primary);

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
                    fvb_core::views::readout_text(val, &unit).trim().to_string(),
                    FontId::monospace(11.0),
                    color.gamma_multiply(0.9),
                );
                let base_vzoom = if let Lane::Analog(i) = lane { doc.chans[*i].vzoom } else { 1.0 };
                if overlays[li].is_empty() {
                    if lane_h > 44.0 {
                        let mut yy = lane_top + 18.0;
                        for (lbl, v) in [("Max", maxv), ("Min", minv), ("СКЗ", rms), ("A1", a1)] {
                            if (lbl == "СКЗ" || lbl == "A1") && matches!(lane, Lane::Digital(_)) {
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
                                fvb_core::views::readout_text(v, &unit).trim().to_string(),
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
                        let (on, ou, oc, .., ov) = lane_stats(doc, &Lane::Analog(oi), k_main, primary);
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
                        let pts = draw_wave(&|k| mode_disp(doc, *i, k, primary), doc.osc.sample_count(), &doc.osc.time);
                        painter.add(Shape::line(pts, Stroke::new(1.3f32, color)));
                        for &oi in &overlays[li] {
                            let oc = doc.chans[oi].color;
                            let op = draw_wave(&|k| mode_disp(doc, oi, k, primary), doc.osc.sample_count(), &doc.osc.time);
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
                if let (Some(lv), Some(Lane::Analog(fi))) = (c.level, lanes.first()) {
                    let base = rect.top();
                    let maxabs = analog_maxabs(doc, *fi, primary).max(1e-9);
                    let yy = base + lane_h * 0.5 - (lv / maxabs) as f32 * (lane_h * 0.42);
                    if yy > base && yy < base + lane_h {
                        painter.line_segment(
                            [Pos2::new(x - 10.0, yy), Pos2::new(x + 10.0, yy)],
                            Stroke::new(1.2f32, COMMENT_COLOR),
                        );
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

            // Horizontal (amplitude) визиры, measured in the first lane's scale.
            let hcur_map = if doc.hcur_on {
                if let Some(Lane::Analog(fi)) = lanes.first() {
                    let vz = doc.chans[*fi].vzoom;
                    let maxabs = (analog_maxabs(doc, *fi, primary) / vz).max(1e-9);
                    Some((rect.top() + lane_h * 0.5, lane_h * 0.42, maxabs))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((base_mid, amp, maxabs)) = hcur_map {
                for (val, col) in [
                    (doc.hcur1, Color32::from_rgb(0x18, 0x8A, 0x42)),
                    (doc.hcur2, Color32::from_rgb(0xD0, 0x80, 0x10)),
                ] {
                    let y = (base_mid - (val / maxabs) as f32 * amp).clamp(rect.top(), plot_y1);
                    painter.line_segment([Pos2::new(plot_x0, y), Pos2::new(plot_x1, y)], Stroke::new(1.3f32, col));
                    painter.rect_filled(Rect::from_min_size(Pos2::new(plot_x1 - 8.0, y - 5.0), Vec2::new(8.0, 10.0)), 1.0f32, col);
                }
            }

            // ---- interaction ----
            // Clicks follow `WM_LBUTTONDOWN`/`WM_RBUTTONDOWN` `0x45dc10` and the
            // menus `WM_RBUTTONUP` `0x45efa0`: the region under the pointer at
            // the click decides what happens, and an opened menu keeps that
            // subject however the pointer moves afterwards.
            let (shift, ctrl) = ui.input(|i| (i.modifiers.shift, i.modifiers.command || i.modifiers.ctrl));
            let eat = menu_busy;
            let hit_at = |pos: Pos2| hit_test(pos, rect.top(), plot_x0, plot_y1, lane_h, &lanes);
            let comment_at = |pos: Pos2| {
                doc.comments.iter().position(|c| {
                    let x = x_of(doc.osc.time.get(c.sample).copied().unwrap_or(0.0));
                    (x - pos.x).abs() < 5.0
                })
            };
            let sample_at = |pos: Pos2| nearest_sample(&doc.osc.time, t_of(pos.x));
            if eat {
                // A press that dismissed a menu, or any click while one is open.
            } else if resp.double_clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    match hit_at(pos) {
                        Hit::Stats(Some(Lane::Analog(i))) => actions.push(Act::OpenProps(i)),
                        Hit::Plot => {
                            if let Some(idx) = comment_at(pos) {
                                actions.push(Act::EditComment(idx));
                            }
                        }
                        _ => {}
                    }
                }
            } else if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    match hit_at(pos) {
                        // A click on the time-axis band deselects every channel.
                        Hit::Axis => actions.push(Act::ClearSelection),
                        Hit::Stats(Some(k)) => {
                            let how = if shift {
                                SelClick::Shift
                            } else if ctrl {
                                SelClick::Ctrl
                            } else {
                                SelClick::Plain
                            };
                            actions.push(Act::Select(k, how));
                        }
                        Hit::Stats(None) => {
                            if !ctrl {
                                actions.push(Act::ClearSelection);
                            }
                        }
                        Hit::Plot => {
                            if doc.comment_mode == 1 {
                                actions.push(Act::PlaceMoment(sample_at(pos)));
                            } else if comment_at(pos).is_none() {
                                actions.push(if shift { Act::SetAux(sample_at(pos)) } else { Act::SetMain(sample_at(pos)) });
                            }
                        }
                    }
                }
            } else if resp.secondary_clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    match hit_at(pos) {
                        Hit::Axis => {
                            actions.push(Act::ClearSelection);
                            actions.push(Act::OpenMenu(pos, LaneMenuKind::TimeAxis));
                        }
                        Hit::Stats(row) => {
                            // Select-first: the row is selected unless it already
                            // is (then the multi-selection is kept).
                            if let Some(k) = row {
                                actions.push(Act::Select(k, SelClick::Right));
                            }
                            actions.push(Act::OpenMenu(pos, LaneMenuKind::Channels));
                        }
                        // The plot has a menu only on a comment; elsewhere the
                        // right button places визир 2.
                        Hit::Plot => match comment_at(pos) {
                            Some(idx) => actions.push(Act::OpenMenu(pos, LaneMenuKind::Comment(idx))),
                            None => actions.push(Act::SetAux(sample_at(pos))),
                        },
                    }
                }
            }
            // Horizontal-визир dragging via the right-edge handles (takes priority
            // over panning).
            if let (Some((base_mid, amp, maxabs)), false) = (hcur_map, eat) {
                if resp.drag_started() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        if pos.x > plot_x1 - 14.0 {
                            let y1 = base_mid - (doc.hcur1 / maxabs) as f32 * amp;
                            let y2 = base_mid - (doc.hcur2 / maxabs) as f32 * amp;
                            let h = if (pos.y - y1).abs() <= (pos.y - y2).abs() { 0u8 } else { 1u8 };
                            actions.push(Act::StartDragHcur(h));
                        }
                    }
                }
                if let (Some(h), true) = (hcur_dragging, resp.dragged()) {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let v = ((base_mid - pos.y) / amp) as f64 * maxabs;
                        actions.push(Act::SetHcur(h, v));
                    }
                }
                if resp.drag_stopped() {
                    actions.push(Act::EndDragHcur);
                }
            }
            if !eat && resp.dragged_by(egui::PointerButton::Primary) && hcur_dragging.is_none() {
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
            rows = lanes.clone();
        });

        for a in actions {
            match a {
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
                Act::PlaceMoment(sample) => {
                    // 155: one click places the moment; the mode switches off.
                    if let Some(d) = self.docs.get_mut(ai) {
                        d.comments.push(crate::app::Comment {
                            sample,
                            text: String::new(),
                            level: None,
                            hidden: false,
                            locked: false,
                        });
                        d.comment_mode = 0;
                    }
                }
                Act::EditComment(idx) => self.open_comment_editor(ai, idx),
                Act::StartDragHcur(h) => self.dragging_hcur = Some(h),
                Act::EndDragHcur => self.dragging_hcur = None,
                Act::SetHcur(h, v) => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        if h == 0 {
                            d.hcur1 = v;
                        } else {
                            d.hcur2 = v;
                        }
                    }
                }
                Act::OpenProps(i) => self.props_channel = Some(i),
                Act::Select(k, how) => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        let (mut sel, mut anchor) = (std::mem::take(&mut d.selection), d.sel_anchor);
                        select_click(&mut sel, &mut anchor, &rows, k, how);
                        d.selection = sel;
                        d.sel_anchor = anchor;
                    }
                }
                Act::ClearSelection => {
                    if let Some(d) = self.docs.get_mut(ai) {
                        d.selection.clear();
                    }
                }
                Act::OpenMenu(pos, kind) => {
                    self.lane_menu_serial += 1;
                    self.lane_menu = Some(LaneMenu { serial: self.lane_menu_serial, pos, kind });
                }
            }
        }

        self.lane_menu_ui(ui.ctx(), ai);
        if self.lane_menu_eat && !ui.input(|i| i.pointer.any_down()) {
            self.lane_menu_eat = false;
        }
    }

    fn open_comment_editor(&mut self, ai: usize, idx: usize) {
        if let Some(c) = self.docs.get(ai).and_then(|d| d.comments.get(idx)) {
            self.comment_edit = Some(CommentEdit {
                sample: c.sample,
                idx: Some(idx),
                text: c.text.clone(),
                level_str: c.level.map(|l| format!("{l}")).unwrap_or_default(),
            });
        }
    }

    /// Show the open context menu in its own popup area (sized to its items,
    /// like a Win32 popup menu) and run the chosen command.
    fn lane_menu_ui(&mut self, ctx: &egui::Context, ai: usize) {
        let Some(menu) = self.lane_menu else { return };
        let Some(doc) = self.docs.get(ai) else {
            self.lane_menu = None;
            return;
        };
        if let LaneMenuKind::Comment(i) = menu.kind {
            if i >= doc.comments.len() {
                self.lane_menu = None;
                return;
            }
        }
        let (time_unit, grid, can_paste) = (self.time_unit, self.show_grid, !self.channel_clip.is_empty());
        let mut chosen: Option<MenuCmd> = None;
        let area = egui::Area::new(egui::Id::new("fv_lane_menu").with(menu.serial))
            .kind(egui::UiKind::Menu)
            .order(egui::Order::Foreground)
            .fixed_pos(menu.pos)
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    menu_style(ui.style_mut());
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                        let mut pick = |c: MenuCmd| chosen = Some(c);
                        match menu.kind {
                            LaneMenuKind::Channels => channel_menu(ui, doc, can_paste, &mut pick),
                            LaneMenuKind::TimeAxis => time_axis_menu(ui, time_unit, grid, &mut pick),
                            LaneMenuKind::Comment(i) => comment_menu(ui, &doc.comments[i], i, &mut pick),
                        }
                    });
                });
            });
        let rect = area.response.rect;
        let (esc, pressed_outside) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p)),
            )
        });
        if chosen.is_some() || esc {
            self.lane_menu = None;
        }
        if pressed_outside {
            self.lane_menu = None;
            self.lane_menu_eat = true;
        }
        if let Some(c) = chosen {
            self.run_menu_cmd(ai, c);
        }
    }

    fn run_menu_cmd(&mut self, ai: usize, cmd: MenuCmd) {
        match cmd {
            MenuCmd::CopyMerge => self.copy_channels_for_merge(),
            MenuCmd::PasteMerge => self.paste_channels_for_merge(),
            MenuCmd::TimeUnit(u) => self.time_unit = u,
            MenuCmd::Grid => self.show_grid = !self.show_grid,
            MenuCmd::Color => self.color_dialog = self.docs.get(ai).and_then(|d| d.first_selected_color()),
            MenuCmd::EditComment(i) | MenuCmd::EditLevel(i) => self.open_comment_editor(ai, i),
            cmd => {
                let Some(d) = self.docs.get_mut(ai) else { return };
                match cmd {
                    MenuCmd::Flag(f) => d.toggle_flag_selected(f),
                    MenuCmd::Hide => d.hide_selected(),
                    MenuCmd::Mode(m) => d.set_mode_selected(m),
                    MenuCmd::ResetZoom => d.reset_zoom_selected(),
                    MenuCmd::Combine => d.combine_selected(),
                    MenuCmd::Split => d.split_selected(),
                    MenuCmd::LockComment(i) => d.comments[i].locked = !d.comments[i].locked,
                    MenuCmd::DeleteComment(i) => {
                        d.comments.remove(i);
                    }
                    MenuCmd::HideComment(i) => d.comments[i].hidden = !d.comments[i].hidden,
                    _ => {}
                }
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
        let unit = first.map(|i| d.osc.analog[i].unit.clone()).unwrap_or_default();
        // Horizontal визиры: their amplitude values when enabled, else the active
        // channel's value at the vertical cursors.
        let (h1, h2, hdiff) = if d.hcur_on {
            (
                format!("{} {}", fmt(d.hcur1), unit),
                format!("{} {}", fmt(d.hcur2), unit),
                format!("{} {}", fmt((d.hcur1 - d.hcur2).abs()), unit),
            )
        } else {
            let hv = |k: usize| match first {
                Some(i) => format!("{} {}", fmt(d.sample(i, k)), unit),
                None => empty(),
            };
            (hv(d.main_cursor), hv(d.aux_cursor), empty())
        };
        [
            f(t1),
            f(t2),
            if u == crate::app::TimeUnit::Samples {
                empty()
            } else {
                format!("{:.1} {}", u.scale_secs((t1 - t2).abs()), u.label())
            },
            h1,
            h2,
            hdiff,
        ]
    }
}

/// The look of a Win32 popup menu: flat items, no wrapping (a menu is as
/// wide as its longest item).
fn menu_style(style: &mut egui::Style) {
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
    style.spacing.button_padding = Vec2::new(2.0, 0.0);
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
}

fn menu_item(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(label)).clicked()
}

fn menu_check(ui: &mut egui::Ui, label: &str, st: ItemState) -> bool {
    let mut c = st.checked;
    ui.add_enabled(st.enabled, egui::Checkbox::new(&mut c, label)).clicked()
}

fn menu_radio(ui: &mut egui::Ui, label: &str, st: ItemState) -> bool {
    ui.add_enabled(st.enabled, egui::RadioButton::new(st.checked, label)).clicked()
}

/// MENU 561 (stats panel), in resource order; every item acts on the
/// selected channels. Diagram flags are radio-styled (SetRadio), the rest
/// check-styled (SetCheck), as the original's CMDUI handlers do.
fn channel_menu(ui: &mut egui::Ui, doc: &crate::app::Doc, can_paste: bool, pick: &mut dyn FnMut(MenuCmd)) {
    for (label, f) in [
        ("Отображать на частотной диаграмме", ChanFlag::OnFreq),
        ("Отображать на векторной диаграмме", ChanFlag::OnVector),
        ("Отображать на круговой диаграмме", ChanFlag::OnCircle),
    ] {
        if menu_radio(ui, label, doc.flag_state(f)) {
            pick(MenuCmd::Flag(f));
        }
    }
    ui.separator();
    if menu_check(ui, "Отображать в отсчетах АЦП", doc.flag_state(ChanFlag::Adc)) {
        pick(MenuCmd::Flag(ChanFlag::Adc));
    }
    ui.separator();
    if menu_item(ui, "Скрыть", true) {
        pick(MenuCmd::Hide);
    }
    ui.separator();
    for (label, m) in [
        ("Мгновенные значения", ValueMode::Instant),
        ("Среднеквадратичные значения", ValueMode::Rms),
        ("Первая гармоника", ValueMode::FirstHarmonic),
    ] {
        if menu_check(ui, label, doc.mode_state(m)) {
            pick(MenuCmd::Mode(m));
        }
    }
    ui.separator();
    if menu_check(ui, "Инвертировать", doc.flag_state(ChanFlag::Invert)) {
        pick(MenuCmd::Flag(ChanFlag::Invert));
    }
    if menu_check(ui, "Удалить постоянную составляющую", doc.flag_state(ChanFlag::RemoveDc)) {
        pick(MenuCmd::Flag(ChanFlag::RemoveDc));
    }
    if menu_item(ui, "Вернуть к исходному размеру", true) {
        pick(MenuCmd::ResetZoom);
    }
    ui.separator();
    if menu_item(ui, "Копировать каналы для объединения осциллограмм", !doc.selected_analog().is_empty()) {
        pick(MenuCmd::CopyMerge);
    }
    if menu_item(ui, "Вставить каналы для объединения осциллограмм", can_paste) {
        pick(MenuCmd::PasteMerge);
    }
    ui.separator();
    if menu_item(ui, "Совместить каналы", doc.can_combine()) {
        pick(MenuCmd::Combine);
    }
    if menu_item(ui, "Разъединить каналы", doc.can_split()) {
        pick(MenuCmd::Split);
    }
    ui.separator();
    if menu_item(ui, "Цвет канала", true) {
        pick(MenuCmd::Color);
    }
}

/// MENU 560 (time-axis band). «Абсолютное время» and «Разница между
/// визирами» are not implemented yet and are shown disabled.
fn time_axis_menu(ui: &mut egui::Ui, unit: TimeUnit, grid: bool, pick: &mut dyn FnMut(MenuCmd)) {
    let on = |u: TimeUnit| ItemState { enabled: true, checked: unit == u };
    for (label, u) in [("Микросекунды", TimeUnit::Micro), ("Миллисекунды", TimeUnit::Milli), ("Секунды", TimeUnit::Sec)] {
        if menu_radio(ui, label, on(u)) {
            pick(MenuCmd::TimeUnit(u));
        }
    }
    menu_radio(ui, "Абсолютное время", ItemState { enabled: false, checked: false });
    if menu_radio(ui, "Отсчеты", on(TimeUnit::Samples)) {
        pick(MenuCmd::TimeUnit(TimeUnit::Samples));
    }
    ui.separator();
    if menu_check(ui, "Сетка", ItemState { enabled: true, checked: grid }) {
        pick(MenuCmd::Grid);
    }
    ui.separator();
    menu_check(ui, "Разница между визирами", ItemState { enabled: false, checked: false });
}

/// MENU 563 (a comment under the pointer), relabelled by the comment's kind
/// as `0x45efa0` does: a moment («визир», strings 206/229/236) or a level
/// line (207/237/238); «Изменить уровень» exists only for a level line.
fn comment_menu(ui: &mut egui::Ui, c: &crate::app::Comment, i: usize, pick: &mut dyn FnMut(MenuCmd)) {
    let level = c.level.is_some();
    let what = if level { "линию уровня" } else { "визир" };
    if menu_item(ui, "Изменить комментарий", true) {
        pick(MenuCmd::EditComment(i));
    }
    let lock = format!("{} {what}", if c.locked { "Разблокировать" } else { "Зафиксировать" });
    if menu_check(ui, &lock, ItemState { enabled: true, checked: c.locked }) {
        pick(MenuCmd::LockComment(i));
    }
    if menu_item(ui, &format!("Удалить {what}"), true) {
        pick(MenuCmd::DeleteComment(i));
    }
    let hide = if c.hidden { "Отображать комментарий" } else { "Скрыть комментарий" };
    if menu_check(ui, hide, ItemState { enabled: true, checked: c.hidden }) {
        pick(MenuCmd::HideComment(i));
    }
    if level && menu_item(ui, "Изменить уровень", true) {
        pick(MenuCmd::EditLevel(i));
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
/// An analog channel's plotted / read-out value in the display mode
/// (`0x461440`): instantaneous, one-period RMS or first harmonic.
fn mode_disp(doc: &crate::app::Doc, i: usize, k: usize, primary: bool) -> f64 {
    let ch = &doc.osc.analog[i];
    let mode = doc.chans[i].mode;
    let sdiv = if doc.chans[i].adc_counts && ch.a.abs() > 1e-12 { ch.a.abs() } else { 1.0 };
    let scale = primary_scale(doc, i, primary) / sdiv;
    match mode {
        ValueMode::Instant => analog_disp(doc, i, k, primary),
        ValueMode::Rms => doc.rms_at(i, k) * scale,
        ValueMode::FirstHarmonic => doc.harmonic_at(i, k) * doc.osc.harmonic1_gain * scale,
    }
}

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
    primary: bool,
) -> (String, String, Color32, f64, f64, f64, f64, f64) {
    match lane {
        Lane::Analog(i) => {
            let ch = &doc.osc.analog[*i];
            let adc = doc.chans[*i].adc_counts;
            // RMS/1st-harmonic are computed in scaled units; divide by |a| for ADC.
            let sdiv = if adc && ch.a.abs() > 1e-12 { ch.a.abs() } else { 1.0 };
            let scale = primary_scale(doc, *i, primary);
            // Value at the cursor and Max/Min follow the display mode; СКЗ
            // and A1 are always the one-period RMS and first harmonic.
            let n = doc.osc.sample_count();
            let mut mx = f64::MIN;
            let mut mn = f64::MAX;
            for k in 0..n {
                let v = mode_disp(doc, *i, k, primary);
                mx = mx.max(v);
                mn = mn.min(v);
            }
            let rms = doc.rms_at(*i, k_main) * scale / sdiv;
            let a1 = doc.harmonic_at(*i, k_main) * doc.osc.harmonic1_gain * scale / sdiv;
            let val = mode_disp(doc, *i, k_main, primary);
            (ch.id.clone(), analog_unit(doc, *i), doc.chans[*i].color, mx, mn, rms, a1, val)
        }
        Lane::Computed(i) => {
            // A computed channel is read out like any other channel.
            let c = &doc.computed[*i];
            let n = doc.osc.samples_per_cycle().max(2);
            // Our computed channels carry no display mode: instantaneous.
            let series = |k: usize| c.samples.get(k).copied().unwrap_or(0.0);
            let (mut mx, mut mn) = (f64::MIN, f64::MAX);
            for k in 0..c.samples.len() {
                let v = series(k);
                mx = mx.max(v);
                mn = mn.min(v);
            }
            let rms = fvb_core::signal::rms_at(&c.samples, k_main, n);
            let a1 = fvb_core::signal::harmonic_rms(&c.samples, k_main, n, 1);
            (c.name.clone(), c.unit.clone(), c.color, mx, mn, rms, a1, series(k_main))
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
        assert!(ticks.iter().all(|&t| (-1e-9..=1.0 + 0.3).contains(&t)));
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;
    use crate::app::Command;

    const SCREEN: Vec2 = Vec2::new(1200.0, 800.0);

    struct Harness {
        ctx: egui::Context,
        app: App,
        t: f64,
    }

    impl Harness {
        fn new() -> Harness {
            let mut app = App::new();
            app.dispatch(Command::Demo);
            let mut h = Harness { ctx: egui::Context::default(), app, t: 0.0 };
            h.frame(vec![]);
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            self.t += 0.05;
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
                time: Some(self.t),
                events,
                ..Default::default()
            };
            let app = &mut self.app;
            let _ = self.ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.lanes_ui(ui));
            });
        }

        fn button(&mut self, pos: Pos2, button: egui::PointerButton) {
            let ev = |pressed| egui::Event::PointerButton { pos, button, pressed, modifiers: Default::default() };
            self.frame(vec![egui::Event::PointerMoved(pos), ev(true)]);
            self.frame(vec![ev(false)]);
            self.frame(vec![]);
            self.frame(vec![]);
        }

        fn menu_rect(&self) -> Rect {
            let m = self.app.lane_menu.expect("a menu is open");
            self.ctx.memory(|mem| mem.area_rect(egui::Id::new("fv_lane_menu").with(m.serial))).expect("menu area")
        }

        /// Centre of stats-panel row `li` and the axis band, from the layout
        /// `lanes_ui` uses (central panel margin 8 px).
        fn row(&self, li: usize) -> Pos2 {
            let (top, h) = self.geometry();
            Pos2::new(40.0, top + h * (li as f32 + 0.5))
        }

        fn geometry(&self) -> (f32, f32) {
            let d = self.app.active_doc().unwrap();
            let n = d.chans.len() + d.computed.len() + d.osc.digital.len();
            let avail = SCREEN.y - 16.0;
            (8.0, ((avail - AXIS_H) / n as f32).max(MIN_LANE))
        }

        fn rows(&self) -> usize {
            let d = self.app.active_doc().unwrap();
            d.chans.len() + d.computed.len() + d.osc.digital.len()
        }
    }

    #[test]
    fn hit_test_reaches_the_top_row_and_the_axis_band() {
        let lanes = [ChanKey::Analog(0), ChanKey::Analog(1)];
        assert_eq!(hit_test(Pos2::new(10.0, 0.5), 0.0, 158.0, 200.0, 60.0, &lanes), Hit::Stats(Some(ChanKey::Analog(0))));
        assert_eq!(hit_test(Pos2::new(10.0, 61.0), 0.0, 158.0, 200.0, 60.0, &lanes), Hit::Stats(Some(ChanKey::Analog(1))));
        assert_eq!(hit_test(Pos2::new(10.0, 150.0), 0.0, 158.0, 200.0, 60.0, &lanes), Hit::Stats(None));
        assert_eq!(hit_test(Pos2::new(300.0, 30.0), 0.0, 158.0, 200.0, 60.0, &lanes), Hit::Plot);
        assert_eq!(hit_test(Pos2::new(300.0, 210.0), 0.0, 158.0, 200.0, 60.0, &lanes), Hit::Axis);
    }

    /// User report: the menu was never about the top row, and its subject
    /// shifted while the mouse moved over the menu.
    #[test]
    fn menu_subject_is_the_right_clicked_row_and_stays_while_the_mouse_moves() {
        let mut h = Harness::new();
        let top = h.row(0);
        h.button(top, egui::PointerButton::Secondary);
        let menu = h.app.lane_menu.expect("right-click on the stats panel opens the channel menu");
        assert_eq!(menu.kind, LaneMenuKind::Channels);
        let want = std::collections::BTreeSet::from([ChanKey::Analog(0)]);
        assert_eq!(h.app.active_doc().unwrap().selection, want, "the top row is the subject");
        // Wander over the menu, then over other rows and the plot.
        for p in [top + Vec2::new(30.0, 40.0), h.row(3), Pos2::new(600.0, 300.0), h.row(h.rows() - 1)] {
            h.frame(vec![egui::Event::PointerMoved(p)]);
        }
        assert_eq!(h.app.lane_menu, Some(menu), "same menu, same subject");
        assert_eq!(h.app.active_doc().unwrap().selection, want);
    }

    /// User report: opened low in the view the menu became long and a few
    /// characters wide, and stayed so until the program was restarted.
    #[test]
    fn menu_is_sized_to_its_items_on_every_opening() {
        // Reference: the channel menu opened on a fresh context.
        let mut fresh = Harness::new();
        let r = fresh.row(0);
        fresh.button(r, egui::PointerButton::Secondary);
        let reference = fresh.menu_rect();
        assert!(reference.width() > 250.0, "items are not wrapped: {reference:?}");

        // Open small menus low in the view first (the axis band, the last
        // row, a right-click below the rows), dismissing each.
        let mut h = Harness::new();
        let bottom = SCREEN.y - 12.0;
        let last = h.row(h.rows() - 1);
        for p in [Pos2::new(600.0, bottom), last, Pos2::new(40.0, bottom - AXIS_H - 2.0)] {
            h.button(p, egui::PointerButton::Secondary);
            assert!(h.app.lane_menu.is_some(), "no menu at {p:?} (rows {}, geometry {:?})", h.rows(), h.geometry());
            h.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }]);
            assert!(h.app.lane_menu.is_none(), "Escape closes the menu");
            // egui still treats the closed popup as on top for the next frame.
            h.frame(vec![]);
        }
        let r = h.row(0);
        h.button(r, egui::PointerButton::Secondary);
        let rect = h.menu_rect();
        assert!((rect.width() - reference.width()).abs() < 1.0, "{rect:?} vs fresh {reference:?}");
        assert!((rect.height() - reference.height()).abs() < 1.0, "{rect:?} vs fresh {reference:?}");
    }

    #[test]
    fn right_click_on_the_plot_places_vizir_2_without_a_menu() {
        let mut h = Harness::new();
        let before = h.app.active_doc().unwrap().aux_cursor;
        h.button(Pos2::new(700.0, h.row(0).y), egui::PointerButton::Secondary);
        assert!(h.app.lane_menu.is_none());
        assert_ne!(h.app.active_doc().unwrap().aux_cursor, before);
    }

    #[test]
    fn a_click_outside_dismisses_the_menu_without_moving_a_cursor() {
        let mut h = Harness::new();
        let r = h.row(0);
        h.button(r, egui::PointerButton::Secondary);
        assert!(h.app.lane_menu.is_some());
        let main = h.app.active_doc().unwrap().main_cursor;
        h.button(Pos2::new(900.0, 300.0), egui::PointerButton::Primary);
        assert!(h.app.lane_menu.is_none());
        assert_eq!(h.app.active_doc().unwrap().main_cursor, main, "the dismissing click is swallowed");
    }
}
