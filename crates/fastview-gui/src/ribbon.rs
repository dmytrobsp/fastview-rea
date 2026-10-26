//! The Office-2007-style ribbon: quick-access toolbar, tab strip and command
//! groups. Layout mirrors the original (QAT: open/save/print/undo/redo; tabs
//! Главная / ОМП; groups Объединение · Диаграммы · Видимость · Вычисления ·
//! Осциллограмма; ОМП tab: Параметры · Анализ — a toggle with the result bar,
//! `omp_ui.rs`). See `notes/UI_SPEC.md`.

use crate::app::{App, Command, Pane, RibbonTab, ValueMode};
use crate::icons::{self, Icon};
use crate::oscgeom::PixelLines;
use crate::theme::Skin;
use crate::tips::{self, TipImage};
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
        // The body's bottom line (row 143, L9.225): the tooltips open under it.
        crate::tips::set_ribbon_bottom(ui.ctx(), full.min.y + 143.0);
        ui.painter().rect_filled(egui::Rect::from_min_size(full.min, egui::vec2(full.width(), 30.0)), 0.0, Color32::WHITE);
        let band = egui::Rect::from_min_size(full.min + egui::vec2(0.0, 30.0), egui::vec2(full.width(), 23.0));
        paint_v_gradient(ui, band, p.ribbon_top, p.ribbon_bottom);

        // The Office 2007 looks' round application button, 45 × 45 at the
        // top left over both rows (`SetApplicationButton` with (45, 45),
        // `0x472fef`; its image 570 is the original's — ours the logo).
        // The older looks' flat button is 41 px about the same centre (VM).
        let orb_d = if p.orb_flat.is_some() { 41.0 } else { 45.0 };
        // Maximised, the caption's content sits a pixel higher (VM).
        let max_up = if ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)) { 1.0 } else { 0.0 };
        let orb = p.app_orb.map(|c| (egui::Rect::from_center_size(full.min + egui::vec2(3.0 + 22.5, 2.0 + 22.5 - max_up), egui::vec2(orb_d, orb_d)), c));
        // Office 2007's quick-access toolbar in the caption: its glass
        // frame (RIBBON_CAPTION_QA_GLASS, VM): from under the orb to x 131,
        // y 6–26, a (173, 176, 184) edge round a light face, the right
        // end rounded; a white disc 25.5 px about the orb's centre cuts its
        // left end, a grey arc along the cut.
        if orb.is_some() && p.orb_flat.is_none() {
            let c = full.min + egui::vec2(22.0, 25.0 - max_up);
            // (the orb's centre, `paint_orb`)
            let glass = egui::Rect::from_min_max(egui::pos2(c.x, full.min.y + 6.0 - max_up), egui::pos2(full.min.x + 132.0, full.min.y + 27.0 - max_up));
            let round = egui::Rounding { nw: 0.0, sw: 0.0, ne: 10.5, se: 10.5 };
            let painter = ui.painter();
            painter.rect_filled(glass, round, Color32::from_rgb(173, 176, 184));
            painter.rect_filled(glass.shrink(1.0), egui::Rounding { ne: 9.5, se: 9.5, ..Default::default() }, Color32::from_rgb(232, 237, 241));
            let cut = painter.with_clip_rect(glass);
            cut.circle_filled(c, 25.5, Color32::WHITE);
            cut.circle_stroke(c, 25.5, egui::Stroke::new(1.0f32, Color32::from_rgb(205, 207, 211)));
        }
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
                // (narrower than the orb: the QAT starts at x 49)
                Some((r, _)) => egui::Button::new("").frame(false).min_size(egui::vec2(r.width() - 10.0, 20.0)),
                None => egui::Button::new(RichText::new(APP_MENU_TITLE).color(p.text).strong()),
            };
            // The main panel is MFC's Office 2007 one (VM, Blue): opened
            // under the tab row with its corner at (0, 26); egui's own menu
            // frame is set aside for this popup and the panel painted
            // under its rows (`main_panel_ground`).
            let saved_style = ui.ctx().style();
            ui.ctx().style_mut(|s| {
                s.visuals.window_fill = Color32::TRANSPARENT;
                s.visuals.window_stroke = egui::Stroke::NONE;
                s.visuals.menu_rounding = egui::Rounding::ZERO;
                s.visuals.popup_shadow = egui::epaint::Shadow::NONE;
                s.spacing.menu_margin = egui::Margin::ZERO;
            });
            {
                let state = egui::menu::BarState::load(ui.ctx(), bar_id);
                let root: &Option<egui::menu::MenuRoot> = &state;
                if let Some(root) = root {
                    root.menu_state.write().rect.min = full.min + egui::vec2(0.0, 26.0);
                }
            }
            let app_button = egui::menu::menu_custom_button(ui, anchor, |ui| {
                // Hovering an item with sub-items shows them in the right
                // column in place of the recent documents; kept only while
                // the menu stays open (stamped each pass; egui returns no
                // inner result while a menu stays open).
                let frame = ui.ctx().cumulative_pass_nr();
                let mut sub = ui.data(|d| d.get_temp::<(Option<AppSub>, u64)>(sub_id)).filter(|s| s.1 + 1 == frame).and_then(|s| s.0);
                let o = ui.max_rect().min;
                let ground = ui.painter().add(egui::Shape::Noop);
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                // The top band: the outer line, white, the gradient, white,
                // the inner line (25 rows).
                ui.allocate_exact_size(egui::vec2(MAIN_W, 25.0), egui::Sense::hover());
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                    ui.add_space(7.0);
                    ui.vertical(|ui| {
                        ui.set_width(MAIN_LEFT_W);
                        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                        let close_cmd = self.active.map(Command::CloseDoc);
                        // (label, its underlined letter, enabled, command, sub-items)
                        let items: [MainItem; 10] = [
                            ("Открыть...", Some(0), true, Some(Command::Open), None),
                            ("Сохранить", Some(0), self.can_save(), Some(Command::Save), None),
                            ("Сохранить как...", Some(10), has_doc, Some(Command::SaveAs), None),
                            ("Проводник", None, true, Some(Command::ToggleExplorer), None),
                            // The main part prints (57607).
                            ("Печать", None, has_doc, Some(Command::Print), Some(AppSub::Print)),
                            ("Параметры файла...", Some(0), has_doc, Some(Command::FileParams), None),
                            ("Комментарии", None, self.hdr_comment_enabled(), Some(Command::HdrComment), None),
                            ("Загрузка настроек...", Some(9), has_doc, Some(Command::LoadSettings), None),
                            ("Настройки", Some(1), true, Some(Command::Settings), None),
                            ("Закрыть", Some(0), close_cmd.is_some(), close_cmd, None),
                        ];
                        for (k, (label, ul, enabled, cmd, opens)) in items.into_iter().chain([("Дополнительно", None, true, None, Some(AppSub::Extras))]).enumerate() {
                            // A separator after «Сохранить как...» and after
                            // every later item (VM): its line from after the
                            // image column, 4 rows in all.
                            if k >= 3 {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(MAIN_LEFT_W, 4.0), egui::Sense::hover());
                                let line = egui::Rect::from_min_max(egui::pos2(r.left() + 35.0, r.top()), egui::pos2(r.right(), r.top() + 1.0));
                                ui.painter().rect_filled(line, 0.0, MAIN_RULE);
                                ui.painter().rect_filled(line.translate(egui::vec2(0.0, 1.0)), 0.0, MAIN_RULE_LIGHT);
                            }
                            // Every item is a large-image button with its text
                            // on the right (the main panel's Repos); the
                            // drawings are ours.
                            let icon = [Some(Icon::Open), Some(Icon::Save), Some(Icon::Save), None, Some(Icon::Print), Some(Icon::Params)].get(k).copied().flatten();
                            let r = main_entry(ui, p, icon, label, ul, enabled || opens.is_some(), opens.is_some() && sub == opens, opens.is_some());
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
                    // The divider: a (197) and a (245) column.
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.set_width(MAIN_RIGHT_W);
                        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                        let title = match sub {
                            Some(AppSub::Print) => "Предварительный просмотр и печать документа",
                            Some(AppSub::Extras) => "Дополнительно (FastView-REA)",
                            None => "Последние документы",
                        };
                        main_header(ui, title);
                        match sub {
                            Some(AppSub::Print) => {
                                for (label, desc, enabled, cmd, icon) in [
                                    ("Параметры страницы", "Изменение параметров страницы", true, Command::PageSetup, Icon::Pages),
                                    ("Предварительный просмотр", "Просмотр документа в полностраничном режиме", has_doc, Command::PrintPreview, Icon::ZoomIn),
                                    ("Печать", "Печать активного документа", has_doc, Command::Print, Icon::Print),
                                ] {
                                    let paint = move |p: &egui::Painter, r: egui::Rect, c: Color32| icons::paint(p, r, icon, c);
                                    if crate::popmenu::large_item(ui, label, desc, enabled, &paint).clicked() && enabled {
                                        cmds.push(cmd);
                                        ui.close_menu();
                                    }
                                }
                            }
                            Some(AppSub::Extras) => {
                                for (label, cmd) in [
                                    ("Демо-осциллограмма", Command::Demo),
                                    ("Экспорт каналов в CSV…", Command::ExportCsv),
                                    ("Архив осциллограммы (.zip)…", Command::Archive),
                                    ("Библиотека характеристик .fvb…", Command::OpenFvb),
                                    ("Роли каналов Ua…Ic…", Command::ChannelMap),
                                    ("Экспорт изображения окна (PNG)…", Command::ExportImage),
                                ] {
                                    if recent_row(ui, label, None).clicked() {
                                        cmds.push(cmd);
                                        ui.close_menu();
                                    }
                                }
                            }
                            None => {
                                let cwd = std::env::current_dir().ok();
                                for (k, path) in mru.iter().enumerate() {
                                    let name = mru_display(path, cwd.as_deref());
                                    // «&1 name» … «1&0 name», then the plain name.
                                    let (text, ul) = if k < 9 { (format!("{} {name}", k + 1), Some(0)) } else if k == 9 { (format!("10 {name}"), Some(1)) } else { (name, None) };
                                    let r = recent_row(ui, &text, ul);
                                    if k < 10 {
                                        menu_rects.push((crate::keytips::Target::Recent(k), r.rect, true));
                                    }
                                    // The full path as a plain tip (VM).
                                    tips::pointer_tip(ui, &r, TipImage::None, "", &path.display().to_string(), false);
                                    if r.clicked() {
                                        cmds.push(Command::OpenPath(path.clone()));
                                        ui.close_menu();
                                    }
                                }
                            }
                        }
                    });
                    ui.add_space(9.0);
                });
                // The frame's lower lines end 1 px below the last item,
                // over its last two rows (VM).
                let bottom = ui.min_rect().bottom() + 1.0;
                ui.painter().set(ground, main_panel_ground(o, bottom));
                ui.data_mut(|d| d.insert_temp(sub_id, (sub, frame)));
            });
            ui.ctx().set_style(saved_style);
            let button = app_button.response.rect;
            self.kt_bar = Some((bar_id, button, app_button.response.id));
            self.kt_rects.push((crate::keytips::Target::AppButton, button, true));
            self.kt_rects.extend(menu_rects);
            // Quick-access toolbar: Open, Save, Print only; Печать is a
            // split button whose arrow opens the application menu's print
            // list (VM), then the customise arrow. Its buttons 22 px apart
            // from x 49, the print arrow after the print button, the
            // customise arrow outside the glass (VM).
            let qat_spacing = ui.spacing().item_spacing.x;
            ui.spacing_mut().item_spacing.x = 0.0;
            if orb.is_some() {
                let to = full.min.x + 49.0 - ui.cursor().min.x;
                ui.add_space(to.max(0.0));
            } else {
                ui.add_space(4.0);
            }
            for (n, (icon, cmd)) in [(Icon::Open, Command::Open), (Icon::Save, Command::Save), (Icon::Print, Command::Print)]
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
                let r = ui.add_enabled_ui(enabled, |ui| qat_button(ui, p, icon)).inner;
                if let Some((title, desc)) = tips::cmd_tip(&cmd) {
                    tips::qat_tip(ui, &r, TipImage::Small(icon), title, desc);
                }
                self.kt_rects.push((crate::keytips::Target::Qat(n), r.rect, enabled));
                if r.clicked() {
                    cmds.push(cmd);
                }
                if r.secondary_clicked() {
                    open_min_menu(ui, r.rect.left_bottom());
                }
                if n == 2 {
                    let arrow = ui.add_enabled_ui(enabled, |ui| qat_arrow(ui, p, false)).inner;
                    // The arrow's tip is the print button's (VM).
                    if let Some((title, desc)) = tips::cmd_tip(&Command::Print) {
                        tips::qat_tip(ui, &arrow, TipImage::Small(Icon::Print), title, desc);
                    }
                    if arrow.clicked() {
                        open_qat_menu(ui, QatMenu::Print, r.rect.left_bottom());
                    }
                }
            }
            ui.add_space(14.5);
            // The customise arrow: one item, «Свернуть ленту».
            let custom = qat_arrow(ui, p, true);
            // A plain one-line tip (mfc120rus 17096).
            tips::qat_tip(ui, &custom, TipImage::None, "", "Настройка панели быстрого доступа");
            ui.spacing_mut().item_spacing.x = qat_spacing;
            // Its menu right under the button's own 20 px (VM: the border at
            // y 24, under the pressed look's 18–23).
            if custom.clicked() || custom.secondary_clicked() {
                open_min_menu(ui, custom.rect.left_bottom() - egui::vec2(0.0, 2.0));
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
                    // The style's TextCenter: off for Aqua (left-aligned);
                    // else 25 px right of the window's centre, maximised or
                    // not (VM: 1305.5 of 2560, 1193 of a 216–2120 window),
                    // a pixel lower when maximised (rows 11–22).
                    let mid = ui.ctx().screen_rect().center().x + 25.0;
                    let x = if self.skin == Skin::Aqua { free.left() + 4.0 } else { (mid - w / 2.0).min(free.right() - w).max(free.left()) };
                    let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    let y = free.center().y - g.size().y / 2.0 + if max { 1.0 } else { 0.0 };
                    ui.painter().with_clip_rect(free).galley(egui::pos2(x, y), g, Color32::BLACK);
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
                    .inner_margin(egui::Margin { left: 4.0, right: 6.0, top: 2.0, bottom: 2.0 })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = row_gap;
                        ui.set_min_height(LARGE_H + 2.0 + 16.0 + 2.0);
                        ui.set_min_width(ui.available_width());
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            self.ribbon_preview(ui)
                        });
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
            // Four pixels between the tabs' frames (VM).
            ui.spacing_mut().item_spacing.x -= 1.0;
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
            // The category body's gradient behind it (set below). Its
            // content from row 56: a panel's frame there, its buttons from
            // 58 (VM).
            let body_bg = ui.painter().add(egui::Shape::Noop);
            let blue = p.bands.is_some_and(|b| b.panel_sep.is_some());
            let body = egui::Frame::none()
                .fill(if p.body.is_some() { Color32::TRANSPARENT } else { p.tab_active })
                .stroke(egui::Stroke::new(1.0f32, if blue { Color32::TRANSPARENT } else { p.group_border }))
                .inner_margin(egui::Margin { left: 4.0, right: 6.0, top: 2.0, bottom: 2.0 })
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
                        child.spacing_mut().item_spacing.x = 0.0;
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
                    // The panels set their own gaps (`ribbon_group`).
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        match self.tab {
                            RibbonTab::Home => self.ribbon_home(ui, &mut cmds),
                            RibbonTab::Omp => self.ribbon_omp(ui, &mut cmds),
                        }
                    });
                });
            if blue {
                ui.painter().set(body_bg, egui::Shape::Vec(body_2007(body.response.rect)));
            } else if let Some([top, dip, bottom]) = p.body {
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
            self.dispatch_from_menu(ui.ctx(), c);
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
            let r = ui.add_enabled_ui(can_paste, |ui| big_button(ui, p, Icon::Paste, "Вставить")).inner;
            tips::ribbon_tip(ui, &r, TipImage::Large(Icon::Paste), &Command::Paste);
            if r.clicked() {
                cmds.push(Command::Paste);
            }
            ui.vertical(|ui| {
                for (icon, label, on, cmd) in [
                    (Icon::Copy, "Копировать", can_copy, Command::Copy),
                    (Icon::Undo, "Отмена", undo, Command::Undo),
                    (Icon::Redo, "Возврат", redo, Command::Redo),
                ] {
                    let r = ui.add_enabled_ui(on, |ui| small_button_mode(ui, p, icon, label, false, step >= 1)).inner;
                    tips::ribbon_tip(ui, &r, TipImage::Small(icon), &cmd);
                    if r.clicked() {
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
                let r = big_button_sel(ui, p, icon, label, self.panes.visible(pane));
                tips::ribbon_tip(ui, &r, TipImage::Large(icon), &Command::TogglePane(pane));
                if r.clicked() {
                    cmds.push(Command::TogglePane(pane));
                }
            }
            if step > 0 {
                ui.vertical(|ui| {
                    for (icon, label, pane) in buttons.into_iter().skip(1) {
                        let label = label.replace("-\n", "");
                        let r = small_button_mode(ui, p, icon, &label, self.panes.visible(pane), step >= 2);
                        tips::ribbon_tip(ui, &r, TipImage::Small(icon), &Command::TogglePane(pane));
                        if r.clicked() {
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
                    for (glyph, _label, enabled, on, cmd) in column {
                        let r = mini_button(ui, p, *glyph, *enabled, *on);
                        tips::ribbon_tip(ui, &r, TipImage::Mini(*glyph), cmd);
                        if r.clicked() {
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
                    for (glyph, _label, st, cmd) in column {
                        let r = mini_button(ui, p, glyph, st.enabled, st.checked);
                        tips::ribbon_tip(ui, &r, TipImage::Mini(glyph), &cmd);
                        if r.clicked() {
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
                    let r = ui.add_enabled_ui(s.enabled, |ui| small_button_mode(ui, p, icon, label, s.checked, step >= 2)).inner;
                    tips::ribbon_tip(ui, &r, TipImage::Small(icon), &Command::ToggleFlag(f));
                    if r.clicked() {
                        cmds.push(Command::ToggleFlag(f));
                    }
                }
            });
            ui.vertical(|ui| {
                // 32866 «Коэффициенты»; 32867 / 32868 a radio pair checked
                // from channel 0 (CMDUI `0x42ffb0`; neither with no analog
                // channel); all three the view's — greyed with no document.
                let has_doc = self.active_doc().is_some();
                let prim = self.active_doc().and_then(|d| d.primary_shown());
                for (icon, label, on, cmd) in [
                    (Icon::Params, "Коэффициенты", false, Command::Coefficients),
                    (Icon::Primary, "Первичные знач.", prim == Some(true), Command::SetPrimaryValues(true)),
                    (Icon::Primary, "Вторичные знач.", prim == Some(false), Command::SetPrimaryValues(false)),
                ] {
                    let r = ui.add_enabled_ui(has_doc, |ui| small_button_mode(ui, p, icon, label, on, step >= 1)).inner;
                    tips::ribbon_tip(ui, &r, TipImage::Small(icon), &cmd);
                    if r.clicked() {
                        cmds.push(cmd);
                    }
                }
            });
            let analog = self.active_doc().is_some_and(|d| !d.osc.analog.is_empty());
            let r = ui.add_enabled_ui(analog, |ui| big_button(ui, p, Icon::Analysis, "Быстрый анализ")).inner;
            tips::ribbon_tip(ui, &r, TipImage::Large(Icon::Analysis), &Command::QuickAnalysis);
            if r.clicked() {
                cmds.push(Command::QuickAnalysis);
            }
        });
        // Осциллограмма
        // The unnamed panel (title " ", «Информация» in the help).
        ribbon_group(ui, p, " ", 1, |ui, _| {
            // 142: toggles the parameters bar, checked while it is shown.
            let r = big_button_sel(ui, p, Icon::Wave, "Осцилло грамма", self.dlg.param_bar);
            tips::ribbon_tip(ui, &r, TipImage::Large(Icon::Wave), &Command::ToggleParamBar);
            if r.clicked() {
                cmds.push(Command::ToggleParamBar);
            }
        });
    }

    /// «Стиль» and Help on the right of the tab row (§2.5).
    fn tab_row_right(&mut self, ui: &mut egui::Ui, p: crate::theme::Palette, cmds: &mut Vec<Command>) {
        // §2.5: «Стиль» and Help on the right of the tab row.
        // Both flat on the tab row (VM, Office 2007 Blue), each with MFC's
        // small menu arrow, framed only under the pointer: «Стиль» in the
        // tab ink, the help button's round blue «?» image.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // The two buttons touch (VM: «Стиль» from x 2480, its arrow at
            // 2521, the «?» image's centre at 2537).
            ui.spacing_mut().item_spacing.x = 0.0;
            // Help split button (57667): the arrow opens {Помощь, О программе...}.
            let (hr, help) = ui.allocate_exact_size(egui::vec2(33.0, 22.0), egui::Sense::click());
            tab_row_hot(ui, &help, &p);
            help_image(ui.painter(), egui::pos2(hr.left() + 10.5, hr.center().y));
            menu_arrow(ui.painter(), egui::pos2(hr.left() + 25.0, hr.center().y - 1.0));
            // mfc120rus 57667, as the original's tip («Help Topics» on the VM).
            tips::pointer_tip(ui, &help, TipImage::Help, "Вызов справки", "Вывод оглавления справочной системы", true);
            if help.clicked() {
                self.open_top_menu(ui.ctx(), TopMenu::Help, help.rect);
            }
            self.kt_rects.push((crate::keytips::Target::Help, help.rect, true));
            // «Стиль» (menu 200): the radio entry of the current look,
            // persisted as `ApplicationLook`.
            let (sr, style) = ui.allocate_exact_size(egui::vec2(48.5, 22.0), egui::Sense::click());
            tab_row_hot(ui, &style, &p);
            ui.painter().text(egui::pos2(sr.left() + 2.0, sr.center().y), egui::Align2::LEFT_CENTER, "Стиль", egui::FontId::proportional(12.0), p.text);
            menu_arrow(ui.painter(), egui::pos2(sr.left() + 43.0, sr.center().y - 1.0));
            // Its own tip and description (strings 287 / 288, VM).
            tips::pointer_tip(ui, &style, TipImage::Blank, "Изменить стиль оформления", "Выберите одну из тем Office 2007", true);
            if style.clicked() {
                self.open_top_menu(ui.ctx(), TopMenu::Style, style.rect);
            }
        });
        self.top_menu_frame(ui.ctx(), cmds);
    }

}

/// A tab-row button's frame under the pointer.
fn tab_row_hot(ui: &egui::Ui, r: &egui::Response, p: &crate::theme::Palette) {
    if r.hovered() {
        ui.painter().rect(r.rect, 2.0f32, p.accent_soft.gamma_multiply(0.5f32), egui::Stroke::new(1.0f32, p.accent_border));
    }
}

/// MFC's small menu arrow (VM): rows of 5, 3 and 1 px in (86, 125, 177)
/// from `at` (its top-left), a white pixel on either end of the lower two.
fn menu_arrow(p: &egui::Painter, at: egui::Pos2) {
    let (x, y) = (at.x.round(), at.y.round());
    let px = |dx: f32, dy: f32, w: f32, c: Color32| p.rect_filled(egui::Rect::from_min_size(egui::pos2(x + dx, y + dy), egui::vec2(w, 1.0)), 0.0, c);
    let ink = Color32::from_rgb(86, 125, 177);
    px(0.0, 0.0, 5.0, ink);
    px(1.0, 1.0, 3.0, ink);
    px(2.0, 2.0, 1.0, ink);
    for (dx, dy) in [(0.0, 1.0), (4.0, 1.0), (1.0, 2.0), (3.0, 2.0)] {
        px(dx, dy, 1.0, Color32::WHITE);
    }
}

/// The help button's image (VM): a blue sphere with a white ring inside a
/// (80, 113, 160) edge, a white «?».
pub(crate) fn help_image(p: &egui::Painter, c: egui::Pos2) {
    p.circle_filled(c, 7.5, Color32::from_rgb(80, 113, 160));
    p.circle_filled(c, 6.5, Color32::WHITE);
    p.circle_filled(c, 5.5, Color32::from_rgb(31, 95, 230));
    p.circle_filled(c + egui::vec2(-1.5, -1.5), 3.0, Color32::from_rgb(79, 136, 250));
    p.text(c + egui::vec2(0.0, 0.5), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(10.0), Color32::WHITE);
}

impl App {
    /// Open a tab-row menu under its button, the original's width (its
    /// font's) ending 1.5 px past the button's right, a pixel up (VM:
    /// «Стиль»'s at (2369, 52)); a ribbon button's menu, without pads.
    pub(crate) fn open_top_menu(&mut self, ctx: &egui::Context, kind: TopMenu, button: egui::Rect) {
        let width = match kind {
            TopMenu::Style => 160.0,
            TopMenu::Help => 123.5,
        };
        let at = egui::pos2(button.right() + 1.5 - width, button.bottom() - 1.0);
        self.top_menu = Some((kind, crate::popmenu::PopMenu::new(ctx.cumulative_pass_nr(), at).unpadded()));
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
                    // MENU 202's texts with their mnemonics (VM: underlined).
                    .zip(["Windows &2000", "Office &XP", "&Windows XP", "Office 200&3", "Visual Studio.NET 200&5"])
                    .map(|(s, name)| Entry::item(name, true, Mark::Radio(self.skin == s), Pick::Skin(s)))
                    .collect();
                let sub = Skin::ALL[5..]
                    .iter()
                    .copied()
                    .zip(["&Голубой стиль", "Ч&ерный стиль", "&Серебристый стиль", "&Зеленовато-голубой стиль"])
                    .map(|(s, name)| Entry::item(name, true, Mark::Radio(self.skin == s), Pick::Skin(s)))
                    .collect();
                e.push(Entry::sub("Office 200&7", sub));
                e
            }
            TopMenu::Help => vec![Entry::item("&Помощь", false, Mark::None, Pick::Nothing), Entry::Sep, Entry::item("&О программе...", true, Mark::None, Pick::About)],
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
                let r = big_button(ui, p, Icon::Params, "Параметры");
                tips::ribbon_tip(ui, &r, TipImage::Large(Icon::Params), &Command::OmpParams);
                if r.clicked() {
                    cmds.push(Command::OmpParams);
                }
            });
            ui.add_enabled_ui(can_run, |ui| {
                let r = big_button_sel(ui, p, Icon::OmpRun, "Анализ", shown);
                tips::ribbon_tip(ui, &r, TipImage::Large(Icon::OmpRun), &Command::OmpAnalysis);
                if r.clicked() {
                    cmds.push(Command::OmpAnalysis);
                }
            });
        });
    }
}

// ---------------- drawing helpers ----------------


/// A vertical gradient through `stops` (y offsets from the top, colours) as
/// one mesh, the last colour down to the bottom.
/// Whether `p` is one of the Office 2007 looks (their gradients and
/// navy ink).
fn office2007(p: &crate::theme::Palette) -> bool {
    p.caption_band.is_some() && p.accent_soft == Color32::from_rgb(255, 228, 150)
}

/// A checked button's ground. The Office 2007 looks (VM, Blue, a large
/// button scanned): a (142, 129, 101) frame, its bottom edge gold (255,
/// 209, 53), round a two-part face — a light gradient over its upper 39 %
/// ((253, 212, 167) → (251, 174, 95)), then a step to (249, 150, 48) going
/// back to (252, 231, 157); under the pointer (`hot`) the same darker:
/// (246, 178, 109) → (235, 152, 84), (230, 131, 51) → (247, 196, 95). The
/// older looks their flat selection colour.
pub(crate) fn checked_bg(painter: &egui::Painter, rect: egui::Rect, p: &crate::theme::Palette, hot: bool) {
    if office2007(p) {
        let face = rect.shrink(1.0);
        let h = face.height();
        let split = (0.39 * h).round();
        let c = |r, g, b| Color32::from_rgb(r, g, b);
        let stops = if hot {
            [(0.0, c(246, 178, 109)), (split - 1.0, c(235, 152, 84)), (split, c(230, 131, 51)), (h, c(247, 196, 95))]
        } else {
            [(0.0, c(253, 212, 167)), (split - 1.0, c(251, 174, 95)), (split, c(249, 150, 48)), (h, c(252, 231, 157))]
        };
        painter.add(gradient_mesh(face, &stops));
        // epaint strokes a rect outside it: the face's rect puts the frame
        // on the button's own first and last rows and columns (VM: row 58
        // and 123 of a 58–123 button).
        painter.rect_stroke(face, 1.0f32, egui::Stroke::new(1.0f32, if hot { c(167, 135, 94) } else { c(142, 129, 101) }));
        painter.px_hline(rect.left() + 2.0..=rect.right() - 3.0, rect.bottom() - 1.0, egui::Stroke::new(1.0f32, c(255, 209, 53)));
    } else {
        painter.rect(rect, 2.0f32, p.accent_soft, egui::Stroke::new(1.0f32, p.accent_border));
    }
}

/// A checked toolbar button (CMFCToolBar, not the ribbon; VM, the
/// differential pane's «Диапазон между визирами»): in the Office 2007
/// looks a (85, 123, 175) frame round a light orange face, (251, 219,
/// 181) → (253, 201, 114) at mid-height → (255, 228, 127); else the
/// ribbon's checked ground.
pub(crate) fn toolbar_checked(painter: &egui::Painter, rect: egui::Rect, p: &crate::theme::Palette) {
    if !office2007(p) {
        checked_bg(painter, rect, p, false);
        return;
    }
    let face = rect.shrink(1.0);
    let h = face.height();
    painter.add(gradient_mesh(face, &[(0.0, Color32::from_rgb(251, 219, 181)), (h / 2.0, Color32::from_rgb(253, 201, 114)), (h, Color32::from_rgb(255, 228, 127))]));
    painter.px_rect_stroke(rect, 1.0, egui::Stroke::new(1.0f32, Color32::from_rgb(85, 123, 175)));
}

/// A small or icon-only button (22 px) under the pointer in the Office 2007
/// looks (VM, «Коэффициенты» and «Показать дискретные» hot): a 1 px frame
/// — (219, 206, 153) on top, (185, 160, 116) at the sides, (126, 114, 90)
/// below — round a face with a light first and last row, (255, 252, 228)
/// → (255, 236, 161) over its upper half, then (255, 216, 66) → (255, 228,
/// 123). The older looks: their soft accent box.
pub(crate) fn hot_small(painter: &egui::Painter, rect: egui::Rect, p: &crate::theme::Palette) {
    if !office2007(p) {
        painter.rect(rect, 2.0f32, p.accent_soft.gamma_multiply(0.7f32), egui::Stroke::new(1.0f32, p.accent_border));
        return;
    }
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    let face = rect.shrink(1.0);
    let h = face.height();
    let mid = (h / 2.0).floor();
    let (hl, low) = (c(255, 255, 251), c(255, 250, 207));
    painter.add(gradient_mesh(face, &[(0.0, hl), (1.0, hl), (1.0, c(255, 252, 228)), (mid, c(255, 236, 161)), (mid, c(255, 216, 66)), (h - 1.0, c(255, 228, 123)), (h - 1.0, low), (h, low)]));
    let (l, r, t, b) = (rect.left(), rect.right() - 1.0, rect.top(), rect.bottom() - 1.0);
    painter.px_hline(l + 1.0..=r - 1.0, t, egui::Stroke::new(1.0f32, c(219, 206, 153)));
    painter.px_hline(l + 1.0..=r - 1.0, b, egui::Stroke::new(1.0f32, c(126, 114, 90)));
    painter.px_vline(l, t + 1.0..=b - 1.0, egui::Stroke::new(1.0f32, c(185, 160, 116)));
    painter.px_vline(r, t + 1.0..=b - 1.0, egui::Stroke::new(1.0f32, c(185, 160, 116)));
}

/// A large button under the pointer in the Office 2007 looks (VM,
/// «Каналы» hot): a 1 px border with its corners cut — (221, 207, 155) on
/// top, (210, 208, 188) below, (210, 192, 141) at the sides — a light inner
/// line, and the face from (255, 253, 220) to (255, 231, 148) down to row
/// 25, (255, 215, 81) to (255, 211, 74) and back to (255, 213, 82) down to
/// row 41, then (255, 214, 85) to (255, 231, 153). The older looks: their
/// soft accent box.
fn hot_large(painter: &egui::Painter, rect: egui::Rect, p: &crate::theme::Palette) {
    if !office2007(p) {
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

pub(crate) fn gradient_mesh(rect: egui::Rect, stops: &[(f32, Color32)]) -> egui::Shape {
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
    // The flat looks' button fills its 41 px; Office 2007's round image is
    // 36 px in the 45 px button, its centre at (22, 25) (VM).
    // (The rim's stroke inside the 18 px radius.)
    let (r, rad) = if flat.is_some() { (r, r.width() / 2.0) } else { (egui::Rect::from_center_size(r.min + egui::vec2(19.0, 23.0), egui::vec2(36.0, 36.0)), 17.25) };
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
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    let painter = ui.painter();
    // Hot: the small buttons' gold face, 20 px from 2 px down (VM: y 6–25).
    if resp.hovered() && ui.is_enabled() {
        hot_small(painter, egui::Rect::from_min_max(rect.min + egui::vec2(0.0, 2.0), rect.max), &p);
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
        // The first tab's frame at x 48 (VM).
        ui.add_space(r.right() - ui.cursor().left() + 1.0);
        let ctx = ui.ctx().clone();
        let hit = ui.interact(r, ui.id().with("fv_app_orb"), egui::Sense::click());
        // Its plain tip: the button's text «Файл» (VM).
        tips::pointer_tip(ui, &hit, TipImage::None, "", "Файл", false);
        if hit.clicked() {
            self.kt_menu(&ctx, true);
        }
        let open = self.kt_bar.is_some_and(|b| {
            let st = egui::menu::BarState::load(&ctx, b.0);
            let root: &Option<egui::menu::MenuRoot> = &st;
            root.is_some()
        });
        let hot = ctx.input(|i| i.pointer.hover_pos()).is_some_and(|q| q.distance(r.center()) <= r.width() / 2.0);
        // Over the open main panel (VM: the orb stays whole above its top
        // band).
        let order = if open { egui::Order::Tooltip } else { egui::Order::Middle };
        paint_orb(&ui.painter().clone().with_layer_id(egui::LayerId::new(order, egui::Id::new("fv_app_orb_layer"))), r, colors, if open { 2 } else if hot { 1 } else { 0 }, p.orb_flat);
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
            // The minimise item is a menu row (its pads, VM); the print list
            // a panel of large items.
            let row_menu = menu == QatMenu::Minimize;
            crate::popmenu::mfc_frame_with(ui, row_menu, row_menu, |ui| match menu {
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
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(if customise { 13.0 } else { 9.0 }, 22.0), egui::Sense::click());
    let painter = ui.painter();
    if resp.hovered() && ui.is_enabled() {
        hot_small(painter, egui::Rect::from_min_max(rect.min + egui::vec2(0.0, 2.0), rect.max), &p);
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
    } else if resp.hovered() && blue {
        // Hot (VM, «ОМП» under the pointer): a (152, 187, 233) frame with
        // rounded top corners from a pixel down, open onto the body's line,
        // a light (237, 244, 254) inner line at the sides, the face (196,
        // 221, 251) → (222, 233, 246) over 10 rows, then (213, 227, 239) →
        // (225, 210, 163) at the bottom.
        let c = |r, g, b| Color32::from_rgb(r, g, b);
        let r = rect.with_min_y(rect.top() + 1.0);
        let face = r.shrink(1.0).with_max_y(r.bottom());
        let h = face.height();
        painter.add(gradient_mesh(face, &[(0.0, c(196, 221, 251)), (10.0, c(222, 233, 246)), (10.0, c(213, 227, 239)), (h, c(225, 210, 163))]));
        let light = egui::Stroke::new(1.0f32, c(237, 244, 254));
        painter.px_vline(face.left(), face.top() + 2.0..=face.bottom() - 1.0, light);
        painter.px_vline(face.right() - 1.0, face.top() + 2.0..=face.bottom() - 1.0, light);
        painter.px_rect_stroke(r.with_max_y(r.bottom() + 1.0), 3.0, egui::Stroke::new(1.0f32, c(152, 187, 233)));
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
        ui.add_space(12.0);
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
        // Under the pointer (Office 2007, VM «Видимость» hot) the panel
        // lights up: a white frame, its face (229, 240, 255) → (231, 241,
        // 253) over the first 13 rows then (220, 234, 252) → (221, 233,
        // 249), the band (200, 224, 255) → (214, 237, 255).
        let blue = p.bands.is_some_and(|b| b.panel_sep.is_some());
        let shapes = if blue {
            panel_2007(r, ui.rect_contains_pointer(r))
        } else if office2007(&p) && ui.rect_contains_pointer(r) {
            let c = |r, g, b| Color32::from_rgb(r, g, b);
            let face = egui::Rect::from_min_max(r.min + egui::vec2(1.0, 1.0), egui::pos2(r.right() - 1.0, band.top()));
            let h = face.height();
            vec![
                gradient_mesh(face, &[(0.0, c(229, 240, 255)), (13.0, c(231, 241, 253)), (13.0, c(220, 234, 252)), (h, c(221, 233, 249))]),
                gradient_mesh(band.shrink2(egui::vec2(1.0, 0.0)), &[(0.0, c(200, 224, 255)), (16.0, c(214, 237, 255))]),
                egui::Shape::rect_stroke(r.shrink(1.0), 2.0f32, egui::Stroke::new(1.0f32, Color32::WHITE)),
            ]
        } else {
            vec![
                egui::Shape::rect_stroke(r.shrink(1.0), 2.0f32, egui::Stroke::new(1.0f32, crate::theme::alpha(255, 255, 255, 130))),
                gradient_mesh(band, &[(0.0, top), (16.0, bottom)]),
            ]
        };
        ui.painter().set(bg, egui::Shape::Vec(shapes));
        if blue {
            ui.add_space(3.0);
            return;
        }
    }
    // right divider
    let r = ui.min_rect();
    ui.painter().line_segment(
        [egui::pos2(r.right() + 1.5, r.top() + 4.0), egui::pos2(r.right() + 1.5, r.bottom() - 2.0)],
        egui::Stroke::new(1.0f32, p.group_border),
    );
    // The next panel 3 px on (VM: frames at 152 / 156, 399 / 403 …).
    ui.add_space(3.0);
}

/// Office 2007 Blue's category body (VM, rows 53–142 of a maximised
/// window): a (141, 178, 227) line along the top and down both sides 1 px
/// in, rounded at the top, a light column inside each side, the frame's
/// brush (191, 219, 255) → (185, 212, 247) outside them; the ground the
/// panels' own two-part face — (222, 231, 245) → (208, 223, 240) over its
/// first rows, then (199, 216, 237) on to (227, 244, 255) — and a light
/// last row. `r` the body frame's rect (its stroke row above it).
fn body_2007(r: egui::Rect) -> Vec<egui::Shape> {
    use egui::{pos2, Rect, Shape};
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    // Rows t … t + 88 (54–142 maximised), the line on t − 1.
    let (l, w, t) = (r.left().floor(), r.right().round() - 1.0, r.top().round());
    let b = t + 89.0;
    let px = |x0: f32, y0: f32, x1: f32, y1: f32, col: Color32| Shape::rect_filled(Rect::from_min_max(pos2(x0, y0), pos2(x1, y1)), 0.0, col);
    let col = |x: f32, y0: f32, y1: f32, stops: &[(f32, Color32)]| gradient_mesh(Rect::from_min_max(pos2(x, y0), pos2(x + 1.0, y1)), stops);
    let line = c(141, 178, 227);
    let light = [(0.0, c(225, 238, 247)), (13.0, c(213, 235, 247)), (14.0, c(204, 232, 245)), (43.0, c(206, 244, 251)), (83.0, c(194, 249, 255))];
    let mut v = vec![
        gradient_mesh(
            Rect::from_min_max(pos2(l, t), pos2(w + 1.0, b - 1.0)),
            &[
                (0.0, c(231, 239, 248)),
                (1.0, c(231, 239, 248)),
                (1.0, c(223, 236, 247)),
                (2.0, c(223, 236, 247)),
                (2.0, c(224, 233, 245)),
                (4.0, c(222, 231, 245)),
                (17.0, c(208, 223, 240)),
                (17.0, c(199, 216, 237)),
                (71.0, c(216, 232, 245)),
                (87.0, c(227, 244, 255)),
                (87.0, c(231, 242, 255)),
                (88.0, c(231, 242, 255)),
            ],
        ),
        px(l, b - 1.0, w + 1.0, b, c(192, 249, 255)),
        // The brush outside the lines.
        col(l, t, b + 1.0, &[(0.0, c(191, 219, 255)), (84.0, c(185, 212, 247)), (90.0, c(189, 216, 252))]),
        col(w, t, b + 1.0, &[(0.0, c(191, 219, 255)), (84.0, c(185, 212, 247)), (90.0, c(189, 216, 252))]),
        // The lines and the light inside them.
        px(l + 5.0, t - 1.0, w - 3.0, t, line),
        col(l + 1.0, t + 3.0, b - 1.0, &[(0.0, line), (82.0, line), (83.0, c(146, 181, 227)), (85.0, c(179, 205, 239))]),
        col(w - 1.0, t + 3.0, b - 1.0, &[(0.0, c(148, 183, 229)), (80.0, c(153, 189, 231)), (82.0, c(140, 177, 225)), (85.0, c(180, 206, 240))]),
        col(l + 2.0, t + 3.0, b - 1.0, &light),
        col(w - 2.0, t + 3.0, b - 1.0, &light),
    ];
    // The rounded top corners.
    let brush = c(191, 219, 255);
    for (x, y, k) in [
        (l, t - 1.0, brush),
        (l + 1.0, t - 1.0, brush),
        (w - 1.0, t - 1.0, brush),
        (w, t - 1.0, brush),
        (l + 4.0, t, c(224, 233, 246)),
        (l + 2.0, t - 1.0, c(184, 214, 251)),
        (l + 3.0, t - 1.0, c(164, 196, 239)),
        (l + 4.0, t - 1.0, c(147, 182, 229)),
        (l + 1.0, t, c(182, 213, 249)),
        (l + 2.0, t, c(160, 193, 236)),
        (l + 3.0, t, c(198, 216, 240)),
        (l + 1.0, t + 1.0, c(162, 194, 237)),
        (l + 2.0, t + 1.0, c(200, 217, 240)),
        (l + 1.0, t + 2.0, c(143, 179, 227)),
        (l + 2.0, t + 2.0, c(223, 235, 247)),
        (w - 3.0, t - 1.0, c(162, 196, 238)),
        (w - 2.0, t - 1.0, c(184, 212, 251)),
        (w - 3.0, t, c(201, 218, 241)),
        (w - 2.0, t, c(144, 179, 227)),
        (w - 1.0, t, c(178, 208, 247)),
        (w, t, brush),
        (w - 3.0, t + 1.0, c(227, 237, 248)),
        (w - 2.0, t + 1.0, c(206, 222, 242)),
        (w - 1.0, t + 1.0, c(157, 192, 235)),
        (w - 2.0, t + 2.0, c(226, 237, 247)),
        (w - 1.0, t + 2.0, c(142, 178, 227)),
        // The rounded bottom corners (rows 140–142).
        (l + 1.0, t + 86.0, c(146, 181, 227)),
        (l + 1.0, t + 87.0, c(160, 190, 229)),
        (l + 2.0, t + 87.0, c(194, 243, 252)),
        (l + 3.0, t + 87.0, c(207, 240, 252)),
        (l + 4.0, t + 87.0, c(226, 240, 254)),
        (l, t + 88.0, c(187, 215, 250)),
        (l + 1.0, t + 88.0, c(179, 205, 239)),
        (l + 2.0, t + 88.0, c(169, 199, 229)),
        (l + 3.0, t + 88.0, c(191, 237, 248)),
        (l + 4.0, t + 88.0, c(193, 247, 254)),
        (w - 2.0, t + 86.0, c(196, 249, 255)),
        (w - 1.0, t + 86.0, c(150, 183, 227)),
        (w - 3.0, t + 87.0, c(202, 240, 252)),
        (w - 2.0, t + 87.0, c(190, 236, 249)),
        (w - 1.0, t + 87.0, c(165, 194, 231)),
        (w - 3.0, t + 88.0, c(187, 229, 243)),
        (w - 2.0, t + 88.0, c(164, 191, 224)),
        (w - 1.0, t + 88.0, c(180, 206, 240)),
        (w, t + 88.0, c(188, 216, 251)),
    ] {
        v.push(px(x, y, x + 1.0, y + 1.0, k));
    }
    v
}

/// Office 2007 Blue's panel (VM, «Видимость» scanned, rows 56–140): a
/// rounded frame shading (194, 207, 221) → (169, 191, 210) → (157, 192,
/// 218) down its sides, (196, 209, 222) along the top and (158, 192, 218)
/// the bottom; a light line inside it at the top and left, one outside it
/// on the right and under it; the face (222, 231, 245) → (208, 223, 240)
/// over 13 rows, then (199, 216, 237) → (216, 232, 245); the 15-row
/// caption band (193, 215, 240) → (191, 216, 239). `r` the panel: its
/// frame on its first and last columns, its top row and the row under it.
/// Under the pointer (`hot`, VM): a bluer frame, (172, 198, 223) → (132,
/// 176, 212) → (126, 172, 210), a white line all round inside it, the
/// face (229, 240, 255) → (233, 241, 253), (221, 235, 252) → (221, 233,
/// 249), the band (200, 224, 255) lighting to (214, 237, 255) over its
/// last rows, no light outside.
fn panel_2007(r: egui::Rect, hot: bool) -> Vec<egui::Shape> {
    use egui::{pos2, Rect, Shape};
    let c = |r, g, b| Color32::from_rgb(r, g, b);
    let (l, rt, t, b) = (r.left(), r.right() - 1.0, r.top(), r.bottom());
    let band = b - 15.0;
    let px = |x0: f32, y0: f32, x1: f32, y1: f32, col: Color32| Shape::rect_filled(Rect::from_min_max(pos2(x0, y0), pos2(x1, y1)), 0.0, col);
    let col = |x: f32, y0: f32, y1: f32, stops: &[(f32, Color32)]| gradient_mesh(Rect::from_min_max(pos2(x, y0), pos2(x + 1.0, y1)), stops);
    if hot {
        let side = [(0.0, c(171, 198, 223)), (66.0, c(132, 176, 212)), (80.0, c(126, 172, 210))];
        let (white, soft) = (Color32::WHITE, c(239, 246, 255));
        let mut v = vec![
            gradient_mesh(Rect::from_min_max(pos2(l + 2.0, t + 2.0), pos2(rt - 1.0, t + 15.0)), &[(0.0, c(229, 240, 255)), (12.0, c(233, 241, 253))]),
            gradient_mesh(Rect::from_min_max(pos2(l + 2.0, t + 15.0), pos2(rt - 1.0, band)), &[(0.0, c(221, 235, 252)), (band - t - 16.0, c(221, 233, 249))]),
            gradient_mesh(Rect::from_min_max(pos2(l + 2.0, band), pos2(rt - 1.0, b - 1.0)), &[(0.0, c(200, 224, 255)), (8.0, c(201, 225, 255)), (13.0, c(214, 237, 255))]),
            // The white line inside the frame, softer at its ends.
            px(l + 2.0, t + 1.0, rt - 1.0, t + 2.0, white),
            px(l + 2.0, b - 1.0, rt - 1.0, b, white),
            px(l + 1.0, t + 2.0, l + 2.0, b - 1.0, white),
            px(rt - 1.0, t + 2.0, rt, b - 1.0, white),
            // The frame.
            px(l + 2.0, t, rt - 1.0, t + 1.0, c(172, 198, 223)),
            px(l + 2.0, b, rt - 1.0, b + 1.0, c(125, 172, 210)),
            col(l, t + 2.0, b - 1.0, &side),
            col(rt, t + 2.0, b - 1.0, &side),
        ];
        for (x, y, col) in [
            (l + 2.0, t + 1.0, soft),
            (rt - 2.0, t + 1.0, soft),
            (l + 1.0, t + 2.0, soft),
            (rt - 1.0, t + 2.0, soft),
            (l + 1.0, b - 2.0, soft),
            (rt - 1.0, b - 2.0, soft),
            (l + 2.0, b - 1.0, soft),
            (rt - 2.0, b - 1.0, soft),
            (l + 1.0, t, c(193, 212, 232)),
            (l, t + 1.0, c(193, 212, 232)),
            (l + 1.0, t + 1.0, c(172, 198, 223)),
            (rt - 1.0, t, c(193, 212, 232)),
            (rt, t + 1.0, c(192, 211, 232)),
            (rt - 1.0, t + 1.0, c(172, 198, 223)),
            (l, b - 1.0, c(167, 201, 228)),
            (l + 1.0, b - 1.0, c(126, 172, 210)),
            (l + 1.0, b, c(166, 201, 228)),
            (rt, b - 1.0, c(167, 201, 228)),
            (rt - 1.0, b - 1.0, c(126, 172, 210)),
            (rt - 1.0, b, c(166, 201, 228)),
        ] {
            v.push(px(x, y, x + 1.0, y + 1.0, col));
        }
        return v;
    }
    let side = [(0.0, c(194, 207, 221)), (21.0, c(169, 191, 210)), (80.0, c(157, 192, 218))];
    let light = [(0.0, c(238, 243, 250)), (12.0, c(232, 239, 248)), (13.0, c(227, 236, 246)), (66.0, c(236, 244, 250))];
    let mut v = vec![
        gradient_mesh(Rect::from_min_max(pos2(l + 2.0, t + 2.0), pos2(rt, t + 15.0)), &[(0.0, c(222, 231, 245)), (12.0, c(208, 223, 240))]),
        gradient_mesh(Rect::from_min_max(pos2(l + 2.0, t + 15.0), pos2(rt, band)), &[(0.0, c(199, 216, 237)), (band - t - 16.0, c(216, 232, 245))]),
        gradient_mesh(Rect::from_min_max(pos2(l + 2.0, band), pos2(rt, b)), &[(0.0, c(193, 215, 240)), (14.0, c(191, 216, 239))]),
        // The inner light: the top row, the left column (lighter in the band).
        px(l + 2.0, t + 1.0, rt, t + 2.0, c(239, 244, 250)),
        col(l + 1.0, t + 2.0, band, &light),
        col(l + 1.0, band, b - 1.0, &[(0.0, c(205, 223, 243)), (13.0, c(201, 220, 240))]),
        // The frame.
        px(l + 2.0, t, rt - 1.0, t + 1.0, c(196, 209, 222)),
        px(l + 2.0, b, rt - 1.0, b + 1.0, c(158, 192, 218)),
        col(l, t + 2.0, b - 1.0, &side),
        col(rt, t + 2.0, b - 1.0, &side),
        // The outer light: right of the frame and under it.
        col(rt + 1.0, t + 2.0, b, &[(0.0, c(238, 243, 250)), (12.0, c(232, 239, 248)), (13.0, c(227, 236, 246)), (66.0, c(236, 244, 250)), (80.0, c(241, 250, 255))]),
        px(l + 3.0, b + 1.0, rt - 1.0, b + 2.0, c(243, 249, 255)),
    ];
    // The rounded corners.
    for (x, y, col) in [
        (l + 1.0, t, c(207, 218, 231)),
        (l, t + 1.0, c(206, 218, 231)),
        (l + 1.0, t + 1.0, c(195, 209, 222)),
        (rt - 1.0, t, c(207, 218, 231)),
        (rt, t + 1.0, c(206, 218, 231)),
        (rt - 1.0, t + 1.0, c(194, 208, 222)),
        (l, b - 1.0, c(186, 214, 232)),
        (l + 1.0, b - 1.0, c(158, 192, 218)),
        (l + 1.0, b, c(186, 214, 233)),
        (rt, b - 1.0, c(186, 214, 232)),
        (rt - 1.0, b - 1.0, c(149, 184, 217)),
        (rt - 1.0, b, c(186, 214, 233)),
    ] {
        v.push(px(x, y, x + 1.0, y + 1.0, col));
    }
    v
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
    let ink = if selected && ui.is_enabled() { checked_ink(&p, ink) } else { ink };
    let hovered = resp.hovered() && ui.is_enabled();
    if selected {
        checked_bg(painter, rect, &p, hovered);
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
/// A main panel item: its label, the letter underlined, enabled, its
/// command, the sub-items it shows.
type MainItem<'a> = (&'a str, Option<usize>, bool, Option<Command>, Option<AppSub>);

/// The application menu's main panel (Office 2007 Blue, VM): 490 px wide,
/// the item column 174, the right column 298.
const MAIN_W: f32 = 490.0;
const MAIN_LEFT_W: f32 = 174.0;
const MAIN_RIGHT_W: f32 = 298.0;
const MAIN_RULE: Color32 = Color32::from_rgb(197, 197, 197);
const MAIN_RULE_LIGHT: Color32 = Color32::from_rgb(245, 245, 245);
/// The items' and recent documents' ink, the header's, and disabled text.
const MAIN_INK: Color32 = Color32::from_rgb(21, 66, 139);
const MAIN_TITLE_INK: Color32 = Color32::from_rgb(0, 21, 110);
const MAIN_DISABLED: Color32 = Color32::from_rgb(141, 141, 141);

/// The main panel's frame and grounds from its corner `o` down to
/// `bottom` (exclusive), as the VM shows them: a 1 px (155, 175, 202)
/// outer line, white, (196, 214, 237) — the top band's first rows a
/// gradient from (215, 229, 247) —, white and the inner line, 7 px at the
/// left, 9 at the right, 25 on top and 3 below; the item column (250), a
/// (197) / (245) divider, the right column (233, 238, 238) under a 25 px
/// (222, 232, 239) header and its rule; a 4 px shadow.
fn main_panel_ground(o: egui::Pos2, bottom: f32) -> egui::Shape {
    let px = |x0: f32, y0: f32, x1: f32, y1: f32, c: Color32| egui::Shape::rect_filled(egui::Rect::from_min_max(egui::pos2(o.x + x0, y0), egui::pos2(o.x + x1, y1)), 0.0, c);
    let (t, b) = (o.y, bottom);
    let outer = Color32::from_rgb(155, 175, 202);
    let blue = Color32::from_rgb(196, 214, 237);
    let mut v = Vec::new();
    // The shadow, right and below: three columns / rows fading out.
    for (i, a) in [97u8, 61, 28].into_iter().enumerate() {
        let i = i as f32;
        v.push(px(MAIN_W + i, t + 4.0 + i, MAIN_W + i + 1.0, b + i, Color32::from_black_alpha(a)));
        v.push(px(4.0 + i, b + i, MAIN_W + i, b + i + 1.0, Color32::from_black_alpha(a)));
    }
    v.extend([
        px(0.0, t, MAIN_W, b, outer),
        px(1.0, t + 1.0, MAIN_W - 1.0, b - 1.0, Color32::WHITE),
        px(2.0, t + 2.0, MAIN_W - 2.0, b - 1.0, blue),
    ]);
    let mut mesh = egui::Mesh::default();
    let g = egui::Rect::from_min_max(egui::pos2(o.x + 2.0, t + 2.0), egui::pos2(o.x + MAIN_W - 2.0, t + 12.0));
    for (pt, c) in [(g.left_top(), (215, 229, 247)), (g.right_top(), (215, 229, 247)), (g.right_bottom(), (196, 214, 237)), (g.left_bottom(), (196, 214, 237))] {
        mesh.colored_vertex(pt, Color32::from_rgb(c.0, c.1, c.2));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    v.push(egui::Shape::mesh(mesh));
    v.extend([
        // The bottom's middle row, then the white ring and the inner line.
        px(1.0, b - 2.0, MAIN_W - 1.0, b - 1.0, Color32::from_rgb(201, 238, 255)),
        px(5.0, t + 23.0, MAIN_W - 7.0, b - 3.0, Color32::WHITE),
        px(6.0, t + 24.0, MAIN_W - 8.0, b - 2.0, outer),
        px(7.0, t + 25.0, 181.0, b - 3.0, Color32::from_gray(250)),
        px(181.0, t + 25.0, 182.0, b - 3.0, MAIN_RULE),
        px(182.0, t + 25.0, 183.0, b - 3.0, MAIN_RULE_LIGHT),
        px(183.0, t + 25.0, 481.0, b - 3.0, Color32::from_rgb(233, 238, 238)),
        px(183.0, t + 25.0, 481.0, t + 50.0, Color32::from_rgb(222, 232, 239)),
        px(183.0, t + 50.0, 481.0, t + 51.0, MAIN_RULE),
        px(183.0, t + 51.0, 481.0, t + 52.0, MAIN_RULE_LIGHT),
    ]);
    egui::Shape::Vec(v)
}

/// `label` at `at` (left, vertical centre) with its letter `ul` underlined
/// (the main panel shows its mnemonics, VM).
fn mnemonic_text(ui: &egui::Ui, at: egui::Pos2, label: &str, ul: Option<usize>, ink: Color32) {
    let font = egui::FontId::proportional(12.0);
    let g = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), ink));
    let top = (at.y - g.size().y / 2.0).round();
    if let Some(i) = ul {
        let before: String = label.chars().take(i).collect();
        let ch: String = label.chars().skip(i).take(1).collect();
        let x0 = ui.fonts(|f| f.layout_no_wrap(before, font.clone(), ink).size().x);
        let w = ui.fonts(|f| f.layout_no_wrap(ch, font.clone(), ink).size().x);
        let y = top + g.size().y - 2.0;
        ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2((at.x + x0).round(), y), egui::pos2((at.x + x0 + w).round(), y + 1.0)), 0.0, ink);
    }
    ui.painter().galley(egui::pos2(at.x, top), g, ink);
}

/// One item of the main panel's column: 44 px, the 32 px image at the
/// left + 3, the text from + 38 (`Repos` `0x1011cea4`), MFC's hot bar
/// under the pointer, an arrow at the right for one with sub-items.
#[allow(clippy::too_many_arguments)]
fn main_entry(ui: &mut egui::Ui, p: crate::theme::Palette, icon: Option<Icon>, label: &str, ul: Option<usize>, enabled: bool, selected: bool, arrow: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(MAIN_LEFT_W, 44.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    if selected || (enabled && resp.hovered()) {
        crate::popmenu::hot_bar(ui.painter(), rect);
    }
    let ink = if enabled { MAIN_INK } else { MAIN_DISABLED };
    if let Some(icon) = icon {
        let img = egui::Rect::from_min_size(egui::pos2(rect.left() + 3.0, rect.center().y - 16.0), egui::vec2(32.0, 32.0));
        crate::icons::paint(ui.painter(), img.shrink(4.0), icon, if enabled { p.text } else { disabled_ink(p) });
    }
    mnemonic_text(ui, egui::pos2(rect.left() + 38.0, rect.center().y), label, ul, ink);
    if arrow {
        let c = egui::pos2(rect.right() - 10.0, rect.center().y);
        ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-2.0, -4.0), c + egui::vec2(2.0, 0.0), c + egui::vec2(-2.0, 4.0)], Color32::from_rgb(86, 125, 177), egui::Stroke::NONE));
    }
    resp
}

/// The right column's header: 25 px, its title bold from + 2, the rule
/// under it painted with the panel.
fn main_header(ui: &mut egui::Ui, title: &str) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(MAIN_RIGHT_W, 27.0), egui::Sense::hover());
    let font = egui::FontId::proportional(12.0);
    let h = ui.fonts(|f| f.row_height(&font));
    crate::lanes::bold_text(ui.painter(), egui::pos2(r.left() + 2.0, (r.top() + 12.5 - h / 2.0).round()), title, font, MAIN_TITLE_INK);
}

/// A row of the right column (a recent document): 30 px, the text from
/// + 3, MFC's hot bar under the pointer.
fn recent_row(ui: &mut egui::Ui, text: &str, ul: Option<usize>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(MAIN_RIGHT_W, 30.0), egui::Sense::click());
    if resp.hovered() {
        crate::popmenu::hot_bar(ui.painter(), rect);
    }
    let room = rect.width() - 6.0;
    let font = egui::FontId::proportional(12.0);
    let shown = if ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font.clone(), MAIN_INK).size().x) <= room { text.to_owned() } else { crate::dialogs::ellipsize(ui, text, font, room) };
    mnemonic_text(ui, egui::pos2(rect.left() + 3.0, rect.center().y), &shown, ul, MAIN_INK);
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
    // (Icon-only, its tooltip is still the command's: `tips`.)
    let painter = &plain_painter(ui);
    let (ink, accent) = inks(ui, p);
    let ink = if selected && ui.is_enabled() { checked_ink(&p, ink) } else { ink };
    if selected {
        checked_bg(painter, rect, &p, resp.hovered() && ui.is_enabled());
    } else if resp.hovered() && ui.is_enabled() {
        hot_small(painter, rect, &p);
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
    // As tall as the large buttons: a taller one stretched its panel. 8 px
    // (VM, Blue): a light column 1 px in and a dark one beside it, from 6
    // px under the buttons' top to 6 above their bottom.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, LARGE_H), egui::Sense::hover());
    let [light, dark] = p.bands.and_then(|b| b.panel_sep).unwrap_or([crate::theme::alpha(255, 255, 255, 160), p.group_border]);
    let col = |dx: f32| egui::Rect::from_min_max(egui::pos2(rect.left() + dx, rect.top() + 6.0), egui::pos2(rect.left() + dx + 1.0, rect.bottom() - 5.0));
    ui.painter().rect_filled(col(1.0), 0.0, light);
    ui.painter().rect_filled(col(2.0), 0.0, dark);
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

/// A checked button's text: the look's own navy in the Office 2007 looks
/// (VM: «Частотная», «Вторичные знач.» checked), elsewhere dark on the
/// light selection fill.
fn checked_ink(p: &crate::theme::Palette, ink: Color32) -> Color32 {
    if office2007(p) && ink.r() < 128 {
        ink
    } else {
        CHECKED_INK
    }
}

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

/// A small icon-only button (26×22), framed while `on` (its tooltip the
/// command's, `tips`); clicks only while enabled.
fn mini_button(ui: &mut egui::Ui, p: crate::theme::Palette, glyph: Mini, enabled: bool, on: bool) -> egui::Response {
    // Icon-only: the small image + 6 tall (`0x10101d6a`), 26 wide with the
    // image 3 px in (VM: the hot and checked faces, «Видимость» and
    // «Вычисления»'s columns on a 26 px pitch).
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 22.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let painter = &plain_painter(ui);
    let enabled = enabled && ui.is_enabled();
    if on {
        checked_bg(painter, rect, &p, enabled && resp.hovered());
    } else if enabled && resp.hovered() {
        hot_small(painter, rect, &p);
    }
    let col = if enabled { p.accent } else { disabled_ink(p) };
    paint_mini(painter, egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), egui::vec2(14.0, 14.0)), glyph, col);
    resp
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
    // The 10 px glyph box 16 px into the button, 6 px over its middle
    // (VM: the boxes at x + 16, y 9 maximised; the dash at x + 17, y 14).
    let o = egui::pos2(r.left() + 16.0, (r.center().y - 6.0).round());
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
            // Pixel centre to pixel centre: a 10 × 10 cross.
            let s = egui::Stroke::new(1.0f32, col);
            p.line_segment([o + egui::vec2(0.5, 0.5), o + egui::vec2(9.5, 9.5)], s);
            p.line_segment([o + egui::vec2(9.5, 0.5), o + egui::vec2(0.5, 9.5)], s);
        }
        (1, false) => square(0.0, 0.0, 9.0),
        (1, true) => {
            square(0.0, 2.0, 7.0);
            run(2.0, 0.0, 9.0, 0.0);
            run(9.0, 0.0, 9.0, 7.0);
        }
        _ => run(1.0, 5.0, 10.0, 5.0),
    }
}

/// The system's caption buttons as Windows 10 / 11 draw them on the DWM
/// frame (FastView's caption is DWM's with composition on, L9.150):
/// minimise, maximise / restore, close (red when hot), 46 px wide each.
fn caption_buttons(ui: &mut egui::Ui) {
    let h = ui.available_height().max(22.0);
    let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    // The buttons edge to edge (VM: 46 px apart); their glyphs grey while
    // the window is inactive (VM: (163) against (25)).
    ui.spacing_mut().item_spacing.x = 0.0;
    let focused = ui.ctx().input(|i| i.viewport().focused.unwrap_or(true));
    for k in 0..3 {
        let (r, resp) = ui.allocate_exact_size(egui::vec2(46.0, h), egui::Sense::click());
        let close = k == 0;
        let hot = resp.hovered();
        // Under the pointer (VM, maximised): a 45 × 28 face from a pixel
        // left of the button — (245) or the close button's (196, 43, 28).
        if hot {
            let face = egui::Rect::from_min_size(egui::pos2(r.left() - 1.0, r.top()), egui::vec2(45.0, 28.0));
            ui.painter().rect_filled(face, 0.0, if close { Color32::from_rgb(196, 43, 28) } else { Color32::from_gray(245) });
        }
        let col = if close && hot { Color32::WHITE } else if focused || hot { Color32::BLACK } else { Color32::from_gray(163) };
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
