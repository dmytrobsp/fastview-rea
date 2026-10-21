//! FastView — a cross-platform reconstruction of НТЦ «Механотроника» Fast View 5,
//! the relay-protection oscillogram/characteristic analysis tool. Office-2007
//! ribbon shell, MDI documents and a lane-based oscillogram view. Builds and runs
//! on Linux, Windows and macOS.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod chandlg;
mod chansel;
mod dialogs;
mod icons;
mod lanes;
mod oscgeom;
mod persist;
mod quick_ui;
mod recdlg;
mod ribbon;
mod theme;
mod util;
mod views;

use app::App;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([960.0, 620.0])
            .with_title("Механотроника FastView"),
        ..Default::default()
    };
    eframe::run_native(
        "FastView",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new().with_persistence()))
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
    fn doc_tabs(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let names: Vec<String> = self.docs.iter().map(|d| format!("{}.cfg", d.name)).collect();
                for (i, name) in names.iter().enumerate() {
                    let active = self.active == Some(i);
                    let p = self.skin.palette();
                    egui::Frame::none()
                        .fill(if active { p.tab_active } else { p.group_bg })
                        .stroke(egui::Stroke::new(1.0f32, p.group_border))
                        .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
                        .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                        .show(ui, |ui| {
                            if ui.selectable_label(active, egui::RichText::new(name).color(p.text)).clicked() {
                                activate = Some(i);
                            }
                            if ui.add(egui::Button::new("✕").frame(false).small()).clicked() {
                                close = Some(i);
                            }
                        });
                }
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.skin);
        self.handle_keys(ctx);

        let pal = self.skin.palette();
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::none().fill(pal.tab_active))
            .show(ctx, |ui| {
                self.ribbon_ui(ui);
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

        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none().fill(pal.group_bg).inner_margin(egui::Margin::symmetric(6.0, 3.0)))
            .show(ctx, |ui| self.status_bar(ui));

        if self.show_explorer {
            egui::SidePanel::left("explorer").default_width(260.0).show(ctx, |ui| self.explorer_panel(ui));
        }

        self.diagram_panes(ctx);
        self.render_tab(ctx);
        self.dialogs_ui(ctx);

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
    fn explorer_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Проводник").strong());
            if ui.small_button("✕").clicked() {
                self.show_explorer = false;
            }
        });
        ui.label(egui::RichText::new(self.explorer_dir.display().to_string()).small().weak());
        ui.separator();
        let mut goto: Option<std::path::PathBuf> = None;
        let mut load: Option<std::path::PathBuf> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(parent) = self.explorer_dir.parent() {
                if ui.selectable_label(false, "📁 ..").clicked() {
                    goto = Some(parent.to_path_buf());
                }
            }
            let mut entries: Vec<std::path::PathBuf> =
                std::fs::read_dir(&self.explorer_dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
            entries.sort_by_key(|p| (!p.is_dir(), p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()));
            for p in entries {
                let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                if p.is_dir() {
                    if ui.selectable_label(false, format!("📁 {name}")).clicked() {
                        goto = Some(p);
                    }
                } else {
                    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                    let is_cfg = ext == "cfg";
                    let resp = ui.selectable_label(false, format!("{} {name}", if is_cfg { "〜" } else { "  " }));
                    if is_cfg && (resp.double_clicked() || resp.clicked()) {
                        load = Some(p);
                    }
                }
            }
        });
        if let Some(dir) = goto {
            self.explorer_dir = dir;
        }
        if let Some(path) = load {
            self.load_comtrade_path(&path);
        }
    }
}
