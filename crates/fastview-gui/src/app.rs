//! Shared application state and data loading for the FastView reconstruction.

use eframe::egui;
use fvb_core::analysis::Analysis;
use fvb_core::comtrade::Oscillogram;
use fvb_core::geom::CharGeometry;
use fvb_core::model::Dimension;
use fvb_core::omp::LineParams;
use fvb_core::Library;
use std::collections::HashMap;

/// The characteristic library shipped by default (the real FastView.fvb, UTF-8).
pub const DEFAULT_LIBRARY: &[u8] = include_bytes!("../../../assets/FastView.fvb");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Круговая диаграмма: protection characteristics + impedance overlay.
    Circle,
    /// Осциллограмма: analog/digital waveforms with cursors.
    Oscillogram,
    /// Векторная диаграмма: phasors at the main cursor.
    Vector,
    /// Быстрый анализ: symmetric components, impedances, formula editor.
    Analysis,
    /// ОМП: fault location.
    Omp,
}

/// Per-characteristic UI state.
pub struct CharState {
    pub visible: bool,
    pub settings: HashMap<String, f64>,
    pub geo: Option<CharGeometry>,
    pub color: egui::Color32,
}

/// A loaded oscillogram plus its derived analysis and view state.
pub struct Loaded {
    pub osc: Oscillogram,
    pub analysis: Analysis,
    pub name: String,
    pub main_cursor: usize,
    pub aux_cursor: usize,
    pub chan_visible: Vec<bool>,
}

/// A user-defined computed channel (formula editor), evaluated per sample.
pub struct Computed {
    pub name: String,
    pub formula: String,
    pub samples: Vec<f64>,
}

pub struct App {
    pub library: Library,
    pub source_name: String,
    pub block_idx: usize,
    pub show_reset: bool,
    pub fill_regions: bool,
    pub chars: HashMap<(usize, usize), CharState>,
    pub selected: Option<usize>,
    pub tab: Tab,
    pub loaded: Option<Loaded>,
    pub computed: Vec<Computed>,
    pub formula_name: String,
    pub formula_text: String,
    pub line: LineParams,
    pub status: String,
}

impl App {
    pub fn new() -> Self {
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
            tab: Tab::Circle,
            loaded: None,
            computed: Vec::new(),
            formula_name: "Uab".to_string(),
            formula_text: "Ua-Ub".to_string(),
            line: LineParams {
                r1_per_km: 0.1,
                x1_per_km: 0.4,
                length_km: 50.0,
            },
            status: String::new(),
        };
        app.ensure_block_state();
        // Test/demo hooks (used for headless screenshots): FV_DEMO loads the
        // bundled fault record; FV_TAB selects the initial tab; FV_BLOCK picks a
        // characteristic block.
        if std::env::var("FV_DEMO").is_ok() {
            if let Ok(osc) = fvb_core::comtrade::parse(
                include_str!("../../../assets/demo/demo_fault.cfg"),
                include_bytes!("../../../assets/demo/demo_fault.dat"),
            ) {
                app.set_oscillogram(osc, "demo_fault".into());
            }
        }
        if let Ok(bi) = std::env::var("FV_BLOCK") {
            if let Ok(i) = bi.parse::<usize>() {
                if i < app.library.blocks.len() {
                    app.block_idx = i;
                    app.selected = None;
                    app.ensure_block_state();
                }
            }
        }
        app.tab = match std::env::var("FV_TAB").as_deref() {
            Ok("osc") => Tab::Oscillogram,
            Ok("vector") => Tab::Vector,
            Ok("analysis") => Tab::Analysis,
            Ok("omp") => Tab::Omp,
            _ => Tab::Circle,
        };
        app
    }

    pub fn palette(i: usize) -> egui::Color32 {
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

    pub fn ensure_block_state(&mut self) {
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

    pub fn recompute_block(&mut self) {
        let b = self.block_idx;
        let Some(block) = self.library.blocks.get(b) else {
            return;
        };
        for (ci, ch) in block.characteristics.iter().enumerate() {
            if let Some(st) = self.chars.get_mut(&(b, ci)) {
                st.geo = ch
                    .context(&st.settings)
                    .and_then(|cx| fvb_core::evaluate(ch, &cx))
                    .ok();
            }
        }
    }

    pub fn recompute_one(&mut self, ci: usize) {
        let b = self.block_idx;
        if let (Some(block), Some(st)) = (self.library.blocks.get(b), self.chars.get_mut(&(b, ci))) {
            if let Some(ch) = block.characteristics.get(ci) {
                st.geo = ch
                    .context(&st.settings)
                    .and_then(|cx| fvb_core::evaluate(ch, &cx))
                    .ok();
            }
        }
    }

    pub fn current_dimension(&self) -> Dimension {
        self.library
            .blocks
            .get(self.block_idx)
            .and_then(|b| b.characteristics.first())
            .map(|c| c.dimension)
            .unwrap_or(Dimension::Ohm)
    }

    pub fn open_fvb(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("FastView library", &["fvb"])
            .pick_file()
        {
            match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| fvb_core::load_fvb_bytes(&b).map_err(|e| e.to_string()))
            {
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
                    self.status = format!("Загружена библиотека: {}", self.source_name);
                }
                Err(e) => self.status = format!("Ошибка загрузки .fvb: {e}"),
            }
        }
    }

    pub fn open_comtrade(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("COMTRADE", &["cfg", "dat", "CFG", "DAT"])
            .pick_file()
        {
            self.load_comtrade_path(&path);
        }
    }

    pub fn load_comtrade_path(&mut self, path: &std::path::Path) {
        match fvb_core::comtrade::load(path) {
            Ok(osc) => self.set_oscillogram(
                osc,
                path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            Err(e) => self.status = format!("Ошибка загрузки осциллограммы: {e}"),
        }
    }

    pub fn set_oscillogram(&mut self, osc: Oscillogram, name: String) {
        let analysis = Analysis::compute(&osc);
        let n = osc.sample_count();
        let nch = osc.analog.len();
        self.loaded = Some(Loaded {
            osc,
            analysis,
            name: name.clone(),
            main_cursor: n / 2,
            aux_cursor: n / 4,
            chan_visible: vec![true; nch],
        });
        self.computed.clear();
        self.status = format!("Загружена осциллограмма: {name} ({n} отсчётов)");
    }

    /// Export the loaded oscillogram (time + analog + computed channels) to CSV.
    pub fn export_csv(&mut self) {
        let Some(l) = &self.loaded else {
            self.status = "Нет осциллограммы для экспорта.".into();
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(format!("{}.csv", l.name))
            .save_file()
        else {
            return;
        };
        let mut out = String::new();
        out.push_str("t");
        for ch in &l.osc.analog {
            out.push(',');
            out.push_str(&ch.id);
        }
        for c in &self.computed {
            out.push(',');
            out.push_str(&c.name);
        }
        out.push('\n');
        for i in 0..l.osc.sample_count() {
            out.push_str(&format!("{}", l.osc.time.get(i).copied().unwrap_or(0.0)));
            for ch in &l.osc.analog {
                out.push_str(&format!(",{}", ch.samples.get(i).copied().unwrap_or(0.0)));
            }
            for c in &self.computed {
                out.push_str(&format!(",{}", c.samples.get(i).copied().unwrap_or(0.0)));
            }
            out.push('\n');
        }
        match std::fs::write(&path, out) {
            Ok(()) => self.status = format!("Экспортировано в {}", path.display()),
            Err(e) => self.status = format!("Ошибка экспорта: {e}"),
        }
    }

    /// Evaluate a per-sample formula over the oscillogram channels and append it
    /// as a computed channel. Identifiers are analog channel ids/phases.
    pub fn add_computed(&mut self) {
        let Some(loaded) = &self.loaded else {
            self.status = "Сначала загрузите осциллограмму.".into();
            return;
        };
        let expr = match fvb_core::Expr::parse(&self.formula_text) {
            Ok(e) => e,
            Err(e) => {
                self.status = format!("Ошибка формулы: {e}");
                return;
            }
        };
        let n = loaded.osc.sample_count();
        let mut samples = Vec::with_capacity(n);
        // Build per-sample context from channel values (lowercased keys).
        for i in 0..n {
            let mut ctx = fvb_core::Context::new();
            for ch in &loaded.osc.analog {
                if let Some(v) = ch.samples.get(i) {
                    ctx.insert(ch.id.to_lowercase(), *v);
                    if !ch.phase.is_empty() {
                        ctx.entry(ch.phase.to_lowercase()).or_insert(*v);
                    }
                }
            }
            for c in &self.computed {
                if let Some(v) = c.samples.get(i) {
                    ctx.insert(c.name.to_lowercase(), *v);
                }
            }
            samples.push(expr.eval(&ctx).unwrap_or(f64::NAN));
        }
        let name = self.formula_name.clone();
        self.computed.push(Computed {
            name: name.clone(),
            formula: self.formula_text.clone(),
            samples,
        });
        self.status = format!("Добавлен расчётный канал «{name}»");
    }
}
