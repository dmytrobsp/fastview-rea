//! Modal-ish dialog windows: About, channel properties, and the comment editor.

use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

impl App {
    pub fn dialogs_ui(&mut self, ctx: &egui::Context) {
        self.about_window(ctx);
        self.channel_props_window(ctx);
        self.comment_window(ctx);
        self.file_params_window(ctx);
        self.channel_map_window(ctx);
        self.settings_window(ctx);
        self.color_window(ctx);
        self.finder_window(ctx);
        self.loading_window(ctx);
        self.omp_windows(ctx);
    }

    /// «Загрузка» (DIALOG 492): an indeterminate progress bar per large file
    /// being read, and «Отмена», which abandons that open.
    fn loading_window(&mut self, ctx: &egui::Context) {
        self.poll_loading();
        if self.loading.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        let mut cancel = None;
        for (i, l) in self.loading.iter().enumerate().filter(|(_, l)| l.big) {
            egui::Window::new("Загрузка").id(egui::Id::new(("fv_loading", i))).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(l.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
                ui.add(egui::ProgressBar::new(0.0).animate(true).desired_width(260.0));
                if ui.button("Отмена").clicked() {
                    cancel = Some(i);
                }
            });
        }
        if let Some(i) = cancel {
            self.loading.remove(i);
        }
    }

    /// «Цвет канала» (33030): a colour dialog started from the first selected
    /// channel's colour; OK gives the colour to every selected channel.
    fn color_window(&mut self, ctx: &egui::Context) {
        let Some(mut col) = self.color_dialog else { return };
        let mut open = true;
        let mut done: Option<bool> = None;
        egui::Window::new("Цвет")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::color_picker::color_picker_color32(ui, &mut col, egui::color_picker::Alpha::Opaque);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        done = Some(true);
                    }
                    if ui.button("Отмена").clicked() {
                        done = Some(false);
                    }
                });
            });
        self.color_dialog = match done {
            Some(true) => {
                if let Some(d) = self.active_doc_mut() {
                    d.set_color_selected(col);
                }
                None
            }
            Some(false) => None,
            None => open.then_some(col),
        };
    }

    /// Program settings (DIALOG 510): the `.cfg` text encoding, re-opening the
    /// active record when it changes.
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        use fvb_core::comtrade::Encoding;
        let mut open = true;
        let mut reopen = false;
        let mut close_btn = false;
        egui::Window::new("Установка настроек программы")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Кодировка текста файла описания (.cfg):");
                for (enc, label) in [
                    (Encoding::Auto, "Автоопределение (UTF-8 / Windows-1251)"),
                    (Encoding::Utf8, "UTF-8"),
                    (Encoding::Cp1251, "Windows-1251"),
                    (Encoding::Cp866, "IBM866 (DOS)"),
                    (Encoding::Koi8r, "KOI8-R"),
                ] {
                    ui.radio_value(&mut self.encoding, enc, label);
                }
                ui.label(
                    egui::RichText::new(
                        "Внимание! При смене кодировки изменения вступают в силу после \
                         переоткрытия файла осциллограммы.",
                    )
                    .small()
                    .weak(),
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.active_doc().and_then(|d| d.path.as_ref()).is_some(), egui::Button::new("Переоткрыть активную")).clicked() {
                        reopen = true;
                    }
                    if ui.button("Закрыть").clicked() {
                        close_btn = true;
                    }
                });
            });
        if reopen {
            self.reopen_active();
        }
        self.show_settings = open && !close_btn;
    }

    /// Explicit channel-to-role assignment (the original's DIALOG 536 / «Отображение
    /// каналов»): pick which analog channel plays Ua/Ub/Uc and Ia/Ib/Ic for every
    /// analysis and diagram. Pre-filled from name-based auto-detection.
    fn channel_map_window(&mut self, ctx: &egui::Context) {
        if !self.show_channel_map {
            return;
        }
        let mut open = true;
        egui::Window::new("Отображение каналов — назначение Ua/Ia…")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let Some(d) = self.active.and_then(|a| self.docs.get_mut(a)) else {
                    ui.label("Нет активной осциллограммы.");
                    return;
                };
                let ids: Vec<(usize, String)> = d
                    .osc
                    .analog
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, format!("{} [{}]", c.id, c.unit)))
                    .collect();
                ui.label("Токи");
                let calc_ib = d.analysis.calc_ib;
                egui::Grid::new("imap").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    role_combo(ui, "Ia:", &mut d.analysis.i_abc[0], &ids, true);
                    ui.end_row();
                    role_combo(ui, "Ib:", &mut d.analysis.i_abc[1], &ids, !calc_ib);
                    ui.end_row();
                    role_combo(ui, "Ic:", &mut d.analysis.i_abc[2], &ids, true);
                    ui.end_row();
                });
                ui.checkbox(&mut d.analysis.calc_ib, "Рассчитать Ib по Ia и Iс (сети с изолированной нейтралью)");
                ui.separator();
                ui.label("Напряжения (фазные)");
                egui::Grid::new("umap").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    role_combo(ui, "Ua:", &mut d.analysis.u_abc[0], &ids, true);
                    ui.end_row();
                    role_combo(ui, "Ub:", &mut d.analysis.u_abc[1], &ids, true);
                    ui.end_row();
                    role_combo(ui, "Uc:", &mut d.analysis.u_abc[2], &ids, true);
                    ui.end_row();
                });
                ui.separator();
                ui.label(
                    egui::RichText::new(
                        "Назначение используется во всех расчётах: симметричные составляющие, \
                         сопротивления, векторная/круговая диаграммы, ОМП.",
                    )
                    .small()
                    .weak(),
                );
            });
        self.show_channel_map = open;
    }

    fn file_params_window(&mut self, ctx: &egui::Context) {
        if !self.show_file_params {
            return;
        }
        let mut open = true;
        egui::Window::new("Параметры файла")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let Some(d) = self.active_doc() else {
                    ui.label("Нет активной осциллограммы.");
                    return;
                };
                let o = &d.osc;
                let dur = o.time.last().copied().unwrap_or(0.0) - o.time.first().copied().unwrap_or(0.0);
                egui::Grid::new("fileparams").num_columns(2).spacing([12.0, 5.0]).show(ui, |ui| {
                    let row = |ui: &mut egui::Ui, k: &str, v: String| {
                        ui.label(k);
                        ui.label(egui::RichText::new(v).strong());
                        ui.end_row();
                    };
                    row(ui, "Имя:", d.name.clone());
                    row(ui, "Станция:", if o.station.is_empty() { "—".into() } else { o.station.clone() });
                    row(ui, "Устройство:", if o.device.is_empty() { "—".into() } else { o.device.clone() });
                    row(ui, "Ревизия COMTRADE:", format!("{}", o.rev_year));
                    row(ui, "Частота сети:", format!("{} Гц", o.line_freq));
                    row(ui, "Частота дискретизации:", format!("{:.0} Гц", o.sample_rate));
                    row(ui, "Отсчётов за период:", format!("{}", o.samples_per_cycle()));
                    row(ui, "Аналоговых каналов:", format!("{}", o.analog.len()));
                    row(ui, "Дискретных каналов:", format!("{}", o.digital.len()));
                    row(ui, "Число отсчётов:", format!("{}", o.sample_count()));
                    row(ui, "Длительность:", format!("{:.4} с", dur));
                });
            });
        self.show_file_params = open;
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
}

/// A labelled channel-role combo bound to an `Option<usize>` slot.
fn role_combo(ui: &mut egui::Ui, label: &str, slot: &mut Option<usize>, ids: &[(usize, String)], enabled: bool) {
    ui.label(label);
    let cur = slot
        .and_then(|i| ids.iter().find(|(j, _)| *j == i))
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| "— нет —".to_string());
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(label).selected_text(cur).width(170.0).show_ui(ui, |ui| {
            ui.selectable_value(slot, None, "— нет —");
            for (i, n) in ids {
                ui.selectable_value(slot, Some(*i), n);
            }
        });
    });
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
