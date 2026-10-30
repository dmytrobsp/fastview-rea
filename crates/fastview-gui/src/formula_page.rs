//! «Редактор формул» (DIALOG 340, `CQAnFormulaEditorPg`, ledger L9.56): the
//! last page of «Быстрый анализ». It edits a copy of the record's formula
//! list (ctor `0x485e30`) — but deleting rows, «Удалить все» and «Загрузить»
//! remove the record's channels at once, and «Отмена» undoes nothing.
//! «Рассчитать» with this page active applies the list (`0x415780`): a
//! channel with the row's name is updated in place (flags and colour kept),
//! a new name becomes a new channel after the analog ones.

use crate::app::App;
use eframe::egui;
use fvb_core::addformula as af;

/// The page while the sheet is up.
#[derive(Debug, Clone, Default)]
pub struct FormulaPage {
    /// (name, formula) rows, the list view's content.
    pub rows: Vec<(String, String)>,
    pub selected: Option<usize>,
    /// `page+0x2f8`: the list was changed.
    pub dirty: bool,
    /// The in-place edit (`0x486850`): row, column, text.
    pub edit: Option<(usize, usize, String)>,
    /// A message box over the sheet (396 / 473, MB_ICONERROR).
    pub alert: Option<&'static str>,
}

/// String 3165: the `.frm` filter.
const FRM_FILTER: &str = "Вычисляемые каналы FastView (*.frm)";

impl App {
    /// The page's copy of the record's formula channels (`0x485e30`).
    pub fn formula_page_open(&mut self) {
        let rows = self
            .active_doc()
            .map(|d| d.computed.iter().filter(|c| crate::setfile::is_formula(c)).map(|c| (c.name.clone(), c.formula.clone())).collect())
            .unwrap_or_default();
        self.formula_page = FormulaPage { rows, ..Default::default() };
    }

    /// `UpdateData(TRUE)` with the list changed: every formula channel of the
    /// record whose name is no longer a row is deleted at once.
    pub fn fp_update(&mut self) {
        if !self.formula_page.dirty {
            return;
        }
        let names: Vec<String> = self.formula_page.rows.iter().map(|r| r.0.clone()).collect();
        if let Some(d) = self.active_doc_mut() {
            d.removing_channels(|d| d.computed.retain(|c| !crate::setfile::is_formula(c) || names.contains(&c.name)));
            d.channels_changed();
        }
    }

    /// The name check of an in-place edit of column 0 (`0x486a50`): a
    /// leading letter (396), not a channel of the record nor another row
    /// (473, case-sensitive — so re-confirming a computed channel's own
    /// name is refused too).
    fn fp_name_error(&self, row: usize, name: &str) -> Option<&'static str> {
        if !name.chars().next().is_some_and(char::is_alphabetic) {
            return Some(af::MSG_BAD_NAME);
        }
        let d = self.active_doc()?;
        let taken = d.osc.analog.iter().map(|c| c.id.as_str()).chain(d.osc.digital.iter().map(|c| c.id.as_str())).chain(d.computed.iter().map(|c| c.name.as_str()));
        let other_row = self.formula_page.rows.iter().enumerate().any(|(i, r)| i != row && r.0 == name);
        (taken.into_iter().any(|n| n == name) || other_row).then_some(af::MSG_NAME_TAKEN)
    }

    /// End the in-place edit (`0x486bf0`): the text goes into the cell (a
    /// name only when it passes the check), dirty when it changed.
    pub fn fp_commit_edit(&mut self) {
        let Some((row, col, text)) = self.formula_page.edit.take() else { return };
        if row >= self.formula_page.rows.len() {
            return;
        }
        if col == 0 {
            if let Some(e) = self.fp_name_error(row, &text) {
                self.formula_page.alert = Some(e);
                return;
            }
        }
        let cell = if col == 0 { &mut self.formula_page.rows[row].0 } else { &mut self.formula_page.rows[row].1 };
        if *cell != text {
            *cell = text;
            self.formula_page.dirty = true;
        }
    }

    /// Kill-active (`0x486d20`): end the edit and update. (The original also
    /// writes the copy into the record's definition list without computing
    /// it; ours computes only on «Рассчитать».)
    pub fn fp_kill_active(&mut self) {
        self.fp_commit_edit();
        self.fp_update();
    }

    /// ↑ (`0x487210`) / ↓ (`0x4873a0`): move the selected row, keep it selected.
    pub fn fp_move(&mut self, up: bool) {
        let p = &mut self.formula_page;
        let Some(i) = p.selected.filter(|&i| i < p.rows.len()) else { return };
        let j = if up { i.checked_sub(1) } else { Some(i + 1).filter(|&j| j < p.rows.len()) };
        let Some(j) = j else { return };
        p.rows.swap(i, j);
        p.selected = Some(j);
        p.dirty = true;
        self.fp_update();
    }

    /// «Удалить» (`0x487170`): no confirmation; the same index (or the new
    /// last row) stays selected; the channel goes at once.
    pub fn fp_delete(&mut self) {
        let p = &mut self.formula_page;
        let Some(i) = p.selected.filter(|&i| i < p.rows.len()) else { return };
        p.rows.remove(i);
        p.selected = (!p.rows.is_empty()).then(|| i.min(p.rows.len() - 1));
        p.dirty = true;
        self.fp_update();
    }

    /// «Удалить все» (`0x487530`): no confirmation.
    pub fn fp_delete_all(&mut self) {
        self.formula_page.rows.clear();
        self.formula_page.selected = None;
        self.formula_page.dirty = true;
        self.fp_update();
    }

    /// DIALOG 359's OK (`0x486df0`): the row appended and selected.
    pub fn fp_add_row(&mut self, name: String, formula: String) {
        let p = &mut self.formula_page;
        p.rows.push((name, formula));
        p.selected = Some(p.rows.len() - 1);
        p.dirty = true;
        self.fp_update();
    }

    /// «Загрузить» (`0x414ee0`): the rows' channels are deleted from the
    /// record first, then the list is the file's; the record is modified.
    pub fn fp_load_rows(&mut self, rows: Vec<(String, String)>) {
        let names: Vec<String> = self.formula_page.rows.iter().map(|r| r.0.clone()).collect();
        if let Some(d) = self.active_doc_mut() {
            d.removing_channels(|d| d.computed.retain(|c| !crate::setfile::is_formula(c) || !names.contains(&c.name)));
            d.set.modified = true;
            d.channels_changed();
        }
        let p = &mut self.formula_page;
        p.rows = rows;
        p.selected = None;
        p.dirty = true;
    }

    /// «Рассчитать» on this page (`0x415780`): each row in order — the
    /// record's channel of that name (exact, else case-insensitive) gets the
    /// formula and is recomputed in place, a new name goes to the end of the
    /// analog-kind block (`0x42ea36`). No diagram flags.
    /// On an error the record's formula channels are left as before and the
    /// message is shown.
    pub fn fp_apply(&mut self) {
        let rows = self.formula_page.rows.clone();
        let show_primary = self.active_doc().is_some_and(crate::app::Doc::primary_values);
        let Some(d) = self.active_doc_mut() else { return };
        d.sync_flagged_names();
        let before = d.computed.clone();
        for (name, formula) in &rows {
            let found = d.computed.iter().position(|c| c.name == *name).or_else(|| {
                let l = name.to_lowercase();
                d.computed.iter().position(|c| c.name.to_lowercase() == l)
            });
            let existing: Vec<_> = d.computed.iter().enumerate().filter(|(i, _)| Some(*i) != found).map(|(_, c)| c.as_core()).collect();
            let batch = [(name.clone(), formula.clone())];
            match fvb_core::quick::evaluate_batch_with(&d.flagged, fvb_core::freqcorr::of(&d.osc), &existing, &batch, show_primary) {
                Ok(mut added) => {
                    let c = added.remove(0);
                    match found {
                        Some(i) => {
                            let cc = &mut d.computed[i];
                            cc.formula = c.formula;
                            cc.unit = c.unit;
                            cc.set_values(c.samples, c.phasors);
                            d.reflag_computed(i);
                        }
                        None => {
                            // At the analog-kind block's end (`rec+0x58`,
                            // `0x42ea36`): after every channel before it.
                            let place = d.kind0_order().len();
                            let color = crate::app::channel_color(&c.name, "", place);
                            let mut cc = crate::app::Computed::new(c.name, c.formula, Vec::new(), color);
                            cc.unit = c.unit;
                            cc.set_values(c.samples, c.phasors);
                            d.computed.push(cc);
                        }
                    }
                }
                Err(e) => {
                    d.computed = before;
                    self.dlg.push_alert(e.to_string());
                    return;
                }
            }
        }
        // `0x415b9a`: the batch done, groups whose member is gone dissolve.
        d.prune_groups();
        d.channels_changed();
    }

    /// The page (DIALOG 340): the two-column list (25 % / 70 %, full-row
    /// selection, grid lines, no sorting), a double click on the selected
    /// row edits the cell under the pointer, and the seven buttons (never
    /// greyed).
    /// DIALOG 340 «Редактор формул» on its 337 × 257 DLU template from the
    /// page's origin `o` (VM: the list at 7, 7 — 316 × 203 — with a (130,
    /// 135, 144) border, a 25 px header with (229) separators 2 px inside
    /// each column, columns 120 and 331 px, 17 px rows ruled in (240)
    /// down the whole list; the buttons at 218).
    pub(crate) fn formula_page_ui(&mut self, ui: &mut egui::Ui, o: egui::Pos2) {
        use crate::dialogs::{dlu, win_button, DLG_FONT};
        use egui::{Color32, Rect, Stroke};
        let origin = Rect::from_min_size(o, egui::Vec2::ZERO);
        let list = dlu(origin, 7, 7, 316, 203);
        let inner = list.shrink(1.0);
        let p = ui.painter().clone();
        p.rect_filled(list, 0.0, Color32::from_rgb(130, 135, 144));
        p.rect_filled(inner, 0.0, Color32::WHITE);
        let (hdr_h, row_h) = (25.0, 17.0);
        let cols = [(0.0, 120.0), (120.0, 331.0)];
        let grid = Stroke::new(1.0f32, Color32::from_gray(240));
        for (x, w) in cols {
            p.vline(inner.left() + x + w - 0.5, inner.top() + hdr_h..=inner.bottom(), grid);
        }
        let mut y = inner.top() + hdr_h + row_h - 0.5;
        while y < inner.bottom() {
            p.hline(inner.x_range(), y, grid);
            y += row_h;
        }
        for ((x, w), t) in cols.into_iter().zip(["Название канала", "Формула"]) {
            let r = Rect::from_min_size(inner.min + egui::vec2(x, 0.0), egui::vec2(w, hdr_h));
            p.vline(r.right() - 1.5, r.y_range(), Stroke::new(1.0f32, Color32::from_gray(229)));
            p.with_clip_rect(r).text(r.left_center() + egui::vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, t, DLG_FONT, Color32::BLACK);
        }
        let mut double = None;
        let mut pressed_outside = false;
        let rows_rect = Rect::from_min_max(egui::pos2(inner.left(), inner.top() + hdr_h), inner.max);
        // The list draws its selection only with the focus (no
        // LVS_SHOWSELALWAYS): not under «Добавить новую формулу» or a
        // message box (VM: «X1» unmarked behind the 473 box).
        let list_focus = self.formula_dlg.is_none() && !crate::dialogs::under_message(ui.ctx());
        // A click below the rows clears the selection (a ListView's, VM).
        let blank = ui.interact(rows_rect, ui.id().with("fp_blank"), egui::Sense::click());
        if blank.clicked() {
            self.formula_page.selected = None;
        }
        // The list is the first tab stop (DIALOG 340's order: the list,
        // «Загрузить», «Сохранить», «Добавить», «Удалить», ▲, ▼, «Удалить
        // все»).
        crate::dialogs::tab_index(ui, 0);
        crate::dialogs::tab_stop(ui, blank.id, crate::dialogs::StopKind::Other);
        let fp = &mut self.formula_page;
        for i in 0..fp.rows.len() {
            let top = rows_rect.top() + row_h * i as f32;
            if top + row_h > rows_rect.bottom() {
                break;
            }
            let sel = fp.selected == Some(i) && list_focus;
            let row = Rect::from_min_size(egui::pos2(inner.left(), top), egui::vec2(cols[1].0 + cols[1].1 - 1.0, row_h - 1.0));
            if sel {
                p.rect_filled(row, 0.0, Color32::from_rgb(0, 120, 215));
            }
            for (col, (x, w)) in cols.into_iter().enumerate() {
                let r = Rect::from_min_size(egui::pos2(inner.left() + x, top), egui::vec2(w - 1.0, row_h - 1.0));
                let editing = matches!(&fp.edit, Some((er, ec, _)) if *er == i && *ec == col);
                if editing {
                    let Some((_, _, text)) = fp.edit.as_mut() else { continue };
                    let resp = ui.put(r, egui::TextEdit::singleline(text).frame(false).font(DLG_FONT).margin(egui::Margin::symmetric(5.0, 1.0)));
                    resp.request_focus();
                    if ui.input(|inp| inp.pointer.any_pressed()) && !resp.hovered() {
                        pressed_outside = true;
                    }
                    continue;
                }
                let text = if col == 0 { &fp.rows[i].0 } else { &fp.rows[i].1 };
                p.with_clip_rect(r).text(r.left_center() + egui::vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, text, DLG_FONT, if sel { Color32::WHITE } else { Color32::BLACK });
                let resp = ui.interact(r, ui.id().with(("fp_cell", i, col)), egui::Sense::click());
                if resp.clicked() {
                    fp.selected = Some(i);
                }
                if resp.double_clicked() {
                    double = Some(col);
                }
            }
        }
        // The edit starts on the selection mark's row, in the clicked column.
        if let (Some(col), Some(row)) = (double, self.formula_page.selected) {
            self.fp_commit_edit();
            let text = if col == 0 { self.formula_page.rows[row].0.clone() } else { self.formula_page.rows[row].1.clone() };
            self.formula_page.edit = Some((row, col, text));
        }
        // Enter and Esc both commit (Esc does not close the sheet).
        if self.formula_page.edit.is_some() && (pressed_outside || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape))) {
            self.fp_commit_edit();
        }
        crate::dialogs::tab_index(ui, 1);
        if win_button(ui, dlu(origin, 7, 218, 52, 14), "Загрузить", false).clicked() {
            if let Some(path) = crate::dialogs::file_dialog().add_filter(FRM_FILTER, &["frm"]).add_filter("Все файлы (*.*)", &["*"]).pick_file() {
                if let Ok(bytes) = std::fs::read(&path) {
                    let text = fvb_core::comtrade::decode_bytes(&bytes, self.encoding);
                    self.fp_load_rows(fvb_core::quick::parse_frm(&text));
                }
            }
        }
        crate::dialogs::tab_index(ui, 2);
        if win_button(ui, dlu(origin, 63, 218, 52, 14), "Сохранить", false).clicked() {
            self.fp_update();
            if let Some(path) = crate::dialogs::file_dialog().add_filter(FRM_FILTER, &["frm"]).add_filter("Все файлы (*.*)", &["*"]).set_file_name("*.frm").save_file() {
                let text = fvb_core::quick::write_frm(&self.formula_page.rows);
                let _ = std::fs::write(path, fvb_core::comtrade::encode_text(&text, fvb_core::comtrade::Encoding::Cp1251));
            }
        }
        crate::dialogs::tab_index(ui, 5);
        if tri_button(ui, dlu(origin, 120, 218, 16, 14), true).clicked() {
            self.fp_move(true);
        }
        crate::dialogs::tab_index(ui, 6);
        if tri_button(ui, dlu(origin, 140, 218, 16, 14), false).clicked() {
            self.fp_move(false);
        }
        crate::dialogs::tab_index(ui, 3);
        if win_button(ui, dlu(origin, 159, 218, 52, 14), "Добавить", false).clicked() {
            self.open_formula_dlg();
        }
        crate::dialogs::tab_index(ui, 4);
        if win_button(ui, dlu(origin, 215, 218, 52, 14), "Удалить", false).clicked() {
            self.fp_delete();
        }
        crate::dialogs::tab_index(ui, 7);
        if win_button(ui, dlu(origin, 271, 218, 52, 14), "Удалить все", false).clicked() {
            self.fp_delete_all();
        }
    }
}

/// ↑ 1018 / ↓ 1019 (`0x4860a0`): bitmap 475's icons 6 / 5 on a push
/// button — a (136, 0, 20) triangle 9 px at the base and 5 rows tall,
/// centred at +11, rows 9–13 of the 24 × 23 button (VM).
fn tri_button(ui: &mut egui::Ui, r: egui::Rect, up: bool) -> egui::Response {
    let resp = crate::dialogs::win_button(ui, r, "", false);
    let ink = egui::Color32::from_rgb(136, 0, 20);
    for k in 0..5 {
        let half = if up { k } else { 4 - k } as f32;
        let row = egui::Rect::from_min_size(egui::pos2(r.left() + 11.0 - half, r.top() + 9.0 + k as f32), egui::vec2(2.0 * half + 1.0, 1.0));
        ui.painter().rect_filled(row, 0.0, ink);
    }
    resp
}
