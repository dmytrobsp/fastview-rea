//! The Office-2007-style ribbon: quick-access toolbar, tab strip and command
//! groups. Layout mirrors the original (QAT: open/save/print/undo/redo; tabs
//! Главная / ОМП; groups Объединение · Диаграммы · Видимость · Вычисления ·
//! Осциллограмма; ОМП tab: Параметры · Анализ — a toggle with the result bar,
//! `omp_ui.rs`). See `notes/UI_SPEC.md`.

use crate::app::{App, Command, Pane, RibbonTab, ValueMode, View};
use crate::icons::{self, Icon};
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
            // Application menu (0x472f30): the original's ten items, the
            // «Последние документы» list, and our own extras in a labelled
            // submenu.
            let has_doc = self.active_doc().is_some();
            let mru = self.mru.clone();
            let sub_id = app_sub_id();
            egui::menu::menu_button(ui, RichText::new("Файл ▾").color(p.text).strong(), |ui| {
                // Main panel (Office 2007): hovering an item with sub-items
                // shows them in the right column in place of the recent
                // documents. Fixed column widths: a separator takes all the
                // width it is offered, which would stretch the menu.
                // Kept only while the menu stays open (stamped each pass;
                // egui returns no inner result while a menu stays open).
                let frame = ui.ctx().cumulative_pass_nr();
                let mut sub = ui.data(|d| d.get_temp::<(Option<AppSub>, u64)>(sub_id)).filter(|s| s.1 + 1 == frame).and_then(|s| s.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(220.0);
                        let close_cmd = self.active.map(Command::CloseDoc);
                        let items: [(&str, bool, Option<Command>, Option<AppSub>); 10] = [
                            ("Открыть...", true, Some(Command::Open), None),
                            ("Сохранить", has_doc, Some(Command::Save), None),
                            ("Сохранить как...", has_doc, Some(Command::SaveAs), None),
                            ("Проводник", true, Some(Command::ToggleExplorer), None),
                            // The main part prints (57607).
                            ("Печать  ▸", has_doc, Some(Command::Print), Some(AppSub::Print)),
                            ("Параметры файла...", has_doc, Some(Command::FileParams), None),
                            ("Комментарии", self.hdr_comment_enabled(), Some(Command::HdrComment), None),
                            ("Загрузка настроек...", has_doc, Some(Command::LoadSettings), None),
                            ("Настройки", true, Some(Command::Settings), None),
                            ("Закрыть", close_cmd.is_some(), close_cmd, None),
                        ];
                        // A separator between every item (each is its own
                        // group in the original's main panel).
                        for (k, (label, enabled, cmd, opens)) in items.into_iter().chain([("Дополнительно  ▸", true, None, Some(AppSub::Extras))]).enumerate() {
                            if k > 0 {
                                ui.separator();
                            }
                            let r = ui.add_enabled(enabled || opens.is_some(), egui::Button::new(label).frame(false).selected(opens.is_some() && sub == opens));
                            if r.hovered() {
                                sub = opens;
                            }
                            if r.clicked() && enabled {
                                if let Some(cmd) = cmd {
                                    cmds.push(cmd);
                                    ui.close_menu();
                                }
                            }
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        match sub {
                            Some(AppSub::Print) => {
                                ui.label(RichText::new("Предварительный просмотр и печать документа").strong());
                                ui.separator();
                                for (label, desc, enabled, cmd) in [
                                    ("Параметры страницы", "Изменение параметров страницы", true, Command::PageSetup),
                                    ("Предварительный просмотр", "Просмотр документа в полностраничном режиме", has_doc, Command::PrintPreview),
                                    ("Печать", "Печать активного документа", has_doc, Command::Print),
                                ] {
                                    if sub_entry(ui, label, desc, enabled).clicked() {
                                        cmds.push(cmd);
                                        ui.close_menu();
                                    }
                                }
                            }
                            Some(AppSub::Extras) => {
                                ui.label(RichText::new("Дополнительно (FastView-REA)").strong());
                                ui.separator();
                                for (label, cmd) in [
                                    ("Демо-осциллограмма", Command::Demo),
                                    ("Экспорт каналов в CSV…", Command::ExportCsv),
                                    ("Архив осциллограммы (.zip)…", Command::Archive),
                                    ("Библиотека характеристик .fvb…", Command::OpenFvb),
                                    ("Роли каналов Ua…Ic…", Command::ChannelMap),
                                    ("Экспорт изображения окна (PNG)…", Command::ExportImage),
                                ] {
                                    if ui.add(egui::Button::new(label).frame(false)).clicked() {
                                        cmds.push(cmd);
                                        ui.close_menu();
                                    }
                                }
                            }
                            None => {
                                ui.label(RichText::new("Последние документы").strong());
                                ui.separator();
                                for (k, path) in mru.iter().enumerate() {
                                    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                                    let b = egui::Button::new(format!("{} {name}", k + 1)).frame(false).truncate();
                                    if ui.add(b).on_hover_text(path.display().to_string()).clicked() {
                                        cmds.push(Command::OpenPath(path.clone()));
                                        ui.close_menu();
                                    }
                                }
                            }
                        }
                    });
                });
                ui.data_mut(|d| d.insert_temp(sub_id, (sub, frame)));
            });
            ui.add_space(4.0);
            // Quick-access toolbar: Open, Save, Print only.
            for (icon, tip, cmd) in [
                (Icon::Open, "Открыть  (Ctrl+O)", Command::Open),
                (Icon::Save, "Сохранить", Command::Save),
                (Icon::Print, "Печать  (Ctrl+P)", Command::Print),
            ] {
                if qat_button(ui, p, icon).on_hover_text(tip).clicked() {
                    cmds.push(cmd);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Help split button (57667): the arrow opens {Помощь, О программе...}.
                ui.menu_button(RichText::new("?").color(p.text).strong(), |ui| {
                    ui.add_enabled(false, egui::Button::new("Помощь"));
                    ui.separator();
                    if ui.button("О программе...").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
                // «Стиль» (menu 200): the radio entry of the current look,
                // persisted as `ApplicationLook`.
                ui.menu_button(RichText::new("Стиль ▾").color(p.text), |ui| {
                    for (s, name) in Skin::ALL.into_iter().zip(["Windows 2000", "Office XP", "Windows XP", "Office 2003", "Visual Studio.NET 2005"]) {
                        if ui.radio(self.skin == s, name).clicked() {
                            cmds.push(Command::SetSkin(s));
                            ui.close_menu();
                        }
                    }
                    ui.menu_button("Office 2007", |ui| {
                        for (s, name) in Skin::ALL[5..].iter().copied().zip(["Голубой стиль", "Черный стиль", "Серебристый стиль", "Зеленовато-голубой стиль"]) {
                            if ui.radio(self.skin == s, name).clicked() {
                                cmds.push(Command::SetSkin(s));
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
        });

        // ---- Row 2: tab strip ----
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            for (label, tab) in [("Главная", RibbonTab::Home), ("ОМП", RibbonTab::Omp)] {
                let r = ribbon_tab(ui, label, self.tab == tab, p.tab_active, p.text);
                if r.double_clicked() {
                    cmds.push(Command::ToggleRibbonMin);
                } else if r.clicked() {
                    self.tab = tab;
                }
                // Right-click on the ribbon: one item, minimise (checked when minimised).
                let min = self.ribbon_minimized;
                r.context_menu(|ui| {
                    if ui.checkbox(&mut { min }, "Свернуть ленту").clicked() {
                        cmds.push(Command::ToggleRibbonMin);
                        ui.close_menu();
                    }
                });
            }
        });

        // ---- Row 3: ribbon body (fixed height; hidden when minimised) ----
        if !self.ribbon_minimized {
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
        }

        for c in cmds {
            self.dispatch(c);
        }
    }

    fn ribbon_home(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<Command>) {
        let p = self.skin.palette();
        // Объединение
        ribbon_group(ui, p, "Объединение", |ui| {
            // 57637 needs the clipboard and no pending paste; 57634 a selected
            // file channel; 57643 / 57644 the undo / redo counts.
            let can_paste = self.clip.available() && !self.active_doc().is_some_and(crate::omp_ui::has_pending_paste);
            let can_copy = self.active_doc().is_some_and(|d| d.selection.iter().any(|k| !matches!(k, crate::app::ChanKey::Computed(_))));
            let (undo, redo) = self.active_doc().map_or((false, false), |d| (!d.undo.is_empty(), !d.redo.is_empty()));
            if ui.add_enabled_ui(can_paste, |ui| big_button(ui, p, Icon::Paste, "Вставить")).inner.clicked() {
                cmds.push(Command::Paste);
            }
            ui.vertical(|ui| {
                for (icon, label, on, cmd) in [
                    (Icon::Copy, "Копировать", can_copy, Command::Copy),
                    (Icon::Undo, "Отмена", undo, Command::Undo),
                    (Icon::Redo, "Возврат", redo, Command::Redo),
                ] {
                    if ui.add_enabled_ui(on, |ui| small_button(ui, p, icon, label)).inner.clicked() {
                        cmds.push(cmd);
                    }
                }
            });
        });
        // Диаграммы
        ribbon_group(ui, p, "Диаграммы", |ui| {
            // 32811 / 32809 / 32808 / 140 toggle the panes; checked while shown.
            for (icon, label, pane) in [
                (Icon::Freq, "Частотная", Pane::Frequency),
                (Icon::Vector, "Векторная", Pane::Vector),
                (Icon::Circle, "Круговая", Pane::Circle),
                (Icon::Diff, "Диффе-\nренциальная", Pane::Differential),
            ] {
                if big_button_sel(ui, p, icon, label, self.panes.visible(pane)).clicked() {
                    cmds.push(Command::TogglePane(pane));
                }
            }
        });
        // Видимость
        ribbon_group(ui, p, "Видимость", |ui| {
            // 32862: DIALOG 463 «Отображение каналов».
            if big_button(ui, p, Icon::Channels, "Каналы").clicked() {
                cmds.push(Command::ChannelsDialog);
            }
            // Columns as the original: [32845/46/47] [32802/03/138]
            // [32800/01/141] [32813/14/139] [32815/16] [155/157/156].
            ui.vertical(|ui| {
                for (k, icon, label) in [(0u8, Icon::ShowAll, "Показать дискретные"), (1, Icon::HideAll, "Скрыть дискретные"), (2, Icon::HideAll, "Скрыть неизменяющиеся")] {
                    if small_button(ui, p, icon, label).clicked() {
                        cmds.push(Command::Discrete(k));
                    }
                }
            });
            let armed = self.rect_zoom != crate::app::RectZoom::Off;
            for column in [
                vec![("Увел. по гор.", Command::ZoomX(true), false), ("Уменьш. по гор.", Command::ZoomX(false), false), ("Приблизить по вертикали", Command::RectZoom, armed)],
                vec![("Увел. по верт.", Command::ZoomY(true), false), ("Уменьш. по верт.", Command::ZoomY(false), false), ("Вернуть к исходному размеру", Command::ResetZoom, false)],
            ] {
                ui.vertical(|ui| {
                    for (label, c, on) in column {
                        if small_toggle(ui, p, label, on) {
                            cmds.push(c);
                        }
                    }
                });
            }
            let shown = |k: usize| self.active_doc().is_some_and(|d| if k < 3 { d.v_shown[k] } else { d.hviz[k - 3].shown });
            for ks in [[0usize, 1, 2].as_slice(), [3, 4].as_slice()] {
                ui.vertical(|ui| {
                    for &k in ks {
                        let label = ["Верт. визир 1", "Верт. визир 2", "Верт. визир 3", "Гор. визир 1", "Гор. визир 2"][k];
                        if small_toggle(ui, p, label, shown(k)) {
                            cmds.push(Command::ToggleVizir(k));
                        }
                    }
                });
            }
            ui.vertical(|ui| {
                // 155 / 157 / 156: one-shot comment tools (a radio group with
                // toggle-off), enabled for records from .cfg / .osc files.
                ui.add_enabled_ui(self.active_doc().is_some_and(|d| d.comments_allowed()), |ui| {
                    for (k, label) in [(1, "Момент времени"), (3, "Область"), (2, "Линия уровня")] {
                        let on = self.active_doc().is_some_and(|d| d.comment_mode == k);
                        if small_toggle(ui, p, label, on) {
                            cmds.push(Command::CommentMode(k));
                        }
                    }
                });
            });
        });
        // Вычисления
        ribbon_group(ui, p, "Вычисления", |ui| {
            // 32817 / 32818 / 32819: stats rows on every analog channel,
            // checked when all channels show the row.
            ui.vertical(|ui| {
                for (k, label) in ["Мин/макс", "СКЗ", "Оценка 1-й гарм."].into_iter().enumerate() {
                    let on = self.active_doc().is_some_and(|d| {
                        !d.chans.is_empty() && d.chans.iter().all(|c| [c.stats.minmax, c.stats.rms, c.stats.a1][k])
                    });
                    if small_toggle(ui, p, label, on) {
                        cmds.push(Command::StatsRow(k));
                    }
                }
            });
            ui.vertical(|ui| {
                // 32820..32822 act on the selected channels; a button is checked
                // only when every selected channel is in that mode.
                let checked = |m| self.active_doc().is_some_and(|d| d.mode_state(m).checked);
                if small_toggle(ui, p, "Мгновенные", checked(ValueMode::Instant)) {
                    cmds.push(Command::SetValueMode(ValueMode::Instant));
                }
                if small_toggle(ui, p, "Среднеквадр.", checked(ValueMode::Rms)) {
                    cmds.push(Command::SetValueMode(ValueMode::Rms));
                }
                if small_toggle(ui, p, "1-я гармоника", checked(ValueMode::FirstHarmonic)) {
                    cmds.push(Command::SetValueMode(ValueMode::FirstHarmonic));
                }
            });
            ui.vertical(|ui| {
                // 32823 / 32824: toggle-all over the selection, checked when
                // every selected channel has the flag.
                use crate::chansel::ChanFlag;
                let st = |f| self.active_doc().map(|d| d.flag_state(f)).unwrap_or(crate::chansel::ItemState { enabled: false, checked: false });
                for (icon, label, f) in [(Icon::Invert, "Инвертировать", ChanFlag::Invert), (Icon::RemoveDc, "Удал. постоянную", ChanFlag::RemoveDc)] {
                    let s = st(f);
                    if ui.add_enabled_ui(s.enabled, |ui| small_button_sel(ui, p, icon, label, s.checked)).inner.clicked() {
                        cmds.push(Command::ToggleFlag(f));
                    }
                }
            });
            ui.vertical(|ui| {
                // 32866 «Коэффициенты»; 32867 / 32868 a radio pair checked
                // from channel 0 (CMDUI `0x42ffb0`).
                if small_button(ui, p, Icon::Params, "Коэффициенты").clicked() {
                    cmds.push(Command::Coefficients);
                }
                let prim = self.active_doc().map(|d| d.primary_values());
                if small_button_sel(ui, p, Icon::Primary, "Первичные знач.", prim == Some(true)).clicked() {
                    cmds.push(Command::SetPrimaryValues(true));
                }
                if small_button_sel(ui, p, Icon::Primary, "Вторичные знач.", prim == Some(false)).clicked() {
                    cmds.push(Command::SetPrimaryValues(false));
                }
            });
            if big_button_sel(ui, p, Icon::Analysis, "Быстрый\nанализ", self.view == View::Analysis).clicked() {
                cmds.push(Command::Show(View::Analysis));
            }
        });
        // Осциллограмма
        // The unnamed panel (title " ", «Информация» in the help).
        ribbon_group(ui, p, " ", |ui| {
            // 142: toggles the parameters bar, checked while it is shown.
            if big_button_sel(ui, p, Icon::Wave, "Осцилло-\nграмма", self.dlg.param_bar)
                .on_hover_text("Показать или скрыть панель параметров осциллограммы")
                .clicked()
            {
                cmds.push(Command::ToggleParamBar);
            }
        });
    }

    /// Category «ОМП», panel ОМП: 32807 Параметры (always enabled with a
    /// document) and 32806 Анализ (a toggle, checked while the result is
    /// shown, disabled while pasted channels are pending); both greyed with
    /// no document. Neither switches the view.
    fn ribbon_omp(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<Command>) {
        let p = self.skin.palette();
        let doc = self.active_doc();
        let has_doc = doc.is_some();
        let can_run = doc.is_some_and(|d| !crate::omp_ui::has_pending_paste(d));
        let shown = doc.is_some_and(|d| d.omp.shown);
        ribbon_group(ui, p, "ОМП", |ui| {
            ui.add_enabled_ui(has_doc, |ui| {
                if big_button(ui, p, Icon::Params, "Параметры").on_hover_text("Параметры определения места повреждения").clicked() {
                    cmds.push(Command::OmpParams);
                }
            });
            ui.add_enabled_ui(can_run, |ui| {
                if big_button_sel(ui, p, Icon::OmpRun, "Анализ", shown).on_hover_text("Произвести определение места повреждения").clicked() {
                    cmds.push(Command::OmpAnalysis);
                }
            });
        });
    }
}

// ---------------- drawing helpers ----------------


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

fn qat_button(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Icon) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
    let painter = ui.painter();
    if resp.hovered() {
        painter.rect(rect, 3.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
    }
    let box_ = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0));
    icons::paint(painter, box_, icon, p.accent);
    resp
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

fn big_button(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Icon, label: &str) -> egui::Response {
    big_button_sel(ui, p, icon, label, false)
}

fn big_button_sel(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Icon, label: &str, selected: bool) -> egui::Response {
    let size = egui::vec2(58.0, 64.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();
    let hovered = resp.hovered();
    if selected || hovered {
        let fill = if selected { p.accent_soft } else { p.accent_soft.gamma_multiply(0.5f32) };
        painter.rect(rect, 3.0f32, fill, egui::Stroke::new(1.0f32, p.accent_border));
    }
    let box_ = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 20.0), egui::vec2(28.0, 28.0));
    icons::paint(painter, box_, icon, p.accent);
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

/// An app-menu item that queues `cmd` and closes the menu.
/// Where the application menu keeps what its right column shows, with the
/// pass number it was last drawn in.
pub fn app_sub_id() -> egui::Id {
    egui::Id::new("fv_app_menu_sub")
}

/// The application menu's right column contents besides the recent files.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppSub {
    Print,
    Extras,
}

/// A main-panel sub-item: the label in bold over its description (the
/// first part of the command's string).
fn sub_entry(ui: &mut egui::Ui, label: &str, desc: &str, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let v = ui.visuals();
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 2.0f32, v.widgets.hovered.weak_bg_fill);
    }
    let (strong, weak) = if enabled { (v.strong_text_color(), v.text_color()) } else { (v.weak_text_color(), v.weak_text_color()) };
    ui.painter().text(rect.left_top() + egui::vec2(6.0, 3.0), egui::Align2::LEFT_TOP, label, egui::FontId::proportional(13.0), strong);
    ui.painter().text(rect.left_top() + egui::vec2(6.0, 20.0), egui::Align2::LEFT_TOP, desc, egui::FontId::proportional(11.0), weak);
    resp
}

fn small_button(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Icon, label: &str) -> egui::Response {
    small_button_sel(ui, p, icon, label, false)
}

fn small_button_sel(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Icon, label: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(126.0, 19.0), egui::Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect(rect, 2.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
    } else if resp.hovered() {
        painter.rect(rect, 2.0f32, p.accent_soft.gamma_multiply(0.7f32), egui::Stroke::new(1.0f32, p.accent_border));
    }
    let box_ = egui::Rect::from_min_size(egui::pos2(rect.left() + 3.0, rect.top() + 2.5), egui::vec2(14.0, 14.0));
    icons::paint(painter, box_, icon, p.accent);
    painter.text(
        egui::pos2(rect.left() + 22.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(11.0),
        p.text,
    );
    resp
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
