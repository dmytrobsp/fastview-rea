//! The Office-2007-style ribbon: quick-access toolbar, tab strip and command
//! groups. Layout mirrors the original (QAT: open/save/print/undo/redo; tabs
//! Главная / ОМП; groups Объединение · Диаграммы · Видимость · Вычисления ·
//! Осциллограмма; ОМП tab: Параметры · Анализ — a toggle with the result bar,
//! `omp_ui.rs`). See `notes/UI_SPEC.md`.

use crate::app::{App, Command, Pane, RibbonTab, ValueMode};
use crate::icons::{self, Icon};
use crate::oscgeom::PixelLines;
use crate::theme::Skin;
use eframe::egui::{self, Color32, RichText};

impl App {
    /// The wheel over the ribbon steps its tabs (CMFCRibbonBar's
    /// OnMouseWheel, VM): a notch down the next, up the previous, stopping
    /// at either end; not with a modifier, nor in the print preview.
    pub fn ribbon_wheel(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if self.print.preview.is_some() || !ui.rect_contains_pointer(rect) {
            return;
        }
        let (deltas, mods) = ui.input(|i| (crate::lanes::wheel_deltas(&i.events), i.modifiers));
        if !mods.is_none() {
            return;
        }
        let tabs = [RibbonTab::Home, RibbonTab::Omp];
        for d in deltas {
            let at = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
            let next = if d < 0.0 { (at + 1).min(tabs.len() - 1) } else { at.saturating_sub(1) };
            self.tab = tabs[next];
        }
    }

    pub fn ribbon_ui(&mut self, ui: &mut egui::Ui) {
        let p = self.skin.palette();
        let mut cmds: Vec<Command> = Vec::new();
        self.kt_rects.clear();

        // The quick-access toolbar's row is the frame's caption, which
        // Windows 11's DWM paints white in every look (VM: rows 0–29); the
        // tab strip below it (30–52) is the look's band — Office 2007 Blue
        // (191, 219, 255), Black (83, 83, 83), … — and the ribbon body
        // below that the lighter `tab_active` fill.
        let full = ui.max_rect();
        ui.painter().rect_filled(egui::Rect::from_min_size(full.min, egui::vec2(full.width(), 30.0)), 0.0, Color32::WHITE);
        let band = egui::Rect::from_min_size(full.min + egui::vec2(0.0, 30.0), egui::vec2(full.width(), 23.0));
        paint_v_gradient(ui, band, p.ribbon_top, p.ribbon_bottom);

        // The Office 2007 looks' round application button, 45 × 45 at the
        // top left over both rows (`SetApplicationButton` with (45, 45),
        // `0x472fef`; its image 570 is the original's — ours the logo).
        // The older looks' flat button is 41 px about the same centre (VM).
        let orb_d = if p.orb_flat.is_some() { 41.0 } else { 45.0 };
        let orb = p.app_orb.map(|c| (egui::Rect::from_center_size(full.min + egui::vec2(3.0 + 22.5, 2.0 + 22.5), egui::vec2(orb_d, orb_d)), c));
        // The rows touch: the caption 30 px, the tab strip 23 px, then the
        // body's frame (VM: rows 0–29, 30–52, the line at 53).
        let row_gap = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        // ---- Row 1: quick-access toolbar + title + style/help ----
        ui.horizontal(|ui| {
            ui.set_height(30.0);
            ui.add_space(4.0);
            if orb.is_none() {
                draw_logo(ui, 18.0, p.accent);
                ui.add_space(2.0);
            }
            // Application menu (0x472f30): the original's ten items, the
            // «Последние документы» list, and our own extras in a labelled
            // submenu.
            let has_doc = self.active_doc().is_some();
            let mru = self.mru.clone();
            let sub_id = app_sub_id();
            // Keytip «F» opens the menu from the keyboard (L9.90).
            let bar_id = ui.id();
            if let Some(b) = &mut self.kt_bar {
                b.0 = bar_id;
            }
            if std::mem::take(&mut self.kt_open_menu) {
                self.kt_menu(ui.ctx(), true);
            }
            let mut menu_rects: Vec<(crate::keytips::Target, egui::Rect, bool)> = Vec::new();
            let anchor = match orb {
                // The orb's upper part anchors the menu (its lower part is
                // in the tab row, below).
                Some((r, _)) => egui::Button::new("").frame(false).min_size(egui::vec2(r.width() - 4.0, 20.0)),
                None => egui::Button::new(RichText::new(APP_MENU_TITLE).color(p.text).strong()),
            };
            let app_button = egui::menu::menu_custom_button(ui, anchor, |ui| {
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
                            ("Сохранить", self.can_save(), Some(Command::Save), None),
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
                        // group in the original's main panel); tight, so the
                        // column fits the 500 px minimum window.
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (k, (label, enabled, cmd, opens)) in items.into_iter().chain([("Дополнительно  ▸", true, None, Some(AppSub::Extras))]).enumerate() {
                            if k > 0 {
                                // From after the image column (4 + 32 px).
                                let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 3.0), egui::Sense::hover());
                                ui.painter().hline((r.left() + 36.0)..=r.right(), r.center().y, ui.visuals().widgets.noninteractive.bg_stroke);
                            }
                            // Every item is a large-image button with its text
                            // on the right (the main panel's Repos); the
                            // drawings are ours.
                            let icon = [Some(Icon::Open), Some(Icon::Save), Some(Icon::Save), None, Some(Icon::Print), Some(Icon::Params)].get(k).copied().flatten();
                            let r = main_entry(ui, p, icon, label, enabled || opens.is_some(), opens.is_some() && sub == opens);
                            if k == 4 {
                                menu_rects.extend([(crate::keytips::Target::PrintMain, r.rect, enabled), (crate::keytips::Target::PrintMenu, r.rect, true)]);
                            } else if k < 10 {
                                menu_rects.push((crate::keytips::Target::AppItem(k), r.rect, enabled));
                            }
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
                                let cwd = std::env::current_dir().ok();
                                for (k, path) in mru.iter().enumerate() {
                                    let name = mru_display(path, cwd.as_deref());
                                    // «&1 name» … «1&0 name», then the plain name.
                                    let text = if k < 10 { format!("{} {name}", k + 1) } else { name };
                                    let b = egui::Button::new(text).frame(false).truncate();
                                    let r = ui.add(b);
                                    if k < 10 {
                                        menu_rects.push((crate::keytips::Target::Recent(k), r.rect, true));
                                    }
                                    if r.on_hover_text(path.display().to_string()).clicked() {
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
            let button = app_button.response.rect;
            self.kt_bar = Some((bar_id, button, app_button.response.id));
            self.kt_rects.push((crate::keytips::Target::AppButton, button, true));
            self.kt_rects.extend(menu_rects);
            ui.add_space(4.0);
            // Quick-access toolbar: Open, Save, Print only; Печать is a
            // split button whose arrow opens the application menu's print
            // list (VM), then the customise arrow.
            for (n, (icon, tip, cmd)) in [
                (Icon::Open, "Открыть  (Ctrl+O)", Command::Open),
                (Icon::Save, "Сохранить", Command::Save),
                (Icon::Print, "Печать  (Ctrl+P)", Command::Print),
            ]
            .into_iter()
            .enumerate()
            {
                // Сохранить / Печать have no handler without a document
                // (MFC disables them); Сохранить is greyed for a .sto record
                // (`0x475750`).
                let enabled = match n {
                    0 => true,
                    1 => self.can_save(),
                    _ => has_doc,
                };
                let r = ui.add_enabled_ui(enabled, |ui| qat_button(ui, p, icon)).inner.on_hover_text(tip);
                self.kt_rects.push((crate::keytips::Target::Qat(n), r.rect, enabled));
                if r.clicked() {
                    cmds.push(cmd);
                }
                if r.secondary_clicked() {
                    open_min_menu(ui, r.rect.left_bottom());
                }
                if n == 2 {
                    let arrow = ui.add_enabled_ui(enabled, |ui| qat_arrow(ui, p, false)).inner;
                    if arrow.clicked() {
                        open_qat_menu(ui, QatMenu::Print, r.rect.left_bottom());
                    }
                }
            }
            ui.add_space(6.0);
            // The customise arrow: one item, «Свернуть ленту».
            let custom = qat_arrow(ui, p, true).on_hover_text("Настройка панели быстрого доступа");
            if custom.clicked() || custom.secondary_clicked() {
                open_min_menu(ui, custom.rect.left_bottom());
            }
            // The caption row (the ribbon's own, `bReplaceFrameCaption`,
            // over DWM's frame — L9.150): the title centred in the free
            // space between the quick-access toolbar and the system
            // buttons; the free space drags the window, a double-click
            // maximises / restores.
            if self.own_caption {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    caption_buttons(ui);
                    let free = ui.available_rect_before_wrap();
                    let hit = ui.interact(free, ui.id().with("fv_caption_drag"), egui::Sense::click_and_drag());
                    if hit.double_clicked() {
                        let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                    } else if hit.drag_started() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    // Windows 11's caption font, Segoe UI 9 pt (12 px).
                    let font = egui::FontId::proportional(12.0);
                    let g = ui.painter().layout_no_wrap(self.window_title.clone(), font, Color32::BLACK);
                    let w = g.size().x.min(free.width());
                    // The style's TextCenter: off for Aqua (left-aligned).
                    let x = if self.skin == Skin::Aqua { free.left() + 4.0 } else { (free.center().x - w / 2.0).max(free.left()) };
                    ui.painter().with_clip_rect(free).galley(egui::pos2(x, free.center().y - g.size().y / 2.0), g, Color32::BLACK);
                });
            }
        });

        // ---- Row 2: tab strip ----
        // Print preview: MFC's own category is the only one shown
        // (`CMFCRibbonBar::SetPrintPreviewMode`; labels from mfc120rus).
        if self.print.preview.is_some() {
            // The application button and the QAT stay (VM).
            ui.horizontal(|ui| {
                ui.set_height(24.0);
                ui.add_space(6.0);
                self.orb_ui(ui, orb, p);
                let r = ribbon_tab(ui, "Просмотр для печати", true, p);
                self.kt_rects.push((crate::keytips::Target::PreviewTab, r.rect, true));
                // «Стиль» and Help stay on the right (VM).
                self.tab_row_right(ui, p, &mut cmds);
            });
            if !self.ribbon_minimized {
                egui::Frame::none()
                    .fill(p.tab_active)
                    .stroke(egui::Stroke::new(1.0f32, p.group_border))
                    .inner_margin(egui::Margin { left: 6.0, right: 6.0, top: 3.0, bottom: 1.0 })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = row_gap;
                        ui.set_min_height(LARGE_H + 2.0 + 16.0 + 2.0);
                        ui.set_min_width(ui.available_width());
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.horizontal_top(|ui| self.ribbon_preview(ui));
                    });
            }
            self.keytips_paint(ui.ctx());
            for c in cmds {
                self.dispatch(c);
            }
            return;
        }
        ui.horizontal(|ui| {
            // 23 px, and the body frame's stroke straddles its edge: 24.
            ui.set_height(24.0);
            ui.add_space(6.0);
            self.orb_ui(ui, orb, p);
            for (n, (label, tab)) in [("Главная", RibbonTab::Home), ("ОМП", RibbonTab::Omp)].into_iter().enumerate() {
                let r = ribbon_tab(ui, label, self.tab == tab, p);
                self.kt_rects.push((crate::keytips::Target::Tab(n), r.rect, true));
                if r.double_clicked() {
                    cmds.push(Command::ToggleRibbonMin);
                } else if r.clicked() {
                    self.tab = tab;
                }
                if r.secondary_clicked() {
                    open_min_menu(ui, r.rect.left_bottom());
                }
            }
            self.tab_row_right(ui, p, &mut cmds);
        });

        // ---- Row 3: ribbon body (fixed height; hidden when minimised) ----
        if !self.ribbon_minimized {
            // The category body's gradient behind it (set below).
            let body_bg = ui.painter().add(egui::Shape::Noop);
            let body = egui::Frame::none()
                .fill(if p.body.is_some() { Color32::TRANSPARENT } else { p.tab_active })
                .stroke(egui::Stroke::new(1.0f32, p.group_border))
                .inner_margin(egui::Margin { left: 6.0, right: 6.0, top: 3.0, bottom: 1.0 })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = row_gap;
                    // A constant height: expanded panels (a large button,
                    // the panel margins and its 15 px caption), across the
                    // whole window — 86 px inside, as on the VM.
                    ui.set_min_height(LARGE_H + 2.0 + 16.0 + 2.0);
                    ui.set_min_width(ui.available_width());
                    // Never wrap a label: a group that does not fit is
                    // collapsed (below), not squeezed.
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    // MFC's reduction (L9.91): every panel's layouts measured
                    // in an invisible pass, then the category plan.
                    let groups: &[(&str, usize, bool)] = match self.tab {
                        RibbonTab::Home => &HOME_GROUPS,
                        RibbonTab::Omp => &OMP_GROUPS,
                    };
                    let avail = ui.available_width();
                    let most = groups.iter().map(|g| g.1).max().unwrap_or(1);
                    // Once per tab: the layouts' widths do not change.
                    let measured = groups.iter().all(|&(t, n, _)| (0..n).all(|k| ui.data(|d| d.get_temp::<f32>(group_width_id(t, k)).is_some())));
                    for step in (0..most).filter(|_| !measured) {
                        ui.data_mut(|d| d.insert_temp(measure_id(), step));
                        let far = egui::Rect::from_min_size(egui::pos2(-30000.0, 0.0), egui::vec2(10000.0, 200.0));
                        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("fv_ribbon_measure", step)).max_rect(far).layout(egui::Layout::left_to_right(egui::Align::TOP)).invisible());
                        child.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        let mut sink = Vec::new();
                        match self.tab {
                            RibbonTab::Home => self.ribbon_home(&mut child, &mut sink),
                            RibbonTab::Omp => self.ribbon_omp(&mut child, &mut sink),
                        }
                    }
                    ui.data_mut(|d| d.remove::<usize>(measure_id()));
                    let widths: Vec<Vec<f32>> = groups
                        .iter()
                        .map(|&(t, n, single)| {
                            let mut v: Vec<f32> = (0..n).map(|k| ui.data(|d| d.get_temp::<f32>(group_width_id(t, k))).unwrap_or(0.0)).collect();
                            let cw = collapsed_width(ui, t);
                            // No collapse step when the panel is no wider
                            // than its button, or a single element under
                            // 1.5 × it (`RecalcWidths` `0x10125189`).
                            if !(v[0] <= cw || (single && v[0] <= 1.5 * cw)) {
                                v.push(cw);
                            }
                            v
                        })
                        .collect();
                    let plan = reduction_plan(&widths, avail);
                    ui.data_mut(|d| {
                        for ((t, n, _), (step, w)) in groups.iter().zip(plan.iter().zip(&widths)) {
                            // The collapsed layout is index n.
                            d.insert_temp(group_step_id(t), if *step >= *n && w.len() > *n { *n } else { (*step).min(n - 1) });
                        }
                    });
                    ui.horizontal_top(|ui| match self.tab {
                        RibbonTab::Home => self.ribbon_home(ui, &mut cmds),
                        RibbonTab::Omp => self.ribbon_omp(ui, &mut cmds),
                    });
                });
            if let Some([top, dip, bottom]) = p.body {
                let r = body.response.rect;
                ui.painter().set(body_bg, gradient_mesh(r, &[(0.0, top), (18.0, dip), (r.height(), bottom)]));
            }
            // A right-click anywhere on the body, its buttons included.
            let at = ui.input(|i| i.pointer.secondary_clicked().then_some(i.pointer.interact_pos()).flatten());
            if let Some(q) = at.filter(|q| body.response.rect.contains(*q)) {
                open_min_menu(ui, q);
            }
        }
        self.qat_menu(ui.ctx(), &mut cmds);

        for c in cmds {
            self.dispatch(c);
        }
        self.keytips_paint(ui.ctx());
    }

    fn ribbon_home(&mut self, ui: &mut egui::Ui, cmds: &mut Vec<Command>) {
        let p = self.skin.palette();
        // Объединение
        // Steps (`Repos` `0x101227d5`): the small column goes icon-only.
        ribbon_group(ui, p, "Объединение", 2, |ui, step| {
            // 57637 needs the clipboard and no pending paste; 57634 a selected
            // file channel; 57643 / 57644 the undo / redo counts.
            let can_paste = self.active_doc().is_some_and(|d| self.clip.available() && !crate::omp_ui::has_pending_paste(d));
            let can_copy = self.active_doc().is_some_and(|d| d.can_copy());
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
                    if ui.add_enabled_ui(on, |ui| small_button_mode(ui, p, icon, label, false, step >= 1)).inner.clicked() {
                        cmds.push(cmd);
                    }
                }
            });
        });
        // Диаграммы
        // Steps: the three right large buttons become small with text,
        // then icon-only.
        ribbon_group(ui, p, "Диаграммы", 3, |ui, step| {
            // 32811 / 32809 / 32808 / 140 toggle the panes; checked while shown.
            let buttons = [
                (Icon::Freq, "Частотная", Pane::Frequency),
                (Icon::Vector, "Векторная", Pane::Vector),
                (Icon::Circle, "Круговая", Pane::Circle),
                (Icon::Diff, "Дифферен циальная", Pane::Differential),
            ];
            for (k, (icon, label, pane)) in buttons.into_iter().enumerate() {
                if k > 0 && step > 0 {
                    continue;
                }
                if big_button_sel(ui, p, icon, label, self.panes.visible(pane)).clicked() {
                    cmds.push(Command::TogglePane(pane));
                }
            }
            if step > 0 {
                ui.vertical(|ui| {
                    for (icon, label, pane) in buttons.into_iter().skip(1) {
                        let label = label.replace("-\n", "");
                        if small_button_mode(ui, p, icon, &label, self.panes.visible(pane), step >= 2).clicked() {
                            cmds.push(Command::TogglePane(pane));
                        }
                    }
                });
            }
        });
        // Видимость
        ribbon_group(ui, p, "Видимость", 1, |ui, _| {
            // Every command here is the view's or the document's: greyed
            // with no document (MFC's auto-disable).
            let has_doc = self.active_doc().is_some();
            // 32862: DIALOG 463 «Отображение каналов».
            if ui.add_enabled_ui(has_doc, |ui| big_button(ui, p, Icon::Channels, "Каналы")).inner.clicked() {
                cmds.push(Command::ChannelsDialog);
            }
            // Small icon-only buttons (the label is the tooltip), three to a
            // column as the original: [32845/46/47] | [32802/03/138]
            // [32800/01/141] [32813/14/139] [32815/16] | [155/157/156].
            let armed = self.rect_zoom != crate::app::RectZoom::Off;
            let shown = |k: usize| self.active_doc().is_some_and(|d| if k < 3 { d.v_shown[k] } else { d.hviz[k - 3].shown });
            let comments_ok = self.active_doc().is_some_and(|d| d.comments_allowed());
            let mode = |k: u8| self.active_doc().is_some_and(|d| d.comment_mode == k);
            let [grow_y, shrink_y, grow_x, shrink_x] = self.zoom_enabled();
            let columns: [&[MiniCmd]; 6] = [
                &[
                    (Mini::ShowDigital, "Показать дискретные", has_doc, false, Command::Discrete(0)),
                    (Mini::HideDigital, "Скрыть дискретные", has_doc, false, Command::Discrete(1)),
                    (Mini::HideUnchanged, "Скрыть неизменяющиеся", has_doc, false, Command::Discrete(2)),
                ],
                &[
                    (Mini::ZoomH(true), "Увел. по гор.", grow_x, false, Command::ZoomX(true)),
                    (Mini::ZoomH(false), "Уменьш. по гор.", shrink_x, false, Command::ZoomX(false)),
                    (Mini::RectZoom, "Приблизить по вертикали", has_doc, armed, Command::RectZoom),
                ],
                &[
                    (Mini::ZoomV(true), "Увел. по верт.", grow_y, false, Command::ZoomY(true)),
                    (Mini::ZoomV(false), "Уменьш. по верт.", shrink_y, false, Command::ZoomY(false)),
                    (Mini::Reset, "Вернуть к исходному размеру", has_doc, false, Command::ResetZoom),
                ],
                &[
                    (Mini::Vizir(0), "Верт. визир 1", has_doc, shown(0), Command::ToggleVizir(0)),
                    (Mini::Vizir(1), "Верт. визир 2", has_doc, shown(1), Command::ToggleVizir(1)),
                    (Mini::Vizir(2), "Верт. визир 3", has_doc, shown(2), Command::ToggleVizir(2)),
                ],
                &[
                    (Mini::Vizir(3), "Гор. визир 1", has_doc, shown(3), Command::ToggleVizir(3)),
                    (Mini::Vizir(4), "Гор. визир 2", has_doc, shown(4), Command::ToggleVizir(4)),
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
        // Steps: the right small column icon-only, then every small one.
        ribbon_group(ui, p, "Вычисления", 3, |ui, step| {
            // 32817 / 32818 / 32819 (CMDUI `0x42fc60`): stats rows on every
            // analog channel — computed and pasted ones too —, disabled with
            // none, checked when all of them show the row; 32820..32822 act
            // on the selected channels, disabled with no selection, checked
            // only when every selected channel is in that mode. Icon-only,
            // as in the original.
            let off = crate::chansel::ItemState { enabled: false, checked: false };
            let stat = |k: usize| self.active_doc().map_or(off, |d| d.stats_row_state(k));
            let mode = |m| self.active_doc().map_or(off, |d| d.mode_state(m));
            let columns: [[(Mini, &str, crate::chansel::ItemState, Command); 3]; 2] = [
                [
                    (Mini::MinMax, "Мин/макс", stat(0), Command::StatsRow(0)),
                    (Mini::Rms, "СКЗ", stat(1), Command::StatsRow(1)),
                    (Mini::A1, "Оценка 1-й гарм.", stat(2), Command::StatsRow(2)),
                ],
                [
                    (Mini::Instant, "Мгновенные", mode(ValueMode::Instant), Command::SetValueMode(ValueMode::Instant)),
                    (Mini::RmsMode, "Среднеквадр.", mode(ValueMode::Rms), Command::SetValueMode(ValueMode::Rms)),
                    (Mini::Harm1, "1-я гармоника", mode(ValueMode::FirstHarmonic), Command::SetValueMode(ValueMode::FirstHarmonic)),
                ],
            ];
            for column in columns {
                ui.vertical(|ui| {
                    for (glyph, tip, st, cmd) in column {
                        if mini_button(ui, p, glyph, tip, st.enabled, st.checked) {
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
                    if ui.add_enabled_ui(s.enabled, |ui| small_button_mode(ui, p, icon, label, s.checked, step >= 2)).inner.clicked() {
                        cmds.push(Command::ToggleFlag(f));
                    }
                }
            });
            ui.vertical(|ui| {
                // 32866 «Коэффициенты»; 32867 / 32868 a radio pair checked
                // from channel 0 (CMDUI `0x42ffb0`; neither with no analog
                // channel); all three the view's — greyed with no document.
                let has_doc = self.active_doc().is_some();
                if ui.add_enabled_ui(has_doc, |ui| small_button_mode(ui, p, Icon::Params, "Коэффициенты", false, step >= 1)).inner.clicked() {
                    cmds.push(Command::Coefficients);
                }
                let prim = self.active_doc().and_then(|d| d.primary_shown());
                if ui.add_enabled_ui(has_doc, |ui| small_button_mode(ui, p, Icon::Primary, "Первичные знач.", prim == Some(true), step >= 1)).inner.clicked() {
                    cmds.push(Command::SetPrimaryValues(true));
                }
                if ui.add_enabled_ui(has_doc, |ui| small_button_mode(ui, p, Icon::Primary, "Вторичные знач.", prim == Some(false), step >= 1)).inner.clicked() {
                    cmds.push(Command::SetPrimaryValues(false));
                }
            });
            let analog = self.active_doc().is_some_and(|d| !d.osc.analog.is_empty());
            if ui.add_enabled_ui(analog, |ui| big_button(ui, p, Icon::Analysis, "Быстрый анализ")).inner.clicked() {
                cmds.push(Command::QuickAnalysis);
            }
        });
        // Осциллограмма
        // The unnamed panel (title " ", «Информация» in the help).
        ribbon_group(ui, p, " ", 1, |ui, _| {
            // 142: toggles the parameters bar, checked while it is shown.
            if big_button_sel(ui, p, Icon::Wave, "Осцилло грамма", self.dlg.param_bar)
                .on_hover_text("Показать или скрыть панель параметров осциллограммы")
                .clicked()
            {
                cmds.push(Command::ToggleParamBar);
            }
        });
    }

    /// «Стиль» and Help on the right of the tab row (§2.5).
    fn tab_row_right(&mut self, ui: &mut egui::Ui, p: crate::theme::Palette, cmds: &mut Vec<Command>) {
        // §2.5: «Стиль» and Help on the right of the tab row.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Help split button (57667): the arrow opens {Помощь, О программе...}.
            let help = ui.button(RichText::new("?").color(p.text).strong());
            if help.clicked() {
                self.open_top_menu(ui.ctx(), TopMenu::Help, help.rect);
            }
            self.kt_rects.push((crate::keytips::Target::Help, help.rect, true));
            // «Стиль» (menu 200): the radio entry of the current look,
            // persisted as `ApplicationLook`.
            let style = ui.button(RichText::new("Стиль ▾").color(p.text));
            if style.clicked() {
                self.open_top_menu(ui.ctx(), TopMenu::Style, style.rect);
            }
        });
        self.top_menu_frame(ui.ctx(), cmds);
    }

    /// Open a tab-row menu under its button, its right edge on the
    /// button's (VM).
    pub(crate) fn open_top_menu(&mut self, ctx: &egui::Context, kind: TopMenu, button: egui::Rect) {
        let width = match kind {
            TopMenu::Style => 160.0,
            TopMenu::Help => 140.0,
        };
        let at = egui::pos2(button.right() - width, button.bottom());
        self.top_menu = Some((kind, crate::popmenu::PopMenu::new(ctx.cumulative_pass_nr(), at)));
    }

    /// The open tab-row menu, as the original's popups.
    fn top_menu_frame(&mut self, ctx: &egui::Context, cmds: &mut Vec<Command>) {
        use crate::popmenu::{Entry, Mark};
        let Some((kind, mut menu)) = self.top_menu else { return };
        if menu.serial == ctx.cumulative_pass_nr() {
            // Not the frame of the click that opened it.
            self.top_menu = Some((kind, menu));
            return;
        }
        #[derive(Clone)]
        enum Pick {
            Skin(Skin),
            About,
            Nothing,
        }
        let entries: Vec<Entry<Pick>> = match kind {
            TopMenu::Style => {
                let mut e: Vec<Entry<Pick>> = Skin::ALL
                    .into_iter()
                    .zip(["Windows 2000", "Office XP", "Windows XP", "Office 2003", "Visual Studio.NET 2005"])
                    .map(|(s, name)| Entry::item(name, true, Mark::Radio(self.skin == s), Pick::Skin(s)))
                    .collect();
                let sub = Skin::ALL[5..]
                    .iter()
                    .copied()
                    .zip(["Голубой стиль", "Черный стиль", "Серебристый стиль", "Зеленовато-голубой стиль"])
                    .map(|(s, name)| Entry::item(name, true, Mark::Radio(self.skin == s), Pick::Skin(s)))
                    .collect();
                e.push(Entry::sub("Office 2007", sub));
                e
            }
            TopMenu::Help => vec![Entry::item("Помощь", false, Mark::None, Pick::Nothing), Entry::Sep, Entry::item("О программе...", true, Mark::None, Pick::About)],
        };
        let out = crate::popmenu::show(ctx, "fv_top_menu", &mut menu, &entries);
        match out.chosen {
            Some(Pick::Skin(s)) => cmds.push(Command::SetSkin(s)),
            Some(Pick::About) => self.show_about = true,
            _ => {}
        }
        self.top_menu = (!out.close).then_some((kind, menu));
    }

    /// MFC's print-preview category (`mfc120u` `0x100fcfc2`): «Печать»
    /// {0xE304 Печать}, «Масштаб» {0xE305 Увеличить, 0xE306 Уменьшить,
    /// 0xE301 Одна / Две страницы}, «Просмотр» {0xE302 Следующая страница,
    /// 0xE303 Предыдущая страница, ─, 0xE300 Закрыть}.
    fn ribbon_preview(&mut self, ui: &mut egui::Ui) {
        use crate::print_ui::PreviewCmd as P;
        let p = self.skin.palette();
        let Some(pv) = self.print.preview.as_ref() else { return };
        let numpage = pv.numpage_label();
        let state: Vec<(P, bool)> = [P::Print, P::ZoomIn, P::ZoomOut, P::NumPage, P::Next, P::Prev, P::Close].into_iter().map(|c| (c, pv.enabled(c))).collect();
        let on = |c: P| state.iter().any(|&(k, e)| k == c && e);
        let fire = std::cell::Cell::new(None);
        let kt = std::cell::RefCell::new(Vec::new());
        let button = |ui: &mut egui::Ui, icon: Icon, label: &str, c: P| {
            let r = ui.add_enabled_ui(on(c), |ui| big_button(ui, p, icon, label)).inner;
            kt.borrow_mut().push((crate::keytips::Target::Preview(c), r.rect, on(c)));
            if r.clicked() {
                fire.set(Some(c));
            }
        };
        ribbon_group(ui, p, "Печать", 1, |ui, _| button(ui, Icon::Print, "Печать", P::Print));
        ribbon_group(ui, p, "Масштаб", 1, |ui, _| {
            button(ui, Icon::ZoomIn, "Увеличить", P::ZoomIn);
            button(ui, Icon::ZoomOut, "Уменьшить", P::ZoomOut);
            button(ui, Icon::Pages, numpage, P::NumPage);
        });
        // «Следующая» / «Предыдущая» have no large image (mfc120u
        // `0x100fd3ad`, `0x100fd437`): small buttons with their text,
        // stacked in one column, before the separator and «Закрыть».
        ribbon_group(ui, p, "Просмотр", 1, |ui, _| {
            ui.vertical(|ui| {
                for (icon, label, c) in [(Icon::NextPage, "Следующая страница", P::Next), (Icon::PrevPage, "Предыдущая страница", P::Prev)] {
                    let r = ui.add_enabled_ui(on(c), |ui| small_button_mode(ui, p, icon, label, false, false)).inner;
                    kt.borrow_mut().push((crate::keytips::Target::Preview(c), r.rect, on(c)));
                    if r.clicked() {
                        fire.set(Some(c));
                    }
                }
            });
            group_separator(ui, p);
            button(ui, Icon::ClosePreview, "Закрыть", P::Close);
        });
        if let (Some(c), Some(pv)) = (fire.get(), self.print.preview.as_mut()) {
            pv.cmd = Some(c);
        }
        self.kt_rects.extend(kt.into_inner());
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
        ribbon_group(ui, p, "ОМП", 1, |ui, _| {
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


/// A vertical gradient through `stops` (y offsets from the top, colours) as
/// one mesh, the last colour down to the bottom.
/// A checked button's ground. The Office 2007 looks (seen on the VM, Blue):
/// a 1 px (147, 135, 108) frame round a two-part gradient — (253, 212, 167)
/// at the top, (249, 151, 50) at 42 %, (252, 230, 156) at the bottom; the
/// older looks their flat selection colour.
fn checked_bg(painter: &egui::Painter, rect: egui::Rect, p: &crate::theme::Palette) {
    if p.caption_band.is_some() && p.accent_soft == Color32::from_rgb(255, 228, 150) {
        let h = rect.height();
        painter.add(gradient_mesh(rect.shrink(1.0), &[(0.0, Color32::from_rgb(253, 212, 167)), (0.42 * h, Color32::from_rgb(249, 151, 50)), (h, Color32::from_rgb(252, 230, 156))]));
        painter.px_rect_stroke(rect, 2.0, egui::Stroke::new(1.0f32, Color32::from_rgb(147, 135, 108)));
    } else {
        painter.rect(rect, 2.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
    }
}

/// A large button under the pointer in the Office 2007 looks (VM,
/// «Каналы» hot): a 1 px border with its corners cut — (221, 207, 155) on
/// top, (210, 208, 188) below, (210, 192, 141) at the sides — a light inner
/// line, and the face from (255, 253, 220) to (255, 231, 148) down to row
/// 25, (255, 215, 81) to (255, 211, 74) and back to (255, 213, 82) down to
/// row 41, then (255, 214, 85) to (255, 231, 153). The older looks: their
/// soft accent box.
fn hot_large(painter: &egui::Painter, rect: egui::Rect, p: &crate::theme::Palette) {
    if !(p.caption_band.is_some() && p.accent_soft == Color32::from_rgb(255, 228, 150)) {
        painter.rect(rect, 3.0f32, p.accent_soft.gamma_multiply(0.5f32), egui::Stroke::new(1.0f32, p.accent_border));
        return;
    }
    let rgb = |c: u32| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
    let face = egui::Rect::from_min_max(rect.min + egui::vec2(2.0, 2.0), rect.max - egui::vec2(2.0, 2.0));
    painter.add(gradient_mesh(face, &[(0.0, rgb(0xfffddc)), (23.0, rgb(0xffe794)), (23.0, rgb(0xffd751)), (31.0, rgb(0xffd34a)), (38.0, rgb(0xffd552)), (39.0, rgb(0xffd655)), (face.height(), rgb(0xffe799))]));
    let (l, r, t, b) = (rect.left(), rect.right() - 1.0, rect.top(), rect.bottom() - 1.0);
    // Pixel columns / rows a..=b as end-exclusive spans.
    painter.px_hline(l + 2.0..=r - 1.0, t + 1.0, egui::Stroke::new(1.0f32, rgb(0xfffff7)));
    painter.px_hline(l + 2.0..=r - 1.0, b - 1.0, egui::Stroke::new(1.0f32, rgb(0xfff89a)));
    painter.px_vline(l + 1.0, t + 1.0..=b, egui::Stroke::new(1.0f32, rgb(0xfff2bd)));
    painter.px_vline(r - 1.0, t + 1.0..=b, egui::Stroke::new(1.0f32, rgb(0xfff2bd)));
    painter.px_hline(l + 1.0..=r, t, egui::Stroke::new(1.0f32, rgb(0xddcf9b)));
    painter.px_hline(l + 1.0..=r, b, egui::Stroke::new(1.0f32, rgb(0xd2d0bc)));
    painter.px_vline(l, t + 1.0..=b, egui::Stroke::new(1.0f32, rgb(0xd2c08d)));
    painter.px_vline(r, t + 1.0..=b, egui::Stroke::new(1.0f32, rgb(0xd2c08d)));
}

fn gradient_mesh(rect: egui::Rect, stops: &[(f32, Color32)]) -> egui::Shape {
    let mut mesh = egui::Mesh::default();
    let mut pts: Vec<(f32, Color32)> = stops.iter().map(|&(y, c)| (rect.top() + y.min(rect.height()), c)).collect();
    if let Some(&(_, c)) = pts.last() {
        pts.push((rect.bottom(), c));
    }
    for w in pts.windows(2) {
        let ((y0, c0), (y1, c1)) = (w[0], w[1]);
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(egui::pos2(rect.left(), y0), c0);
        mesh.colored_vertex(egui::pos2(rect.right(), y0), c0);
        mesh.colored_vertex(egui::pos2(rect.right(), y1), c1);
        mesh.colored_vertex(egui::pos2(rect.left(), y1), c1);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    egui::Shape::mesh(mesh)
}

fn paint_v_gradient(ui: &egui::Ui, rect: egui::Rect, top: Color32, bottom: Color32) {
    // One mesh: strips would leave anti-aliased seams.
    ui.painter().add(gradient_mesh(rect, &[(0.0, top), (rect.height(), bottom)]));
}

/// The Механотроника chevron mark (two stacked carets).
/// The Office 2007 application button (RIBBON_BTN_MAIN, its centre in the
/// normal / hot / pressed states): a round button with a darker rim and a
/// glossy upper half, our logo inside.
fn paint_orb(painter: &egui::Painter, r: egui::Rect, colors: [Color32; 3], state: usize, flat: Option<Color32>) {
    let c = colors[state.min(2)];
    let rad = r.width() / 2.0;
    if let Some(line) = flat {
        // The older looks: the body colour, a 1 px outline, no gloss.
        painter.circle(r.center(), rad - 0.5, c, egui::Stroke::new(1.0f32, line));
    } else {
        let rim = c.lerp_to_gamma(Color32::BLACK, 0.35);
        painter.circle(r.center(), rad, c, egui::Stroke::new(1.5f32, rim));
        // The gloss: a lighter ellipse over the upper half.
        let gloss = crate::theme::alpha(255, 255, 255, 90);
        let top = egui::Rect::from_center_size(r.center() - egui::vec2(0.0, rad * 0.38), egui::vec2(rad * 1.5, rad * 0.9));
        painter.add(egui::Shape::ellipse_filled(top.center(), top.size() / 2.0, gloss));
    }
    // The logo (two chevrons), as `draw_logo`.
    let h = rad * 0.9;
    let lr = egui::Rect::from_center_size(r.center(), egui::vec2(h * 1.2, h));
    let green = Color32::from_rgb(0x4C, 0xAF, 0x50);
    for (yoff, col) in [(0.0_f32, Color32::from_rgb(0x2E, 0x5B, 0x9E)), (h * 0.36, green)] {
        let a = egui::pos2(lr.left(), lr.bottom() - yoff - h * 0.1);
        let b = egui::pos2(lr.center().x, lr.top() + yoff + h * 0.05);
        let cc = egui::pos2(lr.right(), lr.bottom() - yoff - h * 0.1);
        painter.line_segment([a, b], egui::Stroke::new(3.0f32, col));
        painter.line_segment([b, cc], egui::Stroke::new(3.0f32, col));
    }
}

fn draw_logo(ui: &mut egui::Ui, h: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(h * 1.2, h), egui::Sense::hover());
    paint_logo(ui.painter(), rect, color, 2.2);
}

/// Our emblem (the original's logo bitmaps are its artwork): two chevrons,
/// `color` over green, across `rect`.
pub(crate) fn paint_logo(painter: &egui::Painter, rect: egui::Rect, color: Color32, width: f32) {
    let h = rect.height();
    let green = Color32::from_rgb(0x4C, 0xAF, 0x50);
    for (yoff, col) in [(0.0_f32, color), (h * 0.36, green)] {
        let a = egui::pos2(rect.left(), rect.bottom() - yoff - h * 0.1);
        let b = egui::pos2(rect.center().x, rect.top() + yoff + h * 0.05);
        let c = egui::pos2(rect.right(), rect.bottom() - yoff - h * 0.1);
        painter.line_segment([a, b], egui::Stroke::new(width, col));
        painter.line_segment([b, c], egui::Stroke::new(width, col));
    }
}

fn qat_button(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Icon) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
    let painter = ui.painter();
    if resp.hovered() && ui.is_enabled() {
        painter.rect(rect, 3.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
    }
    let box_ = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0));
    icons::paint(painter, box_, icon, if ui.is_enabled() { p.accent } else { disabled_ink(p) });
    resp
}

impl App {
    /// The round application button over the caption and tab rows: room
    /// for it in the tab row, its lower part a hit area that opens the
    /// menu, drawn normal / hot / open.
    fn orb_ui(&mut self, ui: &mut egui::Ui, orb: Option<(egui::Rect, [Color32; 3])>, p: crate::theme::Palette) {
        let Some((r, colors)) = orb else { return };
        ui.add_space(r.right() - ui.cursor().left() + 4.0);
        let ctx = ui.ctx().clone();
        let hit = ui.interact(r, ui.id().with("fv_app_orb"), egui::Sense::click());
        if hit.clicked() {
            self.kt_menu(&ctx, true);
        }
        let open = self.kt_bar.is_some_and(|b| {
            let st = egui::menu::BarState::load(&ctx, b.0);
            let root: &Option<egui::menu::MenuRoot> = &st;
            root.is_some()
        });
        let hot = ctx.input(|i| i.pointer.hover_pos()).is_some_and(|q| q.distance(r.center()) <= r.width() / 2.0);
        paint_orb(&ui.painter().clone().with_layer_id(egui::LayerId::new(egui::Order::Middle, egui::Id::new("fv_app_orb_layer"))), r, colors, if open { 2 } else if hot { 1 } else { 0 }, p.orb_flat);
    }
}

/// The ribbon's own small menus: MFC's one-item ribbon menu (a right-click
/// on the ribbon or the QAT, or the customise arrow) and the QAT print
/// button's list.
#[derive(Clone, Copy, PartialEq)]
enum QatMenu {
    Minimize,
    Print,
}

fn qat_menu_id() -> egui::Id {
    egui::Id::new("fv_ribbon_qat_menu")
}

fn open_min_menu(ui: &egui::Ui, at: egui::Pos2) {
    open_qat_menu(ui, QatMenu::Minimize, at);
}

fn open_qat_menu(ui: &egui::Ui, menu: QatMenu, at: egui::Pos2) {
    let frame = ui.ctx().cumulative_pass_nr();
    ui.data_mut(|d| d.insert_temp(qat_menu_id(), (menu, at, frame)));
}

impl App {
    /// The open small menu, if any. «Свернуть ленту» is MFC's own item
    /// (mfc120rus 17086; «Minimize the Ribbon» under an English Windows,
    /// as on the VM), checked while minimised; the QAT customise items are
    /// not offered. Commands chosen go to `cmds`.
    fn qat_menu(&mut self, ctx: &egui::Context, cmds: &mut Vec<Command>) {
        let Some((menu, at, opened)) = ctx.data(|d| d.get_temp::<(QatMenu, egui::Pos2, u64)>(qat_menu_id())) else { return };
        let mut chosen = false;
        let min = self.ribbon_minimized;
        let area = egui::Area::new(qat_menu_id()).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            crate::popmenu::mfc_frame_with(ui, menu == QatMenu::Minimize, |ui| match menu {
                QatMenu::Minimize => {
                    ui.set_min_width(150.0);
                    if crate::popmenu::mark_item_hot(ui, "Свернуть ленту", true, min.then_some("✓"), false).clicked() {
                        cmds.push(Command::ToggleRibbonMin);
                        chosen = true;
                    }
                }
                QatMenu::Print => {
                    ui.set_min_width(276.0);
                    crate::popmenu::title_band(ui, "Предварительный просмотр и печать документа");
                    for (label, desc, cmd, icon) in [
                        ("Параметры страницы", "Изменение параметров страницы", Command::PageSetup, Icon::Pages),
                        ("Предварительный просмотр", "Просмотр документа в полностраничном режиме", Command::PrintPreview, Icon::ZoomIn),
                        ("Печать", "Печать активного документа", Command::Print, Icon::Print),
                    ] {
                        let paint = move |p: &egui::Painter, r: egui::Rect, c: Color32| icons::paint(p, r, icon, c);
                        if crate::popmenu::large_item(ui, label, desc, true, &paint).clicked() {
                            cmds.push(cmd);
                            chosen = true;
                        }
                    }
                }
            });
        });
        let fresh = ctx.cumulative_pass_nr() == opened;
        let away = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|q| !area.response.rect.contains(q)));
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if chosen || esc || (away && !fresh) {
            ctx.data_mut(|d| d.remove::<(QatMenu, egui::Pos2, u64)>(qat_menu_id()));
        }
    }
}

/// The QAT's small drop-down arrow: the print split part, or the customise
/// arrow (a bar over the triangle).
fn qat_arrow(ui: &mut egui::Ui, p: crate::theme::Palette, customise: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(if customise { 13.0 } else { 11.0 }, 22.0), egui::Sense::click());
    let painter = ui.painter();
    if resp.hovered() && ui.is_enabled() {
        painter.rect(rect, 3.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
    }
    let ink = if ui.is_enabled() { p.text } else { disabled_ink(p) };
    let c = rect.center();
    if customise {
        painter.line_segment([egui::pos2(c.x - 3.0, c.y - 2.5), egui::pos2(c.x + 3.0, c.y - 2.5)], egui::Stroke::new(1.0f32, ink));
    }
    let y = if customise { c.y } else { c.y - 1.0 };
    painter.add(egui::Shape::convex_polygon(vec![egui::pos2(c.x - 3.0, y), egui::pos2(c.x + 3.0, y), egui::pos2(c.x, y + 3.0)], ink, egui::Stroke::NONE));
    resp
}

/// A category tab: the active one on its light fill in `text`
/// (TextHighlighted), the others on the band in the inactive colour.
/// A category tab. The active one as the VM shows it (Office 2007 blue):
/// 23 px tall, a 1 px (141, 178, 227) frame with rounded top corners, open
/// at the bottom into the body, filled (244, 249, 255) → (222, 232, 245);
/// the label in the 9 pt UI font, 12 px of padding each side; an inactive
/// tab is its label alone.
fn ribbon_tab(ui: &mut egui::Ui, label: &str, active: bool, p: crate::theme::Palette) -> egui::Response {
    let font = egui::FontId::proportional(12.0);
    let text = if active { p.text } else { p.tab_text };
    let tw = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), text).size().x);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2((tw + 24.0).round(), 23.0), egui::Sense::click());
    let painter = ui.painter();
    let blue = p.group_bg == Color32::from_rgb(223, 233, 246);
    if active {
        let (top, bottom) = if blue { (Color32::from_rgb(244, 249, 255), Color32::from_rgb(222, 232, 245)) } else { (Color32::WHITE.lerp_to_gamma(p.tab_active, 0.5), p.tab_active) };
        let frame = if blue { Color32::from_rgb(141, 178, 227) } else { p.group_border };
        let rounding = egui::Rounding { nw: 3.0, ne: 3.0, sw: 0.0, se: 0.0 };
        painter.rect_filled(rect, rounding, bottom);
        painter.add(gradient_mesh(rect.shrink2(egui::vec2(1.0, 0.0)).with_min_y(rect.top() + 3.0), &[(0.0, top), (rect.height() - 3.0, bottom)]));
        painter.px_rect_stroke(rect.with_max_y(rect.bottom() + 2.0), 3.0, egui::Stroke::new(1.0f32, frame));
        // Open at the bottom: the body's colour over the frame's lower edge.
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left() + 1.0, rect.bottom()), egui::pos2(rect.right() - 1.0, rect.bottom() + 3.0)), 0.0, bottom);
    } else if resp.hovered() {
        painter.rect(rect, egui::Rounding { nw: 3.0, ne: 3.0, sw: 0.0, se: 0.0 }, p.tab_active.gamma_multiply(0.4f32), egui::Stroke::NONE);
    }
    painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, font, text);
    resp
}

/// A ribbon group: content row/area + centered caption + right divider.
/// The panels of each category, left to right.
/// Each panel: title, its layouts before collapsing, a single element.
const HOME_GROUPS: [(&str, usize, bool); 5] = [("Объединение", 2, false), ("Диаграммы", 3, false), ("Видимость", 1, false), ("Вычисления", 3, false), (" ", 1, true)];
const OMP_GROUPS: [(&str, usize, bool); 1] = [("ОМП", 1, false)];
fn group_width_id(title: &str, step: usize) -> egui::Id {
    egui::Id::new(("fv_ribbon_group_w", title, step))
}

fn group_step_id(title: &str) -> egui::Id {
    egui::Id::new(("fv_ribbon_group_step", title))
}

/// While set, panels draw this layout step and record their widths.
fn measure_id() -> egui::Id {
    egui::Id::new("fv_ribbon_measure")
}

/// A collapsed panel's button width plus the divider: the default panel
/// button (`0x10120c93`: a 16 px image, the 9 px arrow always counted) is
/// max(16 + 2 + 10, T + 5) + 2, T its balanced bottom text.
fn collapsed_width(ui: &egui::Ui, title: &str) -> f32 {
    let font = egui::FontId::proportional(10.5);
    let (_, t) = bottom_text(title, 9.0, |s| ui.fonts(|f| f.layout_no_wrap(s.to_owned(), font.clone(), Color32::BLACK).size().x));
    large_width(18.0, t) + 2.0 + 6.0
}

/// The category's reduction (`CMFCRibbonCategory::RecalcLayout`
/// `0x10106b37`): `widths[i]` are panel i's layouts, its collapsed button
/// last when it can collapse. Every panel starts at its first layout; when
/// even all the last layouts do not fit, all take them at once; otherwise,
/// while the panels overflow, the one with the most steps left takes one
/// (ties: the rightmost). Returns each panel's layout index. The fit test
/// (`0x10106fcb`): Σ(panel + 2) ≤ the category − 8 (a panel's width here
/// already holds its 2 × 4 margins); all collapse when the category − 8 ≤
/// 2 + Σ(collapsed + 2).
pub fn reduction_plan(widths: &[Vec<f32>], avail: f32) -> Vec<usize> {
    let last = |w: &Vec<f32>| w.len().saturating_sub(1);
    let total = |plan: &[usize]| widths.iter().zip(plan).map(|(w, &k)| w.get(k).copied().unwrap_or(0.0) + 2.0).sum::<f32>();
    let mut plan = vec![0; widths.len()];
    if avail - 8.0 <= 2.0 + widths.iter().map(|w| w.last().copied().unwrap_or(0.0) + 2.0).sum::<f32>() {
        return widths.iter().map(last).collect();
    }
    while total(&plan) > avail - 8.0 {
        let left = |i: usize| last(&widths[i]) - plan[i];
        let Some(i) = (0..widths.len()).filter(|&i| left(i) > 0).max_by_key(|&i| (left(i), i)) else { break };
        plan[i] += 1;
    }
    plan
}

/// A panel with `layouts` reduction steps; `content` draws step k.
fn ribbon_group<R>(ui: &mut egui::Ui, p: crate::theme::Palette, title: &str, layouts: usize, content: impl FnOnce(&mut egui::Ui, usize) -> R) {
    let measuring = ui.data(|d| d.get_temp::<usize>(measure_id()));
    let step = match measuring {
        Some(k) => k.min(layouts - 1),
        None => ui.data(|d| d.get_temp::<usize>(group_step_id(title))).unwrap_or(0),
    };
    if step >= layouts {
        // One button with the panel's name, the panel's whole height
        // (`0x10124eab`); the panel opens below it. Office 2007 draws a
        // 38 × 38 frame from the top + 3, the 16 px image at + 10 and the
        // text from + 40 with the arrow (`0x10193c7b`).
        let font = egui::FontId::proportional(10.5);
        let (lines, _) = bottom_text(title, 9.0, |s| ui.fonts(|f| f.layout_no_wrap(s.to_owned(), font.clone(), p.text).size().x));
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(collapsed_width(ui, title) - 6.0, 84.0), egui::Sense::click());
        let painter = ui.painter();
        if resp.hovered() {
            painter.rect(rect, 3.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
        }
        let cx = rect.center().x.round();
        let frame = egui::Rect::from_min_size(egui::pos2(cx - 19.0, rect.top() + 3.0), egui::vec2(38.0, 38.0));
        painter.rect(frame, 3.0f32, p.group_bg, egui::Stroke::new(1.0f32, p.group_border));
        painter.rect_stroke(egui::Rect::from_min_size(egui::pos2(cx - 8.0, rect.top() + 10.0), egui::vec2(16.0, 16.0)).shrink(2.0), 1.0f32, egui::Stroke::new(1.0f32, p.accent));
        for (i, line) in lines.iter().enumerate() {
            painter.text(egui::pos2(cx, rect.top() + 40.0 + i as f32 * LINE_H), egui::Align2::CENTER_TOP, line, font.clone(), p.text);
        }
        // The arrow after the last line (on line 2 when the text is one line).
        let last = lines.last().map(|l| ui.fonts(|f| f.layout_no_wrap(l.clone(), font.clone(), p.text).size().x)).unwrap_or(0.0);
        let (ax, ay) = if lines.len() > 1 { (cx + last / 2.0 + 5.0, rect.top() + 40.0 + LINE_H + 2.0) } else { (cx, rect.top() + 40.0 + LINE_H + 2.0) };
        painter.text(egui::pos2(ax, ay), egui::Align2::CENTER_TOP, "▾", egui::FontId::proportional(9.0), p.text);
        let popup = egui::Id::new(("fv_ribbon_group_popup", title));
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup));
        }
        egui::popup_below_widget(ui, popup, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.horizontal_top(|ui| {
                content(ui, 0);
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
    ribbon_group_expanded(ui, p, title, |ui| content(ui, step));
    let w = ui.cursor().left() - start + ui.spacing().item_spacing.x;
    if measuring.is_some() {
        ui.data_mut(|d| d.insert_temp(group_width_id(title, step), w));
    }
}

fn ribbon_group_expanded<R>(ui: &mut egui::Ui, p: crate::theme::Palette, title: &str, content: impl FnOnce(&mut egui::Ui) -> R) {
    // The panel's caption band and frame, behind its content (set below).
    let bg = ui.painter().add(egui::Shape::Noop);
    let panel = ui.scope(|ui| {
        ui.vertical(|ui| {
            // Content then a 16 px caption, touching (VM: the large
            // buttons 66 px from the panel's top + 2, the caption band
            // right under them).
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::Frame::none()
                .inner_margin(egui::Margin { left: 4.0, right: 4.0, top: 2.0, bottom: 0.0 })
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // MFC packs a panel's elements edge to edge (the
                        // diagram buttons touch; small buttons on a 22 px
                        // pitch — seen on the VM).
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                        content(ui);
                    });
                });
            // The caption in the 9 pt UI font (12 px), centred.
            let w = ui.fonts(|f| f.layout_no_wrap(title.to_owned(), egui::FontId::proportional(12.0), p.caption_text).size().x);
            let (cap, _) = ui.allocate_exact_size(egui::vec2(ui.min_rect().width().max(w + 8.0).max(40.0), 16.0), egui::Sense::hover());
            ui.painter().text(cap.center(), egui::Align2::CENTER_CENTER, title, egui::FontId::proportional(12.0), p.caption_text);
        });
    });
    // Office 2007 (RIBBON_PANEL_BACK_T / _B): a white inner line, the
    // caption band 15 px at the bottom.
    if let Some([top, bottom]) = p.caption_band {
        let r = panel.response.rect;
        let band = egui::Rect::from_min_max(egui::pos2(r.left(), r.bottom() - 16.0), r.max);
        ui.painter().set(
            bg,
            egui::Shape::Vec(vec![
                egui::Shape::rect_stroke(r.shrink(1.0), 2.0f32, egui::Stroke::new(1.0f32, crate::theme::alpha(255, 255, 255, 130))),
                gradient_mesh(band, &[(0.0, top), (16.0, bottom)]),
            ]),
        );
    }
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
    // The 9 pt UI font is Segoe UI's 12 px, between Tahoma's 11 and 12 for
    // these words; 11 (10.5 rounds up) lays the groups out closest (VM:
    // «Диаграммы» 232 px against 242; 12 gives 262).
    let font = egui::FontId::proportional(10.5);
    let measure = |t: &str| ui.fonts(|f| f.layout_no_wrap(t.to_owned(), font.clone(), p.text).size().x);
    let (lines, text_w) = bottom_text(label, 0.0, measure);
    let size = egui::vec2(large_width(32.0, text_w), LARGE_H);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = &plain_painter(ui);
    let (ink, accent) = inks(ui, p);
    // The checked fill is light in every look: dark ink on it.
    let ink = if selected && ui.is_enabled() { CHECKED_INK } else { ink };
    let hovered = resp.hovered() && ui.is_enabled();
    if selected {
        checked_bg(painter, rect, &p);
    } else if hovered {
        hot_large(painter, rect, &p);
    }
    // The 32 px image centred from the top + 3, the lines from + 38
    // (`OnDraw` `0x10100cae`); our drawings fill 28 px of it.
    let img = egui::Rect::from_min_size(egui::pos2((rect.center().x - 16.0).round(), rect.top() + 3.0), egui::vec2(32.0, 32.0));
    icons::paint(painter, img.shrink(2.0), icon, accent);
    for (i, line) in lines.iter().enumerate() {
        painter.text(egui::pos2(rect.center().x, rect.top() + 38.0 + i as f32 * LINE_H), egui::Align2::CENTER_TOP, line, font.clone(), ink);
    }
    resp
}

/// A large button's height: the 32 px image + 1 + max(two text lines,
/// 33) (`GetRegularSize` `0x10101ab9`), 66 at our line height.
const LARGE_H: f32 = 66.0;
/// The bottom text's line height.
const LINE_H: f32 = 12.0;

/// A large button's width (`0x10101ab9`, margin (5, 1) in large mode):
/// max(image + 2 × 5, text + 5).
pub fn large_width(image: f32, text_w: f32) -> f32 {
    (image + 10.0).max(text_w + 5.0)
}

/// A large button's bottom text (`OnCalcTextSize` `0x10100a07`): one line
/// without a space (its width, the arrow not counted); else the break at
/// the space that minimises max(line 1, line 2 + arrow), the width rounded
/// down to even. `arrow` the menu arrow (9 px) when the button has one.
pub fn bottom_text(text: &str, arrow: f32, measure: impl Fn(&str) -> f32) -> (Vec<String>, f32) {
    let spaces: Vec<usize> = text.match_indices(' ').map(|(i, _)| i).collect();
    if spaces.is_empty() {
        return (vec![text.to_string()], measure(text));
    }
    let (w, at) = spaces.iter().map(|&i| (measure(&text[..i]).max(measure(&text[i + 1..]) + arrow), i)).fold((f32::MAX, 0), |b, c| if c.0 < b.0 { c } else { b });
    (vec![text[..at].to_string(), text[at + 1..].to_string()], ((w as i32) & !1) as f32)
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
/// An application-menu item: FastView gives each its large image
/// (`0x473079`: small and large index n), so the row carries a 32 px image
/// with the text on its right.
/// Rows 44 px (max(text, 34) + 2 × 5, `0x1011cea4`), the image at the
/// left + 3 centred, the text from + 38 on one line with an end ellipsis
/// (`0x101012f1`).
fn main_entry(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Option<Icon>, label: &str, enabled: bool, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let v = ui.visuals();
    if selected || (enabled && resp.hovered()) {
        ui.painter().rect_filled(rect, 2.0f32, v.widgets.hovered.weak_bg_fill);
    }
    let ink = if enabled { v.text_color() } else { disabled_ink(p) };
    if let Some(icon) = icon {
        let img = egui::Rect::from_min_size(egui::pos2(rect.left() + 3.0, rect.center().y - 16.0), egui::vec2(32.0, 32.0));
        crate::icons::paint(ui.painter(), img.shrink(4.0), icon, if enabled { p.text } else { disabled_ink(p) });
    }
    let font = egui::FontId::proportional(12.0);
    let room = rect.right() - rect.left() - 38.0 - 4.0;
    let galley = ui.fonts(|f| {
        let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font.clone(), ink);
        job.wrap = egui::text::TextWrapping::truncate_at_width(room);
        f.layout_job(job)
    });
    ui.painter().galley(egui::pos2(rect.left() + 38.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    resp
}

fn sub_entry(ui: &mut egui::Ui, label: &str, desc: &str, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let v = ui.visuals();
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 2.0f32, v.widgets.hovered.weak_bg_fill);
    }
    let (strong, weak) = if enabled { (v.strong_text_color(), v.text_color()) } else { (v.weak_text_color(), v.weak_text_color()) };
    ui.painter().text(rect.left_top() + egui::vec2(6.0, 3.0), egui::Align2::LEFT_TOP, label, egui::FontId::proportional(12.0), strong);
    ui.painter().text(rect.left_top() + egui::vec2(6.0, 20.0), egui::Align2::LEFT_TOP, desc, egui::FontId::proportional(12.0), weak);
    resp
}

/// A small button with its text, or icon-only (a reduced panel; the text
/// becomes the tooltip).
fn small_button_mode(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Icon, label: &str, selected: bool, icon_only: bool) -> egui::Response {
    let font = egui::FontId::proportional(11.0);
    let text_w = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), p.text).size().x);
    // `0x10101d6a` / `0x10101f92`: icon-only (image + 6)², with text the
    // text + image + 10 wide, max(16, image, text) + 6 tall.
    let w = if icon_only { 22.0 } else { text_w + 16.0 + 10.0 };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::click());
    let resp = if icon_only { resp.on_hover_text(label) } else { resp };
    let painter = &plain_painter(ui);
    let (ink, accent) = inks(ui, p);
    let ink = if selected && ui.is_enabled() { CHECKED_INK } else { ink };
    if selected {
        checked_bg(painter, rect, &p);
    } else if resp.hovered() && ui.is_enabled() {
        painter.rect(rect, 2.0f32, p.accent_soft.gamma_multiply(0.7f32), egui::Stroke::new(1.0f32, p.accent_border));
    }
    let box_ = egui::Rect::from_min_size(egui::pos2(rect.left() + 3.0, rect.top() + 3.0), egui::vec2(16.0, 16.0)).shrink(1.0);
    icons::paint(painter, box_, icon, accent);
    if !icon_only {
        painter.text(egui::pos2(rect.left() + 22.0, rect.center().y), egui::Align2::LEFT_CENTER, label, font, ink);
    }
    resp
}

/// A separator inside a panel, as tall as the button rows (an egui
/// separator would take the whole height and push the panel title out).
fn group_separator(ui: &mut egui::Ui, p: crate::theme::Palette) {
    // As tall as the large buttons: a taller one stretched its panel.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, LARGE_H), egui::Sense::hover());
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

/// A disabled label: the Office 2007 looks' fixed TextDisabled (141, 141,
/// 141; Aqua 159, 141, 154 — `0x10193a84`), the older looks the text
/// colour at half strength.
fn disabled_ink(p: crate::theme::Palette) -> Color32 {
    p.disabled.unwrap_or(p.text.gamma_multiply(0.5f32))
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
/// The tab row's own menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopMenu {
    Style,
    Help,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mini {
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
    // Icon-only: (small image + 6)² (`0x10101d6a`).
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let painter = &plain_painter(ui);
    let enabled = enabled && ui.is_enabled();
    if on {
        checked_bg(painter, rect, &p);
    } else if enabled && resp.hovered() {
        painter.rect(rect, 2.0f32, p.accent_soft.gamma_multiply(0.7f32), egui::Stroke::new(1.0f32, p.accent_border));
    }
    let col = if enabled { p.accent } else { disabled_ink(p) };
    paint_mini(painter, rect.shrink(4.0), glyph, col);
    resp.on_hover_text(tip).clicked() && enabled
}

pub(crate) fn paint_mini(painter: &egui::Painter, r: egui::Rect, glyph: Mini, col: Color32) {
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

// ---------------- keytips (L9.90) ----------------

impl App {
    /// This frame's keytip keys (`crate::keytips`): F10 (Alt alone never
    /// reaches the ribbon, `0x448e80`), then — while shown — Esc, Space and typed letters or digits. True
    /// when the keytips took the keyboard (no shortcuts then).
    pub fn keytips_input(&mut self, ctx: &egui::Context) -> bool {
        use crate::keytips::{Outcome, Target};
        let was = self.keytips.shown();
        let (mods, keys, pressed) = ctx.input(|i| {
            let keys: Vec<egui::Key> = i.events.iter().filter_map(|e| if let egui::Event::Key { key, pressed: true, .. } = e { Some(*key) } else { None }).collect();
            (i.modifiers, keys, i.pointer.any_pressed())
        });
        let f10 = keys.contains(&egui::Key::F10) && !mods.shift && !mods.ctrl;
        if f10 {
            self.keytips.f10();
        }
        // Losing the focus (a click anywhere) removes them.
        if pressed {
            self.keytips.exit();
        }
        // A letter with Alt held is a system key: MFC passes it on and
        // never fires a keytip (`0x100ff240`).
        if was && self.keytips.shown() && !f10 && !mods.alt {
            for key in keys {
                let out = match key {
                    egui::Key::Escape => self.keytips.escape(),
                    egui::Key::Space => {
                        self.keytips.exit();
                        Outcome::Exit
                    }
                    k => match k.name().chars().next().filter(|c| k.name().len() == 1 && c.is_ascii_alphanumeric()) {
                        Some(c) => {
                            let rects = &self.kt_rects;
                            self.keytips.key(c, |t| rects.iter().find(|(x, ..)| *x == t).map(|r| r.2))
                        }
                        None => Outcome::None,
                    },
                };
                match out {
                    Outcome::Back => self.kt_menu(ctx, false),
                    Outcome::Activate(t) => {
                        match t {
                            Target::AppButton => self.kt_open_menu = true,
                            Target::Qat(n) => self.dispatch([Command::Open, Command::Save, Command::Print][n.min(2)].clone()),
                            Target::PrintMain => {
                                self.kt_menu(ctx, false);
                                self.dispatch(Command::Print);
                            }
                            Target::PrintMenu => {
                                let frame = ctx.cumulative_pass_nr();
                                ctx.data_mut(|d| d.insert_temp(app_sub_id(), (Some(AppSub::Print), frame.saturating_sub(1))));
                            }
                            // The Help button has a menu and no separate menu
                            // key: its keytip opens the drop-down (`OnKey`
                            // `0x101036b6`); the menu's own keys are Cyrillic.
                            Target::Help => {
                                let r = self.kt_rects.iter().find(|(t, ..)| *t == Target::Help).map(|(_, r, _)| *r).unwrap_or(egui::Rect::ZERO);
                                self.open_top_menu(ctx, TopMenu::Help, r);
                            }
                            Target::Recent(n) => {
                                if let Some(path) = self.mru.get(n).cloned() {
                                    self.kt_menu(ctx, false);
                                    self.dispatch(Command::OpenPath(path));
                                }
                            }
                            Target::Preview(c) => {
                                if let Some(pv) = self.print.preview.as_mut() {
                                    pv.cmd = Some(c);
                                }
                            }
                            // The Cyrillic menu and tab keys cannot be typed;
                            // the preview tab only descends.
                            Target::AppItem(_) | Target::Tab(_) | Target::PreviewTab => {}
                        }
                        break;
                    }
                    Outcome::Exit => break,
                    Outcome::None => {}
                }
            }
        }
        was || self.keytips.shown()
    }

    /// Open or close the application menu (its egui bar state).
    fn kt_menu(&mut self, ctx: &egui::Context, open: bool) {
        // A root shows for the button whose response id it carries (last
        // frame's, stable while the layout is).
        let Some((bar, button, id)) = self.kt_bar else { return };
        let mut state = egui::menu::BarState::load(ctx, bar);
        let root: &mut Option<egui::menu::MenuRoot> = &mut state;
        *root = open.then(|| egui::menu::MenuRoot::new(button.left_bottom(), id));
        state.store(ctx, bar);
    }

    /// Draw the shown level's keytips on their elements: the keys upper-
    /// cased in a box the size of the larger of them and «O» plus 10 × 2
    /// (`0x10064ea3`), placed per element (`keytips::tip_origin`) and kept on
    /// the screen; the Office 2007 looks draw the style's gradient box, a
    /// disabled element's at alpha 128 (`0x10182927`); the others the
    /// tooltip colours (`0x1017b177`).
    pub fn keytips_paint(&self, ctx: &egui::Context) {
        let Some(level) = self.keytips.level else { return };
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("fv_keytips")));
        let font = egui::FontId::proportional(11.0);
        let o = painter.layout_no_wrap("O".into(), font.clone(), Color32::BLACK).size();
        let look = crate::keytips::office2007_box(self.skin);
        for (keys, target) in crate::keytips::tips(level) {
            let Some(&(_, r, enabled)) = self.kt_rects.iter().find(|(t, ..)| *t == target) else { continue };
            // The text: the style's tooltip ink (76,76,76); a disabled
            // element's whole window at alpha 128, so its ink too (VM: the
            // button shows through); COLOR_INFOTEXT else.
            let ink = match (look.is_some(), enabled) {
                (true, true) => Color32::from_gray(76),
                (true, false) => Color32::from_gray(76).gamma_multiply(0.5),
                (false, true) => Color32::BLACK,
                (false, false) => Color32::from_gray(150),
            };
            let g = painter.layout_no_wrap(crate::keytips::Keytips::label(keys), font.clone(), ink);
            let size = egui::vec2(g.size().x.max(o.x) + 10.0, g.size().y.max(o.y) + 2.0).round();
            let at = crate::keytips::tip_origin(target, r, size);
            let bx = crate::keytips::into_screen(egui::Rect::from_min_size(at, size), ctx.screen_rect());
            match look {
                Some((top, bottom, border)) => {
                    let a = if enabled { 1.0 } else { 0.5 };
                    crate::lanes::gradient(&painter, bx.shrink(1.0), top.gamma_multiply(a), bottom.gamma_multiply(a));
                    // The border with its corner pixels clipped.
                    let s = egui::Stroke::new(1.0f32, border.gamma_multiply(a));
                    painter.hline(bx.left() + 1.0..=bx.right() - 1.0, bx.top() + 0.5, s);
                    painter.hline(bx.left() + 1.0..=bx.right() - 1.0, bx.bottom() - 0.5, s);
                    painter.vline(bx.left() + 0.5, bx.top() + 1.0..=bx.bottom() - 1.0, s);
                    painter.vline(bx.right() - 0.5, bx.top() + 1.0..=bx.bottom() - 1.0, s);
                }
                None => {
                    painter.rect(bx, 0.0, Color32::from_rgb(255, 255, 225), egui::Stroke::new(1.0f32, Color32::BLACK));
                }
            }
            painter.galley(bx.center() - g.size() / 2.0, g, ink);
        }
    }
}

/// The application button's title (the menu id is derived from it).
pub const APP_MENU_TITLE: &str = "Файл ▾";

/// A caption button's glyph in its 10 px box (VM, Windows 11): close an
/// ×; maximise a square; restore a 8 px square with the corner of a
/// second one 2 px up and right; minimise a 10 px line.
fn caption_glyph(p: &egui::Painter, r: egui::Rect, k: u8, max: bool, col: Color32) {
    let o = egui::pos2((r.center().x - 5.0).round(), (r.center().y - 5.0).round());
    // A 1 px run from (x0, y0) to (x1, y1) inclusive, in the box.
    let run = |x0: f32, y0: f32, x1: f32, y1: f32| {
        p.rect_filled(egui::Rect::from_min_max(o + egui::vec2(x0, y0), o + egui::vec2(x1 + 1.0, y1 + 1.0)), 0.0, col);
    };
    let square = |x: f32, y: f32, n: f32| {
        run(x, y, x + n, y);
        run(x, y + n, x + n, y + n);
        run(x, y, x, y + n);
        run(x + n, y, x + n, y + n);
    };
    match (k, max) {
        (0, _) => {
            let s = egui::Stroke::new(1.0f32, col);
            p.line_segment([o, o + egui::vec2(10.0, 10.0)], s);
            p.line_segment([o + egui::vec2(10.0, 0.0), o + egui::vec2(0.0, 10.0)], s);
        }
        (1, false) => square(0.0, 0.0, 9.0),
        (1, true) => {
            square(0.0, 2.0, 7.0);
            run(2.0, 0.0, 9.0, 0.0);
            run(9.0, 0.0, 9.0, 7.0);
        }
        _ => run(0.0, 5.0, 9.0, 5.0),
    }
}

/// The system's caption buttons as Windows 10 / 11 draw them on the DWM
/// frame (FastView's caption is DWM's with composition on, L9.150):
/// minimise, maximise / restore, close (red when hot), 46 px wide each.
fn caption_buttons(ui: &mut egui::Ui) {
    let h = ui.available_height().max(22.0);
    let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    // The buttons edge to edge (VM: 46 px apart).
    ui.spacing_mut().item_spacing.x = 0.0;
    for k in 0..3 {
        let (r, resp) = ui.allocate_exact_size(egui::vec2(46.0, h), egui::Sense::click());
        let close = k == 0;
        let hot = resp.hovered();
        if hot {
            ui.painter().rect_filled(r, 0.0, if close { Color32::from_rgb(232, 17, 35) } else { Color32::from_black_alpha(26) });
        }
        let col = if close && hot { Color32::WHITE } else { Color32::BLACK };
        caption_glyph(ui.painter(), r, k, max, col);
        if resp.clicked() {
            ui.ctx().send_viewport_cmd(match k {
                0 => egui::ViewportCommand::Close,
                1 => egui::ViewportCommand::Maximized(!max),
                _ => egui::ViewportCommand::Minimized(true),
            });
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
            // Selected text (a pane tab, a list row) on the selection fill.
            let sel = p.accent_soft.gamma_multiply(0.9f32);
            assert!((lum(p.text) - lum(over(sel, body))).abs() > 60.0, "{skin:?}: selected");
        }
    }

    /// `OnCalcTextSize` `0x10100a07`: the balanced break, rounded to even.
    #[test]
    fn large_button_text_breaks_at_the_balanced_space() {
        let m = |s: &str| s.chars().count() as f32 * 7.0;
        assert_eq!(bottom_text("Каналы", 0.0, m), (vec!["Каналы".to_string()], 42.0));
        assert_eq!(bottom_text("Дифферен циальная", 0.0, m), (vec!["Дифферен".to_string(), "циальная".to_string()], 56.0));
        // Two spaces: the break that keeps the longer line shortest.
        assert_eq!(bottom_text("Две страницы рядом", 0.0, m).0, ["Две страницы", "рядом"]);
        // The arrow counts on line 2; the width rounds down to even.
        assert_eq!(bottom_text("Ab cde", 9.0, m), (vec!["Ab".to_string(), "cde".to_string()], 30.0));
        assert_eq!(large_width(32.0, 20.0), 42.0, "at least the image + 2 × 5");
        assert_eq!(large_width(32.0, 56.0), 61.0);
    }

    #[test]
    fn panels_reduce_by_most_steps_left_rightmost_first() {
        // Layout widths per panel, the collapsed button last.
        let w = vec![vec![190.0, 150.0, 60.0], vec![270.0, 200.0, 150.0, 60.0], vec![300.0, 60.0], vec![80.0]];
        let full: f32 = 8.0 + [190.0, 270.0, 300.0, 80.0].iter().map(|x| x + 2.0).sum::<f32>();
        assert_eq!(reduction_plan(&w, full), vec![0, 0, 0, 0], "fits");
        // One step short: the panel with the most steps left (1) reduces.
        assert_eq!(reduction_plan(&w, full - 10.0), vec![0, 1, 0, 0]);
        // Then 0 and 1 tie at two steps left: the rightmost (1) again.
        assert_eq!(reduction_plan(&w, full - 80.0), vec![0, 2, 0, 0]);
        assert_eq!(reduction_plan(&w, full - 150.0), vec![1, 2, 0, 0]);
        // Below every last layout: all at once.
        assert_eq!(reduction_plan(&w, 200.0), vec![2, 3, 1, 0]);
    }
}

/// A recent file's label (MFC's `CRecentFileList::GetDisplayName`): just
/// its name when it lies in the current folder, else its path through
/// `AbbreviateName` to 30 characters (VM: «C:\Users\...\WIN_1999.cfg»).
fn mru_display(path: &std::path::Path, cwd: Option<&std::path::Path>) -> String {
    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if path.parent().is_some_and(|p| cwd.is_some_and(|c| p == c)) {
        return name;
    }
    let full = path.to_string_lossy();
    let full = if cfg!(windows) { full.replace('/', "\\") } else { full.into_owned() };
    abbreviate_name(&full, 30)
}

/// MFC's `AbbreviateName(path, cchMax, TRUE)`: the whole path when it fits;
/// else the root and its first folder, «\...», and the shortest tail of
/// folders and the name that fits; the bare name when not even
/// «root\first\...\name» does.
fn abbreviate_name(full: &str, max: usize) -> String {
    let sep = if full.contains('\\') { '\\' } else { '/' };
    let chars: Vec<char> = full.chars().collect();
    let n = chars.len();
    if n <= max {
        return full.to_string();
    }
    let name_start = chars.iter().rposition(|&c| c == sep).map_or(0, |i| i + 1);
    let name: String = chars[name_start..].iter().collect();
    let name_len = n - name_start;
    if max < name_len {
        return name;
    }
    // Past "C:" (or a leading separator), then one more folder.
    let mut cur = 2.min(n);
    while cur < n && chars[cur] != sep {
        cur += 1;
    }
    if n - name_len > 3 {
        cur += 1;
        while cur < n && chars[cur] != sep {
            cur += 1;
        }
    }
    let vol = cur;
    if max < vol + 5 + name_len || cur >= n {
        return name;
    }
    while vol + 4 + (n - cur) > max {
        cur += 1;
        while cur < n && chars[cur] != sep {
            cur += 1;
        }
    }
    let head: String = chars[..vol].iter().collect();
    let tail: String = chars[cur..].iter().collect();
    format!("{head}{sep}...{tail}")
}

#[cfg(test)]
mod mru_tests {
    use super::abbreviate_name;

    /// MFC's `AbbreviateName` at 30 characters (VM: the recent list shows
    /// «C:\Users\...\WIN_1999.cfg» for the record in fv-test\records).
    #[test]
    fn recent_names_abbreviate_as_mfc() {
        assert_eq!(abbreviate_name("C:\\Users\\devuser\\fv-test\\records\\WIN_1999.cfg", 30), "C:\\Users\\...\\WIN_1999.cfg");
        assert_eq!(abbreviate_name("C:\\data\\a.cfg", 30), "C:\\data\\a.cfg");
        assert_eq!(abbreviate_name("C:\\Users\\devuser\\records\\x.cfg", 30), "C:\\Users\\devuser\\records\\x.cfg", "30: fits whole");
        assert_eq!(abbreviate_name("C:\\Users\\devuser1\\records\\x.cfg", 30), "C:\\Users\\...\\records\\x.cfg");
        assert_eq!(abbreviate_name("C:\\Users\\a_very_long_record_name_indeed_x.cfg", 30), "a_very_long_record_name_indeed_x.cfg");
        // A Unix path (ours only): the same steps, «/home/user» as the root.
        assert_eq!(abbreviate_name("/home/user/osc/2024/records/rec.cfg", 30), "/home/user/.../records/rec.cfg");
    }
}
