//! The Office-2007-style ribbon: quick-access toolbar, tab strip and command
//! groups. Layout mirrors the original (QAT: open/save/print/undo/redo; tabs
//! Главная / ОМП; groups Объединение · Диаграммы · Видимость · Вычисления ·
//! Осциллограмма; ОМП tab: Параметры · Анализ — a toggle with the result bar,
//! `omp_ui.rs`). See `notes/UI_SPEC.md`.

use crate::app::{App, Command, Pane, RibbonTab, ValueMode};
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
        // Print preview: MFC's own category is the only one shown
        // (`CMFCRibbonBar::SetPrintPreviewMode`; labels from mfc120rus).
        if self.print.preview.is_some() {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ribbon_tab(ui, "Просмотр для печати", true, p.tab_active, p.text);
            });
            if !self.ribbon_minimized {
                egui::Frame::none()
                    .fill(p.tab_active)
                    .stroke(egui::Stroke::new(1.0f32, p.group_border))
                    .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                    .show(ui, |ui| {
                        ui.set_min_height(102.0);
                        ui.set_min_width(ui.available_width());
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.horizontal_top(|ui| self.ribbon_preview(ui));
                    });
            }
            for c in cmds {
                self.dispatch(c);
            }
            return;
        }
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
                    // A constant height: expanded panels (buttons + title),
                    // across the whole window.
                    ui.set_min_height(102.0);
                    ui.set_min_width(ui.available_width());
                    // Never wrap a label: a group that does not fit is
                    // collapsed (below), not squeezed.
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    let titles: &[&str] = match self.tab {
                        RibbonTab::Home => &HOME_GROUPS,
                        RibbonTab::Omp => &OMP_GROUPS,
                    };
                    let widths: Vec<f32> = titles.iter().map(|t| ui.data(|d| d.get_temp::<f32>(group_width_id(t))).unwrap_or(0.0)).collect();
                    let plan = collapse_plan(&widths, ui.available_width(), COLLAPSED_W);
                    ui.data_mut(|d| {
                        for (t, c) in titles.iter().zip(&plan) {
                            d.insert_temp(group_collapsed_id(t), *c);
                        }
                    });
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
            // Small icon-only buttons (the label is the tooltip), three to a
            // column as the original: [32845/46/47] | [32802/03/138]
            // [32800/01/141] [32813/14/139] [32815/16] | [155/157/156].
            let armed = self.rect_zoom != crate::app::RectZoom::Off;
            let shown = |k: usize| self.active_doc().is_some_and(|d| if k < 3 { d.v_shown[k] } else { d.hviz[k - 3].shown });
            let comments_ok = self.active_doc().is_some_and(|d| d.comments_allowed());
            let mode = |k: u8| self.active_doc().is_some_and(|d| d.comment_mode == k);
            let columns: [&[MiniCmd]; 6] = [
                &[
                    (Mini::ShowDigital, "Показать дискретные", true, false, Command::Discrete(0)),
                    (Mini::HideDigital, "Скрыть дискретные", true, false, Command::Discrete(1)),
                    (Mini::HideUnchanged, "Скрыть неизменяющиеся", true, false, Command::Discrete(2)),
                ],
                &[
                    (Mini::ZoomH(true), "Увел. по гор.", true, false, Command::ZoomX(true)),
                    (Mini::ZoomH(false), "Уменьш. по гор.", true, false, Command::ZoomX(false)),
                    (Mini::RectZoom, "Приблизить по вертикали", true, armed, Command::RectZoom),
                ],
                &[
                    (Mini::ZoomV(true), "Увел. по верт.", true, false, Command::ZoomY(true)),
                    (Mini::ZoomV(false), "Уменьш. по верт.", true, false, Command::ZoomY(false)),
                    (Mini::Reset, "Вернуть к исходному размеру", true, false, Command::ResetZoom),
                ],
                &[
                    (Mini::Vizir(0), "Верт. визир 1", true, shown(0), Command::ToggleVizir(0)),
                    (Mini::Vizir(1), "Верт. визир 2", true, shown(1), Command::ToggleVizir(1)),
                    (Mini::Vizir(2), "Верт. визир 3", true, shown(2), Command::ToggleVizir(2)),
                ],
                &[
                    (Mini::Vizir(3), "Гор. визир 1", true, shown(3), Command::ToggleVizir(3)),
                    (Mini::Vizir(4), "Гор. визир 2", true, shown(4), Command::ToggleVizir(4)),
                ],
                // 155 / 157 / 156: one-shot comment tools (a radio group with
                // toggle-off), enabled for records from .cfg / .osc files.
                &[
                    (Mini::Moment, "Момент времени", comments_ok, mode(1), Command::CommentMode(1)),
                    (Mini::Area, "Область", comments_ok, mode(3), Command::CommentMode(3)),
                    (Mini::Level, "Линия уровня", comments_ok, mode(2), Command::CommentMode(2)),
                ],
            ];
            for (k, column) in columns.into_iter().enumerate() {
                if k == 1 || k == 5 {
                    group_separator(ui, p);
                }
                ui.vertical(|ui| {
                    for (glyph, tip, enabled, on, cmd) in column {
                        if mini_button(ui, p, *glyph, tip, *enabled, *on) {
                            cmds.push(cmd.clone());
                        }
                    }
                });
            }
        });
        // Вычисления
        ribbon_group(ui, p, "Вычисления", |ui| {
            // 32817 / 32818 / 32819: stats rows on every analog channel,
            // checked when all channels show the row; 32820..32822 act on the
            // selected channels, checked only when every selected channel is
            // in that mode. Icon-only, as in the original.
            let stat_on = |k: usize| self.active_doc().is_some_and(|d| !d.chans.is_empty() && d.chans.iter().all(|c| [c.stats.minmax, c.stats.rms, c.stats.a1][k]));
            let mode_on = |m| self.active_doc().is_some_and(|d| d.mode_state(m).checked);
            let columns: [[(Mini, &str, bool, Command); 3]; 2] = [
                [
                    (Mini::MinMax, "Мин/макс", stat_on(0), Command::StatsRow(0)),
                    (Mini::Rms, "СКЗ", stat_on(1), Command::StatsRow(1)),
                    (Mini::A1, "Оценка 1-й гарм.", stat_on(2), Command::StatsRow(2)),
                ],
                [
                    (Mini::Instant, "Мгновенные", mode_on(ValueMode::Instant), Command::SetValueMode(ValueMode::Instant)),
                    (Mini::RmsMode, "Среднеквадр.", mode_on(ValueMode::Rms), Command::SetValueMode(ValueMode::Rms)),
                    (Mini::Harm1, "1-я гармоника", mode_on(ValueMode::FirstHarmonic), Command::SetValueMode(ValueMode::FirstHarmonic)),
                ],
            ];
            for column in columns {
                ui.vertical(|ui| {
                    for (glyph, tip, on, cmd) in column {
                        if mini_button(ui, p, glyph, tip, true, on) {
                            cmds.push(cmd);
                        }
                    }
                });
            }
            group_separator(ui, p);
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
            let analog = self.active_doc().is_some_and(|d| !d.osc.analog.is_empty());
            if ui.add_enabled_ui(analog, |ui| big_button(ui, p, Icon::Analysis, "Быстрый\nанализ")).inner.clicked() {
                cmds.push(Command::QuickAnalysis);
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

    /// MFC's print-preview category (`mfc120u` `0x100fcfc2`): «Печать»
    /// {0xE304 Печать}, «Масштаб» {0xE305 Увеличить, 0xE306 Уменьшить,
    /// 0xE301 Одна / Две страницы}, «Просмотр» {0xE302 Следующая страница,
    /// 0xE303 Предыдущая страница, ─, 0xE300 Закрыть}; all large buttons.
    fn ribbon_preview(&mut self, ui: &mut egui::Ui) {
        use crate::print_ui::PreviewCmd as P;
        let p = self.skin.palette();
        let Some(pv) = self.print.preview.as_ref() else { return };
        let numpage = pv.numpage_label();
        let state: Vec<(P, bool)> = [P::Print, P::ZoomIn, P::ZoomOut, P::NumPage, P::Next, P::Prev, P::Close].into_iter().map(|c| (c, pv.enabled(c))).collect();
        let on = |c: P| state.iter().any(|&(k, e)| k == c && e);
        let mut fire = None;
        let mut button = |ui: &mut egui::Ui, icon: Icon, label: &str, c: P| {
            if ui.add_enabled_ui(on(c), |ui| big_button(ui, p, icon, label)).inner.clicked() {
                fire = Some(c);
            }
        };
        ribbon_group(ui, p, "Печать", |ui| button(ui, Icon::Print, "Печать", P::Print));
        ribbon_group(ui, p, "Масштаб", |ui| {
            button(ui, Icon::ZoomIn, "Увеличить", P::ZoomIn);
            button(ui, Icon::ZoomOut, "Уменьшить", P::ZoomOut);
            button(ui, Icon::Pages, numpage, P::NumPage);
        });
        ribbon_group(ui, p, "Просмотр", |ui| {
            button(ui, Icon::NextPage, "Следующая страница", P::Next);
            button(ui, Icon::PrevPage, "Предыдущая страница", P::Prev);
            group_separator(ui, p);
            button(ui, Icon::ClosePreview, "Закрыть", P::Close);
        });
        if let (Some(c), Some(pv)) = (fire, self.print.preview.as_mut()) {
            pv.cmd = Some(c);
        }
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
/// The panels of each category, left to right.
const HOME_GROUPS: [&str; 5] = ["Объединение", "Диаграммы", "Видимость", "Вычисления", " "];
const OMP_GROUPS: [&str; 1] = ["ОМП"];
/// Width a collapsed panel takes (its button, divider and gap).
const COLLAPSED_W: f32 = 82.0;

fn group_width_id(title: &str) -> egui::Id {
    egui::Id::new(("fv_ribbon_group_w", title))
}

fn group_collapsed_id(title: &str) -> egui::Id {
    egui::Id::new(("fv_ribbon_group_collapsed", title))
}

/// Which panels collapse to a single button so the category fits `avail`
/// (MFC ribbon: panels collapse from the right). A width of 0 = not measured
/// yet (drawn expanded once to measure it).
pub fn collapse_plan(widths: &[f32], avail: f32, collapsed_w: f32) -> Vec<bool> {
    let mut plan = vec![false; widths.len()];
    let mut total: f32 = widths.iter().sum();
    for i in (0..widths.len()).rev() {
        if total <= avail {
            break;
        }
        if widths[i] > collapsed_w {
            plan[i] = true;
            total += collapsed_w - widths[i];
        }
    }
    plan
}

fn ribbon_group<R>(ui: &mut egui::Ui, p: crate::theme::Palette, title: &str, content: impl FnOnce(&mut egui::Ui) -> R) {
    let collapsed = ui.data(|d| d.get_temp::<bool>(group_collapsed_id(title))).unwrap_or(false);
    if collapsed {
        // One button with the panel's name; the panel opens below it.
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(COLLAPSED_W - 12.0, 84.0), egui::Sense::click());
        let painter = ui.painter();
        if resp.hovered() {
            painter.rect(rect, 3.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
        }
        painter.text(rect.center() - egui::vec2(0.0, 8.0), egui::Align2::CENTER_CENTER, title, egui::FontId::proportional(11.0), p.text);
        painter.text(rect.center() + egui::vec2(0.0, 10.0), egui::Align2::CENTER_CENTER, "▾", egui::FontId::proportional(11.0), p.text);
        let popup = egui::Id::new(("fv_ribbon_group_popup", title));
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup));
        }
        egui::popup_below_widget(ui, popup, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.horizontal_top(|ui| {
                content(ui);
            });
        });
        let r = ui.min_rect();
        ui.painter().line_segment(
            [egui::pos2(rect.right() + 2.0, r.top() + 4.0), egui::pos2(rect.right() + 2.0, r.bottom() - 2.0)],
            egui::Stroke::new(1.0f32, p.group_border),
        );
        ui.add_space(6.0);
        return;
    }
    let start = ui.cursor().left();
    ribbon_group_expanded(ui, p, title, content);
    let w = ui.cursor().left() - start + ui.spacing().item_spacing.x;
    ui.data_mut(|d| d.insert_temp(group_width_id(title), w));
}

fn ribbon_group_expanded<R>(ui: &mut egui::Ui, p: crate::theme::Palette, title: &str, content: impl FnOnce(&mut egui::Ui) -> R) {
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
    // A large button's two-word label wraps at the space.
    let wrapped;
    let label = if !label.contains('\n') && label.contains(' ') {
        wrapped = label.replacen(' ', "\n", 1);
        wrapped.as_str()
    } else {
        label
    };
    let font = egui::FontId::proportional(10.5);
    let text_w = label.split('\n').map(|l| ui.fonts(|f| f.layout_no_wrap(l.to_owned(), font.clone(), p.text).size().x)).fold(0.0, f32::max);
    let size = egui::vec2((text_w + 6.0).max(50.0), 64.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = &plain_painter(ui);
    let (ink, accent) = inks(ui, p);
    // The checked fill is light in every look: dark ink on it.
    let ink = if selected && ui.is_enabled() { CHECKED_INK } else { ink };
    let hovered = resp.hovered() && ui.is_enabled();
    if selected || hovered {
        let fill = if selected { p.accent_soft } else { p.accent_soft.gamma_multiply(0.5f32) };
        painter.rect(rect, 3.0f32, fill, egui::Stroke::new(1.0f32, p.accent_border));
    }
    let box_ = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 20.0), egui::vec2(28.0, 28.0));
    icons::paint(painter, box_, icon, accent);
    for (i, line) in label.split('\n').enumerate() {
        painter.text(egui::pos2(rect.center().x, rect.top() + 40.0 + i as f32 * 11.0), egui::Align2::CENTER_CENTER, line, font.clone(), ink);
    }
    resp
}

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
    let font = egui::FontId::proportional(11.0);
    let text_w = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), p.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2((22.0 + text_w + 6.0).max(90.0), 19.0), egui::Sense::click());
    let painter = &plain_painter(ui);
    let (ink, accent) = inks(ui, p);
    let ink = if selected && ui.is_enabled() { CHECKED_INK } else { ink };
    if selected {
        painter.rect(rect, 2.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
    } else if resp.hovered() && ui.is_enabled() {
        painter.rect(rect, 2.0f32, p.accent_soft.gamma_multiply(0.7f32), egui::Stroke::new(1.0f32, p.accent_border));
    }
    let box_ = egui::Rect::from_min_size(egui::pos2(rect.left() + 3.0, rect.top() + 2.5), egui::vec2(14.0, 14.0));
    icons::paint(painter, box_, icon, accent);
    painter.text(egui::pos2(rect.left() + 22.0, rect.center().y), egui::Align2::LEFT_CENTER, label, font, ink);
    resp
}

/// A separator inside a panel, as tall as the button rows (an egui
/// separator would take the whole height and push the panel title out).
fn group_separator(ui: &mut egui::Ui, p: crate::theme::Palette) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, 72.0), egui::Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), egui::Stroke::new(1.0f32, p.group_border));
}

/// An icon-only button: glyph, tooltip, enabled, checked, command.
type MiniCmd<'a> = (Mini, &'a str, bool, bool, Command);

/// A painter without egui's disabled fade: a disabled widget's painter fades
/// towards the panel colour, which makes light labels vanish on the dark
/// look; the ribbon's buttons pick their disabled colour themselves.
fn plain_painter(ui: &egui::Ui) -> egui::Painter {
    egui::Painter::new(ui.ctx().clone(), ui.layer_id(), ui.clip_rect())
}

/// Labels on the (light) checked fill.
const CHECKED_INK: Color32 = Color32::from_rgb(0x20, 0x20, 0x20);

/// A disabled label: the text colour at half strength (readable on every
/// look's body).
fn disabled_ink(p: crate::theme::Palette) -> Color32 {
    p.text.gamma_multiply(0.5f32)
}

/// The label and icon colours for the button's enabled state.
fn inks(ui: &egui::Ui, p: crate::theme::Palette) -> (Color32, Color32) {
    if ui.is_enabled() {
        (p.text, p.accent)
    } else {
        (disabled_ink(p), disabled_ink(p))
    }
}

/// The glyphs of the ribbon's small icon-only buttons (our drawings; the
/// original's image list is not reproduced).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mini {
    ShowDigital,
    HideDigital,
    HideUnchanged,
    ZoomH(bool),
    ZoomV(bool),
    RectZoom,
    Reset,
    Vizir(usize),
    Moment,
    Area,
    Level,
    MinMax,
    Rms,
    A1,
    Instant,
    RmsMode,
    Harm1,
}

/// A small icon-only button (24×24) with its label as the tooltip; framed
/// while `on`. Returns whether it was clicked.
fn mini_button(ui: &mut egui::Ui, p: crate::theme::Palette, glyph: Mini, tip: &str, enabled: bool, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let painter = &plain_painter(ui);
    let enabled = enabled && ui.is_enabled();
    if on {
        painter.rect(rect, 2.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
    } else if enabled && resp.hovered() {
        painter.rect(rect, 2.0f32, p.accent_soft.gamma_multiply(0.7f32), egui::Stroke::new(1.0f32, p.accent_border));
    }
    let col = if enabled { p.accent } else { disabled_ink(p) };
    paint_mini(painter, rect.shrink(4.0), glyph, col);
    resp.on_hover_text(tip).clicked() && enabled
}

fn paint_mini(painter: &egui::Painter, r: egui::Rect, glyph: Mini, col: Color32) {
    use egui::{pos2, Stroke};
    let s = Stroke::new(1.5f32, col);
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let line = |a: egui::Pos2, b: egui::Pos2| {
        painter.line_segment([a, b], s);
    };
    let arrow = |a: egui::Pos2, b: egui::Pos2| {
        line(a, b);
        let d = (b - a).normalized() * 3.5;
        let n = egui::vec2(-d.y, d.x);
        line(b, b - d + n);
        line(b, b - d - n);
    };
    let text = |t: &str, size: f32| {
        painter.text(r.center(), egui::Align2::CENTER_CENTER, t, egui::FontId::proportional(size), col);
    };
    let wave = |amp: f32| {
        let pts: Vec<egui::Pos2> = (0..=16).map(|i| {
            let x = i as f32 / 16.0;
            at(x, 0.5 - amp * (x * std::f32::consts::TAU).sin())
        }).collect();
        painter.add(egui::Shape::line(pts, s));
    };
    match glyph {
        Mini::ShowDigital | Mini::HideDigital | Mini::HideUnchanged => {
            // Three digital traces; hidden ones dashed.
            for (i, y) in [0.2f32, 0.5, 0.8].into_iter().enumerate() {
                let hidden = match glyph {
                    Mini::ShowDigital => false,
                    Mini::HideDigital => true,
                    _ => i == 1,
                };
                if hidden {
                    painter.add(egui::Shape::dashed_line(&[at(0.0, y), at(1.0, y)], Stroke::new(1.0f32, col), 2.0, 2.0));
                } else {
                    painter.add(egui::Shape::line(vec![at(0.0, y), at(0.3, y), at(0.3, y - 0.12), at(0.7, y - 0.12), at(0.7, y), at(1.0, y)], s));
                }
            }
        }
        Mini::ZoomH(grow) | Mini::ZoomV(grow) => {
            let h = matches!(glyph, Mini::ZoomH(_));
            let (a, b) = if h { (at(0.0, 0.7), at(1.0, 0.7)) } else { (at(0.7, 1.0), at(0.7, 0.0)) };
            let c = a + (b - a) * 0.5;
            if grow {
                arrow(c, a);
                arrow(c, b);
            } else {
                arrow(a, c - (b - a) * 0.08);
                arrow(b, c + (b - a) * 0.08);
            }
            let sign = if grow { "+" } else { "−" };
            let pos = if h { at(0.5, 0.18) } else { at(0.22, 0.5) };
            painter.text(pos, egui::Align2::CENTER_CENTER, sign, egui::FontId::proportional(12.0), col);
        }
        Mini::RectZoom => {
            painter.add(egui::Shape::dashed_line(&[at(0.1, 0.1), at(0.9, 0.1), at(0.9, 0.9), at(0.1, 0.9), at(0.1, 0.1)], Stroke::new(1.0f32, col), 2.0, 2.0));
            arrow(at(0.5, 0.75), at(0.5, 0.25));
        }
        Mini::Reset => text("1:1", 11.0),
        Mini::Vizir(k) => {
            let colors = [Color32::from_rgb(255, 100, 100), Color32::from_rgb(100, 100, 255), Color32::from_rgb(0, 160, 0), Color32::from_rgb(255, 100, 100), Color32::from_rgb(100, 100, 255)];
            let vs = Stroke::new(1.5f32, colors[k]);
            if k < 3 {
                painter.line_segment([at(0.35, 0.0), at(0.35, 1.0)], vs);
            } else {
                painter.line_segment([at(0.0, 0.35), at(1.0, 0.35)], vs);
            }
            painter.text(at(0.72, 0.72), egui::Align2::CENTER_CENTER, format!("{}", if k < 3 { k + 1 } else { k - 2 }), egui::FontId::proportional(10.0), col);
        }
        Mini::Moment => {
            line(at(0.5, 0.3), at(0.5, 1.0));
            painter.rect_stroke(egui::Rect::from_min_max(at(0.1, 0.0), at(0.9, 0.3)), 1.0f32, Stroke::new(1.0f32, col));
        }
        Mini::Area => {
            painter.rect_filled(egui::Rect::from_min_max(at(0.2, 0.3), at(0.8, 1.0)), 0.0, col.gamma_multiply(0.35f32));
            painter.rect_stroke(egui::Rect::from_min_max(at(0.2, 0.3), at(0.8, 1.0)), 0.0, Stroke::new(1.0f32, col));
        }
        Mini::Level => {
            line(at(0.0, 0.6), at(1.0, 0.6));
            painter.rect_stroke(egui::Rect::from_min_max(at(0.1, 0.0), at(0.7, 0.35)), 1.0f32, Stroke::new(1.0f32, col));
        }
        Mini::MinMax => {
            line(at(0.0, 0.05), at(1.0, 0.05));
            line(at(0.0, 0.95), at(1.0, 0.95));
            wave(0.4);
        }
        Mini::Rms => {
            wave(0.4);
            line(at(0.0, 0.25), at(1.0, 0.25));
        }
        Mini::A1 => text("A1", 11.0),
        Mini::Instant => wave(0.4),
        Mini::RmsMode => {
            line(at(0.0, 0.3), at(0.4, 0.3));
            line(at(0.4, 0.3), at(0.6, 0.6));
            line(at(0.6, 0.6), at(1.0, 0.6));
        }
        Mini::Harm1 => {
            wave(0.25);
            painter.text(at(0.85, 0.15), egui::Align2::CENTER_CENTER, "1", egui::FontId::proportional(9.0), col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ribbon labels stay legible in every look: enabled, disabled and on
    /// the checked fill (the dark look's disabled labels used to fade into
    /// its body).
    #[test]
    fn ribbon_labels_contrast_with_their_background_in_every_look() {
        let lum = |c: Color32| 0.299 * f32::from(c.r()) + 0.587 * f32::from(c.g()) + 0.114 * f32::from(c.b());
        let over = |fg: Color32, bg: Color32| {
            let a = f32::from(fg.a()) / 255.0;
            let mix = |f: u8, b: u8| (f32::from(f) + f32::from(b) * (1.0 - a)) as u8;
            Color32::from_rgb(mix(fg.r(), bg.r()), mix(fg.g(), bg.g()), mix(fg.b(), bg.b()))
        };
        for skin in Skin::ALL {
            let p = skin.palette();
            let body = p.tab_active;
            assert!((lum(p.text) - lum(body)).abs() > 60.0, "{skin:?}: enabled");
            assert!((lum(over(disabled_ink(p), body)) - lum(body)).abs() > 25.0, "{skin:?}: disabled");
            assert!((lum(CHECKED_INK) - lum(p.accent_soft)).abs() > 100.0, "{skin:?}: checked");
        }
    }

    #[test]
    fn panels_collapse_from_the_right_until_the_category_fits() {
        let w = [190.0, 270.0, 300.0, 400.0, 80.0];
        assert_eq!(collapse_plan(&w, 1300.0, 82.0), vec![false; 5]);
        // 1240 > 1000: the last panel is no wider than its button, so the
        // next one in collapses (−318), then fits.
        assert_eq!(collapse_plan(&w, 1000.0, 82.0), vec![false, false, false, true, false]);
        assert_eq!(collapse_plan(&w, 600.0, 82.0), vec![false, true, true, true, false]);
        // Unmeasured panels (0) never collapse.
        assert_eq!(collapse_plan(&[0.0, 0.0], 10.0, 82.0), vec![false, false]);
    }
}
