//! Modal-ish dialog windows: About, channel properties, and the comment editor.

use crate::app::{App, Comment};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

impl App {
    pub fn dialogs_ui(&mut self, ctx: &egui::Context) {
        self.about_window(ctx);
        self.channel_props_window(ctx);
        self.comment_window(ctx);
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        egui::Window::new("О программе")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(64.0, 64.0), egui::Sense::hover());
                    chevron(ui.painter(), rect);
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.heading("Fast View 5");
                        ui.label("Программа для просмотра и анализа осциллограмм\nв формате COMTRADE");
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Кроссплатформенная реконструкция (Rust / egui)").weak());
                        ui.label(egui::RichText::new("© НТЦ «Механотроника» — mtrele.ru").weak());
                    });
                });
                ui.separator();
                ui.label(
                    egui::RichText::new(
                        "Независимая совместимая реализация, созданная по результатам \
                         поведенческого анализа. Оригинальный код не используется.",
                    )
                    .small()
                    .weak(),
                );
            });
        self.show_about = open;
    }

    fn channel_props_window(&mut self, ctx: &egui::Context) {
        let Some(i) = self.props_channel else {
            return;
        };
        let mut open = true;
        let mut recolor_default = false;
        let mut close_btn = false;
        egui::Window::new("Свойства канала")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let Some(doc) = self.active.and_then(|a| self.docs.get_mut(a)) else {
                    ui.label("Нет активной осциллограммы.");
                    return;
                };
                let Some(ch) = doc.osc.analog.get(i) else {
                    ui.label("Канал недоступен.");
                    return;
                };
                let (id, unit, phase) = (ch.id.clone(), ch.unit.clone(), ch.phase.clone());
                egui::Grid::new("chprops").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label("Имя:");
                    ui.label(egui::RichText::new(&id).strong());
                    ui.end_row();
                    ui.label("Фаза:");
                    ui.label(if phase.is_empty() { "—" } else { &phase });
                    ui.end_row();
                    ui.label("Единица:");
                    ui.label(if unit.is_empty() { "—" } else { &unit });
                    ui.end_row();
                    if let Some(cv) = doc.chans.get_mut(i) {
                        ui.label("Цвет:");
                        ui.color_edit_button_srgba(&mut cv.color);
                        ui.end_row();
                        ui.label("Инвертировать:");
                        ui.checkbox(&mut cv.invert, "");
                        ui.end_row();
                        ui.label("Удалить постоянную:");
                        ui.checkbox(&mut cv.remove_dc, "");
                        ui.end_row();
                        ui.label("Видимый:");
                        ui.checkbox(&mut cv.visible, "");
                        ui.end_row();
                        ui.label("На диаграммах:");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut cv.on_freq, "частотн.");
                            ui.checkbox(&mut cv.on_vector, "вект.");
                            ui.checkbox(&mut cv.on_circle, "круг.");
                        });
                        ui.end_row();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Цвет по умолчанию").clicked() {
                        recolor_default = true;
                    }
                    if ui.button("Закрыть").clicked() {
                        close_btn = true;
                    }
                });
            });
        if recolor_default {
            if let Some(doc) = self.active.and_then(|a| self.docs.get_mut(a)) {
                if let (Some(ch), Some(cv)) = (doc.osc.analog.get(i), doc.chans.get_mut(i)) {
                    cv.color = crate::app::default_channel_color(ch, i);
                }
            }
        }
        if !open || close_btn {
            self.props_channel = None;
        }
    }

    fn comment_window(&mut self, ctx: &egui::Context) {
        let Some(mut ed) = self.comment_edit.take() else {
            return;
        };
        let mut keep_open = true;
        let mut action: Option<CommentAction> = None;
        let t = self
            .active_doc()
            .and_then(|d| d.osc.time.get(ed.sample).copied())
            .unwrap_or(0.0);
        egui::Window::new("Комментарий")
            .collapsible(false)
            .resizable(false)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label(format!("От начала осциллограммы: {:.4} с (отсчёт {})", t, ed.sample));
                ui.add(egui::TextEdit::multiline(&mut ed.text).desired_rows(3).desired_width(260.0));
                ui.horizontal(|ui| {
                    if ui.button("Сохранить").clicked() {
                        action = Some(CommentAction::Save);
                    }
                    if ed.idx.is_some() && ui.button("Удалить").clicked() {
                        action = Some(CommentAction::Delete);
                    }
                    if ui.button("Отмена").clicked() {
                        action = Some(CommentAction::Cancel);
                    }
                });
            });
        match action {
            Some(CommentAction::Save) => {
                if let Some(doc) = self.active.and_then(|a| self.docs.get_mut(a)) {
                    match ed.idx {
                        Some(idx) if idx < doc.comments.len() => {
                            doc.comments[idx].text = ed.text.clone();
                        }
                        _ => {
                            if !ed.text.trim().is_empty() {
                                doc.comments.push(Comment { sample: ed.sample, text: ed.text.clone() });
                            }
                        }
                    }
                }
            }
            Some(CommentAction::Delete) => {
                if let (Some(idx), Some(doc)) = (ed.idx, self.active.and_then(|a| self.docs.get_mut(a))) {
                    if idx < doc.comments.len() {
                        doc.comments.remove(idx);
                    }
                }
            }
            Some(CommentAction::Cancel) => {}
            None => {
                if keep_open {
                    // still editing — keep the dialog state for next frame
                    self.comment_edit = Some(ed);
                }
            }
        }
    }
}

enum CommentAction {
    Save,
    Delete,
    Cancel,
}

/// Draw the Механотроника chevron mark in `rect`.
fn chevron(painter: &egui::Painter, rect: Rect) {
    let h = rect.height();
    let blue = Color32::from_rgb(0x2E, 0x5B, 0x9E);
    let green = Color32::from_rgb(0x4C, 0xAF, 0x50);
    for (yoff, col) in [(0.0_f32, blue), (h * 0.34, green)] {
        let a = Pos2::new(rect.left(), rect.bottom() - yoff - h * 0.12);
        let b = Pos2::new(rect.center().x, rect.top() + yoff + h * 0.06);
        let c = Pos2::new(rect.right(), rect.bottom() - yoff - h * 0.12);
        let w = h * 0.12;
        painter.line_segment([a, b], Stroke::new(w, col));
        painter.line_segment([b, c], Stroke::new(w, col));
    }
}
