//! FastView — a cross-platform reconstruction of НТЦ «Механотроника» Fast View 5,
//! the relay-protection oscillogram/characteristic analysis tool. Office-2007
//! ribbon shell, MDI documents and a lane-based oscillogram view. Builds and runs
//! on Linux, Windows and macOS.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod chandlg;
mod chansel;
mod comments;
mod diagram;
mod diagram_ui;
mod dialogs;
mod finder;
mod formula_dlg;
mod icons;
mod lanes;
mod merge_ui;
mod omp_ui;
mod oscgeom;
mod persist;
mod popmenu;
mod quick_ui;
mod recdlg;
mod ribbon;
mod setfile;
mod theme;
mod util;
mod views;

use app::App;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([866.0, 500.0])
            .with_title("Механотроника FastView")
            .with_drag_and_drop(true),
        ..Default::default()
    };
    // Command line (`InitInstance` 0x4483f0): `-Dst <dir>`, `-Find` (open
    // Проводник at start-up) and the file to open.
    let args = startup_args(std::env::args().skip(1));
    eframe::run_native(
        "FastView",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            let mut app = App::new().with_persistence();
            if let Some(dir) = &args.dst {
                app.dst_dir = Some(dir.clone());
            }
            if args.find {
                app.dispatch(app::Command::ToggleExplorer);
            }
            for f in &args.files {
                app.load_comtrade_path(f);
            }
            Ok(Box::new(app))
        }),
    )
}

fn install_fonts(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "C:/Windows/Fonts/segoeui.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cyr".to_owned(), egui::FontData::from_owned(bytes));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .insert(0, "cyr".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}

impl App {
    /// The tabbed-MDI strip: OneNote-style tabs coloured per document, the
    /// close button on the active tab only, and the ▾ document list.
    fn doc_tabs(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            for i in 0..self.docs.len() {
                let active = self.active == Some(i);
                let title = self.docs[i].name.clone();
                let font = egui::FontId::proportional(13.0);
                let text_w = ui.fonts(|f| f.layout_no_wrap(title.clone(), font.clone(), egui::Color32::BLACK).size().x);
                let w = text_w + 34.0 + if active { 20.0 } else { 0.0 };
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::click());
                let base = tab_color(i);
                let top = base.lerp_to_gamma(egui::Color32::WHITE, if active { 0.45 } else { 0.25 });
                // Slanted left edge, square right edge (OneNote style).
                let pts = [rect.left_bottom(), rect.left_top() + egui::vec2(16.0, 0.0), rect.right_top(), rect.right_bottom()];
                let mut mesh = egui::Mesh::default();
                for (p, c) in pts.iter().zip([base, top, top, base]) {
                    mesh.colored_vertex(*p, if active { c } else { c.gamma_multiply(0.85) });
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(mesh));
                ui.painter().add(egui::Shape::line(pts.to_vec(), egui::Stroke::new(1.0f32, egui::Color32::from_gray(140))));
                let text_pos = egui::pos2(rect.left() + 20.0, rect.center().y);
                ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, &title, font.clone(), egui::Color32::BLACK);
                ui.painter().text(text_pos + egui::vec2(0.5, 0.0), egui::Align2::LEFT_CENTER, &title, font, egui::Color32::BLACK);
                if active {
                    let xr = egui::Rect::from_center_size(egui::pos2(rect.right() - 12.0, rect.center().y), egui::vec2(14.0, 14.0));
                    let xresp = ui.interact(xr, ui.id().with(("tab_close", i)), egui::Sense::click());
                    let col = if xresp.hovered() { egui::Color32::from_rgb(200, 0, 0) } else { egui::Color32::from_gray(90) };
                    ui.painter().text(xr.center(), egui::Align2::CENTER_CENTER, "✕", egui::FontId::proportional(11.0), col);
                    if xresp.clicked() {
                        close = Some(i);
                    }
                }
                if resp.clicked() && close.is_none() {
                    activate = Some(i);
                }
                ui.add_space(-6.0);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("▾", |ui| {
                    for i in 0..self.docs.len() {
                        if ui.radio(self.active == Some(i), self.docs[i].name.clone()).clicked() {
                            activate = Some(i);
                            ui.close_menu();
                        }
                    }
                });
            });
        });
        if let Some(i) = activate {
            self.dispatch(app::Command::ActivateDoc(i));
        }
        if let Some(i) = close {
            self.dispatch(app::Command::CloseDoc(i));
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.skin.palette();
        let s = self.cursor_status();
        ui.horizontal(|ui| {
            // Six fixed-width panes (27 / 40-character templates, `0x4729b0`).
            let char_w = 6.2;
            for (i, text) in s.iter().enumerate() {
                let w = if i == 2 || i == 5 { 40.0 } else { 27.0 } * char_w;
                ui.add_sized([w, 16.0], egui::Label::new(egui::RichText::new(text).small().color(p.text)).truncate());
                ui.separator();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("НТЦ Механотроника").small().weak());
                ui.separator();
                ui.label(egui::RichText::new(&self.status).small());
            });
        });
    }
}

/// The original's command line.
#[derive(Debug, Default, PartialEq)]
struct StartupArgs {
    dst: Option<std::path::PathBuf>,
    find: bool,
    files: Vec<std::path::PathBuf>,
}

fn startup_args(args: impl Iterator<Item = String>) -> StartupArgs {
    let mut out = StartupArgs::default();
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        if a.eq_ignore_ascii_case("-dst") {
            out.dst = args.next().map(Into::into);
        } else if a.eq_ignore_ascii_case("-find") {
            out.find = true;
        } else {
            out.files.push(a.into());
        }
    }
    out
}

/// Tab colours of the documents, in order (MFC OneNote auto colours; the
/// first one is the yellow of the help screenshots, the order of the rest is
/// not determined).
fn tab_color(i: usize) -> egui::Color32 {
    const C: [(u8, u8, u8); 8] = [
        (255, 219, 117),
        (148, 175, 230),
        (189, 205, 159),
        (240, 158, 159),
        (186, 166, 225),
        (154, 191, 180),
        (247, 182, 131),
        (216, 171, 192),
    ];
    let (r, g, b) = C[i % C.len()];
    egui::Color32::from_rgb(r, g, b)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Window title «Механотроника FastView - <document>» (0x45f7d0) and
        // files dropped on the window (DragAcceptFiles).
        let title = match self.active_doc() {
            Some(d) => format!("Механотроника FastView - {}", d.name),
            None => "Механотроника FastView".to_string(),
        };
        if self.window_title != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for f in dropped {
            self.load_comtrade_path(&f);
        }
        theme::apply(ctx, self.skin);
        self.handle_keys(ctx);

        let pal = self.skin.palette();
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::none().fill(pal.tab_active))
            .show(ctx, |ui| {
                self.ribbon_ui(ui);
                // Caption bars docked under the ribbon: the ОМП result.
                self.omp_banner(ui);
                if !self.docs.is_empty() {
                    self.doc_tabs(ui);
                }
            });
        // 142: the parameters bar, a 75 px strip under the ribbon.
        if self.dlg.param_bar {
            egui::TopBottomPanel::top("parambar")
                .exact_height(75.0)
                .frame(egui::Frame::none().fill(pal.group_bg))
                .show(ctx, |ui| self.param_bar_ui(ui));
        }

        // The merge bar (CMergeBar, 70 px), shown after a paste.
        if self.merge.is_some() {
            egui::TopBottomPanel::top("mergebar")
                .exact_height(70.0)
                .frame(egui::Frame::none().fill(pal.group_bg).inner_margin(egui::Margin::symmetric(15.0, 10.0)))
                .show(ctx, |ui| self.merge_bar_ui(ui));
        }

        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none().fill(pal.group_bg).inner_margin(egui::Margin::symmetric(6.0, 3.0)))
            .show(ctx, |ui| self.status_bar(ui));


        self.diagram_panes(ctx);
        self.render_tab(ctx);
        self.dialogs_ui(ctx);
        // The record settings follow the active document; leaving asks
        // about modified documents.
        self.track_settings();
        self.handle_exit_request(ctx);

        // Image export (Печать): request a screenshot while pending, save it when
        // the frame is delivered.
        if self.want_screenshot.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let (Some(image), Some(path)) = (shot, self.want_screenshot.clone()) {
            self.want_screenshot = None;
            let [w, h] = image.size;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for px in &image.pixels {
                rgba.extend_from_slice(&px.to_array());
            }
            let png = util::png_rgba(w as u32, h as u32, &rgba);
            self.status = match std::fs::write(&path, png) {
                Ok(()) => format!("Изображение сохранено: {}", path.display()),
                Err(e) => format!("Ошибка сохранения изображения: {e}"),
            };
        }
    }
}

impl App {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_switches() {
        let a = startup_args(["-Dst", "/d", "-FIND", "x.cfg"].into_iter().map(String::from));
        assert_eq!(a, StartupArgs { dst: Some("/d".into()), find: true, files: vec!["x.cfg".into()] });
    }
}
