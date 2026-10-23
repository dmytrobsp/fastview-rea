//! «Добавить новую формулу» (DIALOG 359, `AddFormulaDlg`), opened by
//! «Добавить» of the formula editor (`CQAnFormulaEditorPg`, `0x486df0`):
//! the template tree on the left, «Название канала:» and the selected
//! template's form on the right. Selecting a tree item re-creates the form
//! with its initial values (`0x403280`); a group shows no form and greys OK.
//! OK checks the name as the original's sequence does (396 / 473, the
//! dialog stays) and appends the formula the form builds (`fvb_core::addformula::build`) as a computed channel.

use crate::dialogs::MsgIcon;
use crate::app::App;
use eframe::egui::{self, Rect, Vec2};
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
        let form = self.form?;
        let taken: Vec<&str> = self.taken.iter().map(String::as_str).collect();
        let ddx_ok = af::ddx_name_ok(&self.name);
        if ddx_ok {
            self.member = self.name.clone();
        } else {
            self.alerts.push((af::MSG_BAD_NAME, MsgIcon::Exclamation));
        }
        if let Err(e) = af::check_name(&self.member, &taken) {
            self.alerts.push((e.message(), MsgIcon::Error));
            return None;
        }
        if !ddx_ok {
            self.alerts.push((af::MSG_BAD_NAME, MsgIcon::Exclamation));
            return None;
        }
        let f = af::build(form, &self.values)?;
        Some((self.member.clone(), f))
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
        let taken = channels.iter().cloned().chain(d.osc.digital.iter().map(|c| c.id.clone())).collect();
        let name = af::default_name(&rows);
        self.formula_dlg = Some(AddFormulaDlg::new(name, channels, taken, d.osc.sample_rate, d.osc.line_freq));
    }

    pub fn formula_dlg_window(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.formula_dlg.take() else { return };
        let mut done: Option<bool> = None;
        let mut open = true;
        let modal = st.alerts.is_empty();
        st.item_rects.clear();
        egui::Window::new("Добавить новую формулу")
            .id(egui::Id::new("fv_add_formula"))
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(650.0, 480.0))
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_enabled_ui(modal, |ui| {
                    ui.horizontal_top(|ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.set_width(290.0);
                            egui::ScrollArea::vertical().id_salt("af_tree").max_height(420.0).show(ui, |ui| {
                                ui.set_min_height(420.0);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    let mut pick = None;
                                    tree_ui(ui, af::TREE, &mut Vec::new(), &mut st, &mut pick);
                                    if let Some((path, form)) = pick {
                                        st.select(path, form);
                                    }
                                });
                            });
                        });
                        ui.vertical(|ui| {
                            ui.set_width(330.0);
                            ui.horizontal(|ui| {
                                ui.label("Название канала:");
                                ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(200.0));
                            });
                            ui.add_space(8.0);
                            egui::ScrollArea::vertical().id_salt("af_form").max_height(392.0).show(ui, |ui| {
                                ui.set_min_height(392.0);
                                if let Some(d) = st.form {
                                    form_ui(ui, d, &mut st);
                                }
                            });
                        });
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_sized([72.0, 22.0], egui::Button::new("Отмена")).clicked() {
                            done = Some(false);
                        }
                        let ok = ui.add_enabled_ui(st.form.is_some(), |ui| ui.add_sized([72.0, 22.0], egui::Button::new("OK")));
                        st.ok_rect = Some(ok.inner.rect);
                        if ok.inner.clicked() {
                            done = Some(true);
                        }
                    });
                });
            });
        if let Some(&(msg, icon)) = st.alerts.first() {
            let mut close = false;
            crate::dialogs::modal_backdrop(ctx, "fv_add_formula_alert_block", egui::Order::Middle);
            egui::Window::new(crate::app::APP_TITLE)
                .id(egui::Id::new("fv_add_formula_alert"))
                .collapsible(false)
                .resizable(false)
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    crate::dialogs::message_body(ui, icon, msg);
                    ui.vertical_centered(|ui| close = ui.button("    OK    ").clicked());
                });
            if close || ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                st.alerts.remove(0);
            }
        } else if modal {
            // Enter in the form is the parent's OK (`0x401c40`); Esc cancels.
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && st.form.is_some() {
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

/// The tree rows: a fold toggle and the label for a group, the label for a
/// leaf; clicking a label selects the item.
fn tree_ui(ui: &mut egui::Ui, nodes: &[Node], path: &mut Path, st: &mut AddFormulaDlg, pick: &mut Option<(Path, Option<u16>)>) {
    for n in nodes {
        let (label, kids, form) = match *n {
            Node::Group(l, k) => (l, Some(k), None),
            Node::Leaf(l, d) => (l, None, Some(d)),
        };
        path.push(label);
        let folded = st.collapsed.contains(path);
        let row = ui.horizontal(|ui| {
            ui.add_space(16.0 * (path.len() - 1) as f32);
            match kids {
                Some(_) => {
                    let sign = if folded { "+" } else { "−" };
                    if ui.add(egui::Button::new(sign).small().min_size(Vec2::new(14.0, 14.0))).clicked() {
                        if folded {
                            st.collapsed.remove(path);
                        } else {
                            st.collapsed.insert(path.clone());
                        }
                    }
                }
                None => ui.add_space(18.0),
            }
            let r = ui.selectable_label(st.selected == *path, label);
            if r.clicked() {
                *pick = Some((path.clone(), form));
            }
            r.rect
        });
        st.item_rects.push((path.clone(), row.inner));
        if let (Some(k), false) = (kids, folded) {
            tree_ui(ui, k, path, st, pick);
        }
        path.pop();
    }
}

/// The hosted form: one row per control as in its dialog template.
fn form_ui(ui: &mut egui::Ui, dialog: u16, st: &mut AddFormulaDlg) {
    let Some(form) = af::form(dialog) else { return };
    if form.fields.is_empty() {
        ui.add_space(20.0);
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
                if ui.checkbox(&mut on, label).changed() {
                    st.values.insert(id, if on { "1" } else { "0" }.into());
                }
            } else {
                ui.label(label);
                field_ui(ui, id, kind, st, if pair { 90.0 } else { 200.0 });
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
            ui.add(egui::TextEdit::singleline(text).desired_width(width));
            return;
        }
        Kind::Check => return,
    };
    let cur = st.values.entry(id).or_default();
    egui::ComboBox::from_id_salt(("af_field", id)).selected_text(cur.as_str()).width(width).show_ui(ui, |ui| {
        for it in &items {
            ui.selectable_value(cur, it.clone(), it);
        }
    });
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

    #[test]
    fn folding_hides_children() {
        let mut h = Harness::new();
        assert!(h.dlg().item_rects.iter().any(|(p, _)| p == &["Токи", "Ток Ib"]));
        let row = h.dlg().item_rects.iter().find(|(p, _)| p == &["Токи"]).unwrap().1;
        h.click(row.left_center() - Vec2::new(12.0, 0.0));
        assert!(!h.dlg().item_rects.iter().any(|(p, _)| p == &["Токи", "Ток Ib"]));
    }
}
