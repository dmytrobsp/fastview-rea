//! «Добавить новую формулу» (DIALOG 359, `AddFormulaDlg`), opened by
//! «Добавить» of the formula editor (`CQAnFormulaEditorPg`, `0x486df0`):
//! the template tree on the left, «Название канала:» and the selected
//! template's form on the right. Selecting a tree item re-creates the form
//! with its initial values (`0x403280`); a group shows no form and greys OK.
//! OK checks the name as the original's sequence does (396 / 473, the
//! dialog stays) and appends the formula the form builds (`fvb_core::addformula::build`) as a computed channel.

use crate::dialogs::MsgIcon;
use crate::app::App;
use eframe::egui::{self, Color32, Rect, Vec2};
use fvb_core::addformula::{self as af, Kind, Node, Values};
use std::collections::BTreeSet;

/// A tree item: the labels from the root down to it (leaf labels repeat).
pub type Path = Vec<&'static str>;

/// The dialog while open.
#[derive(Debug, Clone)]
pub struct AddFormulaDlg {
    /// «Название канала:», the edit control's text.
    pub name: String,
    /// The name member `+0x1c4`: the last text that passed the DDX rule
    /// (initially the proposed name).
    pub member: String,
    /// The selected tree item.
    pub selected: Path,
    /// The hosted form: the selected leaf's dialog id (`+0x1cc`); `None` for
    /// a group, which disables OK.
    pub form: Option<u16>,
    /// The form's control texts by id.
    pub values: Values,
    /// Groups the user folded (the original inserts every group expanded).
    pub collapsed: BTreeSet<Path>,
    /// Message boxes over the dialog (396 / 473), shown one after another:
    /// the DDX's 396 is MB_OK (`0x403213`), the member check's 396 / 473
    /// MB_ICONHAND (`0x4033ed` / `0x403519`).
    pub alerts: Vec<(&'static str, MsgIcon)>,
    /// The formula member `+0x1c8`: empty until an OK passes the name check.
    pub formula: String,
    /// A field of the hosted form had the focus this frame (its Enter / Esc
    /// go to the form, `0x401c40` / `0x485d10`).
    pub form_focus: bool,
    /// Items of the channel lists: the record's analog channels, then the
    /// computed channels (`0x411c70`).
    pub channels: Vec<String>,
    /// Names OK refuses with 473: every channel of the record and every
    /// computed channel (`0x4033a0`).
    pub taken: Vec<String>,
    pub sample_rate: f64,
    pub line_freq: f64,
    /// Where the tree items and OK were drawn (for the UI tests).
    pub item_rects: Vec<(Path, Rect)>,
    pub ok_rect: Option<Rect>,
}

impl AddFormulaDlg {
    /// The dialog as `0x401c10` opens it: «Формула» selected, its form shown.
    pub fn new(name: String, channels: Vec<String>, taken: Vec<String>, sample_rate: f64, line_freq: f64) -> Self {
        let mut d = AddFormulaDlg {
            member: name.clone(),
            name,
            selected: Vec::new(),
            form: None,
            values: Values::new(),
            collapsed: BTreeSet::new(),
            alerts: Vec::new(),
            formula: String::new(),
            form_focus: false,
            channels,
            taken,
            sample_rate,
            line_freq,
            item_rects: Vec::new(),
            ok_rect: None,
        };
        d.select(vec!["Формула"], Some(af::FREE_FORMULA));
        d
    }

    /// TVN_SELCHANGED (`0x403260`): host the item's form, fresh.
    pub fn select(&mut self, path: Path, form: Option<u16>) {
        self.selected = path;
        self.form = form;
        self.values = form.map_or_else(Values::new, |d| af::initial_values(d, &self.channels, self.sample_rate, self.line_freq));
    }

    /// OK (`0x403530`), then (name, formula, unit). `UpdateData(TRUE)`
    /// runs the DDX rule — a failure shows 396 and leaves the member as it
    /// was — but its result is ignored: the member is checked (`0x4033a0`,
    /// 396 / 473 stop here), and only then `CDialog::OnOK` runs `UpdateData`
    /// again, which repeats the 396 and keeps the dialog open.
    pub fn accept(&mut self) -> Option<(String, String)> {
        self.on_ok().then(|| (self.member.clone(), self.formula.clone()))
    }

    /// Enter / OK in the hosted form (`0x401c40`): the parent's OnOK, then
    /// EndDialog(IDOK) whatever it did — the caller appends a row from the
    /// two members as they stand (a refused name: the member and the
    /// formula of the last OK that got that far, empty at first).
    pub fn accept_from_form(&mut self) -> (String, String) {
        self.on_ok();
        (self.member.clone(), self.formula.clone())
    }

    /// `0x403530`: true when `CDialog::OnOK` ends the dialog. The formula
    /// is built once the member passes, before the second DDX.
    fn on_ok(&mut self) -> bool {
        let Some(form) = self.form else { return false };
        let taken: Vec<&str> = self.taken.iter().map(String::as_str).collect();
        let ddx_ok = af::ddx_name_ok(&self.name);
        if ddx_ok {
            self.member = self.name.clone();
        } else {
            self.alerts.push((af::MSG_BAD_NAME, MsgIcon::Exclamation));
        }
        if let Err(e) = af::check_name(&self.member, &taken) {
            self.alerts.push((e.message(), MsgIcon::Error));
            return false;
        }
        let Some(f) = af::build(form, &self.values) else { return false };
        self.formula = f;
        if !ddx_ok {
            self.alerts.push((af::MSG_BAD_NAME, MsgIcon::Exclamation));
            return false;
        }
        true
    }
}

impl App {
    /// «Добавить» of the formula editor: open DIALOG 359 for the active
    /// record with the proposed name "Канал%i".
    pub fn open_formula_dlg(&mut self) {
        let Some(d) = self.active_doc() else {
            self.status = "Сначала загрузите осциллограмму.".into();
            return;
        };
        let computed: Vec<&str> = d.computed.iter().map(|c| c.name.as_str()).collect();
        // «Канал%i» over the page's rows (`0x486df0`).
        let rows: Vec<&str> = self.formula_page.rows.iter().map(|r| r.0.as_str()).collect();
        let analog = d.osc.analog.iter().map(|c| c.id.clone());
        let channels: Vec<String> = analog.clone().chain(computed.iter().map(|s| s.to_string())).collect();
        // `0x4033a0` refuses (473, Compare) the record's channels and then
        // the list at `+0x9c`: the page's rows (VM: a second «X1» refused
        // while the first is only a row).
        let taken = channels.iter().cloned().chain(d.osc.digital.iter().map(|c| c.id.clone())).chain(rows.iter().map(|r| r.to_string())).collect();
        let name = af::default_name(&rows);
        self.formula_dlg = Some(AddFormulaDlg::new(name, channels, taken, d.osc.int_rate_hz(), d.osc.line_freq));
    }

    pub fn formula_dlg_window(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.formula_dlg.take() else { return };
        let mut done: Option<bool> = None;
        let mut open = true;
        let modal = st.alerts.is_empty();
        if !modal {
            // The message box has the focus: no typing reaches the dialog
            // under it, which keeps its looks (VM: only its caption greys).
            ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(_) | egui::Event::Key { key: egui::Key::Tab, .. })));
        }
        st.item_rects.clear();
        st.form_focus = false;
        // DIALOG 359 (417 × 310 DLU: 626 × 504 px), centred on its owner,
        // the «Быстрый анализ» sheet (VM).
        crate::dialogs::dlg(ctx, "Добавить новую формулу")
            .id(egui::Id::new("fv_add_formula"))
            .owner(egui::Id::new("fv_quick_sheet"))
            .margin(0.0)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                use crate::dialogs::{dlu, win_button_en, win_edit, win_label, win_tree, TreeRow};
                let (client, _) = ui.allocate_exact_size(crate::dialogs::dlu_size(417, 310), egui::Sense::hover());
                ui.scope(|ui| {
                    // The template tree (1039): lines, buttons and lines at
                    // the root; every group inserted expanded.
                    let mut flat = Vec::new();
                    tree_rows(af::TREE, &mut Vec::new(), &st, &mut flat);
                    let rows: Vec<TreeRow> = flat
                        .iter()
                        .map(|(path, kids, _)| TreeRow { depth: path.len() - 1, label: path[path.len() - 1].to_string(), has_children: *kids, expanded: !st.collapsed.contains(path), selected: st.selected == *path })
                        .collect();
                    let ev = win_tree(ui, dlu(client, 12, 12, 180, 266), egui::Id::new("af_tree"), Color32::from_gray(160), &rows, true, None, true, true, &mut |p, cell, k| branch_icon(p, cell, flat[k].0[0]));
                    for (k, r) in ev.rects.iter().enumerate() {
                        st.item_rects.push((flat[k].0.clone(), *r));
                    }
                    if let Some(k) = ev.toggled.or(ev.double_clicked.filter(|&k| flat[k].1)) {
                        let path = flat[k].0.clone();
                        if !st.collapsed.remove(&path) {
                            st.collapsed.insert(path);
                        }
                    }
                    if let Some(k) = ev.clicked {
                        st.select(flat[k].0.clone(), flat[k].2);
                    }
                    win_label(ui, dlu(client, 204, 14, 62, 8), "Название канала:", true, true);
                    win_edit(ui, dlu(client, 272, 12, 131, 12), egui::Id::new("af_name"), &mut st.name, false);
                    // The selected template's form in the place of control
                    // 1035 (205, 36, 198 × 243).
                    // The form is a child dialog without DS_CONTROL: its fields
                    // are not in this dialog's tab order (tree, name, OK,
                    // Отмена).
                    let place = dlu(client, 205, 36, 198, 243);
                    if let Some(d) = st.form {
                        crate::dialogs::no_tab_stop(ui, |ui| {
                            if !template_form(ui, place.min, d, &mut st) {
                                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(place), |ui| form_ui(ui, d, &mut st));
                            }
                        });
                    }
                    let ok_r = dlu(client, 293, 290, 50, 14);
                    st.ok_rect = Some(ok_r);
                    // The buttons follow the name edit in its span (template, no WS_GROUP).
                    crate::dialogs::group_continue(ui);
                    if win_button_en(ui, ok_r, "OK", true, st.form.is_some()).clicked() {
                        done = Some(true);
                    }
                    if win_button_en(ui, dlu(client, 351, 290, 50, 14), "Отмена", false, true).clicked() {
                        done = Some(false);
                    }
                });
            });
        if let Some(&(msg, icon)) = st.alerts.first() {
            let mut close = false;
            crate::dialogs::modal_backdrop(ctx, "fv_add_formula_alert_block", egui::Order::Foreground);
            crate::dialogs::dlg(ctx, crate::app::APP_TITLE)
                .id(egui::Id::new("fv_add_formula_alert"))
                .margin(0.0)
                .collapsible(false)
                .resizable(false)
                .order(egui::Order::Tooltip)
                .show(ctx, |ui| {
                    close = crate::dialogs::message_box(ui, icon, msg, &["OK"]).is_some();
                });
            if close || ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                st.alerts.remove(0);
            }
        } else if modal {
            // Enter / Esc in the dialog's own controls are its IDOK / IDCANCEL.
            // The form templates lack DS_CONTROL, so in a form field they go
            // to the form: `0x401c40` runs the parent's OnOK and closes it
            // with IDOK whatever happened, `0x485d10` closes it with IDCANCEL.
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && st.form.is_some() {
                if st.form_focus {
                    let (name, formula) = st.accept_from_form();
                    for (msg, icon) in st.alerts.drain(..) {
                        self.dlg.push_alert_icon(msg, icon);
                    }
                    self.fp_add_row(name, formula);
                    return;
                }
                done = Some(true);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some(false);
            }
        }
        if !open {
            done = Some(false);
        }
        match done {
            Some(true) => {
                if let Some((name, formula)) = st.accept() {
                    // The row is appended to the formula page (`0x486df0`).
                    self.fp_add_row(name, formula);
                    return;
                }
                self.formula_dlg = Some(st);
            }
            Some(false) => {}
            None => self.formula_dlg = Some(st),
        }
    }
}

/// The tree's visible rows in order: each item's path, whether it is a
/// group, and a leaf's form.
fn tree_rows(nodes: &[Node], path: &mut Path, st: &AddFormulaDlg, out: &mut Vec<(Path, bool, Option<u16>)>) {
    for n in nodes {
        let (label, kids, form) = match *n {
            Node::Group(l, k) => (l, Some(k), None),
            Node::Leaf(l, d) => (l, None, Some(d)),
        };
        path.push(label);
        out.push((path.clone(), kids.is_some(), form));
        if let Some(k) = kids.filter(|_| !st.collapsed.contains(path)) {
            tree_rows(k, path, st, out);
        }
        path.pop();
    }
}

/// A 16 × 16 glyph per top-level branch (the original's image indices 0,
/// 5, 2, 4, 3, 1, 6 for Формула, Анализ сигналов, Токи, Напряжения,
/// Мощности, Сопротивления, Дифференциальная защита — the images ours).
fn branch_icon(p: &egui::Painter, r: Rect, branch: &str) {
    let (glyph, color) = match branch {
        "Формула" => ("ƒ", egui::Color32::from_rgb(90, 90, 160)),
        "Анализ сигналов" => ("∿", egui::Color32::from_rgb(40, 120, 60)),
        "Токи" => ("I", egui::Color32::from_rgb(190, 60, 40)),
        "Напряжения" => ("U", egui::Color32::from_rgb(40, 80, 180)),
        "Мощности" => ("P", egui::Color32::from_rgb(160, 110, 20)),
        "Сопротивления" => ("Z", egui::Color32::from_rgb(110, 50, 140)),
        _ => ("Δ", egui::Color32::from_rgb(20, 120, 130)),
    };
    p.rect(r.shrink(1.0), 2.0f32, color.gamma_multiply(0.15f32), egui::Stroke::new(1.0f32, color));
    p.text(r.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(11.0 * crate::dpi::img(16.0) / 16.0), color);
}

/// Where the individually laid-out forms (501–542) put their fields, at
/// their templates' dialog units: the field, its label (x, y, w, h,
/// SS_RIGHT) and its control; a check box has no label (BS_LEFTTEXT).
type Place = (u16, Option<(i32, i32, i32, i32, bool)>, (i32, i32, i32, i32), bool);

fn form_places(dialog: u16) -> Option<&'static [Place]> {
    Some(match dialog {
        501 => &[
            (1057, Some((30, 23, 26, 8, false)), (60, 20, 135, 30), false),
            (1052, Some((30, 48, 30, 8, false)), (60, 47, 135, 12), false),
        ],
        502 => &[
            (1057, Some((30, 24, 26, 8, false)), (60, 22, 126, 30), false),
            (1052, Some((37, 49, 16, 8, false)), (60, 48, 126, 12), false),
            (1053, Some((25, 72, 27, 8, false)), (59, 72, 126, 12), false),
        ],
        503 => &[
            (1057, Some((30, 24, 26, 8, false)), (60, 22, 126, 30), false),
            (1052, Some((37, 49, 16, 8, false)), (60, 48, 126, 12), false),
            (1053, Some((33, 72, 20, 8, false)), (59, 72, 126, 12), false),
            (1050, Some((33, 96, 20, 8, false)), (59, 96, 126, 12), false),
        ],
        504 => &[
            (1057, Some((30, 24, 26, 8, false)), (60, 22, 126, 30), false),
            (1052, Some((37, 49, 16, 8, false)), (60, 48, 126, 12), false),
            (1053, Some((34, 73, 18, 8, false)), (59, 72, 126, 12), false),
        ],
        505 => &[
            (1007, Some((14, 22, 28, 8, true)), (49, 19, 135, 30), false),
            (1008, Some((16, 47, 26, 8, true)), (50, 44, 134, 30), false),
            (1009, Some((7, 74, 35, 8, true)), (50, 71, 134, 30), false),
            (2009, Some((6, 100, 36, 8, true)), (50, 98, 134, 30), false),
            (2175, None, (34, 125, 150, 8), true),
        ],
        506 | 507 => &[
            (1007, Some((14, 22, 28, 8, true)), (49, 19, 135, 30), false),
            (1008, Some((16, 47, 26, 8, true)), (50, 44, 134, 30), false),
            (1009, Some((7, 74, 35, 8, true)), (50, 71, 134, 30), false),
            (2009, Some((6, 99, 36, 8, true)), (50, 98, 134, 30), false),
            (2175, None, (34, 125, 150, 8), true),
        ],
        508 | 520 | 521 => &[
            (1007, Some((14, 22, 28, 8, true)), (49, 19, 135, 30), false),
            (1008, Some((16, 47, 26, 8, true)), (50, 44, 134, 30), false),
            (1009, Some((7, 74, 35, 8, true)), (50, 71, 134, 30), false),
            (2010, Some((6, 100, 36, 8, true)), (50, 98, 134, 30), false),
        ],
        522 | 524 => &[
            (2016, Some((19, 9, 10, 8, true)), (30, 6, 60, 30), false),
            (2015, Some((100, 9, 19, 8, true)), (120, 6, 60, 30), false),
            (2017, Some((19, 27, 10, 8, true)), (30, 24, 60, 30), false),
            (2018, Some((109, 26, 10, 8, true)), (120, 24, 60, 30), false),
            (2019, Some((19, 44, 10, 8, true)), (30, 42, 60, 30), false),
            (2020, Some((109, 44, 10, 8, true)), (120, 42, 60, 30), false),
            (2021, Some((20, 63, 10, 8, true)), (30, 60, 60, 30), false),
            (2022, Some((109, 62, 10, 8, true)), (120, 60, 60, 30), false),
            (2023, Some((19, 81, 10, 8, true)), (30, 78, 60, 30), false),
            (2024, Some((105, 81, 14, 8, true)), (120, 78, 60, 30), false),
            (2025, Some((16, 98, 14, 8, true)), (30, 96, 60, 30), false),
            (2026, Some((105, 98, 14, 8, true)), (120, 96, 60, 30), false),
            (2027, Some((16, 116, 14, 8, true)), (30, 114, 60, 30), false),
            (2028, Some((105, 116, 14, 8, true)), (120, 114, 60, 30), false),
            (2029, Some((15, 134, 14, 8, true)), (30, 132, 60, 30), false),
            (2030, Some((105, 135, 14, 8, true)), (120, 132, 60, 30), false),
            (2031, Some((16, 153, 14, 8, true)), (30, 150, 60, 30), false),
            (2032, Some((105, 152, 14, 8, true)), (120, 150, 60, 30), false),
            (2033, Some((15, 170, 14, 8, true)), (30, 168, 60, 30), false),
            (2034, Some((105, 170, 14, 8, true)), (120, 168, 60, 30), false),
            (2035, Some((15, 188, 14, 8, true)), (30, 186, 60, 30), false),
            (2036, Some((105, 188, 14, 8, true)), (120, 186, 60, 30), false),
            (2037, Some((15, 206, 14, 8, true)), (30, 204, 60, 30), false),
            (2038, Some((105, 207, 14, 8, true)), (120, 204, 60, 30), false),
        ],
        523 => &[
            (1007, Some((36, 25, 10, 8, true)), (48, 24, 138, 30), false),
            (1008, Some((36, 49, 10, 8, true)), (48, 48, 138, 30), false),
            (2041, Some((22, 74, 24, 8, true)), (48, 72, 138, 30), false),
        ],
        525 => &[
            (2041, Some((6, 8, 24, 8, true)), (32, 6, 138, 30), false),
            (2016, Some((16, 27, 10, 8, true)), (27, 24, 60, 30), false),
            (2015, Some((97, 27, 19, 8, true)), (117, 24, 60, 30), false),
            (2017, Some((16, 45, 10, 8, true)), (27, 42, 60, 30), false),
            (2018, Some((106, 44, 10, 8, true)), (117, 42, 60, 30), false),
            (2019, Some((16, 62, 10, 8, true)), (27, 60, 60, 30), false),
            (2020, Some((106, 62, 10, 8, true)), (117, 60, 60, 30), false),
            (2021, Some((17, 81, 10, 8, true)), (27, 78, 60, 30), false),
            (2022, Some((106, 80, 10, 8, true)), (117, 78, 60, 30), false),
            (2023, Some((16, 99, 10, 8, true)), (27, 96, 60, 30), false),
            (2024, Some((102, 99, 14, 8, true)), (117, 96, 60, 30), false),
            (2025, Some((13, 116, 14, 8, true)), (27, 114, 60, 30), false),
            (2026, Some((102, 116, 14, 8, true)), (117, 114, 60, 30), false),
            (2027, Some((13, 134, 14, 8, true)), (27, 132, 60, 30), false),
            (2028, Some((102, 134, 14, 8, true)), (117, 132, 60, 30), false),
            (2029, Some((12, 152, 14, 8, true)), (27, 150, 60, 30), false),
            (2030, Some((102, 153, 14, 8, true)), (117, 150, 60, 30), false),
            (2031, Some((13, 171, 14, 8, true)), (27, 168, 60, 30), false),
            (2032, Some((102, 170, 14, 8, true)), (117, 168, 60, 30), false),
            (2033, Some((12, 188, 14, 8, true)), (27, 186, 60, 30), false),
            (2034, Some((102, 188, 14, 8, true)), (117, 186, 60, 30), false),
            (2035, Some((12, 206, 14, 8, true)), (27, 204, 60, 30), false),
            (2036, Some((102, 206, 14, 8, true)), (117, 204, 60, 30), false),
            (2037, Some((12, 224, 14, 8, true)), (27, 222, 60, 30), false),
            (2038, Some((102, 225, 14, 8, true)), (117, 222, 60, 30), false),
        ],
        526 | 528 => &[
            (1007, Some((0, 22, 40, 8, true)), (46, 19, 150, 54), false),
            (1008, Some((0, 41, 40, 8, true)), (46, 39, 150, 54), false),
            (1043, Some((0, 62, 40, 8, true)), (46, 59, 150, 54), false),
            (2042, Some((8, 82, 31, 8, true)), (46, 80, 149, 14), false),
        ],
        527 => &[
            (1007, Some((1, 22, 40, 8, true)), (47, 19, 150, 54), false),
            (1008, Some((1, 41, 40, 8, true)), (47, 39, 150, 54), false),
            (1043, Some((1, 62, 40, 8, true)), (47, 59, 150, 54), false),
            (2042, Some((9, 82, 31, 8, true)), (47, 80, 149, 14), false),
        ],
        529..=531 => &[
            (1043, Some((7, 9, 33, 8, true)), (45, 7, 150, 54), false),
            (1007, Some((7, 29, 32, 8, true)), (45, 27, 150, 54), false),
            (1041, Some((6, 49, 33, 8, true)), (45, 47, 150, 54), false),
            (2043, Some((22, 70, 18, 8, true)), (45, 68, 150, 30), false),
            (1052, Some((7, 92, 33, 8, true)), (45, 90, 150, 12), false),
            (1053, Some((7, 112, 33, 8, true)), (45, 110, 150, 12), false),
            (2044, Some((9, 133, 31, 8, true)), (45, 130, 150, 14), false),
            (1050, Some((8, 154, 33, 8, true)), (46, 152, 150, 12), false),
            (1051, Some((8, 174, 33, 8, true)), (46, 172, 150, 12), false),
            (2045, Some((6, 194, 35, 8, true)), (46, 191, 150, 14), false),
            (2042, Some((10, 214, 31, 8, true)), (46, 212, 149, 14), false),
        ],
        532..=534 => &[
            (1010, Some((0, 20, 40, 8, true)), (45, 18, 150, 54), false),
            (1011, Some((0, 40, 40, 8, true)), (45, 38, 150, 54), false),
            (1062, Some((0, 60, 40, 8, true)), (45, 58, 150, 54), false),
            (1052, Some((16, 81, 24, 8, true)), (46, 79, 150, 12), false),
            (1053, Some((12, 99, 28, 8, true)), (46, 98, 149, 12), false),
        ],
        540..=542 => &[
            (1043, Some((7, 9, 33, 8, true)), (45, 7, 150, 54), false),
            (1007, Some((7, 29, 32, 8, true)), (45, 27, 150, 54), false),
            (1041, Some((6, 49, 33, 8, true)), (45, 47, 150, 54), false),
            (2043, Some((22, 70, 18, 8, true)), (45, 68, 150, 30), false),
            (2121, Some((7, 92, 33, 8, true)), (45, 90, 150, 12), false),
            (2044, Some((9, 115, 31, 8, true)), (45, 112, 150, 14), false),
            (2122, Some((8, 136, 33, 8, true)), (46, 134, 150, 12), false),
            (2045, Some((6, 159, 35, 8, true)), (46, 156, 150, 14), false),
            (2042, Some((10, 180, 31, 8, true)), (46, 178, 149, 14), false),
        ],
        _ => return None,
    })
}

/// A hosted form on its template, from the place of control 1035 (VM:
/// DIALOG 400's «Формула:» at 1347, 496): 400 its label at (0, 0) over
/// the edit at (0, 12) 191 × 12 DLU; 488 «Параметры отсутствуют» at (20,
/// 20); and 401–461, 490, 491 and 1069 — row k's label at (0, 20 + 20k)
/// 40 × 8, its control at (45, 18 + 20k) 150 DLU wide. Returns false for
/// the forms laid out otherwise (their own places; ours in `form_ui`).
fn template_form(ui: &mut egui::Ui, at: egui::Pos2, dialog: u16, st: &mut AddFormulaDlg) -> bool {
    use crate::dialogs::{dlu, win_combo_list, win_edit, win_label};
    let Some(form) = af::form(dialog) else { return true };
    let o = Rect::from_min_size(at, Vec2::ZERO);
    if dialog == 400 {
        win_label(ui, dlu(o, 0, 0, 32, 8), "Формула:", false, true);
        let text = st.values.entry(af::id::FORMULA).or_default();
        let r = win_edit(ui, dlu(o, 0, 12, 191, 12), egui::Id::new(("af_field", af::id::FORMULA)), text, false);
        st.form_focus |= r.has_focus() || r.lost_focus();
        return true;
    }
    if form.fields.is_empty() {
        win_label(ui, dlu(o, 20, 20, 100, 8), af::NO_PARAMS, false, true);
        return true;
    }
    let regular = matches!(dialog, 401..=461 | 490 | 491 | 1069);
    let places = if regular { None } else { form_places(dialog) };
    if !regular && places.is_none() {
        return false;
    }
    for (k, &(label, id)) in form.fields.iter().enumerate() {
        let kind = af::kind(id);
        let r = match places.and_then(|ps| ps.iter().find(|p| p.0 == id)) {
            // An individually laid-out form: its own places.
            Some(&(_, lab, (x, y, w, h), left)) => {
                if kind == Kind::Check {
                    let on = st.values.get(&id).is_some_and(|v| v == "1");
                    if crate::dialogs::win_check_ex(ui, dlu(o, x, y, w, h), label, on, true, left) {
                        st.values.insert(id, if on { "0" } else { "1" }.into());
                    }
                    continue;
                }
                if let Some((lx, ly, lw, lh, right)) = lab {
                    win_label(ui, dlu(o, lx, ly, lw, lh), label, right, true);
                }
                dlu(o, x, y, w, h.min(14))
            }
            None if places.is_some() => continue,
            // The grid of the regular forms.
            None => {
                let y = 20 * k as i32;
                win_label(ui, dlu(o, 0, 20 + y, 40, 8), label, false, true);
                dlu(o, 45, 18 + y, 150, 12)
            }
        };
        let items: Vec<String> = match kind {
            Kind::Channel { .. } => af::channel_items(kind, &st.channels).0,
            Kind::List(items, _) => items.iter().map(|s| s.to_string()).collect(),
            Kind::Harmonic => af::harmonics(st.sample_rate, st.line_freq),
            Kind::Edit(_) => {
                let text = st.values.entry(id).or_default();
                let resp = win_edit(ui, r, egui::Id::new(("af_field", id)), text, false);
                st.form_focus |= resp.has_focus() || resp.lost_focus();
                continue;
            }
            Kind::Check => continue,
        };
        let cur = st.values.entry(id).or_default();
        let at = items.iter().position(|it| it == cur).unwrap_or(0);
        if let Some(k) = win_combo_list(ui, r, egui::Id::new(("af_field", id)), &items, at, true) {
            *cur = items[k].clone();
        }
    }
    true
}

/// The hosted form: one row per control as in its dialog template.
fn form_ui(ui: &mut egui::Ui, dialog: u16, st: &mut AddFormulaDlg) {
    let Some(form) = af::form(dialog) else { return };
    if form.fields.is_empty() {
        ui.add_space(crate::dpi::dy(20.0));
        ui.label(af::NO_PARAMS);
        return;
    }
    // The 24-current forms lay their lists out in two columns.
    let pairs = form.fields.iter().any(|(_, i)| af::CURRENTS24.contains(i));
    egui::Grid::new(("af_grid", dialog)).num_columns(if pairs { 4 } else { 2 }).spacing([8.0, 6.0]).show(ui, |ui| {
        let mut col = 0;
        for &(label, id) in form.fields {
            let pair = pairs && af::CURRENTS24.contains(&id);
            let kind = af::kind(id);
            if kind == Kind::Check {
                let mut on = st.values.get(&id).is_some_and(|v| v == "1");
                ui.label("");
                let r = ui.checkbox(&mut on, label);
                st.form_focus |= r.has_focus() || r.lost_focus();
                if r.changed() {
                    st.values.insert(id, if on { "1" } else { "0" }.into());
                }
            } else {
                ui.label(label);
                field_ui(ui, id, kind, st, crate::dpi::dx(if pair { 90.0 } else { 200.0 }));
            }
            col += 1;
            if !pair || col % 2 == 0 {
                ui.end_row();
                col = 0;
            }
        }
    });
}

fn field_ui(ui: &mut egui::Ui, id: u16, kind: Kind, st: &mut AddFormulaDlg, width: f32) {
    let items: Vec<String> = match kind {
        Kind::Channel { .. } => af::channel_items(kind, &st.channels).0,
        Kind::List(items, _) => items.iter().map(|s| s.to_string()).collect(),
        Kind::Harmonic => af::harmonics(st.sample_rate, st.line_freq),
        Kind::Edit(_) => {
            let text = st.values.entry(id).or_default();
            let r = ui.add(egui::TextEdit::singleline(text).desired_width(width));
            st.form_focus |= r.has_focus() || r.lost_focus();
            return;
        }
        Kind::Check => return,
    };
    let cur = st.values.entry(id).or_default();
    let r = egui::ComboBox::from_id_salt(("af_field", id)).selected_text(cur.as_str()).width(width).show_ui(ui, |ui| {
        for it in &items {
            ui.selectable_value(cur, it.clone(), it);
        }
    });
    st.form_focus |= r.response.has_focus() || r.response.lost_focus();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Command;
    use eframe::egui::{Pos2, Vec2};

    struct Harness {
        ctx: egui::Context,
        app: App,
        t: f64,
    }

    impl Harness {
        fn new() -> Harness {
            let mut app = App::new();
            app.dispatch(Command::Demo);
            app.open_formula_dlg();
            let mut h = Harness { ctx: egui::Context::default(), app, t: 0.0 };
            for _ in 0..3 {
                h.frame(vec![]);
            }
            h
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            self.t += 0.05;
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                time: Some(self.t),
                events,
                ..Default::default()
            };
            let app = &mut self.app;
            let _ = self.ctx.run(raw, |ctx| app.formula_dlg_window(ctx));
        }

        fn click(&mut self, pos: Pos2) {
            let ev = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![egui::Event::PointerMoved(pos), ev(true)]);
            self.frame(vec![ev(false)]);
            // Idle past the double-click delay (0.3 s).
            for _ in 0..7 {
                self.frame(vec![]);
            }
        }

        fn dlg(&self) -> &AddFormulaDlg {
            self.app.formula_dlg.as_ref().expect("dialog open")
        }

        fn item(&self, path: &[&str]) -> Pos2 {
            self.dlg().item_rects.iter().find(|(p, _)| p == path).map(|(_, r)| r.center()).expect("item drawn")
        }

        fn ok(&mut self) {
            let p = self.dlg().ok_rect.expect("OK drawn").center();
            self.click(p);
        }
    }

    #[test]
    fn opens_on_the_free_formula() {
        let h = Harness::new();
        let d = h.dlg();
        assert_eq!(d.name, "Канал1");
        assert_eq!(d.form, Some(400));
        assert_eq!(d.values[&af::id::FORMULA], "Ia + Ib");
        assert_eq!(d.channels.len(), 6);
    }

    /// Selecting «Токи ▸ Ток 3Io» hosts DIALOG 402; OK appends a row to the
    /// formula page (no channel until «Рассчитать»).
    #[test]
    fn builds_a_template_channel() {
        let mut h = Harness::new();
        let p = h.item(&["Токи", "Ток 3Io"]);
        h.click(p);
        assert_eq!(h.dlg().form, Some(402));
        h.app.formula_dlg.as_mut().unwrap().name = "I0x3".into();
        h.ok();
        assert!(h.app.formula_dlg.is_none(), "closed");
        let p = &h.app.formula_page;
        assert_eq!(p.rows.last().map(|r| (r.0.as_str(), r.1.as_str())), Some(("I0x3", "3I0: (Ia, Ib, Ic)")));
        assert_eq!(p.selected, Some(p.rows.len() - 1), "the new row is selected");
        assert!(h.app.active_doc().unwrap().computed.is_empty(), "nothing computed yet");
    }

    /// A name already among the page's rows is taken (`0x4033a0`'s list
    /// at `+0x9c`; VM: a second «X1» → 473, the dialog stays).
    #[test]
    fn a_rows_name_is_taken() {
        let mut h = Harness::new();
        h.app.formula_dlg.as_mut().unwrap().name = "X1".into();
        h.ok();
        assert!(h.app.formula_dlg.is_none());
        h.app.open_formula_dlg();
        for _ in 0..3 {
            h.frame(vec![]);
        }
        h.app.formula_dlg.as_mut().unwrap().name = "X1".into();
        h.ok();
        assert_eq!(h.dlg().alerts, vec![(af::MSG_NAME_TAKEN, MsgIcon::Error)]);
        assert_eq!(h.app.formula_page.rows.iter().filter(|r| r.0 == "X1").count(), 1);
    }

    /// A group hosts no form and greys OK.
    #[test]
    fn group_disables_ok() {
        let mut h = Harness::new();
        let p = h.item(&["Токи"]);
        h.click(p);
        assert_eq!(h.dlg().form, None);
        h.ok();
        assert!(h.app.formula_dlg.is_some());
        assert!(h.app.active_doc().unwrap().computed.is_empty());
    }

    /// An empty name or one starting with a digit → 396 twice (DDX, then
    /// `CDialog::OnOK`'s second DDX) while the stale member passes; a record
    /// channel's name → 473; a bad text over a taken member → 396 then 473.
    /// The dialog stays every time.
    #[test]
    fn name_is_checked() {
        let (bad, taken) = ((af::MSG_BAD_NAME, MsgIcon::Exclamation), (af::MSG_NAME_TAKEN, MsgIcon::Error));
        let mut h = Harness::new();
        for (name, msgs) in [("", vec![bad, bad]), ("2x", vec![bad, bad]), ("Ua", vec![taken]), ("", vec![bad, taken])] {
            h.app.formula_dlg.as_mut().unwrap().name = name.into();
            h.ok();
            assert_eq!(h.dlg().alerts, msgs, "{name:?}");
            for _ in &msgs {
                h.frame(vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }]);
                for _ in 0..3 {
                    h.frame(vec![]);
                }
            }
            assert!(h.dlg().alerts.is_empty());
            for _ in 0..7 {
                h.frame(vec![]);
            }
        }
        assert!(h.app.active_doc().unwrap().computed.is_empty());
    }

    /// Enter in a form field (`0x401c40`): the parent's OnOK, then the
    /// dialog ends with IDOK anyway — a taken name leaves the formula
    /// member empty, a bad name keeps the proposal and builds the formula.
    #[test]
    fn enter_in_the_form_always_closes() {
        let mut h = Harness::new();
        let d = h.app.formula_dlg.as_mut().unwrap();
        d.name = "Ua".into();
        let (name, formula) = d.accept_from_form();
        assert_eq!((name.as_str(), formula.as_str()), ("Ua", ""));
        assert_eq!(d.alerts, vec![(af::MSG_NAME_TAKEN, MsgIcon::Error)]);
        let mut d = AddFormulaDlg::new("Канал1".into(), vec!["Ua".into()], vec!["Ua".into()], 1000.0, 50.0);
        d.name = "2x".into();
        let (name, formula) = d.accept_from_form();
        assert_eq!(name, "Канал1");
        assert!(!formula.is_empty());
        assert_eq!(d.alerts.len(), 2);
    }

    #[test]
    fn folding_hides_children() {
        let mut h = Harness::new();
        assert!(h.dlg().item_rects.iter().any(|(p, _)| p == &["Токи", "Ток Ib"]));
        let row = h.dlg().item_rects.iter().find(|(p, _)| p == &["Токи"]).unwrap().1;
        // The ± box is centred on the row's line, 11 px left of its image.
        h.click(row.left_center() - Vec2::new(11.0, 0.0));
        assert!(!h.dlg().item_rects.iter().any(|(p, _)| p == &["Токи", "Ток Ib"]));
    }
}
