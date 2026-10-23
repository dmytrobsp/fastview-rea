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
            d.computed.retain(|c| !crate::setfile::is_formula(c) || names.contains(&c.name));
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
            d.computed.retain(|c| !crate::setfile::is_formula(c) || !names.contains(&c.name));
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
    /// formula and is recomputed in place, a new name is inserted after the
    /// analog channels (each new one at the same place). No diagram flags.
    /// On an error the record's formula channels are left as before and the
    /// message is shown.
    pub fn fp_apply(&mut self) {
        let rows = self.formula_page.rows.clone();
        let show_primary = self.active_doc().is_some_and(crate::app::Doc::primary_values);
        let base = self.docs.len() + 7;
        let Some(d) = self.active_doc_mut() else { return };
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
                            let mut cc = crate::app::Computed::new(c.name, c.formula, Vec::new(), Self::palette(base + d.computed.len()));
                            cc.unit = c.unit;
                            cc.set_values(c.samples, c.phasors);
                            d.computed.insert(0, cc);
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
        d.channels_changed();
    }

    /// The page (DIALOG 340): the two-column list (25 % / 70 %, full-row
    /// selection, grid lines, no sorting), a double click on the selected
    /// row edits the cell under the pointer, and the seven buttons (never
    /// greyed).
    pub(crate) fn formula_page_ui(&mut self, ui: &mut egui::Ui) {
        let w = ui.available_width().min(520.0);
        let (w0, w1) = (w * 0.25, w * 0.70);
        let mut double = None;
        let mut pressed_outside = false;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(w);
            ui.set_min_height(300.0);
            egui::Grid::new("fp_list").num_columns(2).min_col_width(0.0).spacing([0.0, 0.0]).show(ui, |ui| {
                cell(ui, w0, "Название канала", false, true);
                cell(ui, w1, "Формула", false, true);
                ui.end_row();
                let p = &mut self.formula_page;
                for i in 0..p.rows.len() {
                    let sel = p.selected == Some(i);
                    for (col, width) in [(0usize, w0), (1, w1)] {
                        let editing = matches!(&p.edit, Some((r, c, _)) if *r == i && *c == col);
                        if editing {
                            let Some((_, _, text)) = p.edit.as_mut() else { continue };
                            let r = ui.add_sized([width, 18.0], egui::TextEdit::singleline(text).frame(false));
                            r.request_focus();
                            if ui.input(|inp| inp.pointer.any_pressed()) && !r.hovered() {
                                pressed_outside = true;
                            }
                        } else {
                            let text = if col == 0 { &p.rows[i].0 } else { &p.rows[i].1 };
                            let r = cell(ui, width, text, sel, false);
                            if r.clicked() {
                                p.selected = Some(i);
                            }
                            if r.double_clicked() {
                                double = Some(col);
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        });
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
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Загрузить").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter(FRM_FILTER, &["frm"]).add_filter("Все файлы (*.*)", &["*"]).pick_file() {
                    if let Ok(bytes) = std::fs::read(&path) {
                        let text = fvb_core::comtrade::decode_bytes(&bytes, self.encoding);
                        self.fp_load_rows(fvb_core::quick::parse_frm(&text));
                    }
                }
            }
            if ui.button("Сохранить").clicked() {
                self.fp_update();
                if let Some(path) = rfd::FileDialog::new().add_filter(FRM_FILTER, &["frm"]).add_filter("Все файлы (*.*)", &["*"]).set_file_name("*.frm").save_file() {
                    let text = fvb_core::quick::write_frm(&self.formula_page.rows);
                    let _ = std::fs::write(path, fvb_core::comtrade::encode_text(&text, fvb_core::comtrade::Encoding::Cp1251));
                }
            }
            if ui.button("↑").clicked() {
                self.fp_move(true);
            }
            if ui.button("↓").clicked() {
                self.fp_move(false);
            }
            if ui.button("Добавить").clicked() {
                self.open_formula_dlg();
            }
            if ui.button("Удалить").clicked() {
                self.fp_delete();
            }
            if ui.button("Удалить все").clicked() {
                self.fp_delete_all();
            }
        });
    }
}

/// A list-view cell: left-aligned text, the selected row highlighted, a
/// grid line at the bottom and right; a header cell in bold.
fn cell(ui: &mut egui::Ui, width: f32, text: &str, selected: bool, header: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(egui::Vec2::new(width, 18.0), if header { egui::Sense::hover() } else { egui::Sense::click() });
    let v = ui.visuals();
    if selected {
        ui.painter().rect_filled(rect, 0.0, v.selection.bg_fill);
    }
    let grid = egui::Stroke::new(1.0f32, v.widgets.noninteractive.bg_stroke.color);
    ui.painter().hline(rect.x_range(), rect.bottom(), grid);
    ui.painter().vline(rect.right(), rect.y_range(), grid);
    let color = if selected { v.selection.stroke.color } else { v.text_color() };
    let font = if header { egui::FontId::proportional(13.0) } else { egui::FontId::proportional(12.5) };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    ui.painter().with_clip_rect(rect).galley(rect.left_center() + egui::Vec2::new(3.0, -galley.size().y / 2.0), galley, color);
    r
}
