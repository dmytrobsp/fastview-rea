//! Shared application state, document model, commands and data loading for the
//! FastView reconstruction (ribbon UI, MDI documents, lane-based oscillogram).

use crate::theme::Skin;
use eframe::egui;
use fvb_core::analysis::Analysis;
use fvb_core::comtrade::{Encoding, Oscillogram};
use fvb_core::geom::CharGeometry;
use fvb_core::model::Dimension;
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

/// Which document view fills the work area: always the oscillogram — the
/// diagrams are docking panes, ОМП a result bar plus a dialog (`omp_ui`)
/// and «Быстрый анализ» a modal sheet (`quick_ui`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Осциллограмма — the stacked per-channel lanes (the main document view).
    Oscillogram,
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

/// The application title (`AFX_IDS_APP_TITLE` 57344): the main window's
/// and every message box's caption (`AfxMessageBox` uses `m_pszAppName`).
pub const APP_TITLE: &str = "Механотроника FastView";

/// The tab auto colours of mfc120u's `CMFCBaseTabCtrl` (more than 256
/// colours on screen; `0x1006a9a7`), in order.
pub const TAB_COLORS: [(u8, u8, u8); 16] = [
    (197, 212, 242),
    (255, 220, 120),
    (190, 206, 161),
    (240, 160, 161),
    (188, 168, 225),
    (156, 193, 182),
    (247, 184, 134),
    (217, 173, 194),
    (165, 194, 215),
    (179, 166, 190),
    (234, 214, 163),
    (246, 250, 125),
    (181, 233, 157),
    (95, 195, 207),
    (193, 131, 131),
    (202, 202, 213),
];

/// The two statics behind the tab colours (`0x102b0588` the last index
/// given, `0x102b0584` a counter; both start at 0 and are shared by every
/// tab control of the process).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TabAuto {
    pub last: usize,
    pub counter: usize,
}

impl TabAuto {
    /// The colour of a tab without one at position `pos`, whose left
    /// neighbour has colour `prev` (`0x1006a29c`): `pos` mod 16, unless that
    /// is the last index given or the neighbour's colour — then the next
    /// counter value mod 16, each time.
    pub fn assign(&mut self, pos: usize, prev: Option<usize>) -> usize {
        let n = TAB_COLORS.len();
        let mut idx = pos % n;
        if idx == self.last {
            self.counter += 1;
            idx = self.counter % n;
        }
        if prev.is_some_and(|p| TAB_COLORS[p] == TAB_COLORS[idx]) {
            self.counter += 1;
            idx = self.counter % n;
        }
        self.last = idx;
        idx
    }
}

/// The keyboard focus as copy's picture (`0x405160`) sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CopyFocus {
    /// The oscillogram view (or a window without untitled children): the
    /// view's picture, then the document.
    #[default]
    View,
    /// A diagram pane (a click in its window): that diagram's picture only.
    Diagram(Pane),
    /// A circle / differential property grid (its untitled children are a
    /// scroll bar and a header, no diagram window): the document only.
    Grid,
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

/// (channel, mode) → (generation, [finite min, finite max, min, max]).
pub type MinMaxCache = std::cell::RefCell<std::collections::HashMap<(usize, ValueMode), (u64, [f64; 4])>>;

/// The per-channel value the stats panel and vector/circle views read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Time of day from the record's start stamp (33007).
    Absolute,
    Samples,
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
    /// Vertical window zoom «Приблизить по вертикали» (138), reset by «Вернуть
    /// к исходному размеру» (141).
    pub window: Option<crate::oscgeom::Window>,
    /// Совместить каналы: channels sharing a group are drawn in one lane (the
    /// lane of the group's first visible member), as the original's channel
    /// groups (33035 creates one from the selection, 33037 dissolves it).
    pub group: Option<u32>,
    /// Display mode of this channel (bits 7/8/9 of the original's channel
    /// flags, set on the selected channels by 32820..32822).
    pub mode: ValueMode,
    pub stats: StatRows,
    pub on_freq: bool,
    pub on_vector: bool,
    pub on_circle: bool,
    /// Values in primary units (`ch+0x89`, set on every file channel by
    /// 32867 «Первичные» / 32868 «Вторичные»).
    pub primary: bool,
}

/// Stats-panel rows of a channel (channel flag bits 4 Max/Min, 6 СКЗ, 5 A1):
/// drawn only when set; 32817 / 32818 / 32819 toggle them on every analog
/// channel at once. A new channel shows СКЗ and A1 (as in the help
/// screenshots; the constructor default was not read).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatRows {
    pub minmax: bool,
    pub rms: bool,
    pub a1: bool,
}

impl Default for StatRows {
    fn default() -> Self {
        StatRows { minmax: false, rms: true, a1: true }
    }
}

impl StatRows {
    /// Number of text rows they take (Max and Min are two).
    pub fn count(self) -> usize {
        2 * usize::from(self.minmax) + usize::from(self.rms) + usize::from(self.a1)
    }
}

impl ChanView {
    /// A new channel's view state (the `i`-th analog channel of its record).
    pub fn new(ch: &fvb_core::comtrade::AnalogChannel, i: usize) -> ChanView {
        ChanView {
            visible: true,
            color: default_channel_color(ch, i),
            invert: false,
            remove_dc: false,
            adc_counts: false,
            window: None,
            group: None,
            mode: ValueMode::Instant,
            stats: StatRows::default(),
            on_freq: true,
            // Cleared on attach unless the unit passes the vector test
            // (`0x48fda0`), so a drawn phasor is always listed.
            on_vector: crate::diagram::vector_unit_ok(&ch.unit),
            // The circle flag is cleared on attach unless the unit is
            // Ohm / VA (`0x436b30`).
            on_circle: i == 0 && crate::diagram::circle_pane_unit_ok(&ch.unit),
            primary: false,
        }
    }
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
    pub stats: StatRows,
    /// A pending (pasted) lane's place: in front of this analog / digital
    /// channel (`Digital(n)` = the end of the digital block); `None` for the
    /// computed section (`0x45ad02` insertion points).
    pub before: Option<ChanKey>,
    /// Shown on the oscillogram (bit 0; «Каналы» dialog column 1).
    pub visible: bool,
}

impl Computed {
    pub fn new(name: String, formula: String, samples: Vec<f64>, color: egui::Color32) -> Computed {
        Computed {
            name,
            formula,
            samples,
            color,
            unit: String::new(),
            phasors: None,
            on_vector: false,
            on_circle: false,
            stats: StatRows::default(),
            visible: true,
            before: None,
        }
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

pub use crate::comments::{Comment, CommentEdit};

pub use crate::merge_ui::ClipChan;

/// State of the rectangle vertical zoom (138).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RectZoom {
    Off,
    /// Armed: the next press on an analog lane starts the rectangle.
    Armed,
    /// Dragging on analog channel `chan`'s lane from `y0` to `y1` at `x`.
    Dragging { chan: usize, x: f32, y0: f32, y1: f32 },
}

type LoadResult = Result<(Oscillogram, fvb_core::comtrade::CffSections), fvb_core::comtrade::ComtradeError>;

/// A record being parsed on a worker thread.
pub struct Loading {
    pub path: std::path::PathBuf,
    /// Large enough for the progress dialog.
    pub big: bool,
    rx: std::sync::mpsc::Receiver<LoadResult>,
    /// The worker's counters and the cancel flag.
    pub progress: std::sync::Arc<fvb_core::comtrade::Progress>,
    /// The bar's value and when the 500 ms timer last set it (`0x471870`).
    pub shown: (u32, f64),
}

/// Parse a `.cfg` / `.cff` / `.osc` record.
fn parse_file(path: &std::path::Path, enc: Encoding, prog: Option<&fvb_core::comtrade::Progress>) -> LoadResult {
    if path.to_string_lossy().to_lowercase().ends_with(".cff") {
        std::fs::read(path)
            .map_err(|e| fvb_core::comtrade::ComtradeError::Io(e.to_string()))
            .and_then(|b| fvb_core::comtrade::parse_cff_progress(&b, enc, prog))
    } else {
        fvb_core::comtrade::load_progress(path, enc, prog).map(|o| (o, Default::default()))
    }
}

/// The progress dialog shows when the `.dat` of a `.cfg` or the `.cff` is
/// over 200 000 bytes, or the `.osc` over 20 000.
pub fn loading_shown(path: &std::path::Path) -> bool {
    let size = |p: &std::path::Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let lower = path.to_string_lossy().to_lowercase();
    if lower.ends_with(".cfg") {
        size(&path.with_extension("dat")).max(size(&path.with_extension("DAT"))) > 200_000
    } else if lower.ends_with(".cff") {
        size(path) > 200_000
    } else if lower.ends_with(".osc") {
        size(path) > 20_000
    } else {
        false
    }
}

/// ADC codes of a channel (`(raw − b) / a`, rounded).
fn adc_codes(c: &fvb_core::comtrade::AnalogChannel) -> Vec<f64> {
    c.samples.iter().map(|&v| if c.a.abs() > 1e-12 { ((v - c.b) / c.a).round() } else { v }).collect()
}

/// Mean over the largest whole number of periods from the start: the first
/// `n − n mod p` samples.
pub fn whole_period_mean(s: &[f64], p: usize) -> f64 {
    let count = s.len() - s.len() % p.max(1);
    if count == 0 {
        return 0.0;
    }
    s[..count].iter().sum::<f64>() / count as f64
}

/// A channel's values as the channel math reads them (`0x426020`): the ADC
/// code in ADC mode, minus its DC with «Удалить постоянную», negated with
/// «Инвертировать».
pub fn flag_series(c: &fvb_core::comtrade::AnalogChannel, v: &ChanView, (dc, dc_code): (f64, f64), k: f64) -> Vec<f64> {
    let base = if v.adc_counts { adc_codes(c) } else { c.samples.clone() };
    // `0x426020` subtracts k·`ch+0xf8` — in ADC mode too, where the mean is
    // of the codes and the code itself is not scaled (the units mix).
    let dc = if v.adc_counts { k * dc_code } else { dc };
    base.into_iter()
        .map(|x| {
            let x = if v.remove_dc { x - dc } else { x };
            if v.invert {
                -x
            } else {
                x
            }
        })
        .collect()
}

/// `ch+0xf0` (`0x428490`): the factor a channel's stored values are shown
/// with — primary / secondary when primary values are shown of a file
/// stored in secondary ones (`+0x89` set, `+0x88` clear), secondary /
/// primary in the converse case (a «P» record shown «Вторичные», the
/// default: `0x422460` clears `+0x89`), else 1.
pub fn display_k(ch: &fvb_core::comtrade::AnalogChannel, show_primary: bool) -> f64 {
    match (show_primary, ch.is_primary) {
        (true, false) if ch.secondary != 0.0 => ch.primary / ch.secondary,
        (false, true) if ch.primary != 0.0 => ch.secondary / ch.primary,
        _ => 1.0,
    }
}

/// A horizontal визир (`doc+0x2c8` / `+0x2e0`): shown, the analog channel
/// whose scale it is on (`None` = never placed), and its value in that
/// channel's units.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HVizir {
    pub shown: bool,
    pub chan: Option<usize>,
    pub value: f64,
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
    /// Vertical визир 1 (`doc+0x284`) — "cursor 1" of the diagrams and ОМП.
    pub main_cursor: usize,
    /// Vertical визир 2 (`doc+0x29c`) — "cursor 2".
    pub aux_cursor: usize,
    /// Vertical визир 3 (`doc+0x2b4`).
    pub v3_cursor: usize,
    /// Shown flags of В1 / В2 / В3; a new document shows none (`0x449170`).
    pub v_shown: [bool; 3],
    /// Horizontal визиры ГВ1 / ГВ2, each on one analog channel's scale.
    pub hviz: [HVizir; 2],
    pub chans: Vec<ChanView>,
    pub computed: Vec<Computed>,
    /// Text sections (HDR/INF/SET) of a record opened from a `.cff`, written
    /// back when it is saved as `.cff`.
    pub cff: fvb_core::comtrade::CffSections,
    /// (Iдиф, Iторм) channel pairs of the differential diagram (`doc+0x878`).
    pub dif_pairs: Vec<crate::diagram::ChanPair>,
    /// (R, X) channel pairs of the circle diagram (DIALOG 554, `doc+0x884`).
    pub rx_pairs: Vec<crate::diagram::ChanPair>,
    pub comments: Vec<Comment>,
    /// Level lines of the `.set` on a channel we do not draw them on (a
    /// digital or a missing one): the original keeps every `Level_%d` it
    /// reads (`0x43ff8a`) and saves it back (`0x470b20`), so ours are kept
    /// and written after the others (L9.89).
    pub kept_levels: Vec<fvb_core::recset::CommentRec>,
    /// The element under the pointer (`doc+0x854`), drawn selected: set by
    /// a pointer move over the plot, kept when the pointer leaves it.
    pub comment_hot: Option<usize>,
    /// Its tab's colour, an index into [`TAB_COLORS`], given when the tab
    /// is first laid out (`0x1006a29c`).
    pub tab_color: Option<usize>,
    /// A number of its own (its MDI child window), kept while others close.
    pub serial: u64,
    /// Selected channels (the original's per-channel `+0x100` flag): the
    /// target of the channel context menu and the per-channel commands.
    pub selection: BTreeSet<ChanKey>,
    /// Anchor of a Shift+click range selection (the original's `view+0x114`).
    pub sel_anchor: Option<ChanKey>,
    /// Next free channel-group id (Совместить каналы).
    pub next_group: u32,
    /// Comment placement tool (`doc+0x858`): 0 off, 1 момент времени,
    /// 2 линия уровня, 3 область (one-shot).
    pub comment_mode: u8,
    /// First visible sample (`view+0xfc`).
    pub first: usize,
    /// Horizontal scale `f` (`view+0xf0`, a power of two ≤ 1, 25·f px per
    /// sample); `None` until the view first lays out (initial = stored
    /// `View/XScale`, else twice the fit scale).
    pub xscale: Option<f64>,
    /// Vertical scroll offset of the lanes (`view+0xf8`, ≤ 0).
    pub vscroll: f32,
    /// «Разница между визирами» band (`doc+0x870`, on for a new document).
    pub diff_band: bool,
    /// Per-channel one-cycle RMS of the flagged values.
    pub rms: Vec<Vec<f64>>,
    /// The record as the channel math sees it: each analog channel's samples
    /// with its ADC / remove-DC / invert flags applied (`0x426020`).
    pub flagged: Oscillogram,
    /// Bumped whenever the derived series (flagged values, RMS, phasors)
    /// are rebuilt; keys the min / max cache.
    pub series_gen: u64,
    /// Per channel and display mode: the unscaled series' finite min / max
    /// and its plain min / max (the original keeps them at `+0xa8` /
    /// `+0xb0`, recomputed when the values change, not per paint).
    pub minmax_cache: MinMaxCache,
    /// Per channel the DC removed by «Удалить постоянную составляющую»: the
    /// mean over the whole periods from the record start of the values and of
    /// the ADC codes (`ch+0xf8`, `0x428490`).
    pub dc_mean: Vec<(f64, f64)>,
    /// The document's ОМП settings, last run and result flag (`doc+0x308`).
    pub omp: crate::omp_ui::OmpDoc,
    /// Pasted channels waiting for «Объединить» (bit 14) and their offset in
    /// seconds (`doc+0x6d8`).
    pub pending: Vec<ClipChan>,
    pub merge_offset: f64,
    /// Shown flag of each digital channel (bit 0; «Каналы» dialog column 1; 32845 / 32846 / 32847).
    pub digital_visible: Vec<bool>,
    /// The code page the record's names are currently decoded in (`doc+0x6e8`,
    /// 866 / 1251: file parameters «Кодировка»; see `recdlg`).
    pub codepage: Encoding,
    /// The record comment (`.hdr` text, 32805) and its code page.
    pub hdr: Option<String>,
    pub hdr_cp: Encoding,
    /// The record settings tree and the modified flag (`setfile`).
    pub set: crate::setfile::DocSet,
    /// This document's history (`doc+0x6ec` undo / `doc+0x700` redo deques,
    /// no depth limit).
    pub undo: Vec<DocSnapshot>,
    pub redo: Vec<DocSnapshot>,
    /// The print dialog's checkboxes (`view+0xd0`): Осциллограмма,
    /// Частотная, Векторная, Круговая, Дифференциальная; kept per document,
    /// also on Отмена, not persisted.
    pub print_ticks: [bool; 5],
}

impl Doc {
    pub fn new(osc: Oscillogram, name: String) -> Self {
        let analysis = Analysis::compute(&osc);
        let n = osc.samples_per_cycle().max(2);
        let corr = fvb_core::freqcorr::of(&osc);
        let rms: Vec<Vec<f64>> = osc
            .analog
            .iter()
            .enumerate()
            .map(|(i, c)| fvb_core::freqcorr::sliding_rms(corr, i, &c.samples, n))
            .collect();
        let dc_mean = osc.analog.iter().map(|c| (whole_period_mean(&c.samples, n), whole_period_mean(&adc_codes(c), n))).collect();
        let flagged = osc.clone();
        let chans = (0..osc.analog.len()).map(|i| ChanView::new(&osc.analog[i], i)).collect();
        let digital_visible = vec![true; osc.digital.len()];
        Doc {
            osc,
            analysis,
            name,
            path: None,
            main_cursor: 0,
            aux_cursor: 0,
            v3_cursor: 0,
            v_shown: [false; 3],
            hviz: [HVizir::default(); 2],
            chans,
            computed: Vec::new(),
            cff: Default::default(),
            dif_pairs: Vec::new(),
            rx_pairs: Vec::new(),
            comments: Vec::new(),
            kept_levels: Vec::new(),
            comment_hot: None,
            tab_color: None,
            serial: 0,
            selection: BTreeSet::new(),
            sel_anchor: None,
            next_group: 1,
            comment_mode: 0,
            first: 0,
            xscale: None,
            vscroll: 0.0,
            diff_band: true,
            rms,
            flagged,
            series_gen: 0,
            minmax_cache: Default::default(),
            dc_mean,
            digital_visible,
            codepage: Encoding::Cp1251,
            hdr: None,
            hdr_cp: Encoding::Cp1251,
            omp: Default::default(),
            pending: Vec::new(),
            merge_offset: 0.0,
            set: Default::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            print_ticks: [true, false, false, false, false],
        }
    }

    /// Rebuild everything derived from the channel values after a flag
    /// change (the original's full recompute, `FUN_00428490` per channel):
    /// the flagged record, its RMS and phasors, and the formula channels.
    pub fn recompute(&mut self, show_primary: bool) {
        let n = self.osc.samples_per_cycle().max(2);
        // The record may have been re-timed or re-scaled (file parameters,
        // coefficients): rebuild the copy and the whole-period DC from it.
        self.flagged = self.osc.clone();
        self.series_gen += 1;
        self.dc_mean = self.osc.analog.iter().map(|c| (whole_period_mean(&c.samples, n), whole_period_mean(&adc_codes(c), n))).collect();
        self.rms.resize(self.osc.analog.len(), Vec::new());
        for i in 0..self.osc.analog.len() {
            let k = display_k(&self.osc.analog[i], self.chans[i].primary);
            self.flagged.analog[i].samples = flag_series(&self.osc.analog[i], &self.chans[i], self.dc_mean[i], k);
            self.rms[i] = fvb_core::freqcorr::sliding_rms(fvb_core::freqcorr::of(&self.osc), i, &self.flagged.analog[i].samples, n);
        }
        // The correction reads the frequency channel as stored (L9.65).
        self.analysis = Analysis::compute_with(&self.flagged, fvb_core::freqcorr::of(&self.osc));
        // Formula channels are evaluated again in order on the new values
        // (merged and ОМП channels carry no formula to re-evaluate).
        let mut done: Vec<fvb_core::quick::ComputedChannel> = Vec::new();
        for c in &mut self.computed {
            if c.formula != "ОМП" && c.formula != "объединение" {
                let batch = [(c.name.clone(), c.formula.clone())];
                if let Ok(mut out) = fvb_core::quick::evaluate_batch_with(&self.flagged, fvb_core::freqcorr::of(&self.osc), &done, &batch, show_primary) {
                    if let Some(new) = out.pop() {
                        c.samples = new.samples;
                        c.phasors = new.phasors;
                    }
                }
            }
            done.push(c.as_core());
        }
    }

    /// The displayed sample value of analog channel `i` at index `k`, honouring
    /// invert / remove-DC.
    pub fn sample(&self, i: usize, k: usize) -> f64 {
        self.flagged.analog[i].samples.get(k).copied().unwrap_or(0.0)
    }

    /// The factor `ch+0xf0` every value the diagrams read carries: the
    /// sample accessor `0x426020` and the phasor cache `0x428490` apply it
    /// before any painter runs (an ADC code is taken as is).
    pub fn shown_k(&self, i: usize) -> f64 {
        if self.chans[i].adc_counts {
            1.0
        } else {
            display_k(&self.osc.analog[i], self.chans[i].primary)
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

    /// 32813 / 32814 / 139 (`0x45c120`): a never-placed vertical визир is
    /// placed at ¼ / ½ / ¾ of the plot width and shown, else its visibility
    /// flips. 32815 / 32816: a never-placed horizontal визир goes on the first
    /// analog lane (10 / 20 px from its top, resolved at the next layout),
    /// else it flips.
    pub fn toggle_vizir(&mut self, k: usize, width: f32, left: f32) {
        if k < 3 {
            let s = match k {
                0 => &mut self.main_cursor,
                1 => &mut self.aux_cursor,
                _ => &mut self.v3_cursor,
            };
            if !self.v_shown[k] && *s == 0 {
                let f = self.xscale.unwrap_or(1.0);
                let x = left + (width - left) * (k as f32 + 1.0) / 4.0;
                let n = self.osc.sample_count().max(1);
                *s = crate::oscgeom::sample_at(x, self.first, f, left).clamp(0, n as i64 - 1) as usize;
            }
            self.v_shown[k] = !self.v_shown[k];
        } else if k < 5 {
            let h = &mut self.hviz[k - 3];
            if h.chan.is_none() {
                h.chan = self.chans.iter().position(|c| c.visible);
                h.value = f64::NAN;
                h.shown = h.chan.is_some();
            } else {
                h.shown = !h.shown;
            }
        }
    }

    /// 32802 / 32803 (`0x45c4f0`): `f` ×2 (below 1.0) / ÷2 (not below the
    /// fit scale), anchored on В1 if shown, else В2, else В3; `first` is then
    /// clamped to the record.
    pub fn zoom_x(&mut self, zoom_in: bool, width: f32, left: f32) {
        use crate::oscgeom as g;
        let n = self.osc.sample_count();
        let fit = g::fit_scale(width, n);
        let f = self.xscale.unwrap_or(fit);
        let anchor = [self.main_cursor, self.aux_cursor, self.v3_cursor]
            .into_iter()
            .zip(self.v_shown)
            .find_map(|(s, on)| on.then_some(s));
        let (f, first) = if zoom_in { g::zoom_in(f, self.first, anchor) } else { g::zoom_out(f, fit, self.first, anchor) };
        let vis = g::visible_count(width, left, f);
        self.xscale = Some(f);
        self.first = first.min(g::max_first(n, vis));
    }

    /// 32817..32819 (`FUN_0042f4b0`): when not every analog channel has the
    /// row it is set on all of them, otherwise cleared on all.
    pub fn toggle_stats_row(&mut self, k: usize) {
        let get = |s: &StatRows| [s.minmax, s.rms, s.a1][k.min(2)];
        let all = self.chans.iter().map(|c| &c.stats).chain(self.computed.iter().map(|c| &c.stats)).all(get);
        let set = |s: &mut StatRows| match k {
            0 => s.minmax = !all,
            1 => s.rms = !all,
            _ => s.a1 = !all,
        };
        self.chans.iter_mut().for_each(|c| set(&mut c.stats));
        self.computed.iter_mut().for_each(|c| set(&mut c.stats));
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

/// An undo/redo entry: the record's data model (`doc+0x180 … 0x27f`,
/// assignment `0x4546d0`) — the channels with their samples, flags,
/// selection and pending state, the computed channels, times, rate and
/// header. Визиры, comments, the ОМП state, the merge offset and the
/// modified flag live outside it and are never restored.
#[derive(Clone)]
pub struct DocSnapshot {
    pub osc: Oscillogram,
    pub chans: Vec<ChanView>,
    pub computed: Vec<Computed>,
    pub pending: Vec<crate::merge_ui::ClipChan>,
    pub selection: BTreeSet<ChanKey>,
    pub digital_visible: Vec<bool>,
}

/// Commands dispatched from the ribbon, QAT, keybinds and context menus. The
/// numeric ids mirror the original's command ids where they are meaningful.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Open,          // Ctrl+O / QAT
    Save,          // 57603: onto the document's file, else Save As
    /// 57604 «Сохранить как...».
    SaveAs,        // Ctrl+S (57604)
    /// 32860 «Загрузка настроек...».
    LoadSettings,
    Print,         // 57607 Ctrl+P: the print dialog
    PrintPreview,  // 57609
    PageSetup,     // 57605
    ExportImage,   // our extension: a PNG of the window
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
    /// 137 «Быстрый анализ»: the modal sheet (`0x4167f0`), opened on the
    /// formula editor; enabled iff the record has analog channels.
    QuickAnalysis,
    /// 32809 / 32808 / 140 / 32811: show or hide a diagram pane.
    TogglePane(Pane),
    /// Bring a pane of the tab group to the front.
    ActivatePane(Pane),
    /// 32867 / 32868: every file channel to primary / secondary values.
    SetPrimaryValues(bool),
    /// 32823 / 32824 / 32852: toggle-all of a flag over the selection.
    ToggleFlag(crate::chansel::ChanFlag),
    /// 142: show or hide the parameters bar.
    ToggleParamBar,
    /// 32862 «Каналы»: DIALOG 463.
    ChannelsDialog,
    /// 32866 «Коэффициенты»: DIALOG 477.
    Coefficients,
    /// 32805 «Комментарии»: the record comment (.hdr) editor.
    HdrComment,
    SetValueMode(ValueMode),
    /// 155..157: toggle a comment placement mode (radio group with toggle-off).
    CommentMode(u8),
    /// 32806 «Анализ» (ОМП): clear a shown result, else run.
    OmpAnalysis,
    /// 32807 «Параметры» (ОМП): the modal parameters dialog.
    OmpParams,
    /// 32813 / 32814 / 139 / 32815 / 32816: show or hide В1 / В2 / В3 /
    /// ГВ1 / ГВ2 (index 0..5).
    ToggleVizir(usize),
    /// 32817 / 32818 / 32819: the Max/Min, СКЗ or A1 stats rows (0, 1, 2) on
    /// every analog channel, toggle-all.
    StatsRow(usize),
    /// 138 «Приблизить по вертикали»: arm the rectangle zoom (pressing again
    /// while armed keeps it armed).
    RectZoom,
    /// 141 «Вернуть к исходному размеру»: drop the selected channels' window
    /// zoom (with their groups).
    ResetZoom,
    /// 32802 / 32803: horizontal scale ×2 / ÷2 anchored on a визир.
    ZoomX(bool),
    /// 32800 / 32801: analog lane height ×1.3 / ÷1.3.
    ZoomY(bool),
    /// 32845 / 32846 / 32847: show all / hide all / hide unchanged discrete
    /// channels (analog visibility is the Каналы dialog's).
    Discrete(u8),
    SetSkin(Skin),
    /// Open a recent document («Последние документы»).
    OpenPath(std::path::PathBuf),
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
    /// Diagram windows' own state (zoom/pan, menus, dialogs).
    pub diag: crate::diagram_ui::DiagState,

    // documents
    pub docs: Vec<Doc>,
    pub active: Option<usize>,

    // ribbon / view
    pub tab: RibbonTab,
    pub view: View,
    pub panes: Panes,
    /// The diagram panes' docking (L9.92): the tab group and Частотная,
    /// a unit being dragged after it detached (with the pointer's grab
    /// offset in its rect), and the slid-out auto-hide areas.
    pub dock_group: crate::docking::Dockable,
    pub dock_freq: crate::docking::Dockable,
    pub dock_drag: Option<(crate::docking::Unit, egui::Vec2)>,
    pub dock_areas: [Option<egui::Rect>; 2],
    pub dock_strips: [Option<egui::Rect>; 2],
    pub skin: Skin,
    pub time_unit: TimeUnit,
    pub show_grid: bool,

    // formula editor
    /// «Добавить новую формулу» (DIALOG 359) while open.
    pub formula_dlg: Option<crate::formula_dlg::AddFormulaDlg>,

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
    /// The «Быстрый анализ» sheet is up.
    pub quick_open: bool,
    /// Pages created (shown) since the sheet opened ([`crate::quick_ui::PAGE_KEYS`] order).
    pub quick_shown: [bool; 6],
    /// The sheet's «Редактор формул» page.
    pub formula_page: crate::formula_page::FormulaPage,
    /// Quick-analysis forms (one per tab).
    pub quick: [crate::quick_ui::QuickForms; 5],
    /// ОМП result bar, parameters dialog and message box.
    pub omp_ui: crate::omp_ui::OmpUi,
    /// The settings-file prompts and sync (`setfile`).
    pub setfile: crate::setfile::SetUi,
    /// Persisted characteristic settings (only when enabled by `main`).
    pub store: crate::persist::Store,
    pub persist: bool,

    // dialogs / panels
    pub show_about: bool,
    /// Ribbon keytips (L9.90) and, per frame, the elements they sit on.
    pub keytips: crate::keytips::Keytips,
    pub kt_rects: Vec<(crate::keytips::Target, egui::Rect, bool)>,
    /// The application menu's bar id and button, to open / close it from
    /// the keyboard.
    pub kt_bar: Option<(egui::Id, egui::Rect, egui::Id)>,
    pub kt_open_menu: bool,
    pub show_file_params: bool,
    pub show_channel_map: bool,
    pub show_settings: bool,
    pub encoding: Encoding,
    pub explorer_dir: std::path::PathBuf,
    pub ribbon_minimized: bool,
    pub props_channel: Option<usize>,
    pub comment_edit: Option<CommentEdit>,
    /// Pending image-export target (filled once the screenshot arrives).
    pub want_screenshot: Option<std::path::PathBuf>,

    /// Channels copied for merging oscillograms (Объединение).
    /// The clipboard of «Mechanotronica FastView Clipboard Data».
    pub clip: crate::sysclip::SysClip,

    // undo / redo of per-document view edits

    /// «Приблизить по вертикали» (138): armed (`view+0xe4` mode 7) or a
    /// rectangle being dragged on an analog lane (mode 8).
    pub rect_zoom: RectZoom,
    /// The comment being created or dragged and its part (`view+0xe4` mode 9).
    pub comment_drag: Option<(usize, crate::comments::Part)>,
    /// «Изменить уровень» (33042): the level line being edited in place.
    pub level_edit: Option<(usize, String)>,
    /// The визир being dragged (`view+0xe4` modes 1–5).
    pub vizir_drag: Option<crate::lanes::VizDrag>,
    /// Analog lane height `View/YAnalogSize` (global, persisted).
    pub y_analog: f32,
    /// Last horizontal scale set by a zoom, `View/XScale` (persisted).
    pub xscale_pref: Option<f64>,
    /// Inline rename of an analog channel (double-click on its name badge).
    pub rename_edit: Option<(usize, String)>,
    /// A stats-panel drag waiting to start (MFC's OnBeginDrag loop): where
    /// the pointer was, when, and the channels.
    pub stats_drag_arm: Option<(egui::Pos2, f64, Vec<usize>)>,
    /// Width of the oscillogram view at the last layout (for «вся осциллограмма»).
    pub view_width: f32,
    /// Stats-panel width at the last layout (fitted to the text).
    pub view_left: f32,
    /// The oscillogram view's rect and its client part (without the scroll
    /// bars) as last drawn — the size of copy's picture.
    pub view_rects: Option<(egui::Rect, egui::Rect)>,
    /// The diagram windows (the canvas, without toolbar or property grid)
    /// as last drawn.
    pub pane_canvas: Vec<(Pane, egui::Rect)>,
    /// The pointer over the view's client area at the last frame it was
    /// there: a change is the original's WM_MOUSEMOVE.
    pub view_pointer: Option<egui::Pos2>,
    /// MFC's process-wide auto-colour state of the tabs.
    pub tab_auto: TabAuto,
    /// «Окна» (MFC's windows dialog), when open.
    pub windows_dlg: Option<crate::windows_dlg::WindowsDlg>,
    /// The serial of the next document.
    pub next_serial: u64,
    /// The circle / differential property grids as last drawn.
    pub pane_grid: Vec<egui::Rect>,
    /// Where the keyboard focus is, as far as copy (`0x405160`) cares.
    pub copy_focus: CopyFocus,
    /// Screen pixels per point, for pictures at the screen's resolution.
    pub ppp: f32,
    /// The merge bar (shown after a paste).
    pub merge: Option<crate::merge_ui::MergeBar>,
    /// A pending lane being dragged in time: start x and start offset.
    pub merge_drag: Option<(f32, f64)>,
    /// «Последние документы»: up to 10 files, most recent first.
    pub mru: Vec<std::path::PathBuf>,
    /// Records being parsed.
    pub loading: Vec<Loading>,
    /// «Проводник» (32880), the modal finder.
    pub finder: Option<crate::finder::Finder>,
    /// The window title last sent to the viewport.
    pub window_title: String,
    /// `-Dst <dir>`: the folder of the DST description files.
    pub dst_dir: Option<std::path::PathBuf>,
    /// The open oscillogram context menu, latched at the right-click.
    pub lane_menu: Option<crate::lanes::LaneMenu>,
    /// Bumped on every right-click so each menu gets a fresh popup area.
    pub lane_menu_serial: u64,
    /// A press that dismissed the menu is swallowed until the button is up.
    pub lane_menu_eat: bool,
    /// «Цвет канала» (33030) colour dialog, initialised from the first
    /// selected channel and applied to every selected channel on OK.
    pub color_dialog: Option<egui::Color32>,
    /// A dialog window was up last frame: the original's dialogs are all
    /// modal, so the main window takes no pointer or key input.
    pub modal_up: bool,
    /// Dialogs and bars of `chandlg` / `recdlg` (backlog 16, 18, 28, 29, 34, 39).
    pub dlg: crate::recdlg::DialogState,

    pub status: String,
    /// The last `status` shown as a toast and when it appeared (ctx time).
    pub toast: (String, f64),
    /// The single-instance listener (files handed over by later starts).
    pub instance: Option<crate::instance::Server>,
    /// Printing: page setup, the dialogs and the preview.
    pub print: crate::print_ui::PrintUi,
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
            diag: Default::default(),
            docs: Vec::new(),
            active: None,
            tab: RibbonTab::Home,
            view: View::Oscillogram,
            panes: Panes::default(),
            dock_group: crate::docking::Dockable::new(egui::vec2(500.0, 500.0)),
            dock_freq: crate::docking::Dockable::new(egui::vec2(200.0, 200.0)),
            dock_drag: None,
            dock_areas: [None, None],
            dock_strips: [None, None],
            skin: Skin::Blue,
            time_unit: TimeUnit::Milli,
            show_grid: true,
            formula_dlg: None,
            vec_radians: false,
            vec_direction_only: false,
            vec_show_values: true,
            vec_angle_ref: None,
            circle_readout: 0,
            circle_between_cursors: false,
            freq_periods: 1,
            freq_harmonics: 10,
            // `Frequency Chart/ShowPersents` 0 = percent (`0x46f470`).
            freq_percent: true,
            freq_use_rms: false,
            analysis_tab: AnalysisTab::Formula,
            quick_open: false,
            quick_shown: [false; 6],
            formula_page: Default::default(),
            quick: Default::default(),
            omp_ui: Default::default(),
            setfile: Default::default(),
            store: Default::default(),
            persist: false,
            show_about: false,
            keytips: Default::default(),
            kt_rects: Vec::new(),
            kt_bar: None,
            kt_open_menu: false,
            show_file_params: false,
            show_channel_map: false,
            show_settings: false,
            encoding: Encoding::Cp1251,
            explorer_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            ribbon_minimized: false,
            props_channel: None,
            comment_edit: None,
            want_screenshot: None,
            clip: Default::default(),
            rect_zoom: RectZoom::Off,
            comment_drag: None,
            level_edit: None,
            vizir_drag: None,
            y_analog: crate::oscgeom::Y_ANALOG_DEFAULT,
            xscale_pref: None,
            rename_edit: None,
            stats_drag_arm: None,
            view_width: 1000.0,
            view_left: crate::lanes::LEFT_W_DEFAULT,
            view_rects: None,
            pane_canvas: Vec::new(),
            pane_grid: Vec::new(),
            view_pointer: None,
            tab_auto: TabAuto::default(),
            windows_dlg: None,
            next_serial: 1,
            copy_focus: CopyFocus::View,
            ppp: 1.0,
            window_title: String::new(),
            finder: None,
            loading: Vec::new(),
            mru: Vec::new(),
            merge: None,
            merge_drag: None,
            dst_dir: None,
            lane_menu: None,
            lane_menu_serial: 0,
            lane_menu_eat: false,
            color_dialog: None,
            modal_up: false,
            dlg: Default::default(),
            status: "Готово".to_string(),
            toast: ("Готово".to_string(), f64::NEG_INFINITY),
            instance: None,
            print: Default::default(),
        };
        app.ensure_block_state();

        // Test/demo hooks (used for headless screenshots).
        if std::env::var("FV_DEMO").is_ok() {
            if let Ok(osc) = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT) {
                app.add_doc(Doc::new(osc, "demo_fault".into()));
            }
        }
        app.view = View::Oscillogram;
        if std::env::var("FV_VIEW").as_deref() == Ok("analysis") {
            app.dispatch(Command::QuickAnalysis);
        }
        if std::env::var("FV_VIEW").as_deref() == Ok("omp") {
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

    pub fn add_doc(&mut self, mut doc: Doc) {
        doc.serial = self.next_serial;
        self.next_serial += 1;
        let name = doc.name.clone();
        let n = doc.osc.sample_count();
        self.docs.push(doc);
        self.active = Some(self.docs.len() - 1);
        self.view = View::Oscillogram;
        self.status = format!("Загружена осциллограмма: {name} ({n} отсчётов)");
    }

    /// Close document `i` without asking.
    pub fn close_doc_now(&mut self, i: usize) {
        if i < self.docs.len() {
            self.docs.remove(i);
            // Indices shift: re-bind the ОМП bar and the settings sync on
            // the next frame.
            self.omp_ui.bar_doc = None;
            self.setfile.sync = None;
            self.active = if self.docs.is_empty() {
                None
            } else {
                Some(i.min(self.docs.len() - 1))
            };
        }
    }

    // ---------- command dispatch ----------
    pub fn dispatch(&mut self, cmd: Command) {
        let mark = crate::setfile::marks_modified(&cmd).then_some(self.active).flatten();
        self.dispatch_inner(cmd);
        if let Some(d) = mark.and_then(|i| self.docs.get_mut(i)) {
            d.set.modified = true;
        }
    }

    fn dispatch_inner(&mut self, cmd: Command) {
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
            Command::ToggleExplorer => self.open_finder(),
            Command::ToggleRibbonMin => self.ribbon_minimized = !self.ribbon_minimized,
            Command::Save => {
                self.save_active();
            }
            Command::SaveAs => {
                self.save_active_as();
            }
            Command::LoadSettings => self.load_settings_command(),
            Command::Print => self.print_open(),
            Command::PrintPreview => self.print_preview_open(),
            Command::PageSetup => self.page_setup_open(),
            Command::ExportImage => self.request_image_export(),
            Command::Copy => self.copy_channels_for_merge(),
            Command::Paste => {
                self.paste_channels_for_merge();
            }
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::QuickAnalysis => {
                if self.active_doc().is_some_and(|d| !d.osc.analog.is_empty()) {
                    self.analysis_tab = AnalysisTab::Formula;
                    self.quick_open = true;
                    self.quick_shown = [false; 6];
                    self.formula_page_open();
                    self.quick_defaults();
                    self.quick_show_page();
                }
            }
            Command::OmpAnalysis => self.omp_toggle(),
            Command::OmpParams => self.omp_open_params(),
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
            Command::SetPrimaryValues(p) => self.set_primary_values(p),
            Command::ToggleParamBar => self.dlg.param_bar = !self.dlg.param_bar,
            Command::ChannelsDialog => self.open_channels_dialog(),
            Command::Coefficients => self.open_coef_dialog(),
            Command::HdrComment => self.open_hdr_comment(),
            Command::ToggleFlag(f) => {
                if let Some(d) = self.active_doc_mut() {
                    d.toggle_flag_selected(f);
                }
                if matches!(f, crate::chansel::ChanFlag::Invert | crate::chansel::ChanFlag::RemoveDc | crate::chansel::ChanFlag::Adc) {
                    self.recompute_active();
                }
            }
            Command::SetValueMode(m) => {
                if let Some(d) = self.active_doc_mut() {
                    d.set_mode_selected(m);
                }
            }
            Command::ToggleVizir(k) => {
                let (w, left) = (self.view_width, self.view_left);
                if let Some(d) = self.active_doc_mut() {
                    d.toggle_vizir(k, w, left);
                }
            }
            Command::RectZoom => {
                if let Some(d) = self.active_doc_mut() {
                    d.comment_mode = 0;
                }
                if self.rect_zoom == RectZoom::Off && self.active.is_some() {
                    self.rect_zoom = RectZoom::Armed;
                }
            }
            Command::ResetZoom => {
                if let Some(d) = self.active_doc_mut() {
                    d.reset_zoom_selected();
                }
            }
            Command::CommentMode(k) => {
                self.rect_zoom = RectZoom::Off;
                if let Some(d) = self.active_doc_mut() {
                    if d.comments_allowed() {
                        d.comment_mode = if d.comment_mode == k { 0 } else { k };
                    }
                }
            }
            Command::Discrete(k) => {
                if let Some(d) = self.active_doc_mut() {
                    for (i, ch) in d.osc.digital.iter().enumerate() {
                        let unchanged = ch.samples.iter().all(|&v| v == ch.samples.first().copied().unwrap_or(false));
                        match k {
                            0 => d.digital_visible[i] = true,
                            1 => d.digital_visible[i] = false,
                            _ if unchanged => d.digital_visible[i] = false,
                            _ => {}
                        }
                        if !d.digital_visible[i] {
                            d.selection.remove(&ChanKey::Digital(i));
                        }
                    }
                }
            }
            Command::SetSkin(s) => {
                self.skin = s;
                // `ApplicationLook`: the menu-200 id, 210..218.
                self.store.view.insert("ApplicationLook".into(), s.code().to_string());
                if self.persist {
                    self.store.save();
                }
            }
            Command::OpenPath(p) => self.load_comtrade_path(&p),
            Command::StatsRow(k) => {
                if let Some(d) = self.active_doc_mut() {
                    d.toggle_stats_row(k);
                }
            }
            Command::ZoomX(zin) => {
                let (w, left) = (self.view_width, self.view_left);
                if let Some(d) = self.active_doc_mut() {
                    d.zoom_x(zin, w, left);
                }
                self.persist_view();
            }
            Command::ZoomY(grow) => {
                // `View/YAnalogSize` follows through `track_settings`.
                self.y_analog = crate::oscgeom::lane_zoom(self.y_analog, grow);
            }
            Command::CloseDoc(i) => self.request_close(i),
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
            // Ctrl+S is 57604 «Сохранить как» in the original's table.
            (egui::Key::S, Command::SaveAs),
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
    /// Operate colour of characteristic `i` (`0x43a4a0` palette, mod 9).
    pub fn palette(i: usize) -> egui::Color32 {
        let (r, g, b) = crate::diagram::CHAR_PALETTE[i % 9][0];
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

    /// The full recompute after a value-changing command (`0x42f4b0` tail):
    /// the active record's derived data and, when its result is shown, ОМП.
    pub fn recompute_active(&mut self) {
        if let Some(ai) = self.active {
            self.recompute_doc(ai);
        }
    }

    /// Save the view preferences (`View`: XScale, YAnalogSize, RulerState,
    /// Sells), as the original does after every zoom / ruler / grid change.
    /// `View/XScale` of the active document (`0x45c4f0` writes it into the
    /// settings tree); the other `View` keys follow through
    /// `track_settings`.
    pub fn persist_view(&mut self) {
        if let Some(d) = self.active_doc_mut() {
            if let Some(f) = d.xscale {
                d.set.tree.section_mut("View").set_dbl("XScale", f);
            }
        }
    }

    /// Load persisted characteristic settings and apply them (called by `main`).
    pub fn with_persistence(mut self) -> Self {
        self.store = crate::persist::Store::load();
        self.persist = true;
        self.clip = crate::sysclip::SysClip::system();
        self.print.setup = crate::printout::PageSetup::load(&self.store.view);
        // `View` (XScale, YAnalogSize, RulerState, Sells) is per document,
        // in its settings file (`setfile`).
        let v = &self.store.view;
        if let Some(s) = v.get("ApplicationLook").and_then(|x| x.parse().ok()).and_then(Skin::from_code) {
            self.skin = s;
        }
        self.mru = (0..10).filter_map(|k| v.get(&format!("MRU/{k}")).map(std::path::PathBuf::from)).collect();
        self.load_program_settings();
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

    /// Open a record as a new document. Parsing runs on a worker thread
    /// (`LoadingDlg`, DIALOG 492); the progress dialog shows only for large
    /// files and «Отмена» abandons the open.
    pub fn load_comtrade_path(&mut self, path: &std::path::Path) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (p, enc) = (path.to_path_buf(), self.encoding);
        let progress = std::sync::Arc::new(fvb_core::comtrade::Progress::default());
        let prog = progress.clone();
        std::thread::spawn(move || {
            let r = parse_file(&p, enc, Some(&prog));
            // A cancelled open is abandoned silently (`0x44f391`).
            if !prog.cancelled() {
                let _ = tx.send(r);
            }
        });
        self.loading.push(Loading { path: path.to_path_buf(), big: loading_shown(path), rx, progress, shown: (0, f64::NEG_INFINITY) });
    }

    /// Collect finished loads (called every frame).
    pub fn poll_loading(&mut self) {
        let mut done = Vec::new();
        self.loading.retain(|l| match l.rx.try_recv() {
            Ok(r) => {
                done.push((l.path.clone(), r));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for (path, r) in done {
            self.finish_load(&path, r);
        }
    }

    fn finish_load(&mut self, path: &std::path::Path, loaded: LoadResult) {
        match loaded {
            Ok((osc, cff)) => {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.add_doc(Doc::new(osc, name));
                let enc = self.encoding;
                if let Some(d) = self.active_doc_mut() {
                    d.path = Some(path.to_path_buf());
                    d.cff = cff;
                    d.init_record_text(enc);

                    // Names may have been re-decoded; no flag is set yet.
                    d.flagged = d.osc.clone();
                    d.series_gen += 1;
                }
                if let Some(di) = self.active {
                    self.load_record_settings(di);
                }
                self.remember_recent(path);
            }
            // The original shows "<file> - <message>".
            Err(e) => self.status = format!("{} - {e}", path.display()),
        }
    }

    /// Put `path` first in «Последние документы» (10 entries) and save it.
    fn remember_recent(&mut self, path: &std::path::Path) {
        self.mru.retain(|p| p != path);
        self.mru.insert(0, path.to_path_buf());
        self.mru.truncate(10);
        for (k, p) in self.mru.iter().enumerate() {
            self.store.view.insert(format!("MRU/{k}"), p.to_string_lossy().into_owned());
        }
        if self.persist {
            self.store.save();
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
    pub fn snapshot_of_doc(d: &Doc) -> DocSnapshot {
        DocSnapshot {
            osc: d.osc.clone(),
            chans: d.chans.clone(),
            computed: d.computed.clone(),
            pending: d.pending.clone(),
            selection: d.selection.clone(),
            digital_visible: d.digital_visible.clone(),
        }
    }

    /// Assign a snapshot over the live record. The original then only
    /// repaints (`0x45bf0f`); our derived caches (flagged values, RMS,
    /// phasors, pending lanes) are rebuilt from the restored record.
    fn restore(d: &mut Doc, s: DocSnapshot) {
        d.osc = s.osc;
        d.chans = s.chans;
        d.computed = s.computed;
        d.pending = s.pending;
        d.selection = s.selection;
        d.digital_visible = s.digital_visible;
        d.sel_anchor = None;
        let last = d.osc.time.len().saturating_sub(1);
        for k in [&mut d.main_cursor, &mut d.aux_cursor, &mut d.v3_cursor, &mut d.first] {
            *k = (*k).min(last);
        }
        d.recompute_signals();
        d.refresh_pending();
    }

    /// Push the document's record (`0x454530`), clearing its redo history.
    pub fn push_undo(d: &mut Doc, snap: DocSnapshot) {
        d.undo.push(snap);
        d.redo.clear();
    }

    /// Record the active document's state for undo, clearing its redo stack.
    pub fn snapshot_undo(&mut self) {
        if let Some(d) = self.active_doc_mut() {
            let snap = Self::snapshot_of_doc(d);
            Self::push_undo(d, snap);
        }
    }

    /// 57643 (`0x454640`): the current record goes to redo, the top of undo
    /// is restored. No modified mark, recompute of formulas or ОМП run.
    pub fn undo(&mut self) {
        let Some(d) = self.active_doc_mut() else { return };
        let Some(snap) = d.undo.pop() else { return };
        let cur = Self::snapshot_of_doc(d);
        d.redo.push(cur);
        Self::restore(d, snap);
    }

    /// 57644 (`0x4548d0`), the mirror of [`App::undo`].
    pub fn redo(&mut self) {
        let Some(d) = self.active_doc_mut() else { return };
        let Some(snap) = d.redo.pop() else { return };
        let cur = Self::snapshot_of_doc(d);
        d.undo.push(cur);
        Self::restore(d, snap);
    }

    /// Append computed channels to the active record, evaluated in order in
    /// channel mode (`fvb_core::quick::evaluate_batch`); all or nothing.
    /// Returns the names given to them.
    pub fn append_computed(&mut self, batch: &[(String, String)]) -> Option<Vec<String>> {
        let show_primary = self.active_doc().is_some_and(Doc::primary_values);
        let base = self.docs.len() + 7;
        let Some(d) = self.active_doc() else {
            self.status = "Сначала загрузите осциллограмму.".into();
            return None;
        };
        let existing: Vec<_> = d.computed.iter().map(Computed::as_core).collect();
        match fvb_core::quick::evaluate_batch_with(&d.flagged, fvb_core::freqcorr::of(&d.osc), &existing, batch, show_primary) {
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
                d.channels_changed();
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
        // Map generated names to the names actually given; a name generated
        // twice is its first line's (the original uniq's the flag and pair
        // names against the state before the batch, `0x41699d`).
        let mut given: HashMap<&str, &str> = HashMap::new();
        for (n, r) in g.channels.iter().map(|(n, _)| n.as_str()).zip(names.iter().map(String::as_str)) {
            given.entry(n).or_insert(r);
        }
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
                d.dif_pairs.push(crate::diagram::ChanPair::new(real(dif), real(tor)));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// L9.65: a record with a «Гц» channel opens corrected; «Не
    /// корректировать» recomputes with the fixed window; the next channel
    /// change turns the correction back on.
    #[test]
    fn frequency_correction_on_load_off_and_back() {
        let cfg = DEMO_CFG.replacen("2,Ub,B,,V,", "2,Ub,B,,Гц,", 1);
        let osc = fvb_core::comtrade::parse(&cfg, DEMO_DAT).unwrap();
        let mut d = Doc::new(osc, "f".into());
        assert_eq!(d.osc.freq_sel, 1);
        let k = 300;
        let on = (d.analysis.phasors[3][k], d.rms_at(3, k));
        d.osc.freq_sel = 0;
        d.recompute_signals();
        let n = d.osc.samples_per_cycle();
        assert_eq!(d.analysis.phasors[3][k], fvb_core::signal::phasor_at(&d.flagged.analog[3].samples, k, n));
        assert_ne!(d.analysis.phasors[3][k], on.0);
        d.channels_changed();
        assert_eq!(d.osc.freq_sel, 1);
        assert_eq!((d.analysis.phasors[3][k], d.rms_at(3, k)), on);
    }

    /// A formula channel in Hz is a frequency candidate after the file's
    /// (`0x42ea36`, `0x42ebd0`): adding one to a record without any turns
    /// the correction on with it.
    #[test]
    fn a_formula_channel_in_hz_is_a_frequency_candidate() {
        let osc = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        let mut d = Doc::new(osc, "f".into());
        assert_eq!(d.osc.freq_sel, 0);
        let n = d.osc.sample_count();
        let mut c = Computed::new("F".into(), "Frequency: ()".into(), vec![49.0; n], egui::Color32::RED);
        c.unit = fvb_core::expr::formula_unit("Frequency: ()", &fvb_core::channels::OscChannels::new(&d.osc, &[], false));
        assert_eq!(c.unit, "Hz");
        d.computed.push(c);
        d.channels_changed();
        assert_eq!(d.osc.freq_sel, 1);
        assert_eq!(fvb_core::freqcorr::candidate_names(&d.osc), ["F"]);
        let corr = fvb_core::freqcorr::of(&d.osc).expect("corrected by the formula channel");
        assert!(corr.for_chan(0).is_some(), "no file channel is the frequency channel");
        // Removing it leaves the selection past the list: no correction.
        d.computed.clear();
        d.channels_changed();
        assert!(fvb_core::freqcorr::of(&d.osc).is_none());
    }

    /// `0x1006a29c` with both statics at 0: the first tab is the yellow
    /// (index 1, as the help screenshots), the next ones step on; a tab
    /// opened after a close takes its position's colour unless the last
    /// one given or its neighbour has it.
    #[test]
    fn tab_auto_colours_follow_mfc() {
        let mut t = TabAuto::default();
        assert_eq!([t.assign(0, None), t.assign(1, Some(1)), t.assign(2, Some(2))], [1, 2, 3]);
        assert_eq!(TAB_COLORS[1], (255, 220, 120));
        // Tabs [1, 3] after closing the middle one: position 2 is free.
        assert_eq!(t.assign(2, Some(3)), 2);
        // Tabs [1] alone: position 1 clashes with its neighbour → counter.
        let mut t = TabAuto { last: 2, counter: 3 };
        assert_eq!(t.assign(1, Some(1)), 4);
        // Wraps after sixteen.
        let mut t = TabAuto { last: 0, counter: 15 };
        assert_eq!(t.assign(16, None), 0);
    }

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
        assert!(has(egui::Key::S, Command::SaveAs));
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
    fn quick_analysis_opens_the_sheet_on_the_formula_editor() {
        let mut app = demo_app();
        app.tab = RibbonTab::Omp;
        app.analysis_tab = AnalysisTab::Zero;
        app.dispatch(Command::QuickAnalysis);
        assert!(app.quick_open);
        assert_eq!(app.analysis_tab, AnalysisTab::Formula, "always the formula editor first");
        assert_eq!(app.view, View::Oscillogram, "the view stays");
        assert_eq!(app.tab, RibbonTab::Omp);
        let mut empty = App::new();
        empty.dispatch(Command::QuickAnalysis);
        assert!(!empty.quick_open, "no record, no sheet");
        // «Рассчитать» on a page generates its channels only; on the formula
        // editor nothing.
        app.quick_calc();
        assert!(!app.quick_open);
        assert!(app.active_doc().unwrap().computed.is_empty());
        app.dispatch(Command::QuickAnalysis);
        app.analysis_tab = AnalysisTab::Symmetric;
        app.quick_show_page();
        app.quick_calc();
        let d = app.active_doc().unwrap();
        assert!(!d.computed.is_empty());
        // The pages shown got their `.set` categories; the shown generator
        // page saved its fields.
        assert!(d.set.tree.section("FormulaEditorPage").is_some());
        assert!(d.set.tree.section("SymmetricItemsPage").unwrap().child("IA").is_some());
        assert!(d.set.tree.section("NullSequencePage").is_none(), "never shown, never created");
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
    fn invert_and_dc_reach_every_consumer_and_are_not_undoable() {
        use crate::chansel::ChanFlag;
        let mut app = demo_app();
        let (i, k) = (0usize, 300usize);
        let name = app.active_doc().unwrap().osc.analog[i].id.clone();
        assert!(app.append_computed(&[("x2".into(), format!("2*\"{name}\""))]).is_some());
        let d = app.active_doc().unwrap();
        let (base, ph, comp) = (d.sample(i, k), d.analysis.phasors[i][k], d.computed[0].samples[k]);
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(i));
        let undo = app.active_doc().unwrap().undo.len();
        app.dispatch(Command::ToggleFlag(ChanFlag::Invert));
        let d = app.active_doc().unwrap();
        assert!((d.sample(i, k) + base).abs() < 1e-9, "the displayed value is negated");
        assert!((d.analysis.phasors[i][k] + ph).norm() < 1e-9, "so is the phasor the diagrams read");
        assert!((d.computed[0].samples[k] + comp).abs() < 1e-9, "formula channels are re-evaluated");
        assert_eq!(app.active_doc().unwrap().undo.len(), undo, "invert is not undoable in the original");
        app.dispatch(Command::ToggleFlag(ChanFlag::Invert));
        app.dispatch(Command::ToggleFlag(ChanFlag::RemoveDc));
        let d = app.active_doc().unwrap();
        let p = d.osc.samples_per_cycle();
        let mean = whole_period_mean(&d.osc.analog[i].samples, p);
        assert!((d.sample(i, k) - (base - mean)).abs() < 1e-9, "one DC constant over whole periods");
    }

    #[test]
    fn whole_period_mean_uses_only_complete_periods() {
        assert_eq!(whole_period_mean(&[1.0, 3.0, 1.0, 3.0, 100.0], 2), 2.0);
        assert_eq!(whole_period_mean(&[5.0], 2), 0.0);
    }

    #[test]
    fn discrete_show_hide_and_hide_unchanged() {
        let mut app = demo_app();
        let n = app.active_doc().unwrap().osc.digital.len();
        assert!(n > 0, "the demo record has discrete channels");
        app.dispatch(Command::Discrete(1));
        assert!(app.active_doc().unwrap().digital_visible.iter().all(|v| !v));
        app.dispatch(Command::Discrete(0));
        assert!(app.active_doc().unwrap().digital_visible.iter().all(|&v| v));
        app.dispatch(Command::Discrete(2));
        let d = app.active_doc().unwrap();
        for (ch, vis) in d.osc.digital.iter().zip(&d.digital_visible) {
            let changes = ch.samples.iter().any(|&v| v != ch.samples[0]);
            assert_eq!(*vis, changes, "{}", ch.id);
        }
    }

    #[test]
    fn copy_paste_merges_channels() {
        let mut app = demo_app();
        app.dispatch(Command::Copy);
        assert!(!app.clip.available(), "nothing selected, nothing copied");
        app.active_doc_mut().unwrap().selection.extend([ChanKey::Analog(0), ChanKey::Analog(1)]);
        app.dispatch(Command::Copy);
        let rec = fvb_core::clipdata::ClipRecord::parse(&app.clip.get().unwrap()).unwrap();
        assert_eq!(rec.channels.len(), 2, "copy takes the selected channels");
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
        assert_eq!(app.omp_ui.message.as_deref(), Some("Данные для определения места повреждения не введены."));
        assert!(app.active_doc().unwrap().omp.run.is_none());
    }

    #[test]
    fn omp_run_sets_window_cursors_and_channels() {
        let mut app = demo_app();
        let st = &mut app.active_doc_mut().unwrap().omp.params;
        st.method = fvb_core::omp_params::GROUNDED;
        let st = &mut st.stations[0];
        st.inom = "100".into();
        for (f, n) in [(&mut st.ia, "Ia"), (&mut st.ib, "Ib"), (&mut st.ic, "Ic"), (&mut st.ua, "Ua"), (&mut st.ub, "Ub"), (&mut st.uc, "Uc")] {
            *f = n.into();
        }
        st.sections.push(fvb_core::omp_params::SectionParams {
            length: "50".into(),
            r1: "0.1".into(),
            x1: "0.4".into(),
            r0: "0.3".into(),
            x0: "1.2".into(),
            ..Default::default()
        });
        app.run_omp();
        let d = app.active_doc().unwrap();
        let p = d.osc.samples_per_cycle();
        assert_eq!(d.aux_cursor, d.main_cursor + p - 1, "В1 at the start, В2 one period later");
        assert!(d.v_shown[0] && d.v_shown[1]);
        assert!(d.computed.iter().any(|c| c.name == "Lкз" && c.unit == "m"));
        for name in ["КЗ фазы А", "КЗ фазы B", "КЗ фазы C"] {
            assert!(d.osc.digital.iter().any(|c| c.id == name), "{name}");
        }
        let r = app.omp_result().unwrap();
        assert_eq!(app.omp_ui.bar.as_deref(), Some(r.text.as_str()));
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

    /// `ch+0xf0` (`0x428490`, L9.57): a «P» record shown «Вторичные» (the
    /// default) is scaled by secondary / primary, an «S» record shown
    /// «Первичные» by primary / secondary; ADC + remove-DC subtracts k·mean
    /// of the codes from the unscaled code (`0x426020`).
    #[test]
    fn display_factor_both_ways_and_adc_dc() {
        let app = demo_app();
        let d = app.active_doc().unwrap();
        let mut ch = d.osc.analog[0].clone();
        let view = d.chans[0].clone();
        (ch.primary, ch.secondary, ch.is_primary) = (2000.0, 1.0, true);
        assert_eq!(display_k(&ch, false), 1.0 / 2000.0);
        assert_eq!(display_k(&ch, true), 1.0);
        ch.is_primary = false;
        assert_eq!(display_k(&ch, true), 2000.0);
        assert_eq!(display_k(&ch, false), 1.0);
        ch.samples = vec![1.0, 3.0];
        let v = ChanView { adc_counts: false, remove_dc: true, invert: false, ..view.clone() };
        assert_eq!(flag_series(&ch, &v, (2.0, 7.0), 4.0), [-1.0, 1.0], "the k of the raw values is applied later");
        let v = ChanView { adc_counts: true, remove_dc: true, invert: false, ..view };
        let codes = adc_codes(&ch);
        assert_eq!(flag_series(&ch, &v, (2.0, 7.0), 4.0), [codes[0] - 28.0, codes[1] - 28.0], "code − k·mean code");
    }

    /// «Редактор формул» (L9.56): deleting a row removes the channel at
    /// once, a computed channel's own name is refused (473), «Рассчитать»
    /// updates in place and inserts new names first, «Отмена» undoes nothing.
    #[test]
    fn formula_page_edits_the_record_list() {
        let mut app = demo_app();
        app.append_computed(&[("X".into(), "Ua*2".into()), ("Y".into(), "Ub*2".into())]);
        let names = |app: &App| app.active_doc().unwrap().computed.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        app.dispatch(Command::QuickAnalysis);
        assert_eq!(app.formula_page.rows.len(), 2);
        app.formula_page.selected = Some(0);
        app.fp_delete();
        assert_eq!(names(&app), ["Y"], "X goes at once");
        app.formula_page.edit = Some((0, 0, "Y".into()));
        app.fp_commit_edit();
        assert_eq!(app.formula_page.alert, Some(fvb_core::addformula::MSG_NAME_TAKEN), "its own name is a record channel");
        app.formula_page.alert = None;
        app.formula_page.rows[0].1 = "Ub*3".into();
        app.fp_add_row("Z".into(), "Uc*2".into());
        let y_color = app.active_doc().unwrap().computed[0].color;
        app.quick_calc();
        let d = app.active_doc().unwrap();
        assert_eq!(names(&app), ["Z", "Y"], "the new channel after the analog ones");
        assert_eq!(d.computed[1].formula, "Ub*3");
        assert_eq!(d.computed[1].color, y_color, "updated in place");
        // «Отмена» keeps a deletion.
        app.dispatch(Command::QuickAnalysis);
        app.formula_page.selected = Some(0);
        app.fp_delete();
        app.quick_open = false;
        assert_eq!(names(&app), ["Y"]);
    }

    #[test]
    fn quick_zero_tab_generates_flagged_channels() {
        let mut app = demo_app();
        app.analysis_tab = AnalysisTab::Zero;
        let q = &mut app.quick[crate::quick_ui::page_index(AnalysisTab::Zero).unwrap()];
        q.ia = "Ia".into();
        q.ib = "Ib".into();
        q.ic = "Ic".into();
        q.ua = "Ua".into();
        q.ub = "Ub".into();
        q.uc = "Uc".into();
        let g = q.generate(AnalysisTab::Zero, &[]);
        app.apply_quick(&g);
        let d = app.active_doc().unwrap();
        let get = |n: &str| d.computed.iter().find(|c| c.name == n).unwrap();
        assert!(get("3I0").on_vector && get("3U0").on_vector && get("S0").on_circle);
        assert_eq!(get("S0").unit, "VA");
        assert_eq!(app.vec_angle_ref.as_deref(), Some("3U0"));
        assert_eq!(app.circle_readout, 1);
        // Applying again renames and toggles the new channels' flags.
        let g = app.quick[crate::quick_ui::page_index(AnalysisTab::Zero).unwrap()].generate(AnalysisTab::Zero, &[]);
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

    #[test]
    fn cursor_status_absolute_time() {
        let mut app = demo_app();
        app.time_unit = TimeUnit::Absolute;
        let d = app.active_doc_mut().unwrap();
        d.osc.stamps.start_time = "12:59:59.500000".into();
        let (t0, k) = (d.osc.time[0], 10);
        let dt = d.osc.time[k] - t0;
        d.main_cursor = k;
        d.aux_cursor = 0;
        d.v_shown = [true, true, false];
        let s = app.cursor_status();
        assert_eq!(s[0], format!("Верт. визир 1: {}", crate::oscgeom::time_of_day_text(46_799.5 + dt)));
        assert_eq!(s[1], "Верт. визир 2: 12:59:59.500");
        assert_eq!(s[2], format!("Разница. верт. визиров: {}", crate::oscgeom::status_abs_diff(dt)));
    }
}
