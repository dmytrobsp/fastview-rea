//! FastView — a cross-platform reconstruction of НТЦ «Механотроника» Fast View 5,
//! the relay-protection oscillogram/characteristic analysis tool. Office-2007
//! ribbon shell, MDI documents and a lane-based oscillogram view. Builds and runs
//! on Linux, Windows and macOS.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod dialogs;
mod icons;
mod lanes;
mod ribbon;
mod theme;
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
            Ok(Box::new(App::new()))
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
            let sep = |ui: &mut egui::Ui, label: &str, val: &str| {
                ui.label(egui::RichText::new(label).small().weak());
                ui.label(egui::RichText::new(val).small().color(p.text));
                ui.separator();
            };
            sep(ui, "Верт. визир 1:", &s[0]);
            sep(ui, "Верт. визир 2:", &s[1]);
            sep(ui, "Разница:", &s[2]);
            sep(ui, "Гор. визир 1:", &s[3]);
            sep(ui, "Гор. визир 2:", &s[4]);
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

        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none().fill(pal.group_bg).inner_margin(egui::Margin::symmetric(6.0, 3.0)))
            .show(ctx, |ui| self.status_bar(ui));

        self.render_tab(ctx);
        self.dialogs_ui(ctx);
    }
}
