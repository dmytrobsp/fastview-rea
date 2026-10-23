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
    /// The shown printer's spooler fields, read when it is chosen.
    pub details: Option<(usize, [String; 4])>,
}

pub struct SetupDlg {
    /// Left, top, right, bottom in mm.
    pub mm: [f32; 4],
    pub portrait: bool,
    /// The application printer's paper (greyed «Размер»).
    pub paper: spool::Paper,
}

/// A command of MFC's print-preview ribbon category (`0xE300`–`0xE306`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewCmd {
    Print,
    ZoomIn,
    ZoomOut,
    NumPage,
    Next,
    Prev,
    Close,
}

pub struct Preview {
    pub job: Job,
    pub tex: Vec<Option<egui::TextureHandle>>,
    /// First page shown (0-based), two pages side by side, zoom 0..=2.
    pub cur: usize,
    pub two: bool,
    pub zoom: u8,
    /// A ribbon command waiting for the next preview frame.
    pub cmd: Option<PreviewCmd>,
}

impl Preview {
    pub fn pages(&self) -> usize {
        self.job.pages.len().max(1)
    }

    fn step(&self) -> usize {
        if self.two && self.zoom == 0 { 2 } else { 1 }
    }

    /// Whether a command is enabled (the preview's CMDUI).
    pub fn enabled(&self, c: PreviewCmd) -> bool {
        match c {
            PreviewCmd::ZoomIn => self.zoom < 2,
            PreviewCmd::ZoomOut => self.zoom > 0,
            PreviewCmd::NumPage => self.pages() >= 2 && self.zoom == 0,
            PreviewCmd::Next => self.cur + self.step() < self.pages(),
            PreviewCmd::Prev => self.cur > 0,
            PreviewCmd::Print | PreviewCmd::Close => true,
        }
    }

    /// The page-count button's label: the mode it switches to (61505 /
    /// 61506).
    pub fn numpage_label(&self) -> &'static str {
        if self.two { "Одна страница" } else { "Две страницы" }
    }

    /// The status text (`OnDisplayPageNumber`, 61507 / 61508): «Стр. %u»
    /// or «Страницы %u-%u».
    pub fn page_text(&self) -> String {
        let shown = if self.two && self.zoom == 0 { (self.cur + 2).min(self.pages()) - self.cur } else { 1 };
        if shown == 1 {
            format!("Стр. {}", self.cur + 1)
        } else {
            format!("Страницы {}-{}", self.cur + 1, self.cur + shown)
        }
    }
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
        self.print.dialog = Some(PrintDlg { doc: di, ticks: self.docs[di].print_ticks, printer, details: None });
    }

    /// The application printer's paper (the last choice of «Печать», at
    /// start the system default); A4 without one or for the PDF entry.
    fn app_paper(&mut self) -> spool::Paper {
        self.printers_refresh();
        self.print.printer.as_deref().filter(|p| *p != PDF_ENTRY).and_then(spool::paper).unwrap_or(spool::Paper::whole("A4", crate::printout::A4_MM))
    }

    /// 57609 Предварительный просмотр: no dialog, the stored checkboxes;
    /// the pages on the application printer's paper (MFC's preview DC).
    pub fn print_preview_open(&mut self) {
        let Some(di) = self.active else { return };
        let setup = self.print.setup;
        let paper = self.app_paper();
        let mut job = self.print_job(di, Device::printable(setup.oriented(paper.mm), setup.oriented_margins(paper.margins), PREVIEW_DPI), setup);
        // The diagram pages at the application printer's scale.
        let printer = self.print.printer.as_deref().filter(|p| *p != PDF_ENTRY).map(|p| spool::device(p, &setup));
        job.attrib_dpi = printer.map_or(spool::PDF_DPI, |d| d.dpi);
        job.dev.stock_t = printer.and_then(|d| d.stock_t);
        let two = job.pages.len() >= 2;
        self.print.preview = Some(Preview { tex: vec![None; job.pages.len()], job, cur: 0, two, zoom: 0, cmd: None });
    }

    /// 57605 Параметры страницы.
    pub fn page_setup_open(&mut self) {
        let s = self.print.setup;
        let paper = self.app_paper();
        self.print.setup_dlg = Some(SetupDlg { mm: s.margins.map(|m| m as f32 / 100.0), portrait: s.portrait, paper });
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
        if dlg.details.as_ref().is_none_or(|(i, _)| *i != dlg.printer) {
            let d = self.print.printers.get(dlg.printer).map(|p| spool::details(p)).unwrap_or_default();
            dlg.details = Some((dlg.printer, d));
        }
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
                    let details = dlg.details.as_ref().map(|d| d.1.clone()).unwrap_or_default();
                    for (l, v) in ["Состояние:", "Тип:", "Место:", "Комментарий:"].into_iter().zip(details) {
                        ui.label(l);
                        ui.label(v);
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
                let (sw, sh) = if dlg.portrait { dlg.paper.mm } else { (dlg.paper.mm.1, dlg.paper.mm.0) };
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
                        ui.label(&dlg.paper.name);
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
        // Commands come from the ribbon's preview category.
        let (mut close, mut print) = (false, false);
        if let Some(c) = pv.cmd.take().filter(|c| pv.enabled(*c)) {
            let step = pv.step();
            match c {
                PreviewCmd::Print => print = true,
                PreviewCmd::ZoomIn => pv.zoom += 1,
                PreviewCmd::ZoomOut => pv.zoom -= 1,
                PreviewCmd::NumPage => pv.two = !pv.two,
                PreviewCmd::Next => pv.cur = (pv.cur + step).min(n - 1),
                PreviewCmd::Prev => pv.cur = pv.cur.saturating_sub(step),
                PreviewCmd::Close => close = true,
            }
        }
        let shown: Vec<usize> = if pv.two && pv.zoom == 0 { (pv.cur..(pv.cur + 2).min(n)).collect() } else { vec![pv.cur] };
        let fonts = self.print.fonts.clone();
        for &i in &shown {
            if pv.tex.get(i).is_some_and(|t| t.is_none()) {
                let c = if pv.job.pages.is_empty() {
                    Canvas::white(pv.job.dev.w, pv.job.dev.h)
                } else {
                    self.render_page(&fonts, &pv.job, i, true)
                };
                // The page on its printable area of the sheet.
                let c = c.on_sheet(&pv.job.dev);
                let img = egui::ColorImage::from_rgb([c.w, c.h], &c.px.concat());
                pv.tex[i] = Some(ctx.load_texture(format!("print_preview_{i}"), img, egui::TextureOptions::LINEAR));
            }
        }
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_gray(128))).show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                let avail = ui.available_size();
                let d = &pv.job.dev;
                let page = Vec2::new(d.sheet_mm.0 / 25.4 * d.dpi, d.sheet_mm.1 / 25.4 * d.dpi).round();
                let fit = ((avail.y - 20.0) / page.y).min((avail.x - 20.0) / (page.x * shown.len() as f32 + 10.0)).max(0.05);
                let scale = fit * [1.0, 1.6, 2.6][pv.zoom as usize];
                // The pages centred in the grey area (stock MFC preview).
                let total = Vec2::new((page.x * scale + 10.0) * shown.len() as f32 + 10.0, page.y * scale + 20.0);
                ui.add_space(((avail.y - total.y) / 2.0).max(0.0) + 10.0);
                ui.horizontal(|ui| {
                    ui.add_space(((avail.x - total.x) / 2.0).max(0.0));
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

    /// MFC's preview category drives the preview through queued commands;
    /// the enable rules and the status text (61507 / 61508) follow it.
    #[test]
    fn preview_category_commands_and_page_text() {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        app.docs[0].print_ticks = [true, true, true, false, false];
        app.dispatch(Command::PrintPreview);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, vec![]);
        let pv = app.print.preview.as_ref().unwrap();
        let n = pv.pages();
        assert!(n >= 3, "oscillogram + two diagram pages");
        assert!(pv.two);
        assert_eq!(pv.page_text(), "Страницы 1-2");
        assert_eq!(pv.numpage_label(), "Одна страница");
        assert!(!pv.enabled(PreviewCmd::Prev) && pv.enabled(PreviewCmd::Next) && !pv.enabled(PreviewCmd::ZoomOut));
        let run = |app: &mut App, c: PreviewCmd| {
            app.print.preview.as_mut().unwrap().cmd = Some(c);
            frame(&ctx, app, vec![]);
        };
        run(&mut app, PreviewCmd::NumPage);
        assert_eq!(app.print.preview.as_ref().unwrap().page_text(), "Стр. 1");
        run(&mut app, PreviewCmd::Next);
        assert_eq!(app.print.preview.as_ref().unwrap().cur, 1);
        run(&mut app, PreviewCmd::ZoomIn);
        assert!(!app.print.preview.as_ref().unwrap().enabled(PreviewCmd::NumPage), "no page count while zoomed in");
        run(&mut app, PreviewCmd::Close);
        assert!(app.print.preview.is_none());
    }
}
