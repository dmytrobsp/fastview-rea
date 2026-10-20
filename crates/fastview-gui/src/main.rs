//! FastView — a cross-platform reconstruction of the Mekhanotronika FastView
//! relay-protection oscillogram-analysis tool. Builds and runs on Linux, Windows
//! and macOS.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod views;

use app::{App, Tab};
use eframe::egui;

/// A small demo COMTRADE fault record bundled so the oscillogram/analysis/ОМП
/// features are usable without external files.
const DEMO_CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
const DEMO_DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("FastView — анализ осциллограмм и характеристик защит"),
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Файл", |ui| {
                    if ui.button("Открыть библиотеку .fvb…").clicked() {
                        self.open_fvb();
                        ui.close_menu();
                    }
                    if ui.button("Открыть COMTRADE (.cfg/.dat)…").clicked() {
                        self.open_comtrade();
                        ui.close_menu();
                    }
                    if ui.button("Демо-осциллограмма (авария A-0)").clicked() {
                        match fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT) {
                            Ok(osc) => self.set_oscillogram(osc, "demo_fault".into()),
                            Err(e) => self.status = format!("Демо: {e}"),
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Экспорт каналов в CSV…").clicked() {
                        self.export_csv();
                        ui.close_menu();
                    }
                });
                ui.separator();
                ui.selectable_value(&mut self.tab, Tab::Circle, "Круговая диаграмма");
                ui.selectable_value(&mut self.tab, Tab::Oscillogram, "Осциллограмма");
                ui.selectable_value(&mut self.tab, Tab::Vector, "Векторная диаграмма");
                ui.selectable_value(&mut self.tab, Tab::Analysis, "Быстрый анализ");
                ui.selectable_value(&mut self.tab, Tab::Omp, "ОМП");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.source_name).weak());
                });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("Ctrl+колесо — масштаб · перетаскивание — перемещение")
                            .weak()
                            .small(),
                    );
                });
            });
        });

        self.render_tab(ctx);
    }
}
