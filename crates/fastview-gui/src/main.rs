//! FastView — a cross-platform reconstruction of the Mekhanotronika FastView
//! relay-protection characteristic viewer (the "круговая диаграмма" / impedance
//! plane feature). Builds and runs on Linux, Windows and macOS.
//!
//! Select a device block, toggle characteristics, edit the protection settings
//! and see the operate (срабатывание) and reset (возврат) boundaries plotted in
//! the impedance (R–jX) or differential (Id–Ibrake) plane.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui;
use egui_plot::{Line, LineStyle, Plot, PlotPoints, Polygon};
use fvb_core::geom::{CharGeometry, Shape};
use fvb_core::model::Dimension;
use fvb_core::{evaluate, Library};
use std::collections::HashMap;

/// The characteristic library shipped by default (the real FastView.fvb,
/// decoded to UTF-8). Replaceable at runtime via File → Open.
const DEFAULT_LIBRARY: &[u8] = include_bytes!("../../../assets/FastView.fvb");

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("FastView — характеристики защит"),
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

/// Load a font with Cyrillic coverage so the Russian characteristic names and
/// Cyrillic parameter identifiers render. Falls back silently to egui's default
/// (which lacks Cyrillic) if no system font is found.
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
                fonts.families.entry(family).or_default().insert(0, "cyr".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}

/// Per-characteristic UI state.
struct CharState {
    visible: bool,
    /// Current settings (parameter name -> value), seeded from defaults.
    settings: HashMap<String, f64>,
    /// Cached evaluated geometry; `None` until (re)computed.
    geo: Option<CharGeometry>,
    color: egui::Color32,
}

struct App {
    library: Library,
    source_name: String,
    block_idx: usize,
    show_reset: bool,
    fill_regions: bool,
    /// State keyed by (block_idx, char_idx).
    chars: HashMap<(usize, usize), CharState>,
    /// The characteristic currently open in the settings editor.
    selected: Option<usize>,
    status: String,
}

impl App {
    fn new() -> Self {
        let library = fvb_core::load_fvb_bytes(DEFAULT_LIBRARY)
            .unwrap_or_else(|e| panic!("bundled library failed to parse: {e}"));
        let mut app = App {
            library,
            source_name: "FastView.fvb (встроенная)".to_string(),
            block_idx: 0,
            show_reset: true,
            fill_regions: true,
            chars: HashMap::new(),
            selected: None,
            status: String::new(),
        };
        app.ensure_block_state();
        app
    }

    /// A stable, readable palette cycled across characteristics.
    fn palette(i: usize) -> egui::Color32 {
        const C: &[(u8, u8, u8)] = &[
            (31, 119, 180),
            (214, 39, 40),
            (44, 160, 44),
            (148, 103, 189),
            (255, 127, 14),
            (23, 190, 207),
            (227, 119, 194),
            (188, 189, 34),
            (140, 86, 75),
            (127, 127, 127),
        ];
        let (r, g, b) = C[i % C.len()];
        egui::Color32::from_rgb(r, g, b)
    }

    /// Initialise per-characteristic state for the current block (first one
    /// visible, settings seeded from defaults).
    fn ensure_block_state(&mut self) {
        let b = self.block_idx;
        let Some(block) = self.library.blocks.get(b) else {
            return;
        };
        for (ci, ch) in block.characteristics.iter().enumerate() {
            self.chars.entry((b, ci)).or_insert_with(|| CharState {
                visible: true,
                settings: ch.default_settings(),
                geo: None,
                color: Self::palette(ci),
            });
        }
        if self.selected.is_none() && !block.characteristics.is_empty() {
            self.selected = Some(0);
        }
        self.recompute_block();
    }

    /// Recompute geometry for every characteristic of the current block.
    fn recompute_block(&mut self) {
        let b = self.block_idx;
        let Some(block) = self.library.blocks.get(b) else {
            return;
        };
        for (ci, ch) in block.characteristics.iter().enumerate() {
            if let Some(st) = self.chars.get_mut(&(b, ci)) {
                st.geo = match ch.context(&st.settings).and_then(|cx| evaluate(ch, &cx)) {
                    Ok(g) => Some(g),
                    Err(_) => None,
                };
            }
        }
    }

    fn recompute_one(&mut self, ci: usize) {
        let b = self.block_idx;
        if let (Some(block), Some(st)) =
            (self.library.blocks.get(b), self.chars.get_mut(&(b, ci)))
        {
            if let Some(ch) = block.characteristics.get(ci) {
                st.geo = ch
                    .context(&st.settings)
                    .and_then(|cx| evaluate(ch, &cx))
                    .ok();
            }
        }
    }

    fn current_dimension(&self) -> Dimension {
        self.library
            .blocks
            .get(self.block_idx)
            .and_then(|b| b.characteristics.first())
            .map(|c| c.dimension)
            .unwrap_or(Dimension::Ohm)
    }

    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("FastView library", &["fvb"])
            .pick_file()
        {
            match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| {
                fvb_core::load_fvb_bytes(&b).map_err(|e| e.to_string())
            }) {
                Ok(lib) => {
                    self.library = lib;
                    self.source_name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.block_idx = 0;
                    self.chars.clear();
                    self.selected = None;
                    self.ensure_block_state();
                    self.status = format!("Загружено: {}", self.source_name);
                }
                Err(e) => self.status = format!("Ошибка загрузки: {e}"),
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("📂 Открыть .fvb…").clicked() {
                    self.open_file();
                }
                ui.separator();
                ui.label("Блок:");
                let prev = self.block_idx;
                let block_names: Vec<String> =
                    self.library.blocks.iter().map(|b| b.name.clone()).collect();
                egui::ComboBox::from_id_salt("block")
                    .width(360.0)
                    .selected_text(
                        block_names
                            .get(self.block_idx)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .show_ui(ui, |ui| {
                        for (i, name) in block_names.iter().enumerate() {
                            ui.selectable_value(&mut self.block_idx, i, name);
                        }
                    });
                if self.block_idx != prev {
                    self.selected = None;
                    self.ensure_block_state();
                }
                ui.separator();
                ui.checkbox(&mut self.show_reset, "Характеристики возврата");
                ui.checkbox(&mut self.fill_regions, "Заливка");
                ui.separator();
                ui.label(egui::RichText::new(&self.source_name).weak());
            });
        });

        egui::SidePanel::left("side")
            .default_width(320.0)
            .show(ctx, |ui| {
                self.left_panel(ui);
            });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(
                            "Ctrl+колесо — масштаб · перетаскивание — перемещение",
                        )
                        .weak()
                        .small(),
                    );
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.plot(ui);
        });
    }
}

impl App {
    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("Характеристики");
        ui.separator();
        let b = self.block_idx;
        let names: Vec<(usize, String)> = self
            .library
            .blocks
            .get(b)
            .map(|blk| {
                blk.characteristics
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.name.clone()))
                    .collect()
            })
            .unwrap_or_default();

        egui::ScrollArea::vertical()
            .max_height(ui.available_height() * 0.45)
            .show(ui, |ui| {
                for (ci, name) in &names {
                    ui.horizontal(|ui| {
                        if let Some(st) = self.chars.get_mut(&(b, *ci)) {
                            let dot = egui::RichText::new("⬤").color(st.color);
                            ui.label(dot);
                            ui.checkbox(&mut st.visible, "");
                        }
                        let selected = self.selected == Some(*ci);
                        if ui.selectable_label(selected, name).clicked() {
                            self.selected = Some(*ci);
                        }
                    });
                }
            });

        ui.separator();
        ui.heading("Уставки");
        ui.separator();
        self.settings_editor(ui);
    }

    fn settings_editor(&mut self, ui: &mut egui::Ui) {
        let b = self.block_idx;
        let Some(ci) = self.selected else {
            ui.label("Выберите характеристику для редактирования уставок.");
            return;
        };
        // Collect parameter names (clone to avoid borrow conflicts).
        let params: Vec<(String, f64)> = {
            let Some(ch) = self
                .library
                .blocks
                .get(b)
                .and_then(|blk| blk.characteristics.get(ci))
            else {
                return;
            };
            ch.parameters
                .iter()
                .map(|p| (p.name.clone(), p.default.eval(&Default::default()).unwrap_or(0.0)))
                .collect()
        };
        if params.is_empty() {
            ui.label("Нет настраиваемых уставок.");
            return;
        }
        let mut changed = false;
        egui::ScrollArea::vertical()
            .id_salt("params")
            .show(ui, |ui| {
                egui::Grid::new("paramgrid")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (name, default) in &params {
                            ui.label(name);
                            if let Some(st) = self.chars.get_mut(&(b, ci)) {
                                let v = st.settings.entry(name.clone()).or_insert(*default);
                                if ui
                                    .add(egui::DragValue::new(v).speed(0.05))
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                if ui.button("Сбросить уставки").clicked() {
                    if let (Some(blk), Some(st)) =
                        (self.library.blocks.get(b), self.chars.get_mut(&(b, ci)))
                    {
                        if let Some(ch) = blk.characteristics.get(ci) {
                            st.settings = ch.default_settings();
                            changed = true;
                        }
                    }
                }
            });
        if changed {
            self.recompute_one(ci);
        }
    }

    fn plot(&mut self, ui: &mut egui::Ui) {
        let dim = self.current_dimension();
        let b = self.block_idx;
        // Gather drawables first (immutable borrow), then draw.
        struct Draw {
            color: egui::Color32,
            geo_operate: Vec<Shape>,
            geo_reset: Vec<Shape>,
        }
        let mut draws: Vec<Draw> = Vec::new();
        if let Some(block) = self.library.blocks.get(b) {
            for ci in 0..block.characteristics.len() {
                if let Some(st) = self.chars.get(&(b, ci)) {
                    if !st.visible {
                        continue;
                    }
                    if let Some(geo) = &st.geo {
                        draws.push(Draw {
                            color: st.color,
                            geo_operate: geo.operate.shapes.clone(),
                            geo_reset: if self.show_reset {
                                geo.reset.shapes.clone()
                            } else {
                                Vec::new()
                            },
                        });
                    }
                }
            }
        }

        let fill = self.fill_regions;
        Plot::new("plane")
            .data_aspect(1.0)
            .x_axis_label(dim.x_label())
            .y_axis_label(dim.y_label())
            .legend(egui_plot::Legend::default())
            .show(ui, |plot_ui| {
                for d in &draws {
                    draw_shapes(plot_ui, &d.geo_operate, d.color, fill, false);
                    draw_shapes(plot_ui, &d.geo_reset, d.color, false, true);
                }
            });
    }
}

/// Draw a set of evaluated shapes. Operate boundaries are solid (optionally
/// filled); reset boundaries are drawn dashed and lighter.
fn draw_shapes(
    plot_ui: &mut egui_plot::PlotUi,
    shapes: &[Shape],
    color: egui::Color32,
    fill: bool,
    reset: bool,
) {
    let stroke_color = if reset {
        color.gamma_multiply(0.6)
    } else {
        color
    };
    let style = if reset {
        LineStyle::dashed_dense()
    } else {
        LineStyle::Solid
    };
    for shape in shapes {
        match shape {
            Shape::Region(rings) => {
                for (idx, ring) in rings.iter().enumerate() {
                    let pts: Vec<[f64; 2]> = ring.clone();
                    // Outer ring (idx 0) may be filled; holes are outlined only.
                    if idx == 0 && fill && !reset {
                        let poly = Polygon::new(PlotPoints::from(pts.clone()))
                            .fill_color(color.gamma_multiply(0.12))
                            .stroke(egui::Stroke::new(1.6_f32, stroke_color))
                            .style(style);
                        plot_ui.polygon(poly);
                    } else {
                        // Outline only (closed loop).
                        let mut loop_pts = pts.clone();
                        if let Some(first) = loop_pts.first().copied() {
                            loop_pts.push(first);
                        }
                        plot_ui.line(
                            Line::new(PlotPoints::from(loop_pts))
                                .color(stroke_color)
                                .width(1.6_f32)
                                .style(style),
                        );
                    }
                }
            }
            Shape::Polyline(pl) => {
                plot_ui.line(
                    Line::new(PlotPoints::from(pl.clone()))
                        .color(stroke_color)
                        .width(1.8_f32)
                        .style(style),
                );
            }
        }
    }
}
