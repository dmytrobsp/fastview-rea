//! The print dialogs and the preview (ledger L9.22, spec §7.11): «Печать»
//! (DIALOG 347 on CPrintDialog: printer, «Параметры страницы», the five
//! «Диаграмма» checkboxes, «Предварительный просмотр»), page setup (57605,
//! `0x448a90`: orientation and margins, paper greyed) and the print preview
//! (57609, `AFXPrintPreview`: Печать, Увеличить / Уменьшить, Две страницы,
//! Следующая / Предыдущая страница, Закрыть).

use crate::app::App;
use crate::printout::{Device, Job, PageSetup};
use crate::raster::Canvas;
use crate::spool;
use eframe::egui::{self, Color32, RichText, Vec2};

/// Our extension: a PDF file as the last «Имя» entry.
pub const PDF_ENTRY: &str = "Файл PDF… (FastView-REA)";

#[derive(Default)]
pub struct PrintUi {
    pub setup: PageSetup,
    pub dialog: Option<PrintDlg>,
    pub setup_dlg: Option<SetupDlg>,
    pub preview: Option<Preview>,
    /// The system printers and the default (read when the dialog opens).
    pub printers: Vec<String>,
    /// The printer of the next job (the dialog's last choice; at start the
    /// system default).
    pub printer: Option<String>,
    /// The fonts the window uses, for painting pages off screen.
    pub fonts: egui::FontDefinitions,
}

pub struct PrintDlg {
    pub doc: usize,
    pub ticks: [bool; 5],
    /// Index into the printers, `printers.len()` = the PDF file.
    pub printer: usize,
}

pub struct SetupDlg {
    /// Left, top, right, bottom in mm.
    pub mm: [f32; 4],
    pub portrait: bool,
}

pub struct Preview {
    pub job: Job,
    pub tex: Vec<Option<egui::TextureHandle>>,
    /// First page shown (0-based), two pages side by side, zoom 0..=2.
    pub cur: usize,
    pub two: bool,
    pub zoom: u8,
}

/// Preview resolution.
const PREVIEW_DPI: f32 = 110.0;

const TICK_LABELS: [&str; 5] = ["Осциллограмма", "Частотная диаграмма", "Векторная диаграмма", "Круговая диаграмма", "Дифференциальная диаграмма"];

impl App {
    fn printers_refresh(&mut self) {
        let (list, default) = spool::printers();
        self.print.printers = list;
        if self.print.printer.as_ref().is_none_or(|p| p != PDF_ENTRY && !self.print.printers.contains(p)) {
            self.print.printer = default.filter(|d| self.print.printers.contains(d)).or_else(|| self.print.printers.first().cloned());
        }
    }

    /// 57607 Печать (Ctrl+P): the dialog with the view's checkboxes.
    pub fn print_open(&mut self) {
        let Some(di) = self.active else { return };
        self.printers_refresh();
        let n = self.print.printers.len();
        let printer = self.print.printer.as_ref().and_then(|p| self.print.printers.iter().position(|x| x == p)).unwrap_or(n);
        self.print.dialog = Some(PrintDlg { doc: di, ticks: self.docs[di].print_ticks, printer });
    }

    /// 57609 Предварительный просмотр: no dialog, the stored checkboxes.
    pub fn print_preview_open(&mut self) {
        let Some(di) = self.active else { return };
        let setup = self.print.setup;
        let job = self.print_job(di, Device::sheet(&setup, PREVIEW_DPI), setup);
        let two = job.pages.len() >= 2;
        self.print.preview = Some(Preview { tex: vec![None; job.pages.len()], job, cur: 0, two, zoom: 0 });
    }

    /// 57605 Параметры страницы.
    pub fn page_setup_open(&mut self) {
        let s = self.print.setup;
        self.print.setup_dlg = Some(SetupDlg { mm: s.margins.map(|m| m as f32 / 100.0), portrait: s.portrait });
    }

    /// Render and send a job for document `di` (`printer` None = PDF file).
    fn print_run(&mut self, di: usize, printer: Option<String>) {
        let setup = self.print.setup;
        let dev = match &printer {
            Some(p) => spool::device(p, &setup),
            None => Device::sheet(&setup, spool::PDF_DPI),
        };
        let job = self.print_job(di, dev, setup);
        let fonts = self.print.fonts.clone();
        // With nothing ticked MFC still prints one empty sheet.
        let imgs: Vec<Canvas> = if job.pages.is_empty() {
            vec![Canvas::white(dev.w, dev.h)]
        } else {
            (0..job.pages.len()).map(|i| self.render_page(&fonts, &job, i, false)).collect()
        };
        let title = self.docs[di].name.clone();
        match printer {
            Some(p) => match spool::print(&p, &title, &imgs, &dev, &setup) {
                Ok(()) => self.status = format!("Отправлено на печать: {} стр.", imgs.len()),
                Err(e) => self.dlg.alert = Some(format!("Ошибка печати: {e}")),
            },
            None => {
                if let Some(path) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{title}.pdf")).save_file() {
                    self.status = match std::fs::write(&path, spool::pdf_bytes(&imgs, &dev)) {
                        Ok(()) => format!("Сохранено: {}", path.display()),
                        Err(e) => format!("Ошибка записи: {e}"),
                    };
                }
            }
        }
    }

    /// The print and page-setup dialogs.
    pub fn print_windows(&mut self, ctx: &egui::Context) {
        self.print_dialog_ui(ctx);
        self.page_setup_ui(ctx);
    }

    fn print_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.print.dialog.take() else { return };
        if dlg.doc >= self.docs.len() {
            return;
        }
        let names: Vec<String> = self.print.printers.iter().cloned().chain(Some(PDF_ENTRY.to_string())).collect();
        let (mut ok, mut cancel, mut preview, mut setup) = (false, false, false, false);
        let mut open = true;
        egui::Window::new("Печать").collapsible(false).resizable(false).open(&mut open).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            ui.group(|ui| {
                ui.label(RichText::new("Принтер").strong());
                egui::Grid::new("print_printer").num_columns(2).show(ui, |ui| {
                    ui.label("Имя:");
                    egui::ComboBox::from_id_salt("print_name").width(260.0).selected_text(names[dlg.printer.min(names.len() - 1)].clone()).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut dlg.printer, i, n);
                        }
                    });
                    ui.end_row();
                    for l in ["Состояние:", "Тип:", "Место:", "Комментарий:"] {
                        ui.label(l);
                        ui.label("");
                        ui.end_row();
                    }
                });
                ui.add_enabled(false, egui::Button::new("Свойства..."));
            });
            setup = ui.button("Параметры страницы").clicked();
            ui.group(|ui| {
                ui.label(RichText::new("Диаграмма").strong());
                for (t, l) in dlg.ticks.iter_mut().zip(TICK_LABELS) {
                    ui.checkbox(t, l);
                }
            });
            ui.horizontal(|ui| {
                preview = ui.button("Предварительный просмотр").clicked();
                ok = ui.button("   OK   ").clicked();
                cancel = ui.button("Отмена").clicked();
            });
        });
        // The checkboxes are kept per view, also on Отмена (`0x404b30`).
        self.docs[dlg.doc].print_ticks = dlg.ticks;
        if setup {
            self.page_setup_open();
        }
        if ok || preview {
            self.print.printer = Some(names[dlg.printer.min(names.len() - 1)].clone());
        }
        if ok {
            let printer = (dlg.printer < self.print.printers.len()).then(|| self.print.printers[dlg.printer].clone());
            self.print_run(dlg.doc, printer);
        } else if preview {
            // «Предварительный просмотр» (`0x404960`): ends the dialog and
            // opens the preview with the boxes just ticked.
            let active = self.active;
            self.active = Some(dlg.doc);
            self.print_preview_open();
            self.active = active;
        } else if open && !cancel && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.print.dialog = Some(dlg);
        }
    }

    fn page_setup_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.print.setup_dlg.take() else { return };
        let (mut ok, mut cancel) = (false, false);
        let mut open = true;
        crate::dialogs::modal_backdrop(ctx, "fv_page_setup_block", egui::Order::Middle);
        egui::Window::new("Параметры страницы").collapsible(false).resizable(false).open(&mut open).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).order(egui::Order::Foreground).show(ctx, |ui| {
            // The sample page (bitmaps 345 / 346 in the original).
            let (w, h) = if dlg.portrait { (84.0, 119.0) } else { (119.0, 84.0) };
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
                let p = ui.painter();
                p.rect_filled(r.translate(Vec2::splat(3.0)), 0.0, Color32::GRAY);
                p.rect_filled(r, 0.0, Color32::WHITE);
                p.rect_stroke(r, 0.0, egui::Stroke::new(1.0f32, Color32::BLACK));
                let (sw, sh) = if dlg.portrait { (210.0, 297.0) } else { (297.0, 210.0) };
                let m = egui::Rect::from_min_max(
                    r.min + Vec2::new(dlg.mm[0] / sw * w, dlg.mm[1] / sh * h),
                    r.max - Vec2::new(dlg.mm[2] / sw * w, dlg.mm[3] / sh * h),
                );
                p.rect_stroke(m, 0.0, egui::Stroke::new(1.0f32, Color32::from_gray(160)));
            });
            ui.add_enabled_ui(false, |ui| {
                ui.group(|ui| {
                    ui.label(RichText::new("Бумага").strong());
                    ui.horizontal(|ui| {
                        ui.label("Размер:");
                        ui.label("A4");
                    });
                });
            });
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Ориентация").strong());
                        ui.radio_value(&mut dlg.portrait, true, "Книжная");
                        ui.radio_value(&mut dlg.portrait, false, "Альбомная");
                    });
                });
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Поля (мм)").strong());
                        egui::Grid::new("print_margins").num_columns(4).show(ui, |ui| {
                            for pair in [[(0usize, "Левое:"), (2, "Правое:")], [(1, "Верхнее:"), (3, "Нижнее:")]] {
                                for (i, l) in pair {
                                    ui.label(l);
                                    ui.add(egui::DragValue::new(&mut dlg.mm[i]).range(0.0..=100.0).speed(0.5).fixed_decimals(1));
                                }
                                ui.end_row();
                            }
                        });
                    });
                });
            });
            ui.horizontal(|ui| {
                ok = ui.button("   OK   ").clicked();
                cancel = ui.button("Отмена").clicked();
            });
        });
        if ok {
            self.print.setup = PageSetup { margins: dlg.mm.map(|m| (m * 100.0).round() as i32), portrait: dlg.portrait };
            self.print.setup.store(&mut self.store.view);
            if self.persist {
                self.store.save();
            }
        } else if open && !cancel {
            self.print.setup_dlg = Some(dlg);
        }
    }

    /// The print preview in place of the views (CPreviewViewEx): its
    /// toolbar, then one or two pages on grey.
    pub fn preview_ui(&mut self, ctx: &egui::Context) {
        let Some(mut pv) = self.print.preview.take() else { return };
        if pv.job.di >= self.docs.len() {
            return;
        }
        let n = pv.job.pages.len().max(1);
        let (mut close, mut print) = (false, false);
        egui::TopBottomPanel::top("print_preview_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                print = ui.button("Печать").clicked();
                ui.separator();
                if ui.add_enabled(pv.zoom < 2, egui::Button::new("Увеличить")).clicked() {
                    pv.zoom += 1;
                }
                if ui.add_enabled(pv.zoom > 0, egui::Button::new("Уменьшить")).clicked() {
                    pv.zoom -= 1;
                }
                let label = if pv.two { "Одна страница" } else { "Две страницы" };
                if ui.add_enabled(n >= 2 && pv.zoom == 0, egui::Button::new(label)).clicked() {
                    pv.two = !pv.two;
                }
                ui.separator();
                let step = if pv.two && pv.zoom == 0 { 2 } else { 1 };
                if ui.add_enabled(pv.cur + step < n, egui::Button::new("Следующая страница")).clicked() {
                    pv.cur = (pv.cur + step).min(n - 1);
                }
                if ui.add_enabled(pv.cur > 0, egui::Button::new("Предыдущая страница")).clicked() {
                    pv.cur = pv.cur.saturating_sub(step);
                }
                ui.separator();
                close = ui.button("Закрыть").clicked();
                ui.label(format!("Страница {} из {}", pv.cur + 1, n));
            });
        });
        let shown: Vec<usize> = if pv.two && pv.zoom == 0 { (pv.cur..(pv.cur + 2).min(n)).collect() } else { vec![pv.cur] };
        let fonts = self.print.fonts.clone();
        for &i in &shown {
            if pv.tex.get(i).is_some_and(|t| t.is_none()) {
                let c = if pv.job.pages.is_empty() {
                    Canvas::white(pv.job.dev.w, pv.job.dev.h)
                } else {
                    self.render_page(&fonts, &pv.job, i, true)
                };
                let img = egui::ColorImage::from_rgb([c.w, c.h], &c.px.concat());
                pv.tex[i] = Some(ctx.load_texture(format!("print_preview_{i}"), img, egui::TextureOptions::LINEAR));
            }
        }
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_gray(128))).show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                let avail = ui.available_size();
                let page = Vec2::new(pv.job.dev.w as f32, pv.job.dev.h as f32);
                let fit = ((avail.y - 20.0) / page.y).min((avail.x - 20.0) / (page.x * shown.len() as f32 + 10.0)).max(0.05);
                let scale = fit * [1.0, 1.6, 2.6][pv.zoom as usize];
                ui.horizontal(|ui| {
                    for &i in &shown {
                        if let Some(Some(t)) = pv.tex.get(i) {
                            ui.add_space(10.0);
                            ui.add(egui::Image::new((t.id(), page * scale)).bg_fill(Color32::WHITE));
                        }
                    }
                });
            });
        });
        if print {
            // The preview's Печать closes it and runs 57607.
            let active = self.active;
            self.active = Some(pv.job.di);
            self.print_open();
            self.active = active;
        } else if !close && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.print.preview = Some(pv);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Command;
    use crate::printout::PageKind;

    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1200.0, 900.0))), events, ..Default::default() };
        let _ = ctx.run(raw, |ctx| {
            if app.print.preview.is_some() {
                app.preview_ui(ctx);
            }
            app.print_windows(ctx);
        });
    }

    #[test]
    fn dialog_keeps_the_ticks_and_preview_shows_the_job() {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        app.dispatch(Command::Print);
        let dlg = app.print.dialog.as_mut().expect("the dialog opens");
        assert_eq!(dlg.ticks, [true, false, false, false, false], "Осциллограмма on by default");
        dlg.ticks[2] = true;
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, vec![]);
        assert!(app.docs[0].print_ticks[2], "kept per document");
        // Escape cancels; the ticks stay.
        frame(&ctx, &mut app, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        assert!(app.print.dialog.is_none());
        assert!(app.docs[0].print_ticks[2]);
        // 57609 skips the dialog: the oscillogram pages, then the vector page.
        app.dispatch(Command::PrintPreview);
        let pv = app.print.preview.as_ref().expect("the preview opens");
        assert_eq!(pv.job.pages.last(), Some(&PageKind::Diagram(crate::app::Pane::Vector)));
        assert_eq!(pv.two, pv.job.pages.len() >= 2);
        frame(&ctx, &mut app, vec![]);
        let pv = app.print.preview.as_ref().unwrap();
        assert!(pv.tex[0].is_some(), "the first page is rendered");
        // Page setup: OK stores the margins.
        app.dispatch(Command::PageSetup);
        app.print.setup_dlg.as_mut().unwrap().mm[0] = 10.0;
        app.print.setup_dlg.as_mut().unwrap().portrait = false;
        assert!(app.print.setup_dlg.is_some());
    }
}
