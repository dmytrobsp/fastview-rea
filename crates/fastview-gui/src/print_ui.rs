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
use eframe::egui::{self, Color32, Vec2};

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
    /// The system default printer the dialog starts from (its
    /// RETURNDEFAULT pass, `0x448a90`); OK makes it the application's.
    pub default: Option<String>,
    /// The application printer's paper (greyed «Размер»).
    pub paper: spool::Paper,
    /// The four margin edits' texts (left, top, right, bottom); a changed
    /// one is read into `mm`.
    pub mm_text: [String; 4],
}

/// A margin as the page-setup edits show it: whole millimetres plain,
/// else up to two decimals with the decimal comma.
fn mm_text(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{:.2}", v).trim_end_matches('0').replace('.', ",")
    }
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
    /// Each page as rendered (on its sheet, at the preview resolution).
    sheets: Vec<Option<Canvas>>,
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
    /// or «Страницы %u-%u» — sent to the message bar, which under the
    /// ribbon status bar is never shown (L9.147); kept for the tests.
    #[cfg(test)]
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

/// mfc120rus string 61702.
pub const MSG_PRINT_FAILED: &str = "Не удалось запустить задание печати.";

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
        self.print.preview = Some(Preview { tex: vec![None; job.pages.len()], sheets: vec![None; job.pages.len()], job, cur: 0, two, zoom: 0, cmd: None });
    }

    /// 57605 Параметры страницы.
    ///
    /// A fresh CPageSetupDialog (0x260a) each time: its RETURNDEFAULT pass
    /// takes the SYSTEM DEFAULT printer — not the application's — with the
    /// stored orientation written into its DEVMODE (`0x448b15`–`0x448b20`).
    pub fn page_setup_open(&mut self) {
        let s = self.print.setup;
        let (_, default) = spool::printers();
        let paper = default.as_deref().and_then(spool::paper).unwrap_or(spool::Paper::whole("A4", crate::printout::A4_MM));
        self.print.setup_dlg = Some(SetupDlg { mm: s.margins.map(|m| m as f32 / 100.0), portrait: s.portrait, paper, default, mm_text: s.margins.map(|m| mm_text(m as f32 / 100.0)) });
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
                Ok(()) => {}
                // MFC's OnFilePrint: a printer DC that fails or a StartDoc
                // of −1 shows string 61702 in an MB_OK box (`0x10232bcf`,
                // `0x1023303b`); the cause is not shown.
                Err(_) => self.dlg.push_alert(MSG_PRINT_FAILED),
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
        crate::dialogs::dlg(ctx, "Печать").margin(0.0).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            use crate::dialogs::{dlu, win_button, win_button_en, win_check, win_combo_list, win_group, win_label};
            // The 341 × 258 DLU template (512 × 419 px); «Печать в файл»
            // (1040) is hidden, edit 1154 off the client.
            let (client, _) = ui.allocate_exact_size(Vec2::new(512.0, 419.0), egui::Sense::hover());
            win_group(ui, dlu(client, 8, 4, 323, 97), "Принтер");
            win_label(ui, dlu(client, 16, 20, 36, 8), "Имя:", false, true);
            if let Some(k) = win_combo_list(ui, dlu(client, 70, 18, 180, 152), egui::Id::new("print_name"), &names, dlg.printer.min(names.len() - 1), true) {
                dlg.printer = k;
            }
            // The driver's dialog (Windows); the chosen settings apply to
            // this printer's jobs.
            let real = self.print.printers.get(dlg.printer).cloned();
            if win_button_en(ui, dlu(client, 258, 17, 60, 14), "Свойства...", false, cfg!(windows) && real.is_some()).clicked() {
                if let Some(p) = real {
                    spool::properties(&p);
                    dlg.details = None;
                }
            }
            let details = dlg.details.as_ref().map(|d| d.1.clone()).unwrap_or_default();
            let rows = [((16, 36, 36), (71, 36, 224), "Состояние:"), ((16, 51, 36), (72, 51, 224), "Тип:"), ((16, 66, 36), (72, 66, 224), "Место:"), ((16, 81, 50), (72, 82, 228), "Комментарий:")];
            for (k, ((lx, ly, lw), (vx, vy, vw), l)) in rows.into_iter().enumerate() {
                win_label(ui, dlu(client, lx, ly, lw, 10), l, false, true);
                if let Some(v) = details.get(k) {
                    win_label(ui, dlu(client, vx, vy, vw, 10), v, false, true);
                }
            }
            setup = win_button(ui, dlu(client, 7, 107, 120, 14), "Параметры страницы", false).clicked();
            win_group(ui, dlu(client, 8, 125, 322, 108), "Диаграмма");
            for ((t, l), (y, w)) in dlg.ticks.iter_mut().zip(TICK_LABELS).zip([(144, 68), (161, 90), (178, 90), (195, 86), (212, 122)]) {
                if win_check(ui, dlu(client, 16, y, w, 10), l, *t) {
                    *t = !*t;
                }
            }
            preview = win_button(ui, dlu(client, 7, 238, 120, 14), "Предварительный просмотр", false).clicked();
            ok = win_button(ui, dlu(client, 189, 238, 67, 14), "OK", true).clicked();
            cancel = win_button(ui, dlu(client, 262, 238, 67, 14), "Отмена", false).clicked();
        });
        // The checkboxes are kept per view, also on Отмена (`0x404b30`).
        self.docs[dlg.doc].print_ticks = dlg.ticks;
        if setup {
            self.page_setup_open();
        }
        if ok || preview {
            self.print.printer = Some(names[dlg.printer.min(names.len() - 1)].clone());
        }
        let closed = !open || cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if ok || preview || closed {
            spool::end_properties(ok || preview);
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
        } else if !closed {
            self.print.dialog = Some(dlg);
        }
    }

    fn page_setup_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.print.setup_dlg.take() else { return };
        let (mut ok, mut cancel) = (false, false);
        let mut open = true;
        crate::dialogs::modal_backdrop(ctx, "fv_page_setup_block", egui::Order::Middle);
        // Windows' own Page Setup (PageSetupDlg, comdlg32's template, in
        // the system's language), measured on the VM: a 360 × 390 client
        // in the dialog frame, placed as the others.
        crate::dialogs::dlg(ctx, "Параметры страницы").margin(0.0).collapsible(false).resizable(false).open(&mut open).order(egui::Order::Foreground).show(ctx, |ui| {
            use crate::dialogs::{win_button, win_combo_list, win_edit, win_group, win_label, win_radio};
            let (client, _) = ui.allocate_exact_size(Vec2::new(360.0, 390.0), egui::Sense::hover());
            let at = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(client.min + Vec2::new(x, y), Vec2::new(w, h));
            // The sample page: A4's 91 × 130 px at (134, 13) (VM), across
            // when landscape, centred in the same place; a (128) frame at
            // its left and top, black at its right and bottom, an 8 px
            // (160) shadow 8 px down and right. The original paints its
            // bitmaps 345 / 346 inside (the page-paint hook); ours the
            // margins.
            let (w, h) = if dlg.portrait { (91.0, 130.0) } else { (130.0, 91.0) };
            let r = egui::Rect::from_center_size(client.min + Vec2::new(179.5, 77.5), Vec2::new(w, h));
            let p = ui.painter();
            p.rect_filled(r.translate(Vec2::splat(8.0)), 0.0, Color32::from_gray(160));
            p.rect_filled(r, 0.0, Color32::BLACK);
            p.rect_filled(egui::Rect::from_min_max(r.min, r.max - Vec2::splat(1.0)), 0.0, Color32::from_gray(128));
            p.rect_filled(egui::Rect::from_min_max(r.min + Vec2::splat(1.0), r.max - Vec2::splat(1.0)), 0.0, Color32::WHITE);
            let (sw, sh) = if dlg.portrait { dlg.paper.mm } else { (dlg.paper.mm.1, dlg.paper.mm.0) };
            let m = egui::Rect::from_min_max(r.min + Vec2::new(dlg.mm[0] / sw * w, dlg.mm[1] / sh * h), r.max - Vec2::new(dlg.mm[2] / sw * w, dlg.mm[3] / sh * h));
            p.rect_stroke(m, 0.0, egui::Stroke::new(1.0f32, Color32::from_gray(160)));
            win_group(ui, at(12.0, 156.0, 336.0, 91.0), "Бумага");
            win_label(ui, at(24.0, 182.0, 70.0, 14.0), "Размер:", false, false);
            win_combo_list(ui, at(96.0, 179.0, 240.0, 21.0), egui::Id::new("fv_ps_size"), std::slice::from_ref(&dlg.paper.name), 0, false);
            win_label(ui, at(24.0, 214.0, 70.0, 14.0), "Подача:", false, false);
            win_combo_list(ui, at(96.0, 211.0, 240.0, 21.0), egui::Id::new("fv_ps_source"), &[], 0, false);
            win_group(ui, at(12.0, 254.0, 96.0, 91.0), "Ориентация");
            if win_radio(ui, at(24.0, 277.0, 80.0, 16.0), "Книжная", dlg.portrait) {
                dlg.portrait = true;
            }
            if win_radio(ui, at(24.0, 310.0, 80.0, 16.0), "Альбомная", !dlg.portrait) {
                dlg.portrait = false;
            }
            win_group(ui, at(120.0, 254.0, 228.0, 91.0), "Поля (мм)");
            for (k, (lx, ly, label)) in [(131.0, 279.0, "Левое:"), (131.0, 312.0, "Верхнее:"), (245.0, 279.0, "Правое:"), (245.0, 312.0, "Нижнее:")].into_iter().enumerate() {
                win_label(ui, at(lx, ly, 44.0, 14.0), label, false, true);
                if win_edit(ui, at(lx + 46.0, ly - 3.0, 48.0, 20.0), egui::Id::new(("fv_ps_mm", k)), &mut dlg.mm_text[k], false).changed() {
                    if let Ok(v) = dlg.mm_text[k].trim().replace(',', ".").parse::<f32>() {
                        dlg.mm[k] = v.clamp(0.0, 100.0);
                    }
                }
            }
            // PSD_SHOWHELP: «Справка» (the VM shows it); it opens nothing here.
            win_button(ui, at(12.0, 358.0, 72.0, 23.0), "Справка", false);
            ok = win_button(ui, at(198.0, 358.0, 72.0, 23.0), "ОК", true).clicked();
            cancel = win_button(ui, at(276.0, 358.0, 72.0, 23.0), "Отмена", false).clicked();
        });
        if ok {
            self.print.setup = PageSetup { margins: dlg.mm.map(|m| (m * 100.0).round() as i32), portrait: dlg.portrait };
            self.print.setup.store(&mut self.store.view);
            // SelectPrinter (#12964, `0x448ba3`): the default printer with
            // this orientation becomes the application printer, an earlier
            // «Свойства...» DEVMODE dropped.
            if let Some(d) = dlg.default.clone() {
                spool::forget_properties(&d);
                self.print.printer = Some(d);
            }
            if self.persist {
                self.store.save();
            }
        } else if open && !cancel {
            self.print.setup_dlg = Some(dlg);
        }
    }

    /// The print preview in place of the views (CPreviewViewEx): its
    /// toolbar, then one or two pages on grey.
    pub fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let Some(mut pv) = self.print.preview.take() else { return };
        if pv.job.di >= self.docs.len() {
            return;
        }
        let n = pv.job.pages.len().max(1);
        // Commands come from the ribbon's preview category and MFC's
        // accelerator table 30997: PgDn next page, PgUp previous, Esc close.
        if pv.cmd.is_none() {
            pv.cmd = ctx.input(|i| {
                if i.key_pressed(egui::Key::PageDown) {
                    Some(PreviewCmd::Next)
                } else if i.key_pressed(egui::Key::PageUp) {
                    Some(PreviewCmd::Prev)
                } else {
                    None
                }
            });
        }
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
        // The scale the pages are shown at (as laid out below).
        let avail = ui.available_size();
        let page = {
            let d = &pv.job.dev;
            Vec2::new(d.sheet_mm.0 / 25.4 * d.dpi, d.sheet_mm.1 / 25.4 * d.dpi).round()
        };
        let fit = ((avail.y - 20.0) / page.y).min((avail.x - 20.0) / (page.x * shown.len() as f32 + 10.0)).max(0.05);
        let scale = fit * [1.0, 1.6, 2.6][pv.zoom as usize];
        let ppp = ctx.pixels_per_point();
        for &i in &shown {
            if pv.sheets.get(i).is_some_and(|c| c.is_none()) {
                let c = if pv.job.pages.is_empty() {
                    Canvas::white(pv.job.dev.w, pv.job.dev.h)
                } else {
                    self.render_page(&fonts, &pv.job, i, true)
                };
                // The page on its printable area of the sheet.
                pv.sheets[i] = Some(c.on_sheet(&pv.job.dev));
                pv.tex[i] = None;
            }
            // MFC draws the page at the scale shown, every line kept: ours
            // shrinks the rendered page to the pixels shown by averaging
            // (a texture's linear filter skips thin lines when it shrinks
            // by more than half).
            let want = [((page.x * scale * ppp).round() as usize).max(1), ((page.y * scale * ppp).round() as usize).max(1)];
            let stale = pv.tex.get(i).and_then(|t| t.as_ref()).is_none_or(|t| t.size() != want && want[0] < pv.sheets[i].as_ref().map_or(0, |c| c.w));
            if stale {
                if let Some(c) = &pv.sheets[i] {
                    let shown_px = if want[0] < c.w { c.shrink(want[0], want[1]) } else { c.clone() };
                    let img = egui::ColorImage::from_rgb([shown_px.w, shown_px.h], &shown_px.px.concat());
                    pv.tex[i] = Some(ctx.load_texture(format!("print_preview_{i}"), img, egui::TextureOptions::LINEAR));
                }
            }
        }
        // In the document's window, over the MDI client's colour (VM: the
        // panes, the parameters strip and the tab row stay).
        let bg = self.skin.palette().work_bg;
        egui::Frame::none().fill(bg).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            egui::ScrollArea::both().show(ui, |ui| {
                // The pages centred in the grey area (stock MFC preview).
                let total = Vec2::new((page.x * scale + 10.0) * shown.len() as f32 + 10.0, page.y * scale + 20.0);
                ui.add_space(((avail.y - total.y) / 2.0).max(0.0) + 10.0);
                ui.horizontal(|ui| {
                    ui.add_space(((avail.x - total.x) / 2.0).max(0.0));
                    for &i in &shown {
                        if let Some(Some(t)) = pv.tex.get(i) {
                            ui.add_space(10.0);
                            let r = ui.add(egui::Image::new((t.id(), page * scale)).bg_fill(Color32::WHITE)).rect;
                            // The sheet's frame and its shadow (VM: (100,
                            // 100, 100), then 3 px of (160, 160, 160)).
                            let p = ui.painter();
                            p.rect_filled(egui::Rect::from_min_max(egui::pos2(r.right() + 1.0, r.top() + 3.0), egui::pos2(r.right() + 4.0, r.bottom() + 4.0)), 0.0, Color32::from_gray(160));
                            p.rect_filled(egui::Rect::from_min_max(egui::pos2(r.left() + 3.0, r.bottom() + 1.0), egui::pos2(r.right() + 4.0, r.bottom() + 4.0)), 0.0, Color32::from_gray(160));
                            p.rect_stroke(r.expand(0.5), 0.0, egui::Stroke::new(1.0f32, Color32::from_gray(100)));
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
                egui::CentralPanel::default().show(ctx, |ui| app.preview_ui(ui));
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
        // MFC's table 30997: PgUp / PgDn page.
        let key = |k| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        frame(&ctx, &mut app, vec![key(egui::Key::PageUp)]);
        assert_eq!(app.print.preview.as_ref().unwrap().cur, 0, "PgUp");
        frame(&ctx, &mut app, vec![key(egui::Key::PageDown)]);
        assert_eq!(app.print.preview.as_ref().unwrap().cur, 1, "PgDn");
        run(&mut app, PreviewCmd::ZoomIn);
        assert!(!app.print.preview.as_ref().unwrap().enabled(PreviewCmd::NumPage), "no page count while zoomed in");
        run(&mut app, PreviewCmd::Close);
        assert!(app.print.preview.is_none());
    }
}
