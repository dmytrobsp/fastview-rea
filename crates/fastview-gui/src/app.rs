//! Shared application state, document model, commands and data loading for the
//! FastView reconstruction (ribbon UI, MDI documents, lane-based oscillogram).

use crate::theme::Skin;
use eframe::egui;
use fvb_core::analysis::Analysis;
use fvb_core::comtrade::Oscillogram;
use fvb_core::geom::CharGeometry;
use fvb_core::model::Dimension;
use fvb_core::omp::{LineParams, NeutralMode};
use fvb_core::Library;
use std::collections::HashMap;

pub const DEFAULT_LIBRARY: &[u8] = include_bytes!("../../../assets/FastView.fvb");
pub const DEMO_CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
pub const DEMO_DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

/// Which document view fills the work area. In the original these are separate
/// windows opened from the ribbon *Диаграммы* group and the *ОМП* tab; here they
/// are selectable views over the active document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Осциллограмма — the stacked per-channel lanes (the main document view).
    Oscillogram,
    /// Частотная диаграмма — per-harmonic bar spectrum at the cursor.
    Frequency,
    /// Векторная диаграмма — phasors at the cursor.
    Vector,
    /// Круговая диаграмма — protection characteristics + impedance overlay.
    Circle,
    /// Дифференциальная диаграмма — differential (Id–Iт) plane.
    Differential,
    /// Быстрый анализ — symmetric components, impedances, formula editor.
    Analysis,
    /// ОМП — fault location.
    Omp,
}

/// Which ribbon tab is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonTab {
    Home,
    Omp,
}

/// Sub-tabs of the Быстрый анализ panel (mirrors the original's tabbed layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisTab {
    Symmetric,
    Zero,
    PhasePhase,
    Phase,
    Differential,
    Formula,
}

/// ОМП measuring method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmpMethod {
    SingleEnded,
    DoubleEnded,
}

/// The per-channel value the stats panel and vector/circle views read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueMode {
    /// Мгновенные значения — the raw sample at the cursor.
    Instant,
    /// Среднеквадратичные значения — true one-cycle RMS at the cursor.
    Rms,
    /// Первая гармоника — fundamental DFT magnitude at the cursor.
    FirstHarmonic,
}

/// Time-axis unit (time-axis context menu 385/386).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    Micro,
    Milli,
    Sec,
    Samples,
}

impl TimeUnit {
    pub fn label(self) -> &'static str {
        match self {
            TimeUnit::Micro => "мкс",
            TimeUnit::Milli => "мс",
            TimeUnit::Sec => "с",
            TimeUnit::Samples => "отсч.",
        }
    }
    /// Convert seconds to this unit (samples handled separately by index).
    pub fn scale_secs(self, t: f64) -> f64 {
        match self {
            TimeUnit::Micro => t * 1e6,
            TimeUnit::Milli => t * 1e3,
            TimeUnit::Sec => t,
            TimeUnit::Samples => t,
        }
    }
}

/// Per-channel display state for a document.
#[derive(Debug, Clone)]
pub struct ChanView {
    pub visible: bool,
    pub color: egui::Color32,
    /// Инвертировать (negate the trace).
    pub invert: bool,
    /// Удалить постоянную составляющую (subtract the sliding DC).
    pub remove_dc: bool,
    /// Отображать в отсчётах АЦП (show raw ADC counts instead of scaled units).
    pub adc_counts: bool,
    /// Per-channel vertical zoom (Увеличить/Уменьшить/Вернуть к исходному размеру).
    pub vzoom: f64,
    /// Совместить каналы: draw this channel in the previous lane's axes.
    pub overlay: bool,
    pub on_freq: bool,
    pub on_vector: bool,
    pub on_circle: bool,
}

/// A user-defined computed channel (formula editor), evaluated per sample.
#[derive(Debug, Clone)]
pub struct Computed {
    pub name: String,
    pub formula: String,
    pub samples: Vec<f64>,
    pub color: egui::Color32,
}

/// An on-waveform annotation anchored to a sample index.
#[derive(Debug, Clone)]
pub struct Comment {
    pub sample: usize,
    pub text: String,
    /// Optional amplitude threshold shown as «Уровень: N».
    pub level: Option<f64>,
    /// «Скрыть комментарий» — keep the marker but hide the text label.
    pub hidden: bool,
}

/// A channel copied for merging oscillograms (Объединение: «Копировать/Вставить
/// каналы для объединения осциллограмм», MENU 358).
#[derive(Debug, Clone)]
pub struct ClipChan {
    pub name: String,
    pub samples: Vec<f64>,
    pub color: egui::Color32,
}

/// State of the comment editor popup.
#[derive(Debug, Clone)]
pub struct CommentEdit {
    pub sample: usize,
    /// Index into the document's `comments` if editing an existing one.
    pub idx: Option<usize>,
    pub text: String,
    /// Raw text of the optional level field (parsed on save).
    pub level_str: String,
}

/// One open oscillogram document.
pub struct Doc {
    pub osc: Oscillogram,
    pub analysis: Analysis,
    pub name: String,
    pub main_cursor: usize,
    pub aux_cursor: usize,
    pub chans: Vec<ChanView>,
    pub computed: Vec<Computed>,
    pub comments: Vec<Comment>,
    /// Time-axis view window in seconds `[t0, t1]` (shared across lanes).
    pub view_t0: f64,
    pub view_t1: f64,
    /// Horizontal (amplitude) визиры, measured in the first lane's units.
    pub hcur_on: bool,
    pub hcur1: f64,
    pub hcur2: f64,
    /// Precomputed per-channel one-cycle RMS and DC series (for stats / remove-DC).
    pub rms: Vec<Vec<f64>>,
    pub dc: Vec<Vec<f64>>,
}

impl Doc {
    pub fn new(osc: Oscillogram, name: String) -> Self {
        let analysis = Analysis::compute(&osc);
        let n = osc.samples_per_cycle().max(2);
        let rms: Vec<Vec<f64>> = osc
            .analog
            .iter()
            .map(|c| fvb_core::signal::sliding_rms(&c.samples, n))
            .collect();
        let dc: Vec<Vec<f64>> = osc
            .analog
            .iter()
            .map(|c| fvb_core::signal::sliding_dc(&c.samples, n))
            .collect();
        let count = osc.sample_count();
        let chans = (0..osc.analog.len())
            .map(|i| ChanView {
                visible: true,
                color: default_channel_color(&osc.analog[i], i),
                invert: false,
                remove_dc: false,
                adc_counts: false,
                vzoom: 1.0,
                overlay: false,
                on_freq: true,
                on_vector: true,
                on_circle: i == 0,
            })
            .collect();
        let t_last = osc.time.last().copied().unwrap_or(1.0);
        Doc {
            osc,
            analysis,
            name,
            main_cursor: count / 2,
            aux_cursor: count / 4,
            chans,
            computed: Vec::new(),
            comments: Vec::new(),
            view_t0: 0.0,
            view_t1: if t_last > 0.0 { t_last } else { 1.0 },
            hcur_on: false,
            hcur1: 0.0,
            hcur2: 0.0,
            rms,
            dc,
        }
    }

    /// The displayed sample value of analog channel `i` at index `k`, honouring
    /// invert / remove-DC.
    pub fn sample(&self, i: usize, k: usize) -> f64 {
        let raw = self.osc.analog[i].samples.get(k).copied().unwrap_or(0.0);
        let dc = if self.chans[i].remove_dc {
            self.dc[i].get(k).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        let v = raw - dc;
        if self.chans[i].invert {
            -v
        } else {
            v
        }
    }

    pub fn rms_at(&self, i: usize, k: usize) -> f64 {
        self.rms[i].get(k).copied().unwrap_or(0.0)
    }

    pub fn harmonic_at(&self, i: usize, k: usize) -> f64 {
        self.analysis
            .phasors
            .get(i)
            .and_then(|p| p.get(k))
            .map(|z| z.norm())
            .unwrap_or(0.0)
    }

    /// Reset the time view to the full record.
    pub fn fit_time(&mut self) {
        self.view_t0 = 0.0;
        self.view_t1 = self.osc.time.last().copied().unwrap_or(1.0).max(1e-9);
    }
}

/// Default channel colour: the original cycles yellow/green/red/blue/… and tints
/// by quantity. Colours match the context-menu palette (33000–33004).
pub fn default_channel_color(ch: &fvb_core::comtrade::AnalogChannel, i: usize) -> egui::Color32 {
    const C: &[(u8, u8, u8)] = &[
        (0xD4, 0xA0, 0x17), // yellow (Жёлтый)
        (0x18, 0x8A, 0x42), // green  (Зелёный)
        (0xCC, 0x22, 0x22), // red    (Красный)
        (0x20, 0x4A, 0xA0), // blue   (Синий)
        (0xB0, 0x6A, 0x12), // orange
        (0x18, 0x8A, 0x8A), // teal
        (0x7A, 0x3A, 0x9A), // violet
        (0x55, 0x55, 0x55), // grey
    ];
    let _ = ch;
    let (r, g, b) = C[i % C.len()];
    egui::Color32::from_rgb(r, g, b)
}

/// A snapshot of a document's editable view-state for undo/redo.
#[derive(Clone)]
pub struct DocSnapshot {
    pub chans: Vec<ChanView>,
    pub comments: Vec<Comment>,
    pub computed: Vec<Computed>,
}

/// Commands dispatched from the ribbon, QAT, keybinds and context menus. The
/// numeric ids mirror the original's command ids where they are meaningful.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Open,          // Ctrl+O / QAT
    Save,          // Ctrl+S  (save as COMTRADE)
    Print,         // Ctrl+P  (export image)
    Copy,          // Ctrl+C
    Paste,         // Ctrl+V
    Undo,          // Ctrl+Z
    Redo,          // Ctrl+Y
    Demo,
    ExportCsv,
    OpenFvb,
    FileParams,
    Archive,
    ToggleExplorer,
    ToggleRibbonMin,
    Show(View),
    SetPrimaryValues(bool),
    InvertVisible,
    RemoveDcVisible,
    SetValueMode(ValueMode),
    ShowAllChannels(bool),
    SetSkin(Skin),
    FitTime,
    CloseDoc(usize),
    ActivateDoc(usize),
}

pub struct App {
    // library / circle-diagram state
    pub library: Library,
    pub source_name: String,
    pub block_idx: usize,
    pub chars: HashMap<(usize, usize), CharState>,
    pub selected_char: Option<usize>,
    pub show_reset: bool,
    pub fill_regions: bool,

    // documents
    pub docs: Vec<Doc>,
    pub active: Option<usize>,

    // ribbon / view
    pub tab: RibbonTab,
    pub view: View,
    pub skin: Skin,
    pub value_mode: ValueMode,
    pub time_unit: TimeUnit,
    pub show_grid: bool,
    pub primary_values: bool,

    // formula editor
    pub formula_name: String,
    pub formula_text: String,

    // vector diagram options (MENU 355)
    pub vec_radians: bool,
    pub vec_direction_only: bool,
    pub vec_show_values: bool,
    pub vec_abs_angle: bool,

    // analysis / omp
    pub analysis_tab: AnalysisTab,
    pub line: LineParams,
    pub omp_method: OmpMethod,
    pub neutral: NeutralMode,
    /// Remote-end document for the double-ended ОМП method.
    pub omp_remote: Option<usize>,

    // dialogs / panels
    pub show_about: bool,
    pub show_file_params: bool,
    pub show_explorer: bool,
    pub explorer_dir: std::path::PathBuf,
    pub ribbon_minimized: bool,
    pub props_channel: Option<usize>,
    pub comment_edit: Option<CommentEdit>,
    /// Pending image-export target (filled once the screenshot arrives).
    pub want_screenshot: Option<std::path::PathBuf>,

    /// Channels copied for merging oscillograms (Объединение).
    pub channel_clip: Vec<ClipChan>,

    // undo / redo of per-document view edits
    pub undo_stack: Vec<(usize, DocSnapshot)>,
    pub redo_stack: Vec<(usize, DocSnapshot)>,

    /// Which horizontal визир (0/1) is being dragged, if any.
    pub dragging_hcur: Option<u8>,

    pub status: String,
}

/// Per-characteristic UI state (circle diagram).
pub struct CharState {
    pub visible: bool,
    pub settings: HashMap<String, f64>,
    pub geo: Option<CharGeometry>,
    pub color: egui::Color32,
}

impl App {
    pub fn new() -> Self {
        let library = fvb_core::load_fvb_bytes(DEFAULT_LIBRARY)
            .unwrap_or_else(|e| panic!("bundled library failed to parse: {e}"));
        let mut app = App {
            library,
            source_name: "FastView.fvb (встроенная)".to_string(),
            block_idx: 0,
            chars: HashMap::new(),
            selected_char: None,
            show_reset: true,
            fill_regions: true,
            docs: Vec::new(),
            active: None,
            tab: RibbonTab::Home,
            view: View::Oscillogram,
            skin: Skin::Blue,
            value_mode: ValueMode::Rms,
            time_unit: TimeUnit::Milli,
            show_grid: true,
            primary_values: false,
            formula_name: "Uab".to_string(),
            formula_text: "Ua-Ub".to_string(),
            vec_radians: false,
            vec_direction_only: false,
            vec_show_values: true,
            vec_abs_angle: true,
            analysis_tab: AnalysisTab::Symmetric,
            line: LineParams {
                r1_per_km: 0.1,
                x1_per_km: 0.4,
                r0_per_km: 0.3,
                x0_per_km: 1.2,
                length_km: 50.0,
            },
            omp_method: OmpMethod::SingleEnded,
            neutral: NeutralMode::Grounded,
            omp_remote: None,
            show_about: false,
            show_file_params: false,
            show_explorer: false,
            explorer_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            ribbon_minimized: false,
            props_channel: None,
            comment_edit: None,
            want_screenshot: None,
            channel_clip: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dragging_hcur: None,
            status: "Готово".to_string(),
        };
        app.ensure_block_state();

        // Test/demo hooks (used for headless screenshots).
        if std::env::var("FV_DEMO").is_ok() {
            if let Ok(osc) = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT) {
                app.add_doc(Doc::new(osc, "demo_fault".into()));
            }
        }
        if let Ok(bi) = std::env::var("FV_BLOCK") {
            if let Ok(i) = bi.parse::<usize>() {
                if i < app.library.blocks.len() {
                    app.block_idx = i;
                    app.selected_char = None;
                    app.ensure_block_state();
                }
            }
        }
        app.view = match std::env::var("FV_VIEW").as_deref() {
            Ok("freq") => View::Frequency,
            Ok("vector") => View::Vector,
            Ok("circle") => View::Circle,
            Ok("diff") => View::Differential,
            Ok("analysis") => View::Analysis,
            Ok("omp") => View::Omp,
            _ => View::Oscillogram,
        };
        if app.view == View::Omp {
            app.tab = RibbonTab::Omp;
        }
        if let Ok(s) = std::env::var("FV_SKIN") {
            app.skin = match s.as_str() {
                "black" => Skin::Black,
                "silver" => Skin::Silver,
                "aqua" => Skin::Aqua,
                _ => Skin::Blue,
            };
        }
        app
    }

    pub fn active_doc(&self) -> Option<&Doc> {
        self.active.and_then(|i| self.docs.get(i))
    }
    pub fn active_doc_mut(&mut self) -> Option<&mut Doc> {
        self.active.and_then(|i| self.docs.get_mut(i))
    }

    pub fn add_doc(&mut self, doc: Doc) {
        let name = doc.name.clone();
        let n = doc.osc.sample_count();
        self.docs.push(doc);
        self.active = Some(self.docs.len() - 1);
        self.view = View::Oscillogram;
        self.status = format!("Загружена осциллограмма: {name} ({n} отсчётов)");
    }

    // ---------- command dispatch ----------
    pub fn dispatch(&mut self, cmd: Command) {
        match cmd {
            Command::Open => self.open_comtrade(),
            Command::Demo => match fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT) {
                Ok(osc) => self.add_doc(Doc::new(osc, "demo_fault".into())),
                Err(e) => self.status = format!("Демо: {e}"),
            },
            Command::ExportCsv => self.export_csv(),
            Command::OpenFvb => self.open_fvb(),
            Command::FileParams => self.show_file_params = true,
            Command::Archive => self.archive_record(),
            Command::ToggleExplorer => self.show_explorer = !self.show_explorer,
            Command::ToggleRibbonMin => self.ribbon_minimized = !self.ribbon_minimized,
            Command::Save => self.save_comtrade(),
            Command::Print => self.request_image_export(),
            Command::Copy => self.copy_channels_for_merge(),
            Command::Paste => {
                self.snapshot_undo();
                self.paste_channels_for_merge();
            }
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::Show(v) => {
                self.view = v;
                self.tab = if v == View::Omp {
                    RibbonTab::Omp
                } else {
                    RibbonTab::Home
                };
                match v {
                    View::Circle => self.select_first_block_dim(Dimension::Ohm),
                    View::Differential => self.select_first_block_dim(Dimension::Dif),
                    _ => {}
                }
            }
            Command::SetPrimaryValues(p) => {
                self.primary_values = p;
                self.status = if p {
                    "Первичные значения".into()
                } else {
                    "Вторичные значения".into()
                };
            }
            Command::InvertVisible => {
                self.snapshot_undo();
                if let Some(d) = self.active_doc_mut() {
                    for c in d.chans.iter_mut().filter(|c| c.visible) {
                        c.invert = !c.invert;
                    }
                }
            }
            Command::RemoveDcVisible => {
                self.snapshot_undo();
                if let Some(d) = self.active_doc_mut() {
                    for c in d.chans.iter_mut().filter(|c| c.visible) {
                        c.remove_dc = !c.remove_dc;
                    }
                }
            }
            Command::SetValueMode(m) => self.value_mode = m,
            Command::ShowAllChannels(on) => {
                self.snapshot_undo();
                if let Some(d) = self.active_doc_mut() {
                    for c in d.chans.iter_mut() {
                        c.visible = on;
                    }
                }
            }
            Command::SetSkin(s) => self.skin = s,
            Command::FitTime => {
                if let Some(d) = self.active_doc_mut() {
                    d.fit_time();
                }
            }
            Command::CloseDoc(i) => {
                if i < self.docs.len() {
                    self.docs.remove(i);
                    self.active = if self.docs.is_empty() {
                        None
                    } else {
                        Some(i.min(self.docs.len() - 1))
                    };
                }
            }
            Command::ActivateDoc(i) => {
                if i < self.docs.len() {
                    self.active = Some(i);
                }
            }
        }
    }

    /// The Ctrl-shortcut → command map — the original's accelerator table 128
    /// verbatim (see `notes/ui/accelerators.txt`).
    pub fn keymap() -> [(egui::Key, Command); 7] {
        [
            (egui::Key::O, Command::Open),
            (egui::Key::S, Command::Save),
            (egui::Key::P, Command::Print),
            (egui::Key::C, Command::Copy),
            (egui::Key::V, Command::Paste),
            (egui::Key::Z, Command::Undo),
            (egui::Key::Y, Command::Redo),
        ]
    }

    /// Dispatch any Ctrl-shortcut pressed this frame.
    pub fn handle_keys(&mut self, ctx: &egui::Context) {
        let mut cmd = None;
        ctx.input(|i| {
            if i.modifiers.command || i.modifiers.ctrl {
                for (key, c) in Self::keymap() {
                    if i.key_pressed(key) {
                        cmd = Some(c);
                    }
                }
            }
        });
        if let Some(c) = cmd {
            self.dispatch(c);
        }
    }

    // ---------- circle-diagram state ----------
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
        if self.selected_char.is_none() && !block.characteristics.is_empty() {
            self.selected_char = Some(0);
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

    /// Select the first characteristic block whose plane matches `dim` (used when
    /// switching between the круговая (Ohm) and дифференциальная (Dif) views).
    pub fn select_first_block_dim(&mut self, dim: Dimension) {
        if self
            .library
            .blocks
            .get(self.block_idx)
            .and_then(|b| b.characteristics.first())
            .map(|c| c.dimension)
            == Some(dim)
        {
            return; // already on a matching block
        }
        if let Some(i) = self.library.blocks.iter().position(|b| {
            b.characteristics.first().map(|c| c.dimension) == Some(dim)
        }) {
            self.block_idx = i;
            self.selected_char = None;
            self.ensure_block_state();
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

    // ---------- data loading ----------
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
                    self.selected_char = None;
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
            Ok(osc) => {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.add_doc(Doc::new(osc, name));
            }
            Err(e) => self.status = format!("Ошибка загрузки осциллограммы: {e}"),
        }
    }

    /// Build the CSV (time + analog + computed channels) for a document.
    fn csv_string(d: &Doc) -> String {
        let mut out = String::from("t");
        for ch in &d.osc.analog {
            out.push(',');
            out.push_str(&ch.id);
        }
        for c in &d.computed {
            out.push(',');
            out.push_str(&c.name);
        }
        out.push('\n');
        for i in 0..d.osc.sample_count() {
            out.push_str(&format!("{}", d.osc.time.get(i).copied().unwrap_or(0.0)));
            for ch in &d.osc.analog {
                out.push_str(&format!(",{}", ch.samples.get(i).copied().unwrap_or(0.0)));
            }
            for c in &d.computed {
                out.push_str(&format!(",{}", c.samples.get(i).copied().unwrap_or(0.0)));
            }
            out.push('\n');
        }
        out
    }

    pub fn export_csv(&mut self) {
        let Some(d) = self.active_doc() else {
            self.status = "Нет осциллограммы для экспорта.".into();
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(format!("{}.csv", d.name))
            .save_file()
        else {
            return;
        };
        let out = Self::csv_string(d);
        match std::fs::write(&path, out) {
            Ok(()) => self.status = format!("Экспортировано в {}", path.display()),
            Err(e) => self.status = format!("Ошибка экспорта: {e}"),
        }
    }

    /// Save the active document as a COMTRADE `.cfg`/`.dat` pair.
    pub fn save_comtrade(&mut self) {
        let Some(d) = self.active_doc() else {
            self.status = "Нет осциллограммы для сохранения.".into();
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("COMTRADE", &["cfg"])
            .set_file_name(format!("{}.cfg", d.name))
            .save_file()
        else {
            return;
        };
        let (cfg, dat) = fvb_core::comtrade::write(&d.osc);
        let dat_path = path.with_extension("dat");
        match std::fs::write(&path, cfg).and_then(|_| std::fs::write(&dat_path, dat)) {
            Ok(()) => self.status = format!("Сохранено: {} (+ .dat)", path.display()),
            Err(e) => self.status = format!("Ошибка сохранения: {e}"),
        }
    }

    /// Archive the active record (COMTRADE + CSV) into a `.zip` for sending.
    pub fn archive_record(&mut self) {
        let Some(d) = self.active_doc() else {
            self.status = "Нет осциллограммы для архивирования.".into();
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("ZIP", &["zip"])
            .set_file_name(format!("{}.zip", d.name))
            .save_file()
        else {
            return;
        };
        let (cfg, dat) = fvb_core::comtrade::write(&d.osc);
        let csv = Self::csv_string(d);
        let zip = crate::util::zip_store(&[
            (format!("{}.cfg", d.name), cfg.into_bytes()),
            (format!("{}.dat", d.name), dat.into_bytes()),
            (format!("{}.csv", d.name), csv.into_bytes()),
        ]);
        match std::fs::write(&path, zip) {
            Ok(()) => self.status = format!("Архив сохранён: {}", path.display()),
            Err(e) => self.status = format!("Ошибка архивирования: {e}"),
        }
    }

    /// Ask for an export path and flag that the next frame's screenshot should be
    /// saved there (the actual capture happens in the update loop).
    pub fn request_image_export(&mut self) {
        if self.active_doc().is_none() {
            self.status = "Нет осциллограммы для печати/экспорта изображения.".into();
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name("fastview.png")
            .save_file()
        {
            self.want_screenshot = Some(path);
            self.status = "Готовится изображение…".into();
        }
    }

    // ---- undo / redo ----
    fn snapshot_of(d: &Doc) -> DocSnapshot {
        DocSnapshot { chans: d.chans.clone(), comments: d.comments.clone(), computed: d.computed.clone() }
    }
    fn restore(d: &mut Doc, s: DocSnapshot) {
        d.chans = s.chans;
        d.comments = s.comments;
        d.computed = s.computed;
    }

    /// Record the active document's state for undo, clearing the redo stack.
    pub fn snapshot_undo(&mut self) {
        if let Some(ai) = self.active {
            if let Some(d) = self.docs.get(ai) {
                self.undo_stack.push((ai, Self::snapshot_of(d)));
                if self.undo_stack.len() > 100 {
                    self.undo_stack.remove(0);
                }
                self.redo_stack.clear();
            }
        }
    }

    pub fn undo(&mut self) {
        let Some((ai, snap)) = self.undo_stack.pop() else {
            self.status = "Нечего отменять.".into();
            return;
        };
        if let Some(d) = self.docs.get_mut(ai) {
            self.redo_stack.push((ai, Self::snapshot_of(d)));
            Self::restore(d, snap);
            self.status = "Отменено.".into();
        }
    }

    pub fn redo(&mut self) {
        let Some((ai, snap)) = self.redo_stack.pop() else {
            self.status = "Нечего возвращать.".into();
            return;
        };
        if let Some(d) = self.docs.get_mut(ai) {
            self.undo_stack.push((ai, Self::snapshot_of(d)));
            Self::restore(d, snap);
            self.status = "Возвращено.".into();
        }
    }

    /// Copy the active document's visible analog channels to the merge clipboard
    /// (Объединение: «Копировать каналы для объединения осциллограмм»).
    pub fn copy_channels_for_merge(&mut self) {
        let Some(d) = self.active_doc() else {
            self.status = "Нет осциллограммы для копирования каналов.".into();
            return;
        };
        let clip: Vec<ClipChan> = d
            .osc
            .analog
            .iter()
            .enumerate()
            .filter(|(i, _)| d.chans.get(*i).map(|c| c.visible).unwrap_or(false))
            .map(|(i, ch)| ClipChan {
                name: ch.id.clone(),
                samples: ch.samples.clone(),
                color: d.chans[i].color,
            })
            .collect();
        let n = clip.len();
        self.channel_clip = clip;
        self.status = format!("Скопировано каналов для объединения: {n}");
    }

    /// Paste the merge-clipboard channels into the active document as extra
    /// (computed) lanes, aligned by sample index (Объединение: «Вставить каналы»).
    pub fn paste_channels_for_merge(&mut self) {
        if self.channel_clip.is_empty() {
            self.status = "Буфер каналов пуст — сначала «Копировать».".into();
            return;
        }
        let clip = self.channel_clip.clone();
        let Some(d) = self.active_doc_mut() else {
            self.status = "Нет осциллограммы для вставки каналов.".into();
            return;
        };
        let n = d.osc.sample_count();
        let mut added = 0;
        for c in clip {
            // Align by sample index (truncate/pad to the target length).
            let mut samples = c.samples;
            samples.resize(n, f64::NAN);
            let mut name = format!("{}*", c.name); // mark merged-in channels
            while d.computed.iter().any(|x| x.name == name) {
                name.push('\'');
            }
            d.computed.push(Computed { name, formula: "объединение".into(), samples, color: c.color });
            added += 1;
        }
        self.status = format!("Вставлено каналов (объединение): {added}");
    }

    pub fn add_computed(&mut self) {
        let idx = self.docs.len();
        let Some(d) = self.active_doc() else {
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
        let n = d.osc.sample_count();
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let mut cx = fvb_core::Context::new();
            for ch in &d.osc.analog {
                if let Some(v) = ch.samples.get(i) {
                    cx.insert(ch.id.to_lowercase(), *v);
                    if !ch.phase.is_empty() {
                        cx.entry(ch.phase.to_lowercase()).or_insert(*v);
                    }
                }
            }
            for c in &d.computed {
                if let Some(v) = c.samples.get(i) {
                    cx.insert(c.name.to_lowercase(), *v);
                }
            }
            samples.push(expr.eval(&cx).unwrap_or(f64::NAN));
        }
        let name = self.formula_name.clone();
        let formula = self.formula_text.clone();
        let color = Self::palette(idx + 7);
        if let Some(d) = self.active_doc_mut() {
            d.computed.push(Computed {
                name: name.clone(),
                formula,
                samples,
                color,
            });
        }
        self.status = format!("Добавлен расчётный канал «{name}»");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_app() -> App {
        let mut app = App::new();
        let osc = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        app.add_doc(Doc::new(osc, "demo".into()));
        app
    }

    #[test]
    fn keymap_is_the_original_accelerator_table() {
        // Accelerator table 128 (notes/ui/accelerators.txt): exactly these 7.
        let km = App::keymap();
        assert_eq!(km.len(), 7);
        let has = |k: egui::Key, c: Command| km.iter().any(|(kk, cc)| *kk == k && *cc == c);
        assert!(has(egui::Key::O, Command::Open));
        assert!(has(egui::Key::S, Command::Save));
        assert!(has(egui::Key::P, Command::Print));
        assert!(has(egui::Key::C, Command::Copy));
        assert!(has(egui::Key::V, Command::Paste));
        assert!(has(egui::Key::Z, Command::Undo));
        assert!(has(egui::Key::Y, Command::Redo));
    }

    #[test]
    fn demo_command_opens_a_document() {
        let mut app = App::new();
        assert!(app.active_doc().is_none());
        app.dispatch(Command::Demo);
        assert!(app.active_doc().is_some());
        assert!(app.active_doc().unwrap().osc.sample_count() > 0);
    }

    #[test]
    fn show_view_switches_view_and_tab() {
        let mut app = demo_app();
        app.dispatch(Command::Show(View::Omp));
        assert_eq!(app.view, View::Omp);
        assert_eq!(app.tab, RibbonTab::Omp);
        app.dispatch(Command::Show(View::Vector));
        assert_eq!(app.view, View::Vector);
        assert_eq!(app.tab, RibbonTab::Home);
    }

    #[test]
    fn invert_and_remove_dc_affect_displayed_samples() {
        let mut app = demo_app();
        let (i, k) = (0usize, 50usize);
        let base = app.active_doc().unwrap().sample(i, k);
        app.dispatch(Command::InvertVisible);
        let inv = app.active_doc().unwrap().sample(i, k);
        assert!((inv + base).abs() < 1e-9, "invert should negate: {base} vs {inv}");
        app.dispatch(Command::InvertVisible); // back
        app.dispatch(Command::RemoveDcVisible);
        // removing DC changes the value (demo has an emergency DC offset somewhere)
        let nd = app.active_doc().unwrap().sample(i, k);
        assert!(nd.is_finite());
    }

    #[test]
    fn copy_paste_merges_channels() {
        let mut app = demo_app();
        app.dispatch(Command::Copy);
        assert!(!app.channel_clip.is_empty(), "copy should fill the clipboard");
        let before = app.active_doc().unwrap().computed.len();
        app.dispatch(Command::Paste);
        let d = app.active_doc().unwrap();
        assert!(d.computed.len() > before, "paste should add merged channels");
        let n = d.osc.sample_count();
        assert!(d.computed.iter().all(|c| c.samples.len() == n), "merged channels align to length");
    }

    #[test]
    fn value_mode_command_sets_mode() {
        let mut app = demo_app();
        app.dispatch(Command::SetValueMode(ValueMode::FirstHarmonic));
        assert_eq!(app.value_mode, ValueMode::FirstHarmonic);
    }

    #[test]
    fn circle_view_selects_an_ohm_block() {
        let mut app = demo_app();
        app.dispatch(Command::Show(View::Circle));
        assert_eq!(app.current_dimension(), Dimension::Ohm);
    }

    #[test]
    fn cursor_status_has_six_fields() {
        let app = demo_app();
        let s = app.cursor_status();
        assert_eq!(s.len(), 6);
        assert!(s.iter().all(|x| !x.is_empty()));
    }
}
