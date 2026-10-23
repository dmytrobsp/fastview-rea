//! The channel dialogs of the ribbon: «Каналы» (32862, DIALOG 463
//! «Отображение каналов», `ChannelsDlg`) and «Коэффициенты» (32866, DIALOG
//! 477 «Коэффициенты трансформации», `CMultiplierDlg`).

use crate::app::{App, ChanKey, Doc};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// DIALOG 463 «Отображение каналов»
// ---------------------------------------------------------------------------

/// Columns of the channel grid (`0x433970`): 0 the name, 1–5 image columns.
pub const COLS: usize = 6;

/// Message 397.
pub const MSG_NAME_TAKEN: &str = "Данное имя канала уже используется.";
/// Message 398.
pub const MSG_NO_RENAME_COMPUTED: &str = "Переименование расчетных каналов запрещено.";

/// State of the open «Каналы» dialog: the in-place rename edit, if any.
#[derive(Debug, Clone, Default)]
pub struct ChannelsDlg {
    pub rename: Option<(ChanKey, String)>,
    /// Top-left of the header row at the last frame (for tests).
    pub origin: Option<Pos2>,
}

/// Rows in the original's order: the analog channels (file channels, then
/// the computed ones, which the original keeps in the same vector with bit
/// 13), then the discrete channels (kind 1). The two digital-entry groups
/// (`doc+0x878` / `+0x884`, columns 4/5) have no counterpart in our record.
pub fn rows(doc: &Doc) -> Vec<ChanKey> {
    (0..doc.osc.analog.len())
        .map(ChanKey::Analog)
        .chain((0..doc.computed.len()).map(ChanKey::Computed))
        .chain((0..doc.osc.digital.len()).map(ChanKey::Digital))
        .collect()
}

pub fn row_name(doc: &Doc, key: ChanKey) -> String {
    match key {
        ChanKey::Analog(i) => doc.osc.analog[i].id.clone(),
        ChanKey::Computed(j) => doc.computed[j].name.clone(),
        ChanKey::Digital(k) => doc.osc.digital[k].id.clone(),
    }
}

/// The unit the eligibility tests read (`0x414c30`): empty in ADC mode.
fn unit_of(doc: &Doc, key: ChanKey) -> Option<String> {
    match key {
        ChanKey::Analog(i) => Some(if doc.chans[i].adc_counts { String::new() } else { doc.osc.analog[i].unit.clone() }),
        ChanKey::Computed(j) => Some(doc.computed[j].unit.clone()),
        ChanKey::Digital(_) => None,
    }
}

/// Column 3 (vector, bit 2; `0x434d0a`): an analog channel whose
/// canonical unit is "A", "V", «ном» (string 162) or empty — the same test
/// as the menu's (the click compares case-sensitively, the paint does not;
/// canonical units make that moot).
pub fn vector_unit_ok(unit: &str) -> bool {
    crate::diagram::vector_unit_ok(unit)
}

/// Column 4 (circle, bit 3; `0x434e81`): an analog channel, not pending a
/// merge, whose unit is Ohm (string 3060), W, VA or var (no-case compare).
pub fn circle_unit_ok(unit: &str) -> bool {
    ["Ohm", "W", "VA", "var"].iter().any(|u| unit.eq_ignore_ascii_case(u))
}

/// The flag a cell shows, or `None` where it does not apply (drawn grey).
pub fn cell(doc: &Doc, key: ChanKey, col: usize) -> Option<bool> {
    let unit = unit_of(doc, key);
    match (key, col) {
        (ChanKey::Analog(i), 1) => Some(doc.chans[i].visible),
        (ChanKey::Computed(j), 1) => Some(doc.computed[j].visible),
        (ChanKey::Digital(k), 1) => Some(doc.digital_visible.get(k).copied().unwrap_or(true)),
        // Column 2: analog, not pending a merge (our computed channels have
        // no frequency-diagram flag).
        (ChanKey::Analog(i), 2) => Some(doc.chans[i].on_freq),
        (ChanKey::Analog(i), 3) if vector_unit_ok(unit.as_deref().unwrap_or("")) => Some(doc.chans[i].on_vector),
        (ChanKey::Computed(j), 3) if vector_unit_ok(unit.as_deref().unwrap_or("")) => Some(doc.computed[j].on_vector),
        (ChanKey::Analog(i), 4) if circle_unit_ok(unit.as_deref().unwrap_or("")) => Some(doc.chans[i].on_circle),
        (ChanKey::Computed(j), 4) if circle_unit_ok(unit.as_deref().unwrap_or("")) => Some(doc.computed[j].on_circle),
        _ => None,
    }
}

/// Set a cell's flag (only where it applies). Hiding an analog channel also
/// dissolves its channel group and deselects it (`0x4349c0`, column 1).
pub fn set_cell(doc: &mut Doc, key: ChanKey, col: usize, v: bool) {
    if cell(doc, key, col).is_none() {
        return;
    }
    // Every click marks the document modified (`0x4349c0`).
    doc.set.modified = true;
    match (key, col) {
        (ChanKey::Analog(i), 1) => {
            doc.chans[i].visible = v;
            if !v {
                if let Some(g) = doc.chans[i].group {
                    for c in doc.chans.iter_mut().filter(|c| c.group == Some(g)) {
                        c.group = None;
                    }
                }
                doc.selection.remove(&key);
            }
        }
        (ChanKey::Computed(j), 1) => {
            doc.computed[j].visible = v;
            if !v {
                doc.selection.remove(&key);
            }
        }
        (ChanKey::Digital(k), 1) => {
            if let Some(f) = doc.digital_visible.get_mut(k) {
                *f = v;
            }
            if !v {
                doc.selection.remove(&key);
            }
        }
        (ChanKey::Analog(i), 2) => doc.chans[i].on_freq = v,
        (ChanKey::Analog(i), 3) => doc.chans[i].on_vector = v,
        (ChanKey::Computed(j), 3) => doc.computed[j].on_vector = v,
        (ChanKey::Analog(i), 4) => doc.chans[i].on_circle = v,
        (ChanKey::Computed(j), 4) => doc.computed[j].on_circle = v,
        _ => {}
    }
}

/// A cell click (NM_CLICK `0x4349c0`) flips the flag.
pub fn toggle_cell(doc: &mut Doc, key: ChanKey, col: usize) {
    if let Some(v) = cell(doc, key, col) {
        set_cell(doc, key, col, !v);
    }
}

/// A header click (HDN_ITEMCLICK `0x435080`): when every applicable row has
/// the flag it is cleared on all of them, otherwise set on all of them.
pub fn toggle_column(doc: &mut Doc, col: usize) {
    let keys: Vec<ChanKey> = rows(doc).into_iter().filter(|&k| cell(doc, k, col).is_some()).collect();
    if keys.is_empty() {
        return;
    }
    let all = keys.iter().all(|&k| cell(doc, k, col) == Some(true));
    for k in keys {
        set_cell(doc, k, col, !all);
    }
}

/// Can the name cell be edited? Computed channels (bit 13) answer 398.
pub fn can_rename(key: ChanKey) -> Result<(), &'static str> {
    match key {
        ChanKey::Computed(_) => Err(MSG_NO_RENAME_COMPUTED),
        _ => Ok(()),
    }
}

/// Commit an in-place rename (`0x435c80`): unchanged text does nothing; a
/// name another channel already has is refused with 397.
pub fn rename(doc: &mut Doc, key: ChanKey, name: &str) -> Result<(), &'static str> {
    can_rename(key)?;
    if name == row_name(doc, key) {
        return Ok(());
    }
    if rows(doc).into_iter().any(|k| k != key && row_name(doc, k) == name) {
        return Err(MSG_NAME_TAKEN);
    }
    match key {
        ChanKey::Analog(i) => doc.osc.analog[i].id = name.to_string(),
        ChanKey::Digital(k) => doc.osc.digital[k].id = name.to_string(),
        ChanKey::Computed(_) => {}
    }
    doc.set.modified = true;
    Ok(())
}

// ---------------------------------------------------------------------------
// DIALOG 477 «Коэффициенты трансформации»
// ---------------------------------------------------------------------------

/// `_wtof`: the longest leading decimal number (after white space), 0 when
/// there is none — the dialog does no validation.
pub fn wtof(s: &str) -> f64 {
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let d0 = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i > d0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let f0 = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits |= i > f0;
    }
    if !digits {
        return 0.0;
    }
    let mut end = i;
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let e0 = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > e0 {
            end = j;
        }
    }
    s[..end].parse().unwrap_or(0.0)
}

/// A cell of the coefficient grid: (row, column 0 = primary / 1 = secondary).
pub type Cell = (usize, usize);

/// The coefficient grid (`CMultiplierDlg`): one row per file analog channel,
/// `%g` texts of `ch+0xd0` / `ch+0xd8`, a rectangular Shift selection and an
/// in-place edit whose text fills every selected cell of its column.
#[derive(Debug, Clone, Default)]
pub struct CoefDlg {
    pub names: Vec<String>,
    pub cells: Vec<[String; 2]>,
    pub sel: BTreeSet<Cell>,
    pub anchor: Option<Cell>,
    pub edit: Option<(Cell, String)>,
    pub dirty: bool,
    /// Top-left of the header row at the last frame (for tests).
    pub origin: Option<Pos2>,
}

impl CoefDlg {
    pub fn new(doc: &Doc) -> CoefDlg {
        let g = fvb_core::views::fmt_g;
        CoefDlg {
            names: doc.osc.analog.iter().map(|c| c.id.clone()).collect(),
            cells: doc.osc.analog.iter().map(|c| [g(c.primary), g(c.secondary)]).collect(),
            ..Default::default()
        }
    }

    fn rect(a: Cell, b: Cell) -> BTreeSet<Cell> {
        let (r0, r1) = (a.0.min(b.0), a.0.max(b.0));
        let (c0, c1) = (a.1.min(b.1), a.1.max(b.1));
        (r0..=r1).flat_map(|r| (c0..=c1).map(move |c| (r, c))).collect()
    }

    /// A press on a cell (`0x47af90`): with Shift the selection becomes the
    /// rectangle from the anchor; without it the selection clears and the
    /// cell becomes the anchor. A running edit is committed first (into the
    /// selected cells of its column).
    pub fn press(&mut self, at: Cell, shift: bool) {
        if shift {
            if let Some(a) = self.anchor {
                self.sel = Self::rect(a, at);
            }
        } else {
            self.sel.clear();
            self.anchor = Some(at);
        }
        self.commit();
    }

    /// A double-click opens the in-place edit over the cell.
    pub fn open_edit(&mut self, at: Cell) {
        self.commit();
        self.anchor = Some(at);
        self.edit = Some((at, self.cells[at.0][at.1].clone()));
    }

    /// Commit the edit (`0x47b3d0`): the text goes into the edited cell and
    /// every selected cell of the same column.
    pub fn commit(&mut self) {
        let Some(((r, c), text)) = self.edit.take() else { return };
        self.cells[r][c] = text.clone();
        for &(sr, sc) in &self.sel {
            if sc == c {
                self.cells[sr][sc] = text.clone();
            }
        }
        self.dirty = true;
    }

    pub fn cancel_edit(&mut self) {
        self.edit = None;
    }

    /// OK (`0x47b610`): nothing dirty → nothing; otherwise every cell is read
    /// with `_wtof` and the changed values stored. Returns the channels whose
    /// coefficients changed.
    pub fn apply(&mut self, doc: &mut Doc) -> Vec<usize> {
        self.commit();
        if !self.dirty {
            return Vec::new();
        }
        let mut changed = Vec::new();
        for (i, [p, s]) in self.cells.iter().enumerate() {
            let Some(ch) = doc.osc.analog.get_mut(i) else { continue };
            let (p, s) = (wtof(p), wtof(s));
            if p != ch.primary || s != ch.secondary {
                ch.primary = p;
                ch.secondary = s;
                changed.push(i);
            }
        }
        changed
    }
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

const GREY: Color32 = Color32::from_rgb(150, 150, 150);
const SEL_BG: Color32 = Color32::from_rgb(100, 100, 255);

/// Header glyph of an image column (bitmap 464 is not reproduced: our own
/// marks for visible / frequency / vector / circle / digital).
fn header_glyph(p: &egui::Painter, r: Rect, col: usize, c: Color32) {
    let m = r.center();
    let s = Stroke::new(1.3f32, c);
    match col {
        1 => {
            // an eye
            p.circle_stroke(m, 3.0, s);
            p.line_segment([m + Vec2::new(-7.0, 0.0), m + Vec2::new(0.0, -4.5)], s);
            p.line_segment([m + Vec2::new(0.0, -4.5), m + Vec2::new(7.0, 0.0)], s);
            p.line_segment([m + Vec2::new(-7.0, 0.0), m + Vec2::new(0.0, 4.5)], s);
            p.line_segment([m + Vec2::new(0.0, 4.5), m + Vec2::new(7.0, 0.0)], s);
        }
        2 => {
            for (k, h) in [5.0f32, 9.0, 3.0, 6.0].into_iter().enumerate() {
                let x = m.x - 6.0 + k as f32 * 4.0;
                p.line_segment([Pos2::new(x, m.y + 6.0), Pos2::new(x, m.y + 6.0 - h)], Stroke::new(2.0f32, c));
            }
        }
        3 => {
            let a = m + Vec2::new(-6.0, 5.0);
            let b = m + Vec2::new(6.0, -5.0);
            p.line_segment([a, b], s);
            p.line_segment([b, b + Vec2::new(-5.0, 0.5)], s);
            p.line_segment([b, b + Vec2::new(-0.5, 5.0)], s);
        }
        4 => {
            p.circle_stroke(m, 6.0, s);
        }
        _ => {
            let pts = [(-7.0, 4.0), (-3.0, 4.0), (-3.0, -4.0), (3.0, -4.0), (3.0, 4.0), (7.0, 4.0)];
            let v: Vec<Pos2> = pts.iter().map(|&(x, y)| m + Vec2::new(x, y)).collect();
            p.add(egui::Shape::line(v, s));
        }
    }
}

fn cell_mark(p: &egui::Painter, r: Rect, v: Option<bool>, accent: Color32) {
    let b = Rect::from_center_size(r.center(), Vec2::splat(11.0));
    match v {
        None => {
            p.line_segment([b.left_center(), b.right_center()], Stroke::new(1.0f32, GREY));
        }
        Some(on) => {
            p.rect_stroke(b, 1.0f32, Stroke::new(1.0f32, accent));
            if on {
                p.add(egui::Shape::line(
                    vec![b.min + Vec2::new(2.0, 5.5), b.min + Vec2::new(4.5, 8.5), b.min + Vec2::new(9.0, 2.5)],
                    Stroke::new(1.8f32, accent),
                ));
            }
        }
    }
}

impl App {
    pub fn open_channels_dialog(&mut self) {
        if self.active_doc().is_some() {
            self.dlg.channels = Some(ChannelsDlg::default());
        }
    }

    pub fn open_coef_dialog(&mut self) {
        if let Some(d) = self.active_doc() {
            self.dlg.coef = Some(CoefDlg::new(d));
        }
    }

    /// DIALOG 463: the channel grid.
    pub fn channels_window(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.dlg.channels.take() else { return };
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else { return };
        let pal = self.skin.palette();
        let mut open = true;
        let mut ok = false;
        let mut alert: Option<&'static str> = None;
        egui::Window::new("Отображение каналов")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let doc = &mut self.docs[ai];
                const LIST_W: f32 = 420.0;
                let name_w = LIST_W - 185.0;
                let row_h = 20.0;
                let col_x = |c: usize| if c == 0 { 0.0 } else { name_w + 32.0 * (c - 1) as f32 };
                let col_w = |c: usize| if c == 0 { name_w } else { 32.0 };
                let keys = rows(doc);
                ui.spacing_mut().item_spacing.y = 0.0;
                // Header.
                let (hr, _) = ui.allocate_exact_size(Vec2::new(LIST_W - 25.0, row_h + 2.0), Sense::hover());
                st.origin = Some(hr.min);
                for c in 0..COLS {
                    let r = Rect::from_min_size(hr.min + Vec2::new(col_x(c), 0.0), Vec2::new(col_w(c), hr.height()));
                    let resp = ui.interact(r, ui.id().with(("fv_ch_hdr", c)), Sense::click());
                    let p = ui.painter();
                    p.rect(r, 0.0f32, pal.group_bg, Stroke::new(1.0f32, pal.group_border));
                    if c == 0 {
                        p.text(r.left_center() + Vec2::new(4.0, 0.0), egui::Align2::LEFT_CENTER, "Каналы", egui::FontId::proportional(12.0), pal.text);
                    } else {
                        header_glyph(p, r, c, pal.accent);
                    }
                    if c > 0 && resp.clicked() {
                        toggle_column(doc, c);
                    }
                }
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for key in keys {
                        let (rr, _) = ui.allocate_exact_size(Vec2::new(LIST_W - 25.0, row_h), Sense::hover());
                        for c in 0..COLS {
                            let r = Rect::from_min_size(rr.min + Vec2::new(col_x(c), 0.0), Vec2::new(col_w(c), row_h));
                            ui.painter().rect_stroke(r, 0.0f32, Stroke::new(1.0f32, pal.group_border));
                            if c == 0 {
                                if let Some((k, text)) = st.rename.as_mut().filter(|(k, _)| *k == key) {
                                    let k = *k;
                                    let te = ui.put(r.shrink(1.0), egui::TextEdit::singleline(text).id(egui::Id::new(("fv_ch_rename", k))).desired_width(r.width() - 4.0));
                                    te.request_focus();
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        st.rename = None;
                                    } else if te.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        let name = text.clone();
                                        st.rename = None;
                                        if let Err(m) = rename(doc, k, &name) {
                                            alert = Some(m);
                                        }
                                    }
                                    continue;
                                }
                                let resp = ui.interact(r, ui.id().with(("fv_ch_name", key)), Sense::click());
                                let col = if matches!(key, ChanKey::Computed(_)) { GREY } else { pal.text };
                                ui.painter().with_clip_rect(r).text(
                                    r.left_center() + Vec2::new(4.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    row_name(doc, key),
                                    egui::FontId::proportional(12.0),
                                    col,
                                );
                                if resp.clicked() {
                                    match can_rename(key) {
                                        Ok(()) => st.rename = Some((key, row_name(doc, key))),
                                        Err(m) => alert = Some(m),
                                    }
                                }
                            } else {
                                let resp = ui.interact(r, ui.id().with(("fv_ch_cell", key, c)), Sense::click());
                                cell_mark(ui.painter(), r, cell(doc, key, c), pal.accent);
                                if resp.clicked() {
                                    toggle_cell(doc, key, c);
                                }
                            }
                        }
                    }
                });
                ui.add_space(4.0);
                ui.vertical_centered(|ui| {
                    if ui.button("    OK    ").clicked() {
                        ok = true;
                    }
                });
            });
        if let Some(m) = alert {
            self.dlg.push_alert(m.to_string());
        }
        // OK commits a running rename.
        if ok {
            if let Some((k, text)) = st.rename.take() {
                if let Err(m) = rename(&mut self.docs[ai], k, &text) {
                    self.dlg.push_alert(m.to_string());
                    self.dlg.channels = Some(st);
                    return;
                }
            }
        }
        if open && !ok {
            self.dlg.channels = Some(st);
        }
    }

    /// DIALOG 477: the coefficient grid.
    pub fn coef_window(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.dlg.coef.take() else { return };
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else { return };
        let pal = self.skin.palette();
        let mut open = true;
        let mut result: Option<bool> = None;
        egui::Window::new("Коэффициенты трансформации")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let shift = ui.input(|i| i.modifiers.shift);
                const W: f32 = 420.0;
                let cw = W * 0.33;
                let name_w = W - 2.0 * cw;
                let row_h = 20.0;
                let (hr, _) = ui.allocate_exact_size(Vec2::new(W, row_h + 2.0), Sense::hover());
                st.origin = Some(hr.min);
                for (t, x, w) in [("Канал", 0.0, name_w), ("Первичное значение", name_w, cw), ("Вторичное значение", name_w + cw, cw)] {
                    let r = Rect::from_min_size(hr.min + Vec2::new(x, 0.0), Vec2::new(w, hr.height()));
                    let p = ui.painter();
                    p.rect(r, 0.0f32, pal.group_bg, Stroke::new(1.0f32, pal.group_border));
                    p.with_clip_rect(r).text(r.left_center() + Vec2::new(4.0, 0.0), egui::Align2::LEFT_CENTER, t, egui::FontId::proportional(12.0), pal.text);
                }
                ui.spacing_mut().item_spacing.y = 0.0;
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for row in 0..st.cells.len() {
                        let (rr, _) = ui.allocate_exact_size(Vec2::new(W, row_h), Sense::hover());
                        let nr = Rect::from_min_size(rr.min, Vec2::new(name_w, row_h));
                        ui.painter().rect_stroke(nr, 0.0f32, Stroke::new(1.0f32, pal.group_border));
                        ui.painter().with_clip_rect(nr).text(nr.left_center() + Vec2::new(4.0, 0.0), egui::Align2::LEFT_CENTER, &st.names[row], egui::FontId::proportional(12.0), pal.text);
                        for col in 0..2 {
                            let r = Rect::from_min_size(rr.min + Vec2::new(name_w + cw * col as f32, 0.0), Vec2::new(cw, row_h));
                            if let Some((_, text)) = st.edit.as_mut().filter(|(at, _)| *at == (row, col)) {
                                let te = ui.put(r.shrink(1.0), egui::TextEdit::singleline(text).id(egui::Id::new(("fv_coef_edit", row, col))).desired_width(r.width() - 4.0));
                                te.request_focus();
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    st.cancel_edit();
                                } else if te.lost_focus() {
                                    // A press elsewhere: the pressed cell decides
                                    // the Shift range, then the text is committed.
                                    let target = ui.input(|i| i.pointer.press_origin()).and_then(|p| coef_cell_at(rr, p, name_w, cw, row_h, row, st.cells.len()));
                                    match target {
                                        Some(t) => st.press(t, shift),
                                        None => st.commit(),
                                    }
                                }
                                continue;
                            }
                            let selected = st.sel.contains(&(row, col));
                            let resp = ui.interact(r, ui.id().with(("fv_coef", row, col)), Sense::click());
                            let p = ui.painter();
                            if selected {
                                p.rect_filled(r, 0.0f32, SEL_BG);
                            }
                            p.rect_stroke(r, 0.0f32, Stroke::new(1.0f32, pal.group_border));
                            p.with_clip_rect(r).text(
                                r.left_center() + Vec2::new(4.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                &st.cells[row][col],
                                egui::FontId::proportional(12.0),
                                if selected { Color32::WHITE } else { pal.text },
                            );
                            if resp.double_clicked() {
                                st.open_edit((row, col));
                            } else if resp.clicked() {
                                st.press((row, col), shift);
                            }
                        }
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        result = Some(true);
                    }
                    if ui.button("Отмена").clicked() {
                        result = Some(false);
                    }
                });
            });
        match result {
            Some(true) => {
                let changed = st.apply(&mut self.docs[ai]);
                if !changed.is_empty() {
                    self.recompute_doc(ai);
                }
            }
            Some(false) => {}
            None if open => self.dlg.coef = Some(st),
            None => {}
        }
    }
}

/// The value cell under `p` in the coefficient grid, given one row's rect
/// (rows are laid out one under the other from that rect's row index).
fn coef_cell_at(rr: Rect, p: Pos2, name_w: f32, cw: f32, row_h: f32, row: usize, n: usize) -> Option<Cell> {
    let top = rr.top() - row as f32 * row_h;
    let r = ((p.y - top) / row_h).floor();
    let x = p.x - rr.left() - name_w;
    if r < 0.0 || r as usize >= n || x < 0.0 || x >= 2.0 * cw {
        return None;
    }
    Some((r as usize, (x / cw) as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_doc() -> Doc {
        let osc = fvb_core::comtrade::parse(crate::app::DEMO_CFG, crate::app::DEMO_DAT).unwrap();
        Doc::new(osc, "demo".into())
    }

    #[test]
    fn eligibility_by_unit_as_the_original_reads_it() {
        assert!(vector_unit_ok("A") && vector_unit_ok("V") && vector_unit_ok("ном") && vector_unit_ok(""));
        assert!(!vector_unit_ok("kV") && !vector_unit_ok("Ohm"));
        assert!(circle_unit_ok("Ohm") && circle_unit_ok("va") && circle_unit_ok("W") && circle_unit_ok("var"));
        assert!(!circle_unit_ok("A") && !circle_unit_ok(""));
        let mut d = demo_doc();
        // Ua is in V: vector yes, circle no; the discrete channel has only column 1.
        assert_eq!(cell(&d, ChanKey::Analog(0), 3), Some(true));
        assert_eq!(cell(&d, ChanKey::Analog(0), 4), None);
        assert_eq!(cell(&d, ChanKey::Digital(0), 2), None);
        assert!(cell(&d, ChanKey::Digital(0), 1).is_some());
        // ADC mode empties the unit: still vector-eligible.
        d.chans[0].adc_counts = true;
        assert!(cell(&d, ChanKey::Analog(0), 3).is_some());
    }

    #[test]
    fn header_click_sets_all_unless_all_are_set() {
        let mut d = demo_doc();
        d.chans[1].on_freq = false;
        toggle_column(&mut d, 2);
        assert!(d.chans.iter().all(|c| c.on_freq));
        toggle_column(&mut d, 2);
        assert!(d.chans.iter().all(|c| !c.on_freq));
    }

    #[test]
    fn hiding_dissolves_the_group_and_deselects() {
        let mut d = demo_doc();
        d.chans[0].group = Some(7);
        d.chans[1].group = Some(7);
        d.selection.insert(ChanKey::Analog(0));
        toggle_cell(&mut d, ChanKey::Analog(0), 1);
        assert!(!d.chans[0].visible);
        assert_eq!((d.chans[0].group, d.chans[1].group), (None, None));
        assert!(d.selection.is_empty());
        toggle_cell(&mut d, ChanKey::Digital(0), 1);
        assert!(!d.digital_visible[0]);
    }

    #[test]
    fn rename_rules_397_398() {
        let mut d = demo_doc();
        d.computed.push(crate::app::Computed::new("X".into(), "Ua".into(), vec![], Color32::RED));
        assert_eq!(rename(&mut d, ChanKey::Computed(0), "Y"), Err(MSG_NO_RENAME_COMPUTED));
        assert_eq!(rename(&mut d, ChanKey::Analog(0), "Ub"), Err(MSG_NAME_TAKEN));
        assert_eq!(rename(&mut d, ChanKey::Analog(0), "X"), Err(MSG_NAME_TAKEN));
        assert_eq!(rename(&mut d, ChanKey::Analog(0), "Ua1"), Ok(()));
        assert_eq!(d.osc.analog[0].id, "Ua1");
        assert_eq!(rename(&mut d, ChanKey::Digital(0), "Пуск"), Ok(()));
        assert_eq!(d.osc.digital[0].id, "Пуск");
    }

    #[test]
    fn wtof_takes_the_leading_number() {
        assert_eq!(wtof(" 12.5kV"), 12.5);
        assert_eq!(wtof("abc"), 0.0);
        assert_eq!(wtof("1,5"), 1.0);
        assert_eq!(wtof("-2e3x"), -2000.0);
        assert_eq!(wtof("3e"), 3.0);
        assert_eq!(wtof(".5"), 0.5);
    }

    #[test]
    fn coefficient_bulk_entry_fills_the_shift_range_of_one_column() {
        let mut d = demo_doc();
        let mut g = CoefDlg::new(&d);
        assert_eq!(g.cells[0], ["1".to_string(), "1".to_string()]);
        // Double-click row 0 primary, type, Shift+press row 2 secondary.
        g.press((0, 0), false);
        g.open_edit((0, 0));
        g.edit.as_mut().unwrap().1 = "110000".into();
        g.press((2, 1), true);
        assert_eq!(g.sel.len(), 6);
        for r in 0..3 {
            assert_eq!(g.cells[r][0], "110000");
            assert_eq!(g.cells[r][1], "1");
        }
        assert_eq!(g.cells[3][0], "1");
        // Without Shift only the edited cell, and the selection clears.
        g.open_edit((4, 1));
        g.edit.as_mut().unwrap().1 = "bad".into();
        g.press((5, 0), false);
        assert!(g.sel.is_empty());
        assert_eq!(g.cells[4][1], "bad");
        let changed = g.apply(&mut d);
        assert_eq!(changed, vec![0, 1, 2, 4]);
        assert_eq!(d.osc.analog[1].primary, 110000.0);
        // No validation: bad text is 0.
        assert_eq!(d.osc.analog[4].secondary, 0.0);
    }

    #[test]
    fn coefficients_ok_without_edits_changes_nothing() {
        let mut d = demo_doc();
        let mut g = CoefDlg::new(&d);
        g.press((1, 1), false);
        assert!(g.apply(&mut d).is_empty());
    }
}

/// Headless egui tests of the two dialogs (synthetic pointer and key events).
#[cfg(test)]
mod ui_tests {
    use super::*;
    use crate::app::Command;

    struct Harness {
        ctx: egui::Context,
        app: App,
        t: f64,
    }

    impl Harness {
        fn new() -> Harness {
            let mut app = App::new();
            app.dispatch(Command::Demo);
            Harness { ctx: egui::Context::default(), app, t: 0.0 }
        }

        fn frame_mod(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
            self.t += 0.05;
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                time: Some(self.t),
                events,
                modifiers,
                ..Default::default()
            };
            let app = &mut self.app;
            let _ = self.ctx.run(raw, |ctx| {
                app.channels_window(ctx);
                app.coef_window(ctx);
            });
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            self.frame_mod(events, Default::default());
        }

        fn click_mod(&mut self, pos: Pos2, modifiers: egui::Modifiers) {
            let ev = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers };
            self.frame_mod(vec![egui::Event::PointerMoved(pos), ev(true)], modifiers);
            self.frame_mod(vec![ev(false)], modifiers);
        }

        /// Where the «Каналы» grid is now (the window settles its size over
        /// the first frames).
        fn ch_origin(&self) -> Pos2 {
            self.app.dlg.channels.as_ref().and_then(|s| s.origin).expect("dialog drawn")
        }

        /// A click, then idle frames past the double-click delay (0.3 s).
        fn click(&mut self, pos: Pos2) {
            self.click_mod(pos, Default::default());
            for _ in 0..7 {
                self.frame(vec![]);
            }
        }

        fn double_click(&mut self, pos: Pos2) {
            self.click_mod(pos, Default::default());
            self.click_mod(pos, Default::default());
            self.frame(vec![]);
        }
    }

    /// Centre of cell (row, col) of the «Каналы» grid; row None = header.
    fn ch_cell(o: Pos2, row: Option<usize>, col: usize) -> Pos2 {
        let name_w = 420.0 - 185.0;
        let x = if col == 0 { 40.0 } else { name_w + 32.0 * (col - 1) as f32 + 16.0 };
        let y = match row {
            None => 11.0,
            Some(r) => 22.0 + 20.0 * r as f32 + 10.0,
        };
        o + Vec2::new(x, y)
    }

    #[test]
    fn channels_grid_header_cell_and_rename_clicks() {
        let mut h = Harness::new();
        h.app.dispatch(Command::ChannelsDialog);
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(h.app.dlg.channels.as_ref().and_then(|s| s.origin).is_some(), "dialog drawn");
        // Header «frequency» with one channel off → all on.
        h.app.docs[0].chans[2].on_freq = false;
        h.click(ch_cell(h.ch_origin(), None, 2));
        assert!(h.app.docs[0].chans.iter().all(|c| c.on_freq));
        // Cell (row 0, visible) hides Ua.
        h.click(ch_cell(h.ch_origin(), Some(0), 1));
        assert!(!h.app.docs[0].chans[0].visible);
        // Name cell → in-place edit; a name Ub already has, Enter → 397.
        h.click(ch_cell(h.ch_origin(), Some(0), 0));
        assert!(h.app.dlg.channels.as_ref().unwrap().rename.is_some());
        h.app.dlg.channels.as_mut().unwrap().rename.as_mut().unwrap().1 = "Ub".into();
        let enter = egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        h.frame(vec![enter]);
        h.frame(vec![]);
        assert_eq!(h.app.dlg.alert(), Some(MSG_NAME_TAKEN));
        assert_eq!(h.app.docs[0].osc.analog[0].id, "Ua");
    }

    #[test]
    fn coefficients_double_click_type_and_shift_click_fill() {
        let mut h = Harness::new();
        h.app.dispatch(Command::Coefficients);
        h.frame(vec![]);
        for _ in 0..6 {
            h.frame(vec![]);
        }
        let o = h.app.dlg.coef.as_ref().and_then(|s| s.origin).expect("dialog drawn");
        let cw = 420.0 * 0.33;
        let name_w = 420.0 - 2.0 * cw;
        let cell = |r: usize, c: usize| o + Vec2::new(name_w + cw * c as f32 + cw / 2.0, 22.0 + 20.0 * r as f32 + 10.0);
        h.double_click(cell(0, 0));
        assert_eq!(h.app.dlg.coef.as_ref().unwrap().edit.as_ref().map(|e| e.0), Some((0, 0)));
        // Replace the text, then Shift+click row 3 of the same column.
        h.app.dlg.coef.as_mut().unwrap().edit.as_mut().unwrap().1 = "6000".into();
        h.click_mod(cell(3, 0), egui::Modifiers::SHIFT);
        h.frame(vec![]);
        let st = h.app.dlg.coef.as_ref().unwrap();
        assert!(st.edit.is_none());
        assert_eq!((0..4).map(|r| st.cells[r][0].as_str()).collect::<Vec<_>>(), ["6000"; 4]);
        assert_eq!(st.cells[4][0], "1");
        assert!(st.cells.iter().all(|c| c[1] == "1"));
    }
}
