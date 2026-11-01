//! The lane-based oscillogram view — the original's primary document window.
//!
//! Each analog/computed/digital channel is a horizontal lane with a left stats
//! panel (name badge · value@cursor · Max/Min/СКЗ/A1) and a waveform. Two
//! vertical визиры (red main, blue aux) with a shaded selection between them,
//! a shared time axis, on-waveform comments, and the original's context menus
//! chosen by click region (MENU 560 axis band, 561 stats panel acting on the
//! selected channels, 563 on a comment — ledger L9.1). Lanes have a minimum
//! height and the view scrolls vertically when there are many channels.

use crate::app::{App, ChanKey, Command, CommentEdit, RectZoom, TimeUnit, ValueMode};
use crate::oscgeom::{self as g, PixelLines};
use crate::chansel::{select_click, ChanFlag, ItemState, SelClick};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

/// Stats-panel width before the first measurement (`0x4606d0` default).
pub const LEFT_W_DEFAULT: f32 = 125.0;
/// Names are cut to this many characters in the stats panel.
const NAME_CHARS: usize = 25;
/// Name / value font (Tahoma 15 bold / heavy in the original) and the row
/// font (Tahoma 13); `ROW_H` is the name font's line height ("Ay").
/// The lane names and values: Tahoma Bold 12 px (measured on the VM with a
/// typical lane name and a milliamp value).
const NAME_FONT: FontId = FontId::proportional(12.0);
const ROW_FONT: FontId = FontId::proportional(11.0);
const ROW_H: f32 = 15.0;
/// Horizontal scrollbar height below the view and vertical scrollbar width:
/// the view's window scroll bars, at the default system metric (96 dpi).
#[cfg(test)]
const HSCROLL_H: f32 = 17.0;
#[cfg(test)]
const VSCROLL_W: f32 = 17.0;

/// The view's window scroll bars at the system DPI: SM_CXVSCROLL / SM_CYHSCROLL
/// grow for a system-DPI-aware window: MulDiv(17, dpi, 96), the metric graph-scaling measured on clones
/// (17, 21, 26, 30, 34 px at 100, 125, 150, 175, 200 %; `oscgeom::WinMetrics`); what the view paints in them
/// stays its own.
pub(crate) fn scroll_w() -> f32 {
    crate::oscgeom::WinMetrics::at(crate::dpi::dpi()).scroll
}

fn scroll_h() -> f32 {
    scroll_w()
}
/// The V scroll bar's line step (`0x45d6d0`: SB_LINEUP/DOWN = 100 px).
const VSCROLL_LINE: f32 = 100.0;
/// A group member's stats line height (`view+0x88`: "Ay" in the row font,
/// Tahoma 13 — ours scaled like `ROW_H`).
const GROUP_STATS_H: f32 = 13.0;
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

/// One lane of the view: its row (an analog-kind lane carries all members
/// of its channel group in channel order, the first being the row's own
/// channel) and its height.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: ChanKey,
    pub members: Vec<ChanKey>,
    pub h: f32,
}

impl Row {
    fn selected(&self, doc: &crate::app::Doc) -> bool {
        doc.selection.contains(&self.key) || self.members.iter().any(|m| doc.selection.contains(m))
    }

    /// Is channel `k` drawn in this lane (its own or a group member)?
    pub fn has(&self, k: ChanKey) -> bool {
        self.key == k || self.members.contains(&k)
    }
}

/// Lane order and heights (`0x467ed0`, `0x460290`): visible analog channels
/// (a group's members pulled together at its first member), computed
/// channels, then digital channels; pending pasted lanes sit at their
/// insertion point (`Computed::before`). Analog lanes `YAnalogSize` high (a
/// group at least one name row per member), digital lanes 20 px. No stretch.
pub fn lane_layout(doc: &crate::app::Doc, y_analog: f32) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let mut groups: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    // A visible analog-kind channel: its own lane, or a member of its
    // group's lane (at the first visible member; formula channels too).
    let push = |key: Lane, h: f32, rows: &mut Vec<Row>, groups: &mut std::collections::HashMap<u32, usize>| {
        if let Some(gid) = doc.group_of(key) {
            if let Some(&ri) = groups.get(&gid) {
                rows[ri].members.push(key);
                return;
            }
            groups.insert(gid, rows.len());
        }
        rows.push(Row { key, members: vec![key], h });
    };
    let pending_at = |key: Option<ChanKey>, rows: &mut Vec<Row>, groups: &mut std::collections::HashMap<u32, usize>| {
        for j in (0..doc.computed.len()).filter(|&j| doc.computed[j].visible && doc.computed[j].before == key) {
            push(Lane::Computed(j), g::DIGITAL_H, rows, groups);
        }
    };
    for k in doc.kind0_order() {
        let visible = match k {
            Lane::Analog(i) => doc.chans[i].visible,
            Lane::Computed(j) => doc.computed[j].visible,
            Lane::Digital(_) => false,
        };
        if visible {
            push(k, y_analog, &mut rows, &mut groups);
        }
    }
    for i in 0..=doc.osc.digital.len() {
        pending_at(Some(Lane::Digital(i)), &mut rows, &mut groups);
        if i < doc.osc.digital.len() && doc.digital_visible.get(i).copied().unwrap_or(true) {
            rows.push(Row { key: Lane::Digital(i), members: Vec::new(), h: g::DIGITAL_H });
        }
    }
    // need = name row + 10 + one row per enabled stat (Max/Min two) + 23
    // when any status icon shows; a group lane one name row (+33) per member.
    for r in rows.iter_mut().filter(|r| !matches!(r.key, Lane::Digital(_)) && !matches!(r.key, Lane::Computed(j) if doc.pending_digital(j))) {
        let icons = if status_icons(doc, r).is_empty() { 0.0 } else { 23.0 };
        let need = if r.members.len() > 1 {
            // `0x460290`: N × the owner's stride (each member is painted
            // with its own — the original's mismatch, kept).
            r.members.len() as f32 * group_stride(doc, r.members[0])
        } else {
            ROW_H + 10.0 + stat_rows(doc, &r.key).map_or(0, |s| s.count()) as f32 * ROW_H + icons
        };
        r.h = r.h.max(need);
    }
    rows
}

/// The geometry and settings one paint of the oscillogram view needs; the
/// screen (`lanes_ui`) and the printout (`printout`) paint through it.
pub struct OscView<'a> {
    pub doc: &'a crate::app::Doc,
    pub unit: TimeUnit,
    /// Horizontal scale, first drawn sample, sample count.
    pub f: f64,
    pub first: usize,
    pub n: usize,
    /// Plot left edge (the stats panel's right edge), plot right edge, the
    /// view's left edge and the stats label column width.
    pub left: f32,
    pub right: f32,
    pub x0: f32,
    pub label_col: f32,
}

impl OscView<'_> {
    pub fn x_of(&self, k: f64) -> f32 {
        g::x_of(k, self.first, self.f, self.left)
    }

    fn time_at(&self, k: usize) -> f64 {
        let t = &self.doc.osc.time;
        t.get(k).copied().unwrap_or(0.0) - t.first().copied().unwrap_or(0.0)
    }

    /// A ruler / band label's time and sample: relative to the trigger
    /// sample (`doc+0x214`; 0 for COMTRADE) — the tick's µs is
    /// t(k) − t(trigger) for every unit (`0x4649cf`–`0x464a0d`), so
    /// «Абсолютное время» is the start's time of day plus it (a quirk with
    /// an `.osc` trigger); samples print k − trigger + 1.
    /// Past the record's end the times run on at its rate (`0x454b40`):
    /// the ruler labels the ticks out to the plot's edge (VM, L9.398).
    fn label_at(&self, k: usize) -> (f64, i64) {
        let (osc, trig) = (&self.doc.osc, self.doc.osc.trigger_sample);
        let us = |i| g::time_us(&osc.time, osc.rate_r, i);
        ((us(k) - us(trig)) / 1e6, k as i64 - trig as i64)
    }

    /// One lane's stats text (`0x461440`) and waveforms (`0x45fbe0`);
    /// returns the name badge rectangle.
    pub fn row(&self, lp: &egui::Painter, pp: &egui::Painter, r: &Row, top: f32, (lo, hi): (f64, f64)) -> Rect {
        let (doc, left, plot_right) = (self.doc, self.left, self.right);
        let (first, n, f) = (self.first, self.n, self.f);
        let h = r.h;
        let k_main = doc.main_cursor;
        let x_of = |k: f64| self.x_of(k);
        let lane = &r.key;
        let (name, unit, color, maxv, minv, rms, a1, val) = lane_stats(doc, lane, k_main);
        // Stats text: the name (cut to 25 characters) framed in its colour,
        // the value at В1 after the label column, then the enabled rows;
        // digital lanes show the name only.
        let col_x = self.x0 + self.label_col + 5.0;
        let header = |y: f32, name: &str, unit: &fvb_core::units::Shown, color: Color32, val: f64| {
            let short: String = name.chars().take(NAME_CHARS).collect();
            let name_rect = bold_text(lp, Pos2::new(self.x0 + 5.0, y), &short, NAME_FONT, color);
            lp.px_rect_stroke(
                Rect::from_min_max(Pos2::new(self.x0 + 3.0, y), Pos2::new(name_rect.right() + 2.0, name_rect.bottom())),
                0.0,
                Stroke::new(1.0f32, Color32::from_gray(128)),
            );
            if let Some(t) = fvb_core::views::stats_value_text(val, unit) {
                bold_text(lp, Pos2::new(col_x, y), &t, NAME_FONT, Color32::from_rgb(69, 85, 199));
            }
            name_rect
        };
        // Status icons, right to left from the panel edge, tops at `y`.
        let icons = |glyphs: Vec<&str>, y: f32| {
            for (j, glyph) in glyphs.into_iter().enumerate() {
                // Image list 489's look (seen on the VM): a 16 px box, a
                // 1 px (142, 140, 99) frame on a light lavender ground, the
                // glyph in blue — our own glyphs (the bitmap is the
                // original's).
                let at = Rect::from_min_size(Pos2::new(left - 21.0 - 21.0 * j as f32, y), Vec2::splat(16.0));
                lp.rect_filled(at, 0.0, Color32::from_rgb(243, 238, 246));
                lp.px_rect_stroke(at, 0.0, Stroke::new(1.0f32, Color32::from_rgb(142, 140, 99)));
                lp.text(at.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(9.0), Color32::from_rgb(80, 80, 200));
            }
        };
        let row_col = Color32::from_rgb(0, 0, 127);
        let digital_row = matches!(lane, Lane::Digital(_)) || matches!(lane, Lane::Computed(j) if doc.pending_digital(*j));
        let name_rect = if digital_row {
            let short: String = name.chars().take(NAME_CHARS).collect();
            // An inverted digital channel's icon at (leftW − 18, y + 2)
            // (`0x462e88`).
            if let Lane::Digital(i) = lane {
                if doc.dig(*i).invert {
                    let at = Rect::from_min_size(Pos2::new(left - 18.0, top + 2.0), Vec2::splat(16.0));
                    lp.rect_filled(at, 0.0, Color32::from_rgb(243, 238, 246));
                    lp.px_rect_stroke(at, 0.0, Stroke::new(1.0f32, Color32::from_rgb(142, 140, 99)));
                    lp.text(at.center(), Align2::CENTER_CENTER, "−", FontId::proportional(9.0), Color32::from_rgb(80, 80, 200));
                }
            }
            bold_text(lp, Pos2::new(self.x0 + 5.0, top + 2.0), &short, NAME_FONT, color)
        } else if r.members.len() > 1 {
            // A channel group (`0x461440`): every member in channel order
            // gets the full header at top + 5 + k·S (S of that member, not
            // a running sum), one `|`-joined stats line under it when it
            // has stat rows, and its own icons ending at top + (k+1)·S.
            let mut first = None;
            for (k, &m) in r.members.iter().enumerate() {
                let (mn, mu, mc, mmax, mmin, mrms, ma1, mv) = lane_stats(doc, &m, k_main);
                let stride = group_stride(doc, m);
                let y = top + 5.0 + k as f32 * stride;
                let nr = header(y, &mn, &mu, mc, mv);
                first.get_or_insert(nr);
                let st = stat_rows(doc, &m).unwrap_or_default();
                if st.count() > 0 {
                    let line = fvb_core::views::group_stats_line(st.minmax.then_some((mmax, mmin)), st.rms.then_some(mrms), st.a1.then_some(ma1), &mu);
                    let band = Rect::from_min_size(Pos2::new(self.x0 + 5.0, top + 8.0 + k as f32 * stride + GROUP_STATS_H), Vec2::new(left - self.x0 - 5.0, GROUP_STATS_H));
                    lp.with_clip_rect(band.intersect(lp.clip_rect())).text(band.left_center(), Align2::LEFT_CENTER, line, ROW_FONT, row_col);
                }
                icons(key_icons(doc, m), top - 21.0 + (k + 1) as f32 * stride);
            }
            first.expect("a group has members")
        } else {
            let name_rect = header(top + 5.0, &name, &unit, color, val);
            // The rows at top + 8 + i · hS, i from 1, hS the 13 px row font's
            // height (`0x4621fe`, `0x4627d6`; seen on the VM: СКЗ at top + 21,
            // A1 at top + 34).
            let mut yy = top + 8.0 + GROUP_STATS_H;
            if let Some(st) = stat_rows(doc, lane) {
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
                    lp.text(Pos2::new(self.x0 + 5.0, yy), Align2::LEFT_TOP, lbl, ROW_FONT, row_col);
                    lp.text(Pos2::new(col_x, yy), Align2::LEFT_TOP, fvb_core::views::readout_text(v, &unit), ROW_FONT, row_col);
                    yy += GROUP_STATS_H;
                }
            }
            icons(status_icons(doc, r), top + h - 21.0);
            name_rect
        };

        // Waveforms: every visible sample, 1 px, sample marks above 4 px per
        // sample.
        match lane {
            Lane::Digital(i) => {
                let s = &doc.osc.digital[*i].samples;
                let mid = top + 10.0;
                // The base bar (`0x463e9a`): N·25f from the plot left,
                // whatever the scroll position.
                let end_x = (left + g::base_bar_len(s.len(), f, doc.lane_ratio(*lane))).min(plot_right);
                pp.rect_filled(Rect::from_min_max(Pos2::new(left, mid - 2.0), Pos2::new(end_x, mid + 2.0)), 0.0, Color32::from_rgb(150, 150, 255));
                let fill = if r.selected(doc) { Color32::from_rgb(253, 219, 102) } else { digital_fill(doc.dig(*i).omp) };
                let red = Color32::from_rgb(200, 0, 0);
                // The painter's runs and transition ticks (`0x42a0f0`), x
                // from leftW, GDI rectangles mid−6..mid+6.
                // The display cache: inverted with the flag (`0x428490`).
                let on = |k: usize| doc.dig_on(*i, k);
                let shapes = g::digital_shapes(&on, s.len(), first, f, plot_right - left);
                for (a, b) in shapes.runs {
                    let rr = Rect::from_min_max(Pos2::new(left + a, mid - 6.0), Pos2::new(left + b.max(a + 1.0), mid + 6.0));
                    pp.rect_filled(rr, 0.0, fill);
                    pp.px_rect_stroke(rr, 0.0, Stroke::new(1.0f32, red));
                }
                for t in shapes.ticks {
                    pp.rect_filled(Rect::from_min_max(Pos2::new(left + t, mid - 6.0), Pos2::new(left + t + 1.0, mid + 6.0)), 0.0, red);
                }
            }
            Lane::Computed(j) if doc.pending_digital(*j) => self.pasted_digital(pp, *j, top, r.selected(doc)),
            _ => {
                // A window-zoomed channel is drawn through a clip region of
                // its lane, (0, top + 5)–(W, top + h) (`0x4295a5`,
                // `0x429cb6`): values outside the window are cut, never
                // clamped. Unzoomed channels need none.
                let zoomed = |m: ChanKey| matches!(m, Lane::Analog(i) if doc.chans.get(i).is_some_and(|c| c.window.is_some()));
                let lane_clip = pp.with_clip_rect(pp.clip_rect().intersect(Rect::from_min_max(Pos2::new(left, top + 5.0), Pos2::new(plot_right, top + h))));
                let painter_for = |m: Option<ChanKey>| if m.is_some_and(zoomed) { &lane_clip } else { pp };
                // A group's curves in member-set order (`0x4311e0`); the
                // first draws the zero line.
                let order = doc.group_paint_order(&r.members);
                if lo <= 0.0 && 0.0 <= hi {
                    let zy = g::value_y(0.0, top, h, lo, hi);
                    painter_for(order.first().copied()).px_hline(left..=plot_right, zy, Stroke::new(1.0f32, Color32::from_rgb(200, 200, 200)));
                }
                // Offsets 0..min(⌈W/(k·R/r)⌉, len − first) (`0x4295fc`,
                // `0x429d24`), R the record's integer rate and r the lane's
                // own rate, if it has one.
                let last_of = |m: ChanKey| g::drawn_last(n, first, f, plot_right - left, doc.lane_ratio(m)).unwrap_or(first.saturating_sub(1));
                for &m in &order {
                    let last = last_of(m);
                    match m {
                        Lane::Analog(i) => {
                            let c = doc.chans[i].color;
                            draw_series(painter_for(Some(m)), (first, last), &|k| mode_disp(doc, i, k), &x_of, top, h, (lo, hi), c, f);
                        }
                        Lane::Computed(j) if doc.pending_of(j).is_some() => self.pasted_analog(pp, j, top, h, (lo, hi), doc.computed[j].color),
                        Lane::Computed(j) => {
                            // The display cache (`0x429490` reads ch+0x2c).
                            let c = &doc.computed[j];
                            draw_series(pp, (first, last.min(c.samples.len().saturating_sub(1))), &|k| c.shown(k), &x_of, top, h, (lo, hi), c.color, f);
                        }
                        Lane::Digital(_) => {}
                    }
                }
            }
        }
        name_rect
    }

    /// The times of pending lane `j` against the record.
    fn pasted_times(&self, j: usize) -> Option<(g::PastedTimes<'_>, &crate::merge_ui::ClipChan)> {
        self.doc.pasted_times(j)
    }

    /// A pasted analog channel (`0x429490`, bit 14): icon 0 at (leftW + 2,
    /// top + h − 21) first, then its own samples k..min(N, ⌈W/(25f·R/r)⌉ + k)
    /// at their times (`PastedTimes::xs`), marks above 4 px per sample.
    fn pasted_analog(&self, pp: &egui::Painter, j: usize, top: f32, h: f32, range: (f64, f64), color: Color32) {
        let Some((t, p)) = self.pasted_times(j) else { return };
        let Ok(a) = &p.pasted.channel else { return };
        paste_icon(pp, Pos2::new(self.left + 2.0, top + h - 21.0), true);
        let n = a.samples.len();
        let k = t.first(self.first);
        let end = t.analog_end(n, k, self.f, self.right - self.left);
        if end <= k {
            return;
        }
        let xs = t.xs(k, end, self.first, self.f);
        // The display cache (flags and mode over its own samples).
        let c = &self.doc.computed[j];
        let pts: Vec<Pos2> = xs.iter().zip(k..end).map(|(&x, i)| Pos2::new(self.left + x, g::value_y(c.own_shown(i), top, h, range.0, range.1))).collect();
        pp.add(Shape::line(pts.clone(), Stroke::new(1.0f32, color)));
        if self.f > 0.16 {
            for q in &pts {
                sample_mark(pp, *q, color);
            }
        }
    }

    /// A pasted digital channel (`0x42a0f0`, bit 14): the base bar moved by
    /// the paste offset, the runs of its own samples at their times
    /// (`pasted_digital_runs`, no column OR-ing), then icon 1 at
    /// (leftW − 3, top + h − 19).
    fn pasted_digital(&self, pp: &egui::Painter, j: usize, top: f32, selected: bool) {
        let Some((t, p)) = self.pasted_times(j) else { return };
        let Err(d) = &p.pasted.channel else { return };
        let (left, mid) = (self.left, top + 10.0);
        let n = d.samples.len();
        let bar = g::base_bar_len(n, self.f, t.ratio()) + t.offset_px(self.f);
        pp.rect_filled(Rect::from_min_max(Pos2::new(left, mid - 2.0), Pos2::new((left + bar).min(self.right), mid + 2.0)), 0.0, Color32::from_rgb(150, 150, 255));
        let fill = if selected { Color32::from_rgb(253, 219, 102) } else { DIGITAL_FILL };
        let red = Color32::from_rgb(200, 0, 0);
        let k = t.first(self.first);
        let end = t.digital_end(n, self.first, k, self.f, self.right - self.left);
        let xs = t.xs(k, end, self.first, self.f);
        let own = &self.doc.computed[j].pending_own;
        let on = |i: usize| own.get(i).map_or(d.samples.get(i).copied().unwrap_or(false), |v| *v != 0.0);
        for (a, b) in g::pasted_digital_runs(&on, n, k, &xs) {
            let rr = Rect::from_min_max(Pos2::new(left + a, mid - 6.0), Pos2::new(left + b.max(a + 1.0), mid + 6.0));
            pp.rect_filled(rr, 0.0, fill);
            pp.px_rect_stroke(rr, 0.0, Stroke::new(1.0f32, red));
        }
        paste_icon(pp, Pos2::new(left - 3.0, top + g::DIGITAL_H - 19.0), false);
    }

    /// The time ruler (`0x464350`) with its bottom at `bottom`: a white
    /// rectangle `height` high (30, or 75 over the difference band — VM)
    /// with a black top line across, black ticks every 25 px from 30 px up,
    /// labels every 100 px; in absolute mode the start date under the stats
    /// panel.
    pub fn ruler(&self, painter: &egui::Painter, bottom: f32, client_right: f32, height: f32) {
        let (doc, left, plot_right, first, f) = (self.doc, self.left, self.right, self.first, self.f);
        let ruler_top = bottom - g::RULER_H;
        let rect_top = bottom - height;
        painter.rect_filled(Rect::from_min_max(Pos2::new(self.x0, rect_top), Pos2::new(client_right, bottom)), 0.0, Color32::WHITE);
        painter.px_hline(self.x0..=client_right, rect_top, Stroke::new(1.0f32, Color32::BLACK));
        let text_col = Color32::from_rgb(0, 0, 127);
        let start_tod = fvb_core::comtrade::time_of_day(&doc.osc.stamps.start_time);
        if self.unit == TimeUnit::Absolute {
            // The start date at (5, H − 20) under the stats panel.
            if let Some(date) = fvb_core::comtrade::stamp_date_text(&doc.osc.stamps.start_date, doc.osc.rev_year) {
                painter.text(Pos2::new(self.x0 + 5.0, bottom - 20.0), Align2::LEFT_TOP, date, FontId::proportional(11.0), text_col);
            }
        }
        let mut first_label = true;
        for t in g::ruler_ticks(left, plot_right) {
            painter.px_vline(t.x, ruler_top..=ruler_top + t.len, Stroke::new(1.0f32, Color32::BLACK));
            if t.labelled {
                let k = g::tick_sample(first, t.i, f);
                let (tk, kr) = self.label_at(k);
                let s = g::ruler_label(self.unit, tk, kr, first_label, start_tod);
                painter.text(Pos2::new(t.x, bottom - 20.0), Align2::CENTER_TOP, s, FontId::proportional(11.0), text_col);
                first_label = false;
            }
        }
    }

    fn viz(&self) -> [(usize, bool, Color32, &'static str); 3] {
        let doc = self.doc;
        [
            (doc.main_cursor, doc.v_shown[0], Color32::from_rgb(255, 100, 100), "В1"),
            (doc.aux_cursor, doc.v_shown[1], Color32::from_rgb(100, 100, 255), "В2"),
            (doc.v3_cursor, doc.v_shown[2], Color32::from_rgb(0, 160, 0), "В3"),
        ]
    }

    /// The difference band (`0x466630`) above a ruler whose top is
    /// `ruler_top`, on the ruler's white: a (100) line across at its top,
    /// the (230) grid on through it, marker labels and Δt dimension lines.
    pub fn band(&self, painter: &egui::Painter, ruler_top: f32, client_right: f32, grid: bool) {
        let (doc, left, plot_right) = (self.doc, self.left, self.right);
        let text_col = Color32::from_rgb(0, 0, 127);
        let start_tod = fvb_core::comtrade::time_of_day(&doc.osc.stamps.start_time);
        let band_top = ruler_top - g::DIFF_BAND_H;
        painter.px_hline(self.x0..=client_right, ruler_top, Stroke::new(1.0f32, Color32::from_rgb(100, 100, 100)));
        if grid {
            let mut x = left + 50.0;
            while x < plot_right {
                painter.px_vline(x, band_top..=ruler_top, Stroke::new(1.0f32, Color32::from_rgb(230, 230, 230)));
                x += 50.0;
            }
        }
        // Positions (`0x4669d8`): leftW + trunc((s − first)·25·f).
        let bx = |k: usize| left + ((k as f64 - self.first as f64) * g::PX_PER_SAMPLE * self.f).trunc() as f32;
        // Labels: В1 В2 В3 first, then the comment moments by sample; one is
        // dropped when its cell ±3 px overlaps a placed one; only x > leftW.
        // A map by sample (`0x4406e0`): a later moment on the same sample
        // replaces the earlier one.
        let moments: std::collections::BTreeMap<usize, Option<&str>> = doc.comments.iter().filter_map(|c| c.moment().map(|k| (k, (!c.hidden).then_some(c.text.as_str())))).collect();
        let order = self.viz().iter().filter(|v| v.1).map(|v| (v.0, None)).chain(moments.iter().map(|(&k, &t)| (k, t))).collect::<Vec<_>>();
        let mut taken: Vec<(f32, f32)> = Vec::new();
        for (k, text) in order {
            let x = bx(k);
            if x <= left {
                continue;
            }
            let (t, kr) = self.label_at(k);
            let s = g::mark_label(self.unit, t, kr, start_tod);
            let galley = painter.layout_no_wrap(s, FontId::proportional(11.0), text_col);
            let (l, r) = (x - (galley.size().x / 2.0).trunc(), x + (galley.size().x / 2.0).trunc() + 1.0);
            if taken.iter().any(|&(a, b)| l - 3.0 <= b && a <= r + 3.0) {
                continue;
            }
            taken.push((l, r));
            painter.galley(Pos2::new(l, ruler_top - 18.0 - galley.size().y / 2.0), galley, text_col);
            if let Some(t) = text.filter(|t| !t.is_empty()) {
                painter.text(Pos2::new(x, ruler_top - 10.0), Align2::CENTER_TOP, t.lines().next().unwrap_or(""), FontId::proportional(9.0), COMMENT_COLOR);
            }
        }
        // Dimension lines between consecutive marks of a map keyed by sample
        // (moments and shown визиры, deduplicated): drawn when x_a > leftW and
        // x_b − x_a > 16; no right-edge test.
        let keys: std::collections::BTreeSet<usize> = self.viz().iter().filter(|v| v.1).map(|v| v.0).chain(moments.keys().copied()).collect();
        let marks: Vec<(f32, usize)> = keys.into_iter().map(|k| (bx(k), k)).collect();
        let dim_y = ruler_top - 35.0;
        for w in marks.windows(2) {
            let (xa, xb) = (w[0].0, w[1].0);
            if xa <= left || xb - xa <= 16.0 {
                continue;
            }
            let st = Stroke::new(1.0f32, text_col);
            painter.px_hline(xa..=xb, dim_y, st);
            painter.px_vline(xa, dim_y - 4.0..=dim_y + 4.0, st);
            painter.px_vline(xb, dim_y - 4.0..=dim_y + 4.0, st);
            for (tip, dir) in [(xa, 1.0f32), (xb, -1.0)] {
                painter.add(Shape::convex_polygon(
                    vec![Pos2::new(tip, dim_y), Pos2::new(tip + 8.0 * dir, dim_y - 2.0), Pos2::new(tip + 8.0 * dir, dim_y + 2.0)],
                    text_col,
                    Stroke::NONE,
                ));
            }
            let dt = (self.time_at(w[1].1) - self.time_at(w[0].1)).abs();
            let s = g::diff_label(self.unit, dt, (w[1].1 as i64 - w[0].1 as i64).abs());
            let galley = painter.layout_no_wrap(s, FontId::proportional(10.0), text_col);
            if galley.size().x + 4.0 < xb - xa && xb - xa > 20.0 {
                let pos = Pos2::new((xa + xb) / 2.0 - galley.size().x / 2.0, dim_y - galley.size().y - 1.0);
                painter.galley(pos, galley, text_col);
            }
        }
    }

    /// The vertical визиры (`0x493110`) from `top` to `v_bottom`, 1 px, no
    /// shading between В1 and В2; `labels` = the В1/В2/В3 badges at the top.
    /// A vertical визир's x on screen (`0x493110`, with its fallback quirk).
    pub fn vizir_x(&self, k: usize) -> f32 {
        self.x0 + g::vizir_x(k, self.first, self.n, self.f, self.left - self.x0, self.right - self.x0)
    }

    /// The vertical визиры (`0x493110`): every shown line with x > leftW
    /// from `top` to `v_bottom` (В2 takes В3's green pen when it coincides
    /// with В1, shown or not), then the label row at `top` — clamped,
    /// merged (`g::vizir_labels`), each on an opaque white text cell.
    pub fn vizirs(&self, painter: &egui::Painter, top: f32, v_bottom: f32, labels: bool) {
        // The trigger moment (`0x464219`): a dotted line when the record
        // has one (`.osc` with pre-trigger blocks).
        let trig = self.doc.osc.trigger_sample;
        if trig != 0 {
            let x = self.vizir_x(trig);
            if x > self.left {
                let pts = [Pos2::new(x, top), Pos2::new(x, v_bottom)];
                painter.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.0f32, Color32::from_rgb(35, 194, 87)), 1.0, 1.0));
            }
        }
        let viz = self.viz();
        let xs: Vec<f32> = viz.iter().map(|v| self.vizir_x(v.0)).collect();
        for (i, (_, shown, col, _)) in viz.iter().enumerate() {
            if !shown || xs[i] <= self.left {
                continue;
            }
            let col = if i == 1 && xs[1] == xs[0] { viz[2].2 } else { *col };
            painter.px_vline(xs[i], top..=v_bottom, Stroke::new(1.0f32, col));
        }
        if !labels {
            return;
        }
        let measure = |t: &str| painter.layout_no_wrap(t.to_string(), FontId::proportional(11.0), Color32::BLACK).size().x;
        let rel = |i: usize| (viz[i].1, xs[i] - self.x0);
        let row = g::vizir_labels([rel(0), rel(1), rel(2)], self.left - self.x0, self.right - self.x0, &measure);
        for l in row {
            let col = viz[l.idx].2;
            let galley = painter.layout_no_wrap(l.text, FontId::proportional(11.0), col);
            let (w, h) = (galley.size().x, galley.size().y);
            let x = self.x0 + l.x;
            // DrawText(DT_CENTER) in (x − w/2, 0, x + 1 + w/2, h), opaque.
            let cell = Rect::from_min_max(Pos2::new(x - (w / 2.0).trunc(), top), Pos2::new(x + 1.0 + (w / 2.0).trunc(), top + h));
            painter.rect_filled(cell, 0.0, Color32::WHITE);
            painter.galley(Pos2::new(cell.center().x - w / 2.0, top), galley, col);
        }
    }
}

/// The horizontal визиры (`0x49414c`–`0x494437`) over the view whose client
/// spans `x0..right` × `top..` with the bottom band at `band_top`: a shown
/// line lies at `ys[j]` (None without a channel) from leftW to W when above
/// the band; ГВ2 takes В3's green pen on ГВ1's y. A label appears **only
/// for a line out of view** — clamped to the top or to the band — just left
/// (ГВ1) or right (ГВ2) of half the client width, on an opaque cell.
pub fn paint_hvizirs(painter: &egui::Painter, (x0, left, right): (f32, f32, f32), (top, band_top): (f32, f32), ys: [(bool, Option<f32>); 2]) {
    let red = Color32::from_rgb(255, 100, 100);
    let blue = Color32::from_rgb(100, 100, 255);
    let green = Color32::from_rgb(0, 160, 0);
    let mid = x0 + ((right - x0) / 2.0).trunc();
    // ГВ1's y as ГВ2's pen test sees it: its clamped label y when clamped.
    let mut y1_seen: Option<f32> = None;
    for (j, (shown, y)) in ys.into_iter().enumerate() {
        let Some(y) = y else { continue };
        let label_y = if y <= top {
            Some(top)
        } else if y >= band_top {
            Some(band_top)
        } else {
            None
        };
        if j == 0 {
            y1_seen = Some(label_y.unwrap_or(y));
        }
        if !shown {
            continue;
        }
        if y < band_top {
            let col = if j == 1 && y1_seen == Some(y) { green } else if j == 0 { red } else { blue };
            painter.px_hline(left..=right, y, Stroke::new(1.0f32, col));
        }
        if let Some(ly) = label_y {
            let col = if j == 0 { red } else { blue };
            let galley = painter.layout_no_wrap(if j == 0 { "ГВ1" } else { "ГВ2" }.to_string(), FontId::proportional(11.0), col);
            let (cx, cy) = (galley.size().x, galley.size().y);
            let (l, r) = if j == 0 { (mid - cx - 1.0, mid) } else { (mid, mid + cx + 1.0) };
            let (t, b) = if ly == top { (top, top + cy) } else { ((ly - cy).max(top), ly) };
            let cell = Rect::from_min_max(Pos2::new(l, t), Pos2::new(r, b));
            painter.rect_filled(cell, 0.0, Color32::WHITE);
            painter.galley(Pos2::new(cell.center().x - cx / 2.0, t), galley, col);
        }
    }
}

/// A group member's block height (`0x461440`, `0x460290`): the name row
/// + 33, + the stats line when the member has stat rows.
fn group_stride(doc: &crate::app::Doc, m: ChanKey) -> f32 {
    ROW_H + 33.0 + if stat_rows(doc, &m).is_some_and(|s| s.count() > 0) { GROUP_STATS_H } else { 0.0 }
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
fn status_icons(doc: &crate::app::Doc, r: &Row) -> Vec<&'static str> {
    key_icons(doc, r.key)
}

/// The status icons of one channel.
pub(crate) fn key_icons(doc: &crate::app::Doc, k: ChanKey) -> Vec<&'static str> {
    match k {
        Lane::Analog(i) => chan_icons(doc, i),
        // A computed or pasted channel's icons: no bit-13 / 14 gate but the
        // ratio's (`0x462b15`).
        Lane::Computed(j) if !doc.pending_digital(j) => {
            let c = &doc.computed[j];
            // A pasted lane read ×k in its copied mode (VM: «*K» on a
            // pasted channel with a ratio, shown secondary).
            let scaled = c.formula == crate::merge_ui::PENDING && doc.pending.iter().find(|p| p.lane == c.seq).is_some_and(|p| p.ratio_icon());
            [(c.remove_dc, "DC"), (c.invert, "−"), (scaled, "*K"), (c.mode == ValueMode::Rms, "~"), (c.mode == ValueMode::FirstHarmonic, "1"), (c.adc_counts, "#")]
                .into_iter()
                .filter_map(|(on, g)| on.then_some(g))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// The first sample after a page of the view's horizontal scroll bar
/// (Shift+wheel, `0x45d990`). The bar runs 0…10000, its position read back
/// from the first sample: a page is ⌊V·10000 / N⌋ (V the samples shown, N
/// the record's), the first sample ⌊(N − V)·pos / 10000⌋, never past the
/// last screen (VM: two notches from 0 land on the bar's second page, short of
/// two visible widths).
pub(crate) fn hscroll_page(first: usize, n: usize, visible: usize, up: bool) -> usize {
    let range = n.saturating_sub(visible);
    if n == 0 || range == 0 {
        return 0;
    }
    let step = visible * 10000 / n;
    let pos = first * 10000 / range;
    if up {
        range * pos.saturating_sub(step) / 10000
    } else {
        (range * (pos + step) / 10000).min(range)
    }
}

/// The «*K» test (`0x462b22`–`0x462b8f`): the channel's shown and stored primary / secondary flags
/// differ and either stored ratio value is not 1 (a zero or a NaN is not 1). The stored values are
/// tested, not the factor they give: primary == secondary ≠ 1 shows the icon, a zero value too.
pub(crate) fn ratio_icon(ch: &fvb_core::comtrade::AnalogChannel, show_primary: bool) -> bool {
    show_primary != ch.is_primary && (ch.primary != 1.0 || ch.secondary != 1.0)
}

/// The status icons of analog channel `i`.
fn chan_icons(doc: &crate::app::Doc, i: usize) -> Vec<&'static str> {
    let (c, ch) = (&doc.chans[i], &doc.osc.analog[i]);
    // The channel's own flags (`+0x88` / `+0x89`) and ratio decide, not the document's mode.
    let scaled = ratio_icon(ch, c.primary);
    [
        (c.window.is_some(), "↕"),
        (c.remove_dc, "DC"),
        (c.invert, "−"),
        // The coefficient's «*K» (the ratio applied, `0x462b15`).
        (scaled, "*K"),
        (c.mode == ValueMode::Rms, "~"),
        (c.mode == ValueMode::FirstHarmonic, "1"),
        (c.adc_counts, "#"),
    ]
    .into_iter()
    .filter_map(|(on, g)| on.then_some(g))
    .collect()
}

/// The stats panel's columns (`0x4606d0`): every channel of the record is
/// measured in the names font, hidden ones too — the label column is the
/// widest analog / computed name (cut to 25 characters) + 5, at least
/// «Max:»; the value template (": -0.00000000 A" for SI units, else
/// ": -0.0000000E-000 ") + 10 widens the rest; discrete names (+ 10) only
/// bound the whole width from below, as does the absolute-time stamp. A
/// group member with stat rows on widens the label column to its stats
/// line's template + 10 in the group line's font (`0x468b00` at
/// `0x46121c`): «Max:000.00mV|Min:000.00mV», «СКЗ:000.00mV», «A1:000.00mV»
/// for the rows it shows, `|`-joined.
/// Returns (label column, panel width).
pub(crate) fn stats_columns(fonts: &egui::text::Fonts, doc: &crate::app::Doc, rows: &[Row], abs_time: bool) -> (f32, f32) {
    // The names are drawn bold: measured so.
    let name_font = bold_font(fonts, NAME_FONT.size).unwrap_or(NAME_FONT);
    let w = |s: &str, f: &FontId| {
        let f = if *f == NAME_FONT { &name_font } else { f };
        fonts.layout_no_wrap(s.to_string(), f.clone(), Color32::BLACK).size().x
    };
    let cut = |n: &str| n.chars().take(NAME_CHARS).collect::<String>();
    let mut label = w("Max:", &NAME_FONT);
    let (mut templ, mut digital) = (0.0f32, 0.0f32);
    let analog = (0..doc.osc.analog.len()).map(Lane::Analog).chain((0..doc.computed.len()).map(Lane::Computed));
    let mut any = false;
    for lane in analog {
        any = true;
        let (name, unit, ..) = lane_stats(doc, &lane, 0);
        label = label.max(w(&cut(&name), &NAME_FONT) + 5.0);
        let t = if fvb_core::views::si_unit(&unit.canon) { ": -0.00000000 A" } else { ": -0.0000000E-000 " };
        templ = templ.max(w(t, &NAME_FONT) + 10.0);
    }
    for c in &doc.osc.digital {
        any = true;
        digital = digital.max(w(&cut(&c.id), &NAME_FONT) + 10.0);
        templ = templ.max(10.0);
    }
    if !any {
        return (label, label);
    }
    for m in rows.iter().filter(|r| r.members.len() > 1).flat_map(|r| r.members.iter()) {
        let st = stat_rows(doc, m).unwrap_or_default();
        let pieces: Vec<&str> = [(st.minmax, "Max:000.00mV|Min:000.00mV"), (st.rms, "СКЗ:000.00mV"), (st.a1, "A1:000.00mV")].into_iter().filter_map(|(on, t)| on.then_some(t)).collect();
        if !pieces.is_empty() {
            label = label.max(w(&pieces.join("|"), &ROW_FONT) + 10.0);
        }
    }
    let mut left = label + templ;
    if abs_time {
        left = left.max(w("99/99/9999   ", &ROW_FONT) + 5.0 + w("99:99:99.999", &ROW_FONT) / 2.0);
    }
    (label, left.max(digital))
}

/// Where this frame's button press happened: egui's press origin, or —
/// when the release came in the same frame (a quick click, which egui
/// clears it for) — the press event's own position.
pub(crate) fn press_pos(i: &egui::InputState) -> Option<egui::Pos2> {
    i.pointer.press_origin().or_else(|| i.events.iter().find_map(|e| match e {
        egui::Event::PointerButton { pos, pressed: true, .. } => Some(*pos),
        _ => None,
    }))
}

/// A sample's mark (`0x429b52`): GDI's Ellipse(x − 2, y − 2, x + 2, y + 2)
/// with the channel's 1 px pen and the DC's white brush — a 4 × 4 ring,
/// the right and bottom edges left out, its middle white over the line
/// (VM).
fn sample_mark(p: &egui::Painter, q: Pos2, color: Color32) {
    // Whole pixels, unfeathered, the point rounded as the line's
    // (`oscgeom::aliased_runs`).
    let (x, y) = (q.x.round(), q.y.round());
    let mut mesh = egui::Mesh::default();
    for (dx, dy) in [(-1.0, -2.0), (0.0, -2.0), (-2.0, -1.0), (1.0, -1.0), (-2.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (0.0, 1.0)] {
        mesh.add_colored_rect(Rect::from_min_size(Pos2::new(x + dx, y + dy), Vec2::splat(1.0)), color);
    }
    for (dx, dy) in [(-1.0, -1.0), (0.0, -1.0), (-1.0, 0.0), (0.0, 0.0)] {
        mesh.add_colored_rect(Rect::from_min_size(Pos2::new(x + dx, y + dy), Vec2::splat(1.0)), Color32::WHITE);
    }
    p.add(Shape::mesh(mesh));
}

/// The bold face at `size`, when the fonts have it (the app's Tahoma Bold;
/// a bare context has none).
pub(crate) fn bold_font(fonts: &egui::text::Fonts, size: f32) -> Option<FontId> {
    let fam = egui::FontFamily::Name("bold".into());
    fonts.families().contains(&fam).then(|| FontId::new(size, fam))
}

/// Text in the bold face — without one, the glyphs drawn twice, half a
/// pixel apart.
pub(crate) fn bold_text(p: &egui::Painter, pos: Pos2, text: &str, font: FontId, color: Color32) -> Rect {
    if let Some(b) = p.ctx().fonts(|f| bold_font(f, font.size)) {
        return p.text(pos, Align2::LEFT_TOP, text, b, color);
    }
    p.text(pos + Vec2::new(0.5, 0.0), Align2::LEFT_TOP, text, font.clone(), color);
    p.text(pos, Align2::LEFT_TOP, text, font, color)
}

/// The amplitude range of a lane: min..max of the displayed series over the
/// whole record (a group: min of mins, max of maxes).
pub(crate) fn row_range(doc: &crate::app::Doc, r: &Row) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut take = |v: f64| {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    };
    match r.key {
        _ if r.members.len() > 1 => {
            for &m in &r.members {
                let (a, b) = member_range(doc, m);
                take(a);
                take(b);
            }
        }
        Lane::Analog(i) => {
            let (a, b) = chan_range(doc, i);
            take(a);
            take(b);
        }
        // A pasted channel's own samples (its `+0xa8` / `+0xb0`).
        Lane::Computed(i) => match doc.pending_of(i).map(|p| &p.pasted.channel) {
            Some(Ok(_)) => doc.computed[i].own_series().iter().for_each(|&v| take(v)),
            Some(Err(_)) => return (0.0, 1.0),
            None => doc.computed[i].shown_series().iter().for_each(|&v| take(v)),
        },
        Lane::Digital(_) => return (0.0, 1.0),
    }
    if lo > hi {
        (0.0, 0.0)
    } else {
        (lo, hi)
    }
}

/// An analog channel's drawing range: min..max of its displayed series over
/// the whole record, or its window zoom (`0x42b640`).
/// A level's channel range for 33042's commit (ch+0xb0 / ch+0xa8, the
/// whole record): `None` when the channel does not resolve.
fn level_range(doc: &crate::app::Doc, chan: crate::app::ChanKey) -> Option<(f64, f64)> {
    use crate::app::ChanKey;
    match chan {
        ChanKey::Analog(i) if i < doc.osc.analog.len() => Some(chan_range(doc, i)),
        ChanKey::Computed(j) => doc.computed.get(j).map(|c| c.samples.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| (lo.min(v), hi.max(v)))),
        ChanKey::Digital(k) if k < doc.osc.digital.len() => Some((0.0, 1.0)),
        _ => None,
    }
}

/// A group member's own range (`0x42b640` over its `+0xa8` / `+0xb0`): a
/// file channel's [`chan_range`], a computed one's displayed series.
fn member_range(doc: &crate::app::Doc, k: ChanKey) -> (f64, f64) {
    match k {
        Lane::Analog(i) => chan_range(doc, i),
        Lane::Computed(j) => {
            let v = doc.computed[j].shown_series().iter().copied().filter(|v| v.is_finite());
            let (lo, hi) = v.fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), x| (l.min(x), h.max(x)));
            if lo > hi {
                (0.0, 0.0)
            } else {
                (lo, hi)
            }
        }
        Lane::Digital(_) => (0.0, 1.0),
    }
}

pub fn chan_range(doc: &crate::app::Doc, i: usize) -> (f64, f64) {
    let [mut lo, mut hi, ..] = scaled_minmax(doc, i);
    if lo > hi {
        (lo, hi) = (0.0, 0.0);
    }
    let c = &doc.chans[i];
    g::effective_range(lo, hi, c.window, c.mode == ValueMode::Instant)
}

/// Fill `rect` with a vertical gradient.
pub(crate) fn gradient(p: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
}

/// The paste mark of a pending lane (image list 489, 30×18: icon 0 a
/// large ◀|•|▶, icon 1 a small ◀|▶, magenta) — our own drawing of it.
fn paste_icon(p: &egui::Painter, at: Pos2, large: bool) {
    let c = Color32::from_rgb(230, 20, 140);
    let pt = |x: f32, y: f32| at + Vec2::new(x, y);
    let (l, r, top, bot) = if large { (2.0, 28.0, 2.0, 16.0) } else { (6.0, 24.0, 5.0, 13.0) };
    let mid = 9.0;
    let (bl, br) = if large { (12.0, 18.0) } else { (13.0, 17.0) };
    p.add(Shape::convex_polygon(vec![pt(l, mid), pt(bl, top), pt(bl, bot)], c, Stroke::NONE));
    p.add(Shape::convex_polygon(vec![pt(r, mid), pt(br, bot), pt(br, top)], c, Stroke::NONE));
    p.px_vline(at.x + bl + 1.5, (at.y + top)..=(at.y + bot), Stroke::new(1.0f32, c));
    p.px_vline(at.x + br - 1.5, (at.y + top)..=(at.y + bot), Stroke::new(1.0f32, c));
    if large {
        p.rect_filled(Rect::from_min_size(pt(14.0, mid - 1.0), Vec2::splat(2.0)), 0.0, c);
    }
}

/// The fill of a digital lane's "1" runs: the channel's own colour
/// (`ch+0x110`, the brush at `0x42a18d`) — (255, 255, 100) from every
/// channel constructor (`0x4225d7`, `0x424070`, …), (249, 175, 172) for the
/// ОМП flags (`0x4245a0` from `0x42ead0`); the name text keeps the table
/// colour by place (L9.170).
pub const DIGITAL_FILL: Color32 = Color32::from_rgb(255, 255, 100);

fn digital_fill(omp: bool) -> Color32 {
    if omp {
        Color32::from_rgb(249, 175, 172)
    } else {
        DIGITAL_FILL
    }
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
    // Aliased, as GDI's Polyline; the marks come after the line (`0x429b52`).
    let mut mesh = egui::Mesh::default();
    for r in g::aliased_runs(&pts) {
        mesh.add_colored_rect(r, color);
    }
    p.add(Shape::mesh(mesh));
    if f > 0.16 {
        for q in &pts {
            sample_mark(p, *q, color);
        }
    }
}

/// Row geometry for hit-testing.
pub struct RowGeo<'a> {
    pub left: f32,
    pub axis_top: f32,
    pub tops: &'a [f32],
    pub rows: &'a [Row],
}

/// The analog lane under `y` and the value there on its scale.
/// A level line's place (`0x471020`): the lane of channel `k` and its
/// current range — a digital channel its raw 0 / 1 window, drawn snapped.
pub fn level_place(doc: &crate::app::Doc, (rows, tops, ranges): (&[Row], &[f32], &[(f64, f64)]), k: Lane, v: f64) -> Option<g::LevelY> {
    let li = rows.iter().position(|r| r.has(k))?;
    let ((lo, hi), digital) = match k {
        Lane::Digital(j) => (g::digital_window(&doc.osc.digital.get(j)?.samples), true),
        _ if rows[li].members.len() > 1 => (member_range(doc, k), false),
        _ => (ranges[li], false),
    };
    g::level_y(v, tops[li], rows[li].h, lo, hi, digital)
}

/// An analog-kind channel (`ch+0x5c` == 0): the record's analog channels,
/// formula channels and pasted analog ones — not a digital or pasted
/// digital one.
fn analog_kind(doc: &crate::app::Doc, k: Lane) -> bool {
    match k {
        Lane::Analog(_) => true,
        Lane::Computed(j) => !doc.pending_digital(j),
        Lane::Digital(_) => false,
    }
}

/// The lane a level line is placed on (`0x43eb40`: a visible analog-kind
/// lane) and the value under `y`.
fn level_lane_at(doc: &crate::app::Doc, y: f32, geo: &RowGeo, ranges: &[(f64, f64)]) -> Option<(Lane, f64)> {
    let li = geo.tops.iter().zip(geo.rows).position(|(&t, r)| y >= t && y < t + r.h)?;
    let r = &geo.rows[li];
    analog_kind(doc, r.key).then(|| (r.key, g::y_value(y, geo.tops[li], r.h, ranges[li].0, ranges[li].1)))
}

fn analog_at(y: f32, geo: &RowGeo, ranges: &[(f64, f64)]) -> Option<(usize, f64)> {
    let li = geo.tops.iter().zip(geo.rows).position(|(&t, r)| y >= t && y < t + r.h)?;
    let r = &geo.rows[li];
    let Lane::Analog(ch) = r.key else { return None };
    let (lo, hi) = ranges[li];
    Some((ch, g::y_value(y, geo.tops[li], r.h, lo, hi)))
}

/// A window scroll bar (the view's own `SB_HORZ` / `SB_VERT`, `0x45d4a0`)
/// over `[0, max]` with a page of `page`: arrow buttons step by `line`, the
/// track by one page, the thumb drags. Shown disabled (no thumb, inert) when
/// there is nothing to scroll (`SIF_DISABLENOSCROLL`). Returns the new value.
/// `page` sizes the thumb (nPage); `line` / `jump` are the steps of an
/// arrow / a click on the track.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scrollbar(ui: &mut egui::Ui, rect: Rect, horizontal: bool, value: f32, max: f32, page: f32, line: f32, jump: f32) -> Option<f32> {
    let p = ui.painter_at(rect);
    // Windows 11's bar (VM): a BTNFACE track; idle, the arrows hidden (their
    // room kept) and the thumb a 2 px (133, 133, 133) line along the
    // middle, 1 px in from its ends; under the pointer, the arrows (no box)
    // and a 6 px rounded thumb along the middle (VM: rows 1148–1153 of the
    // view's 1143–1158 bar, the pointer on its thumb, track or arrow).
    p.rect_filled(rect, 0.0, Color32::from_gray(240));
    let hot = ui.rect_contains_pointer(rect);
    let len = if horizontal { rect.width() } else { rect.height() };
    let thick = if horizontal { rect.height() } else { rect.width() };
    if len <= 0.0 || thick <= 0.0 {
        return None;
    }
    let enabled = max > 0.0;
    // Arrow buttons, squeezed when the bar is shorter than two of them.
    let arrow = thick.min(len / 2.0);
    let along = |a: f32, b: f32| {
        if horizontal {
            Rect::from_min_max(Pos2::new(rect.left() + a, rect.top()), Pos2::new(rect.left() + b, rect.bottom()))
        } else {
            Rect::from_min_max(Pos2::new(rect.left(), rect.top() + a), Pos2::new(rect.right(), rect.top() + b))
        }
    };
    let (dec, inc) = (along(0.0, arrow), along(len - arrow, len));
    let glyph = Color32::from_gray(if enabled { 96 } else { 180 });
    for (r, back) in [(dec, true), (inc, false)].into_iter().filter(|_| hot) {
        let (c, s) = (r.center(), (arrow / 4.0).min(4.0));
        let tri = match (horizontal, back) {
            (true, true) => [c + Vec2::new(-s, 0.0), c + Vec2::new(s, -s), c + Vec2::new(s, s)],
            (true, false) => [c + Vec2::new(s, 0.0), c + Vec2::new(-s, -s), c + Vec2::new(-s, s)],
            (false, true) => [c + Vec2::new(0.0, -s), c + Vec2::new(-s, s), c + Vec2::new(s, s)],
            (false, false) => [c + Vec2::new(0.0, s), c + Vec2::new(-s, -s), c + Vec2::new(s, -s)],
        };
        p.add(Shape::convex_polygon(tri.to_vec(), glyph, Stroke::NONE));
    }
    if !enabled {
        return None;
    }
    let track = len - 2.0 * arrow;
    let thumb_len = (track * page / (max + page)).max(8.0_f32.min(track)).min(track).max(0.0);
    let travel = track - thumb_len;
    let at = arrow + travel * (value / max).clamp(0.0, 1.0);
    let thumb = along(at, at + thumb_len).shrink2(if horizontal { Vec2::new(0.0, 2.0) } else { Vec2::new(2.0, 0.0) });
    if thumb_len > 0.0 {
        if hot {
            let c = thumb.center();
            let fat = if horizontal {
                Rect::from_min_max(Pos2::new(thumb.left() + 1.0, (c.y - 3.0).round()), Pos2::new(thumb.right() - 1.0, (c.y - 3.0).round() + 6.0))
            } else {
                Rect::from_min_max(Pos2::new((c.x - 3.0).round(), thumb.top() + 1.0), Pos2::new((c.x - 3.0).round() + 6.0, thumb.bottom() - 1.0))
            };
            p.rect_filled(fat, 3.0, Color32::from_gray(133));
        } else {
            let c = thumb.center();
            let line = if horizontal {
                Rect::from_min_max(Pos2::new(thumb.left() + 1.0, (c.y - 1.0).round()), Pos2::new(thumb.right() - 1.0, (c.y - 1.0).round() + 2.0))
            } else {
                Rect::from_min_max(Pos2::new((c.x - 1.0).round(), thumb.top() + 1.0), Pos2::new((c.x - 1.0).round() + 2.0, thumb.bottom() - 1.0))
            };
            p.rect_filled(line, 0.0, Color32::from_gray(133));
        }
    }
    let id = ui.id().with(("fv_scroll", horizontal));
    let r = ui.interact(rect, id, Sense::click_and_drag());
    let along_of = |p: Pos2| if horizontal { p.x - rect.left() } else { p.y - rect.top() };
    // The system's own context menu of a window scroll bar (seen on the VM:
    // «Scroll Here», …, in the system's language). Its commands reach the
    // view's handlers: WM_HSCROLL `0x45d6d0` ignores SB_LEFT / SB_RIGHT
    // (`0x45d81a`); WM_VSCROLL `0x45d870` takes nPos for SB_TOP and
    // SB_BOTTOM too (`0x45d917`) — 0, so both go to the top; the page items
    // step as the arrows (`0x45d848`, `0x45d97c`).
    let menu_id = id.with("menu");
    if r.secondary_clicked() {
        if let Some(q) = r.interact_pointer_pos() {
            let menu = crate::popmenu::PopMenu::new(ui.ctx().cumulative_pass_nr(), q);
            ui.data_mut(|d| d.insert_temp(menu_id, (menu, along_of(q))));
        }
    }
    if let Some((mut menu, at_click)) = ui.data(|d| d.get_temp::<(crate::popmenu::PopMenu, f32)>(menu_id)) {
        use crate::popmenu::{Entry, Mark};
        let names = if horizontal {
            ["Scroll Here", "Left Edge", "Right Edge", "Page Left", "Page Right", "Scroll Left", "Scroll Right"]
        } else {
            ["Scroll Here", "Top", "Bottom", "Page Up", "Page Down", "Scroll Up", "Scroll Down"]
        };
        let mut entries = Vec::new();
        for (k, name) in names.into_iter().enumerate() {
            if matches!(k, 1 | 3 | 5) {
                entries.push(Entry::Sep);
            }
            entries.push(Entry::item(name, true, Mark::None, k));
        }
        let out = crate::popmenu::show(ui.ctx(), "fv_scroll_menu", &mut menu, &entries);
        if out.close {
            ui.data_mut(|d| d.remove::<(crate::popmenu::PopMenu, f32)>(menu_id));
        } else {
            ui.data_mut(|d| d.insert_temp(menu_id, (menu, at_click)));
        }
        let track = len - 2.0 * arrow;
        let thumb_len = (track * page / (max + page)).max(8.0_f32.min(track)).min(track).max(0.0);
        let travel = track - thumb_len;
        return match out.chosen {
            // SB_THUMBPOSITION for the thumb centred on the click.
            Some(0) if travel > 0.0 => Some(((at_click - arrow - thumb_len / 2.0) / travel * max).clamp(0.0, max)),
            Some(1 | 2) if !horizontal => Some(0.0),
            Some(3 | 5) => Some((value - if horizontal { jump } else { line }).clamp(0.0, max)),
            Some(4 | 6) => Some((value + if horizontal { jump } else { line }).clamp(0.0, max)),
            _ => None,
        };
    }
    // Thumb tracking: where the thumb was grabbed, kept for the whole drag;
    // the thumb then follows the pointer's position along the bar wherever
    // the pointer goes (a press on the track does not grab it).
    let grab_id = id.with("grab");
    if r.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        let grab = origin.filter(|o| thumb.contains(*o)).map(|o| along_of(o) - at);
        ui.data_mut(|d| d.insert_temp(grab_id, grab));
    }
    let grab = ui.data(|d| d.get_temp::<Option<f32>>(grab_id)).flatten();
    if r.drag_stopped() {
        ui.data_mut(|d| d.remove::<Option<f32>>(grab_id));
    }
    if let (true, Some(g), Some(p)) = (r.dragged() && travel > 0.0, grab, r.interact_pointer_pos()) {
        return Some(((along_of(p) - g - arrow) / travel * max).clamp(0.0, max));
    }
    // Arrows and track act on the press and then repeat while the button
    // is held (window scroll bar behaviour; the track stops once the thumb
    // reaches the pointer). Delays are the system's [I]: 0.4 s, then 0.05 s.
    let rep_id = id.with("repeat");
    let now = ui.input(|i| i.time);
    let before_thumb = |q: Pos2| if horizontal { q.x < thumb.left() } else { q.y < thumb.top() };
    let part_at = |q: Pos2| -> i8 {
        if dec.contains(q) {
            -1
        } else if inc.contains(q) {
            1
        } else if thumb.contains(q) {
            0
        } else if before_thumb(q) {
            -2
        } else {
            2
        }
    };
    let step_of = |part: i8| match part {
        -1 => -line,
        1 => line,
        -2 => -jump,
        _ => jump,
    };
    let press = ui.input(|i| i.pointer.primary_pressed().then(|| press_pos(i)).flatten());
    if let Some(at) = press.filter(|p| r.rect.contains(*p)) {
        let part = part_at(at);
        if part != 0 {
            ui.data_mut(|d| d.insert_temp(rep_id, (part, now + 0.4)));
            return Some((value + step_of(part)).clamp(0.0, max));
        }
    }
    if !r.is_pointer_button_down_on() {
        ui.data_mut(|d| d.remove::<(i8, f64)>(rep_id));
        return None;
    }
    if let Some((part, next)) = ui.data(|d| d.get_temp::<(i8, f64)>(rep_id)) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
        // The track repeats only while the pointer is still beyond the thumb.
        let still = r.interact_pointer_pos().is_some_and(|q| part.abs() == 1 || part_at(q) == part);
        if now >= next && still {
            ui.data_mut(|d| d.insert_temp(rep_id, (part, now + 0.05)));
            return Some((value + step_of(part)).clamp(0.0, max));
        }
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
    PlaceComment(crate::comments::Comment, bool),
    CommentDrag(usize, crate::comments::Part),
    MoveComment(usize, crate::comments::Part, usize, Option<(ChanKey, Option<f64>)>),
    EndCommentDrag,
    RectZoom(RectZoom),
    MergeDrag(Option<(f32, f64)>),
    MergeOffset(f64),
    WindowZoom(usize, f64, f64),
    EditComment(usize),
    Hot(Option<usize>),
    Rename(usize),
    Select(ChanKey, SelClick),
    ClearSelection,
    OpenMenu(Pos2, LaneMenuKind),
    DropCombine(Vec<ChanKey>, ChanKey),
    Cmd(Command),
}

/// The drag payload of the stats panel (`0x42be30`): every selected
/// analog-kind channel of the active document (`ch+0x100` set; the collector
/// `0x45edc4` loops to `doc+0x1d8`, which counts file analog, formula and
/// pasted analog records only) — pasted lanes included, digital ones never
/// (#102).
#[derive(Debug, Clone, PartialEq)]
pub struct ChanDrag {
    pub chans: Vec<usize>,
    /// The formula channels among them, pasted (pending) lanes too
    /// (analog-kind records of the same vector, `0x4415b0` walks them all).
    pub computed: Vec<usize>,
    /// The record the drag started in (its serial): the payload carries
    /// channel numbers only, so a drop on another record's view or pane
    /// must not apply them there (#74, an improvement).
    pub doc: u64,
}

impl ChanDrag {
    fn of(keys: Vec<ChanKey>, doc: u64) -> ChanDrag {
        let pick = |f: fn(&ChanKey) -> Option<usize>| keys.iter().filter_map(f).collect::<Vec<_>>();
        ChanDrag { chans: pick(|k| if let ChanKey::Analog(i) = k { Some(*i) } else { None }), computed: pick(|k| if let ChanKey::Computed(j) = k { Some(*j) } else { None }), doc }
    }

    /// The dragged channels as keys.
    pub fn keys(&self) -> Vec<ChanKey> {
        let a = self.chans.iter().map(|&i| ChanKey::Analog(i));
        a.chain(self.computed.iter().map(|&j| ChanKey::Computed(j))).collect()
    }
}

/// What a drop target shows for a move drop. The original shows OLE's
/// move-drop cursor (an arrow with a small box); egui's cursor set has
/// no such shape, the plain arrow is the nearest here — on Windows the main
/// window draws the original's one instead (`dragcursor`, #157).
pub const MOVE_DROP_CURSOR: egui::CursorIcon = egui::CursorIcon::Default;

/// Which OLE picture the last pass showed for a channel drag (move over a
/// target, copy with Ctrl in the scroll band, none).
#[cfg(test)]
pub fn drop_picture(ctx: &egui::Context) -> crate::dragcursor::Drop {
    ctx.data(|d| d.get_temp::<crate::dragcursor::Drop>(egui::Id::new("fv_drag_cursor").with("kind")).unwrap_or(crate::dragcursor::Drop::None))
}

impl App {
    /// The channel drag's cursor: what a target put in `drag_cursor` this
    /// frame, "no-drop" everywhere else (ribbon, bars, the differential
    /// pane, captions, other records' views, #73). egui's own end-of-pass
    /// hook sets its "grabbing" cursor for any drag, after everything the
    /// frame drew, so ours runs after it (callbacks run in the order they
    /// were registered) and the frame hands it the decision.
    pub fn finish_drag_cursor(&mut self, ctx: &egui::Context) {
        let key = egui::Id::new("fv_drag_cursor");
        if !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
            ctx.data_mut(|d| d.insert_temp(key, true));
            ctx.on_end_pass(
                "fv_drag_cursor",
                std::sync::Arc::new(move |ctx: &egui::Context| {
                    let mut kind = crate::dragcursor::Drop::None;
                    if egui::DragAndDrop::has_payload_of_type::<ChanDrag>(ctx) {
                        let set = ctx.data(|d| d.get_temp::<Option<egui::CursorIcon>>(key.with("icon")).flatten());
                        ctx.set_cursor_icon(set.unwrap_or(egui::CursorIcon::NoDrop));
                        kind = match set {
                            Some(c) if c == MOVE_DROP_CURSOR => crate::dragcursor::Drop::Move,
                            Some(egui::CursorIcon::Copy) => crate::dragcursor::Drop::Copy,
                            _ => crate::dragcursor::Drop::None,
                        };
                    }
                    // The OLE pictures of move and copy where the platform
                    // has them (#157, `dragcursor`); egui's cursor set has
                    // none.
                    ctx.data_mut(|d| d.insert_temp(key.with("kind"), kind));
                    crate::dragcursor::show(kind);
                }),
            );
        }
        let set = self.drag_cursor.take();
        ctx.data_mut(|d| d.insert_temp(key.with("icon"), set));
    }
}

/// MFC's drag delay (`DD_DEFDRAGDELAY`, ms → s).
const DRAG_DELAY: f64 = 0.2;
/// MFC's drag-scroll inset (DD_DEFSCROLLINSET) and its delay / interval
/// (DD_DEFSCROLLDELAY / DD_DEFSCROLLINTERVAL, 50 ms).
const DRAG_INSET: f32 = 11.0;
const DRAG_SCROLL_S: f64 = 0.05;

/// Message 1086.
pub const MSG_COMBINE_UNITS: &str = "Совмещение невозможно.\nУ каналов разные единицы измерения.";

/// Message 239 (error icon), «Совместить каналы» with channels of different rates.
pub const MSG_COMBINE_RATES: &str = "Выбранные каналы имеют разную частоту";

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
    DiffBand,
    EditComment(usize),
    LockComment(usize),
    DeleteComment(usize),
    HideComment(usize),
    EditLevel(usize),
}

impl App {
    pub fn lanes_ui(&mut self, ui: &mut egui::Ui) {
        // No document: the MDI client's bare ground (`view_oscillogram`).
        let Some(ai) = self.active else { return };
        let (time_unit, show_grid, y_analog) = (self.time_unit, self.show_grid, self.y_analog);
        let full = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(full, Sense::click_and_drag());
        self.view_width = full.width();
        {
            let pref = self.xscale_pref;
            if self.docs[ai].xscale.is_none() {
                // Measured as a default-sized MDI child (see
                // `view_width_at_creation`), not at its final size.
                let area = self.mdi_area;
                let d = &self.docs[ai];
                let band = g::RULER_H + if d.diff_band { g::DIFF_BAND_H } else { 0.0 };
                let lanes_h: f32 = lane_layout(d, y_analog).iter().map(|r| r.h).sum::<f32>();
                let w = if area.width() > 0.0 { g::view_width_at_creation(area.width(), area.height(), lanes_h, band, g::WinMetrics::at(crate::dpi::dpi())) } else { full.width() };
                let n = d.osc.sample_count();
                self.docs[ai].xscale = Some(g::initial_scale(pref, w, n));
            }
        }
        let mut actions: Vec<Act> = Vec::new();
        let menu_busy = self.lane_menu.is_some() || self.lane_menu_eat;
        let doc = &self.docs[ai];
        let n = doc.osc.sample_count();
        let f = doc.xscale.unwrap_or(1.0);

        // ---- layout (`0x460290`, `0x45d4a0`) ----
        let band = g::RULER_H + if doc.diff_band { g::DIFF_BAND_H } else { 0.0 };
        let rows = lane_layout(doc, y_analog);
        let abs_time = self.time_unit == crate::app::TimeUnit::Absolute;
        let (label_col, left_w) = ui.fonts(|fo| stats_columns(fo, doc, &rows, abs_time));
        self.view_left = left_w;
        let content_h: f32 = rows.iter().map(|r| r.h).sum();
        // Each window scroll bar only while there is something to scroll
        // (seen on the VM: with the whole record and every lane in view
        // neither is shown); each takes room from the other's test.
        // Without the H bar: if the record fits, that layout stands.
        let hbar_on = {
            let vbar = content_h + band + 2.0 > full.height();
            let plot_w = full.width() - if vbar { scroll_w() } else { 0.0 };
            g::max_first(n, g::visible_count(plot_w, left_w, f)) > 0
        };
        let client = Rect::from_min_max(full.min, Pos2::new(full.right(), full.bottom() - if hbar_on { scroll_h() } else { 0.0 }));
        let vbar = content_h + band + 2.0 > client.height();
        let plot_right = client.right() - if vbar { scroll_w() } else { 0.0 };
        self.view_rects = Some((full, Rect::from_min_max(client.min, Pos2::new(plot_right, client.bottom()))));
        let left = client.left() + left_w;
        let plot_bottom = client.bottom() - band;
        // `0x45d8db`: the offset down to −(Σ lanes + band − client − 2).
        let vmin = -(content_h + band - 2.0 - client.height()).max(0.0);
        let vscroll = doc.vscroll.clamp(vmin, 0.0);
        let visible = g::visible_count(plot_right - client.left(), left_w, f);
        let first = doc.first.min(g::max_first(n, visible));
        let x_of = |k: f64| g::x_of(k, first, f, left);
        let sample_at = |x: f32| g::sample_at(x, first, f, left).clamp(0, n.max(1) as i64 - 1) as usize;
        let mut tops = Vec::with_capacity(rows.len());
        let mut y = client.top() + vscroll;
        for r in &rows {
            tops.push(y);
            y += r.h;
        }
        let ranges: Vec<(f64, f64)> = rows.iter().map(|r| row_range(doc, r)).collect();
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
            lp.px_hline(client.left()..=plot_right, tops[li] + r.h - 1.0, Stroke::new(1.0f32, Color32::from_rgb(200, 200, 200)));
        }
        if show_grid {
            let mut x = left + 50.0;
            while x < plot_right {
                painter.px_vline(x, client.top()..=grid_bottom, Stroke::new(1.0f32, Color32::from_rgb(230, 230, 230)));
                x += 50.0;
            }
        }
        painter.px_vline(left, client.top()..=grid_bottom, Stroke::new(1.0f32, Color32::from_rgb(100, 100, 100)));

        // ---- stats panel and waveforms ----
        let k_main = doc.main_cursor;
        let mut rename_rect = None;
        let mut cut_hover: Option<String> = None;
        let hover = resp.hover_pos();
        let ov = OscView { doc, unit: time_unit, f, first, n, left, right: plot_right, x0: client.left(), label_col };
        for (li, r) in rows.iter().enumerate() {
            let (top, h) = (tops[li], r.h);
            if top > plot_bottom || top + h < client.top() {
                continue;
            }
            let name_rect = ov.row(&lp, &pp, r, top, ranges[li]);
            let lane = &r.key;
            let name = lane_stats(doc, lane, k_main).0;
            if name.chars().count() > NAME_CHARS && hover.is_some_and(|p| name_rect.contains(p)) {
                cut_hover = Some(name.clone());
            }
            if let (Lane::Analog(i), Some((ri, _))) = (lane, &self.rename_edit) {
                if i == ri {
                    rename_rect = Some(name_rect.expand2(Vec2::new(2.0, 1.0)));
                }
            }
        }

        // ---- time ruler (`0x464350`) and difference band (`0x466630`) ----
        let ruler_top = client.bottom() - g::RULER_H;
        ov.ruler(&painter, client.bottom(), plot_right, band);
        let t0 = doc.osc.time.first().copied().unwrap_or(0.0);
        let time_at = |k: usize| doc.osc.time.get(k).copied().unwrap_or(0.0) - t0;
        let viz = [
            (doc.main_cursor, doc.v_shown[0], Color32::from_rgb(255, 100, 100), "В1"),
            (doc.aux_cursor, doc.v_shown[1], Color32::from_rgb(100, 100, 255), "В2"),
            (doc.v3_cursor, doc.v_shown[2], Color32::from_rgb(0, 160, 0), "В3"),
        ];
        if doc.diff_band {
            ov.band(&painter, ruler_top, plot_right, show_grid);
        }
        // The ruler's last step (`0x465302`): the stats / plot separator
        // again, down through the band to the ruler.
        painter.px_vline(left, client.top()..=ruler_top, Stroke::new(1.0f32, Color32::from_rgb(100, 100, 100)));

        // ---- визиры (`0x493110`): no shading between В1 and В2 ----
        let v_bottom = if doc.diff_band { ruler_top - 25.0 } else { ruler_top };
        let mut level_rect: Option<Rect> = None;
        ov.vizirs(&painter, client.top(), v_bottom, true);
        let mut hrows: [Option<(f32, usize)>; 2] = [None, None];
        for (j, h) in doc.hviz.iter().enumerate() {
            let Some(ch) = h.chan else { continue };
            let Some(li) = rows.iter().position(|r| r.has(Lane::Analog(ch))) else { continue };
            let (lo, hi) = ranges[li];
            if h.value.is_nan() {
                // First show: 10 / 20 px from the top of its lane.
                let v = g::y_value(tops[li] + 10.0 * (j as f32 + 1.0), tops[li], rows[li].h, lo, hi);
                actions.push(Act::SetH(j, ch, v));
                continue;
            }
            let yy = g::value_y(h.value, tops[li], rows[li].h, lo, hi);
            hrows[j] = Some((yy, li));
        }
        paint_hvizirs(&painter, (client.left(), left, plot_right), (client.top(), plot_bottom), [0, 1].map(|j| (doc.hviz[j].shown, hrows[j].map(|r| r.0))));

        // ---- comments (`0x43f630`), the one under the pointer selected ----
        let lane_of = |k: ChanKey| {
            rows.iter().position(|r| r.has(k)).map(|li| (tops[li], rows[li].h))
        };
        // A level line (`0x471020` / `0x470e4a`) uses its channel's current
        // range — a digital channel its raw 0 / 1 window, the line snapped —
        // outside it the line is neither drawn nor hit.
        let level_y = |k: ChanKey, v: f64| level_place(doc, (&rows, &tops, &ranges), k, v);
        let us_at = |k: usize| g::micros(time_at(k));
        let abs_at = |dt: i64| fvb_core::comtrade::moment_time_text(&doc.osc.stamps, doc.osc.rev_year, dt);
        let cgeo = crate::comments::Geo { x_of: &x_of, lane: &lane_of, value_y: &level_y, top: client.top(), v_bottom, us: &us_at, abs: &abs_at };
        let comment_at = |pos: Pos2| {
            doc.comments.iter().enumerate().find_map(|(i, c)| crate::comments::hit(c, pos, &cgeo).map(|part| (i, part)))
        };
        // The drag / cursor test (`0x43fab0`): the first unlocked element
        // with a grabbable part (an area's top and bottom border have none).
        let part_at = |pos: Pos2| {
            doc.comments.iter().filter(|c| !c.locked).find_map(|c| crate::comments::hit(c, pos, &cgeo).filter(|&part| part != 0))
        };
        // A pointer move over the client right of the stats panel with no
        // drag under way re-picks the element under it (`0x45ef09`); it is
        // kept while the pointer is elsewhere (no mouse-leave handler).
        let pointer_now = hover.filter(|p| client.contains(*p));
        let idle = self.vizir_drag.is_none() && self.comment_drag.is_none() && self.merge_drag.is_none() && !matches!(self.rect_zoom, RectZoom::Dragging { .. });
        // A popup menu is modal: the view gets no mouse moves under it, so
        // the element stays picked through the menu and after it (VM: the
        // level stays selected for «Изменить уровень»).
        let hot = match pointer_now.filter(|p| idle && !menu_busy && p.x >= left && self.view_pointer != Some(*p)) {
            Some(p) => {
                let h = comment_at(p).map(|h| h.0);
                if h != doc.comment_hot {
                    actions.push(Act::Hot(h));
                }
                h
            }
            None => doc.comment_hot,
        };
        let hovered = self.comment_drag.map(|(i, _)| i).or(hot);
        crate::comments::paint(&pp, &doc.comments, hovered, &cgeo);
        if let Some((i, _)) = &self.level_edit {
            if let Some(crate::comments::CommentKind::Level { chan, value, from, .. }) = doc.comments.get(*i).map(|c| &c.kind) {
                if let Some(y) = level_y(*chan, *value).map(|l| l.draw) {
                    // `0x45ce40` after `0x440530`: left max(x(from), leftW),
                    // top y + 3, 25 × the width of "A", 1.2 × the text height.
                    let (aw, th) = ui.fonts(|fo| {
                        let g = fo.layout_no_wrap("A".into(), ROW_FONT, Color32::BLACK);
                        (g.size().x, g.size().y)
                    });
                    // The view's scroll is not added (VM: in a scrolled view
                    // the edit sits that far below the line).
                    level_rect = Some(Rect::from_min_size(Pos2::new(x_of(*from as f64).max(left), y - vscroll + 3.0), Vec2::new(25.0 * aw, (1.2 * th).trunc())));
                }
            }
        }

        // ---- scrollbars (line and page step: one screen / 100 px) ----
        // Window scroll bars: H along the whole bottom, V down the whole
        // right side, the corner between them blank — each only while there
        // is something to scroll.
        let hmax = g::max_first(n, visible);
        if hbar_on {
            let hbar = Rect::from_min_max(Pos2::new(full.left(), client.bottom()), Pos2::new(plot_right, full.bottom()));
            if let Some(v) = scrollbar(ui, hbar, true, first as f32, hmax as f32, visible as f32, visible as f32, visible as f32) {
                actions.push(Act::SetFirst(v.round().max(0.0) as usize));
            }
        }
        if vbar {
            ui.painter().rect_filled(Rect::from_min_max(Pos2::new(plot_right, client.bottom()), full.max), 0.0, Color32::from_gray(236));
            let vrect = Rect::from_min_max(Pos2::new(plot_right, client.top()), Pos2::new(client.right(), client.bottom()));
            let page = (client.height() - band).max(0.0);
            // WM_VSCROLL `0x45d870`: a page is a line, 100 px.
            if let Some(v) = scrollbar(ui, vrect, false, -vscroll, -vmin, page, VSCROLL_LINE, VSCROLL_LINE) {
                actions.push(Act::SetVScroll(-v));
            }
        }

        // ---- input (`0x45dc10`, `0x45ea70`, `0x45e6b0`, `0x45efa0`) ----
        let geo = RowGeo { left, axis_top: plot_bottom, tops: &tops, rows: &rows };
        let (shift, ctrl, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.command || i.modifiers.ctrl, i.modifiers.alt));
        // Hit tests (`0x495800`, on integer pixels): a визир at
        // trunc(leftW + (s − first)·25·f) — not the painter's column search —
        // within |Δ| < 3; a ГВ at its drawn y, likewise.
        let x_rel_left = left - client.left();
        let v_hit = |x: f32| {
            let px = (x - client.left()).floor();
            (0..3).find(|&k| viz[k].1 && (px - (x_rel_left as f64 + (viz[k].0 as f64 - first as f64) * g::PX_PER_SAMPLE * f).trunc() as f32).abs() < 3.0)
        };
        let h_hit = |y: f32| (0..2).find(|&j| doc.hviz[j].shown && hrows[j].is_some_and(|(hy, _)| (hy.floor() - y.floor()).abs() < 3.0));
        // Whether `y` is over a pending (pasted, not yet merged) lane.
        let pending_at = |y: f32| {
            rows.iter().zip(&tops).any(|(r, &t)| {
                y >= t && y < t + r.h && matches!(r.key, Lane::Computed(i) if doc.computed[i].formula == crate::merge_ui::PENDING)
            })
        };
        let (pressed_l, pressed_r, press_pos) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.secondary_pressed(), press_pos(i)));
        let in_view = |p: Pos2| client.contains(p) && p.x < plot_right;
        if !menu_busy && (pressed_l || pressed_r) {
            if let Some(pos) = press_pos.filter(|p| in_view(*p) && resp.hovered()) {
                match hit_test(pos, &geo) {
                    Hit::Axis => actions.push(Act::ClearSelection),
                    Hit::Stats(row) => {
                        if self.rect_zoom != RectZoom::Off {
                            actions.push(Act::RectZoom(RectZoom::Off));
                        }
                        let how = if pressed_r {
                            SelClick::Right
                        } else if shift && ctrl {
                            SelClick::CtrlShift
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
                        } else if let Some(part) = part_at(pos).filter(|_| pressed_l) {
                            // An unlocked element's part under the pointer: drag
                            // the selected element (`0x43efa0` moves `+0x854`,
                            // whatever the text flag); before a pending lane.
                            if let Some(ci) = hot {
                                actions.push(Act::CommentDrag(ci, part));
                            }
                        } else if pressed_l && pending_at(pos.y) {
                            // A pending pasted lane: drag it in time (mode 6).
                            actions.push(Act::MergeDrag(Some((pos.x, doc.merge_offset))));
                        } else if doc.comment_mode != 0 && (!pressed_l || hot.is_some()) {
                            // A tool is armed: any other press does nothing.
                        } else if doc.comment_mode != 0 {
                            // Placement tools (`0x43eb40`, LMB, nothing selected):
                            // a moment in one click; a level line on an analog
                            // lane and an area on any lane are dragged out from
                            // the press.
                            use crate::comments::{Comment, CommentKind};
                            let lane = rows.iter().zip(&tops).position(|(r, &t)| pos.y >= t && pos.y < t + r.h);
                            match doc.comment_mode {
                                1 => actions.push(Act::PlaceComment(Comment::new(CommentKind::Moment { pos: k }), false)),
                                2 => {
                                    if let Some((chan, value)) = level_lane_at(doc, pos.y, &geo, &ranges) {
                                        actions.push(Act::PlaceComment(Comment::new(CommentKind::Level { chan, value, from: k, to: k }), true));
                                    }
                                }
                                _ => {
                                    if let Some(li) = lane {
                                        let key = rows[li].key;
                                        actions.push(Act::PlaceComment(Comment::new(CommentKind::Area { chan_from: key, chan_to: key, from: k, to: k }), true));
                                    }
                                }
                            }
                        } else if shift {
                            if let Some((ch, v)) = analog_at(pos.y, &geo, &ranges) {
                                actions.push(Act::SetH(usize::from(pressed_r), ch, v));
                            }
                        } else if pressed_l && self.rect_zoom == RectZoom::Armed {
                            // 138 armed: the press on an analog lane starts the
                            // rectangle (mode 8).
                            if let Some((chan, _)) = analog_at(pos.y, &geo, &ranges) {
                                actions.push(Act::RectZoom(RectZoom::Dragging { chan, x: pos.x, y0: pos.y, y1: pos.y }));
                            }
                        } else if hot.is_none() {
                            // `0x45e65d`: no визир under a selected element.
                            actions.push(Act::PlaceV(usize::from(pressed_r), k));
                        }
                    }
                }
            }
        }
        if let (RectZoom::Dragging { chan, x, y0, .. }, Some(pos)) = (self.rect_zoom, ui.input(|i| i.pointer.interact_pos())) {
            if let Some(li) = rows.iter().position(|r| r.has(Lane::Analog(chan))) {
                let (top, h) = (tops[li], rows[li].h);
                // The end stays on the lane (`0x45ec4e`: top..top + h − 1).
                let y1 = pos.y.clamp(top, top + h - 1.0);
                if ui.input(|i| i.pointer.primary_down()) {
                    actions.push(Act::RectZoom(RectZoom::Dragging { chan, x, y0, y1 }));
                    // The rubber line with ±5 px end ticks.
                    let st = Stroke::new(1.0f32, Color32::BLACK);
                    painter.px_vline(x, y0.min(y1)..=y0.max(y1), st);
                    painter.px_hline(x - 5.0..=x + 5.0, y0, st);
                    painter.px_hline(x - 5.0..=x + 5.0, y1, st);
                } else {
                    let (a, b) = (((y0.min(y1) - top) / h) as f64, ((y0.max(y1) - top) / h) as f64);
                    actions.push(Act::WindowZoom(chan, a, b));
                }
            }
        }
        if let (Some((x0, off0)), Some(pos)) = (self.merge_drag, ui.input(|i| i.pointer.interact_pos())) {
            if ui.input(|i| i.pointer.primary_down()) {
                let px_per_s = g::PX_PER_SAMPLE * f * doc.osc.sample_rate.max(1.0);
                actions.push(Act::MergeOffset(off0 + f64::from(pos.x - x0) / px_per_s));
            } else {
                actions.push(Act::MergeDrag(None));
            }
        }
        if let (Some((ci, part)), Some(pos)) = (self.comment_drag, ui.input(|i| i.pointer.interact_pos())) {
            if ui.input(|i| i.pointer.primary_down()) {
                let k = sample_at(pos.x.clamp(left, plot_right.max(left)));
                let under = rows.iter().zip(&tops).position(|(r, &t)| pos.y >= t && pos.y < t + r.h).map(|li| {
                    let v = analog_kind(doc, rows[li].key).then(|| g::y_value(pos.y, tops[li], rows[li].h, ranges[li].0, ranges[li].1));
                    (rows[li].key, v)
                });
                actions.push(Act::MoveComment(ci, part, k, under));
            } else {
                actions.push(Act::EndCommentDrag);
            }
        }
        if let (Some(drag), Some(pos)) = (self.vizir_drag, ui.input(|i| i.pointer.interact_pos())) {
            if ui.input(|i| i.pointer.primary_down()) {
                match drag {
                    VizDrag::V(k) => actions.push(Act::MoveV(k, sample_at(pos.x.clamp(left, plot_right.max(left))))),
                    VizDrag::H(j) => {
                        if let Some((ch, v)) = analog_at(pos.y.clamp(client.top(), (plot_bottom - 1.0).max(client.top())), &geo, &ranges) {
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
                    // `0x465890`: the selected element, right of the stats.
                    Hit::Plot | Hit::Axis if pos.x >= left && hot.is_some() => actions.push(Act::EditComment(hot.unwrap_or_default())),
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
                        if let Some(idx) = hot {
                            actions.push(Act::OpenMenu(pos, LaneMenuKind::Comment(idx)));
                        }
                    }
                }
            }
        }
        // Stats-panel drag (`0x45ea70`): a pointer move over the panel with
        // LMB held, no Ctrl / RMB, no drag mode and selected file analog
        // channels calls DoDragDrop, whose wait loop (MFC's
        // `COleDropSource::OnBeginDrag`) starts the drag once the pointer
        // leaves the 2 px rectangle around where it was, or after 200 ms with
        // the button held; a release, another press or Esc cancels it (a
        // later move arms it again). The payload is taken at the move.
        let ctx = ui.ctx().clone();
        let serial = doc.serial;
        let (l_down, r_down, origin, now, released, t, moved, esc) = ui.input(|i| {
            let p = &i.pointer;
            (p.primary_down(), p.secondary_down(), p.press_origin(), p.latest_pos(), p.primary_released(), i.time, p.delta() != Vec2::ZERO, i.key_pressed(egui::Key::Escape))
        });
        if !l_down || r_down || esc || egui::DragAndDrop::has_any_payload(&ctx) {
            // A release that comes after the 200 ms with the button held
            // (a slow frame: the move that armed and the release in
            // consecutive frames) finds the loop's drag already started:
            // it drops.
            if let Some((_, t0, chans)) = self.stats_drag_arm.take().filter(|_| released && !r_down && !esc && !egui::DragAndDrop::has_any_payload(&ctx)) {
                if t - t0 > DRAG_DELAY {
                    egui::DragAndDrop::set_payload(&ctx, ChanDrag::of(chans, serial));
                }
            }
        } else if let Some((a, t0, chans)) = self.stats_drag_arm.take() {
            let p = now.unwrap_or(a);
            let (dx, dy) = ((p.x - a.x).floor(), (p.y - a.y).floor());
            if !(-2.0..2.0).contains(&dx) || !(-2.0..2.0).contains(&dy) || t - t0 > DRAG_DELAY {
                egui::DragAndDrop::set_payload(&ctx, ChanDrag::of(chans, serial));
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(DRAG_DELAY - (t - t0) + 0.001));
                self.stats_drag_arm = Some((a, t0, chans));
            }
        } else if moved && !ctrl && !menu_busy && self.rename_edit.is_none() && resp.is_pointer_button_down_on() {
            let mode0 = self.vizir_drag.is_none() && self.comment_drag.is_none() && self.merge_drag.is_none() && !matches!(self.rect_zoom, RectZoom::Dragging { .. });
            if let (Some(o), Some(p)) = (origin, now) {
                // Every selected analog-kind record goes (`0x45edc4`, bound
                // `doc+0x1d8`), pasted analog lanes too; digital ones lie
                // past the bound.
                let sel: Vec<ChanKey> = doc
                    .selected_analog()
                    .into_iter()
                    .map(ChanKey::Analog)
                    .chain(doc.selected_computed().into_iter().filter(|&j| !doc.pending_digital(j)).map(ChanKey::Computed))
                    .collect();
                if mode0 && in_view(o) && p.x < left && !sel.is_empty() {
                    self.stats_drag_arm = Some((p, t, sel));
                }
            }
        }
        // Drag auto-scroll during the channel drag only (COleDropTarget::
        // OnDragScroll, mfc120u `0x102172dd`; the view has no timer of its
        // own): inside the client but within 11 px of an edge, a code of a
        // horizontal (left / right) and a vertical (top / bottom) line
        // part; a new code restarts the 50 ms delay, a step comes once
        // strictly more than it has passed, then every 50 ms. The view's
        // OnScroll `0x468e40` drops the horizontal part and scrolls up only
        // for a vertical line-up — everything else down (`0x468f00`,
        // WM_VSCROLL, 100 px): the side bands scroll down.
        let code = now.filter(|p| client.contains(*p) && egui::DragAndDrop::has_payload_of_type::<ChanDrag>(&ctx)).and_then(|p| {
            let h = if p.x < client.left() + DRAG_INSET { 0u8 } else if p.x >= client.right() - DRAG_INSET { 1 } else { 0xff };
            let v = if p.y < client.top() + DRAG_INSET { 0u8 } else if p.y >= client.bottom() - DRAG_INSET { 1 } else { 0xff };
            (h != 0xff || v != 0xff).then_some(u16::from(v) << 8 | u16::from(h))
        });
        match code {
            Some(code) => {
                if self.drag_scroll.is_none_or(|d| d.0 != code) {
                    self.drag_scroll = Some((code, t));
                }
                if let Some((_, last)) = self.drag_scroll.as_mut() {
                    if t - *last > DRAG_SCROLL_S + 1e-9 {
                        *last = t;
                        let up = code >> 8 == 0;
                        actions.push(Act::SetVScroll(vscroll + if up { VSCROLL_LINE } else { -VSCROLL_LINE }));
                    }
                }
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(DRAG_SCROLL_S));
            }
            None => self.drag_scroll = None,
        }
        // Drop on a lane (`0x4415b0`, kind 1): x is ignored; the analog-kind
        // lane under y (the search stops at `doc+0x1d8`: a digital lane is
        // no target and nothing happens), unless it is one of the dragged
        // channels, joins them; a pasted one is refused by 33035's enable
        // check (`0x468c80`): message 1086.
        // The window under the pointer takes the drop, not the lane under
        // it: a floating or slid-out pane's window over the view (its
        // caption, tab strip, toolbar; a diagram of it takes the drop as a
        // pane's) leaves the lane out (clone).
        let over_float = |p: Pos2| ctx.layer_id_at(p).is_some_and(crate::views::is_pane_window_layer);
        if released && !self.modal_up {
            if let Some(p) = now.filter(|p| in_view(*p) && p.y < plot_bottom && !over_float(*p)) {
                let lane = rows.iter().zip(&tops).find(|(r, &t)| p.y >= t && p.y < t + r.h).map(|(r, _)| r.key);
                if egui::DragAndDrop::has_payload_of_type::<ChanDrag>(&ctx) {
                    if let Some(drag) = egui::DragAndDrop::take_payload::<ChanDrag>(&ctx) {
                        // Another record's view: nothing (#74); the payload
                        // holds channel numbers of the record it began in.
                        let target = lane.filter(|_| drag.doc == serial).filter(|k| match k {
                            Lane::Digital(_) => false,
                            Lane::Computed(j) if doc.pending_digital(*j) => false,
                            _ => !drag.keys().contains(k),
                        });
                        if let Some(target) = target {
                            actions.push(Act::DropCombine(drag.keys(), target));
                        }
                    }
                }
            }
        }
        // The cursor over the view (#73): the move-drop one over the view
        // window of the record the drag began in, "copy" with Ctrl inside
        // the auto-scroll band (MFC's OnDragScroll), "no-drop" over another
        // record's view and under a window lying over it.
        if let (Some(_), Some(drag)) = (now.filter(|p| resp.rect.contains(*p) && !over_float(*p)), egui::DragAndDrop::payload::<ChanDrag>(&ctx)) {
            self.drag_cursor = Some(if drag.doc != serial {
                egui::CursorIcon::NoDrop
            } else if ctrl && code.is_some() {
                // MFC's OnDragScroll answers "link" for Ctrl+Shift, which
                // the drag does not allow: no-drop.
                if shift { egui::CursorIcon::NoDrop } else { egui::CursorIcon::Copy }
            } else {
                MOVE_DROP_CURSOR
            });
        }
        // Wheel (`0x45d990`), per WM_MOUSEWHEEL (one per wheel event): Alt
        // = lane height, exactly Ctrl (no button held) = time zoom, exactly
        // Shift = a page of the horizontal scroll bar sideways — one step
        // each, whatever the delta —, else the lanes scroll by the raw delta
        // in pixels (120 a notch); see [`hscroll_page`].

        if resp.hovered() && !menu_busy {
            let (wheels, buttons) = ui.input(|i| (wheel_deltas(&i.events), i.pointer.any_down()));
            let (mut first_w, mut vscroll_w) = (first as i64, vscroll);
            for delta in wheels {
                let up = delta > 0.0;
                if alt {
                    actions.push(Act::Cmd(Command::ZoomY(up)));
                } else if ctrl && !shift && !buttons {
                    actions.push(Act::Cmd(Command::ZoomX(up)));
                } else if shift && !ctrl && !buttons {
                    first_w = (hscroll_page(first_w.max(0) as usize, n, visible, up) as i64).clamp(0, hmax as i64);
                    actions.push(Act::SetFirst(first_w as usize));
                } else {
                    vscroll_w += delta;
                    actions.push(Act::SetVScroll(vscroll_w));
                }
            }
        }
        // Keys (`0x45f490`, the view's WM_KEYDOWN — so only while the view
        // has the focus, and never with Alt, a WM_SYSKEYDOWN): ←/→ move В1
        // (Ctrl: В2) a sample, ↑/↓ move ГВ1 (Ctrl: ГВ2) a pixel; the setter
        // (`0x492f20`) shows the визир. Ours caps → at N − 1 (the original
        // at N, `0x45f52f`, a sample past the data).
        let view_keys = self.copy_focus == crate::app::CopyFocus::View;
        let popup_up = self.pane_menu.is_some() || self.diag.menu.is_some();
        if view_keys && !menu_busy && !popup_up && !ui.ctx().wants_keyboard_input() && self.rename_edit.is_none() && !self.modal_up {
            // Each press is a WM_KEYDOWN, with the modifiers it came with
            // (several in one slow frame each move; Ctrl may be up again
            // by the frame's end).
            use egui::Key::{ArrowDown, ArrowLeft, ArrowRight, ArrowUp};
            let keys: Vec<(egui::Key, egui::Modifiers)> = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Key { key: k @ (ArrowLeft | ArrowRight | ArrowUp | ArrowDown), pressed: true, modifiers, .. } if !modifiers.alt => Some((*k, *modifiers)),
                        _ => None,
                    })
                    .collect()
            });
            let mut cur = [None::<i64>; 2];
            let mut y = [hrows[0].map(|(hy, _)| hy), hrows[1].map(|(hy, _)| hy)];
            let mut moved_h = [false; 2];
            for (k, km) in keys {
                let vk = usize::from(km.ctrl);
                match k {
                    ArrowLeft | ArrowRight => {
                        let c = cur[vk].unwrap_or(viz[vk].0 as i64) + if k == ArrowRight { 1 } else { -1 };
                        cur[vk] = Some(c.clamp(0, n.max(1) as i64 - 1));
                    }
                    _ => {
                        // y ∓ 1, ↑ clamped at the view's top; a ГВ never
                        // placed has y = −1 (`0x4944a0`), so either key puts
                        // it at the top.
                        y[vk] = Some(match y[vk] {
                            Some(hy) if k == ArrowDown => hy + 1.0,
                            Some(hy) => (hy - 1.0).max(client.top()),
                            None => client.top(),
                        });
                        moved_h[vk] = true;
                    }
                }
            }
            for vk in 0..2 {
                if let Some(c) = cur[vk] {
                    actions.push(Act::MoveV(vk, c as usize));
                }
                if let Some(y) = y[vk].filter(|_| moved_h[vk]) {
                    if let Some((ch, v)) = analog_at(y, &geo, &ranges) {
                        actions.push(Act::SetH(vk, ch, v));
                    }
                }
            }
        }
        // Cursor shapes (WM_SETCURSOR `0x45f660`, anywhere on the view): the
        // hand for a merge drag or a pending lane wins; then the визир /
        // comment resize cursors; then the rectangle-zoom arrow.
        if let Some(pos) = hover.filter(|p| client.contains(*p)) {
            let icon = if self.merge_drag.is_some() || pending_at(pos.y) {
                Some(egui::CursorIcon::Grab)
            } else if matches!(self.vizir_drag, Some(VizDrag::V(_))) || v_hit(pos.x).is_some() || part_at(pos).is_some_and(|p| p != 4) {
                Some(egui::CursorIcon::ResizeHorizontal)
            } else if matches!(self.vizir_drag, Some(VizDrag::H(_))) || h_hit(pos.y).is_some() || part_at(pos) == Some(4) {
                Some(egui::CursorIcon::ResizeVertical)
            } else if self.rect_zoom != RectZoom::Off {
                Some(egui::CursorIcon::ResizeNorth)
            } else {
                None
            };
            if let Some(i) = icon {
                ui.ctx().set_cursor_icon(i);
            }
        }
        let row_keys: Vec<ChanKey> = rows.iter().map(|r| r.key).collect();
        let (vmin_c, hmax_c) = (vmin, hmax);

        if pointer_now.is_some() {
            self.view_pointer = pointer_now;
        } else if menu_busy {
            // Under the popup the view's own hover is empty: the pointer
            // still counts as seen there, so closing the menu is no move.
            if let Some(p) = ui.input(|i| i.pointer.hover_pos()).filter(|p| client.contains(*p)) {
                self.view_pointer = Some(p);
            }
        }
        let v_before = self.docs.get(ai).map(|d| d.vscroll);
        let sel_before = self.docs.get(ai).map(|d| d.selection.clone());
        let paste_dragged = actions.iter().any(|a| matches!(a, Act::MergeOffset(_)));
        for a in actions {
            let d = &mut self.docs[ai];
            match a {
                Act::PlaceV(k, s) => {
                    match k {
                        0 => d.main_cursor = s,
                        _ => d.aux_cursor = s,
                    }
                    d.v_shown[k] = true;
                    d.v1_placed |= k == 0;
                }
                Act::MoveV(k, s) => {
                    match k {
                        0 => d.main_cursor = s,
                        1 => d.aux_cursor = s,
                        _ => d.v3_cursor = s,
                    }
                    d.v_shown[k] = true;
                    d.v1_placed |= k == 0;
                }
                Act::SetH(j, ch, v) => {
                    d.hviz[j] = crate::app::HVizir { shown: true, chan: Some(ch), value: v };
                }
                Act::Drag(k) => self.vizir_drag = Some(k),
                Act::EndDrag => self.vizir_drag = None,
                Act::SetFirst(v) => d.first = v.min(hmax_c),
                Act::SetVScroll(v) => d.vscroll = v.clamp(vmin_c, 0.0),
                Act::PlaceComment(c, drag) => {
                    // The tools are one-shot: a moment ends here, a level line or
                    // an area when the button is released.
                    d.comments.push(c);
                    if drag {
                        self.comment_drag = Some((d.comments.len() - 1, 1));
                    } else {
                        d.comment_mode = 0;
                    }
                }
                Act::CommentDrag(i, part) => self.comment_drag = Some((i, part)),
                Act::RectZoom(z) => self.rect_zoom = z,
                Act::MergeDrag(s) => self.merge_drag = s,
                Act::MergeOffset(o) => d.set_merge_offset(o),
                Act::WindowZoom(chan, a, b) => {
                    let dd: &crate::app::Doc = d;
                    let cur: Vec<(f64, f64)> = (0..dd.chans.len()).map(|i| chan_range(dd, i)).collect();
                    d.apply_window_zoom(chan, a, b, &|i| cur[i]);
                    self.rect_zoom = RectZoom::Off;
                }
                Act::MoveComment(i, part, k, under) => {
                    if let Some(c) = d.comments.get_mut(i) {
                        c.drag(part, k, under);
                    }
                }
                Act::EndCommentDrag => {
                    if let Some((i, part)) = self.comment_drag.take() {
                        if let Some(c) = d.comments.get_mut(i) {
                            c.normalise();
                        }
                        // The element stays the selected one (VM: a level
                        // just drawn keeps its 3 px line and its info box
                        // with the pointer still).
                        d.comment_hot = Some(i);
                        if part == 1 {
                            d.comment_mode = 0;
                        }
                    }
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
                Act::Hot(h) => d.comment_hot = h,
                Act::DropCombine(chans, target) => {
                    // The dragged channels and the target are selected (other
                    // selections stay), then 33035 or message 1086.
                    d.selection.extend(chans.iter().chain([&target]).copied());
                    if d.can_combine() {
                        if d.combine_selected() {
                            // 33035 reaches the refresh-all (`0x42f4b0` from `0x467a62`).
                            self.refresh_blocks();
                        } else {
                            // VM: after the box (and a repaint) the target stays selected beside the dragged lane.
                            self.dlg.push_alert_icon(MSG_COMBINE_RATES, crate::dialogs::MsgIcon::Error);
                        }
                    } else {
                        self.dlg.push_alert(MSG_COMBINE_UNITS.to_string());
                    }
                }
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
        // The view's OnScrollBy (`0x468f00`) ends in the refresh-all (`0x468f3d`), as does its mouse routine
        // `0x45dc10` once a click has selected a channel (`0x45e312`) and a mouse-move branch after `0x45a820`
        // (`0x45ec2a`: case 6 of the drag-mode switch, the pasted lanes' drag in time, `0x45a820`).
        if self.docs.get(ai).map(|d| d.vscroll) != v_before || self.docs.get(ai).map(|d| d.selection.clone()) != sel_before || paste_dragged {
            self.refresh_blocks();
        }

        // Hover popup over a cut name (`0x469060`): the full text.
        if let Some(full) = cut_hover {
            egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new("fv_cut_name"), |ui| ui.label(full));
        }
        self.rename_ui(ui, ai, rename_rect);
        self.level_edit_ui(ui, ai, level_rect);
        self.lane_menu_ui(ui.ctx(), ai);
        if self.lane_menu_eat && !ui.input(|i| i.pointer.any_down()) {
            self.lane_menu_eat = false;
        }
    }

    /// The inline channel-rename edit over the name badge (`0x465890`):
    /// Enter commits and Esc cancels (`0x470ae0`); a left, right or middle
    /// press anywhere in the view outside the edit commits (the view's
    /// PreTranslateMessage `0x466100`) — losing the focus otherwise (the
    /// ribbon, another window) leaves it open (`0x470ac0` does nothing); a
    /// name in use is refused with message 397.
    fn rename_ui(&mut self, ui: &mut egui::Ui, ai: usize, at: Option<Rect>) {
        let Some((i, mut text)) = self.rename_edit.take() else { return };
        let Some(at) = at else { return };
        let mut done: Option<bool> = None;
        let view = ui.max_rect();
        let opened = egui::Id::new(("fv_rename_opened", i));
        let mut edit_rect = at;
        egui::Area::new(egui::Id::new("fv_rename").with(i)).order(egui::Order::Foreground).fixed_pos(at.min).show(ui.ctx(), |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(at.width().max(120.0)));
            edit_rect = r.rect;
            // The edit takes the focus when it opens, not again.
            if ui.data(|d| d.get_temp::<bool>(opened).is_none()) {
                r.request_focus();
                ui.data_mut(|d| d.insert_temp(opened, true));
            }
            let keys = r.has_focus() || r.lost_focus();
            if edit_key_at_release(ui, opened, egui::Key::Escape, keys) {
                done = Some(false);
            } else if edit_key_at_release(ui, opened, egui::Key::Enter, keys) {
                done = Some(true);
            }
        });
        let press = ui.input(|i| i.pointer.any_pressed().then(|| i.pointer.interact_pos()).flatten());
        if done.is_none() && press.is_some_and(|p| view.contains(p) && !edit_rect.contains(p)) {
            done = Some(true);
        }
        if done.is_some() {
            ui.data_mut(|d| d.remove::<bool>(opened));
            edit_keys_disarm(ui, opened);
        }
        match done {
            None => self.rename_edit = Some((i, text)),
            // Esc reaches the common tail of `0x465df0` (`0x46607c`): the refresh, no modified mark.
            Some(false) => self.refresh_blocks(),
            Some(true) => {
                // `0x465df0`: the text as typed (no trim) is the alias,
                // refused when another channel has it as its name or its
                // file's (case-sensitive) with message box 397 (MB_OK,
                // `0x4660ab`); an empty one shows the file's name again
                // (`0x425a30` → `0x425960`); the document is marked modified
                // either way (`0x466062`).
                let d = &mut self.docs[ai];
                let key = crate::app::ChanKey::Analog(i);
                if crate::chandlg::name_taken(d, key, &text) {
                    self.dlg.push_alert(crate::chandlg::MSG_NAME_TAKEN.to_string());
                } else {
                    // The levels and areas keep the names they were made on,
                    // as after a rename in «Каналы» (L9.227).
                    let recs: Vec<_> = d.comments.iter().map(|c| d.comment_rec(c)).collect();
                    d.ensure_name_raw();
                    d.osc.analog[i].id = crate::chandlg::shown_after_rename(d, key, &text);
                    d.mark_alias(key, !text.is_empty());
                    d.comments = recs.iter().map(|r| d.comment_of(r)).collect();
                }
                d.set.modified = true;
                // The rename `0x465df0` ends in the refresh-all (`0x46607c`).
                self.refresh_blocks();
            }
        }
    }

    fn open_comment_editor(&mut self, ai: usize, idx: usize) {
        if let Some(c) = self.docs.get(ai).and_then(|d| d.comments.get(idx)) {
            self.comment_edit = Some(CommentEdit { idx, text: c.text.clone() });
        }
    }

    /// «Изменить уровень» (33042): an edit over the level line pre-filled
    /// with `%.2f`. The commit (the level branch of `0x465df0` → `0x4404b0`)
    /// on Enter or a press elsewhere: `_wtof` of the text, stored only if the
    /// element selected NOW is a level whose channel resolves and the value
    /// lies within the channel's whole-record [min, max] — else dropped —;
    /// the document is marked modified either way. Esc just closes.
    fn level_edit_ui(&mut self, ui: &mut egui::Ui, ai: usize, at: Option<Rect>) {
        let Some((i, mut text)) = self.level_edit.take() else { return };
        let Some(at) = at else { return };
        let mut done: Option<bool> = None;
        let area = egui::Area::new(egui::Id::new("fv_level_edit").with(i)).order(egui::Order::Foreground).fixed_pos(at.min).show(ui.ctx(), |ui| {
            // A plain white field with a (100) line at its left, right and
            // bottom (VM).
            use crate::oscgeom::PixelLines;
            let (r, _) = ui.allocate_exact_size(at.size(), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(r, 0.0, Color32::WHITE);
            let ink = egui::Stroke::new(1.0f32, Color32::from_gray(100));
            p.px_hline(r.x_range(), r.bottom() - 1.0, ink);
            p.px_vline(r.left(), r.y_range(), ink);
            p.px_vline(r.right() - 1.0, r.y_range(), ink);
            let field = Rect::from_min_max(r.min + Vec2::new(2.0, 0.0), r.max - Vec2::new(2.0, 1.0));
            let r = egui::TextEdit::singleline(&mut text).frame(false).font(ROW_FONT).text_color(Color32::BLACK).margin(egui::Margin::ZERO).desired_width(field.width()).show(&mut ui.new_child(egui::UiBuilder::new().max_rect(field))).response;
            r.request_focus();
            // As the rename edit, at the key-up (clone, 33042: Enter held, the edit
            // stays; let go, it commits; Esc likewise).
            if edit_key_at_release(ui, egui::Id::new(("fv_level_edit_keys", i)), egui::Key::Escape, true) {
                done = Some(false);
            } else if edit_key_at_release(ui, egui::Id::new(("fv_level_edit_keys", i)), egui::Key::Enter, true) {
                done = Some(true);
            }
        });
        let pressed_out = ui.input(|inp| inp.pointer.any_pressed() && inp.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
        if done.is_none() && pressed_out {
            done = Some(true);
        }
        if done.is_some() {
            edit_keys_disarm(ui, egui::Id::new(("fv_level_edit_keys", i)));
        }
        match done {
            None => self.level_edit = Some((i, text)),
            Some(false) => {}
            Some(true) => {
                let v = crate::chandlg::wtof(&text);
                let d = &mut self.docs[ai];
                let target = d.comment_hot;
                let range = target.and_then(|t| d.comments.get(t)).and_then(|c| match c.kind {
                    crate::comments::CommentKind::Level { chan, .. } => level_range(d, chan),
                    _ => None,
                });
                if let (Some(t), Some((lo, hi))) = (target, range) {
                    if (lo..=hi).contains(&v) {
                        if let crate::comments::CommentKind::Level { value, .. } = &mut d.comments[t].kind {
                            *value = v;
                        }
                    }
                }
                d.set.modified = true;
            }
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
        // 57637 (`0x45bf88`): the clipboard format and no pending channel.
        let can_paste = self.clip.available() && !crate::omp_ui::has_pending_paste(doc);
        let (time_unit, grid, band) = (self.time_unit, self.show_grid, doc.diff_band);
        let mut chosen: Option<MenuCmd> = None;
        // The keyboard state: reset for a new menu, rows counted afresh.
        ctx.data_mut(|d| {
            let k: &mut MenuKeys = d.get_temp_mut_or_default(menu_keys_id());
            if k.serial != menu.serial {
                *k = MenuKeys { serial: menu.serial, ..Default::default() };
            }
            k.count = 0;
            k.enabled.clear();
        });
        let menu_id = egui::Id::new("fv_lane_menu").with(menu.serial);
        let area = egui::Area::new(menu_id)
            .kind(egui::UiKind::Menu)
            .order(egui::Order::Foreground)
            .fixed_pos(crate::popmenu::menu_pos(ctx, menu_id, menu.pos))
            .constrain_to(crate::popmenu::bounds(ctx, menu.pos))
            .show(ctx, |ui| {
                crate::popmenu::mfc_frame(ui, |ui| {
                    crate::popmenu::cut_to_screen(ui, menu_id, menu.pos, |ui| {
                        let mut pick = |c: MenuCmd| chosen = Some(c);
                        match menu.kind {
                            LaneMenuKind::Channels => channel_menu(ui, doc, can_paste, &mut pick),
                            LaneMenuKind::TimeAxis => time_axis_menu(ui, time_unit, grid, band, &mut pick),
                            LaneMenuKind::Comment(i) => comment_menu(ui, &doc.comments[i], i, &mut pick),
                        }
                    });
                });
            });
        let rect = area.response.rect;
        // ↑ / ↓ over the rows, the disabled ones too (wrapping; VM: a
        // grey bar on a greyed row); Enter picks an enabled one on the
        // next pass.
        // Taken from the input, as a Win32 menu loop eats its keys.
        let (up, down, enter) = ctx.input_mut(|i| {
            let mut k = |key| i.consume_key(egui::Modifiers::NONE, key);
            (k(egui::Key::ArrowUp), k(egui::Key::ArrowDown), k(egui::Key::Enter))
        });
        ctx.data_mut(|d| {
            let k: &mut MenuKeys = d.get_temp_mut_or_default(menu_keys_id());
            k.activate = false;
            let n = k.enabled.len();
            if (up || down) && n > 0 {
                let mut i = k.hot.unwrap_or(if down { n - 1 } else { 0 });
                i = if down { (i + 1) % n } else { (i + n - 1) % n };
                k.hot = Some(i);
            }
            if enter && k.hot.is_some() {
                k.activate = true;
            }
        });
        if up || down || enter {
            ctx.request_repaint();
        }
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
        // The picks whose handlers end in the refresh-all `0x475420`: the flags and value modes (the same
        // commands as the ribbon's, which `dispatch` refreshes), 33037 (`0x467e94`) and 33033 (`0x4557d0`); 33035
        // too, once it has combined (below). «Цвет канала» refreshes when its dialog is accepted.
        let refresh = matches!(cmd, MenuCmd::Flag(_) | MenuCmd::Mode(_) | MenuCmd::Split | MenuCmd::DiffBand);
        self.run_menu_cmd_inner(ai, cmd);
        if refresh {
            self.refresh_blocks();
        }
    }

    fn run_menu_cmd_inner(&mut self, ai: usize, cmd: MenuCmd) {
        match cmd {
            MenuCmd::CopyMerge => self.copy_channels_for_merge(),
            MenuCmd::PasteMerge => self.paste_channels_for_merge(),
            // 32853–32857 (`0x45caa0`) and 33007 (`0x45d290`): every pick —
            // the ticked one too — writes `View/Sells` and `View/RulerState`
            // and marks the document modified.
            MenuCmd::TimeUnit(_) | MenuCmd::Grid => {
                match cmd {
                    MenuCmd::TimeUnit(u) => self.time_unit = u,
                    _ => self.show_grid = !self.show_grid,
                }
                let (grid, ruler) = (self.show_grid, i32::from(crate::persist::ruler_state(self.time_unit)));
                if let Some(d) = self.docs.get_mut(ai) {
                    let v = d.set.tree.section_mut("View");
                    v.set_bool("Sells", grid);
                    v.set_int("RulerState", ruler);
                    d.set.modified = true;
                }
            }
            // 33033 (`0x4557d0`): a per-document toggle, no modified mark.
            MenuCmd::DiffBand => {
                if let Some(d) = self.docs.get_mut(ai) {
                    d.diff_band = !d.diff_band;
                }
            }
            // `0x4310b0`: always the dialog, from the first selected
            // channel's colour or black (OK then changes nothing).
            MenuCmd::Color => self.color_dialog = self.docs.get(ai).map(|d| d.first_selected_color().unwrap_or(egui::Color32::BLACK)),
            MenuCmd::EditComment(i) => self.open_comment_editor(ai, i),
            MenuCmd::EditLevel(i) => {
                if let Some(crate::comments::CommentKind::Level { value, .. }) = self.docs.get(ai).and_then(|d| d.comments.get(i)).map(|c| &c.kind) {
                    // An open edit is destroyed without its commit.
                    self.rename_edit = None;
                    self.level_edit = Some((i, format!("{value:.2}")));
                }
            }
            cmd => {
                let Some(d) = self.docs.get_mut(ai) else { return };
                match cmd {
                    MenuCmd::Flag(f) => {
                        d.toggle_flag_selected(f);
                        self.after_flag_command(f);
                    }
                    MenuCmd::Hide => d.hide_selected(),
                    MenuCmd::Mode(m) => {
                        d.set_mode_selected(m);
                        self.run_definition_batch();
                    }
                    MenuCmd::ResetZoom => d.reset_zoom_selected(),
                    MenuCmd::Combine => {
                        let enabled = d.can_combine();
                        if d.combine_selected() {
                            if enabled {
                                self.refresh_blocks();
                            }
                        } else {
                            self.dlg.push_alert_icon(MSG_COMBINE_RATES, crate::dialogs::MsgIcon::Error);
                        }
                    }
                    MenuCmd::Split => d.split_selected(),
                    MenuCmd::LockComment(i) => d.comments[i].locked = !d.comments[i].locked,
                    MenuCmd::DeleteComment(i) => {
                        // `0x43f5f0`: the selection goes with it.
                        d.comments.remove(i);
                        d.comment_hot = None;
                    }
                    MenuCmd::HideComment(i) => d.comments[i].hidden = !d.comments[i].hidden,
                    _ => {}
                }
            }
        }
    }

    /// The six status-bar panes (`0x4947d0`), each one complete string:
    /// the caption alone while its визир(s) are hidden, else the value
    /// relative to the trigger sample (0 for COMTRADE, L9.125) in
    /// the ruler unit; the V difference needs both V визиры shown, the H
    /// difference both H визиры on the same channel.
    /// `None` with no active document: the bar keeps what it last showed
    /// (nothing resets it, `0x472e10`).
    pub fn cursor_status(&self) -> Option<[String; 6]> {
        let captions = ["Верт. визир 1:", "Верт. визир 2:", "Разница. верт. визиров:", "Гор. визир 1:", "Гор. визир 2:", "Разница г. визиров:"];
        let mut out = captions.map(String::from);
        let d = self.active_doc()?;
        // Relative to the trigger sample (`0x49484d`; 0 for COMTRADE), the
        // absolute mode too (the start's time of day plus it).
        let trig = d.osc.trigger_sample;
        let t0 = d.osc.time.first().copied().unwrap_or(0.0);
        let tz = d.osc.time.get(trig).copied().unwrap_or(t0);
        let t = |k: usize| d.osc.time.get(k).copied().unwrap_or(0.0) - tz;
        let start_tod = fvb_core::comtrade::time_of_day(&d.osc.stamps.start_time);
        // Absolute mode (strings 240/241/202): V1/V2 as the time of day, the
        // difference as `ss.mmm` within the minute.
        // Whole µs (`0x454b40`); ms rounded half up (`0x411bf0`).
        let us = |dt: f64| (dt * 1e6).round();
        let time = |dt: f64, k: i64, diff: bool| match self.time_unit {
            TimeUnit::Micro => format!("{} мкс", us(dt) as i64),
            TimeUnit::Milli => format!("{} мс", (us(dt) / 1000.0 + 0.5).floor() as i64),
            TimeUnit::Sec => format!("{dt:.3} сек."),
            TimeUnit::Absolute if diff => g::status_abs_diff(dt),
            TimeUnit::Absolute => g::time_of_day_text(start_tod + dt),
            TimeUnit::Samples => format!("{k}"),
        };
        let (v1, v2) = (d.main_cursor, d.aux_cursor);
        if d.v_shown[0] {
            out[0] = format!("{} {}", captions[0], time(t(v1), v1 as i64 - trig as i64 + 1, false));
        }
        if d.v_shown[1] {
            out[1] = format!("{} {}", captions[1], time(t(v2), v2 as i64 - trig as i64 + 1, false));
        }
        if d.v_shown[0] && d.v_shown[1] {
            out[2] = format!("{} {}", captions[2], time((t(v1) - t(v2)).abs(), (v1 as i64 - v2 as i64).abs(), true));
        }
        // Absolute time with a start time that has no ':' (`0x494c31`):
        // the shown V panes stay empty.
        if self.time_unit == TimeUnit::Absolute && !d.osc.stamps.start_time.contains(':') {
            for (k, on) in [d.v_shown[0], d.v_shown[1], d.v_shown[0] && d.v_shown[1]].into_iter().enumerate() {
                if on {
                    out[k].clear();
                }
            }
        }
        let hval = |j: usize| -> Option<(usize, f64)> {
            let h = &d.hviz[j];
            (h.shown && h.value.is_finite()).then_some(())?;
            Some((h.chan?, h.value))
        };
        // «Гор. визир N: %4g %s» (3171 / 3173): the value as stored, the
        // plain unit (`0x42bc50`), none in ADC mode.
        let adc = |ch: usize| d.chans.get(ch).is_some_and(|c| c.adc_counts);
        let plain = |ch: usize| if adc(ch) { String::new() } else { d.osc.analog[ch].shown_unit().text };
        for j in 0..2 {
            if let Some((ch, v)) = hval(j) {
                out[3 + j] = format!("{} {:>4} {}", captions[3 + j], fvb_core::views::fmt_g(v), plain(ch));
            }
        }
        // «Разница г. визиров: %g %s%s» (3429, `0x4960f0`): SI-prefixed; in
        // ADC mode rounded half up with neither prefix nor unit.
        if let (Some((c1, a)), Some((c2, b))) = (hval(0), hval(1)) {
            if c1 == c2 {
                let dv = (a - b).abs();
                out[5] = if adc(c1) {
                    format!("{} {} ", captions[5], fvb_core::views::fmt_g((dv + 0.5).floor()))
                } else {
                    let (v, unit) = fvb_core::views::si_scaled(dv, d.osc.analog[c1].shown_unit());
                    format!("{} {} {unit}", captions[5], fvb_core::views::fmt_g(v))
                };
            }
        }
        Some(out)
    }
}

/// The view menus' keyboard state (a Win32 popup's ↑ / ↓ / Enter): the
/// rows are counted as they are laid out each frame.
#[derive(Debug, Clone, Default)]
struct MenuKeys {
    serial: u64,
    hot: Option<usize>,
    activate: bool,
    count: usize,
    enabled: Vec<bool>,
}

fn menu_keys_id() -> egui::Id {
    egui::Id::new("fv_lane_menu_keys")
}

/// One row: laid out, counted, highlighted / picked by the keyboard.
fn menu_row(ui: &mut egui::Ui, label: &str, enabled: bool, mark: Option<&str>) -> bool {
    menu_row_keys(ui, label, None, enabled, mark)
}

fn menu_row_keys(ui: &mut egui::Ui, label: &str, keys: Option<&str>, enabled: bool, mark: Option<&str>) -> bool {
    menu_row_icon(ui, label, keys, enabled, mark, None)
}

/// A row with its image in the menu's column (MENU 561's items carry the
/// toolbar images; ours are our glyphs).
fn menu_row_icon(ui: &mut egui::Ui, label: &str, keys: Option<&str>, enabled: bool, mark: Option<&str>, icon: Option<crate::popmenu::RowIcon>) -> bool {
    let mut k: MenuKeys = ui.ctx().data(|d| d.get_temp(menu_keys_id())).unwrap_or_default();
    let idx = k.count;
    k.count += 1;
    k.enabled.push(enabled);
    let hot = k.hot == Some(idx);
    let picked = enabled && hot && k.activate;
    ui.ctx().data_mut(|d| d.insert_temp(menu_keys_id(), k));
    crate::popmenu::mark_item_icon(ui, label, keys, enabled, mark, hot, icon).clicked() || picked
}

fn menu_item(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    menu_row(ui, label, enabled, None)
}

fn menu_check(ui: &mut egui::Ui, label: &str, st: ItemState) -> bool {
    menu_row(ui, label, st.enabled, st.checked.then_some("✓"))
}

fn menu_check_icon(ui: &mut egui::Ui, label: &str, st: ItemState, icon: crate::popmenu::RowIcon) -> bool {
    menu_row_icon(ui, label, None, st.enabled, st.checked.then_some("✓"), Some(icon))
}

/// The column images of MENU 561's items (our glyphs for the toolbar's).
fn mini_icon(m: crate::ribbon::Mini) -> impl Fn(&egui::Painter, Rect, Color32) {
    move |p, r, c| crate::ribbon::paint_mini(p, r, m, c)
}

fn big_icon(i: crate::icons::Icon) -> impl Fn(&egui::Painter, Rect, Color32) {
    move |p, r, c| crate::icons::paint(p, r, i, c)
}

fn menu_radio(ui: &mut egui::Ui, label: &str, st: ItemState) -> bool {
    menu_row(ui, label, st.enabled, st.checked.then_some("•"))
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
    crate::popmenu::separator(ui);
    if menu_check(ui, "Отображать в отсчетах АЦП", doc.flag_state(ChanFlag::Adc)) {
        pick(MenuCmd::Flag(ChanFlag::Adc));
    }
    crate::popmenu::separator(ui);
    if menu_item(ui, "Скрыть", true) {
        pick(MenuCmd::Hide);
    }
    crate::popmenu::separator(ui);
    use crate::icons::Icon;
    use crate::ribbon::Mini;
    for (label, m, g) in [
        ("Мгновенные значения", ValueMode::Instant, Mini::Instant),
        ("Среднеквадратичные значения", ValueMode::Rms, Mini::RmsMode),
        ("Первая гармоника", ValueMode::FirstHarmonic, Mini::Harm1),
    ] {
        if menu_check_icon(ui, label, doc.mode_state(m), &mini_icon(g)) {
            pick(MenuCmd::Mode(m));
        }
    }
    crate::popmenu::separator(ui);
    if menu_check_icon(ui, "Инвертировать", doc.flag_state(ChanFlag::Invert), &big_icon(Icon::Invert)) {
        pick(MenuCmd::Flag(ChanFlag::Invert));
    }
    if menu_check_icon(ui, "Удалить постоянную составляющую", doc.flag_state(ChanFlag::RemoveDc), &big_icon(Icon::RemoveDc)) {
        pick(MenuCmd::Flag(ChanFlag::RemoveDc));
    }
    if menu_row_icon(ui, "Вернуть к исходному размеру", None, true, None, Some(&mini_icon(Mini::Reset))) {
        pick(MenuCmd::ResetZoom);
    }
    crate::popmenu::separator(ui);
    // MENU 561 carries the accelerators' text after a tab (VM).
    if menu_row_icon(ui, "Копировать каналы для объединения осциллограмм", Some("Ctrl+C"), doc.can_copy(), None, Some(&big_icon(Icon::Copy))) {
        pick(MenuCmd::CopyMerge);
    }
    if menu_row_icon(ui, "Вставить каналы для объединения осциллограмм", Some("Ctrl+V"), can_paste, None, Some(&big_icon(Icon::Paste))) {
        pick(MenuCmd::PasteMerge);
    }
    crate::popmenu::separator(ui);
    if menu_item(ui, "Совместить каналы", doc.can_combine()) {
        pick(MenuCmd::Combine);
    }
    if menu_item(ui, "Разъединить каналы", doc.can_split()) {
        pick(MenuCmd::Split);
    }
    crate::popmenu::separator(ui);
    if menu_item(ui, "Цвет канала", true) {
        pick(MenuCmd::Color);
    }
}

/// MENU 560 (time-axis band): the RulerState radios, Сетка and 33033
/// «Разница между визирами» (checked while the band is shown, `0x4557b0`);
/// all always enabled.
fn time_axis_menu(ui: &mut egui::Ui, unit: TimeUnit, grid: bool, band: bool, pick: &mut dyn FnMut(MenuCmd)) {
    let on = |u: TimeUnit| ItemState { enabled: true, checked: unit == u };
    for (label, u) in [
        ("Микросекунды", TimeUnit::Micro),
        ("Миллисекунды", TimeUnit::Milli),
        ("Секунды", TimeUnit::Sec),
        ("Абсолютное время", TimeUnit::Absolute),
        ("Отсчеты", TimeUnit::Samples),
    ] {
        if menu_radio(ui, label, on(u)) {
            pick(MenuCmd::TimeUnit(u));
        }
    }
    crate::popmenu::separator(ui);
    if menu_check(ui, "Сетка", ItemState { enabled: true, checked: grid }) {
        pick(MenuCmd::Grid);
    }
    crate::popmenu::separator(ui);
    if menu_check(ui, "Разница между визирами", ItemState { enabled: true, checked: band }) {
        pick(MenuCmd::DiffBand);
    }
}

/// MENU 563 (a comment under the pointer), relabelled by the comment's kind
/// as `0x45efa0` does: a moment («визир», strings 206/229/236) or a level
/// line (207/237/238); «Изменить уровень» exists only for a level line.
/// The wheel events of a frame as WM_MOUSEWHEEL deltas in the view's
/// pixels: a notch (a line) is 120, a page three notches, points as they
/// come.
pub fn wheel_deltas(events: &[egui::Event]) -> Vec<f32> {
    let mut out = Vec::new();
    for e in events {
        let egui::Event::MouseWheel { unit, delta, .. } = e else { continue };
        let d = delta.x + delta.y;
        // egui-winit merges a frame's notches into one event (VM: two
        // notches in a slow frame came as 2.0 lines): whole lines are
        // split back into their WM_MOUSEWHEELs.
        let whole = d.abs().round();
        if *unit == egui::MouseWheelUnit::Line && whole >= 1.0 && (d.abs() - whole).abs() < 1e-3 {
            out.extend(std::iter::repeat_n(d.signum() * g::WHEEL_NOTCH_PX, whole as usize));
            continue;
        }
        let px = match unit {
            egui::MouseWheelUnit::Line => d * g::WHEEL_NOTCH_PX,
            egui::MouseWheelUnit::Page => d * 3.0 * g::WHEEL_NOTCH_PX,
            egui::MouseWheelUnit::Point => d,
        };
        if px != 0.0 {
            out.push(px);
        }
    }
    out
}

fn comment_menu(ui: &mut egui::Ui, c: &crate::app::Comment, i: usize, pick: &mut dyn FnMut(MenuCmd)) {
    let what = match c.kind_no() {
        1 => "визир",
        2 => "линию уровня",
        _ => "область",
    };
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
    // 33041 (`0x45d160`): checked while the comment is shown, «Скрыть…»
    // (str 203) then, «Отображать…» (204) when hidden.
    let hide = if c.hidden { "Отображать комментарий" } else { "Скрыть комментарий" };
    if menu_check(ui, hide, ItemState { enabled: true, checked: !c.hidden }) {
        pick(MenuCmd::HideComment(i));
    }
    if c.kind_no() == 2 && menu_item(ui, "Изменить уровень", true) {
        pick(MenuCmd::EditLevel(i));
    }
}

/// An analog channel's plotted / read-out value in the display mode (`0x461440`), before any factor:
/// an instantaneous value is the channel's shown series (the accessor `0x426020`: x·k − dc·k with
/// «Удалить постоянную», read with the channel's own factor `ch+0xf0`), RMS and first harmonic are
/// sums over those same shown samples.
fn mode_raw(doc: &crate::app::Doc, i: usize, k: usize) -> f64 {
    match doc.chans[i].mode {
        ValueMode::Instant => doc.shown_samples(i).get(k).copied().unwrap_or(0.0),
        ValueMode::Rms => doc.rms_at(i, k),
        ValueMode::FirstHarmonic => doc.harmonic_at(i, k) * doc.osc.harmonic1_gain,
    }
}

/// [finite min, finite max, min, max] of channel `i`'s display series, the
/// scan cached per (channel, mode, `series_gen`).
fn scaled_minmax(doc: &crate::app::Doc, i: usize) -> [f64; 4] {
    let key = (i, doc.chans[i].mode);
    let cached = doc.minmax_cache.borrow().get(&key).filter(|c| c.0 == doc.series_gen).map(|c| c.1);
    cached.unwrap_or_else(|| {
        let mut m = [f64::INFINITY, f64::NEG_INFINITY, f64::MAX, f64::MIN];
        for k in 0..doc.osc.sample_count() {
            let v = mode_raw(doc, i, k);
            if v.is_finite() {
                m[0] = m[0].min(v);
                m[1] = m[1].max(v);
            }
            m[2] = m[2].min(v);
            m[3] = m[3].max(v);
        }
        doc.minmax_cache.borrow_mut().insert(key, (doc.series_gen, m));
        m
    })
}

fn mode_disp(doc: &crate::app::Doc, i: usize, k: usize) -> f64 {
    mode_raw(doc, i, k)
}

fn analog_unit(doc: &crate::app::Doc, i: usize) -> fvb_core::units::Shown {
    if doc.chans[i].adc_counts {
        // No unit in ADC mode (`0x414c30`; the stats readout shows the bare
        // number — a clone run, #42: ours had «отсч.»).
        fvb_core::units::Shown::default()
    } else {
        doc.osc.analog[i].shown_unit()
    }
}

/// Compute (name, unit, color, max, min, rms, a1, value@cursor) for a lane.
pub(crate) fn lane_stats(
    doc: &crate::app::Doc,
    lane: &Lane,
    k_main: usize,
) -> (String, fvb_core::units::Shown, Color32, f64, f64, f64, f64, f64) {
    match lane {
        Lane::Analog(i) => {
            let ch = &doc.osc.analog[*i];
            // Value at the cursor and Max/Min follow the display mode; СКЗ
            // and A1 are always the one-period RMS and first harmonic.
            let [.., mn, mx] = scaled_minmax(doc, *i);
            let rms = doc.stat_rms(*i, k_main);
            let a1 = doc.stat_a1(*i, k_main) * doc.osc.harmonic1_gain;
            let val = mode_disp(doc, *i, k_main);
            (ch.id.clone(), analog_unit(doc, *i), doc.chans[*i].color, mx, mn, rms, a1, val)
        }
        Lane::Computed(i) => {
            // A computed channel is read out like any other channel.
            let c = &doc.computed[*i];
            let n = doc.osc.samples_per_cycle().max(2);
            let n_own = doc.computed_period(*i);
            // Value and Max / Min from the display cache (mode, invert, DC).
            let series = |k: usize| c.shown(k);
            let (mut mx, mut mn) = (f64::MIN, f64::MAX);
            for &v in c.shown_series() {
                mx = mx.max(v);
                mn = mn.min(v);
            }
            let corr = doc.computed_corr(*i);
            // The uncorrected A1 carries the record's harmonic-1 gain
            // (`0x426240`, `rec+0x278`).
            let gain = if corr.is_some() { 1.0 } else { doc.osc.harmonic1_gain };
            if let Some((t, Ok(a))) = doc.pasted_times(*i).map(|(t, p)| (t, &p.pasted.channel)) {
                // A pasted analog channel (`0x461440` with bit 14): its own
                // samples — the value interpolated at T(В1) − offset, СКЗ /
                // A1 over its own window at the bracketing own indices,
                // Max / Min over every finite own sample.
                let len = a.samples.len();
                let (_, i_hi, _) = t.bracket(k_main, len);
                let p = t.own_period(n);
                // `0x426020`: NaN and samples past the values read 0; the
                // flags applied (`Computed::pending_own`).
                let mut x: Vec<f64> = c.pending_own.clone();
                x.resize(x.len().max(i_hi + 1).max(len), 0.0);
                let own_corr = corr.map(|c| fvb_core::freqcorr::FreqCorr { fs: if t.own_rate == 0.0 { c.fs } else { t.own_rate }, ..c });
                let rms = t.stat_at(k_main, len, &|m| fvb_core::freqcorr::rms_at(own_corr, fvb_core::freqcorr::OTHER, &x, m, p, 1));
                let a1 = t.stat_at(k_main, len, &|m| fvb_core::freqcorr::fundamental_at(own_corr, fvb_core::freqcorr::OTHER, &x, m, p) * gain);
                let val = t.value_at(k_main, len, &|m| c.own_shown(m));
                let finite = c.own_series().iter().copied().filter(|v| v.is_finite());
                let (mx, mn) = finite.fold((f64::MIN, f64::MAX), |(hi, lo), v| (hi.max(v), lo.min(v)));
                let unit = if c.adc_counts { fvb_core::units::Shown::default() } else { fvb_core::units::Shown::computed(&c.unit) };
                return (c.name.clone(), unit, c.color, mx, mn, rms, a1, val);
            }
            let rms = fvb_core::freqcorr::rms_at(corr, fvb_core::freqcorr::OTHER, &c.samples, k_main, n_own, 1);
            let a1 = fvb_core::freqcorr::fundamental_at(corr, fvb_core::freqcorr::OTHER, &c.samples, k_main, n_own) * gain;
            (c.name.clone(), fvb_core::units::Shown::computed(&c.unit), c.color, mx, mn, rms, a1, series(k_main))
        }
        Lane::Digital(i) => {
            let d = &doc.osc.digital[*i];
            let val = if doc.dig_on(*i, k_main) { 1.0 } else { 0.0 };
            // By its place in the vector (`0x42b5f0`), unless set (33030).
            let color = doc.color_of(*lane);
            (d.id.clone(), Default::default(), color, 1.0, 0.0, 0.0, 0.0, val)
        }
    }
}

/// The edit `id` closed: its keys no longer wait for a release.
fn edit_keys_disarm(ui: &egui::Ui, id: egui::Id) {
    ui.data_mut(|d| {
        for key in [egui::Key::Enter, egui::Key::Escape] {
            d.remove::<bool>(id.with(("fv_key_armed", key)));
        }
    });
}

/// An inline edit's Enter and Esc act when the key is let go, not when it
/// goes down (the rename edit commits on Enter and cancels on Esc at
/// WM_KEYUP, `0x470ae0`, L9.106; a press while the edit listens arms the
/// key). `id` names the edit's state.
fn edit_key_at_release(ui: &egui::Ui, id: egui::Id, key: egui::Key, listening: bool) -> bool {
    let armed_id = id.with(("fv_key_armed", key));
    let (pressed, released) = ui.input(|i| (i.key_pressed(key), i.events.iter().any(|e| matches!(e, egui::Event::Key { key: k, pressed: false, .. } if *k == key))));
    let armed = ui.data(|d| d.get_temp::<bool>(armed_id)).unwrap_or(false) || (pressed && listening);
    if armed && released {
        ui.data_mut(|d| d.remove::<bool>(armed_id));
        return true;
    }
    ui.data_mut(|d| d.insert_temp(armed_id, armed));
    false
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
        /// The last frame's painted shapes.
        shapes: Vec<egui::epaint::ClippedShape>,
        /// The last frame's cursor.
        cursor: egui::CursorIcon,
        /// A window drawn over the lanes (its area id, order, rect).
        cover: Option<(egui::Id, egui::Order, Rect)>,
    }

    impl Harness {
        fn new() -> Harness {
            let mut app = App::new();
            app.dispatch(Command::Demo);
            let mut h = Harness { ctx: egui::Context::default(), app, t: 0.0, shapes: vec![], cursor: egui::CursorIcon::Default, cover: None };
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
            let cover = self.cover;
            let out = self.ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.lanes_ui(ui));
                app.finish_drag_cursor(ctx);
                if let Some((id, order, r)) = cover {
                    egui::Area::new(id).order(order).fixed_pos(r.min).show(ctx, |ui| {
                        ui.allocate_exact_size(r.size(), egui::Sense::click_and_drag());
                    });
                }
            });
            self.cursor = out.platform_output.cursor_icon;
            self.shapes = out.shapes;
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
            let rows = lane_layout(self.app.active_doc().unwrap(), self.app.y_analog);
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
            let n = lane_layout(self.app.active_doc().unwrap(), self.app.y_analog).len();
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

    /// The view's bars are the system metric MulDiv(17, dpi, 96), also past 150 % where the first-zoom rule
    /// (`WinMetrics`) measured 30 / 34 px (a straight line through 17, 21, 26 gave 31 / 36).
    #[test]
    fn the_view_scroll_bars_follow_the_system_metric() {
        for (dpi, w) in [(96, 17.0), (120, 21.0), (144, 26.0), (168, 30.0), (192, 34.0)] {
            crate::dpi::set(dpi);
            assert_eq!((scroll_w(), scroll_h()), (w, w), "{dpi}");
        }
        crate::dpi::set(96);
    }

    #[test]
    fn hit_test_reaches_the_top_row_and_the_axis_band() {
        let rows = [
            Row { key: ChanKey::Analog(0), members: vec![ChanKey::Analog(0)], h: 93.0 },
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
        let rows = lane_layout(h.doc(), h.app.y_analog);
        for r in &rows {
            let want = if matches!(r.key, ChanKey::Digital(_)) { 20.0 } else { 93.0 };
            assert_eq!(r.h, want, "{:?}", r.key);
        }
    }

    /// A group lane (`0x460290`): N × the owner's stride — name row + 33,
    /// + the stats line when the owner has stat rows; the members merge.
    #[test]
    fn group_lane_height_is_members_times_the_owners_stride() {
        let mut h = Harness::new();
        let d = h.app.active_doc_mut().unwrap();
        d.chans[0].group = Some(7);
        d.chans[1].group = Some(7);
        let n = lane_layout(h.doc(), h.app.y_analog).len();
        let rows = lane_layout(h.doc(), h.app.y_analog);
        assert_eq!(rows[0].members, [ChanKey::Analog(0), ChanKey::Analog(1)]);
        assert_eq!(rows[0].h, 2.0 * (ROW_H + 33.0 + GROUP_STATS_H), "default СКЗ + A1 rows");
        h.app.active_doc_mut().unwrap().chans[0].stats = crate::app::StatRows { minmax: false, rms: false, a1: false };
        let rows = lane_layout(h.doc(), h.app.y_analog);
        assert_eq!(rows[0].h, h.app.y_analog.max(2.0 * (ROW_H + 33.0)), "the owner's flags decide");
        assert_eq!(rows.len(), n);
    }

    /// The left button places В1 (shown) and a drag does not pan the plot;
    /// grabbing В1 within 3 px drags it.
    #[test]
    fn left_press_places_v1_and_grabbing_it_drags_without_panning() {
        let mut h = Harness::new();
        assert!(!h.doc().v_shown[0], "a new document shows no визир");
        let y = h.row(0).y;
        h.button(Pos2::new(500.0, y), egui::PointerButton::Primary);
        assert!(h.doc().v_shown[0] && h.doc().v1_placed, "placed");
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

    /// Press on stats row `from`, move in 5 px steps to row `to`, release.
    fn drag_rows(h: &mut Harness, from: usize, to: usize) {
        let (a, b) = (h.row(from), h.row(to));
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(a), ev(a, true)]);
        let steps = ((b.y - a.y).abs() / 5.0).ceil().max(1.0) as usize;
        for k in 1..=steps {
            h.frame(vec![egui::Event::PointerMoved(a + (b - a) * (k as f32 / steps as f32))]);
        }
        h.frame(vec![ev(b, false)]);
        h.frame(vec![]);
    }

    /// A drag that starts in the plot and crosses into the stats panel is
    /// a channel drag (L9.61: the move's own point decides); with a slow
    /// frame the release comes in the frame after the arming move, after
    /// MFC's 200 ms: a drop (VM: «Совмещение невозможно» onto a lane of
    /// another unit).
    #[test]
    fn a_plot_drag_into_the_stats_panel_drops_after_the_delay() {
        let mut h = Harness::new();
        let rows = lane_layout(h.doc(), h.app.y_analog);
        let unit = |li: usize| match rows[li].key {
            Lane::Analog(i) => Some(h.doc().osc.analog[i].unit.clone()),
            _ => None,
        };
        let other = (1..h.rows()).find(|&li| unit(li).is_some() && unit(li) != unit(0)).expect("a lane of another unit");
        h.app.active_doc_mut().unwrap().selection = [ChanKey::Analog(0)].into_iter().collect();
        let b = h.row(other);
        let a = egui::pos2(b.x + 400.0, b.y);
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(a), ev(a, true)]);
        h.frame(vec![egui::Event::PointerMoved(b)]);
        for _ in 0..4 {
            h.t += 0.05;
        }
        h.frame(vec![ev(b, false)]);
        h.frame(vec![]);
        assert_eq!(h.app.dlg.alert(), Some(MSG_COMBINE_UNITS));
    }

    #[test]
    fn dragging_a_channel_onto_another_lane_combines_them() {
        let mut h = Harness::new();
        let rows = lane_layout(h.doc(), h.app.y_analog);
        let unit = |li: usize| match rows[li].key {
            Lane::Analog(i) => Some(h.doc().osc.analog[i].unit.clone()),
            _ => None,
        };
        let n = h.rows();
        let same = (1..n).find(|&li| unit(li).is_some() && unit(li) == unit(0)).expect("a lane with the same unit");
        let other = (1..n).find(|&li| unit(li).is_some() && unit(li) != unit(0)).expect("a lane with another unit");
        let (Lane::Analog(c0), Lane::Analog(cs)) = (rows[0].key, rows[same].key) else { unreachable!() };
        let co = match rows[other].key {
            Lane::Analog(i) => i,
            _ => unreachable!(),
        };
        drag_rows(&mut h, 0, other);
        assert_eq!(h.app.dlg.alert(), Some(MSG_COMBINE_UNITS), "different units: message 1086");
        assert!(h.doc().chans[c0].group.is_none() && h.doc().chans[co].group.is_none());
        h.app.dlg.alerts.clear();
        h.app.active_doc_mut().unwrap().selection.clear();
        // Another own rate on the dragged channel: message 239 (error
        // icon), nothing changed (#57).
        h.app.active_doc_mut().unwrap().osc.analog[c0].own_rate = 1234.0;
        drag_rows(&mut h, 0, same);
        assert_eq!(h.app.dlg.alert(), Some(MSG_COMBINE_RATES), "different rates: message 239");
        assert!(h.doc().chans[c0].group.is_none() && h.doc().chans[cs].group.is_none());
        h.app.dlg.alerts.clear();
        h.app.active_doc_mut().unwrap().osc.analog[c0].own_rate = 0.0;
        h.app.active_doc_mut().unwrap().selection.clear();
        drag_rows(&mut h, 0, same);
        let d = h.doc();
        assert!(d.chans[c0].group.is_some() && d.chans[c0].group == d.chans[cs].group, "one lane now");
        assert!(!egui::DragAndDrop::has_any_payload(&h.ctx));
    }

    /// A lane dragged and released on a slid-out pane's window over the view
    /// drops nowhere, as on a floating one (clone: the pane out, the release
    /// on its caption over another lane of the unit: nothing grouped).
    #[test]
    fn a_channel_dropped_on_a_slid_out_pane_over_the_view_drops_nowhere() {
        let unit = |h: &Harness, li: usize| match lane_layout(h.doc(), h.app.y_analog)[li].key {
            Lane::Analog(i) => Some(h.doc().osc.analog[i].unit.clone()),
            _ => None,
        };
        for covered in [false, true] {
            let mut h = Harness::new();
            let same = (1..h.rows()).find(|&li| unit(&h, li).is_some() && unit(&h, li) == unit(&h, 0)).expect("a lane with the same unit");
            let to = h.row(same);
            if covered {
                let id = egui::Id::new(("fv_slide", crate::docking::Unit::Frequency));
                h.cover = Some((id, egui::Order::Foreground, Rect::from_min_size(Pos2::new(to.x + 100.0, to.y - 20.0), Vec2::new(300.0, 40.0))));
                h.frame(vec![]);
            }
            let (Lane::Analog(c0), Lane::Analog(cs)) = (lane_layout(h.doc(), h.app.y_analog)[0].key, lane_layout(h.doc(), h.app.y_analog)[same].key) else { unreachable!() };
            // Release at x 200 past the stats panel, over the cover when there is one.
            let b = Pos2::new(to.x + 200.0, to.y);
            let a = h.row(0);
            let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, modifiers: Default::default(), pressed };
            h.frame(vec![egui::Event::PointerMoved(a), ev(a, true)]);
            let steps = ((b - a).length() / 5.0).ceil().max(1.0) as usize;
            for k in 1..=steps {
                h.frame(vec![egui::Event::PointerMoved(a + (b - a) * (k as f32 / steps as f32))]);
            }
            h.frame(vec![ev(b, false)]);
            h.frame(vec![]);
            let d = h.doc();
            let grouped = d.chans[c0].group.is_some() && d.chans[c0].group == d.chans[cs].group;
            assert_eq!(grouped, !covered, "covered by a slid-out pane: {covered}");
        }
    }

    /// The drop works for formula channels too (#57): a formula added after
    /// the rate changed has no own rate while the merged lane has one, so the
    /// drop shows message 239 and changes nothing; with agreeing rates it groups.
    #[test]
    fn dragging_a_formula_lane_onto_a_merged_lane() {
        let mut h = Harness::new();
        assert!(h.app.append_computed(&[("X".into(), "Ub".into())]).is_some());
        let rows = lane_layout(h.doc(), h.app.y_analog);
        let from = rows.iter().position(|r| r.key == Lane::Computed(0)).expect("the formula lane");
        let to = rows.iter().position(|r| r.key == Lane::Analog(1)).expect("a file lane");
        let rate = h.doc().osc.int_rate_hz();
        h.app.active_doc_mut().unwrap().osc.analog[1].own_rate = rate * 2.0;
        drag_rows(&mut h, from, to);
        assert_eq!(h.app.dlg.alert(), Some(MSG_COMBINE_RATES), "message 239");
        assert!(h.doc().computed[0].group.is_none() && h.doc().chans[1].group.is_none());
        assert!(h.doc().selection.contains(&Lane::Analog(1)) && h.doc().selection.contains(&Lane::Computed(0)), "both stay selected after a refused drop (VM)");
        h.app.dlg.alerts.clear();
        h.app.active_doc_mut().unwrap().osc.analog[1].own_rate = 0.0;
        h.app.active_doc_mut().unwrap().selection.clear();
        drag_rows(&mut h, from, to);
        assert!(h.app.dlg.alert().is_none());
        assert!(h.doc().computed[0].group.is_some() && h.doc().computed[0].group == h.doc().chans[1].group, "one lane now");
    }

    /// After «Объединить» and a rate change the record's rate is above the
    /// channels' own: the analog painters visit ⌈W/(25f·R/r)⌉ samples, so a
    /// lane stops short of the plot's right edge, at W·r/R from its left
    /// (VM, the original and ours: the lanes of the demo record end 300 px
    /// into a 370 px plot after 2000 → 2450 Hz) — #100.
    #[test]
    fn merged_lanes_stop_short_of_the_right_edge_after_a_rate_rise() {
        let mut h = Harness::new();
        h.app.active_doc_mut().unwrap().selection = [ChanKey::Analog(0)].into_iter().collect();
        h.app.copy_channels_for_merge();
        h.app.paste_channels_for_merge();
        h.app.merge_records();
        let curve_right = |h: &Harness| h.shapes.iter().filter_map(|c| if let Shape::Mesh(m) = &c.shape { Some(m) } else { None }).filter(|m| m.vertices.len() > 300).flat_map(|m| m.vertices.iter().map(|v| v.pos.x)).fold(0.0f32, f32::max);
        h.app.active_doc_mut().unwrap().xscale = Some(0.25);
        h.frame(vec![]);
        h.frame(vec![]);
        let full = curve_right(&h);
        assert_eq!(h.app.apply_file_rate(0, "2450"), Ok(()));
        h.frame(vec![]);
        h.frame(vec![]);
        let (left, short) = (h.app.view_left, curve_right(&h));
        assert!(full > 1100.0, "before the change the curves run to the right edge: {full}");
        let w = full - left;
        assert!((short - left - w / 1.225).abs() < 12.0, "after it they stop at W/1.225: {short} (W = {w})");
    }

    /// A digital lane's base bar (`0x463e2d`) is trunc(25·N·f·R/r) long: after
    /// a merge and a rate rise it is 1.225 times the record's length (#100).
    #[test]
    fn a_merged_digital_lane_draws_its_base_bar_by_the_ratio() {
        let mut h = Harness::new();
        h.app.active_doc_mut().unwrap().selection = [ChanKey::Analog(0)].into_iter().collect();
        h.app.copy_channels_for_merge();
        h.app.paste_channels_for_merge();
        h.app.merge_records();
        let bar = |h: &Harness| h.shapes.iter().find_map(|c| if let Shape::Rect(r) = &c.shape { (r.fill == Color32::from_rgb(150, 150, 255)).then(|| r.rect.width()) } else { None });
        h.app.active_doc_mut().unwrap().xscale = Some(0.03125);
        h.frame(vec![]);
        h.frame(vec![]);
        let before = bar(&h).expect("the base bar");
        assert_eq!(h.app.apply_file_rate(0, "2450"), Ok(()));
        h.frame(vec![]);
        h.frame(vec![]);
        let after = bar(&h).expect("the base bar");
        assert!((after / before - 1.225).abs() < 0.01, "{before} -> {after}");
    }

    /// A pasted analog lane takes part in a lane drag and a drop on one: the
    /// drop selects the dragged lanes with the target and 33035's enable check
    /// (`0x468c80`) refuses them with message 1086. Digital lanes (file and
    /// pasted) lie past `doc+0x1d8`, the bound of the drag collector and the
    /// drop's loops: a digital-only selection starts no drag, a drop on a
    /// digital lane does nothing, and with a digital lane also selected the
    /// enable check (it walks the whole list) refuses (#102).
    #[test]
    fn pasted_and_digital_lanes_take_part_in_a_drag_and_the_drop_is_refused() {
        let mut h = Harness::new();
        h.app.active_doc_mut().unwrap().selection = [ChanKey::Analog(1)].into_iter().collect();
        h.app.copy_channels_for_merge();
        h.app.paste_channels_for_merge();
        h.frame(vec![]);
        h.frame(vec![]);
        let rows = lane_layout(h.doc(), h.app.y_analog);
        let pend = rows.iter().position(|r| matches!(r.key, Lane::Computed(j) if h.doc().pending_of(j).is_some())).expect("the pasted lane");
        let file = rows.iter().position(|r| matches!(r.key, Lane::Analog(_))).expect("a file lane");
        let (pk, fk) = (rows[pend].key, rows[file].key);
        // The pasted lane dragged onto a file lane.
        h.app.active_doc_mut().unwrap().selection = [pk].into_iter().collect();
        drag_rows(&mut h, pend, file);
        assert_eq!(h.app.dlg.alert(), Some(MSG_COMBINE_UNITS), "message 1086");
        assert!(h.doc().selection.contains(&pk) && h.doc().selection.contains(&fk), "the drop selected both");
        h.app.dlg.alerts.clear();
        // A file lane dragged onto the pasted one.
        h.app.active_doc_mut().unwrap().selection = [fk].into_iter().collect();
        drag_rows(&mut h, file, pend);
        assert_eq!(h.app.dlg.alert(), Some(MSG_COMBINE_UNITS), "a drop on a pasted lane: message 1086");
        h.app.dlg.alerts.clear();
        // A digital lane dragged: no drag starts, nothing happens.
        let dig = rows.iter().position(|r| matches!(r.key, Lane::Digital(_))).expect("a digital lane");
        let dk = rows[dig].key;
        h.app.active_doc_mut().unwrap().selection = [dk].into_iter().collect();
        drag_rows(&mut h, dig, file);
        assert!(h.app.dlg.alert().is_none(), "a digital-only selection starts no drag");
        assert_eq!(h.doc().selection, [dk].into_iter().collect(), "the selection is unchanged");
        // A drop on a digital lane: no target, nothing happens.
        h.app.active_doc_mut().unwrap().selection = [fk].into_iter().collect();
        drag_rows(&mut h, file, dig);
        assert!(h.app.dlg.alert().is_none(), "a digital lane is no target");
        assert_eq!(h.doc().selection, [fk].into_iter().collect(), "the selection is unchanged");
        // An analog and a digital lane selected, dropped on another analog
        // lane: the enable check walks the whole list and refuses.
        let other = (file + 1..rows.len()).find(|&i| matches!(rows[i].key, Lane::Analog(_))).expect("another analog lane");
        let (pa, pb) = (h.row(file), h.row(other));
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(pa), ev(pa, true)]);
        // The press selected the lane alone; the digital lane joins it.
        h.app.active_doc_mut().unwrap().selection = [fk, dk].into_iter().collect();
        for k in 1..=((pb.y - pa.y).abs() / 5.0).ceil().max(1.0) as usize {
            h.frame(vec![egui::Event::PointerMoved(pa + (pb - pa) * (k as f32 * 5.0 / (pb.y - pa.y).abs()).min(1.0))]);
        }
        h.frame(vec![ev(pb, false)]);
        h.frame(vec![]);
        assert_eq!(h.app.dlg.alert(), Some(MSG_COMBINE_UNITS), "message 1086 through the enable check");
        h.app.dlg.alerts.clear();
        // A pasted digital lane: the same, and it never reaches a pane.
        let mut h = Harness::new();
        h.app.active_doc_mut().unwrap().selection = [ChanKey::Digital(0)].into_iter().collect();
        h.app.copy_channels_for_merge();
        h.app.paste_channels_for_merge();
        h.frame(vec![]);
        h.frame(vec![]);
        let rows = lane_layout(h.doc(), h.app.y_analog);
        let pd = rows.iter().position(|r| matches!(r.key, Lane::Computed(j) if h.doc().pending_digital(j))).expect("a pasted digital lane");
        let file = rows.iter().position(|r| matches!(r.key, Lane::Analog(_))).unwrap();
        let (pdk, fk) = (rows[pd].key, rows[file].key);
        let started = |h: &Harness| egui::DragAndDrop::has_payload_of_type::<ChanDrag>(&h.ctx);
        let hold = |h: &mut Harness, from: usize| {
            let (pa, pb) = (h.row(from), h.row(from) + Vec2::new(0.0, 12.0));
            let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            h.frame(vec![egui::Event::PointerMoved(pa), ev(pa, true)]);
            h.frame(vec![egui::Event::PointerMoved(pb)]);
            h.t += 0.3;
            h.frame(vec![egui::Event::PointerMoved(pb + Vec2::new(0.0, 3.0))]);
            started(h)
        };
        h.app.active_doc_mut().unwrap().selection = [pdk].into_iter().collect();
        assert!(!hold(&mut h, pd), "a pasted digital lane alone starts no drag");
        assert!(h.app.dlg.alert().is_none());
        h.frame(vec![egui::Event::PointerButton { pos: h.row(pd), button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        let dig = rows.iter().position(|r| matches!(r.key, Lane::Digital(_))).unwrap();
        h.app.active_doc_mut().unwrap().selection = [rows[dig].key].into_iter().collect();
        assert!(!hold(&mut h, dig), "a file digital lane alone starts no drag");
        h.frame(vec![egui::Event::PointerButton { pos: h.row(dig), button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        // A drop on the pasted digital lane does nothing.
        h.app.active_doc_mut().unwrap().selection = [fk].into_iter().collect();
        drag_rows(&mut h, file, pd);
        assert!(h.app.dlg.alert().is_none(), "a pasted digital lane is no target");
        assert_eq!(h.doc().selection, [fk].into_iter().collect(), "the selection is unchanged");
        // In a pane drop it is skipped.
        let j = match pdk { Lane::Computed(j) => j, _ => unreachable!() };
        h.app.active_doc_mut().unwrap().computed[j].on_freq = false;
        h.app.drop_channels(&[pdk], crate::app::Pane::Frequency);
        assert!(!h.doc().computed[j].on_freq, "a pasted digital lane never reaches a pane");
    }

    /// The drop on a diagram pane takes formula channels too (#103): the
    /// pane's command 35000+i sets the channel's bit for every record
    /// (`0x46f100`, `0x48f590`); the circle diagram only an Ohm / VA channel
    /// that is not pending.
    #[test]
    fn dropping_formula_channels_on_diagram_panes() {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        assert!(app.append_computed(&[("X".into(), "Ub".into())]).is_some());
        let d = app.active_doc_mut().unwrap();
        d.computed[0].unit = "Ohm".into();
        (d.computed[0].on_freq, d.computed[0].on_vector, d.computed[0].on_circle) = (false, false, false);
        app.drop_channels(&[ChanKey::Computed(0)], crate::app::Pane::Frequency);
        app.drop_channels(&[ChanKey::Computed(0)], crate::app::Pane::Vector);
        app.drop_channels(&[ChanKey::Computed(0)], crate::app::Pane::Circle);
        let d = app.active_doc().unwrap();
        assert!(d.computed[0].on_freq && d.computed[0].on_vector && d.computed[0].on_circle);
        assert!(d.set.modified);
        // A pasted analog lane: the frequency / vector bits are set, the circle's never.
        app.active_doc_mut().unwrap().selection = [ChanKey::Analog(1)].into_iter().collect();
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let p = d.computed.iter().position(|c| c.formula == crate::merge_ui::PENDING).expect("the pasted lane");
        d.computed[p].unit = "Ohm".into();
        (d.computed[p].on_freq, d.computed[p].on_vector, d.computed[p].on_circle) = (false, false, false);
        for pane in [crate::app::Pane::Frequency, crate::app::Pane::Vector, crate::app::Pane::Circle] {
            app.drop_channels(&[ChanKey::Computed(p)], pane);
        }
        let c = &app.active_doc().unwrap().computed[p];
        assert!(c.on_freq && c.on_vector && !c.on_circle, "a pasted lane takes the frequency / vector bits, never the circle's");
        // Another unit: not on the circle diagram.
        let d = app.active_doc_mut().unwrap();
        d.computed[0].unit = "A".into();
        d.computed[0].on_circle = false;
        app.drop_channels(&[ChanKey::Computed(0)], crate::app::Pane::Circle);
        assert!(!app.active_doc().unwrap().computed[0].on_circle);
    }

    #[test]
    fn dropping_on_diagram_panes_adds_the_channels() {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        let d = app.active_doc_mut().unwrap();
        for c in &mut d.chans {
            (c.on_freq, c.on_vector, c.on_circle) = (false, false, false);
        }
        let ohm = d.osc.analog.iter().position(|a| a.unit.eq_ignore_ascii_case("ohm"));
        app.drop_channels(&[ChanKey::Analog(0), ChanKey::Analog(1)], crate::app::Pane::Frequency);
        app.drop_channels(&[ChanKey::Analog(0)], crate::app::Pane::Vector);
        app.drop_channels(&[ChanKey::Analog(0)], crate::app::Pane::Circle);
        app.drop_channels(&[ChanKey::Analog(0)], crate::app::Pane::Differential);
        let d = app.active_doc().unwrap();
        assert!(d.chans[0].on_freq && d.chans[1].on_freq && d.chans[0].on_vector);
        assert_eq!(d.chans[0].on_circle, ohm == Some(0) || d.osc.analog[0].unit.eq_ignore_ascii_case("va"), "the circle diagram takes Ohm / VA only");
        assert!(d.set.modified);
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
        // The bar's page, by the rule on a synthetic record (9746 samples, 1523
        // shown: page 1562 of 10000, the bar spans 8223 samples).
        assert_eq!(hscroll_page(0, 9746, 1523, false), 1284);
        assert_eq!(hscroll_page(1284, 9746, 1523, false), 2568);
        assert_eq!(hscroll_page(2568, 9746, 1523, true), 1282);
        assert_eq!(hscroll_page(8000, 9746, 1523, false), 8223, "the last screen");
        // Shift+wheel: a page sideways (zoomed in so there is room).
        h.wheel(at, 1.0, egui::Modifiers::CTRL);
        h.wheel(at, 1.0, egui::Modifiers::CTRL);
        let first = h.doc().first;
        h.wheel(at, -1.0, egui::Modifiers::SHIFT);
        assert!(h.doc().first > first, "Shift+wheel scrolls right");
        // One step per WM_MOUSEWHEEL: two notches in one frame zoom twice;
        // a touchpad's 30 points scroll the lanes 30 px, not a notch.
        let f = h.doc().xscale.unwrap();
        let notch = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: Vec2::new(0.0, -1.0), modifiers: egui::Modifiers::CTRL };
        h.frame_mod(vec![egui::Event::PointerMoved(at), notch.clone(), notch], egui::Modifiers::CTRL);
        let fit = g::fit_scale(h.app.view_width, h.doc().osc.sample_count());
        assert_eq!(h.doc().xscale.unwrap(), (f / 4.0).max(fit), "two steps");
        // egui-winit's merged notches (2.0 lines in one event) are two too.
        let merged = |d: f32| egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: Vec2::new(0.0, d), modifiers: egui::Modifiers::NONE };
        assert_eq!(wheel_deltas(&[merged(2.0)]), vec![g::WHEEL_NOTCH_PX; 2]);
        assert_eq!(wheel_deltas(&[merged(-0.25)]), vec![-0.25 * g::WHEEL_NOTCH_PX], "a fine wheel's part-notch stays one");
        let v = h.doc().vscroll;
        assert!(v <= -30.0, "scrolled down before: {v}");
        let touch = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, 30.0), modifiers: egui::Modifiers::NONE };
        h.frame_mod(vec![egui::Event::PointerMoved(at), touch], egui::Modifiers::NONE);
        assert_eq!(h.doc().vscroll, v + 30.0, "raw pixels");
    }

    /// With the whole record in view there is no horizontal bar (seen on
    /// the VM, L9.196): nothing is painted along the bottom.
    #[test]
    fn no_horizontal_bar_while_the_record_fits() {
        let mut h = Harness::new();
        let fit = g::fit_scale(h.app.view_width, h.doc().osc.sample_count());
        h.app.active_doc_mut().unwrap().xscale = Some(fit / 4.0);
        h.frame(vec![]);
        h.frame(vec![]);
        let y = SCREEN.y - 8.0 - HSCROLL_H / 2.0;
        let bar = |h: &Harness| h.shapes.iter().any(|c| matches!(&c.shape, Shape::Rect(r) if r.rect.contains(Pos2::new(400.0, y)) && r.fill == Color32::from_gray(240)));
        assert!(!bar(&h), "no bar while the record fits");
        h.app.active_doc_mut().unwrap().xscale = Some(fit * 8.0);
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(bar(&h), "the bar once it does not");
    }

    #[test]
    fn the_horizontal_scroll_bar_is_painted_along_the_bottom_and_steps_by_a_screen() {
        let mut h = Harness::new();
        let at = Pos2::new(600.0, h.row(0).y);
        for _ in 0..3 {
            h.wheel(at, 1.0, egui::Modifiers::CTRL);
        }
        let doc = h.doc();
        let rows = lane_layout(doc, h.app.y_analog);
        let band = g::RULER_H + if doc.diff_band { g::DIFF_BAND_H } else { 0.0 };
        let client_h = SCREEN.y - 16.0 - HSCROLL_H;
        let vbar = rows.iter().map(|r| r.h).sum::<f32>() + band + 2.0 > client_h;
        let plot_right = SCREEN.x - 8.0 - if vbar { VSCROLL_W } else { 0.0 };
        let visible = g::visible_count(plot_right - 8.0, h.app.view_left, doc.xscale.unwrap());
        assert!(g::max_first(doc.osc.sample_count(), visible) > 2 * visible, "zoomed in far enough");
        assert_eq!(doc.first, 0);
        // Visible: the track is painted inside its clip rect, also under the
        // stats panel.
        let y = SCREEN.y - 8.0 - HSCROLL_H / 2.0;
        let painted = |p: Pos2| {
            h.shapes.iter().any(|c| {
                c.clip_rect.contains(p) && matches!(&c.shape, Shape::Rect(r) if r.rect.contains(p) && r.fill == Color32::from_gray(240))
            })
        };
        assert!(painted(Pos2::new(400.0, y)));
        assert!(painted(Pos2::new(8.0 + HSCROLL_H + 4.0, y)), "the bar starts at the view's left edge");
        // The arrows step one screen, the track one page (also one screen).
        let right_arrow = Pos2::new(plot_right - HSCROLL_H / 2.0, y);
        h.button(right_arrow, egui::PointerButton::Primary);
        assert_eq!(h.doc().first, visible);
        h.button(Pos2::new(8.0 + HSCROLL_H / 2.0, y), egui::PointerButton::Primary);
        assert_eq!(h.doc().first, 0);
        h.button(Pos2::new(plot_right - 3.0 * HSCROLL_H, y), egui::PointerButton::Primary);
        assert_eq!(h.doc().first, visible, "a track click right of the thumb pages right");
        // The system's menu (`0x45d6d0`): «Page Right» pages, «Left Edge»
        // (SB_LEFT) does nothing.
        let item = |h: &Harness, label: &str| {
            h.shapes.iter().find_map(|c| if let Shape::Text(t) = &c.shape { (t.galley.text() == label).then(|| t.pos + t.galley.rect.center().to_vec2()) } else { None })
        };
        h.button(Pos2::new(400.0, y), egui::PointerButton::Secondary);
        let at = item(&h, "Page Right").expect("the menu is open");
        h.button(at, egui::PointerButton::Primary);
        assert_eq!(h.doc().first, 2 * visible);
        h.button(Pos2::new(400.0, y), egui::PointerButton::Secondary);
        let at = item(&h, "Left Edge").expect("the menu is open");
        h.button(at, egui::PointerButton::Primary);
        assert_eq!(h.doc().first, 2 * visible, "SB_LEFT is ignored");
        assert!(item(&h, "Left Edge").is_none(), "closed");
    }

    /// Dragging the thumb follows the pointer along the bar wherever it
    /// wanders above or below it (the drag used to stop once the thumb had
    /// moved away from the press point).
    #[test]
    fn scroll_thumb_drag_survives_leaving_the_bar() {
        let mut h = Harness::new();
        let at = Pos2::new(600.0, h.row(0).y);
        for _ in 0..3 {
            h.wheel(at, 1.0, egui::Modifiers::CTRL);
        }
        let y = SCREEN.y - 8.0 - HSCROLL_H / 2.0;
        // The thumb sits just right of the left arrow at first = 0.
        let grab = Pos2::new(8.0 + HSCROLL_H + 3.0, y);
        let press = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(grab)]);
        h.frame(vec![press(grab, true)]);
        let mut last = 0;
        for k in 1..=20 {
            // Rightwards, wandering 40 px above and below the bar.
            let p = Pos2::new(grab.x + 15.0 * k as f32, y + if k % 2 == 0 { -40.0 } else { 40.0 });
            h.frame(vec![egui::Event::PointerMoved(p)]);
            let first = h.doc().first;
            assert!(first > last, "step {k}: first {first} did not follow the pointer");
            last = first;
        }
        h.frame(vec![press(Pos2::new(grab.x + 300.0, y), false)]);
    }

    /// «Приблизить по вертикали» (138): the zoomed channel is drawn through
    /// a clip of its own lane, (top + 5)–(top + h) (`0x4295a5`), so the part
    /// of the curve outside the window never reaches the neighbours.
    #[test]
    fn a_rect_zoomed_curve_is_clipped_to_its_lane() {
        let mut h = Harness::new();
        let rows = lane_layout(h.doc(), h.app.y_analog);
        let (top, lh) = (8.0f32, rows[0].h);
        h.app.dispatch(Command::RectZoom);
        h.frame(vec![]);
        // Drag over the middle fifth of lane 0 in the plot.
        let x = 600.0;
        drag(&mut h, Pos2::new(x, top + lh * 0.4), Pos2::new(x, top + lh * 0.6));
        let Lane::Analog(ch) = rows[0].key else { panic!("an analog lane first") };
        assert!(h.doc().chans[ch].window.is_some(), "the lane is window-zoomed");
        h.frame(vec![]);
        let lane = |c: &egui::epaint::ClippedShape| (c.clip_rect.top() - (top + 5.0)).abs() < 0.5 && (c.clip_rect.bottom() - (top + lh)).abs() < 0.5;
        assert!(h.shapes.iter().any(|c| lane(c) && matches!(c.shape, Shape::Path(_) | Shape::LineSegment { .. } | Shape::Vec(_))), "the curve is drawn through the lane clip");
        // An unzoomed lane keeps the plot clip.
        assert!(h.doc().chans.iter().filter(|c| c.window.is_some()).count() == 1);
    }

    /// Holding an arrow keeps scrolling after a delay; holding the track
    /// pages until the thumb reaches the pointer.
    #[test]
    fn held_scroll_arrow_and_track_repeat() {
        let mut h = Harness::new();
        let at = Pos2::new(600.0, h.row(0).y);
        for _ in 0..4 {
            h.wheel(at, 1.0, egui::Modifiers::CTRL);
        }
        let y = SCREEN.y - 8.0 - HSCROLL_H / 2.0;
        let plot_right = SCREEN.x - 8.0 - VSCROLL_W;
        let press = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let arrow = Pos2::new(plot_right - HSCROLL_H / 2.0, y);
        h.frame(vec![egui::Event::PointerMoved(arrow), press(arrow, true)]);
        let one = h.doc().first;
        assert!(one > 0, "the press steps at once");
        for _ in 0..20 {
            h.frame(vec![]);
        }
        let held = h.doc().first;
        assert!(held >= 5 * one, "a held arrow repeats: {held} after {one}");
        h.frame(vec![press(arrow, false)]);
        for _ in 0..10 {
            h.frame(vec![]);
        }
        assert_eq!(h.doc().first, held, "release stops it");
        // Track left of the thumb (now at the right end), held mid-track:
        // pages back and stops with the thumb under the pointer, not at 0.
        let spot = Pos2::new((8.0 + plot_right) / 2.0, y);
        h.frame(vec![egui::Event::PointerMoved(spot), press(spot, true)]);
        for _ in 0..80 {
            h.frame(vec![]);
        }
        h.frame(vec![press(spot, false)]);
        assert!(h.doc().first > 0 && h.doc().first < held, "stopped at the pointer: {}", h.doc().first);
    }

    /// `0x493110`: a ГВ label only for a line out of view (clamped to the
    /// top), close В labels merged into one, an off-screen В label clamped
    /// onto the plot's edge.
    #[test]
    fn vizir_labels_follow_the_original() {
        let texts = |h: &Harness| -> Vec<String> {
            h.shapes.iter().filter_map(|c| if let Shape::Text(t) = &c.shape { Some(t.galley.text().to_string()) } else { None }).collect()
        };
        let mut h = Harness::new();
        let (lo, hi) = chan_range(h.doc(), 0);
        let d = h.app.active_doc_mut().unwrap();
        d.hviz[0] = crate::app::HVizir { shown: true, chan: Some(0), value: (lo + hi) / 2.0 };
        h.frame(vec![]);
        assert!(!texts(&h).iter().any(|t| t == "ГВ1"), "an on-screen ГВ has no label");
        // Tall lanes scrolled so lane 0 is above the view: the label hangs
        // at the top.
        h.app.y_analog = 400.0;
        h.app.active_doc_mut().unwrap().vscroll = -1000.0;
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(texts(&h).iter().any(|t| t == "ГВ1"), "an out-of-view ГВ is labelled");
        h.app.active_doc_mut().unwrap().vscroll = 0.0;
        h.app.y_analog = 93.0;
        // Three close визиры: one merged label.
        let d = h.app.active_doc_mut().unwrap();
        let k = d.first + 40;
        (d.main_cursor, d.aux_cursor, d.v3_cursor) = (k, k + 1, k + 2);
        d.v_shown = [true, true, true];
        h.frame(vec![]);
        let t = texts(&h);
        assert!(t.iter().any(|t| t == "В1 В2 В3") && !t.iter().any(|t| t == "В2"), "{t:?}");
        // В2 far left of the window: clamped onto the plot, unmerged.
        let d = h.app.active_doc_mut().unwrap();
        d.first = k + 100;
        h.frame(vec![]);
        assert!(texts(&h).iter().filter(|t| t.starts_with('В')).count() >= 3, "clamped labels never merge");
    }

    /// The min / max cache follows value changes: Invert flips the range,
    /// a mode change has its own entry.
    #[test]
    fn range_cache_follows_value_changes() {
        let mut h = Harness::new();
        let (lo, hi) = chan_range(h.doc(), 0);
        h.app.active_doc_mut().unwrap().selection = [ChanKey::Analog(0)].into_iter().collect();
        h.app.dispatch(Command::ToggleFlag(ChanFlag::Invert));
        let (lo2, hi2) = chan_range(h.doc(), 0);
        assert!((lo2 + hi).abs() < 1e-9 && (hi2 + lo).abs() < 1e-9, "inverted: {lo2}..{hi2} vs {lo}..{hi}");
        h.app.dispatch(Command::SetValueMode(ValueMode::Rms));
        let (lo3, _) = chan_range(h.doc(), 0);
        assert!(lo3 >= 0.0, "an RMS series is never negative: {lo3}");
    }

    /// `ch+0xf0`: every reading of a channel carries the channel's own factor. A channel whose
    /// primary / secondary flag differs from channel 0's (a pasted one) shows its waveform, its value at
    /// the cursor, its range and its СКЗ in one factor.
    #[test]
    fn a_lane_reads_every_value_with_its_own_factor() {
        use fvb_core::signal::rms_at;
        let mut h = Harness::new();
        let d = h.app.active_doc_mut().unwrap();
        (d.osc.analog[1].primary, d.osc.analog[1].secondary, d.osc.analog[1].is_primary) = (1234.0, 1.0, true);
        d.chans[0].primary = true;
        d.chans[1].primary = false;
        d.recompute(false);
        let k_main = d.osc.samples_per_cycle().max(2) * 2;
        let n = d.period_of(1);
        let want_k = 1.0 / 1234.0;
        assert_eq!(d.shown_k(1), want_k);
        let shown = d.shown_samples(1).to_vec();
        let (_, _, _, mx, mn, rms, _, val) = lane_stats(d, &Lane::Analog(1), k_main);
        assert_eq!(val.to_bits(), shown[k_main].to_bits(), "the value at the cursor is the shown series'");
        assert_eq!(rms.to_bits(), rms_at(&shown, k_main, n).to_bits(), "СКЗ over the same shown samples");
        let (lo, hi) = (shown.iter().copied().fold(f64::MAX, f64::min), shown.iter().copied().fold(f64::MIN, f64::max));
        assert_eq!((mn, mx), (lo, hi), "Max / Min in the same factor");
        let (rlo, rhi) = chan_range(d, 1);
        assert!(rlo >= lo - (hi - lo) && rhi <= hi + (hi - lo), "the zoom range is in the same factor: {rlo}..{rhi} vs {lo}..{hi}");
    }

    /// `0x462b15`: a lane's «*K» icon follows the channel's own primary / secondary flag, not the document's.
    #[test]
    fn a_lanes_ratio_icon_follows_the_channels_own_mode() {
        let mut h = Harness::new();
        let d = h.app.active_doc_mut().unwrap();
        (d.osc.analog[1].primary, d.osc.analog[1].secondary, d.osc.analog[1].is_primary) = (1234.0, 1.0, true);
        // The document (channel 0) shows primary values, this channel (stored primary) secondary: its
        // ratio applies. Without the fix the document's mode decided and the icon was missing.
        (d.chans[0].primary, d.chans[1].primary) = (true, false);
        assert!(key_icons(d, Lane::Analog(1)).contains(&"*K"), "the channel's own mode scales it");
        // The document shows secondary values, this channel primary (as stored): no ratio. Without the
        // fix the icon was wrongly there.
        (d.chans[0].primary, d.chans[1].primary) = (false, true);
        assert!(!key_icons(d, Lane::Analog(1)).contains(&"*K"), "the channel's own mode leaves it as stored");
    }

    /// `0x462b22`–`0x462b8f`: the «*K» icon tests the stored ratio values (either not 1), not the factor.
    #[test]
    fn a_lanes_ratio_icon_tests_the_stored_values() {
        let mut h = Harness::new();
        let d = h.app.active_doc_mut().unwrap();
        d.osc.analog[1].is_primary = true;
        d.chans[1].primary = false;
        // primary == secondary != 1: the factor is 1, the original still shows the icon.
        (d.osc.analog[1].primary, d.osc.analog[1].secondary) = (7.5, 7.5);
        assert!(key_icons(d, Lane::Analog(1)).contains(&"*K"), "equal values other than 1");
        // A zero value: the factor guard gave 1, the original shows the icon.
        (d.osc.analog[1].primary, d.osc.analog[1].secondary) = (1234.0, 0.0);
        assert!(key_icons(d, Lane::Analog(1)).contains(&"*K"), "a zero secondary");
        (d.osc.analog[1].primary, d.osc.analog[1].secondary) = (0.0, 1.0);
        assert!(key_icons(d, Lane::Analog(1)).contains(&"*K"), "a zero primary");
        // Both 1: none. Flags equal: none whatever the values.
        (d.osc.analog[1].primary, d.osc.analog[1].secondary) = (1.0, 1.0);
        assert!(!key_icons(d, Lane::Analog(1)).contains(&"*K"), "both 1");
        (d.osc.analog[1].primary, d.osc.analog[1].secondary) = (7.5, 7.5);
        d.chans[1].primary = true;
        assert!(!key_icons(d, Lane::Analog(1)).contains(&"*K"), "same flags");
        // The other direction: stored secondary, shown primary.
        d.osc.analog[1].is_primary = false;
        d.chans[1].primary = true;
        assert!(key_icons(d, Lane::Analog(1)).contains(&"*K"), "stored secondary, shown primary: equal values");
        (d.osc.analog[1].primary, d.osc.analog[1].secondary) = (1.0, 1.0);
        assert!(!key_icons(d, Lane::Analog(1)).contains(&"*K"), "stored secondary, shown primary: both 1");
        (d.osc.analog[1].primary, d.osc.analog[1].secondary) = (0.0, 1.0);
        assert!(key_icons(d, Lane::Analog(1)).contains(&"*K"), "stored secondary, shown primary: a zero");
    }

    /// With «Удалить постоянную» an instantaneous value is the accessor's `x·k − dc·k`, not `(x − dc)·k`.
    #[test]
    fn an_instantaneous_value_follows_the_accessors_remove_dc_order() {
        let mut h = Harness::new();
        let d = h.app.active_doc_mut().unwrap();
        (d.osc.analog[0].primary, d.osc.analog[0].secondary, d.osc.analog[0].is_primary) = (1234.0, 7.0, false);
        d.chans[0].primary = true;
        d.chans[0].remove_dc = true;
        d.recompute(true);
        let k = d.shown_k(0);
        assert_eq!(k, 1234.0 / 7.0);
        let dc = d.dc_mean[0].0;
        let mut differs = false;
        for j in 0..d.osc.sample_count() {
            let x = d.osc.analog[0].samples[j];
            let want = x * k - dc * k;
            differs |= want.to_bits() != ((x - dc) * k).to_bits();
            let (.., val) = lane_stats(d, &Lane::Analog(0), j);
            assert_eq!(val.to_bits(), want.to_bits(), "sample {j}");
        }
        assert!(differs, "the record must tell the two orders apart");
    }

    /// The визир hit test (`0x495800`) uses trunc(leftW + (s − first)·25·f),
    /// not the painter's column search: В1 on `first` is not drawn (the
    /// painter's fallback quirk) yet is grabbed just right of the stats panel.
    #[test]
    fn a_vizir_on_the_first_sample_is_grabbed_at_the_plot_edge() {
        let mut h = Harness::new();
        let d = h.app.active_doc_mut().unwrap();
        d.main_cursor = d.first;
        d.v_shown[0] = true;
        h.frame(vec![]);
        let left = 8.0 + h.app.view_left;
        let y = h.row(0).y;
        drag(&mut h, Pos2::new(left + 1.0, y), Pos2::new(left + 200.0, y));
        assert!(h.doc().main_cursor > h.doc().first, "В1 dragged from the plot edge: {}", h.doc().main_cursor);
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
        let rows = lane_layout(h.doc(), h.app.y_analog);
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

    /// Press, move, release with the left button.
    fn drag(h: &mut Harness, from: Pos2, to: Pos2) {
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(from), ev(from, true)]);
        for t in [0.25f32, 0.5, 0.75, 1.0] {
            h.frame(vec![egui::Event::PointerMoved(from + (to - from) * t)]);
        }
        h.frame(vec![ev(to, false)]);
        h.frame(vec![]);
    }

    /// `0x45ef09`: a pointer move over the plot picks the element under
    /// it; moves over the stats panel or leaving the view keep it (drawn
    /// selected with its info box); a move off it in the plot drops it.
    #[test]
    fn the_comment_under_the_pointer_sticks_until_the_next_plot_move() {
        use crate::comments::{Comment, CommentKind};
        let mut h = Harness::new();
        let y0 = h.row(0).y;
        h.app.dispatch(Command::CommentMode(1));
        h.button(Pos2::new(500.0, y0), egui::PointerButton::Primary);
        let x = 500.0;
        assert!(matches!(h.doc().comments[0].kind, CommentKind::Moment { .. }));
        let info_boxes = |h: &Harness| {
            h.shapes.iter().filter(|c| matches!(&c.shape, egui::Shape::Rect(r) if r.fill == Color32::from_rgb(255, 230, 230))).count()
        };
        h.frame(vec![egui::Event::PointerMoved(Pos2::new(x + 2.0, y0))]);
        assert_eq!(h.doc().comment_hot, Some(0));
        h.frame(vec![egui::Event::PointerMoved(h.row(0))]);
        assert_eq!(h.doc().comment_hot, Some(0), "the stats panel keeps it");
        h.frame(vec![egui::Event::PointerGone]);
        h.frame(vec![]);
        assert_eq!(h.doc().comment_hot, Some(0), "leaving the view keeps it");
        assert_eq!(info_boxes(&h), 1, "still drawn selected");
        h.frame(vec![egui::Event::PointerMoved(Pos2::new(x + 200.0, y0))]);
        assert_eq!(h.doc().comment_hot, None, "a plot move off it drops it");
        h.frame(vec![]);
        assert_eq!(info_boxes(&h), 0);
        // A second element: still none until the pointer moves onto it.
        h.app.active_doc_mut().unwrap().comments.push(Comment::new(CommentKind::Moment { pos: 0 }));
        h.frame(vec![]);
        assert_eq!(h.doc().comment_hot, None);
    }

    /// `0x43fab0` / `0x45f660` / `0x45dc10`: the first unlocked element's
    /// part sets the cursor (a level line's body N–S, an end W–E) and the
    /// drag; a locked one is selected but neither grabbed nor passed
    /// through to a визир; a hidden text does not stop a drag.
    #[test]
    fn comment_cursor_codes_lock_and_visir_suppression() {
        use egui::CursorIcon::{ResizeHorizontal, ResizeVertical};
        let mut h = Harness::new();
        let y0 = h.row(0).y;
        h.app.dispatch(Command::CommentMode(2));
        drag(&mut h, Pos2::new(600.0, y0), Pos2::new(700.0, y0));
        let at = |x: f32| Pos2::new(x, y0);
        let hover = |h: &mut Harness, p: Pos2| {
            h.frame(vec![egui::Event::PointerMoved(p)]);
            h.cursor
        };
        hover(&mut h, at(900.0));
        assert_eq!(hover(&mut h, at(650.0)), ResizeVertical, "the body moves the line vertically");
        assert_eq!(hover(&mut h, at(600.0)), ResizeHorizontal, "an end");
        // Locked: selected, but an arrow, no drag and no В1 under it.
        h.app.active_doc_mut().unwrap().comments[0].locked = true;
        hover(&mut h, at(900.0));
        assert_ne!(hover(&mut h, at(650.0)), ResizeVertical);
        assert_eq!(h.doc().comment_hot, Some(0));
        let before = h.doc().comments[0].kind.clone();
        drag(&mut h, at(600.0), at(500.0));
        assert_eq!(h.doc().comments[0].kind, before, "a locked line stays");
        assert!(!h.doc().v_shown[0], "no визир under a selected element");
        // Unlocked with its text hidden: the end still drags.
        let d = h.app.active_doc_mut().unwrap();
        d.comments[0].locked = false;
        d.comments[0].hidden = true;
        hover(&mut h, at(600.0));
        drag(&mut h, at(600.0), at(500.0));
        assert_ne!(h.doc().comments[0].kind, before, "a hidden text does not stop the drag");
    }

    /// 33042's commit (`0x465df0` → `0x4404b0`): `_wtof` of the text, kept
    /// only within the channel's range, on the element selected now; the
    /// document marked modified on every commit, not on Esc.
    #[test]
    fn the_level_edit_commits_as_the_original() {
        use crate::comments::{Comment, CommentKind};
        let key = |k| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        let mut h = Harness::new();
        let d = h.app.active_doc_mut().unwrap();
        d.comments.push(Comment { kind: CommentKind::Level { chan: crate::app::ChanKey::Analog(0), value: 0.0, from: 0, to: 50 }, text: String::new(), locked: false, hidden: false });
        d.comment_hot = Some(0);
        d.set.modified = false;
        let value = |h: &Harness| match h.doc().comments[0].kind {
            CommentKind::Level { value, .. } => value,
            _ => f64::NAN,
        };
        for (text, k, want, modified) in [("1,5", egui::Key::Enter, 1.0, true), ("1e9", egui::Key::Enter, 1.0, true), ("2", egui::Key::Escape, 1.0, false)] {
            h.app.active_doc_mut().unwrap().set.modified = false;
            h.app.level_edit = Some((0, text.into()));
            h.frame(vec![]);
            h.frame(vec![key(k)]);
            // At the key-up, as the rename edit (clone): open while it is down.
            h.frame(vec![]);
            assert!(h.app.level_edit.is_some(), "{text}: open while the key is down");
            h.frame(vec![egui::Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() }]);
            h.frame(vec![]);
            assert!(h.app.level_edit.is_none(), "{text}: closed");
            assert_eq!(value(&h), want, "{text}");
            assert_eq!(h.doc().set.modified, modified, "{text}: modified");
        }
    }

    /// A channel drag resting in the view's bottom 11 px scrolls a 100 px
    /// line down every 50 ms after the first 50 ms; in the top 11 px, up.
    #[test]
    fn a_channel_drag_scrolls_at_the_top_and_bottom_edges() {
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let mut h = Harness::new();
        h.app.y_analog = 400.0;
        h.frame(vec![]);
        let o = h.row(0);
        h.frame(vec![egui::Event::PointerMoved(o), ev(o, true)]);
        h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(1.0, 0.0))]);
        h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(5.0, 0.0))]);
        assert!(egui::DragAndDrop::has_payload_of_type::<ChanDrag>(&h.ctx));
        let client = h.app.view_rects.unwrap().1;
        let bottom = Pos2::new(o.x + 5.0, client.bottom() - 5.0);
        h.frame(vec![egui::Event::PointerMoved(bottom)]);
        assert_eq!(h.doc().vscroll, 0.0, "not before 50 ms");
        h.frame(vec![]);
        assert_eq!(h.doc().vscroll, 0.0, "50 ms is not more than 50 ms");
        h.frame(vec![]);
        assert_eq!(h.doc().vscroll, -100.0);
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(h.doc().vscroll, -200.0);
        // A new code restarts the delay; the top inset scrolls up.
        let top = Pos2::new(o.x + 5.0, client.top() + 5.0);
        h.frame(vec![egui::Event::PointerMoved(top)]);
        h.frame(vec![]);
        assert_eq!(h.doc().vscroll, -200.0, "restarted");
        h.frame(vec![]);
        assert_eq!(h.doc().vscroll, -100.0, "the top inset scrolls up");
        // The side bands scroll down (`0x468e40`).
        let side = Pos2::new(client.left() + 5.0, client.center().y);
        h.frame(vec![egui::Event::PointerMoved(side)]);
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(h.doc().vscroll, -200.0, "the left band scrolls down");
        h.frame(vec![ev(top, false)]);
    }

    /// MFC's OnBeginDrag after the stats panel's DoDragDrop (`0x45ee88`):
    /// the drag starts once the pointer leaves [x − 2, x + 2) around where
    /// the first move was, or after 200 ms held; a release first cancels.
    #[test]
    fn a_stats_drag_starts_after_2_px_or_200_ms() {
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let started = |h: &Harness| egui::DragAndDrop::has_payload_of_type::<ChanDrag>(&h.ctx);
        let mut h = Harness::new();
        let o = h.row(0);
        let press = |h: &mut Harness| {
            h.frame(vec![egui::Event::PointerMoved(o), ev(o, true)]);
            h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(1.0, 0.0))]);
        };
        // Held still: 0.05 s a frame, so the fifth frame is past 200 ms.
        press(&mut h);
        for _ in 0..3 {
            h.frame(vec![]);
            assert!(!started(&h));
        }
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(started(&h), "200 ms held");
        h.frame(vec![ev(o, false)]);
        h.frame(vec![]);
        assert!(!started(&h), "dropped on its own lane: nothing");
        assert!(h.app.dlg.alert().is_none(), "no message");
        // Released early: no drag.
        press(&mut h);
        h.frame(vec![ev(o, false)]);
        for _ in 0..5 {
            h.frame(vec![]);
        }
        assert!(!started(&h));
        // Two left of the anchor stays inside, three leaves; two right leaves.
        press(&mut h);
        h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(-1.0, 0.0))]);
        assert!(!started(&h));
        h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(-2.0, 0.0))]);
        assert!(started(&h));
        h.frame(vec![ev(o, false)]);
        h.frame(vec![]);
        press(&mut h);
        h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(3.0, 0.0))]);
        assert!(started(&h));
        h.frame(vec![ev(o, false)]);
        h.frame(vec![]);
    }

    /// `0x465df0`: the inline rename takes the text as typed, refuses a
    /// name another channel has with message box 397, and marks the record
    /// modified on every commit.
    #[test]
    fn inline_rename_commits_as_typed_or_refuses_with_397() {
        let enter = || egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        let enter_up = || egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() };
        let mut h = Harness::new();
        h.app.active_doc_mut().unwrap().init_set_keys();
        let commit = |h: &mut Harness, text: &str| {
            h.app.rename_edit = Some((0, text.to_string()));
            h.frame(vec![]);
            h.frame(vec![enter()]);
            // Enter commits when it is let go, not while it is down (L9.106).
            h.frame(vec![]);
            assert!(h.app.rename_edit.is_some(), "still open while the key is down");
            h.frame(vec![enter_up()]);
            h.frame(vec![]);
        };
        let other = h.doc().osc.analog[1].id.clone();
        let own = h.doc().osc.analog[0].id.clone();
        h.app.active_doc_mut().unwrap().set.modified = false;
        commit(&mut h, &other);
        assert_eq!(h.app.dlg.alert(), Some(crate::chandlg::MSG_NAME_TAKEN));
        assert_eq!(h.doc().osc.analog[0].id, own, "refused");
        assert!(h.doc().set.modified, "modified even when refused");
        h.app.dlg.alerts.clear();
        commit(&mut h, &other.to_lowercase());
        assert!(h.app.dlg.alert().is_none(), "the comparison is case-sensitive");
        commit(&mut h, " Ia 2 ");
        assert_eq!(h.doc().osc.analog[0].id, " Ia 2 ", "no trim");
        // A press elsewhere in the view commits too (`0x466100`); idle
        // frames (the focus anywhere) leave the edit open.
        h.app.rename_edit = Some((0, "Ia 3".to_string()));
        for _ in 0..3 {
            h.frame(vec![]);
        }
        assert!(h.app.rename_edit.is_some(), "still open");
        let at = Pos2::new(600.0, h.row(1).y);
        h.frame(vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: true, modifiers: Default::default() }]);
        h.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: false, modifiers: Default::default() }]);
        assert_eq!(h.doc().osc.analog[0].id, "Ia 3", "committed by the press");
        commit(&mut h, "");
        assert_eq!(h.doc().osc.analog[0].id, own, "an empty alias shows the file's name (VM)");
        // Another channel's file name is refused too, though it shows
        // another name now.
        h.app.dlg.alerts.clear();
        commit(&mut h, "Ia 4");
        h.app.rename_edit = Some((1, own.clone()));
        h.frame(vec![]);
        h.frame(vec![enter()]);
        h.frame(vec![enter_up()]);
        h.frame(vec![]);
        assert_eq!(h.app.dlg.alert(), Some(crate::chandlg::MSG_NAME_TAKEN));
        assert_eq!(h.doc().osc.analog[1].id, other);
    }

    /// The inline rename as «Каналы»'s: a level keeps its channel's old name
    /// (unbound, L9.227), and a formula takes the new one (issue #7).
    #[test]
    fn an_inline_rename_unbinds_a_level_and_feeds_the_formulas() {
        use crate::comments::{Comment, CommentKind};
        let enter = || egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        let mut h = Harness::new();
        let old = h.doc().osc.analog[0].id.clone();
        h.app.active_doc_mut().unwrap().comments.push(Comment { kind: CommentKind::Level { chan: ChanKey::Analog(0), value: 1.0, from: 10, to: 20 }, text: String::new(), locked: false, hidden: false });
        h.app.rename_edit = Some((0, "Новое".to_string()));
        h.frame(vec![]);
        h.frame(vec![enter()]);
        h.frame(vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() }]);
        h.frame(vec![]);
        assert_eq!(h.doc().osc.analog[0].id, "Новое");
        assert!(matches!(&h.doc().comments[0].kind, CommentKind::Kept(r) if matches!(&r.kind, fvb_core::recset::CommentKindRec::Level { channel, .. } if *channel == old)));
        assert!(h.app.append_computed(&[("X".into(), "Новое*2".into())]).is_some(), "{:?}", h.app.dlg.alert());
    }

    /// `0x43eb40`: a level line goes on any analog-kind lane — a formula
    /// channel's too — and is saved by that channel's name.
    #[test]
    fn a_level_line_on_a_formula_channel() {
        use crate::comments::CommentKind;
        let mut h = Harness::new();
        h.app.append_computed(&[("X".into(), "Ua*2".into())]).unwrap();
        for c in h.app.active_doc_mut().unwrap().chans.iter_mut().skip(1) {
            c.visible = false;
        }
        h.frame(vec![]);
        let y = h.row(1).y;
        h.app.dispatch(Command::CommentMode(2));
        drag(&mut h, Pos2::new(600.0, y), Pos2::new(700.0, y));
        assert!(matches!(h.doc().comments.last().map(|c| &c.kind), Some(CommentKind::Level { chan: ChanKey::Computed(0), .. })), "{:?}", h.doc().comments);
    }

    #[test]
    fn comment_tools_are_one_shot_and_drag_out_levels_and_areas() {
        use crate::comments::CommentKind;
        let mut h = Harness::new();
        let (y0, y1) = (h.row(0).y, h.row(1).y);
        h.app.dispatch(Command::CommentMode(1));
        h.button(Pos2::new(500.0, y0), egui::PointerButton::Primary);
        assert!(matches!(h.doc().comments[0].kind, CommentKind::Moment { .. }));
        assert_eq!(h.doc().comment_mode, 0, "the moment tool pops out");
        assert!(!h.doc().v_shown[0], "placing a comment does not place В1");

        h.app.dispatch(Command::CommentMode(2));
        drag(&mut h, Pos2::new(700.0, y0), Pos2::new(600.0, y0));
        match h.doc().comments[1].kind {
            CommentKind::Level { chan, from, to, .. } => {
                assert_eq!(chan, ChanKey::Analog(0));
                assert!(from < to, "normalised on release");
            }
            ref k => panic!("{k:?}"),
        }
        assert_eq!(h.doc().comment_hot, Some(1), "the level just drawn stays selected (VM)");
        assert_eq!(h.doc().comment_mode, 0);

        h.app.dispatch(Command::CommentMode(3));
        drag(&mut h, Pos2::new(800.0, y0), Pos2::new(900.0, y1));
        assert!(matches!(h.doc().comments[2].kind, CommentKind::Area { chan_from: ChanKey::Analog(0), chan_to: ChanKey::Analog(1), .. }));

        // Grab the moment and move it; a locked one stays.
        let before = h.doc().comments[0].moment().unwrap();
        drag(&mut h, Pos2::new(500.0, y1 + 20.0), Pos2::new(560.0, y1 + 20.0));
        let moved = h.doc().comments[0].moment().unwrap();
        assert!(moved > before, "{moved} > {before}");
        h.app.active_doc_mut().unwrap().comments[0].locked = true;
        let x = 560.0;
        drag(&mut h, Pos2::new(x, y1 + 20.0), Pos2::new(x + 60.0, y1 + 20.0));
        assert_eq!(h.doc().comments[0].moment().unwrap(), moved, "locked");

        // The comment menu is relabelled by kind.
        h.button(Pos2::new(x, y1 + 20.0), egui::PointerButton::Secondary);
        assert_eq!(h.app.lane_menu.map(|m| m.kind), Some(LaneMenuKind::Comment(0)));
    }

    #[test]
    fn rectangle_zoom_windows_the_lane_and_141_resets_it() {
        let mut h = Harness::new();
        let r0 = lane_layout(h.doc(), h.app.y_analog)[0].clone();
        let top = 8.0;
        h.app.dispatch(Command::RectZoom);
        assert_eq!(h.app.rect_zoom, RectZoom::Armed);
        let full = chan_range(h.doc(), 0);
        // From a quarter of the lane down to its middle.
        drag(&mut h, Pos2::new(600.0, top + r0.h * 0.25), Pos2::new(600.0, top + r0.h * 0.5));
        assert_eq!(h.app.rect_zoom, RectZoom::Off, "one-shot");
        let w = h.doc().chans[0].window.expect("window zoom applied");
        let span = full.1 - full.0;
        assert!((w.hi - (full.1 - 0.25 * span)).abs() < 1e-6 * span.abs().max(1.0));
        assert!((w.lo - (full.0 + 0.5 * span)).abs() < 1e-6 * span.abs().max(1.0));
        h.app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        h.app.dispatch(Command::ResetZoom);
        assert!(h.doc().chans[0].window.is_none());
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
        // Quick presses landing in one frame each move (VM: three → +3).
        let r = || key(egui::Key::ArrowRight, egui::Modifiers::NONE);
        h.frame_mod(vec![r(), r(), r()], egui::Modifiers::NONE);
        assert_eq!(h.doc().main_cursor, v1 + 4);
        let l = || key(egui::Key::ArrowLeft, egui::Modifiers::NONE);
        h.frame_mod(vec![l(), l(), l()], egui::Modifiers::NONE);
        assert_eq!(h.doc().main_cursor, v1 + 1);
        // A hidden визир is shown by the key (`0x492f20`); Alt does nothing.
        let d = h.app.active_doc_mut().unwrap();
        (d.v_shown[0], d.v1_placed) = (false, false);
        h.frame_mod(vec![key(egui::Key::ArrowLeft, egui::Modifiers::ALT)], egui::Modifiers::ALT);
        assert!(!h.doc().v_shown[0], "Alt+← is a WM_SYSKEYDOWN");
        h.frame_mod(vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)], egui::Modifiers::NONE);
        assert!(h.doc().v_shown[0] && h.doc().main_cursor == v1, "shown, moved back");
        assert!(h.doc().v1_placed, "the key places В1 too");
        // With a diagram focused the keys are the pane's.
        h.app.copy_focus = crate::app::CopyFocus::Diagram(crate::app::Pane::Vector);
        h.frame_mod(vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)], egui::Modifiers::NONE);
        assert_eq!(h.doc().main_cursor, v1, "not the view's key");
        // ↑ on a ГВ never placed puts it at the view's top (`0x4944a0` → −1,
        // clamped to 0): on the top lane, shown.
        h.app.copy_focus = crate::app::CopyFocus::View;
        h.app.active_doc_mut().unwrap().hviz[0] = crate::app::HVizir::default();
        h.frame_mod(vec![key(egui::Key::ArrowUp, egui::Modifiers::NONE)], egui::Modifiers::NONE);
        let hv = h.doc().hviz[0];
        assert!(hv.shown && hv.chan == Some(0), "{hv:?}");
    }

    /// MFC's menu rows: a checked item's amber box in the column (its
    /// image in it, or a tick / bullet without one), none for an unchecked
    /// one (VM).
    #[test]
    fn menu_rows_tick_only_checked_items() {
        let mut h = Harness::new();
        h.app.active_doc_mut().unwrap().chans[0].invert = true;
        h.app.active_doc_mut().unwrap().chans[0].on_vector = true;
        let top = h.row(0);
        h.button(top, egui::PointerButton::Secondary);
        assert!(h.app.lane_menu.is_some());
        let texts: Vec<String> = h.shapes.iter().filter_map(|c| if let Shape::Text(t) = &c.shape { Some(t.galley.text().to_string()) } else { None }).collect();
        assert!(texts.iter().any(|t| t == "Инвертировать"));
        let before = |mark: &str, label: &str| texts.windows(2).any(|w| w[0] == mark && w[1] == label);
        assert!(before("•", "Отображать на векторной диаграмме"), "SetRadio: a bullet");
        assert_eq!(texts.iter().filter(|t| *t == "✓").count(), 0, "items with an image show it, not a tick");
        let boxes = h.shapes.iter().filter(|c| matches!(&c.shape, Shape::Rect(r) if r.fill == Color32::from_rgb(255, 227, 149))).count();
        assert_eq!(boxes, 3, "Инвертировать, Мгновенные значения and the vector flag");
    }

    /// The view's menus follow the keyboard as a Win32 popup: ↓ lands on
    /// the first enabled row, Enter picks it; the arrows do not move a
    /// визир meanwhile.
    #[test]
    fn view_menus_follow_the_keyboard() {
        let mut h = Harness::new();
        let top = h.row(0);
        h.button(top, egui::PointerButton::Secondary);
        assert!(h.app.lane_menu.is_some());
        let (before, v1) = (h.doc().chans[0].on_freq, h.doc().main_cursor);
        let key = |k| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        h.frame(vec![key(egui::Key::ArrowDown)]);
        h.frame(vec![key(egui::Key::Enter)]);
        h.frame(vec![]);
        assert!(h.app.lane_menu.is_none(), "picked and closed");
        assert_ne!(h.doc().chans[0].on_freq, before, "the first row: «Отображать на частотной диаграмме»");
        assert_eq!(h.doc().main_cursor, v1);
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

    /// A quick click: press and release in one frame (egui then clears its
    /// press origin; VM: the harness's clicks on the software runner).
    #[test]
    fn a_quick_click_on_the_plot_still_acts() {
        let mut h = Harness::new();
        let at = Pos2::new(700.0, h.row(0).y);
        let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(at)]);
        h.frame(vec![ev(true), ev(false)]);
        h.frame(vec![]);
        assert!(h.doc().v_shown[1] && h.doc().aux_cursor > 0, "В2 placed by a one-frame click");
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

    /// #23: the lane menu's picks that reach the refresh-all in the original (33030's dialog aside) — the flags,
    /// the value modes, 33033 and 33037 — clear a stale circle / vector flag.
    #[test]
    fn lane_menu_picks_end_in_the_refresh_all() {
        let picks: [(&str, MenuCmd); 4] = [
            ("flag", MenuCmd::Flag(ChanFlag::OnFreq)),
            ("mode", MenuCmd::Mode(ValueMode::Rms)),
            ("33033", MenuCmd::DiffBand),
            ("33037", MenuCmd::Split),
        ];
        for (what, cmd) in picks {
            let mut app = App::new();
            app.dispatch(crate::app::Command::Demo);
            let d = app.active_doc_mut().unwrap();
            d.selection = std::collections::BTreeSet::from([ChanKey::Analog(1)]);
            crate::app::stale_flags::make(&mut app);
            assert!(crate::app::stale_flags::is_stale(&app), "{what}");
            app.run_menu_cmd(0, cmd);
            assert!(crate::app::stale_flags::swept(&app), "{what}");
        }
    }

    /// 33035 (`0x467a62` → `0x42f4b0`) ends in the refresh-all once it has combined; a refused one does not.
    #[test]
    fn combining_lanes_ends_in_the_refresh_all() {
        let mut app = App::new();
        app.dispatch(crate::app::Command::Demo);
        app.active_doc_mut().unwrap().selection = std::collections::BTreeSet::from([ChanKey::Analog(0), ChanKey::Analog(1)]);
        crate::app::stale_flags::make(&mut app);
        app.active_doc_mut().unwrap().osc.analog[1].unit = "Hz".into();
        assert!(app.active_doc().unwrap().can_combine());
        app.run_menu_cmd(0, MenuCmd::Combine);
        let d = app.active_doc().unwrap();
        assert!(d.group_of(ChanKey::Analog(0)).is_some(), "combined");
        assert!(crate::app::stale_flags::swept(&app));
        // Refused for different rates: nothing changes, no refresh.
        let mut app = App::new();
        app.dispatch(crate::app::Command::Demo);
        let d = app.active_doc_mut().unwrap();
        d.osc.analog[0].unit = "Hz".into();
        d.osc.analog[1].unit = "Hz".into();
        d.osc.analog[1].own_rate = 1234.5;
        d.selection = std::collections::BTreeSet::from([ChanKey::Analog(0), ChanKey::Analog(1)]);
        d.chans[0].on_circle = true;
        app.run_menu_cmd(0, MenuCmd::Combine);
        assert!(app.active_doc().unwrap().chans[0].on_circle, "a refused 33035 stops before the refresh");
    }

    /// #23: the view's scroll (`OnScrollBy` `0x468f00` → `0x468f3d`) and the lane rename (`0x465df0` → `0x46607c`)
    /// end in the refresh-all, which clears a stale circle / vector flag.
    #[test]
    fn scrolling_and_renaming_end_in_the_refresh_all() {
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let mut h = Harness::new();
        h.app.y_analog = 400.0;
        h.frame(vec![]);
        let o = h.row(0);
        h.frame(vec![egui::Event::PointerMoved(o), ev(o, true)]);
        h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(1.0, 0.0))]);
        h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(5.0, 0.0))]);
        let client = h.app.view_rects.unwrap().1;
        crate::app::stale_flags::make(&mut h.app);
        h.frame(vec![egui::Event::PointerMoved(Pos2::new(o.x + 5.0, client.bottom() - 5.0))]);
        for _ in 0..3 {
            h.frame(vec![]);
        }
        assert_eq!(h.doc().vscroll, -100.0, "the drag scrolled");
        assert!(crate::app::stale_flags::swept(&h.app), "scroll");
        // The rename.
        let mut h = Harness::new();
        h.app.active_doc_mut().unwrap().init_set_keys();
        crate::app::stale_flags::make(&mut h.app);
        h.app.rename_edit = Some((1, "Zz9".to_string()));
        h.frame(vec![]);
        h.frame(vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        h.frame(vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() }]);
        h.frame(vec![]);
        assert_eq!(h.doc().osc.analog[1].id, "Zz9");
        assert!(crate::app::stale_flags::swept(&h.app), "rename");
    }

    /// #23: Esc on a rename (`0x465df0` → `0x46607c`) and a click that selects a channel (`0x45dc10` → `0x45e312`)
    /// reach the refresh-all too.
    #[test]
    fn rename_escape_and_a_selecting_click_refresh() {
        let mut h = Harness::new();
        crate::app::stale_flags::make(&mut h.app);
        h.app.rename_edit = Some((1, "Zz9".to_string()));
        h.frame(vec![]);
        h.frame(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        h.frame(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() }]);
        h.frame(vec![]);
        assert!(h.app.rename_edit.is_none());
        assert!(crate::app::stale_flags::swept(&h.app), "Esc");
        let mut h = Harness::new();
        h.frame(vec![]);
        h.app.active_doc_mut().unwrap().selection.clear();
        crate::app::stale_flags::make(&mut h.app);
        let o = h.row(0);
        let ev = |pressed| egui::Event::PointerButton { pos: o, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(o), ev(true)]);
        h.frame(vec![ev(false)]);
        assert!(!h.doc().selection.is_empty(), "the click selected the lane");
        assert!(crate::app::stale_flags::swept(&h.app), "click");
    }

    /// #23: dragging a pending pasted lane in time (the view's mouse-move case 6, `0x45ec2a`) ends in the
    /// refresh-all: a stale flag is swept.
    #[test]
    fn dragging_a_pasted_lane_in_time_refreshes() {
        let mut h = Harness::new();
        h.app.active_doc_mut().unwrap().selection = [ChanKey::Analog(1)].into_iter().collect();
        h.app.copy_channels_for_merge();
        h.app.paste_channels_for_merge();
        h.frame(vec![]);
        h.frame(vec![]);
        let rows = lane_layout(h.doc(), h.app.y_analog);
        let pend = rows.iter().position(|r| matches!(r.key, Lane::Computed(j) if h.doc().pending_of(j).is_some())).expect("the pasted lane");
        let o = h.row(pend) + Vec2::new(150.0, 0.0);
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(o), ev(o, true)]);
        h.frame(vec![]);
        let before = h.doc().merge_offset;
        crate::app::stale_flags::make(&mut h.app);
        h.frame(vec![egui::Event::PointerMoved(o + Vec2::new(40.0, 0.0))]);
        h.frame(vec![]);
        assert_ne!(h.doc().merge_offset, before, "the lane moved in time");
        assert!(crate::app::stale_flags::swept(&h.app));
    }
}
