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

/// The channel rows: the analog channels (file channels, then the computed
/// ones, which the original keeps in the same vector with bit 13), then the
/// discrete channels (kind 1); the pair rows go between them ([`dlg_rows`]).
pub fn rows(doc: &Doc) -> Vec<ChanKey> {
    // The record's vector (L9.188): the analog block — a pasted analog
    // channel at its insertion point (before a file channel, or at the
    // block's end, `doc+0x1d8`, with the formula channels in creation
    // order) —, then the digital block with a pasted digital channel at
    // its point (before a digital channel, or the end).
    let comp = |want_digital: bool, at: Option<ChanKey>| {
        (0..doc.computed.len()).filter(move |&j| doc.pending_digital(j) == want_digital && doc.computed[j].before == at).map(ChanKey::Computed)
    };
    let nd = doc.osc.digital.len();
    let mut v: Vec<ChanKey> = Vec::new();
    for i in 0..doc.osc.analog.len() {
        v.extend(comp(false, Some(ChanKey::Analog(i))));
        v.push(ChanKey::Analog(i));
    }
    v.extend((0..doc.computed.len()).filter(|&j| !doc.pending_digital(j) && !matches!(doc.computed[j].before, Some(ChanKey::Analog(i)) if i < doc.osc.analog.len())).map(ChanKey::Computed));
    for n in 0..nd {
        v.extend(comp(true, Some(ChanKey::Digital(n))));
        v.push(ChanKey::Digital(n));
    }
    v.extend((0..doc.computed.len()).filter(|&j| doc.pending_digital(j) && !matches!(doc.computed[j].before, Some(ChanKey::Digital(n)) if n < nd)).map(ChanKey::Computed));
    v
}

/// Is a row a discrete channel's (a pasted digital one included)?
pub fn digital_row(doc: &Doc, k: ChanKey) -> bool {
    match k {
        ChanKey::Digital(_) => true,
        ChanKey::Computed(j) => doc.pending_digital(j),
        ChanKey::Analog(_) => false,
    }
}

/// A pair row (`0x433b00`–`0x433d6a`): a differential pair (`doc+0x878`)
/// or an R/X pair (`doc+0x884`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PairRow {
    Dif(usize),
    Rx(usize),
}

/// A row of the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DlgRow {
    Chan(ChanKey),
    Pair(PairRow),
}

/// Every row in the original's order: the analog channels, one row per
/// differential pair, one per R/X pair, then the discrete channels.
pub fn dlg_rows(doc: &Doc) -> Vec<DlgRow> {
    let (dig, chan): (Vec<ChanKey>, Vec<ChanKey>) = rows(doc).into_iter().partition(|&k| digital_row(doc, k));
    chan.into_iter()
        .map(DlgRow::Chan)
        .chain((0..doc.dif_pairs.len()).map(|i| DlgRow::Pair(PairRow::Dif(i))))
        .chain((0..doc.rx_pairs.len()).map(|i| DlgRow::Pair(PairRow::Rx(i))))
        .chain(dig.into_iter().map(DlgRow::Chan))
        .collect()
}

/// A pair row's text: the stored first name « | » the second (" | ",
/// `0x4c65d0`) — for a differential pair Iторм | Iдиф (`doc+0x878`'s order;
/// ours stores (Iдиф, Iторм), L9.81).
pub fn pair_name(doc: &Doc, p: PairRow) -> String {
    match p {
        PairRow::Dif(i) => doc.dif_pairs.get(i).map(|q| format!("{} | {}", q.b, q.a)),
        PairRow::Rx(i) => doc.rx_pairs.get(i).map(|q| format!("{} | {}", q.a, q.b)),
    }
    .unwrap_or_default()
}

/// A pair row's cell (`0x4349c0`): column 5 on a differential pair, column
/// 4 on an R/X pair — its enabled flag; nothing elsewhere.
pub fn pair_cell(doc: &Doc, p: PairRow, col: usize) -> Option<bool> {
    match (p, col) {
        (PairRow::Dif(i), 5) => doc.dif_pairs.get(i).map(|q| q.on),
        (PairRow::Rx(i), 4) => doc.rx_pairs.get(i).map(|q| q.on),
        _ => None,
    }
}

fn set_pair_cell(doc: &mut Doc, p: PairRow, col: usize, v: bool) {
    if pair_cell(doc, p, col).is_none() {
        return;
    }
    doc.set.modified = true;
    match p {
        PairRow::Dif(i) => doc.dif_pairs[i].on = v,
        PairRow::Rx(i) => doc.rx_pairs[i].on = v,
    }
}

/// A click on a pair's cell flips it (then the diagrams are rebound,
/// `0x44c090`, and the document marked modified).
pub fn toggle_pair_cell(doc: &mut Doc, p: PairRow, col: usize) {
    if let Some(v) = pair_cell(doc, p, col) {
        set_pair_cell(doc, p, col, !v);
    }
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

/// The circle window's refresh (`0x436d77`–`0x436e64`, every refresh-all
/// `0x475420`): a channel on the circle diagram whose canonical unit is
/// neither "Ohm" (string 3060) nor "VA" (CompareNoCase) leaves it — so a W
/// or var channel ticked in column 4 keeps the tick only until then.
pub fn circle_refresh_flags(doc: &mut Doc) {
    for key in rows(doc) {
        let ok = unit_of(doc, key).is_some_and(|u| ["Ohm", "VA"].iter().any(|x| u.eq_ignore_ascii_case(x)));
        match key {
            ChanKey::Analog(i) if !ok => doc.chans[i].on_circle = false,
            ChanKey::Computed(j) if !ok => doc.computed[j].on_circle = false,
            _ => {}
        }
    }
    // The vector window's refresh (`0x48fda0`, the same refresh-all) clears
    // the vector flag of every channel failing the vector test (`0x4900cb`).
    for key in rows(doc) {
        let ok = unit_of(doc, key).is_some_and(|u| vector_unit_ok(&u));
        match key {
            ChanKey::Analog(i) if !ok => doc.chans[i].on_vector = false,
            ChanKey::Computed(j) if !ok => doc.computed[j].on_vector = false,
            _ => {}
        }
    }
}

/// The flag a cell shows, or `None` where it does not apply (drawn grey).
pub fn cell(doc: &Doc, key: ChanKey, col: usize) -> Option<bool> {
    let unit = unit_of(doc, key);
    match (key, col) {
        (ChanKey::Analog(i), 1) => Some(doc.chans[i].visible),
        (ChanKey::Computed(j), 1) => Some(doc.computed[j].visible),
        (ChanKey::Digital(k), 1) => Some(doc.digital_visible.get(k).copied().unwrap_or(true)),
        // Column 2: every analog-kind record not pending a merge.
        (ChanKey::Analog(i), 2) => Some(doc.chans[i].on_freq),
        (ChanKey::Computed(j), 2) if !pending(doc, j) => Some(doc.computed[j].on_freq),
        (ChanKey::Analog(i), 3) if vector_unit_ok(unit.as_deref().unwrap_or("")) => Some(doc.chans[i].on_vector),
        (ChanKey::Computed(j), 3) if vector_unit_ok(unit.as_deref().unwrap_or("")) => Some(doc.computed[j].on_vector),
        (ChanKey::Analog(i), 4) if circle_unit_ok(unit.as_deref().unwrap_or("")) => Some(doc.chans[i].on_circle),
        // Not for a channel pending a merge (bit 14; click `0x434e81`, paint).
        (ChanKey::Computed(j), 4) if circle_unit_ok(unit.as_deref().unwrap_or("")) && !pending(doc, j) => Some(doc.computed[j].on_circle),
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
        // Hiding takes the whole group: every member hidden and deselected,
        // the group dissolved (`0x434b1c`–`0x434c71`, `0x455df0`).
        (ChanKey::Analog(_) | ChanKey::Computed(_), 1) if !v => {
            let members = doc.with_groups(&[key]);
            doc.dissolve_groups(&members);
            for m in members {
                match m {
                    ChanKey::Analog(i) => doc.chans[i].visible = false,
                    ChanKey::Computed(j) => doc.computed[j].visible = false,
                    ChanKey::Digital(_) => {}
                }
                doc.selection.remove(&m);
            }
        }
        (ChanKey::Analog(i), 1) => doc.chans[i].visible = true,
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
        (ChanKey::Computed(j), 2) => doc.computed[j].on_freq = v,
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

fn pending(doc: &Doc, j: usize) -> bool {
    doc.computed[j].formula == crate::merge_ui::PENDING
}

/// A header click (HDN_ITEMCLICK `0x435080`): the flag is set everywhere it
/// can be when fewer rows have it than the column counts as applicable,
/// else cleared — with the original's counts, which are not the cells':
/// column 2 counts every analog row (computed and pending ones included)
/// and writes the non-pending ones, column 3 counts the flag on any row,
/// column 4 counts the eligible rows pending or not (and the R/X pairs) but
/// writes the non-pending ones; column 5 is the differential pairs alone.
/// No group is dissolved; the document is marked and the diagrams
/// refreshed even when nothing applies.
pub fn toggle_column(doc: &mut Doc, col: usize) {
    let keys = rows(doc);
    let analog: Vec<ChanKey> = keys.iter().copied().filter(|&k| !digital_row(doc, k)).collect();
    let unit_ok = |doc: &Doc, k: ChanKey, f: fn(&str) -> bool| unit_of(doc, k).is_some_and(|u| f(&u));
    match col {
        1 => {
            let set = keys.iter().filter(|&&k| cell(doc, k, 1) == Some(true)).count();
            let v = set < keys.len();
            for &k in &keys {
                match k {
                    ChanKey::Analog(i) => doc.chans[i].visible = v,
                    ChanKey::Computed(j) => doc.computed[j].visible = v,
                    ChanKey::Digital(d) => {
                        if let Some(f) = doc.digital_visible.get_mut(d) {
                            *f = v;
                        }
                    }
                }
                if !v {
                    doc.selection.remove(&k);
                }
            }
        }
        2 => {
            // Set: bit 1 and not bit 14; applicable: every analog-kind row;
            // written: the non-pending ones (`0x435080`).
            let set = doc.chans.iter().filter(|c| c.on_freq).count() + (0..doc.computed.len()).filter(|&j| doc.computed[j].on_freq && !pending(doc, j)).count();
            let v = set < analog.len();
            for c in &mut doc.chans {
                c.on_freq = v;
            }
            for j in 0..doc.computed.len() {
                if !pending(doc, j) {
                    doc.computed[j].on_freq = v;
                }
            }
        }
        3 => {
            let eligible: Vec<ChanKey> = analog.iter().copied().filter(|&k| unit_ok(doc, k, vector_unit_ok)).collect();
            let set = doc.chans.iter().filter(|c| c.on_vector).count() + doc.computed.iter().filter(|c| c.on_vector).count();
            let v = set < eligible.len();
            for k in eligible {
                set_flag(doc, k, 3, v);
            }
        }
        4 => {
            let eligible: Vec<ChanKey> = analog.iter().copied().filter(|&k| unit_ok(doc, k, circle_unit_ok)).collect();
            let set = eligible.iter().filter(|&&k| match k {
                ChanKey::Computed(j) => doc.computed[j].on_circle && !pending(doc, j),
                ChanKey::Analog(i) => doc.chans[i].on_circle,
                ChanKey::Digital(_) => false,
            }).count() + doc.rx_pairs.iter().filter(|p| p.on).count();
            let v = set < eligible.len() + doc.rx_pairs.len();
            for k in eligible {
                if !matches!(k, ChanKey::Computed(j) if pending(doc, j)) {
                    set_flag(doc, k, 4, v);
                }
            }
            for p in &mut doc.rx_pairs {
                p.on = v;
            }
        }
        5 => {
            let v = doc.dif_pairs.iter().filter(|p| p.on).count() < doc.dif_pairs.len();
            for p in &mut doc.dif_pairs {
                p.on = v;
            }
        }
        _ => {}
    }
    doc.set.modified = true;
    circle_refresh_flags(doc);
}

fn set_flag(doc: &mut Doc, k: ChanKey, col: usize, v: bool) {
    match (k, col) {
        (ChanKey::Analog(i), 3) => doc.chans[i].on_vector = v,
        (ChanKey::Computed(j), 3) => doc.computed[j].on_vector = v,
        (ChanKey::Analog(i), 4) => doc.chans[i].on_circle = v,
        (ChanKey::Computed(j), 4) => doc.computed[j].on_circle = v,
        _ => {}
    }
}

/// Can the name cell be edited? Computed channels (bit 13) answer 398 —
/// the ОМП's flag channels too (`0x435ac0`).
pub fn can_rename(doc: &Doc, key: ChanKey) -> Result<(), &'static str> {
    match key {
        ChanKey::Computed(_) => Err(MSG_NO_RENAME_COMPUTED),
        ChanKey::Digital(i) if doc.dig_computed(i) => Err(MSG_NO_RENAME_COMPUTED),
        _ => Ok(()),
    }
}

/// Commit an in-place rename (`0x435c80`): unchanged text does nothing; a
/// name another channel already has is refused with 397.
pub fn rename(doc: &mut Doc, key: ChanKey, name: &str) -> Result<(), &'static str> {
    can_rename(doc, key)?;
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
    /// Where each value cell was drawn at the last frame (for tests).
    pub cell_rects: Vec<(Cell, Rect)>,
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

    /// A press anywhere but the edit (`0x47af90` → `0x47b3d0`): only the
    /// commit — a single click selects nothing.
    pub fn press(&mut self, shift: bool) {
        self.commit(shift);
    }

    /// A double-click on a cell (`0x47b020`): the edit committed; with Shift
    /// every row between the anchor's and this one takes the rectangle's
    /// columns (those rows overwritten, the others kept); the cell joins the
    /// selection and its edit opens.
    pub fn double_click(&mut self, at: Cell, shift: bool) {
        self.commit(shift);
        if shift {
            if let Some(a) = self.anchor {
                let (r0, r1) = (a.0.min(at.0), a.0.max(at.0));
                let (c0, c1) = (a.1.min(at.1), a.1.max(at.1));
                self.sel.retain(|&(r, _)| !(r0..=r1).contains(&r));
                self.sel.extend((r0..=r1).flat_map(|r| (c0..=c1).map(move |c| (r, c))));
            }
        }
        self.sel.insert(at);
        self.edit = Some((at, self.cells[at.0][at.1].clone()));
    }

    /// Commit the edit (`0x47b3d0`): a text that differs from the cell's
    /// makes the grid dirty and goes into the edited cell and every selected
    /// cell of either column (the rows' item-data bits); then the edited
    /// cell is the anchor and, without Shift, the selection clears — after
    /// the write.
    pub fn commit(&mut self, shift: bool) {
        let Some(((r, c), text)) = self.edit.take() else { return };
        if text != self.cells[r][c] {
            self.cells[r][c] = text.clone();
            for &(sr, sc) in &self.sel {
                self.cells[sr][sc] = text.clone();
            }
            self.dirty = true;
        }
        self.anchor = Some((r, c));
        if !shift {
            self.sel.clear();
        }
    }

    pub fn cancel_edit(&mut self) {
        self.edit = None;
    }

    /// OK (`0x47b610`): nothing dirty → OnCancel (`None`); otherwise every
    /// cell is read with `_wtof` and the changed values stored — the caller
    /// marks the document modified on IDOK (`0x42fea7`). Returns the
    /// channels whose coefficients changed.
    pub fn apply(&mut self, doc: &mut Doc) -> Option<Vec<usize>> {
        self.commit(false);
        if !self.dirty {
            return None;
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
        Some(changed)
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
        // Column 1 is bitmap 464's image 0: a waveform (the oscillogram).
        1 => sine_glyph(p, m, c),
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
            // Column 5 is the differential pairs': a differential
            // characteristic (bitmap 464's image 4) — axes, a flat start
            // and a rising slope.
            p.line_segment([m + Vec2::new(-7.0, 6.0), m + Vec2::new(7.0, 6.0)], s);
            p.line_segment([m + Vec2::new(-7.0, 6.0), m + Vec2::new(-7.0, -6.0)], s);
            let pts = [(-7.0, 2.0), (-2.0, 2.0), (7.0, -6.0)];
            p.add(egui::Shape::line(pts.iter().map(|&(x, y)| m + Vec2::new(x, y)).collect(), s));
        }
    }
}

/// A small sine wave, 14 px wide (our drawing of a waveform image).
fn sine_glyph(p: &egui::Painter, m: Pos2, c: Color32) {
    let pts: Vec<Pos2> = (0..=14).map(|k| m + Vec2::new(k as f32 - 7.0, -4.5 * (k as f32 / 14.0 * std::f32::consts::TAU).sin())).collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.3f32, c)));
}

/// The row's image at x + 5 (`0x433db0`, bitmap 464): a channel 8 (a
/// discrete one 9), a differential pair 4, an R/X pair 3 — our drawings.
fn row_icon(p: &egui::Painter, r: Rect, image: u8, c: Color32) {
    let at = Rect::from_min_size(Pos2::new(r.left() + 5.0, r.center().y - 8.0), Vec2::splat(16.0));
    match image {
        8 => sine_glyph(p, at.center(), c),
        9 => {
            let m = at.center();
            let pts = [(-7.0, 4.0), (-3.0, 4.0), (-3.0, -4.0), (3.0, -4.0), (3.0, 4.0), (7.0, 4.0)];
            p.add(egui::Shape::line(pts.iter().map(|&(x, y)| m + Vec2::new(x, y)).collect(), Stroke::new(1.3f32, c)));
        }
        3 => header_glyph(p, at, 4, c),
        _ => header_glyph(p, at, 5, c),
    }
}

fn cell_mark(p: &egui::Painter, r: Rect, v: Option<bool>, accent: Color32) {
    let b = Rect::from_center_size(r.center(), Vec2::splat(11.0));
    match v {
        // Not applicable: a greyed-out box (bitmap 464's image 7).
        None => {
            p.rect(b, 1.0f32, Color32::from_gray(236), Stroke::new(1.0f32, Color32::from_gray(190)));
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
        // The template holds only the list: no OK button. Enter is IDOK, whose
        // handler (`0x436030`) only commits a running rename; Esc is MFC's
        // OnCancel and closes the dialog, a running rename dropped.
        let esc = self.dialog_keys(ctx) == Some(false);
        let mut open = true;
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
                    for row in dlg_rows(doc) {
                        let key = match row {
                            DlgRow::Chan(k) => k,
                            DlgRow::Pair(pr) => {
                                let (rr, _) = ui.allocate_exact_size(Vec2::new(LIST_W - 25.0, row_h), Sense::hover());
                                for c in 0..COLS {
                                    let r = Rect::from_min_size(rr.min + Vec2::new(col_x(c), 0.0), Vec2::new(col_w(c), row_h));
                                    ui.painter().rect_stroke(r, 0.0f32, Stroke::new(1.0f32, pal.group_border));
                                    if c == 0 {
                                        let p = ui.painter().with_clip_rect(r);
                                        row_icon(&p, r, if matches!(pr, PairRow::Dif(_)) { 4 } else { 3 }, pal.accent);
                                        p.text(r.left_center() + Vec2::new(26.0, 0.0), egui::Align2::LEFT_CENTER, pair_name(doc, pr), egui::FontId::proportional(12.0), pal.text);
                                    } else {
                                        let resp = ui.interact(r, ui.id().with(("fv_ch_pair", pr, c)), Sense::click());
                                        cell_mark(ui.painter(), r, pair_cell(doc, pr, c), pal.accent);
                                        if resp.clicked() {
                                            toggle_pair_cell(doc, pr, c);
                                            // Every click ends in the refresh-all
                                            // (`0x43502b` → `0x475420`).
                                            circle_refresh_flags(doc);
                                        }
                                    }
                                }
                                continue;
                            }
                        };
                        let (rr, _) = ui.allocate_exact_size(Vec2::new(LIST_W - 25.0, row_h), Sense::hover());
                        for c in 0..COLS {
                            let r = Rect::from_min_size(rr.min + Vec2::new(col_x(c), 0.0), Vec2::new(col_w(c), row_h));
                            ui.painter().rect_stroke(r, 0.0f32, Stroke::new(1.0f32, pal.group_border));
                            if c == 0 {
                                if let Some((k, text)) = st.rename.as_mut().filter(|(k, _)| *k == key) {
                                    let k = *k;
                                    // A borderless edit over the cell from 23 px in, 1 px
                                    // inside top and bottom, to column 0's right edge
                                    // (`0x435ac0`).
                                    let er = Rect::from_min_max(r.min + Vec2::new(23.0, 1.0), Pos2::new(r.right(), r.bottom() - 1.0));
                                    ui.painter().rect_filled(er, 0.0, Color32::WHITE);
                                    let te = ui.put(er, egui::TextEdit::singleline(text).id(egui::Id::new(("fv_ch_rename", k))).frame(false).margin(egui::Margin::symmetric(1.0, 0.0)).desired_width(er.width() - 2.0));
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
                                // Every name in black; a renamed channel's original
                                // name follows in (150, 150, 150) (`0x4342e3`–`0x434416`).
                                let p = ui.painter().with_clip_rect(r);
                                row_icon(&p, r, if digital_row(doc, key) { 9 } else { 8 }, pal.accent);
                                let at = r.left_center() + Vec2::new(26.0, 0.0);
                                let name = p.text(at, egui::Align2::LEFT_CENTER, row_name(doc, key), egui::FontId::proportional(12.0), pal.text);
                                if let Some(orig) = doc.original_name(key) {
                                    p.text(Pos2::new(name.right() + 5.0, at.y), egui::Align2::LEFT_CENTER, format!("({orig})"), egui::FontId::proportional(12.0), GREY);
                                }
                                if resp.clicked() {
                                    match can_rename(doc, key) {
                                        Ok(()) => st.rename = Some((key, row_name(doc, key))),
                                        Err(m) => alert = Some(m),
                                    }
                                }
                            } else {
                                let resp = ui.interact(r, ui.id().with(("fv_ch_cell", key, c)), Sense::click());
                                cell_mark(ui.painter(), r, cell(doc, key, c), pal.accent);
                                if resp.clicked() {
                                    toggle_cell(doc, key, c);
                                    circle_refresh_flags(doc);
                                }
                            }
                        }
                    }
                });
            });
        if let Some(m) = alert {
            self.dlg.push_alert(m.to_string());
        }
        if open && !esc {
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
                st.cell_rects.clear();
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
                            st.cell_rects.push(((row, col), r));
                            if let Some((_, text)) = st.edit.as_mut().filter(|(at, _)| *at == (row, col)) {
                                let te = ui.put(r.shrink(1.0), egui::TextEdit::singleline(text).id(egui::Id::new(("fv_coef_edit", row, col))).desired_width(r.width() - 4.0));
                                te.request_focus();
                                // The edit is a plain CEdit: Enter reaches IDOK
                                // (commit, apply, close), Esc IDCANCEL.
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    st.cancel_edit();
                                    result = Some(false);
                                } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    result = Some(true);
                                } else if te.lost_focus() {
                                    // A press elsewhere commits (`0x47b3d0`).
                                    st.press(shift);
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
                                st.double_click((row, col), shift);
                            } else if resp.clicked() {
                                st.press(shift);
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
                // OK is the default button, Esc cancels.
                if st.edit.is_none() && result.is_none() {
                    result = ui.input(|i| if i.key_pressed(egui::Key::Enter) { Some(true) } else if i.key_pressed(egui::Key::Escape) { Some(false) } else { None });
                }
            });
        match result {
            Some(true) => {
                if let Some(changed) = st.apply(&mut self.docs[ai]) {
                    self.docs[ai].set.modified = true;
                    if !changed.is_empty() {
                        self.recompute_doc(ai);
                    }
                }
            }
            Some(false) => {}
            None if open => self.dlg.coef = Some(st),
            None => {}
        }
    }
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
        // Ua is in V: vector yes (unticked — 0xE1 puts no channel on a
        // diagram), circle no; the discrete channel has only column 1.
        assert_eq!(cell(&d, ChanKey::Analog(0), 3), Some(false));
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
        d.selection.insert(ChanKey::Analog(1));
        d.hviz[0] = crate::app::HVizir { shown: true, chan: Some(1), value: 5.0 };
        d.hviz[1] = crate::app::HVizir { shown: true, chan: Some(2), value: 5.0 };
        toggle_cell(&mut d, ChanKey::Analog(0), 1);
        assert!(!d.chans[0].visible && !d.chans[1].visible, "the whole group hides (`0x434b1c`)");
        assert_eq!((d.chans[0].group, d.chans[1].group), (None, None));
        assert!(d.selection.is_empty());
        assert_eq!(d.hviz[0], crate::app::HVizir::default(), "a ГВ on a member: never placed (`0x455df0`)");
        assert!(d.hviz[1].shown, "a ГВ on another channel stays");
        toggle_cell(&mut d, ChanKey::Digital(0), 1);
        assert!(!d.digital_visible[0]);
    }

    /// The header's counts (`0x435080`): column 2 counts every analog-kind
    /// row and writes the non-pending ones — formula channels too (L9.188);
    /// column 1 hides without dissolving groups.
    #[test]
    fn header_clicks_use_the_originals_counts() {
        let mut d = demo_doc();
        d.computed.push(crate::app::Computed::new("X".into(), "Ua".into(), vec![], Color32::RED));
        toggle_column(&mut d, 2);
        assert!(d.chans.iter().all(|c| c.on_freq) && d.computed[0].on_freq);
        assert_eq!(cell(&d, ChanKey::Computed(0), 2), Some(true));
        toggle_column(&mut d, 2);
        assert!(d.chans.iter().all(|c| !c.on_freq) && !d.computed[0].on_freq, "all set: cleared");
        d.chans[0].group = Some(3);
        d.chans[1].group = Some(3);
        toggle_column(&mut d, 1);
        assert!(d.chans.iter().all(|c| !c.visible));
        assert_eq!(d.chans[0].group, Some(3), "no dissolve from the header");
    }

    /// A renamed channel shows its file name after the new one (`0x4342e3`).
    #[test]
    fn a_renamed_channel_keeps_its_original_name() {
        let mut d = demo_doc();
        d.init_set_keys();
        assert_eq!(d.original_name(ChanKey::Analog(0)), None);
        let old = d.osc.analog[0].id.clone();
        assert_eq!(rename(&mut d, ChanKey::Analog(0), "Новое"), Ok(()));
        assert_eq!(d.original_name(ChanKey::Analog(0)), Some(old));
    }

    /// `0x436d77`–`0x436e64`: the refresh keeps only Ohm / VA channels on
    /// the circle diagram (no case), W / var ones ticked in column 4 leave.
    #[test]
    fn the_circle_refresh_keeps_ohm_and_va_only() {
        let mut d = demo_doc();
        for (name, unit) in [("P", "W"), ("Q", "var"), ("Z", "OHM"), ("S", "VA")] {
            let mut c = crate::app::Computed::new(name.into(), "Ua".into(), vec![], Color32::RED);
            c.unit = unit.into();
            c.on_circle = true;
            d.computed.push(c);
        }
        assert!(d.computed.iter().all(|c| cell(&d, ChanKey::Computed(d.computed.iter().position(|x| x.name == c.name).unwrap()), 4).is_some()), "all four tickable");
        circle_refresh_flags(&mut d);
        assert_eq!(d.computed.iter().map(|c| c.on_circle).collect::<Vec<_>>(), [false, false, true, true]);
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

    /// `0x47b020` / `0x47b3d0`: the range is built by Shift + double-click
    /// from the anchor; the commit fills every selected cell, then clears
    /// the selection without Shift; a single click selects nothing.
    #[test]
    fn coefficient_bulk_entry_fills_the_shift_range() {
        let mut d = demo_doc();
        let mut g = CoefDlg::new(&d);
        assert_eq!(g.cells[0], ["1".to_string(), "1".to_string()]);
        // Double-click row 0 primary, then Shift + double-click row 2
        // secondary: the rectangle, its edit open; type and commit.
        g.double_click((0, 0), false);
        g.double_click((2, 1), true);
        assert_eq!(g.sel.len(), 6);
        g.edit.as_mut().unwrap().1 = "110000".into();
        g.press(false);
        for r in 0..3 {
            assert_eq!(g.cells[r], ["110000".to_string(), "110000".to_string()]);
        }
        assert_eq!(g.cells[3][0], "1");
        assert!(g.sel.is_empty(), "cleared after the write");
        // A single click selects nothing; a double-click edits one cell.
        g.press(false);
        assert!(g.sel.is_empty());
        g.double_click((4, 1), false);
        g.edit.as_mut().unwrap().1 = "bad".into();
        g.press(false);
        assert_eq!(g.cells[4][1], "bad");
        let changed = g.apply(&mut d);
        assert_eq!(changed, Some(vec![0, 1, 2, 4]));
        assert_eq!(d.osc.analog[1].primary, 110000.0);
        // No validation: bad text is 0.
        assert_eq!(d.osc.analog[4].secondary, 0.0);
    }

    /// DIALOG 463's pair rows (`0x433b00`): differential then R/X pairs
    /// between the analog and the discrete channels, «b | a» / «a | b»;
    /// column 5 toggles a differential pair, column 4 an R/X one.
    #[test]
    fn channels_dialog_lists_the_pairs() {
        let mut d = demo_doc();
        d.dif_pairs = vec![crate::diagram::ChanPair::new("Ia", "Ib")];
        d.rx_pairs = vec![crate::diagram::ChanPair::new("Ua", "Ia")];
        let r = dlg_rows(&d);
        let na = d.osc.analog.len();
        assert_eq!((r[na], r[na + 1]), (DlgRow::Pair(PairRow::Dif(0)), DlgRow::Pair(PairRow::Rx(0))));
        assert!(matches!(r[na + 2], DlgRow::Chan(ChanKey::Digital(0))));
        assert_eq!((pair_name(&d, PairRow::Dif(0)), pair_name(&d, PairRow::Rx(0))), ("Ib | Ia".to_string(), "Ua | Ia".to_string()));
        assert_eq!((pair_cell(&d, PairRow::Dif(0), 4), pair_cell(&d, PairRow::Rx(0), 5)), (None, None));
        let on = d.dif_pairs[0].on;
        toggle_pair_cell(&mut d, PairRow::Dif(0), 5);
        assert_eq!(d.dif_pairs[0].on, !on);
        assert!(d.set.modified);
    }

    #[test]
    fn coefficients_ok_without_edits_changes_nothing() {
        let mut d = demo_doc();
        let mut g = CoefDlg::new(&d);
        g.press(false);
        // An edit that keeps the text is no change either.
        g.double_click((1, 1), false);
        g.press(false);
        assert!(g.apply(&mut d).is_none(), "nothing dirty: OnCancel");
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
            // The pointer arrives first: a move and a press in one frame
            // would be read as a drag (and move the window).
            self.frame_mod(vec![egui::Event::PointerMoved(pos)], modifiers);
            self.frame_mod(vec![ev(true)], modifiers);
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
            self.double_click_mod(pos, Default::default());
        }

        fn double_click_mod(&mut self, pos: Pos2, m: egui::Modifiers) {
            // Past the last click's double-click window first (0.3 s).
            for _ in 0..7 {
                self.frame(vec![]);
            }
            self.click_mod(pos, m);
            self.click_mod(pos, m);
            self.frame_mod(vec![], m);
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

    /// DIALOG 463 has no OK: Enter only commits a rename, Esc closes the
    /// dialog and drops a running one.
    #[test]
    fn channels_enter_commits_esc_closes() {
        let key = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        let mut h = Harness::new();
        h.app.dispatch(Command::ChannelsDialog);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click(ch_cell(h.ch_origin(), Some(0), 0));
        h.app.dlg.channels.as_mut().unwrap().rename.as_mut().unwrap().1 = "Ua1".into();
        h.frame(vec![key(egui::Key::Enter)]);
        h.frame(vec![]);
        assert_eq!(h.app.docs[0].osc.analog[0].id, "Ua1");
        assert!(h.app.dlg.channels.is_some(), "Enter keeps the dialog");
        h.click(ch_cell(h.ch_origin(), Some(0), 0));
        h.app.dlg.channels.as_mut().unwrap().rename.as_mut().unwrap().1 = "Ux".into();
        h.frame(vec![key(egui::Key::Escape)]);
        h.frame(vec![]);
        assert!(h.app.dlg.channels.is_none(), "Esc closes");
        assert_eq!(h.app.docs[0].osc.analog[0].id, "Ua1", "the rename is dropped");
    }

    #[test]
    fn coefficients_double_click_type_and_shift_click_fill() {
        let mut h = Harness::new();
        h.app.dispatch(Command::Coefficients);
        h.frame(vec![]);
        for _ in 0..6 {
            h.frame(vec![]);
        }
        // The cells' places as drawn.
        let cell = |h: &Harness, r: usize, c: usize| {
            let st = h.app.dlg.coef.as_ref().expect("dialog drawn");
            st.cell_rects.iter().find(|(at, _)| *at == (r, c)).map(|(_, rect)| rect.center()).expect("cell drawn")
        };
        let at = cell(&h, 0, 0);
        h.double_click(at);
        assert_eq!(h.app.dlg.coef.as_ref().unwrap().edit.as_ref().map(|e| e.0), Some((0, 0)));
        // Shift + double-click row 3 of the same column (`0x47b020`): the
        // range from the anchor, the edit on row 3; type, press elsewhere.
        let at = cell(&h, 3, 0);
        h.double_click_mod(at, egui::Modifiers::SHIFT);
        assert_eq!(h.app.dlg.coef.as_ref().unwrap().sel.len(), 4);
        h.app.dlg.coef.as_mut().unwrap().edit.as_mut().unwrap().1 = "6000".into();
        let at = cell(&h, 5, 1);
        h.click(at);
        let st = h.app.dlg.coef.as_ref().unwrap();
        assert!(st.edit.is_none());
        assert_eq!((0..4).map(|r| st.cells[r][0].as_str()).collect::<Vec<_>>(), ["6000"; 4]);
        assert_eq!(st.cells[4][0], "1");
        assert!(st.cells.iter().all(|c| c[1] == "1"));
    }
}
