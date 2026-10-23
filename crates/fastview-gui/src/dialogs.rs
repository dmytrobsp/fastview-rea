//! Modal-ish dialog windows: About, channel properties, and the comment editor.

use crate::app::App;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

/// A transparent full-screen layer at `order` that swallows the pointer
/// below a modal window (the windows of a higher order stay live).
pub fn modal_backdrop(ctx: &egui::Context, id: &str, order: egui::Order) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new(id)).order(order).fixed_pos(screen.min).show(ctx, |ui| {
        ui.allocate_exact_size(screen.size(), egui::Sense::click_and_drag());
    });
}

/// The icon of a message box: `AfxMessageBox` adds the exclamation to
/// MB_OK / MB_OKCANCEL and the question to MB_YESNO(CANCEL) when the call
/// gives none; MB_ICONHAND (0x10) is the error mark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MsgIcon {
    #[default]
    Exclamation,
    Error,
    Question,
}

/// A message box's body: its icon left of the text (our drawing of the
/// system icons).
pub fn message_body(ui: &mut egui::Ui, icon: MsgIcon, text: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::hover());
        let p = ui.painter();
        let c = r.center();
        let (mark, bg) = match icon {
            MsgIcon::Exclamation => {
                let tri = vec![Pos2::new(c.x, r.top() + 2.0), Pos2::new(r.right() - 1.0, r.bottom() - 3.0), Pos2::new(r.left() + 1.0, r.bottom() - 3.0)];
                p.add(egui::Shape::convex_polygon(tri, Color32::from_rgb(252, 204, 34), Stroke::new(1.0f32, Color32::from_rgb(190, 140, 0))));
                ("!", None)
            }
            MsgIcon::Error => ("✕", Some(Color32::from_rgb(214, 40, 40))),
            MsgIcon::Question => ("?", Some(Color32::from_rgb(40, 100, 200))),
        };
        if let Some(bg) = bg {
            p.circle_filled(c, 15.0, bg);
        }
        let color = if bg.is_some() { Color32::WHITE } else { Color32::BLACK };
        let at = if bg.is_some() { c } else { c + Vec2::new(0.0, 4.0) };
        p.text(at, egui::Align2::CENTER_CENTER, mark, egui::FontId::proportional(20.0), color);
        ui.add_space(8.0);
        ui.label(text);
    });
}

/// Whether a dialog window (egui's `Order::Middle` layer, where every
/// `egui::Window` of ours lives or puts its backdrop) was shown last frame.
pub fn window_up(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Middle))
}

impl App {
    pub fn dialogs_ui(&mut self, ctx: &egui::Context) {
        self.about_window(ctx);
        self.windows_dialog(ctx);
        self.channel_props_window(ctx);
        self.comment_window(ctx);
        self.channel_map_window(ctx);
        self.color_window(ctx);
        self.finder_window(ctx);
        self.loading_window(ctx);
        self.omp_windows(ctx);
        self.record_dialogs_ui(ctx);
        self.quick_sheet_window(ctx);
        self.formula_dlg_window(ctx);
        self.save_prompt_window(ctx);
    }

    /// «Загрузка» (DIALOG 492): an indeterminate progress bar per large file
    /// being read, and «Отмена», which abandons that open.
    fn loading_window(&mut self, ctx: &egui::Context) {
        self.poll_loading();
        if self.loading.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        let now = ctx.input(|i| i.time);
        let mut cancel = None;
        // DIALOG 492: a progress bar 0..100 and «Отмена», no close box, no
        // text; the bar is set every 500 ms (`0x4717d0`, `0x471870`).
        for (i, l) in self.loading.iter_mut().enumerate().filter(|(_, l)| l.big) {
            if now - l.shown.1 >= 0.5 {
                l.shown = (l.progress.percent(), now);
            }
            egui::Window::new("Загрузка осциллограммы").id(egui::Id::new(("fv_loading", i))).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::ProgressBar::new(l.shown.0 as f32 / 100.0).desired_width(240.0));
                    if ui.button("Отмена").clicked() {
                        cancel = Some(i);
                    }
                });
            });
        }
        // «Отмена» sets the flag the readers poll; the open is abandoned.
        if let Some(i) = cancel {
            self.loading[i].progress.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
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

    /// Channel-role assignment — **our extension**, not a dialog of the
    /// original (whose «Отображение каналов» is the visibility grid, DIALOG
    /// 463, `chandlg`): pick which analog channel plays Ua/Ub/Uc and Ia/Ib/Ic
    /// for our analyses, diagrams and ОМП. Pre-filled from the names.
    fn channel_map_window(&mut self, ctx: &egui::Context) {
        if !self.show_channel_map {
            return;
        }
        let mut open = true;
        egui::Window::new("Роли каналов Ua…Ic (дополнение FastView-REA)")
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
                            // Eligibility by unit as CMDUI 32850 / 32851 (`0x430650`).
                            ui.add_enabled(crate::diagram::vector_unit_ok(unit.trim()), egui::Checkbox::new(&mut cv.on_vector, "вект."));
                            ui.add_enabled(crate::diagram::circle_flag_unit_ok(unit.trim()), egui::Checkbox::new(&mut cv.on_circle, "круг."));
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

/// MFC 12's DDX_Text of an int (`0x101e0e79`): up to 64 characters of the
/// window text through `_snwscanf_s` "%d" — leading blanks and a sign,
/// then at least one digit; anything after the number is ignored ("50.5"
/// → 50, "12 Гц" → 12); `None` = «Введите целое число.» (61712).
pub fn ddx_int(s: &str) -> Option<i32> {
    let s: String = s.chars().take(63).collect();
    let t = s.trim_start();
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits: Vec<u32> = rest.chars().map_while(|c| c.to_digit(10)).collect();
    if digits.is_empty() {
        return None;
    }
    // Overflow wraps (INFERRED from the CRT's conversion).
    let v = digits.iter().fold(0i32, |a, &d| a.wrapping_mul(10).wrapping_add(d as i32));
    Some(if neg { v.wrapping_neg() } else { v })
}

#[cfg(test)]
mod ddx_tests {
    #[test]
    fn ddx_int_as_scanf() {
        use super::ddx_int;
        assert_eq!(ddx_int(" 50.5"), Some(50));
        assert_eq!(ddx_int("12 Гц"), Some(12));
        assert_eq!(ddx_int("-7x"), Some(-7));
        assert_eq!(ddx_int("0x10"), Some(0));
        assert_eq!((ddx_int(""), ddx_int("-"), ddx_int("abc")), (None, None, None));
    }
}
