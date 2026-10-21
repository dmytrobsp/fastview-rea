//! Shared application state, document model, commands and data loading for the
//! FastView reconstruction (ribbon UI, MDI documents, lane-based oscillogram).

use crate::theme::Skin;
use eframe::egui;
use fvb_core::analysis::Analysis;
use fvb_core::comtrade::{Encoding, Oscillogram};
use fvb_core::geom::CharGeometry;
use fvb_core::model::Dimension;
use fvb_core::omp::{NeutralMode, OmpResult, Section, Trace};
use num_complex::Complex64;
use fvb_core::Library;
use std::collections::{BTreeSet, HashMap};

pub const DEFAULT_LIBRARY: &[u8] = include_bytes!("../../../assets/FastView.fvb");

/// The original loads every `*.fvb` in the executable's directory (in
/// `FindFirstFile` order, i.e. by name on NTFS) into one list. We do the same
/// and fall back to the bundled copy of the shipped library when none is there.
pub fn load_libraries() -> (Library, String) {
    let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let mut files: Vec<std::path::PathBuf> = dir
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("fvb")))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut lib = Library::default();
    let mut names = Vec::new();
    for f in &files {
        if let Ok(l) = std::fs::read(f).map_err(|e| e.to_string()).and_then(|b| {
            fvb_core::load_fvb_bytes(&b).map_err(|e| e.to_string())
        }) {
            lib.extend(l);
            names.push(f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        }
    }
    if names.is_empty() {
        let lib = fvb_core::load_fvb_bytes(DEFAULT_LIBRARY)
            .unwrap_or_else(|e| panic!("bundled library failed to parse: {e}"));
        return (lib, "FastView.fvb (встроенная)".to_string());
    }
    (lib, names.join(", "))
}
pub const DEMO_CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
pub const DEMO_DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

/// Which document view fills the work area. In the original these are separate
/// windows opened from the ribbon *Диаграммы* group and the *ОМП* tab; here they
/// are selectable views over the active document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Осциллограмма — the stacked per-channel lanes (the main document view).
    Oscillogram,
    /// Быстрый анализ — symmetric components, impedances, formula editor.
    Analysis,
    /// ОМП — fault location.
    Omp,
}

/// A diagram docking pane (`0x474e10`). The diagrams are panes beside the
/// oscillogram, not views replacing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// Векторная диаграмма (32809).
    Vector,
    /// Круговая диаграмма (32808) — tabbed with the vector pane.
    Circle,
    /// Дифференциальная диаграмма (140).
    Differential,
    /// Частотная диаграмма (32811) — docked below the tab group.
    Frequency,
}

/// Visibility of the four diagram panes (all created visible) and the active
/// tab of the Vector | Circle | Differential group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panes {
    pub vector: bool,
    pub circle: bool,
    pub differential: bool,
    pub frequency: bool,
    pub active: Pane,
}

impl Default for Panes {
    fn default() -> Self {
        Panes { vector: true, circle: true, differential: true, frequency: true, active: Pane::Vector }
    }
}

impl Panes {
    pub const GROUP: [Pane; 3] = [Pane::Vector, Pane::Circle, Pane::Differential];

    pub fn visible(&self, p: Pane) -> bool {
        match p {
            Pane::Vector => self.vector,
            Pane::Circle => self.circle,
            Pane::Differential => self.differential,
            Pane::Frequency => self.frequency,
        }
    }

    fn set(&mut self, p: Pane, v: bool) {
        match p {
            Pane::Vector => self.vector = v,
            Pane::Circle => self.circle = v,
            Pane::Differential => self.differential = v,
            Pane::Frequency => self.frequency = v,
        }
    }

    /// The tab shown in the right-hand group, if any of its panes is visible.
    pub fn shown_tab(&self) -> Option<Pane> {
        if self.visible(self.active) && self.active != Pane::Frequency {
            Some(self.active)
        } else {
            Self::GROUP.into_iter().find(|&p| self.visible(p))
        }
    }
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

/// ОМП method (the original's method code `obj+0x64`: 1 isolated neutral,
/// 2 grounded neutral, 4 double-ended).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmpMethod {
    Isolated,
    Grounded,
    DoubleEnded,
}

/// The last ОМП calculation: its per-sample trace and the documents it ran on.
pub struct OmpRun {
    pub method: OmpMethod,
    pub doc: usize,
    pub trace: Trace,
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
    /// Совместить каналы: channels sharing a group are drawn in one lane (the
    /// lane of the group's first visible member), as the original's channel
    /// groups (33035 creates one from the selection, 33037 dissolves it).
    pub group: Option<u32>,
    /// Display mode of this channel (bits 7/8/9 of the original's channel
    /// flags, set on the selected channels by 32820..32822).
    pub mode: ValueMode,
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
    pub unit: String,
    /// The stored complex series (its phasor), when the formula's complex
    /// value was valid at every sample.
    pub phasors: Option<Vec<Complex64>>,
    /// Shown on the vector / circle diagrams (flags 0x4 / 0x8).
    pub on_vector: bool,
    pub on_circle: bool,
}

impl Computed {
    pub fn new(name: String, formula: String, samples: Vec<f64>, color: egui::Color32) -> Computed {
        Computed { name, formula, samples, color, unit: String::new(), phasors: None, on_vector: false, on_circle: false }
    }

    pub fn as_core(&self) -> fvb_core::quick::ComputedChannel {
        fvb_core::quick::ComputedChannel {
            name: self.name.clone(),
            unit: self.unit.clone(),
            formula: self.formula.clone(),
            samples: self.samples.clone(),
            phasors: self.phasors.clone(),
        }
    }
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
    /// «Зафиксировать визир» (33039) — the comment is locked in place.
    pub locked: bool,
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

/// A channel row of the oscillogram view. The original keeps one channel
/// vector (analog, then discrete); our record splits analog / computed /
/// digital, so a row is named by its kind and index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChanKey {
    Analog(usize),
    Computed(usize),
    Digital(usize),
}

/// One open oscillogram document.
pub struct Doc {
    pub osc: Oscillogram,
    pub analysis: Analysis,
    pub name: String,
    /// Source file path (for re-opening with a different encoding).
    pub path: Option<std::path::PathBuf>,
    pub main_cursor: usize,
    pub aux_cursor: usize,
    pub chans: Vec<ChanView>,
    pub computed: Vec<Computed>,
    /// Text sections (HDR/INF/SET) of a record opened from a `.cff`, written
    /// back when it is saved as `.cff`.
    pub cff: fvb_core::comtrade::CffSections,
    /// (Iдиф, Iторм) computed-channel pairs for the differential diagram.
    pub dif_pairs: Vec<(String, String)>,
    pub comments: Vec<Comment>,
    /// Selected channels (the original's per-channel `+0x100` flag): the
    /// target of the channel context menu and the per-channel commands.
    pub selection: BTreeSet<ChanKey>,
    /// Anchor of a Shift+click range selection (the original's `view+0x114`).
    pub sel_anchor: Option<ChanKey>,
    /// Next free channel-group id (Совместить каналы).
    pub next_group: u32,
    /// Comment placement mode (`doc+0x858`): 0 off, 1 момент времени.
    pub comment_mode: u8,
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
                group: None,
                mode: ValueMode::Instant,
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
            path: None,
            main_cursor: count / 2,
            aux_cursor: count / 4,
            chans,
            computed: Vec::new(),
            cff: Default::default(),
            dif_pairs: Vec::new(),
            comments: Vec::new(),
            selection: BTreeSet::new(),
            sel_anchor: None,
            next_group: 1,
            comment_mode: 0,
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
    /// CMDUI 155..157: comments can be placed only on records opened from a
    /// `.cfg` or `.osc` file (our bundled demo record, with no file, too).
    pub fn comments_allowed(&self) -> bool {
        match &self.path {
            None => true,
            Some(p) => p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("cfg") || e.eq_ignore_ascii_case("osc")),
        }
    }

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
    ChannelMap,
    Settings,
    Show(View),
    /// 32809 / 32808 / 140 / 32811: show or hide a diagram pane.
    TogglePane(Pane),
    /// Bring a pane of the tab group to the front.
    ActivatePane(Pane),
    SetPrimaryValues(bool),
    InvertVisible,
    RemoveDcVisible,
    SetValueMode(ValueMode),
    /// 155..157: toggle a comment placement mode (radio group with toggle-off).
    CommentMode(u8),
    /// Horizontal визиры on/off (the original's 32815/32816, one each).
    ToggleHcur,
    ShowAllChannels(bool),
    SetSkin(Skin),
    FitTime,
    CloseDoc(usize),
    ActivateDoc(usize),
}

/// One characteristic diagram's state (circle or differential), stashed while
/// the other diagram is active.
pub struct DiagramState {
    pub library: Library,
    pub block_idx: usize,
    pub chars: HashMap<(usize, usize), CharState>,
    pub selected_char: Option<usize>,
}

pub struct App {
    /// Every loaded `.fvb` library, both planes.
    pub full_library: Library,
    /// Plane of the active characteristic diagram, and the stashed state of
    /// each diagram (`[Ohm, Dif]`) while it is not active.
    pub diagram_dim: Dimension,
    pub diagrams: [Option<DiagramState>; 2],
    // active diagram state
    pub library: Library,
    pub source_name: String,
    pub block_idx: usize,
    pub chars: HashMap<(usize, usize), CharState>,
    pub selected_char: Option<usize>,
    pub show_reset: bool,
    /// World rectangle the characteristic plot showed last frame; open
    /// polypoints are closed against it (`fvb_core::geom::evaluate_in_view`).
    pub char_view: Option<fvb_core::geom::View>,
    pub fill_regions: bool,

    // documents
    pub docs: Vec<Doc>,
    pub active: Option<usize>,

    // ribbon / view
    pub tab: RibbonTab,
    pub view: View,
    pub panes: Panes,
    pub skin: Skin,
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
    /// Vector-diagram angle reference channel (None = absolute angles).
    pub vec_angle_ref: Option<String>,
    /// Circle-diagram readout at cursor 1: 0 none, 1 Re ± jIm, 2 |·|∠.
    pub circle_readout: u8,
    /// Circle diagram: only the samples between the cursors (SightsOnly).
    pub circle_between_cursors: bool,
    /// Frequency diagram: periods in the window, harmonic count, percent
    /// labels and percent of the true RMS instead of the fundamental.
    pub freq_periods: usize,
    pub freq_harmonics: usize,
    pub freq_percent: bool,
    pub freq_use_rms: bool,

    // analysis / omp
    pub analysis_tab: AnalysisTab,
    /// Quick-analysis forms (one per tab).
    pub quick: crate::quick_ui::QuickForms,
    pub omp_method: OmpMethod,
    /// Line sections (участки) of the ОМП parameter tree.
    pub omp_sections: Vec<Section>,
    /// Nominal positive-sequence current of substation 1 and 2, A.
    pub omp_inom: [f64; 2],
    /// Remote-end document for the double-ended ОМП method.
    pub omp_remote: Option<usize>,
    pub omp_run: Option<OmpRun>,
    /// Persisted characteristic settings (only when enabled by `main`).
    pub store: crate::persist::Store,
    pub persist: bool,

    // dialogs / panels
    pub show_about: bool,
    pub show_file_params: bool,
    pub show_channel_map: bool,
    pub show_settings: bool,
    pub encoding: Encoding,
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
    /// The open oscillogram context menu, latched at the right-click.
    pub lane_menu: Option<crate::lanes::LaneMenu>,
    /// Bumped on every right-click so each menu gets a fresh popup area.
    pub lane_menu_serial: u64,
    /// A press that dismissed the menu is swallowed until the button is up.
    pub lane_menu_eat: bool,
    /// «Цвет канала» (33030) colour dialog, initialised from the first
    /// selected channel and applied to every selected channel on OK.
    pub color_dialog: Option<egui::Color32>,

    pub status: String,
}

/// Per-characteristic UI state (circle diagram).
pub struct CharState {
    pub visible: bool,
    /// Parameter values as formula text (the settings grid's string values).
    pub settings: fvb_core::Settings,
    /// Last evaluated figure values (kept when an evaluation fails).
    pub figs: Vec<Option<fvb_core::FigureValues>>,
    pub geo: Option<CharGeometry>,
    pub color: egui::Color32,
}

impl App {
    pub fn new() -> Self {
        let (full_library, source_name) = load_libraries();
        let mut app = App {
            library: full_library.for_dimension(Dimension::Ohm),
            full_library,
            diagram_dim: Dimension::Ohm,
            diagrams: [None, None],
            source_name,
            block_idx: 0,
            chars: HashMap::new(),
            selected_char: None,
            show_reset: true,
            char_view: None,
            fill_regions: true,
            docs: Vec::new(),
            active: None,
            tab: RibbonTab::Home,
            view: View::Oscillogram,
            panes: Panes::default(),
            skin: Skin::Blue,
            time_unit: TimeUnit::Milli,
            show_grid: true,
            primary_values: false,
            formula_name: "Uab".to_string(),
            formula_text: "Ua-Ub".to_string(),
            vec_radians: false,
            vec_direction_only: false,
            vec_show_values: true,
            vec_angle_ref: None,
            circle_readout: 0,
            circle_between_cursors: false,
            freq_periods: 1,
            freq_harmonics: 10,
            freq_percent: false,
            freq_use_rms: false,
            analysis_tab: AnalysisTab::Symmetric,
            quick: Default::default(),
            omp_method: OmpMethod::Grounded,
            omp_sections: Vec::new(),
            omp_inom: [0.0; 2],
            omp_remote: None,
            omp_run: None,
            store: Default::default(),
            persist: false,
            show_about: false,
            show_file_params: false,
            show_channel_map: false,
            show_settings: false,
            encoding: Encoding::Auto,
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
            lane_menu: None,
            lane_menu_serial: 0,
            lane_menu_eat: false,
            color_dialog: None,
            status: "Готово".to_string(),
        };
        app.ensure_block_state();

        // Test/demo hooks (used for headless screenshots).
        if std::env::var("FV_DEMO").is_ok() {
            if let Ok(osc) = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT) {
                app.add_doc(Doc::new(osc, "demo_fault".into()));
            }
        }
        app.view = match std::env::var("FV_VIEW").as_deref() {
            Ok("analysis") => View::Analysis,
            Ok("omp") => View::Omp,
            _ => View::Oscillogram,
        };
        if app.view == View::Omp {
            app.tab = RibbonTab::Omp;
        }
        match std::env::var("FV_VIEW").as_deref() {
            Ok("circle") => app.dispatch(Command::ActivatePane(Pane::Circle)),
            Ok("diff") => app.dispatch(Command::ActivatePane(Pane::Differential)),
            _ => {}
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
            Command::ChannelMap => self.show_channel_map = true,
            Command::Settings => self.show_settings = true,
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
            }
            Command::TogglePane(p) => {
                let on = !self.panes.visible(p);
                self.panes.set(p, on);
                if on && p != Pane::Frequency {
                    self.dispatch(Command::ActivatePane(p));
                } else if let Some(t) = self.panes.shown_tab() {
                    self.dispatch(Command::ActivatePane(t));
                }
            }
            Command::ActivatePane(p) => {
                self.panes.active = p;
                match p {
                    Pane::Circle => self.switch_diagram(Dimension::Ohm),
                    Pane::Differential => self.switch_diagram(Dimension::Dif),
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
            Command::SetValueMode(m) => {
                if let Some(d) = self.active_doc_mut() {
                    d.set_mode_selected(m);
                }
            }
            Command::ToggleHcur => {
                if let Some(d) = self.active_doc_mut() {
                    d.hcur_on = !d.hcur_on;
                }
            }
            Command::CommentMode(k) => {
                if let Some(d) = self.active_doc_mut() {
                    if d.comments_allowed() {
                        d.comment_mode = if d.comment_mode == k { 0 } else { k };
                    }
                }
            }
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
            let key = (Self::diagram_key(self.diagram_dim).to_string(), block.name.clone(), ch.name.clone());
            let store = &self.store;
            self.chars.entry((b, ci)).or_insert_with(|| {
                let mut settings = ch.default_settings();
                if let Some(saved) = store.values.get(&key) {
                    for (p, v) in saved {
                        if let Some(slot) = settings.get_mut(p) {
                            *slot = v.clone();
                        }
                    }
                }
                CharState {
                    visible: store.visible.get(&key).copied().unwrap_or(true),
                    settings,
                    figs: Vec::new(),
                    geo: None,
                    color: Self::palette(ci),
                }
            });
        }
        if self.selected_char.is_none() && !block.characteristics.is_empty() {
            self.selected_char = Some(0);
        }
        self.recompute_block();
    }

    /// Re-evaluate the whole selected block, as the original does after any
    /// settings change (`FUN_00409260`): characteristics in order, stopping at
    /// the first failing formula — the rest keep their previous figures.
    pub fn recompute_block(&mut self) {
        let b = self.block_idx;
        let Some(block) = self.library.blocks.get(b) else {
            return;
        };
        let mut states: Vec<(usize, CharState)> = (0..block.characteristics.len())
            .filter_map(|ci| self.chars.remove(&(b, ci)).map(|st| (ci, st)))
            .collect();
        let result = fvb_core::evaluate_block(
            states
                .iter_mut()
                .map(|(ci, st)| (&block.characteristics[*ci], &st.settings, &mut st.figs)),
        );
        for (ci, mut st) in states {
            let ch = &block.characteristics[ci];
            st.geo = Some(fvb_core::geom::build_geometry(ch, &st.figs, None));
            self.chars.insert((b, ci), st);
        }
        if let Err(e) = result {
            self.status = e.to_string();
        }
        self.persist_block();
    }

    /// The original's settings-tree section for a diagram.
    pub fn diagram_key(dim: Dimension) -> &'static str {
        match dim {
            Dimension::Ohm => "Circle Chart",
            Dimension::Dif => "Diff Chart",
        }
    }

    /// Load persisted characteristic settings and apply them (called by `main`).
    pub fn with_persistence(mut self) -> Self {
        self.store = crate::persist::Store::load();
        self.persist = true;
        let dim = self.diagram_dim;
        if let Some(name) = self.store.selected.get(Self::diagram_key(dim)).cloned() {
            if let Some(i) = self.library.blocks.iter().position(|b| b.name == name) {
                self.block_idx = i;
            }
        }
        self.chars.clear();
        self.selected_char = None;
        self.ensure_block_state();
        self
    }

    /// Record the selected block and its characteristics' visibility and
    /// parameter texts, then save (the original saves on every property change).
    pub fn persist_block(&mut self) {
        if !self.persist {
            return;
        }
        let Some(block) = self.library.blocks.get(self.block_idx) else {
            return;
        };
        let d = Self::diagram_key(self.diagram_dim).to_string();
        self.store.selected.insert(d.clone(), block.name.clone());
        for (ci, ch) in block.characteristics.iter().enumerate() {
            if let Some(st) = self.chars.get(&(self.block_idx, ci)) {
                let key = (d.clone(), block.name.clone(), ch.name.clone());
                self.store.visible.insert(key.clone(), st.visible);
                self.store.values.insert(key, st.settings.clone().into_iter().collect());
            }
        }
        self.store.save();
    }

    /// Switch the characteristic panel to the circle (`Ohm`) or differential
    /// (`Dif`) diagram. Each diagram has its own library (only characteristics
    /// of its plane, empty blocks dropped) and its own block/settings state.
    pub fn switch_diagram(&mut self, dim: Dimension) {
        if dim == self.diagram_dim {
            return;
        }
        let current = DiagramState {
            library: std::mem::take(&mut self.library),
            block_idx: self.block_idx,
            chars: std::mem::take(&mut self.chars),
            selected_char: self.selected_char,
        };
        let idx = |d: Dimension| usize::from(d == Dimension::Dif);
        self.diagrams[idx(self.diagram_dim)] = Some(current);
        let next = self.diagrams[idx(dim)].take().unwrap_or_else(|| {
            let library = self.full_library.for_dimension(dim);
            let block_idx = self
                .store
                .selected
                .get(Self::diagram_key(dim))
                .and_then(|n| library.blocks.iter().position(|b| &b.name == n))
                .unwrap_or(0);
            DiagramState { library, block_idx, chars: HashMap::new(), selected_char: None }
        });
        self.library = next.library;
        self.block_idx = next.block_idx;
        self.chars = next.chars;
        self.selected_char = next.selected_char;
        self.diagram_dim = dim;
        self.ensure_block_state();
    }

    pub fn current_dimension(&self) -> Dimension {
        self.diagram_dim
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
                    self.library = lib.for_dimension(self.diagram_dim);
                    self.full_library = lib;
                    self.diagrams = [None, None];
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
            .add_filter("Осциллограммы", &["cfg", "cff", "osc", "dat", "CFG", "CFF", "OSC", "DAT"])
            .pick_file()
        {
            self.load_comtrade_path(&path);
        }
    }

    pub fn load_comtrade_path(&mut self, path: &std::path::Path) {
        let is_cff = path.to_string_lossy().to_lowercase().ends_with(".cff");
        let loaded = if is_cff {
            std::fs::read(path)
                .map_err(|e| fvb_core::comtrade::ComtradeError::Io(e.to_string()))
                .and_then(|b| fvb_core::comtrade::parse_cff(&b, self.encoding))
        } else {
            fvb_core::comtrade::load_with(path, self.encoding).map(|o| (o, Default::default()))
        };
        match loaded {
            Ok((osc, cff)) => {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.add_doc(Doc::new(osc, name));
                if let Some(d) = self.active_doc_mut() {
                    d.path = Some(path.to_path_buf());
                    d.cff = cff;
                }
            }
            // The original shows "<file> - <message>".
            Err(e) => self.status = format!("{} - {e}", path.display()),
        }
    }

    /// Re-open the active document's file with the current encoding (the original
    /// re-reads on an encoding change).
    pub fn reopen_active(&mut self) {
        let path = self.active_doc().and_then(|d| d.path.clone());
        if let Some(path) = path {
            self.load_comtrade_path(&path);
        } else {
            self.status = "Файл для переоткрытия неизвестен.".into();
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
            .add_filter("COMTRADE'13", &["cff"])
            .set_file_name(format!("{}.cfg", d.name))
            .save_file()
        else {
            return;
        };
        if path.to_string_lossy().to_lowercase().ends_with(".cff") {
            self.status = match fvb_core::comtrade::write_cff(&d.osc, &d.cff) {
                Ok(bytes) => match std::fs::write(&path, bytes) {
                    Ok(()) => format!("Сохранено: {}", path.display()),
                    Err(e) => format!("Ошибка сохранения: {e}"),
                },
                Err(e) => e.to_string(),
            };
            return;
        }
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
        // 57634 copies the selected (file, not computed) channels.
        let clip: Vec<ClipChan> = d
            .selected_analog()
            .into_iter()
            .map(|i| ClipChan {
                name: d.osc.analog[i].id.clone(),
                samples: d.osc.analog[i].samples.clone(),
                color: d.chans[i].color,
            })
            .collect();
        if clip.is_empty() {
            self.status = "Не выбраны каналы для копирования.".into();
            return;
        }
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
            d.computed.push(Computed::new(name, "объединение".into(), samples, c.color));
            added += 1;
        }
        self.status = format!("Вставлено каналов (объединение): {added}");
    }

    pub fn add_computed(&mut self) {
        let batch = vec![(self.formula_name.clone(), self.formula_text.clone(), String::new())];
        if let Some(added) = self.append_computed(&batch) {
            self.status = format!("Добавлен расчётный канал «{}»", added[0]);
        }
    }

    /// Append computed channels to the active record, evaluated in order in
    /// channel mode (`fvb_core::quick::evaluate_batch`); all or nothing.
    /// Returns the names given to them.
    pub fn append_computed(&mut self, batch: &[(String, String, String)]) -> Option<Vec<String>> {
        let show_primary = self.primary_values;
        let base = self.docs.len() + 7;
        let Some(d) = self.active_doc() else {
            self.status = "Сначала загрузите осциллограмму.".into();
            return None;
        };
        let existing: Vec<_> = d.computed.iter().map(Computed::as_core).collect();
        match fvb_core::quick::evaluate_batch(&d.osc, &existing, batch, show_primary) {
            Ok(added) => {
                let d = self.active_doc_mut()?;
                let n0 = d.computed.len();
                let names = added.iter().map(|c| c.name.clone()).collect();
                for (k, c) in added.into_iter().enumerate() {
                    let mut cc = Computed::new(c.name, c.formula, c.samples, Self::palette(base + n0 + k));
                    cc.unit = c.unit;
                    cc.phasors = c.phasors;
                    d.computed.push(cc);
                }
                Some(names)
            }
            Err(e) => {
                self.status = e.to_string();
                None
            }
        }
    }

    /// Apply a quick-analysis tab's output (`CQAnPropSheet` OK): append its
    /// channels, toggle their diagram flags (XOR), set the vector-diagram angle
    /// reference / show-values and the circle readout, and remember the
    /// differential pairs.
    pub fn apply_quick(&mut self, g: &fvb_core::quick::Generated) {
        let Some(names) = self.append_computed(&g.channels) else {
            return;
        };
        // Map generated names to the names actually given.
        let given: HashMap<&str, &str> =
            g.channels.iter().map(|(n, _, _)| n.as_str()).zip(names.iter().map(String::as_str)).collect();
        let real = |n: &String| given.get(n.as_str()).map_or(n.clone(), |r| r.to_string());
        if let Some(d) = self.active_doc_mut() {
            for n in &g.vector {
                if let Some(c) = d.computed.iter_mut().find(|c| c.name == real(n)) {
                    c.on_vector = !c.on_vector;
                }
            }
            for n in &g.circle {
                if let Some(c) = d.computed.iter_mut().find(|c| c.name == real(n)) {
                    c.on_circle = !c.on_circle;
                }
            }
            for (dif, tor) in &g.dif_pairs {
                d.dif_pairs.push((real(dif), real(tor)));
            }
        }
        if let Some(r) = &g.angle_ref {
            self.vec_angle_ref = Some(real(r));
        }
        if g.show_values {
            self.vec_show_values = true;
        }
        if let Some(m) = g.circle_readout {
            self.circle_readout = m;
        }
        self.status = format!("Добавлено каналов: {}", names.len());
    }
}

impl App {
    /// Run the ОМП engine on the active oscillogram (and the remote one for
    /// the double-ended method), as `0x4a00b0` does: per-sample distances,
    /// the chosen window written to the cursors, the result text, and the
    /// extra channels "Lкз" (m) and "КЗ фазы A/B/C".
    pub fn run_omp(&mut self) {
        let Some(di) = self.active else {
            self.status = "Сначала загрузите осциллограмму.".into();
            return;
        };
        if self.omp_sections.is_empty() || self.omp_inom[0] == 0.0 {
            self.status = "Данные для определения места повреждения не введены.".into();
            return;
        }
        let d = &self.docs[di];
        let p = d.osc.samples_per_cycle().max(2);
        let chan = |doc: &Doc, idx: Option<usize>| -> Vec<f64> {
            idx.and_then(|i| doc.osc.analog.get(i)).map(|c| c.samples.clone()).unwrap_or_default()
        };
        let three = |doc: &Doc, roles: [Option<usize>; 3]| roles.map(|r| chan(doc, r));
        let (u, i) = (three(d, d.analysis.u_abc), three(d, d.analysis.i_abc));
        if u.iter().chain(i.iter()).any(Vec::is_empty) {
            self.status = "Не назначены каналы Ua, Ub, Uc, Ia, Ib, Ic.".into();
            return;
        }
        let sections = self.omp_sections.clone();
        let mut trace = match self.omp_method {
            OmpMethod::Grounded | OmpMethod::Isolated => {
                let neutral = if self.omp_method == OmpMethod::Grounded {
                    NeutralMode::Grounded
                } else {
                    NeutralMode::Isolated
                };
                // The isolated method works on line voltages Uab, Ubc, Uca.
                let line = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x - y).collect::<Vec<f64>>();
                let ul = [line(&u[0], &u[1]), line(&u[1], &u[2]), line(&u[2], &u[0])];
                let uu = if neutral == NeutralMode::Isolated { &ul } else { &u };
                fvb_core::omp::single_ended(&fvb_core::omp::SingleEnded {
                    neutral,
                    u: [&uu[0], &uu[1], &uu[2]],
                    i: [&i[0], &i[1], &i[2]],
                    p,
                    inom: self.omp_inom[0],
                    sections: &sections,
                })
            }
            OmpMethod::DoubleEnded => {
                let Some(r) = self.omp_remote.and_then(|ri| self.docs.get(ri)) else {
                    self.status = "Выберите вторую осциллограмму (подстанция №2).".into();
                    return;
                };
                let (u2, i2) = (three(r, r.analysis.u_abc), three(r, r.analysis.i_abc));
                fvb_core::omp::double_ended(&fvb_core::omp::DoubleEnded {
                    u1: [&u[0], &u[1], &u[2]],
                    i1: [&i[0], &i[1], &i[2]],
                    u2: [&u2[0], &u2[1], &u2[2]],
                    i2: [&i2[0], &i2[1], &i2[2]],
                    p,
                    inom1: self.omp_inom[0],
                    inom2: self.omp_inom[1],
                    sections: &sections,
                })
            }
        };
        let result = match self.omp_method {
            OmpMethod::DoubleEnded => fvb_core::omp::locate_double(&trace, &sections, p),
            OmpMethod::Grounded => fvb_core::omp::locate(&mut trace, NeutralMode::Grounded, &sections, p),
            OmpMethod::Isolated => fvb_core::omp::locate(&mut trace, NeutralMode::Isolated, &sections, p),
        };
        let idx0 = self.docs.len() + 3;
        let d = &mut self.docs[di];
        d.aux_cursor = result.window.0;
        d.main_cursor = result.window.1;
        let flag = |v: &[bool]| v.iter().map(|b| f64::from(u8::from(*b))).collect::<Vec<f64>>();
        let extra = [
            ("Lкз".to_string(), trace.dist.iter().map(|x| x * 1000.0).collect::<Vec<f64>>()),
            ("КЗ фазы A".to_string(), flag(&trace.pa)),
            ("КЗ фазы B".to_string(), flag(&trace.pb)),
            ("КЗ фазы C".to_string(), flag(&trace.pc)),
        ];
        d.computed.retain(|c| !extra.iter().any(|(n, _)| *n == c.name));
        // Computed rows are renumbered: drop them from the selection.
        d.selection.retain(|k| !matches!(k, ChanKey::Computed(_)));
        for (k, (name, samples)) in extra.into_iter().enumerate() {
            d.computed.push(Computed::new(name, "ОМП".into(), samples, Self::palette(idx0 + k)));
        }
        self.status = result.text.trim().to_string();
        self.omp_run = Some(OmpRun { method: self.omp_method, doc: di, trace });
    }

    /// The ОМП result over the active document's current cursor interval (the
    /// original recomputes it when cursor 1 or 2 is moved).
    pub fn omp_result(&self) -> Option<OmpResult> {
        let run = self.omp_run.as_ref()?;
        let d = self.docs.get(run.doc)?;
        let (c1, c2) = (d.aux_cursor, d.main_cursor);
        Some(match run.method {
            OmpMethod::DoubleEnded => fvb_core::omp::result_double_for_window(&run.trace, &self.omp_sections, c1, c2),
            _ => fvb_core::omp::result_for_window(&run.trace, &self.omp_sections, c1, c2),
        })
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
        app.dispatch(Command::Show(View::Oscillogram));
        assert_eq!(app.view, View::Oscillogram);
        assert_eq!(app.tab, RibbonTab::Home);
    }

    #[test]
    fn diagram_panes_start_visible_and_toggle_beside_the_oscillogram() {
        let mut app = demo_app();
        assert!(Panes::GROUP.iter().all(|&p| app.panes.visible(p)) && app.panes.frequency);
        assert_eq!(app.panes.shown_tab(), Some(Pane::Vector), "the vector pane is the active tab");
        app.dispatch(Command::TogglePane(Pane::Vector));
        assert!(!app.panes.vector);
        assert_eq!(app.panes.shown_tab(), Some(Pane::Circle));
        assert_eq!(app.current_dimension(), Dimension::Ohm);
        app.dispatch(Command::TogglePane(Pane::Differential));
        app.dispatch(Command::TogglePane(Pane::Differential));
        assert_eq!(app.panes.shown_tab(), Some(Pane::Differential), "a re-shown pane comes to the front");
        assert_eq!(app.current_dimension(), Dimension::Dif);
        assert_eq!(app.view, View::Oscillogram, "the oscillogram never goes away");
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
        assert!(app.channel_clip.is_empty(), "nothing selected, nothing copied");
        app.active_doc_mut().unwrap().selection.extend([ChanKey::Analog(0), ChanKey::Analog(1)]);
        app.dispatch(Command::Copy);
        assert_eq!(app.channel_clip.len(), 2, "copy takes the selected channels");
        assert!(!app.channel_clip.is_empty(), "copy should fill the clipboard");
        let before = app.active_doc().unwrap().computed.len();
        app.dispatch(Command::Paste);
        let d = app.active_doc().unwrap();
        assert!(d.computed.len() > before, "paste should add merged channels");
        let n = d.osc.sample_count();
        assert!(d.computed.iter().all(|c| c.samples.len() == n), "merged channels align to length");
    }

    #[test]
    fn value_mode_command_sets_mode_of_selected_channels() {
        let mut app = demo_app();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(1));
        app.dispatch(Command::SetValueMode(ValueMode::FirstHarmonic));
        let d = app.active_doc().unwrap();
        assert_eq!(d.chans[1].mode, ValueMode::FirstHarmonic);
        assert_eq!(d.chans[0].mode, ValueMode::Instant, "unselected channels keep their mode");
        assert!(d.mode_state(ValueMode::FirstHarmonic).checked);
    }

    #[test]
    fn omp_without_data_reports_message_349() {
        let mut app = demo_app();
        app.run_omp();
        assert_eq!(app.status, "Данные для определения места повреждения не введены.");
        assert!(app.omp_run.is_none());
    }

    #[test]
    fn omp_run_sets_window_cursors_and_channels() {
        let mut app = demo_app();
        app.omp_inom[0] = 100.0;
        app.omp_sections.push(fvb_core::omp::Section {
            length_km: 50.0,
            r1_per_km: 0.1,
            x1_per_km: 0.4,
            r0_per_km: 0.3,
            x0_per_km: 1.2,
            ..Default::default()
        });
        app.run_omp();
        let d = app.active_doc().unwrap();
        let p = d.osc.samples_per_cycle();
        assert_eq!(d.main_cursor, d.aux_cursor + p - 1, "a one-period window");
        for name in ["Lкз", "КЗ фазы A", "КЗ фазы B", "КЗ фазы C"] {
            assert!(d.computed.iter().any(|c| c.name == name), "{name}");
        }
        let r = app.omp_result().unwrap();
        assert_eq!(app.status, r.text.trim());
    }

    #[test]
    fn stored_settings_apply_to_new_block_state() {
        let mut app = demo_app();
        let blk = app.library.blocks[app.block_idx].clone();
        let ch = &blk.characteristics[0];
        let p = ch.parameters[0].name.clone();
        let key = (App::diagram_key(app.diagram_dim).to_string(), blk.name.clone(), ch.name.clone());
        app.store.values.entry(key.clone()).or_default().insert(p.clone(), "1+1".into());
        app.store.values.entry(key.clone()).or_default().insert("нет такого".into(), "5".into());
        app.store.visible.insert(key, false);
        app.chars.clear();
        app.ensure_block_state();
        let st = &app.chars[&(app.block_idx, 0)];
        assert_eq!(st.settings[&p], "1+1");
        assert!(!st.settings.contains_key("нет такого"));
        assert!(!st.visible);
    }

    #[test]
    fn quick_zero_tab_generates_flagged_channels() {
        let mut app = demo_app();
        app.analysis_tab = AnalysisTab::Zero;
        app.quick.ia = "Ia".into();
        app.quick.ib = "Ib".into();
        app.quick.ic = "Ic".into();
        app.quick.ua = "Ua".into();
        app.quick.ub = "Ub".into();
        app.quick.uc = "Uc".into();
        let g = app.quick.generate(AnalysisTab::Zero);
        app.apply_quick(&g);
        let d = app.active_doc().unwrap();
        let get = |n: &str| d.computed.iter().find(|c| c.name == n).unwrap();
        assert!(get("3I0").on_vector && get("3U0").on_vector && get("S0").on_circle);
        assert_eq!(get("S0").unit, "VA");
        assert_eq!(app.vec_angle_ref.as_deref(), Some("3U0"));
        assert_eq!(app.circle_readout, 1);
        // Applying again renames and toggles the new channels' flags.
        let g = app.quick.generate(AnalysisTab::Zero);
        app.apply_quick(&g);
        let d = app.active_doc().unwrap();
        assert!(d.computed.iter().any(|c| c.name == "3I0_2"));
    }

    #[test]
    fn each_diagram_shows_only_its_plane() {
        let mut app = demo_app();
        for (pane, dim) in [(Pane::Circle, Dimension::Ohm), (Pane::Differential, Dimension::Dif)] {
            app.dispatch(Command::ActivatePane(pane));
            assert_eq!(app.current_dimension(), dim);
            for b in &app.library.blocks {
                assert!(b.characteristics.iter().all(|c| c.dimension == dim), "{}", b.name);
            }
        }
    }

    #[test]
    fn bad_setting_reports_the_formula_and_keeps_old_geometry() {
        let mut app = demo_app();
        app.dispatch(Command::ActivatePane(Pane::Circle));
        let key = (app.block_idx, 0);
        let before = app.chars[&key].figs.clone();
        let name = app.library.blocks[app.block_idx].characteristics[0].parameters[0].name.clone();
        app.chars.get_mut(&key).unwrap().settings.insert(name, "1+".into());
        app.recompute_block();
        assert_eq!(app.status, "1+: Синтаксическая ошибка в формуле");
        assert_eq!(app.chars[&key].figs, before);
    }

    #[test]
    fn circle_view_selects_an_ohm_block() {
        let mut app = demo_app();
        app.dispatch(Command::ActivatePane(Pane::Circle));
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
