//! The Office-2007-style ribbon: quick-access toolbar, tab strip and command
//! groups. Layout mirrors the original (QAT: open/save/print/undo/redo; tabs
//! Главная / ОМП; groups Объединение · Диаграммы · Видимость · Вычисления ·
//! Осциллограмма; ОМП tab: Параметры · Анализ). See `notes/UI_SPEC.md`.

use crate::app::{App, Command, RibbonTab, ValueMode, View};
use crate::theme::Skin;
use eframe::egui::{self, Color32, RichText};

impl App {
    pub fn ribbon_ui(&mut self, ui: &mut egui::Ui) {
        let p = self.skin.palette();
        let mut cmds: Vec<Command> = Vec::new();

        // Gradient band behind the quick-access toolbar + tab strip (the ribbon
        // body below is the lighter `tab_active` fill).
        let full = ui.max_rect();
        let band = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), 52.0));
        paint_v_gradient(ui, band, p.ribbon_top, p.ribbon_bottom);

        // ---- Row 1: quick-access toolbar + title + style/help ----
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            draw_logo(ui, 18.0, p.accent);
            ui.add_space(2.0);
            egui::menu::menu_button(ui, RichText::new("Файл ▾").color(p.text).strong(), |ui| {
                if ui.button("Открыть COMTRADE (.cfg/.dat)…   Ctrl+O").clicked() {
                    cmds.push(Command::Open);
                    ui.close_menu();
                }
                if ui.button("Демо-осциллограмма (авария A-0)").clicked() {
                    cmds.push(Command::Demo);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Экспорт каналов в CSV…").clicked() {
                    cmds.push(Command::ExportCsv);
                    ui.close_menu();
                }
                if ui.button("Открыть библиотеку характеристик .fvb…").clicked() {
                    cmds.push(Command::OpenFvb);
                    ui.close_menu();
                }
            });
            ui.add_space(4.0);
            for (glyph, tip, cmd) in [
                ("Откр", "Открыть  (Ctrl+O)", Command::Open),
                ("Сохр", "Сохранить  (Ctrl+S)", Command::Save),
                ("Печ", "Печать  (Ctrl+P)", Command::Print),
                ("←", "Отмена  (Ctrl+Z)", Command::Undo),
                ("→", "Возврат  (Ctrl+Y)", Command::Redo),
            ] {
                if qat_button(ui, glyph).on_hover_text(tip).clicked() {
                    cmds.push(cmd);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Help
                if ui
                    .add(egui::Button::new(RichText::new("?").color(p.accent).strong()).frame(false))
                    .on_hover_text("О программе Fast View 5")
                    .clicked()
                {
                    self.status =
                        "Fast View 5 — просмотр и анализ осциллограмм COMTRADE. © НТЦ «Механотроника»."
                            .into();
                }
                // Style switcher (MENU 200)
                egui::ComboBox::from_id_salt("skincombo")
                    .selected_text(RichText::new(format!("Стиль: {}", skin_short(self.skin))))
                    .show_ui(ui, |ui| {
                        for s in Skin::ALL {
                            if ui
                                .selectable_label(self.skin == s, s.label())
                                .clicked()
                            {
                                cmds.push(Command::SetSkin(s));
                            }
                        }
                    });
                ui.label(RichText::new("Fast View 5 — Механотроника").color(p.text).weak());
            });
        });

        // ---- Row 2: tab strip ----
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            if ribbon_tab(ui, "Главная", self.tab == RibbonTab::Home, p.tab_active, p.text).clicked() {
                self.tab = RibbonTab::Home;
            }
            if ribbon_tab(ui, "ОМП", self.tab == RibbonTab::Omp, p.tab_active, p.text).clicked() {
                self.tab = RibbonTab::Omp;
            }
        });

        // ---- Row 3: ribbon body (fixed height) ----
        egui::Frame::none()
            .fill(p.tab_active)
            .stroke(egui::Stroke::new(1.0f32, p.group_border))
            .inner_margin(egui::Margin::symmetric(6.0, 4.0))
            .show(ui, |ui| {
                ui.set_min_height(86.0);
                ui.horizontal_top(|ui| match self.tab {
                    RibbonTab::Home => self.ribbon_home(ui, &mut cmds),
                    RibbonTab::Omp => self.ribbon_omp(ui, &mut cmds),
                });
            });

        for c in cmds {
            self.dispatch(c);
        }
    }

    fn ribbon_home(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<Command>) {
        let p = self.skin.palette();
        // Объединение
        ribbon_group(ui, p, "Объединение", |ui| {
            if big_button(ui, p, "▣", "Вставить").clicked() {
                cmds.push(Command::Paste);
            }
            ui.vertical(|ui| {
                if small_button(ui, p, "Копировать").clicked() {
                    cmds.push(Command::Copy);
                }
                if small_button(ui, p, "← Отмена").clicked() {
                    cmds.push(Command::Undo);
                }
                if small_button(ui, p, "→ Возврат").clicked() {
                    cmds.push(Command::Redo);
                }
            });
        });
        // Диаграммы
        ribbon_group(ui, p, "Диаграммы", |ui| {
            for (glyph, label, v) in [
                ("∑", "Частотная", View::Frequency),
                ("→", "Векторная", View::Vector),
                ("○", "Круговая", View::Circle),
                ("±", "Диффе-\nренциальная", View::Differential),
            ] {
                if big_button_sel(ui, p, glyph, label, self.view == v).clicked() {
                    cmds.push(Command::Show(v));
                }
            }
        });
        // Видимость
        ribbon_group(ui, p, "Видимость", |ui| {
            if big_button(ui, p, "≡", "Каналы").clicked() {
                cmds.push(Command::Show(View::Oscillogram));
            }
            ui.vertical(|ui| {
                if small_button(ui, p, "Показать все").clicked() {
                    cmds.push(Command::ShowAllChannels(true));
                }
                if small_button(ui, p, "Скрыть все").clicked() {
                    cmds.push(Command::ShowAllChannels(false));
                }
                if small_button(ui, p, "Вся осцил.").clicked() {
                    cmds.push(Command::FitTime);
                }
            });
        });
        // Вычисления
        ribbon_group(ui, p, "Вычисления", |ui| {
            ui.vertical(|ui| {
                if small_toggle(ui, p, "Мгновенные знач.", self.value_mode == ValueMode::Instant) {
                    cmds.push(Command::SetValueMode(ValueMode::Instant));
                }
                if small_toggle(ui, p, "Среднеквадр. (СКЗ)", self.value_mode == ValueMode::Rms) {
                    cmds.push(Command::SetValueMode(ValueMode::Rms));
                }
                if small_toggle(ui, p, "Первая гармоника", self.value_mode == ValueMode::FirstHarmonic) {
                    cmds.push(Command::SetValueMode(ValueMode::FirstHarmonic));
                }
            });
            ui.vertical(|ui| {
                if small_button(ui, p, "Инвертировать").clicked() {
                    cmds.push(Command::InvertVisible);
                }
                if small_button(ui, p, "Удал. постоянную").clicked() {
                    cmds.push(Command::RemoveDcVisible);
                }
                if small_toggle(ui, p, "Первичные знач.", self.primary_values) {
                    cmds.push(Command::SetPrimaryValues(!self.primary_values));
                }
            });
            if big_button_sel(ui, p, "≈", "Быстрый\nанализ", self.view == View::Analysis).clicked() {
                cmds.push(Command::Show(View::Analysis));
            }
        });
        // Осциллограмма
        ribbon_group(ui, p, "Осциллограмма", |ui| {
            if big_button_sel(ui, p, "~", "Осцилло-\nграмма", self.view == View::Oscillogram).clicked() {
                cmds.push(Command::Show(View::Oscillogram));
            }
        });
    }

    fn ribbon_omp(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<Command>) {
        let p = self.skin.palette();
        ribbon_group(ui, p, "ОМП", |ui| {
            if big_button(ui, p, "◆", "Параметры").clicked() {
                cmds.push(Command::Show(View::Omp));
            }
            if big_button_sel(ui, p, "▲", "Анализ", self.view == View::Omp).clicked() {
                cmds.push(Command::Show(View::Omp));
            }
        });
    }
}

// ---------------- drawing helpers ----------------

fn skin_short(s: Skin) -> &'static str {
    match s {
        Skin::Blue => "Голубой",
        Skin::Black => "Чёрный",
        Skin::Silver => "Серебристый",
        Skin::Aqua => "Зелёно-голубой",
    }
}

fn paint_v_gradient(ui: &egui::Ui, rect: egui::Rect, top: Color32, bottom: Color32) {
    let painter = ui.painter();
    let steps = 24;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let y0 = rect.top() + rect.height() * t0;
        let y1 = rect.top() + rect.height() * t1;
        let c = lerp_color(top, bottom, (t0 + t1) * 0.5);
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(rect.left(), y0), egui::pos2(rect.right(), y1)),
             0.0f32,
            c,
        );
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// The Механотроника chevron mark (two stacked carets).
fn draw_logo(ui: &mut egui::Ui, h: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(h * 1.2, h), egui::Sense::hover());
    let painter = ui.painter();
    let green = Color32::from_rgb(0x4C, 0xAF, 0x50);
    for (yoff, col) in [(0.0_f32, color), (h * 0.36, green)] {
        let a = egui::pos2(rect.left(), rect.bottom() - yoff - h * 0.1);
        let b = egui::pos2(rect.center().x, rect.top() + yoff + h * 0.05);
        let c = egui::pos2(rect.right(), rect.bottom() - yoff - h * 0.1);
        painter.line_segment([a, b], egui::Stroke::new(2.2f32, col));
        painter.line_segment([b, c], egui::Stroke::new(2.2f32, col));
    }
}

fn qat_button(ui: &mut egui::Ui, glyph: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(glyph).size(11.5)).min_size(egui::vec2(0.0, 22.0)))
}

fn ribbon_tab(ui: &mut egui::Ui, label: &str, active: bool, active_bg: Color32, text: Color32) -> egui::Response {
    let fill = if active { active_bg } else { Color32::TRANSPARENT };
    let btn = egui::Button::new(RichText::new(label).color(text).strong())
        .fill(fill)
        .stroke(if active {
            egui::Stroke::new(1.0f32, text.gamma_multiply(0.3f32))
        } else {
            egui::Stroke::NONE
        })
        .rounding(egui::Rounding {
            nw: 4.0,
            ne: 4.0,
            sw: 0.0,
            se: 0.0,
        })
        .min_size(egui::vec2(72.0, 22.0));
    ui.add(btn)
}

/// A ribbon group: content row/area + centered caption + right divider.
fn ribbon_group<R>(ui: &mut egui::Ui, p: crate::theme::Palette, title: &str, content: impl FnOnce(&mut egui::Ui) -> R) {
    ui.scope(|ui| {
        ui.vertical(|ui| {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        content(ui);
                    });
                });
            ui.add_space(1.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.min_rect().width().max(40.0), 14.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.label(RichText::new(title).small().color(p.text.gamma_multiply(0.8f32)));
                },
            );
        });
    });
    // right divider
    let r = ui.min_rect();
    ui.painter().line_segment(
        [egui::pos2(r.right() + 2.0, r.top() + 4.0), egui::pos2(r.right() + 2.0, r.bottom() - 2.0)],
        egui::Stroke::new(1.0f32, p.group_border),
    );
    ui.add_space(6.0);
}

fn big_button(ui: &mut egui::Ui, p: crate::theme::Palette, glyph: &str, label: &str) -> egui::Response {
    big_button_sel(ui, p, glyph, label, false)
}

fn big_button_sel(ui: &mut egui::Ui, p: crate::theme::Palette, glyph: &str, label: &str, selected: bool) -> egui::Response {
    let size = egui::vec2(58.0, 64.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();
    let hovered = resp.hovered();
    if selected || hovered {
        let fill = if selected { p.accent_soft } else { p.accent_soft.gamma_multiply(0.5f32) };
        painter.rect(rect, 3.0f32, fill, egui::Stroke::new(1.0f32, p.accent_border));
    }
    painter.text(
        egui::pos2(rect.center().x, rect.top() + 20.0),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(22.0),
        p.accent,
    );
    for (i, line) in label.split('\n').enumerate() {
        painter.text(
            egui::pos2(rect.center().x, rect.top() + 40.0 + i as f32 * 11.0),
            egui::Align2::CENTER_CENTER,
            line,
            egui::FontId::proportional(10.5),
            p.text,
        );
    }
    resp
}

fn small_button(ui: &mut egui::Ui, p: crate::theme::Palette, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(11.0).color(p.text))
            .frame(true)
            .fill(Color32::TRANSPARENT)
            .min_size(egui::vec2(118.0, 19.0)),
    )
}

/// A small latching toggle (shows a check when on).
fn small_toggle(ui: &mut egui::Ui, p: crate::theme::Palette, label: &str, on: bool) -> bool {
    let txt = if on {
        format!("✓ {label}")
    } else {
        format!("   {label}")
    };
    let fill = if on { p.accent_soft } else { Color32::TRANSPARENT };
    ui.add(
        egui::Button::new(RichText::new(txt).size(11.0).color(p.text))
            .fill(fill)
            .min_size(egui::vec2(140.0, 19.0)),
    )
    .clicked()
}
