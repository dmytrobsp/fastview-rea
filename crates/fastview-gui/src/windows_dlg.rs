//! MFC's «Окна» dialog (`CMFCWindowsManagerDialog`, template 16927 of
//! mfc120rus), opened by «Окна...» at the end of the tabs' ▾ list: the main
//! frame calls `EnableWindowsDialog(131, 132 «&Окна...», always, no help)`
//! (`0x47296b`). Read in the carved mfc120u (L9.64): OnInitDialog
//! `0x101aa7c7`, the list fill `0x101aaafc`, the selection `0x101aaa4e`, the
//! button states `0x101aa897` and the handlers of its message map.

use crate::app::App;
use eframe::egui;
use std::collections::BTreeSet;

/// The dialog's state; documents are kept by serial, as indices shift
/// when one closes.
#[derive(Debug, Default)]
pub struct WindowsDlg {
    pub sel: BTreeSet<u64>,
    anchor: Option<u64>,
    /// «Закрыть окна» under way: the windows still to close, in order.
    pub closing: Vec<u64>,
    /// Refill and reselect once the closing is done.
    refresh: bool,
}

/// The enabled buttons for `n` selected windows (`0x101aa897`); every
/// window of ours can be closed and saved.
#[derive(Debug, PartialEq, Eq)]
pub struct Buttons {
    pub activate: bool,
    pub save: bool,
    pub close: bool,
    pub arrange: bool,
    pub minimize: bool,
}

pub fn buttons(n: usize) -> Buttons {
    Buttons { activate: n == 1, save: n > 0, close: n > 0, arrange: n >= 2, minimize: n > 0 }
}

/// A click on a row of the extended-selection list (LBS_EXTENDEDSEL):
/// alone, Ctrl toggles, Shift takes the range from the anchor.
fn click(dlg: &mut WindowsDlg, rows: &[u64], at: u64, ctrl: bool, shift: bool) {
    let pos = |s: u64| rows.iter().position(|&r| r == s);
    match (shift, dlg.anchor.and_then(pos), pos(at)) {
        (true, Some(a), Some(b)) => {
            if !ctrl {
                dlg.sel.clear();
            }
            dlg.sel.extend(&rows[a.min(b)..=a.max(b)]);
        }
        _ if ctrl => {
            if !dlg.sel.remove(&at) {
                dlg.sel.insert(at);
            }
            dlg.anchor = Some(at);
        }
        _ => {
            dlg.sel = [at].into();
            dlg.anchor = Some(at);
        }
    }
}

impl App {
    pub fn open_windows_dialog(&mut self) {
        let mut dlg = WindowsDlg::default();
        self.select_active_window(&mut dlg);
        self.windows_dlg = Some(dlg);
    }

    /// `0x101aaa4e`: everything deselected, then the active window.
    fn select_active_window(&self, dlg: &mut WindowsDlg) {
        dlg.sel = self.active.and_then(|a| self.docs.get(a)).map(|d| d.serial).into_iter().collect();
    }

    /// The rows: every document's title, sorted as LBS_SORT does (ignoring
    /// case).
    pub fn window_rows(&self) -> Vec<(u64, String)> {
        let mut rows: Vec<(u64, String)> = self.docs.iter().map(|d| (d.serial, d.title())).collect();
        rows.sort_by_key(|(_, t)| t.to_lowercase());
        rows
    }

    fn doc_by_serial(&self, s: u64) -> Option<usize> {
        self.docs.iter().position(|d| d.serial == s)
    }

    pub fn windows_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.windows_dlg.take() else { return };
        // `0x101aa666`: WM_CLOSE to each selected window, the last row
        // first; each runs its own save question, «Отмена» keeps that one
        // and the next follows.
        if self.setfile.prompt.is_none() {
            while !dlg.closing.is_empty() {
                let s = dlg.closing.remove(0);
                if let Some(i) = self.doc_by_serial(s) {
                    self.request_close(i);
                    if self.setfile.prompt.is_some() {
                        break;
                    }
                }
            }
            if dlg.closing.is_empty() && dlg.refresh {
                dlg.refresh = false;
                self.select_active_window(&mut dlg);
            }
        }
        let rows = self.window_rows();
        dlg.sel.retain(|s| rows.iter().any(|(r, _)| r == s));
        let b = buttons(dlg.sel.len());
        let (ctrl, shift) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift));
        let mut open = true;
        let mut done = false;
        let mut activate = false;
        let mut save = false;
        let mut close = false;
        crate::dialogs::modal_backdrop(ctx, "fv_windows_block", egui::Order::Middle);
        egui::Window::new("Окна")
            .id(egui::Id::new("fv_windows"))
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .interactable(self.setfile.prompt.is_none())
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Выбрать окно:");
                        egui::Frame::none().stroke(egui::Stroke::new(1.0f32, egui::Color32::from_gray(130))).show(ui, |ui| {
                            ui.set_min_size(egui::vec2(330.0, 234.0));
                            ui.set_max_width(330.0);
                            egui::ScrollArea::vertical().max_height(234.0).show(ui, |ui| {
                                let order: Vec<u64> = rows.iter().map(|r| r.0).collect();
                                for (s, title) in &rows {
                                    // A list-box row: the full width, the
                                    // system highlight when selected.
                                    let (rect, r) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), egui::Sense::click());
                                    let on = dlg.sel.contains(s);
                                    if on {
                                        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(0, 120, 215));
                                    }
                                    let color = if on { egui::Color32::WHITE } else { egui::Color32::BLACK };
                                    ui.painter().text(rect.left_center() + egui::vec2(3.0, 0.0), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(13.0), color);
                                    if r.clicked() {
                                        click(&mut dlg, &order, *s, ctrl, shift);
                                    }
                                    // LBN_DBLCLK → Активировать.
                                    if r.double_clicked() {
                                        dlg.sel = [*s].into();
                                        activate = true;
                                    }
                                }
                            });
                        });
                    });
                    ui.vertical(|ui| {
                        let size = egui::vec2(120.0, 22.0);
                        let button = |ui: &mut egui::Ui, on: bool, text: &str| ui.add_enabled(on, egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend).min_size(size)).clicked();
                        activate |= button(ui, b.activate, "Активировать");
                        done |= button(ui, true, "ОК");
                        save |= button(ui, b.save, "Сохранить");
                        close |= button(ui, b.close, "Закрыть окна");
                        // The MDI arrangements: kept enabled as read, but our
                        // tabbed view has nothing to arrange.
                        button(ui, b.arrange, "Каскадом");
                        button(ui, b.arrange, "Горизонтально");
                        button(ui, b.arrange, "Вертикально");
                        button(ui, b.minimize, "Свернуть");
                    });
                });
            });
        // Активировать is the default button.
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && b.activate && self.setfile.prompt.is_none() {
            activate = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.setfile.prompt.is_none() {
            open = false;
        }
        // `0x101aa495`: the one selected window becomes active, the dialog
        // ends (IDOK).
        if activate && dlg.sel.len() == 1 {
            if let Some(i) = dlg.sel.first().and_then(|&s| self.doc_by_serial(s)) {
                self.dispatch(crate::app::Command::ActivateDoc(i));
            }
            return;
        }
        // `0x101aa5aa`: each selected window's document saved, in list
        // order; then the list is refilled and the active one selected.
        if save {
            let prev = self.active;
            for (s, _) in rows.iter().filter(|(s, _)| dlg.sel.contains(s)) {
                if let Some(i) = self.doc_by_serial(*s) {
                    self.active = Some(i);
                    self.save_active();
                }
            }
            self.active = prev;
            self.select_active_window(&mut dlg);
        }
        if close {
            dlg.closing = rows.iter().rev().filter(|(s, _)| dlg.sel.contains(s)).map(|r| r.0).collect();
            dlg.refresh = true;
        }
        if open && !done {
            self.windows_dlg = Some(dlg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(app: &mut App, ctx: &egui::Context) {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.windows_dialog(ctx));
    }

    /// «Закрыть окна» closes the selected windows one by one (a modified
    /// one asks first and waits for the answer), then the active one is
    /// selected; opening selects the active window.
    #[test]
    fn close_windows_one_by_one() {
        let ctx = egui::Context::default();
        let mut app = App::new();
        app.dispatch(crate::app::Command::Demo);
        let osc = fvb_core::comtrade::parse(crate::app::DEMO_CFG, crate::app::DEMO_DAT).unwrap();
        app.add_doc(crate::app::Doc::new(osc.clone(), "Б".into()));
        app.add_doc(crate::app::Doc::new(osc, "А".into()));
        app.open_windows_dialog();
        let serials: Vec<u64> = app.docs.iter().map(|d| d.serial).collect();
        assert_eq!(app.windows_dlg.as_ref().unwrap().sel, [serials[2]].into(), "the active window");
        assert_eq!(app.window_rows().iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), ["demo_fault", "А", "Б"]);
        app.docs[1].set.modified = true;
        {
            let dlg = app.windows_dlg.as_mut().unwrap();
            dlg.closing = vec![serials[1], serials[2]];
            dlg.refresh = true;
        }
        frame(&mut app, &ctx);
        assert!(app.setfile.prompt.is_some(), "«Б» was modified: its question first");
        assert_eq!(app.docs.len(), 3);
        app.answer_save_prompt(&ctx, Some(false));
        frame(&mut app, &ctx);
        assert_eq!(app.docs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["demo_fault"]);
        let dlg = app.windows_dlg.as_ref().unwrap();
        assert!(dlg.closing.is_empty());
        assert_eq!(dlg.sel, [serials[0]].into(), "the remaining active window");
    }

    #[test]
    fn extended_selection_and_button_states() {
        let rows = [10, 20, 30, 40];
        let mut d = WindowsDlg::default();
        click(&mut d, &rows, 20, false, false);
        click(&mut d, &rows, 40, false, true);
        assert_eq!(d.sel, [20, 30, 40].into());
        click(&mut d, &rows, 30, true, false);
        assert_eq!(d.sel, [20, 40].into());
        click(&mut d, &rows, 10, false, false);
        assert_eq!(d.sel, [10].into());
        assert_eq!(buttons(1), Buttons { activate: true, save: true, close: true, arrange: false, minimize: true });
        assert_eq!(buttons(2), Buttons { activate: false, save: true, close: true, arrange: true, minimize: true });
        assert_eq!(buttons(0), Buttons { activate: false, save: false, close: false, arrange: false, minimize: false });
    }
}
