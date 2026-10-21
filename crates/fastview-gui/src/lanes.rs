//! The lane-based oscillogram view — the original's primary document window.
//!
//! Each analog/computed/digital channel is a horizontal lane with a left stats
//! panel (name badge · value@cursor · Max/Min/СКЗ/A1) and a waveform. Two
//! vertical визиры (red main, blue aux) with a shaded selection between them,
//! a shared time axis, on-waveform comments, and the original's context menus
//! chosen by click region (MENU 560 axis band, 561 stats panel acting on the
//! selected channels, 563 on a comment — ledger L9.1). Lanes have a minimum
//! height and the view scrolls vertically when there are many channels.

use crate::app::{App, ChanKey, Command, CommentEdit, TimeUnit, ValueMode};
use crate::oscgeom as g;
use crate::chansel::{select_click, ChanFlag, ItemState, SelClick};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

/// Stats-panel width before the first measurement (`0x4606d0` default).
pub const LEFT_W_DEFAULT: f32 = 125.0;
/// Names are cut to this many characters in the stats panel.
const NAME_CHARS: usize = 25;
/// Name / value font (Tahoma 15 bold / heavy in the original) and the row
/// font (Tahoma 13); `ROW_H` is the name font's line height ("Ay").
const NAME_FONT: FontId = FontId::proportional(13.0);
const ROW_FONT: FontId = FontId::proportional(11.0);
const ROW_H: f32 = 15.0;
/// Horizontal scrollbar height below the view and vertical scrollbar width.
const HSCROLL_H: f32 = 14.0;
const VSCROLL_W: f32 = 12.0;
/// Height of one member row of a channel group lane (name row + 33).
const GROUP_ROW_H: f32 = 49.0;
const COMMENT_COLOR: Color32 = Color32::from_rgb(0x00, 0x80, 0x80);

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

/// Classify `pos` (`0x45dc10` region order: bottom band, stats panel, plot).
pub fn hit_test(pos: Pos2, geo: &RowGeo) -> Hit {
    if pos.y > geo.axis_top {
        Hit::Axis
    } else if pos.x >= geo.left {
        Hit::Plot
    } else {
        Hit::Stats(geo.tops.iter().zip(geo.rows).find(|(&t, r)| pos.y >= t && pos.y < t + r.h).map(|(_, r)| r.key))
    }
}

/// Which визир is being dragged (`view+0xe4` modes 1–3 vertical, 4–5 horizontal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VizDrag {
    V(usize),
    H(usize),
}

/// One lane of the view: its row (an analog lane carries all members of its
/// channel group, the first being the row's own channel) and its height.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: ChanKey,
    pub members: Vec<usize>,
    pub h: f32,
}

impl Row {
    fn selected(&self, doc: &crate::app::Doc) -> bool {
        doc.selection.contains(&self.key) || self.members.iter().any(|&m| doc.selection.contains(&Lane::Analog(m)))
    }
}

/// Lane order and heights (`0x467ed0`, `0x460290`): visible analog channels
/// (a group's members pulled together at its first member), computed
/// channels, then digital channels; analog lanes `YAnalogSize` high (a group
/// at least one name row per member), digital lanes 20 px. No stretch.
pub fn lane_layout(doc: &crate::app::Doc, y_analog: f32, primary: bool) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let mut group_row: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (i, c) in doc.chans.iter().enumerate() {
        if !c.visible {
            continue;
        }
        if let Some(gid) = c.group {
            if let Some(&ri) = group_row.get(&gid) {
                rows[ri].members.push(i);
                continue;
            }
            group_row.insert(gid, rows.len());
        }
        rows.push(Row { key: Lane::Analog(i), members: vec![i], h: y_analog });
    }
    rows.extend((0..doc.computed.len()).map(|i| Row { key: Lane::Computed(i), members: Vec::new(), h: y_analog }));
    // need = name row + 10 + one row per enabled stat (Max/Min two) + 23
    // when any status icon shows; a group lane one name row (+33) per member.
    for r in &mut rows {
        let icons = if status_icons(doc, r, primary).is_empty() { 0.0 } else { 23.0 };
        let need = if r.members.len() > 1 {
            r.members.len() as f32 * GROUP_ROW_H
        } else {
            ROW_H + 10.0 + stat_rows(doc, &r.key).map_or(0, |s| s.count()) as f32 * ROW_H + icons
        };
        r.h = r.h.max(need);
    }
    rows.extend((0..doc.osc.digital.len()).map(|i| Row { key: Lane::Digital(i), members: Vec::new(), h: g::DIGITAL_H }));
    rows
}

/// The stat rows a lane shows (none for digital lanes).
fn stat_rows(doc: &crate::app::Doc, key: &ChanKey) -> Option<crate::app::StatRows> {
    match key {
        Lane::Analog(i) => Some(doc.chans[*i].stats),
        Lane::Computed(i) => Some(doc.computed[*i].stats),
        Lane::Digital(_) => None,
    }
}

/// Status icons of a lane (bitmap 0x136 in the original; our own glyphs):
/// window zoom, remove DC, invert, primary/secondary scaling, RMS mode,
/// harmonic mode, ADC counts.
fn status_icons(doc: &crate::app::Doc, r: &Row, primary: bool) -> Vec<&'static str> {
    let Lane::Analog(i) = r.key else { return Vec::new() };
    let (c, ch) = (&doc.chans[i], &doc.osc.analog[i]);
    let scaled = primary && !ch.is_primary && ch.secondary != 0.0 && ch.primary / ch.secondary != 1.0;
    [
        (c.vzoom != 1.0, "↕"),
        (c.remove_dc, "DC"),
        (c.invert, "−"),
        (scaled, "П"),
        (c.mode == ValueMode::Rms, "~"),
        (c.mode == ValueMode::FirstHarmonic, "1"),
        (c.adc_counts, "#"),
    ]
    .into_iter()
    .filter_map(|(on, g)| on.then_some(g))
    .collect()
}

/// Stats-panel columns (`0x4606d0`): the label column is the widest
/// (cut) name + 10 (at least «Max:»); the panel adds a value template —
/// `": -0.00000000 A"` for SI units, `": -0.0000000E-000 "` otherwise.
fn stats_columns(fonts: &egui::text::Fonts, doc: &crate::app::Doc, rows: &[Row], primary: bool) -> (f32, f32) {
    let w = |s: &str, f: &FontId| fonts.layout_no_wrap(s.to_string(), f.clone(), Color32::BLACK).size().x;
    let mut label = w("Max:", &ROW_FONT);
    let mut template: f32 = 0.0;
    for r in rows {
        let (name, unit, ..) = lane_stats(doc, &r.key, 0, primary);
        let names = std::iter::once(name).chain(r.members.iter().skip(1).map(|&m| doc.osc.analog[m].id.clone()));
        for n in names {
            let short: String = n.chars().take(NAME_CHARS).collect();
            label = label.max(w(&short, &NAME_FONT) + 10.0);
        }
        let t = if fvb_core::views::si_unit(&unit) { ": -0.00000000 A" } else { ": -0.0000000E-000 " };
        template = template.max(w(t, &ROW_FONT));
    }
    if rows.is_empty() {
        return (label, LEFT_W_DEFAULT);
    }
    (label, label + template)
}

/// Text in a heavier weight (egui's default font has no bold face: the
/// glyphs are drawn twice, half a pixel apart).
fn bold_text(p: &egui::Painter, pos: Pos2, text: &str, font: FontId, color: Color32) -> Rect {
    p.text(pos + Vec2::new(0.5, 0.0), Align2::LEFT_TOP, text, font.clone(), color);
    p.text(pos, Align2::LEFT_TOP, text, font, color)
}

/// The amplitude range of a lane: min..max of the displayed series over the
/// whole record (a group: min of mins, max of maxes).
fn row_range(doc: &crate::app::Doc, r: &Row, primary: bool) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut take = |v: f64| {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    };
    match r.key {
        Lane::Analog(_) => {
            for &m in &r.members {
                for k in 0..doc.osc.sample_count() {
                    take(mode_disp(doc, m, k, primary));
                }
            }
        }
        Lane::Computed(i) => doc.computed[i].samples.iter().for_each(|&v| take(v)),
        Lane::Digital(_) => return (0.0, 1.0),
    }
    if lo > hi {
        (0.0, 0.0)
    } else {
        (lo, hi)
    }
}

/// Fill `rect` with a vertical gradient.
fn gradient(p: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
}

/// Runs of "1" samples within `first..=last`.
fn digital_runs(s: &[bool], first: usize, last: usize) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = None;
    let last = last.min(s.len().saturating_sub(1));
    for (k, &on) in s.iter().enumerate().take(last + 1).skip(first) {
        match (on, start) {
            (true, None) => start = Some(k),
            (false, Some(a)) => {
                runs.push((a, k - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(a) = start {
        runs.push((a, last));
    }
    runs
}

/// A waveform: every sample of `first..=last` as a 1 px polyline, with 4×4
/// circles at the samples above 4 px per sample (`f > 0.16`).
#[allow(clippy::too_many_arguments)]
fn draw_series(
    p: &egui::Painter,
    (first, last): (usize, usize),
    value: &dyn Fn(usize) -> f64,
    x_of: &dyn Fn(f64) -> f32,
    top: f32,
    h: f32,
    (lo, hi): (f64, f64),
    color: Color32,
    f: f64,
) {
    if last < first {
        return;
    }
    let pts: Vec<Pos2> = (first..=last).map(|k| Pos2::new(x_of(k as f64), g::value_y(value(k), top, h, lo, hi))).collect();
    if f > 0.16 {
        for q in &pts {
            p.circle_stroke(*q, 2.0f32, Stroke::new(1.0f32, color));
        }
    }
    p.add(Shape::line(pts, Stroke::new(1.0f32, color)));
}

/// Row geometry for hit-testing.
pub struct RowGeo<'a> {
    pub left: f32,
    pub axis_top: f32,
    pub tops: &'a [f32],
    pub rows: &'a [Row],
}

/// The analog lane under `y` and the value there on its scale.
fn analog_at(y: f32, geo: &RowGeo, ranges: &[(f64, f64)]) -> Option<(usize, f64)> {
    let li = geo.tops.iter().zip(geo.rows).position(|(&t, r)| y >= t && y < t + r.h)?;
    let r = &geo.rows[li];
    let Lane::Analog(ch) = r.key else { return None };
    let (lo, hi) = ranges[li];
    Some((ch, g::y_value(y, geo.tops[li], r.h, lo, hi)))
}

/// A scrollbar over `[0, max]` with a page of `page`; returns the new value
/// when the thumb is dragged or the track clicked (one page per click).
fn scrollbar(ui: &mut egui::Ui, p: &egui::Painter, rect: Rect, horizontal: bool, value: f32, max: f32, page: f32) -> Option<f32> {
    p.rect_filled(rect, 0.0, Color32::from_gray(236));
    if max <= 0.0 {
        return None;
    }
    let len = if horizontal { rect.width() } else { rect.height() };
    let thumb_len = (len * page / (max + page)).clamp(16.0, len);
    let travel = (len - thumb_len).max(1.0);
    let at = travel * (value / max).clamp(0.0, 1.0);
    let thumb = if horizontal {
        Rect::from_min_size(Pos2::new(rect.left() + at, rect.top() + 2.0), Vec2::new(thumb_len, rect.height() - 4.0))
    } else {
        Rect::from_min_size(Pos2::new(rect.left() + 2.0, rect.top() + at), Vec2::new(rect.width() - 4.0, thumb_len))
    };
    p.rect_filled(thumb, 2.0, Color32::from_gray(176));
    let id = ui.id().with(("fv_scroll", horizontal));
    let r = ui.interact(rect, id, Sense::click_and_drag());
    if r.dragged() {
        let d = if horizontal { r.drag_delta().x } else { r.drag_delta().y };
        return Some((value + d / travel * max).clamp(0.0, max));
    }
    if r.clicked() {
        let pos = r.interact_pointer_pos()?;
        let before = if horizontal { pos.x < thumb.left() } else { pos.y < thumb.top() };
        return Some((value + if before { -page } else { page }).clamp(0.0, max));
    }
    None
}

enum Act {
    PlaceV(usize, usize),
    MoveV(usize, usize),
    SetH(usize, usize, f64),
    Drag(VizDrag),
    EndDrag,
    SetFirst(usize),
    SetVScroll(f32),
    PlaceMoment(usize),
    EditComment(usize),
    Rename(usize),
    Select(ChanKey, SelClick),
    ClearSelection,
    OpenMenu(Pos2, LaneMenuKind),
    Cmd(Command),
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
        let (time_unit, show_grid, primary, y_analog) = (self.time_unit, self.show_grid, self.primary_values, self.y_analog);
        let full = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(full, Sense::click_and_drag());
        self.view_width = full.width();
        {
            let pref = self.xscale_pref;
            let d = &mut self.docs[ai];
            if d.xscale.is_none() {
                d.xscale = Some(g::initial_scale(pref, full.width(), d.osc.sample_count()));
            }
        }
        let mut actions: Vec<Act> = Vec::new();
        let menu_busy = self.lane_menu.is_some() || self.lane_menu_eat;
        let doc = &self.docs[ai];
        let n = doc.osc.sample_count();
        let f = doc.xscale.unwrap_or(1.0);

        // ---- layout (`0x460290`, `0x45d4a0`) ----
        let band = g::RULER_H + if doc.diff_band { g::DIFF_BAND_H } else { 0.0 };
        let client = Rect::from_min_max(full.min, Pos2::new(full.right(), full.bottom() - HSCROLL_H));
        let rows = lane_layout(doc, y_analog, primary);
        let (label_col, left_w) = ui.fonts(|fo| stats_columns(fo, doc, &rows, primary));
        self.view_left = left_w;
        let content_h: f32 = rows.iter().map(|r| r.h).sum();
        let vbar = content_h + band + 2.0 > client.height();
        let plot_right = client.right() - if vbar { VSCROLL_W } else { 0.0 };
        let left = client.left() + left_w;
        let plot_bottom = client.bottom() - band;
        let vmin = -(content_h + band + 2.0 - client.height()).max(0.0);
        let vscroll = doc.vscroll.clamp(vmin, 0.0);
        let visible = g::visible_count(plot_right - client.left(), left_w, f);
        let first = doc.first.min(g::max_first(n, visible));
        let last = (first + visible + 1).min(n.saturating_sub(1));
        let x_of = |k: f64| g::x_of(k, first, f, left);
        let sample_at = |x: f32| g::sample_at(x, first, f, left).clamp(0, n.max(1) as i64 - 1) as usize;
        let mut tops = Vec::with_capacity(rows.len());
        let mut y = client.top() + vscroll;
        for r in &rows {
            tops.push(y);
            y += r.h;
        }
        let ranges: Vec<(f64, f64)> = rows.iter().map(|r| row_range(doc, r, primary)).collect();
        let grid_bottom = client.bottom() - g::RULER_H;
        let lanes_clip = Rect::from_min_max(client.min, Pos2::new(plot_right, plot_bottom));
        let painter = ui.painter_at(client);
        let lp = painter.with_clip_rect(lanes_clip);
        let pp = painter.with_clip_rect(Rect::from_min_max(Pos2::new(left, client.top()), Pos2::new(plot_right, plot_bottom)));

        // ---- backgrounds, separators, grid, divider (`0x462ff0`) ----
        painter.rect_filled(client, 0.0, Color32::WHITE);
        for (li, r) in rows.iter().enumerate() {
            let lane = Rect::from_min_size(Pos2::new(client.left(), tops[li]), Vec2::new(plot_right - client.left(), r.h));
            if r.selected(doc) {
                gradient(&lp, lane, Color32::from_rgb(254, 225, 140), Color32::from_rgb(253, 252, 173));
            } else if li % 2 == 1 {
                gradient(&lp, lane, Color32::from_rgb(214, 255, 255), Color32::from_rgb(243, 255, 255));
            }
            lp.hline(client.left()..=plot_right, tops[li] + r.h - 1.0, Stroke::new(1.0f32, Color32::from_rgb(200, 200, 200)));
        }
        if show_grid {
            let mut x = left + 50.0;
            while x < plot_right {
                painter.vline(x, client.top()..=grid_bottom, Stroke::new(1.0f32, Color32::from_rgb(230, 230, 230)));
                x += 50.0;
            }
        }
        painter.vline(left, client.top()..=grid_bottom, Stroke::new(1.0f32, Color32::from_rgb(100, 100, 100)));

        // ---- stats panel and waveforms ----
        let k_main = doc.main_cursor;
        let mut rename_rect = None;
        let mut cut_hover: Option<String> = None;
        let hover = resp.hover_pos();
        for (li, r) in rows.iter().enumerate() {
            let (top, h) = (tops[li], r.h);
            if top > plot_bottom || top + h < client.top() {
                continue;
            }
            let lane = &r.key;
            let (name, unit, color, maxv, minv, rms, a1, val) = lane_stats(doc, lane, k_main, primary);
            // Stats text (`0x461440`): the name (cut to 25 characters) framed
            // in its colour, the value at В1 after the label column, then
            // the enabled rows; digital lanes show the name only.
            let short: String = name.chars().take(NAME_CHARS).collect();
            let name_pos = Pos2::new(client.left() + 5.0, top + if matches!(lane, Lane::Digital(_)) { 2.0 } else { 5.0 });
            let name_rect = bold_text(&lp, name_pos, &short, NAME_FONT, color);
            if short.len() < name.len() && hover.is_some_and(|p| name_rect.contains(p)) {
                cut_hover = Some(name.clone());
            }
            if let (Lane::Analog(i), Some((ri, _))) = (lane, &self.rename_edit) {
                if i == ri {
                    rename_rect = Some(name_rect.expand2(Vec2::new(2.0, 1.0)));
                }
            }
            let col_x = client.left() + label_col + 5.0;
            if !matches!(lane, Lane::Digital(_)) {
                lp.rect_stroke(
                    Rect::from_min_max(Pos2::new(client.left() + 3.0, top + 5.0), Pos2::new(name_rect.right() + 2.0, name_rect.bottom())),
                    0.0,
                    Stroke::new(1.0f32, Color32::from_gray(128)),
                );
                if let Some(t) = fvb_core::views::stats_value_text(val, &unit) {
                    bold_text(&lp, Pos2::new(col_x, top + 5.0), &t, NAME_FONT, Color32::from_rgb(69, 85, 199));
                }
            }
            let row_col = Color32::from_rgb(0, 0, 127);
            let mut yy = top + 5.0 + ROW_H + 3.0;
            if r.members.len() > 1 {
                // A channel group: its other members listed under the first.
                for &oi in &r.members[1..] {
                    let (on, ou, oc, .., ov) = lane_stats(doc, &Lane::Analog(oi), k_main, primary);
                    let on: String = on.chars().take(NAME_CHARS).collect();
                    bold_text(&lp, Pos2::new(client.left() + 5.0, yy), &on, NAME_FONT, oc);
                    if let Some(t) = fvb_core::views::stats_value_text(ov, &ou) {
                        bold_text(&lp, Pos2::new(col_x, yy), &t, NAME_FONT, Color32::from_rgb(69, 85, 199));
                    }
                    yy += ROW_H + 4.0;
                }
            } else if let Some(st) = stat_rows(doc, lane) {
                let mut rows_txt: Vec<(&str, f64)> = Vec::new();
                if st.minmax {
                    rows_txt.extend([("Max:", maxv), ("Min:", minv)]);
                }
                if st.rms {
                    rows_txt.push(("СКЗ:", rms));
                }
                if st.a1 {
                    rows_txt.push(("A1:", a1));
                }
                for (lbl, v) in rows_txt {
                    lp.text(Pos2::new(client.left() + 5.0, yy), Align2::LEFT_TOP, lbl, ROW_FONT, row_col);
                    lp.text(Pos2::new(col_x, yy), Align2::LEFT_TOP, fvb_core::views::readout_text(v, &unit), ROW_FONT, row_col);
                    yy += ROW_H;
                }
            }
            // Status icons, right to left from the panel edge at the lane bottom.
            for (j, glyph) in status_icons(doc, r, primary).into_iter().enumerate() {
                let at = Rect::from_min_size(Pos2::new(left - 21.0 - 21.0 * j as f32, top + h - 21.0), Vec2::splat(16.0));
                lp.rect_filled(at, 2.0, Color32::from_rgb(232, 236, 248));
                lp.rect_stroke(at, 2.0, Stroke::new(1.0f32, Color32::from_rgb(120, 130, 170)));
                lp.text(at.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(9.0), Color32::from_rgb(40, 50, 110));
            }

            // Waveforms (`0x45fbe0`): every visible sample, 1 px, sample marks
            // above 4 px per sample.
            let (lo, hi) = ranges[li];
            match lane {
                Lane::Digital(i) => {
                    let s = &doc.osc.digital[*i].samples;
                    let mid = top + 10.0;
                    let end_x = x_of(n.saturating_sub(1) as f64).min(plot_right);
                    pp.rect_filled(Rect::from_min_max(Pos2::new(left, mid - 2.0), Pos2::new(end_x, mid + 2.0)), 0.0, Color32::from_rgb(150, 150, 255));
                    let fill = if r.selected(doc) { Color32::from_rgb(253, 219, 102) } else { color };
                    for (a, b) in digital_runs(s, first, last) {
                        let x0 = x_of(a as f64);
                        let x1 = if b + 1 >= n { x_of(b as f64) + 4.0 } else { x_of((b + 1) as f64) };
                        let rr = Rect::from_min_max(Pos2::new(x0, mid - 6.0), Pos2::new(x1.max(x0 + 1.0), mid + 6.0));
                        pp.rect_filled(rr, 0.0, fill);
                        pp.rect_stroke(rr, 0.0, Stroke::new(1.0f32, Color32::from_rgb(200, 0, 0)));
                    }
                }
                _ => {
                    if lo <= 0.0 && 0.0 <= hi {
                        let zy = g::value_y(0.0, top, h, lo, hi);
                        pp.hline(left..=plot_right, zy, Stroke::new(1.0f32, Color32::from_rgb(200, 200, 200)));
                    }
                    for &m in &r.members {
                        let c = doc.chans[m].color;
                        draw_series(&pp, (first, last), &|k| mode_disp(doc, m, k, primary), &x_of, top, h, (lo, hi), c, f);
                    }
                    if let Lane::Computed(i) = lane {
                        let s = &doc.computed[*i].samples;
                        draw_series(&pp, (first, last.min(s.len().saturating_sub(1))), &|k| s.get(k).copied().unwrap_or(0.0), &x_of, top, h, (lo, hi), color, f);
                    }
                }
            }
        }

        // ---- comments (moment markers) ----
        for (ci, c) in doc.comments.iter().enumerate() {
            let x = x_of(c.sample as f64);
            if x < left || x > plot_right {
                continue;
            }
            pp.vline(x, client.top()..=plot_bottom, Stroke::new(1.2f32, COMMENT_COLOR));
            pp.rect_filled(Rect::from_min_size(Pos2::new(x, client.top() + 1.0), Vec2::new(12.0, 12.0)), 0.0, COMMENT_COLOR);
            pp.text(Pos2::new(x + 6.0, client.top() + 7.0), Align2::CENTER_CENTER, format!("{}", ci + 1), FontId::proportional(9.0), Color32::WHITE);
            if !c.hidden && !c.text.is_empty() {
                let label: String = c.text.lines().next().unwrap_or("").chars().take(24).collect();
                pp.text(Pos2::new(x + 15.0, client.top() + 2.0), Align2::LEFT_TOP, label, FontId::proportional(10.0), COMMENT_COLOR);
            }
        }

        // ---- time ruler (`0x464350`) and difference band (`0x466630`) ----
        let ruler_top = client.bottom() - g::RULER_H;
        painter.rect_filled(Rect::from_min_max(Pos2::new(client.left(), ruler_top), client.max), 0.0, Color32::WHITE);
        painter.hline(client.left()..=client.right(), ruler_top, Stroke::new(1.0f32, Color32::BLACK));
        let t0 = doc.osc.time.first().copied().unwrap_or(0.0);
        let time_at = |k: usize| doc.osc.time.get(k).copied().unwrap_or(0.0) - t0;
        let text_col = Color32::from_rgb(0, 0, 127);
        let mut first_label = true;
        for t in g::ruler_ticks(left, plot_right) {
            painter.vline(t.x, ruler_top..=ruler_top + t.len, Stroke::new(1.0f32, Color32::from_rgb(100, 100, 100)));
            if t.labelled {
                let k = g::tick_sample(first, t.i, f);
                if k < n {
                    let s = g::ruler_label(time_unit, time_at(k), k as i64, first_label);
                    painter.text(Pos2::new(t.x, client.bottom() - 20.0), Align2::CENTER_TOP, s, FontId::proportional(11.0), text_col);
                    first_label = false;
                }
            }
        }
        let viz = [
            (doc.main_cursor, doc.v_shown[0], Color32::from_rgb(255, 100, 100), "В1"),
            (doc.aux_cursor, doc.v_shown[1], Color32::from_rgb(100, 100, 255), "В2"),
            (doc.v3_cursor, doc.v_shown[2], Color32::from_rgb(0, 160, 0), "В3"),
        ];
        if doc.diff_band {
            let band_top = ruler_top - g::DIFF_BAND_H;
            painter.rect_filled(Rect::from_min_max(Pos2::new(client.left(), band_top), Pos2::new(plot_right, ruler_top)), 0.0, Color32::WHITE);
            let mut marks: Vec<(f32, usize, Option<&str>)> = viz.iter().filter(|v| v.1).map(|v| (x_of(v.0 as f64), v.0, None)).collect();
            marks.extend(doc.comments.iter().filter(|c| !c.hidden).map(|c| (x_of(c.sample as f64), c.sample, Some(c.text.as_str()))));
            marks.retain(|m| m.0 > left && m.0 <= plot_right);
            marks.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut taken: Vec<Rect> = Vec::new();
            for (x, k, text) in &marks {
                let s = g::ruler_label(time_unit, time_at(*k), *k as i64, true);
                let galley = painter.layout_no_wrap(s, FontId::proportional(11.0), text_col);
                let r = Rect::from_center_size(Pos2::new(*x, ruler_top - 18.0), galley.size()).expand(3.0);
                if taken.iter().any(|t| t.intersects(r)) {
                    continue;
                }
                taken.push(r);
                painter.galley(r.min + Vec2::splat(3.0), galley, text_col);
                if let Some(t) = text.filter(|t| !t.is_empty()) {
                    painter.text(Pos2::new(*x, ruler_top - 10.0), Align2::CENTER_TOP, t.lines().next().unwrap_or(""), FontId::proportional(9.0), COMMENT_COLOR);
                }
            }
            let dim_y = ruler_top - 35.0;
            for w in marks.windows(2) {
                let (xa, xb) = (w[0].0, w[1].0);
                if xb - xa <= 16.0 {
                    continue;
                }
                let st = Stroke::new(1.0f32, text_col);
                painter.hline(xa..=xb, dim_y, st);
                painter.vline(xa, dim_y - 4.0..=dim_y + 4.0, st);
                painter.vline(xb, dim_y - 4.0..=dim_y + 4.0, st);
                for (tip, dir) in [(xa, 1.0f32), (xb, -1.0)] {
                    painter.add(Shape::convex_polygon(
                        vec![Pos2::new(tip, dim_y), Pos2::new(tip + 8.0 * dir, dim_y - 2.0), Pos2::new(tip + 8.0 * dir, dim_y + 2.0)],
                        text_col,
                        Stroke::NONE,
                    ));
                }
                let dt = (time_at(w[1].1) - time_at(w[0].1)).abs();
                let s = g::ruler_label(time_unit, dt, w[1].1 as i64 - w[0].1 as i64 - 1, true);
                let galley = painter.layout_no_wrap(s, FontId::proportional(10.0), text_col);
                if galley.size().x + 4.0 < xb - xa && xb - xa > 20.0 {
                    let pos = Pos2::new((xa + xb) / 2.0 - galley.size().x / 2.0, dim_y - galley.size().y - 1.0);
                    painter.galley(pos, galley, text_col);
                }
            }
        }

        // ---- визиры (`0x493110`): no shading between В1 and В2 ----
        let v_bottom = if doc.diff_band { ruler_top - 25.0 } else { ruler_top };
        for (k, shown, col, label) in viz {
            let x = x_of(k as f64);
            if !shown || x <= left || x > plot_right {
                continue;
            }
            painter.vline(x, client.top()..=v_bottom, Stroke::new(1.0f32, col));
            let galley = painter.layout_no_wrap(label.to_string(), FontId::proportional(11.0), col);
            let r = Rect::from_center_size(Pos2::new(x, client.top() + 8.0), galley.size() + Vec2::new(4.0, 0.0));
            painter.rect_filled(r, 0.0, Color32::WHITE);
            painter.galley(r.min + Vec2::new(2.0, 0.0), galley, col);
        }
        let mut hrows: [Option<(f32, usize)>; 2] = [None, None];
        for (j, h) in doc.hviz.iter().enumerate() {
            let Some(ch) = h.chan else { continue };
            let Some(li) = rows.iter().position(|r| r.members.contains(&ch)) else { continue };
            let (lo, hi) = ranges[li];
            if h.value.is_nan() {
                // First show: 10 / 20 px from the top of its lane.
                let v = g::y_value(tops[li] + 10.0 * (j as f32 + 1.0), tops[li], rows[li].h, lo, hi);
                actions.push(Act::SetH(j, ch, v));
                continue;
            }
            let yy = g::value_y(h.value, tops[li], rows[li].h, lo, hi);
            hrows[j] = Some((yy, li));
            if !h.shown || yy >= plot_bottom || yy < client.top() {
                continue;
            }
            let col = if j == 0 { Color32::from_rgb(255, 100, 100) } else { Color32::from_rgb(100, 100, 255) };
            painter.hline(left..=plot_right, yy, Stroke::new(1.0f32, col));
            painter.text(Pos2::new((left + plot_right) / 2.0, yy - 1.0), Align2::CENTER_BOTTOM, if j == 0 { "ГВ1" } else { "ГВ2" }, FontId::proportional(11.0), col);
        }

        // ---- scrollbars (line and page step: one screen / 100 px) ----
        let hbar = Rect::from_min_max(Pos2::new(left, client.bottom()), Pos2::new(plot_right, full.bottom()));
        let hmax = g::max_first(n, visible);
        if let Some(v) = scrollbar(ui, &painter, hbar, true, first as f32, hmax as f32, visible as f32) {
            actions.push(Act::SetFirst(v.round().max(0.0) as usize));
        }
        if vbar {
            let vrect = Rect::from_min_max(Pos2::new(plot_right, client.top()), Pos2::new(client.right(), plot_bottom));
            if let Some(v) = scrollbar(ui, &painter, vrect, false, -vscroll, -vmin, client.height() - band) {
                actions.push(Act::SetVScroll(-v));
            }
        }

        // ---- input (`0x45dc10`, `0x45ea70`, `0x45e6b0`, `0x45efa0`) ----
        let geo = RowGeo { left, axis_top: plot_bottom, tops: &tops, rows: &rows };
        let (shift, ctrl, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.command || i.modifiers.ctrl, i.modifiers.alt));
        let v_hit = |x: f32| (0..3).find(|&k| viz[k].1 && (x_of(viz[k].0 as f64) - x).abs() < 3.0);
        let h_hit = |y: f32| (0..2).find(|&j| doc.hviz[j].shown && hrows[j].is_some_and(|(hy, _)| (hy - y).abs() < 3.0));
        let comment_at = |pos: Pos2| doc.comments.iter().position(|c| (x_of(c.sample as f64) - pos.x).abs() < 5.0);
        let (pressed_l, pressed_r, press_pos) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.secondary_pressed(), i.pointer.press_origin()));
        let in_view = |p: Pos2| client.contains(p) && p.x < plot_right;
        if !menu_busy && (pressed_l || pressed_r) {
            if let Some(pos) = press_pos.filter(|p| in_view(*p) && resp.hovered()) {
                match hit_test(pos, &geo) {
                    Hit::Axis => actions.push(Act::ClearSelection),
                    Hit::Stats(row) => {
                        let how = if pressed_r {
                            SelClick::Right
                        } else if shift {
                            SelClick::Shift
                        } else if ctrl {
                            SelClick::Ctrl
                        } else {
                            SelClick::Plain
                        };
                        match row {
                            Some(k) => actions.push(Act::Select(k, how)),
                            None if !(ctrl || pressed_r) => actions.push(Act::ClearSelection),
                            None => {}
                        }
                    }
                    Hit::Plot => {
                        let k = sample_at(pos.x);
                        if pressed_l && v_hit(pos.x).is_some() {
                            actions.push(Act::Drag(VizDrag::V(v_hit(pos.x).unwrap())));
                        } else if pressed_l && h_hit(pos.y).is_some() {
                            actions.push(Act::Drag(VizDrag::H(h_hit(pos.y).unwrap())));
                        } else if comment_at(pos).is_some() {
                            // Comment drag / menu (on release).
                        } else if pressed_l && doc.comment_mode == 1 {
                            actions.push(Act::PlaceMoment(k));
                        } else if shift {
                            if let Some((ch, v)) = analog_at(pos.y, &geo, &ranges) {
                                actions.push(Act::SetH(usize::from(pressed_r), ch, v));
                            }
                        } else {
                            actions.push(Act::PlaceV(usize::from(pressed_r), k));
                        }
                    }
                }
            }
        }
        if let (Some(drag), Some(pos)) = (self.vizir_drag, ui.input(|i| i.pointer.interact_pos())) {
            if ui.input(|i| i.pointer.primary_down()) {
                match drag {
                    VizDrag::V(k) => actions.push(Act::MoveV(k, sample_at(pos.x.clamp(left, plot_right)))),
                    VizDrag::H(j) => {
                        if let Some((ch, v)) = analog_at(pos.y.clamp(client.top(), plot_bottom - 1.0), &geo, &ranges) {
                            actions.push(Act::SetH(j, ch, v));
                        }
                    }
                }
            } else {
                actions.push(Act::EndDrag);
            }
        }
        if !menu_busy && resp.double_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                match hit_test(pos, &geo) {
                    Hit::Plot => {
                        if let Some(idx) = comment_at(pos) {
                            actions.push(Act::EditComment(idx));
                        }
                    }
                    // Double-click on an analog name badge: inline rename.
                    Hit::Stats(Some(Lane::Analog(i))) => {
                        let li = rows.iter().position(|r| r.key == Lane::Analog(i)).unwrap_or(0);
                        if pos.y < tops[li] + 22.0 {
                            actions.push(Act::Rename(i));
                        }
                    }
                    _ => {}
                }
            }
        }
        if !menu_busy && resp.secondary_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                match hit_test(pos, &geo) {
                    Hit::Axis => actions.push(Act::OpenMenu(pos, LaneMenuKind::TimeAxis)),
                    Hit::Stats(_) => actions.push(Act::OpenMenu(pos, LaneMenuKind::Channels)),
                    Hit::Plot => {
                        if let Some(idx) = comment_at(pos) {
                            actions.push(Act::OpenMenu(pos, LaneMenuKind::Comment(idx)));
                        }
                    }
                }
            }
        }
        // Wheel (`0x45d990`): Alt = lane height, exactly Ctrl = time zoom,
        // exactly Shift = one screen sideways, else scroll the lanes.
        if resp.hovered() && !menu_busy {
            let raw = ui.input(|i| i.raw_scroll_delta);
            let line = ui.ctx().options(|o| o.line_scroll_speed);
            let delta = raw.x + raw.y;
            if delta != 0.0 {
                let mag = ((delta.abs() / line).round() as i32).clamp(1, 10);
                let notches = if delta > 0.0 { mag } else { -mag };
                if alt {
                    actions.push(Act::Cmd(Command::ZoomY(notches > 0)));
                } else if ctrl && !shift {
                    actions.push(Act::Cmd(Command::ZoomX(notches > 0)));
                } else if shift && !ctrl {
                    let v = first as i64 - i64::from(notches) * visible.max(1) as i64;
                    actions.push(Act::SetFirst(v.clamp(0, hmax as i64) as usize));
                } else {
                    actions.push(Act::SetVScroll(vscroll + notches as f32 * g::WHEEL_NOTCH_PX));
                }
            }
        }
        // Keys (`0x45f490`): ←/→ move В1 (Ctrl: В2) a sample, ↑/↓ move ГВ1
        // (Ctrl: ГВ2) a pixel.
        if !ui.ctx().wants_keyboard_input() && self.rename_edit.is_none() {
            let (l, r, u, d) = ui.input(|i| {
                (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown))
            });
            let vk = usize::from(ctrl);
            if l || r {
                let cur = viz[vk].0 as i64 + if r { 1 } else { -1 };
                actions.push(Act::MoveV(vk, cur.clamp(0, n.max(1) as i64 - 1) as usize));
            }
            if u || d {
                if let Some((hy, _)) = hrows[vk] {
                    if let Some((ch, v)) = analog_at(hy + if d { 1.0 } else { -1.0 }, &geo, &ranges) {
                        actions.push(Act::SetH(vk, ch, v));
                    }
                }
            }
        }
        // Cursor shapes (`0x45f660`).
        if let Some(pos) = hover.filter(|p| p.x > left && p.y < plot_bottom) {
            match self.vizir_drag {
                Some(VizDrag::V(_)) => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal),
                Some(VizDrag::H(_)) => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical),
                None if v_hit(pos.x).is_some() || comment_at(pos).is_some() => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal),
                None if h_hit(pos.y).is_some() => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical),
                None => {}
            }
        }
        let row_keys: Vec<ChanKey> = rows.iter().map(|r| r.key).collect();
        let (vmin_c, hmax_c) = (vmin, hmax);

        for a in actions {
            let d = &mut self.docs[ai];
            match a {
                Act::PlaceV(k, s) => {
                    match k {
                        0 => d.main_cursor = s,
                        _ => d.aux_cursor = s,
                    }
                    d.v_shown[k] = true;
                }
                Act::MoveV(k, s) => match k {
                    0 => d.main_cursor = s,
                    1 => d.aux_cursor = s,
                    _ => d.v3_cursor = s,
                },
                Act::SetH(j, ch, v) => {
                    d.hviz[j] = crate::app::HVizir { shown: true, chan: Some(ch), value: v };
                }
                Act::Drag(k) => self.vizir_drag = Some(k),
                Act::EndDrag => self.vizir_drag = None,
                Act::SetFirst(v) => d.first = v.min(hmax_c),
                Act::SetVScroll(v) => d.vscroll = v.clamp(vmin_c, 0.0),
                Act::PlaceMoment(sample) => {
                    // 155: one click places the moment; the mode switches off.
                    d.comments.push(crate::app::Comment { sample, text: String::new(), level: None, hidden: false, locked: false });
                    d.comment_mode = 0;
                }
                Act::EditComment(idx) => self.open_comment_editor(ai, idx),
                Act::Rename(i) => self.rename_edit = Some((i, d.osc.analog[i].id.clone())),
                Act::Select(k, how) => {
                    let (mut sel, mut anchor) = (std::mem::take(&mut d.selection), d.sel_anchor);
                    select_click(&mut sel, &mut anchor, &row_keys, k, how);
                    d.selection = sel;
                    d.sel_anchor = anchor;
                }
                Act::ClearSelection => d.selection.clear(),
                Act::OpenMenu(pos, kind) => {
                    self.lane_menu_serial += 1;
                    self.lane_menu = Some(LaneMenu { serial: self.lane_menu_serial, pos, kind });
                }
                Act::Cmd(c) => self.dispatch(c),
            }
        }
        if let Some(d) = self.docs.get_mut(ai) {
            d.first = d.first.min(hmax_c);
            d.vscroll = d.vscroll.clamp(vmin_c, 0.0);
        }

        // Hover popup over a cut name (`0x469060`): the full text.
        if let Some(full) = cut_hover {
            egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new("fv_cut_name"), |ui| ui.label(full));
        }
        self.rename_ui(ui, ai, rename_rect);
        self.lane_menu_ui(ui.ctx(), ai);
        if self.lane_menu_eat && !ui.input(|i| i.pointer.any_down()) {
            self.lane_menu_eat = false;
        }
    }

    /// The inline channel-rename edit over the name badge (`0x465890`):
    /// Enter or a click elsewhere commits, Esc cancels; a name in use is
    /// refused with message 397.
    fn rename_ui(&mut self, ui: &mut egui::Ui, ai: usize, at: Option<Rect>) {
        let Some((i, mut text)) = self.rename_edit.take() else { return };
        let Some(at) = at else { return };
        let mut done: Option<bool> = None;
        egui::Area::new(egui::Id::new("fv_rename").with(i)).order(egui::Order::Foreground).fixed_pos(at.min).show(ui.ctx(), |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(at.width().max(120.0)));
            r.request_focus();
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some(false);
            } else if r.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = Some(true);
            }
        });
        match done {
            None => self.rename_edit = Some((i, text)),
            Some(false) => {}
            Some(true) => {
                let name = text.trim().to_string();
                let d = &mut self.docs[ai];
                let taken = d.osc.analog.iter().enumerate().any(|(j, c)| j != i && c.id == name) || d.computed.iter().any(|c| c.name == name);
                if taken {
                    self.status = "Данное имя канала уже используется.".into();
                } else if !name.is_empty() {
                    d.osc.analog[i].id = name;
                }
            }
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

    /// The six status-bar panes (`0x4947d0`), each one complete string:
    /// the caption alone while its визир(s) are hidden, else the value
    /// relative to the record start (COMTRADE carries no trigger sample) in
    /// the ruler unit; the V difference needs both V визиры shown, the H
    /// difference both H визиры on the same channel.
    pub fn cursor_status(&self) -> [String; 6] {
        let captions = ["Верт. визир 1:", "Верт. визир 2:", "Разница. верт. визиров:", "Гор. визир 1:", "Гор. визир 2:", "Разница г. визиров:"];
        let mut out = captions.map(String::from);
        let Some(d) = self.active_doc() else { return out };
        let t0 = d.osc.time.first().copied().unwrap_or(0.0);
        let t = |k: usize| d.osc.time.get(k).copied().unwrap_or(0.0) - t0;
        let time = |dt: f64, k: i64| match self.time_unit {
            TimeUnit::Micro => format!("{} мкс", (dt * 1e6).round() as i64),
            TimeUnit::Milli => format!("{} мс", (dt * 1e3).round() as i64),
            TimeUnit::Sec => format!("{dt:.3} сек."),
            TimeUnit::Samples => format!("{k}"),
        };
        let (v1, v2) = (d.main_cursor, d.aux_cursor);
        if d.v_shown[0] {
            out[0] = format!("{} {}", captions[0], time(t(v1), v1 as i64 + 1));
        }
        if d.v_shown[1] {
            out[1] = format!("{} {}", captions[1], time(t(v2), v2 as i64 + 1));
        }
        if d.v_shown[0] && d.v_shown[1] {
            out[2] = format!("{} {}", captions[2], time((t(v1) - t(v2)).abs(), (v1 as i64 - v2 as i64).abs()));
        }
        let hval = |j: usize| -> Option<(usize, f64)> {
            let h = &d.hviz[j];
            (h.shown && h.value.is_finite()).then_some(())?;
            Some((h.chan?, h.value))
        };
        let unit_of = |ch: usize| analog_unit(d, ch);
        for j in 0..2 {
            if let Some((ch, v)) = hval(j) {
                out[3 + j] = format!("{} {}", captions[3 + j], fvb_core::views::readout_text(v, &unit_of(ch)).trim());
            }
        }
        if let (Some((c1, a)), Some((c2, b))) = (hval(0), hval(1)) {
            if c1 == c2 {
                out[5] = format!("{} {}", captions[5], fvb_core::views::readout_text((a - b).abs(), &unit_of(c1)).trim());
            }
        }
        out
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
            self.frame_mod(events, Default::default());
        }

        fn frame_mod(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
            self.t += 0.05;
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
                time: Some(self.t),
                events,
                modifiers,
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

        /// Centre of stats-panel row `li`, from the layout `lanes_ui` uses
        /// (central panel margin 8 px, no scroll).
        fn row(&self, li: usize) -> Pos2 {
            let rows = lane_layout(self.app.active_doc().unwrap(), self.app.y_analog, false);
            let top: f32 = 8.0 + rows[..li].iter().map(|r| r.h).sum::<f32>();
            Pos2::new(40.0, top + rows[li].h / 2.0)
        }

        /// Bottom of the plot (top of the ruler / difference band).
        fn plot_bottom(&self) -> f32 {
            let band = g::RULER_H + if self.app.active_doc().unwrap().diff_band { g::DIFF_BAND_H } else { 0.0 };
            SCREEN.y - 8.0 - HSCROLL_H - band
        }

        /// Rows whose centre is above the bottom band.
        fn rows(&self) -> usize {
            let n = lane_layout(self.app.active_doc().unwrap(), self.app.y_analog, false).len();
            (0..n).take_while(|&li| self.row(li).y < self.plot_bottom()).count()
        }

        fn wheel(&mut self, at: Pos2, notches: f32, modifiers: egui::Modifiers) {
            let ev = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: Vec2::new(0.0, notches), modifiers };
            self.frame_mod(vec![egui::Event::PointerMoved(at), ev], modifiers);
            self.frame(vec![]);
        }

        fn doc(&self) -> &crate::app::Doc {
            self.app.active_doc().unwrap()
        }
    }

    #[test]
    fn hit_test_reaches_the_top_row_and_the_axis_band() {
        let rows = [
            Row { key: ChanKey::Analog(0), members: vec![0], h: 93.0 },
            Row { key: ChanKey::Digital(0), members: vec![], h: 20.0 },
        ];
        let geo = RowGeo { left: 158.0, axis_top: 200.0, tops: &[0.0, 93.0], rows: &rows };
        assert_eq!(hit_test(Pos2::new(10.0, 0.5), &geo), Hit::Stats(Some(ChanKey::Analog(0))));
        assert_eq!(hit_test(Pos2::new(10.0, 100.0), &geo), Hit::Stats(Some(ChanKey::Digital(0))));
        assert_eq!(hit_test(Pos2::new(10.0, 150.0), &geo), Hit::Stats(None));
        assert_eq!(hit_test(Pos2::new(300.0, 30.0), &geo), Hit::Plot);
        assert_eq!(hit_test(Pos2::new(300.0, 210.0), &geo), Hit::Axis);
    }

    #[test]
    fn lanes_are_fixed_height_analog_93_digital_20() {
        let h = Harness::new();
        let rows = lane_layout(h.doc(), h.app.y_analog, false);
        for r in &rows {
            let want = if matches!(r.key, ChanKey::Digital(_)) { 20.0 } else { 93.0 };
            assert_eq!(r.h, want, "{:?}", r.key);
        }
    }

    /// The left button places В1 (shown) and a drag does not pan the plot;
    /// grabbing В1 within 3 px drags it.
    #[test]
    fn left_press_places_v1_and_grabbing_it_drags_without_panning() {
        let mut h = Harness::new();
        assert!(!h.doc().v_shown[0], "a new document shows no визир");
        let y = h.row(0).y;
        h.button(Pos2::new(500.0, y), egui::PointerButton::Primary);
        assert!(h.doc().v_shown[0]);
        let (v1, first) = (h.doc().main_cursor, h.doc().first);
        // Drag from 2 px off the визир to 80 px further right.
        let at = Pos2::new(502.0, y);
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(at), ev(at, true)]);
        for dx in [20.0, 40.0, 60.0, 80.0] {
            h.frame(vec![egui::Event::PointerMoved(at + Vec2::new(dx, 0.0))]);
        }
        h.frame(vec![ev(at + Vec2::new(80.0, 0.0), false)]);
        h.frame(vec![]);
        assert!(h.doc().main_cursor > v1, "В1 followed the drag");
        assert_eq!(h.doc().first, first, "the plot never pans on drag");
        assert!(h.app.vizir_drag.is_none());
    }

    #[test]
    fn wheel_modifiers_zoom_time_resize_lanes_and_scroll() {
        let mut h = Harness::new();
        let at = Pos2::new(600.0, h.row(0).y);
        h.button(at, egui::PointerButton::Primary);
        let f0 = h.doc().xscale.unwrap();
        h.wheel(at, 1.0, egui::Modifiers::CTRL);
        assert_eq!(h.doc().xscale.unwrap(), f0 * 2.0, "Ctrl+wheel: ×2");
        h.wheel(at, -1.0, egui::Modifiers::CTRL);
        assert_eq!(h.doc().xscale.unwrap(), f0);
        h.wheel(at, 1.0, egui::Modifiers::ALT);
        assert_eq!(h.app.y_analog, 121.0, "Alt+wheel: lane height ×1.3");
        h.wheel(at, -1.0, egui::Modifiers::NONE);
        assert!(h.doc().vscroll < 0.0, "plain wheel scrolls the lanes");
        // Shift+wheel: one screen sideways (zoomed in so there is room).
        h.wheel(at, 1.0, egui::Modifiers::CTRL);
        h.wheel(at, 1.0, egui::Modifiers::CTRL);
        let first = h.doc().first;
        h.wheel(at, -1.0, egui::Modifiers::SHIFT);
        assert!(h.doc().first > first, "Shift+wheel scrolls right");
    }

    #[test]
    fn stats_rows_toggle_on_all_channels_and_grow_the_lane() {
        let mut h = Harness::new();
        assert!(h.doc().chans.iter().all(|c| !c.stats.minmax && c.stats.rms && c.stats.a1));
        h.app.dispatch(Command::StatsRow(0));
        assert!(h.doc().chans.iter().all(|c| c.stats.minmax), "32817 sets Max/Min on all");
        h.app.dispatch(Command::StatsRow(1));
        assert!(h.doc().chans.iter().all(|c| !c.stats.rms), "all had СКЗ: cleared on all");
        h.app.dispatch(Command::StatsRow(1));
        let d = h.app.active_doc_mut().unwrap();
        d.chans[0].invert = true;
        // name row + 10 + Max, Min, СКЗ, A1 + the icon row = 15 + 10 + 60 + 23.
        let rows = lane_layout(h.doc(), h.app.y_analog, false);
        assert_eq!(rows[0].h, 108.0);
        assert_eq!(rows[1].h, 93.0, "no icons: YAnalogSize");
    }

    #[test]
    fn stats_panel_width_fits_the_longest_name_cut_to_25() {
        let mut h = Harness::new();
        let w0 = h.app.view_left;
        h.app.active_doc_mut().unwrap().osc.analog[0].id = "Ш".repeat(25);
        h.frame(vec![]);
        let w25 = h.app.view_left;
        assert!(w25 > w0, "{w25} > {w0}");
        h.app.active_doc_mut().unwrap().osc.analog[0].id = "Ш".repeat(40);
        h.frame(vec![]);
        assert_eq!(h.app.view_left, w25, "names are cut to 25 characters");
    }

    #[test]
    fn arrow_keys_move_v1_and_ctrl_v2_by_one_sample() {
        let mut h = Harness::new();
        let y = h.row(0).y;
        h.button(Pos2::new(500.0, y), egui::PointerButton::Primary);
        h.button(Pos2::new(700.0, y), egui::PointerButton::Secondary);
        let (v1, v2) = (h.doc().main_cursor, h.doc().aux_cursor);
        let key = |k, m: egui::Modifiers| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: m };
        h.frame_mod(vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)], egui::Modifiers::NONE);
        assert_eq!(h.doc().main_cursor, v1 + 1);
        h.frame_mod(vec![key(egui::Key::ArrowLeft, egui::Modifiers::CTRL)], egui::Modifiers::CTRL);
        assert_eq!(h.doc().aux_cursor, v2 - 1);
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
        let bottom = SCREEN.y - 8.0 - HSCROLL_H - 10.0;
        let last = h.row(h.rows() - 1);
        for p in [Pos2::new(600.0, bottom), last, Pos2::new(40.0, h.plot_bottom() - 2.0)] {
            h.button(p, egui::PointerButton::Secondary);
            assert!(h.app.lane_menu.is_some(), "no menu at {p:?} (rows {}, geometry {:?})", h.rows(), h.plot_bottom());
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
        assert!(!h.doc().v_shown[1]);
        h.button(Pos2::new(700.0, h.row(0).y), egui::PointerButton::Secondary);
        assert!(h.app.lane_menu.is_none());
        assert!(h.doc().v_shown[1] && h.doc().aux_cursor > 0, "В2 placed and shown");
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
