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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// The pane last activated (a tab clicked, a pane shown).
    pub active: Pane,
    /// When each pane was last activated: a tabbed group shows the most
    /// recent of its visible tabs.
    pub stamp: [u32; 4],
}

impl Default for Panes {
    fn default() -> Self {
        Panes { vector: true, circle: true, differential: true, frequency: true, active: Pane::Vector, stamp: [4, 3, 2, 1] }
    }
}

impl Panes {
    #[cfg(test)]
    pub const GROUP: [Pane; 3] = [Pane::Vector, Pane::Circle, Pane::Differential];

    pub fn visible(&self, p: Pane) -> bool {
        match p {
            Pane::Vector => self.vector,
            Pane::Circle => self.circle,
            Pane::Differential => self.differential,
            Pane::Frequency => self.frequency,
        }
    }

    pub fn set(&mut self, p: Pane, v: bool) {
        match p {
            Pane::Vector => self.vector = v,
            Pane::Circle => self.circle = v,
            Pane::Differential => self.differential = v,
            Pane::Frequency => self.frequency = v,
        }
    }

    pub fn activate(&mut self, p: Pane) {
        self.active = p;
        self.stamp[p as usize] = self.stamp.iter().max().copied().unwrap_or(0) + 1;
    }

    /// The front tab among `members`: the most recently activated visible one.
    pub fn front(&self, members: impl IntoIterator<Item = Pane>) -> Option<Pane> {
        members.into_iter().filter(|&p| self.visible(p)).max_by_key(|&p| self.stamp[p as usize])
    }

    /// The front tab of the Векторная | Круговая | Дифференциальная panes.
    #[cfg(test)]
    pub fn shown_tab(&self) -> Option<Pane> {
        self.front(Self::GROUP)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ValueMode {
    /// Мгновенные значения — the raw sample at the cursor.
    #[default]
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
    /// A merged channel pasted at the analog block's end (`None`) or in
    /// front of the formula channel named, with its lane's creation
    /// sequence: «Объединить» moves no record (`0x47d990`), so it keeps
    /// that place among the formula channels.
    pub merged_after: Option<(Option<String>, u64)>,
    /// The colour was set by 33030 or a `.set` (`ch+0x140`): it no longer
    /// follows the channel's place.
    pub custom_color: bool,
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
            // The channel constructor's ShowFlags 0xE1 (`0x422536`): visible,
            // the СКЗ / A1 rows, instant values — on no diagram (seen on the
            // VM: a record opened without a `.set` shows empty frequency and
            // vector diagrams, and its new `.set` says 225).
            on_freq: false,
            on_vector: false,
            on_circle: false,
            primary: false,
            merged_after: None,
            custom_color: false,
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
    /// Shown on the frequency diagram (flag 0x2): formula channels are
    /// analog records below `doc+0x1d8`, so «Каналы» column 2, the
    /// frequency menu and «Показать все» reach them (L9.188); off at
    /// creation (flags 0xE1).
    pub on_freq: bool,
    pub stats: StatRows,
    /// A pending (pasted) lane's place: in front of this analog / digital
    /// channel (`Digital(n)` = the end of the digital block); `None` for the
    /// computed section (`0x45ad02` insertion points).
    pub before: Option<ChanKey>,
    /// Shown on the oscillogram (bit 0; «Каналы» dialog column 1).
    pub visible: bool,
    /// The evaluation's values and complex series; `samples` / `phasors`
    /// are them as the accessor reads them (`0x426020`, `0x428490`'s
    /// bit-13 phasor): minus the whole-period mean with remove-DC (0x400),
    /// negated with invert (0x800, the phasor too).
    pub raw: Vec<f64>,
    pub raw_phasors: Option<Vec<Complex64>>,
    pub invert: bool,
    pub remove_dc: bool,
    /// The display mode (bits 7–9): the lane, its value and Max / Min read
    /// `display` (`ch+0x2c`), the RMS / first-harmonic series of `samples`.
    pub mode: ValueMode,
    pub display: Vec<f64>,
    /// A pending lane's own samples as the accessor reads them (NaN as 0,
    /// minus its DC over its own period, negated; a digital one 1 / 0,
    /// inverted with the flag); `display` is then its cache per mode.
    pub pending_own: Vec<f64>,
    /// A pasted lane's name bytes and the text last decoded from them, once
    /// «Кодировка» has re-decoded it (`Doc::set_codepage`).
    pub name_raw: Option<RawName>,
    /// A pending analog lane's phasor cache (`ch+0x38`) over
    /// [`Self::pending_own`] at its own N (`0x428490` → `0x426fc0` /
    /// `0x4267e0`); the vector diagram interpolates it by own time.
    pub pending_phasors: Vec<Complex64>,
    /// A channel group (33035): formula channels can be members
    /// (`0x468c80`).
    pub group: Option<u32>,
    /// Creation order: a new analog-kind record goes to the block's end
    /// (`rec+0x58`, `0x42ea36`), so the block's end lists them by it.
    pub seq: u64,
    /// The colour was set by 33030 or a `.set` (`ch+0x140`).
    pub custom_color: bool,
}

static NEXT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Computed {
    pub fn new(name: String, formula: String, samples: Vec<f64>, color: egui::Color32) -> Computed {
        let samples_raw = samples.clone();
        Computed {
            name,
            formula,
            samples,
            color,
            unit: String::new(),
            phasors: None,
            on_vector: false,
            on_circle: false,
            on_freq: false,
            stats: StatRows::default(),
            visible: true,
            before: None,
            raw: samples_raw,
            raw_phasors: None,
            invert: false,
            remove_dc: false,
            mode: ValueMode::Instant,
            display: Vec::new(),
            pending_own: Vec::new(),
            name_raw: None,
            pending_phasors: Vec::new(),
            group: None,
            seq: NEXT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            custom_color: false,
        }
    }

    /// New evaluation results; the flags apply at the next
    /// [`Doc::reflag_computed`].
    pub fn set_values(&mut self, samples: Vec<f64>, phasors: Option<Vec<Complex64>>) {
        self.raw = samples.clone();
        self.raw_phasors = phasors.clone();
        self.samples = samples;
        self.phasors = phasors;
        self.display.clear();
    }

    /// A pending lane's display cache at own sample `k`.
    pub fn own_shown(&self, k: usize) -> f64 {
        if self.mode == ValueMode::Instant { self.pending_own.get(k) } else { self.display.get(k) }.copied().unwrap_or(0.0)
    }

    /// A pending lane's whole own display series.
    pub fn own_series(&self) -> &[f64] {
        if self.mode == ValueMode::Instant {
            &self.pending_own
        } else {
            &self.display
        }
    }

    /// The display cache at `k` (`ch+0x2c`).
    pub fn shown(&self, k: usize) -> f64 {
        if self.mode == ValueMode::Instant { self.samples.get(k) } else { self.display.get(k) }.copied().unwrap_or(0.0)
    }

    /// The whole display series.
    pub fn shown_series(&self) -> &[f64] {
        if self.mode == ValueMode::Instant {
            &self.samples
        } else {
            &self.display
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

type ParseResult = Result<(Oscillogram, fvb_core::comtrade::CffSections), fvb_core::comtrade::ComtradeError>;
/// A parsed record with its derived arrays, built on the worker.
type LoadResult = Result<(Box<Doc>, fvb_core::comtrade::CffSections), fvb_core::comtrade::ComtradeError>;

/// «Нет» (string 3306): the empty block the library loader `0x40c7e0` puts
/// first in each diagram's «Тип блока» list.
pub const NO_BLOCK: &str = "Нет";

/// A diagram's blocks: «Нет», then the library's blocks with
/// characteristics of that plane.
pub fn plane_library(full: &Library, dim: Dimension) -> Library {
    let mut l = full.for_dimension(dim);
    l.blocks.insert(0, fvb_core::model::Block { name: NO_BLOCK.into(), characteristics: Vec::new() });
    l
}

/// AfxComparePath: the same file (both paths made absolute where they
/// exist; case ignored where the file system ignores it).
pub fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    let full = |p: &std::path::Path| {
        let s = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().into_owned();
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    full(a) == full(b)
}

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
fn parse_file(path: &std::path::Path, enc: Encoding, prog: Option<&fvb_core::comtrade::Progress>) -> ParseResult {
    let lower = path.to_string_lossy().to_lowercase();
    if lower.ends_with(".sto") {
        // A Configurator-MT record (`0x4563b0`): the name re-built with a
        // lowercase ".sto" (`0x44c710`), opened, its header read by the
        // converter (OscillHelper.dll — ⛔, no sample) — never a `.cfg`
        // beside it. A failed open or header read throws string 124 with
        // that path (`0x4564cd`–`0x4564fe`); ours stops there.
        let sto = path.with_extension("sto");
        return Err(fvb_core::comtrade::ComtradeError::Plain(format!("Ошибка чтения файла {}", sto.display())));
    }
    if lower.ends_with(".cff") {
        std::fs::read(path)
            .map_err(|e| fvb_core::comtrade::ComtradeError::Io(e.to_string()))
            .and_then(|b| fvb_core::comtrade::parse_cff_progress(&b, enc, prog))
    } else {
        fvb_core::comtrade::load_progress(path, enc, prog).map(|o| (o, Default::default()))
    }
}

/// Parse a record and build its document on the loader's thread: the
/// original's phase 2 (`0x428490` through the `.set` apply `0x44f5b0`, the
/// per-sample recompute) runs there too, under the progress dialog.
fn open_file(path: &std::path::Path, enc: Encoding, prog: Option<&fvb_core::comtrade::Progress>) -> LoadResult {
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    parse_file(path, enc, prog).map(|(osc, cff)| (Box::new(Doc::new(osc, name)), cff))
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

/// ADC codes of a channel ([`fvb_core::comtrade::AnalogChannel::adc_code`]).
fn adc_codes(c: &fvb_core::comtrade::AnalogChannel) -> Vec<f64> {
    c.samples.iter().map(|&v| c.adc_code(v)).collect()
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
    /// Source file path (for re-opening with a different encoding) — MFC's
    /// path, which a Save As moves.
    pub path: Option<std::path::PathBuf>,
    /// The path the record was opened from (`doc+0x260`): the save's
    /// same-file test compares with it, case-sensitively, and nothing ever
    /// updates it (`0x44d772`, `0x44da96`).
    pub rec_path: Option<std::path::PathBuf>,
    /// Vertical визир 1 (`doc+0x284`) — "cursor 1" of the diagrams and ОМП.
    pub main_cursor: usize,
    /// Vertical визир 2 (`doc+0x29c`) — "cursor 2".
    pub aux_cursor: usize,
    /// Vertical визир 3 (`doc+0x2b4`).
    pub v3_cursor: usize,
    /// Shown flags of В1 / В2 / В3; a new document shows none (`0x449170`).
    pub v_shown: [bool; 3],
    /// В1 has been shown in this document (placed by a click, 32813, the
    /// keys or ОМП): the vector table's value cells wait for it (VM; they
    /// stay when it is hidden again).
    pub v1_placed: bool,
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
    /// The element under the pointer (`doc+0x854`), drawn selected: set by
    /// a pointer move over the plot, kept when the pointer leaves it.
    pub comment_hot: Option<usize>,
    /// Its tab's colour, an index into [`TAB_COLORS`], given when the tab
    /// is first laid out (`0x1006a29c`).
    pub tab_color: Option<usize>,
    /// A number of its own (its MDI child window), kept while others close.
    pub serial: u64,
    /// Its MDI tab group, 0 the first (`crate::mdi`, L9.263).
    pub mdi_group: usize,
    /// When it was last the active document: a group shows its most
    /// recent one in front.
    pub front_stamp: u64,
    /// Selected channels (the original's per-channel `+0x100` flag): the
    /// target of the channel context menu and the per-channel commands.
    pub selection: BTreeSet<ChanKey>,
    /// Anchor of a Shift+click range selection (the original's `view+0x114`).
    pub sel_anchor: Option<ChanKey>,
    /// Next free channel-group id (Совместить каналы).
    pub next_group: u32,
    /// Each group's members as the document keeps them (`doc+0x174`): their
    /// original names' ANSI bytes, resolved by `0x42f020`; a group whose
    /// name no longer resolves is dissolved after a formula evaluation
    /// (`0x455c70`). Membership itself is the channels' `group`.
    pub group_names: std::collections::BTreeMap<u32, Vec<Vec<u8>>>,
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
    /// Each digital channel's value flags (ch+0x60; see [`DigFlags`]).
    pub dig_flags: Vec<DigFlags>,
    /// The code page the record's names are currently decoded in (`doc+0x6e8`,
    /// 866 / 1251: file parameters «Кодировка»; see `recdlg`).
    pub codepage: Encoding,
    /// The station's, the recorder's and each channel's name as read (see
    /// [`RawName`]), in step with the channels (`Doc::ensure_name_raw`).
    pub name_raw: Vec<RawName>,
    /// The record comment (`.hdr` text, 32805) and its code page.
    pub hdr: Option<String>,
    pub hdr_cp: Encoding,
    /// The encoding the names were read in (the program's, or the UTF-8 a
    /// `.cfg` fixes): «Кодировка»'s ANSI — the names as read.
    pub read_enc: Encoding,
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
    /// The document title the tab, the caption and «Окна...» show: MFC's
    /// SetPathName → AfxGetFileTitle, the file name with its extension (seen
    /// on the VM: «WIN_1999.cfg»); a document without a file, its name.
    pub fn title(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map_or_else(|| self.name.clone(), |n| n.to_string_lossy().into_owned())
    }

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
            rec_path: None,
            main_cursor: 0,
            aux_cursor: 0,
            v3_cursor: 0,
            v_shown: [false; 3],
            v1_placed: false,
            hviz: [HVizir::default(); 2],
            chans,
            computed: Vec::new(),
            cff: Default::default(),
            dif_pairs: Vec::new(),
            rx_pairs: Vec::new(),
            comments: Vec::new(),
            comment_hot: None,
            tab_color: None,
            serial: 0,
            mdi_group: 0,
            front_stamp: 0,
            selection: BTreeSet::new(),
            sel_anchor: None,
            next_group: 1,
            group_names: Default::default(),
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
            dig_flags: Vec::new(),
            codepage: Encoding::Cp1251,
            name_raw: Vec::new(),
            hdr: None,
            hdr_cp: Encoding::Cp1251,
            read_enc: Encoding::Cp1251,
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
        self.flag_digital(false);
        // The correction reads the frequency channel as stored (L9.65).
        self.analysis = Analysis::compute_with(&self.flagged, fvb_core::freqcorr::of(&self.osc));
        // The copy was just taken: no name to bring over.
        let _ = self.evaluate_formula_list(show_primary);
    }

    /// The formulas resolve a channel by its name as shown now (`0x42edf0`
    /// compares `+0x68`; the original keeps no copy of the record): the
    /// flagged copy takes the names a change left behind it — «Кодировка»
    /// re-decodes them in place (`0x4543e0` → `0x425960`, issue #7), a
    /// rename in «Каналы» or on the lane writes one — none of which
    /// recomputes.
    /// The digital list is copied anew: ОМП's КЗ flags come, go and take
    /// new values with no recompute (`omp_compute` / `omp_clear`).
    pub fn sync_flagged_names(&mut self) {
        for (f, c) in self.flagged.analog.iter_mut().zip(&self.osc.analog) {
            f.id.clone_from(&c.id);
        }
        self.flag_digital(true);
    }

    /// Digital channels as the accessor reads them: inverted with the flag.
    fn flag_digital(&mut self, copy: bool) {
        if copy {
            self.flagged.digital.clone_from(&self.osc.digital);
        }
        for (i, dch) in self.flagged.digital.iter_mut().enumerate() {
            if self.dig_flags.get(i).is_some_and(|f| f.invert) {
                dch.samples.iter_mut().for_each(|v| *v = !*v);
            }
        }
    }

    /// The batch `0x415780` over the record's formula channels (merged and
    /// ОМП channels carry no formula to re-evaluate): in order,
    /// each updated as it goes; the first failure stops it (the ones before
    /// keep their new values) and is returned; only a full success sweeps
    /// the groups (`0x415b97` → `0x455c70`).
    pub fn try_reevaluate_formulas(&mut self, show_primary: bool) -> Result<(), fvb_core::expr::FormulaError> {
        self.sync_flagged_names();
        self.evaluate_formula_list(show_primary)
    }

    /// [`Doc::try_reevaluate_formulas`] over the flagged copy as it is.
    fn evaluate_formula_list(&mut self, show_primary: bool) -> Result<(), fvb_core::expr::FormulaError> {
        let mut done: Vec<fvb_core::quick::ComputedChannel> = Vec::new();
        for j in 0..self.computed.len() {
            let c = &mut self.computed[j];
            if c.formula != "ОМП" && c.formula != "объединение" && c.formula != crate::merge_ui::PENDING {
                let batch = [(c.name.clone(), c.formula.clone())];
                let mut out = fvb_core::quick::evaluate_batch_with(&self.flagged, fvb_core::freqcorr::of(&self.osc), &done, &batch, show_primary)?;
                if let Some(new) = out.pop() {
                    c.set_values(new.samples, new.phasors);
                }
            }
            // A later formula reads this one through the accessor and the
            // phasor cache: flagged.
            self.reflag_computed(j);
            done.push(self.computed[j].as_core());
        }
        self.prune_groups();
        Ok(())
    }

    /// Computed channel `j`'s flags over its evaluation (`0x428490` for a
    /// bit-13 channel): the DC is the mean of the whole periods from the
    /// start, the phasor negated by invert only; then the display cache
    /// per mode — RMS `0x4272a0`, first harmonic `0x426240` (× the
    /// record's gain when uncorrected), over the flagged values.
    pub fn reflag_computed(&mut self, j: usize) {
        let n = self.osc.samples_per_cycle().max(2);
        let corr = fvb_core::freqcorr::of(&self.osc).filter(|_| fvb_core::freqcorr::selected_extra(&self.osc) != Some(j));
        let gain = if corr.is_some() { 1.0 } else { self.osc.harmonic1_gain };
        let Some(c) = self.computed.get_mut(j) else { return };
        let sign = if c.invert { -1.0 } else { 1.0 };
        if c.formula == crate::merge_ui::PENDING {
            // A pasted channel (bit 14) over its own samples: N from its own
            // rate ÷ 50 (`0x427315`), its own rate for the correction.
            let Some(p) = self.pending.iter().find(|p| p.lane == c.seq) else { return };
            match &p.pasted.channel {
                Ok(a) => {
                    let pn = if p.pasted.rate == 0.0 { n } else { (p.pasted.rate / 50.0).round().max(1.0) as usize };
                    let own_corr = corr.map(|q| fvb_core::freqcorr::FreqCorr { fs: if p.pasted.rate == 0.0 { q.fs } else { p.pasted.rate }, ..q });
                    // Read ×k in the copied channel's display mode.
                    let k = p.display_k();
                    let x: Vec<f64> = a.samples.iter().map(|v| if v.is_nan() { 0.0 } else { *v * k }).collect();
                    let dc = if c.remove_dc { whole_period_mean(&x, pn) } else { 0.0 };
                    c.pending_own = x.iter().map(|v| sign * (v - dc)).collect();
                    c.pending_phasors = fvb_core::freqcorr::sliding_phasors(own_corr, fvb_core::freqcorr::OTHER, &c.pending_own, pn);
                    c.display = match c.mode {
                        ValueMode::Instant => Vec::new(),
                        ValueMode::Rms => (0..x.len()).map(|k| fvb_core::freqcorr::rms_at(own_corr, fvb_core::freqcorr::OTHER, &c.pending_own, k, pn, 1)).collect(),
                        ValueMode::FirstHarmonic => (0..x.len()).map(|k| fvb_core::freqcorr::fundamental_at(own_corr, fvb_core::freqcorr::OTHER, &c.pending_own, k, pn) * gain).collect(),
                    };
                }
                Err(dch) => {
                    c.pending_own = dch.samples.iter().map(|&b| if b != c.invert { 1.0 } else { 0.0 }).collect();
                    c.pending_phasors = Vec::new();
                    c.display = Vec::new();
                }
            }
            return;
        }
        let dc = if c.remove_dc { whole_period_mean(&c.raw, n) } else { 0.0 };
        c.samples = c.raw.iter().map(|x| sign * (x - dc)).collect();
        c.phasors = c.raw_phasors.as_ref().map(|p| p.iter().map(|z| z * sign).collect());
        c.display = match c.mode {
            ValueMode::Instant => Vec::new(),
            ValueMode::Rms => (0..c.samples.len()).map(|k| fvb_core::freqcorr::rms_at(corr, fvb_core::freqcorr::OTHER, &c.samples, k, n, 1)).collect(),
            ValueMode::FirstHarmonic => (0..c.samples.len()).map(|k| fvb_core::freqcorr::fundamental_at(corr, fvb_core::freqcorr::OTHER, &c.samples, k, n) * gain).collect(),
        };
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

    /// The statistics panel's СКЗ and A1 (without the record gain) at `k`,
    /// computed when painted with the current correction (`0x4272a0`,
    /// `0x426240` read `rec+0x874` live) — unlike the cached series, which
    /// keep the correction they were built with until a full recompute.
    pub fn stat_rms(&self, i: usize, k: usize) -> f64 {
        let n = self.osc.samples_per_cycle().max(2);
        self.flagged.analog.get(i).map_or(0.0, |c| fvb_core::freqcorr::rms_at(fvb_core::freqcorr::of(&self.osc), i, &c.samples, k, n, 1))
    }

    pub fn stat_a1(&self, i: usize, k: usize) -> f64 {
        let n = self.osc.samples_per_cycle().max(2);
        self.flagged.analog.get(i).map_or(0.0, |c| fvb_core::freqcorr::fundamental_at(fvb_core::freqcorr::of(&self.osc), i, &c.samples, k, n))
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
    /// Pairs naming a channel the record no longer has go (`0x4554e0` R/X,
    /// `0x455210` differential — the circle / differential painters and
    /// 33027 call them): the analog and computed channels' names, compared
    /// case-sensitively (Compare, mfc #2948).
    pub fn purge_pairs(&mut self) {
        let names: std::collections::HashSet<&str> = self.osc.analog.iter().map(|c| c.id.as_str()).chain(self.computed.iter().map(|c| c.name.as_str())).collect();
        let keep = |p: &crate::diagram::ChanPair| names.contains(p.a.as_str()) && names.contains(p.b.as_str());
        let (rx, dif): (Vec<_>, Vec<_>) = (self.rx_pairs.iter().filter(|p| keep(p)).cloned().collect(), self.dif_pairs.iter().filter(|p| keep(p)).cloned().collect());
        (self.rx_pairs, self.dif_pairs) = (rx, dif);
    }

    /// Digital channel `i`'s flags (the defaults past the stored ones).
    pub fn dig(&self, i: usize) -> DigFlags {
        self.dig_flags.get(i).copied().unwrap_or_default()
    }

    pub fn dig_mut(&mut self, i: usize) -> &mut DigFlags {
        if self.dig_flags.len() <= i {
            self.dig_flags.resize(i + 1, DigFlags::default());
        }
        &mut self.dig_flags[i]
    }

    /// Digital channel `i`'s displayed state at sample `k` (the display
    /// cache: inverted when the flag is set).
    pub fn dig_on(&self, i: usize, k: usize) -> bool {
        self.osc.digital.get(i).and_then(|d| d.samples.get(k)).copied().unwrap_or(false) != self.dig(i).invert
    }

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
            self.v1_placed |= k == 0 && self.v_shown[k];
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

    /// 32802 / 32803's state (`0x45c8c0`): `f` below 1.0 / above the fit
    /// scale.
    pub fn zoom_x_enabled(&self, width: f32) -> [bool; 2] {
        let fit = crate::oscgeom::fit_scale(width, self.osc.sample_count());
        let f = self.xscale.unwrap_or(fit);
        [f < 1.0, f > fit]
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

    /// 32817..32819's state (`0x42fc60`): every analog channel counts —
    /// computed and pasted ones too; disabled with none, checked when all
    /// of them have the row.
    pub fn stats_row_state(&self, k: usize) -> crate::chansel::ItemState {
        let get = |s: &StatRows| [s.minmax, s.rms, s.a1][k.min(2)];
        let all = self.chans.iter().map(|c| &c.stats).chain(self.computed.iter().map(|c| &c.stats)).all(get);
        let any = !self.chans.is_empty() || !self.computed.is_empty();
        crate::chansel::ItemState { enabled: any, checked: any && all }
    }

    /// 57634 / menu 561's copy (`0x45bf60`): a selected channel that is not
    /// computed (digital and pasted ones count).
    pub fn can_copy(&self) -> bool {
        self.selection.iter().any(|k| match k {
            ChanKey::Computed(j) => self.computed.get(*j).is_some_and(|c| c.formula == crate::merge_ui::PENDING),
            ChanKey::Digital(i) => !self.dig_computed(*i),
            ChanKey::Analog(_) => true,
        })
    }

    /// A digital channel made by a computation: the ОМП's «КЗ фазы» flags
    /// carry bit 13 (`0x4245a0` → `0x425f90`), so the copy, its CMDUI and
    /// the rename treat them as computed channels (L9.188).
    pub fn dig_computed(&self, i: usize) -> bool {
        self.osc.digital.get(i).is_some_and(|c| crate::omp_ui::KZ_PHASES.contains(&c.id.as_str()))
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
    channel_color(&ch.id, &ch.phase, i)
}

/// A channel's colour without a custom one (`0x42b520`): the phase class
/// (`0x42b0c0`) — 1 (192,192,0), 2 (0,114,41), 3 (255,0,0), 4 (0,0,255) —,
/// else the 10-entry table `0x4c2fe0` by the channel's place in the record's
/// channel vector (analog, computed, digital) mod 10.
pub fn channel_color(name: &str, phase: &str, index: usize) -> egui::Color32 {
    match phase_class(name, phase) {
        1 => egui::Color32::from_rgb(192, 192, 0),
        2 => egui::Color32::from_rgb(0, 114, 41),
        3 => egui::Color32::from_rgb(255, 0, 0),
        4 => egui::Color32::from_rgb(0, 0, 255),
        _ => table_color(index),
    }
}

/// The table `0x4c2fe0` (the first of each pair) by vector place mod 10 —
/// a digital channel's colour (never classed).
pub fn table_color(index: usize) -> egui::Color32 {
    const TABLE: [(u8, u8, u8); 10] = [
        (54, 6, 186),
        (194, 2, 116),
        (193, 52, 52),
        (0, 111, 122),
        (0, 114, 41),
        (183, 39, 191),
        (34, 2, 121),
        (217, 92, 5),
        (167, 3, 48),
        (79, 3, 169),
    ];
    let (r, g, b) = TABLE[index % 10];
    egui::Color32::from_rgb(r, g, b)
}

/// `0x42b0c0`: the phase field (lowered, exact: a / а → 1, b / б → 2,
/// c / с → 3, ab / аб → 5, bc / бс → 6, ca / са → 7), else the first of these
/// in the lowered name: uab → 5, ubc → 6, uca → 7, ua / ia → 1, ub / ib → 2,
/// uc / ic → 3, uo / un / u0 / io / in / i0 → 4 — the letters after `u` /
/// `i` also in their Cyrillic look-alikes (а б с о п); 0 for none.
pub fn phase_class(name: &str, phase: &str) -> u8 {
    let ph = phase.to_lowercase();
    for (c, list) in [(1, ["a", "а"]), (2, ["b", "б"]), (3, ["c", "с"]), (5, ["ab", "аб"]), (6, ["bc", "бс"]), (7, ["ca", "са"])] {
        if list.contains(&ph.as_str()) {
            return c;
        }
    }
    let n = name.to_lowercase();
    let rules: [(u8, &[&str]); 7] = [
        (5, &["uab", "uаб"]),
        (6, &["ubc", "uбс"]),
        (7, &["uca", "uса"]),
        (1, &["ua", "uа", "ia", "iа"]),
        (2, &["ub", "uб", "ib", "iб"]),
        (3, &["uc", "uс", "ic", "iс"]),
        (4, &["uo", "uо", "un", "uп", "u0", "io", "iо", "in", "iп", "i0"]),
    ];
    rules.iter().find(|(_, subs)| subs.iter().any(|s| n.contains(s))).map_or(0, |(c, _)| *c)
}

/// A digital channel's value flags (ch+0x60): invert (0x800) makes the
/// display cache 1 where the sample is 0 and 0 elsewhere (`0x428490`'s
/// digital branch, `0x426020`), so the lane and the merge see it; remove-DC
/// (0x400) and the display mode (0x80 / 0x100 / 0x200, 0x80 by default)
/// change nothing but the menus' checks (L9.134).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DigFlags {
    pub invert: bool,
    pub remove_dc: bool,
    pub mode: ValueMode,
    /// A colour set by 33030 (`ch+0x140` / `ch+0x144`), else the table's by
    /// place.
    pub color: Option<egui::Color32>,
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
    pub dig_flags: Vec<DigFlags>,
    /// The names' bytes go with the channels (the original's snapshot is
    /// its channel list, the raw names in it); the code page stays.
    pub name_raw: Vec<RawName>,
}

/// A name as read (`+0x64`): its bytes, the text last decoded from them in
/// the record's code page, and whether the channel shows an alias instead
/// (`+0x6c` non-empty — kept as typed across «Кодировка», `0x425960`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawName {
    pub bytes: Vec<u8>,
    pub seen: String,
    pub alias: bool,
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
    /// The document each diagram last loaded its block values from
    /// (`this+0x98` of the circle / differential window; its serial).
    pub diag_bound: [Option<u64>; 2],
    // active diagram state
    pub library: Library,
    pub source_name: String,
    pub block_idx: usize,
    /// «Тип блока»'s text when the document names a block the library
    /// lacks: the refresh writes the saved name into the cell (`0x436d3b`)
    /// while block 0 is active (`0x407814`); a click on the cell saves the
    /// active block over it.
    pub block_text: Option<String>,
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
    /// The diagram panes' docking (L9.92): each pane's unit, a group
    /// being dragged after it detached (with the pointer's grab offset in
    /// its rect), and the slid-out auto-hide areas.
    pub docks: crate::docking::Docks,
    pub dock_drag: Option<(crate::docking::Unit, egui::Vec2)>,
    /// MFC's pane menu open on a caption (`0x100ce72c`): the unit, the
    /// pane shown in it, another shown tab of its group, the popup.
    pub pane_menu: Option<(crate::docking::Unit, Pane, Option<Pane>, crate::popmenu::PopMenu)>,
    /// The tab row's own menus («Стиль», Help), open.
    pub top_menu: Option<(crate::ribbon::TopMenu, crate::popmenu::PopMenu)>,
    /// The frame client (the edge markers' rect) and each docked group's
    /// pane rect this frame (by the group's own unit).
    pub dock_client: egui::Rect,
    pub dock_rects: [Option<egui::Rect>; 4],
    /// The diagram tabs' order (a tab dragged over another swaps with it).
    pub tab_order: [Pane; 4],
    /// Keeps a smart-docking drag going without a held button (the
    /// `window_dump` state `dockmarkers`).
    pub dock_drag_hold: bool,
    pub dock_areas: [Option<egui::Rect>; 4],
    pub dock_strips: [Option<egui::Rect>; 4],
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
    /// A channel drag in the view's 11 px scroll inset: its scroll code
    /// (vertical part high, horizontal low) and when the delay last
    /// restarted or it last scrolled.
    pub drag_scroll: Option<(u16, f64)>,
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
    /// The diagram tab strips' tabs this frame.
    pub pane_tab_rects: Vec<(Pane, egui::Rect)>,
    /// The document tabs this frame, in strip order.
    pub doc_tab_rects: Vec<egui::Rect>,
    /// The pointer over the view's client area at the last frame it was
    /// there: a change is the original's WM_MOUSEMOVE.
    pub view_pointer: Option<egui::Pos2>,
    /// MFC's process-wide auto-colour state of the tabs.
    pub tab_auto: TabAuto,
    /// The MDI tab groups' widths left to right, the 4 px splitters
    /// between them not counted (`crate::mdi`); fewer than two: one group.
    pub mdi_widths: Vec<f32>,
    /// The length along the groups' axis `mdi_widths` were fitted to.
    pub mdi_len: f32,
    /// The groups side by side (else stacked); with one group the next
    /// split may go either way.
    pub mdi_vertical: bool,
    /// The area the groups were last laid out in.
    pub mdi_area: egui::Rect,
    /// Each group's window (with its edge) as last laid out.
    pub mdi_wins: Vec<egui::Rect>,
    /// What each group's documents get (its window less its edge) as last
    /// laid out; the press that activates a group is tested against them
    /// before anything is drawn.
    pub mdi_rects: Vec<egui::Rect>,
    /// Each group's tab strip as last drawn.
    pub mdi_strips: Vec<egui::Rect>,
    /// A document tab let go off its strip (its document's serial, where).
    pub mdi_drop: Option<(u64, egui::Pos2)>,
    /// The counter behind `Doc::front_stamp`.
    pub front_counter: u64,
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
    /// The scale the minimum window size was last sent for.
    pub min_size_ppp: f32,
    /// `-Dst <file>` (app+0x118): the `.dst` description the next `.sto`
    /// open uses as is (`0x45646b`); emptied by every open that gets past
    /// the `.sto` step (`0x44f05a`).
    pub dst_file: Option<std::path::PathBuf>,
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
    /// The frame's last restored (not maximised) rect this session, saved
    /// as the normal placement when the frame closes maximised (GDI's
    /// `rcNormalPosition`).
    pub frame_normal: Option<egui::Rect>,
    /// An `Event::Paste` came since Ctrl+V was last let go.
    pub paste_announced: bool,
    /// The window has no system decorations and draws its own caption row
    /// (the ribbon's, L9.150); off in tests and with `FV_SYSTEM_TITLEBAR`.
    pub own_caption: bool,
    /// The start-up splash (`0x4478a0`): shown until this time (s).
    pub splash_until: Option<f64>,
    /// The status bar's six panes as last written (`0x472e10`'s cache):
    /// the creation texts (3130, 3134, 3426, 3170, 3172, 3428) until a
    /// document is active, then whatever the last one showed.
    pub status_panes: [String; 6],
    /// The status bar's elements hidden from its «Status Bar
    /// Configuration» menu: the six panes, then the extended one.
    pub status_hidden: [bool; 7],
    /// That menu, open at a point since a pass.
    pub status_menu: Option<(egui::Pos2, u64)>,
    /// Commands from a menu that open a system file dialog, run in a
    /// later pass once the closed menu has been painted (the dialog blocks
    /// the frame loop).
    pub deferred: Vec<(Command, u64)>,
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
    /// The plane-scale extent of the figures (`geom::integer_extent`),
    /// kept with the geometry.
    pub extent: f64,
    pub color: egui::Color32,
}

impl App {
    pub fn new() -> Self {
        let (full_library, source_name) = load_libraries();
        let mut app = App {
            library: plane_library(&full_library, Dimension::Ohm),
            full_library,
            diagram_dim: Dimension::Ohm,
            diagrams: [None, None],
            diag_bound: [None, None],
            source_name,
            block_idx: 0,
            block_text: None,
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
            // The group on the right edge; Частотная below it in the same
            // column (the user's screenshot, L9.3 — the original's create
            // docks it on the frame's bottom edge).
            docks: crate::docking::Docks::default(),
            dock_drag: None,
            pane_menu: None,
            top_menu: None,
            dock_client: egui::Rect::NOTHING,
            dock_rects: [None; 4],
            tab_order: [Pane::Vector, Pane::Circle, Pane::Differential, Pane::Frequency],
            dock_drag_hold: false,
            dock_areas: [None; 4],
            dock_strips: [None; 4],
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
            drag_scroll: None,
            view_width: 1000.0,
            view_left: crate::lanes::LEFT_W_DEFAULT,
            view_rects: None,
            pane_canvas: Vec::new(),
            pane_tab_rects: Vec::new(),
            doc_tab_rects: Vec::new(),
            pane_grid: Vec::new(),
            view_pointer: None,
            tab_auto: TabAuto::default(),
            mdi_widths: Vec::new(),
            mdi_len: 0.0,
            mdi_vertical: true,
            mdi_area: egui::Rect::NOTHING,
            mdi_wins: Vec::new(),
            mdi_rects: Vec::new(),
            mdi_strips: Vec::new(),
            mdi_drop: None,
            front_counter: 0,
            windows_dlg: None,
            next_serial: 1,
            copy_focus: CopyFocus::View,
            ppp: 1.0,
            window_title: String::new(),
            min_size_ppp: 0.0,
            finder: None,
            loading: Vec::new(),
            mru: Vec::new(),
            merge: None,
            merge_drag: None,
            dst_file: None,
            lane_menu: None,
            lane_menu_serial: 0,
            lane_menu_eat: false,
            color_dialog: None,
            modal_up: false,
            frame_normal: None,
            paste_announced: false,
            own_caption: false,
            splash_until: None,
            status_hidden: [false; 7],
            status_menu: None,
            deferred: Vec::new(),
            status_panes: ["Верт. визир 1:", "Верт. визир 2:", "Разница в. визиров:", "Гор. визир 1:", "Гор. визир 2:", "Разница г. визиров:"].map(String::from),
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
        // Into the active tab group (MFC adds a new child to it).
        doc.mdi_group = self.active_doc().map_or(0, |d| d.mdi_group);
        // The original's open says nothing (no status line, no message).
        self.docs.push(doc);
        self.active = Some(self.docs.len() - 1);
        self.view = View::Oscillogram;
    }

    /// Close document `i` without asking.
    pub fn close_doc_now(&mut self, i: usize) {
        if i < self.docs.len() {
            let g = self.docs.remove(i).mdi_group;
            // Indices shift: re-bind the ОМП bar and the settings sync on
            // the next frame.
            self.omp_ui.bar_doc = None;
            self.setfile.sync = None;
            self.active = if self.docs.is_empty() {
                None
            } else {
                // The group's next document in front, else another group's.
                self.group_front(g).or(Some(i.min(self.docs.len() - 1)))
            };
            self.prune_groups();
        }
    }

    // ---------- command dispatch ----------
    /// A command chosen in a menu: one that opens a system file dialog
    /// waits for the next pass, so the menu is gone from the window behind
    /// the dialog (VM: «Загрузка настроек...» from the application menu).
    pub fn dispatch_from_menu(&mut self, ctx: &egui::Context, cmd: Command) {
        if matches!(cmd, Command::Open | Command::SaveAs | Command::LoadSettings | Command::ExportCsv | Command::Archive | Command::OpenFvb | Command::ExportImage) {
            self.deferred.push((cmd, ctx.cumulative_pass_nr()));
            ctx.request_repaint();
        } else {
            self.dispatch(cmd);
        }
    }

    /// The deferred commands two passes on: the one after the choice draws
    /// the window without the menu and is painted, then the dialog.
    pub fn run_deferred(&mut self, ctx: &egui::Context) {
        let now = ctx.cumulative_pass_nr();
        let (due, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.deferred).into_iter().partition(|(_, at)| *at + 1 < now);
        self.deferred = wait;
        if !self.deferred.is_empty() {
            ctx.request_repaint();
        }
        for (c, _) in due {
            self.dispatch(c);
        }
    }

    // ---------- command dispatch ----------
    pub fn dispatch(&mut self, cmd: Command) {
        let mark = crate::setfile::marks_modified(&cmd).then_some(self.active).flatten();
        // The handlers that end in the frame's refresh-all `0x475420` (the
        // diagrams' refresh, slot 90): paste 57637 and undo / redo
        // (`0x45acb0`, `0x45bf26`), 32849–32852 (`0x430340`), 32817–32824
        // (`0x42f4b0`), 32866–32868 (`0x42fdb0`), the визиры (`0x492f20`),
        // the `.cfg` / `.cff` writers (`0x44d720`, `0x44d0f0`).
        let refresh = matches!(
            cmd,
            Command::Paste | Command::Undo | Command::Redo | Command::ToggleFlag(_) | Command::SetValueMode(_) | Command::StatsRow(_) | Command::SetPrimaryValues(_) | Command::ToggleVizir(_) | Command::Save | Command::SaveAs
        );
        self.dispatch_inner(cmd);
        if let Some(d) = mark.and_then(|i| self.docs.get_mut(i)) {
            d.set.modified = true;
        }
        if refresh {
            self.refresh_blocks();
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
                    // New pages on every DoModal: defaults, then the `.set`.
                    self.quick = Default::default();
                    self.formula_page_open();
                    self.quick_defaults();
                    self.quick_show_page();
                }
            }
            Command::OmpAnalysis => self.omp_toggle(),
            Command::OmpParams => self.omp_open_params(),
            // `ShowPane(!visible, FALSE, FALSE)` (`0x475040`): a re-shown
            // tab comes to the front only when no other tab of its group
            // shows; hiding the front tab brings the nearest visible one on
            // its right in the strip, else on its left (`0x10067f1f`).
            Command::TogglePane(p) => {
                let on = !self.panes.visible(p);
                let host = self.docks.host(p);
                let strip: Vec<Pane> = self.tab_order.into_iter().filter(|&q| self.docks.host(q) == host).collect();
                let was_front = self.panes.front(strip.iter().copied()) == Some(p);
                self.panes.set(p, on);
                if !on && was_front {
                    let i = strip.iter().position(|&q| q == p).unwrap_or(0);
                    let next = strip[i + 1..].iter().find(|&&q| self.panes.visible(q)).or_else(|| strip[..i].iter().rev().find(|&&q| self.panes.visible(q)));
                    if let Some(&q) = next {
                        self.dispatch(Command::ActivatePane(q));
                    }
                }
            }
            Command::ActivatePane(p) => {
                self.panes.activate(p);
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

    /// Dispatch any Ctrl-shortcut pressed this frame. TranslateAccelerator
    /// matches Ctrl alone (no Shift / Alt, the entries are VIRTKEY|CONTROL)
    /// and MFC drops an accelerator whose command is disabled (CN_UPDATE_
    /// COMMAND_UI first). egui-winit turns Ctrl+C into `Event::Copy` and
    /// Ctrl+V into `Event::Paste` — the latter only while the OS clipboard
    /// holds text —, with no key event; Ctrl+V's key-up still comes, so a
    /// paste no `Event::Paste` announced fires there.
    pub fn handle_keys(&mut self, ctx: &egui::Context) {
        let mut cmds = Vec::new();
        ctx.input(|i| {
            let m = i.modifiers;
            // No MDI keys: TranslateMDISysAccel (Ctrl+F4 / Ctrl+F6 /
            // Ctrl+Tab, L9.130) acts through a child's system menu, which the
            // tabbed MDI children have not — on the VM Ctrl+Tab and Ctrl+F4
            // do nothing with two documents open (L9.294).
            // Each key by the modifiers it came with: in a slow frame Ctrl
            // may be up again by the frame's end (the software runner, VM).
            // `Copy` / `Paste` come only from their shortcuts (Ctrl+Shift
            // too: refused while Shift / Alt is held).
            let accel = |m: &egui::Modifiers| (m.command || m.ctrl) && !m.shift && !m.alt;
            let plain = !m.shift && !m.alt;
            for e in &i.events {
                match e {
                    egui::Event::Key { key, pressed: true, modifiers, .. } if accel(modifiers) => {
                        if let Some((_, c)) = Self::keymap().into_iter().find(|(k, _)| k == key) {
                            cmds.push(c);
                        }
                    }
                    egui::Event::Copy if plain => cmds.push(Command::Copy),
                    egui::Event::Paste(_) if plain => {
                        self.paste_announced = true;
                        cmds.push(Command::Paste);
                    }
                    // The key-up also clears the announcement.
                    egui::Event::Key { key: egui::Key::V, pressed: false, modifiers, .. } if accel(modifiers) && !std::mem::take(&mut self.paste_announced) => cmds.push(Command::Paste),
                    _ => {}
                }
            }
        });
        for c in cmds {
            if self.shortcut_enabled(&c) {
                self.dispatch(c);
            }
        }
    }

    /// The table-128 commands' update rules (`0x45bf60`; Open is the
    /// app's, always enabled; Save As / Print need a document).
    fn shortcut_enabled(&self, c: &Command) -> bool {
        let Some(d) = self.active_doc() else { return matches!(c, Command::Open) };
        match c {
            Command::Copy => d.can_copy(),
            Command::Paste => self.clip.available() && !crate::omp_ui::has_pending_paste(d),
            Command::Undo => !d.undo.is_empty(),
            Command::Redo => !d.redo.is_empty(),
            _ => true,
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
            self.chars.entry((b, ci)).or_insert_with(|| CharState {
                visible: true,
                settings: ch.default_settings(),
                figs: Vec::new(),
                geo: None,
                extent: 0.0,
                color: Self::palette(ci),
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
            st.extent = fvb_core::geom::integer_extent(ch, &st.figs);
            self.chars.insert((b, ci), st);
        }
        // The block evaluation's exception is posted and reported in an
        // MB_OK box (`0x409260` → `0x475680`).
        if let Err(e) = result {
            self.dlg.push_alert(e.to_string());
        }
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
        self.restore_workspace();
        self.chars.clear();
        self.selected_char = None;
        self.ensure_block_state();
        self
    }

    /// The workspace on closing the main frame (`OnClosingMainFrame`
    /// `0x101a9717` → `SaveState`): the panes' places, recent floating
    /// rects and visibility, the group's shown tab and the window's
    /// placement; read back by [`App::restore_workspace`] and `main`.
    pub fn save_workspace(&mut self, placement: Option<(egui::Rect, bool)>) {
        if !self.persist {
            return;
        }
        let v = &mut self.store.view;
        self.docks.store_all(v);
        for (k, p) in [("Vector", Pane::Vector), ("Circle", Pane::Circle), ("Differential", Pane::Differential), ("Frequency", Pane::Frequency)] {
            v.insert(format!("Workspace/{k}/IsVisible"), u8::from(self.panes.visible(p)).to_string());
            v.insert(format!("Workspace/{k}/Activated"), self.panes.stamp[p as usize].to_string());
            if self.panes.active == p {
                v.insert("Workspace/ActiveTab".into(), k.into());
            }
        }
        // Maximised, the normal rect stays the one saved before (GDI's
        // `rcNormalPosition`; egui reports only the maximised one).
        let names = self.tab_order.map(|p| format!("{p:?}"));
        v.insert("Workspace/TabOrder".into(), names.join(" "));
        // The parameters bar's visibility (`BasePane-810` IsVisible).
        v.insert("Workspace/ParamBar/IsVisible".into(), u8::from(self.dlg.param_bar).to_string());
        // The status bar's hidden elements by command id
        // (`MFCStatusBarPanes`): the six panes share 32838, the extended
        // element is 32839.
        let ids: Vec<&str> = [(self.status_hidden[..6].iter().any(|&h| h), "32838"), (self.status_hidden[6], "32839")].into_iter().filter(|x| x.0).map(|x| x.1).collect();
        v.insert("Workspace/StatusBar/Hidden".into(), ids.join(" "));
        if let Some((r, max)) = placement {
            let fmt = |r: egui::Rect| format!("{} {} {} {}", r.left(), r.top(), r.right(), r.bottom());
            let saved = v.get("Workspace/WindowPlacement").and_then(|s| s.rsplit_once(' ')).map(|(r, _)| r.to_string());
            let rect = match (self.frame_normal, saved) {
                (Some(n), _) if max => fmt(n),
                (None, Some(n)) if max => n,
                _ => fmt(r),
            };
            v.insert("Workspace/WindowPlacement".into(), format!("{rect} {}", u8::from(max)));
        }
        self.store.save();
    }

    /// `LoadState` (`0x101a8eea`) for the panes.
    pub fn restore_workspace(&mut self) {
        let v = &self.store.view;
        if let Some(x) = v.get("Workspace/ParamBar/IsVisible") {
            self.dlg.param_bar = x != "0";
        }
        // Loaded by id: one pane hidden at exit hides all six (VM).
        if let Some(x) = v.get("Workspace/StatusBar/Hidden") {
            let has = |id: &str| x.split_whitespace().any(|w| w == id);
            self.status_hidden = [has("32838"); 7];
            self.status_hidden[6] = has("32839");
        }
        if let Some(o) = v.get("Workspace/TabOrder") {
            let all = [Pane::Vector, Pane::Circle, Pane::Differential, Pane::Frequency];
            let read: Vec<Pane> = o.split_whitespace().filter_map(|w| all.into_iter().find(|p| format!("{p:?}") == w)).collect();
            if read.len() == 4 && all.iter().all(|p| read.contains(p)) {
                self.tab_order.copy_from_slice(&read);
            }
        }
        self.docks.restore_all(v);
        for (k, p) in [("Vector", Pane::Vector), ("Circle", Pane::Circle), ("Differential", Pane::Differential), ("Frequency", Pane::Frequency)] {
            if let Some(on) = v.get(&format!("Workspace/{k}/IsVisible")) {
                self.panes.set(p, on == "1");
            }
            if let Some(n) = v.get(&format!("Workspace/{k}/Activated")).and_then(|n| n.parse().ok()) {
                self.panes.stamp[p as usize] = n;
            }
        }
        if let Some(p) = v.get("Workspace/ActiveTab").and_then(|a| [Pane::Vector, Pane::Circle, Pane::Differential, Pane::Frequency].into_iter().find(|p| format!("{p:?}") == *a)) {
            self.panes.active = p;
            // A workspace from before the stamps: the active tab in front.
            if !v.contains_key("Workspace/Vector/Activated") {
                self.panes.activate(p);
            }
        }
    }

    /// `0x408cf0` (DiagramWnd slot 94, after a visibility click, a
    /// parameter commit or a block change): the block into the active
    /// document's settings tree, the document marked modified; with no
    /// document nothing is kept.
    pub fn save_block(&mut self) {
        let Some(block) = self.library.blocks.get(self.block_idx) else { return };
        let chars: Vec<fvb_core::recset::SavedChar> = block
            .characteristics
            .iter()
            .enumerate()
            .filter_map(|(ci, ch)| {
                let st = self.chars.get(&(self.block_idx, ci))?;
                let params = ch.parameters.iter().map(|p| (p.name.clone(), st.settings.get(&p.name).cloned().unwrap_or_default())).collect();
                Some((ch.name.clone(), st.visible, params))
            })
            .collect();
        let (key, name) = (Self::diagram_key(self.diagram_dim), block.name.clone());
        let Some(d) = self.active_doc_mut() else { return };
        fvb_core::recset::write_block(d.set.tree.section_mut(key), &name, &chars);
        d.set.modified = true;
    }

    /// The diagrams' refresh (`0x436b30` / `0x442a10`, slot 90 — on a
    /// document's activation, 32860, …): each picks its block by the active
    /// document's `SelectedBlock` (empty or unknown: «Нет»), and loads the
    /// block's values from it only when the document is not the one it
    /// last loaded from. No document: nothing changes.
    pub fn refresh_blocks(&mut self) {
        if let Some(d) = self.active_doc_mut() {
            crate::chandlg::circle_refresh_flags(d);
        }
        self.refresh_live_block();
        let (here, other) = (self.diagram_dim, if self.diagram_dim == Dimension::Ohm { Dimension::Dif } else { Dimension::Ohm });
        if self.diagrams[usize::from(other == Dimension::Dif)].is_some() {
            self.switch_diagram(other);
            self.refresh_live_block();
            self.switch_diagram(here);
        }
    }

    fn refresh_live_block(&mut self) {
        let gi = usize::from(self.diagram_dim == Dimension::Dif);
        let Some(d) = self.active_doc() else { return };
        let sec = d.set.tree.section(Self::diagram_key(self.diagram_dim)).cloned();
        let sec = sec.as_ref();
        let name = sec.map(|s| s.get_str("SelectedBlock", "")).filter(|n| !n.is_empty()).unwrap_or_else(|| NO_BLOCK.to_string());
        let serial = d.serial;
        let mut changed = false;
        self.block_text = (!self.library.blocks.is_empty() && !self.library.blocks.iter().any(|b| b.name == name)).then(|| name.clone());
        if self.library.blocks.get(self.block_idx).map(|b| b.name.as_str()) != Some(name.as_str()) {
            self.block_idx = self.library.blocks.iter().position(|b| b.name == name).unwrap_or(0);
            self.selected_char = None;
            changed = true;
        }
        if self.diag_bound[gi] != Some(serial) {
            self.diag_bound[gi] = Some(serial);
            if let Some(block) = self.library.blocks.get(self.block_idx) {
                for (ci, ch) in block.characteristics.iter().enumerate() {
                    let saved = fvb_core::recset::saved_char(sec, &ch.name);
                    let mut settings = ch.default_settings();
                    for p in &ch.parameters {
                        if let Some(v) = saved.and_then(|c| c.child(&p.name)).and_then(|n| n.attrs.get("value")) {
                            settings.insert(p.name.clone(), v.clone());
                        }
                    }
                    let st = self.chars.entry((self.block_idx, ci)).or_insert_with(|| CharState {
                        visible: true,
                        settings: Default::default(),
                        figs: Vec::new(),
                        geo: None,
                        extent: 0.0,
                        color: Self::palette(ci),
                    });
                    st.visible = saved.is_none_or(|c| c.get_bool("visible", true));
                    st.settings = settings;
                }
            }
            changed = true;
        }
        if changed {
            self.ensure_block_state();
        }
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
        // A diagram first shown starts on «Нет», then refreshes from the
        // active document (the original's windows exist from the start).
        let fresh = self.diagrams[idx(dim)].is_none();
        let next = self.diagrams[idx(dim)].take().unwrap_or_else(|| DiagramState {
            library: plane_library(&self.full_library, dim),
            block_idx: 0,
            chars: HashMap::new(),
            selected_char: None,
        });
        self.library = next.library;
        self.block_idx = next.block_idx;
        self.chars = next.chars;
        self.selected_char = next.selected_char;
        self.diagram_dim = dim;
        self.ensure_block_state();
        if fresh {
            self.diag_bound[idx(dim)] = None;
            self.refresh_live_block();
        }
    }

    pub fn current_dimension(&self) -> Dimension {
        self.diagram_dim
    }

    // ---------- data loading ----------
    pub fn open_fvb(&mut self) {
        if let Some(path) = crate::dialogs::file_dialog()
            .add_filter("FastView library", &["fvb"])
            .pick_file()
        {
            match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| fvb_core::load_fvb_bytes(&b).map_err(|e| e.to_string()))
            {
                Ok(lib) => {
                    self.library = plane_library(&lib, self.diagram_dim);
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
        // String 328 (`0x447e30` when opening); patterns in both cases for
        // the case-sensitive dialogs off Windows.
        if let Some(path) = crate::dialogs::file_dialog()
            .add_filter("Осциллограммы (*.cfg, *.cff, *.osc, *.sto)", &["cfg", "cff", "osc", "sto", "CFG", "CFF", "OSC", "STO"])
            .add_filter("Осциллограммы COMTRADE (*.cfg)", &["cfg", "CFG"])
            .add_filter("Осциллограммы COMTRADE'13 (*.cff)", &["cff", "CFF"])
            .add_filter("Осциллограммы OSC2 (*.osc)", &["osc", "OSC"])
            .add_filter("Осциллограммы Configurator-MT (*.sto)", &["sto", "STO"])
            .add_filter("Все файлы (*.*)", &["*"])
            .pick_file()
        {
            self.load_comtrade_path(&path);
        }
    }

    /// Open a record as a new document. Parsing runs on a worker thread
    /// (`LoadingDlg`, DIALOG 492); the progress dialog shows only for large
    /// files and «Отмена» abandons the open.
    pub fn load_comtrade_path(&mut self, path: &std::path::Path) {
        // MFC opens by AfxFullPath (GetFullPathName): the record's path is
        // the full one in backslashes (VM: the ОМП report's
        // «Осциллограмма:» line for a record opened as C:/…).
        let full = if cfg!(windows) { std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()) } else { path.to_path_buf() };
        let path = full.as_path();
        // MFC's OpenDocumentFile (the stock document manager — CDocManagerEx
        // overrides only the file dialog): a file already open just brings
        // its document forward (AfxComparePath, case-insensitive), for the
        // dialog, the MRU, a drop and the hand-over alike.
        let same = |p: &std::path::Path| same_file(p, path);
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref().is_some_and(same)) {
            self.active = Some(i);
            return;
        }
        if self.loading.iter().any(|l| same(&l.path)) {
            return;
        }
        // Past the `.sto` step every open empties `-Dst` (`0x44f05a`); a
        // `.sto` stops before it here (its converter ⛔).
        if !path.to_string_lossy().to_lowercase().ends_with(".sto") {
            self.dst_file = None;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let (p, enc) = (path.to_path_buf(), self.encoding);
        let progress = std::sync::Arc::new(fvb_core::comtrade::Progress::default());
        let prog = progress.clone();
        std::thread::spawn(move || {
            let r = open_file(&p, enc, Some(&prog));
            // A cancelled open is abandoned silently (`0x44f391`) — but an
            // error the reader posted still shows (the `.cfg` loader's
            // zero-sample check, `0x450bc1`: a `.cff` cancelled during its
            // split, which polls nothing).
            if !prog.cancelled() || r.is_err() {
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
            Ok((doc, cff)) => {
                self.add_doc(*doc);
                let enc = self.encoding;
                if let Some(d) = self.active_doc_mut() {
                    d.path = Some(path.to_path_buf());
                    d.rec_path = Some(path.to_path_buf());
                    d.cff = cff;
                    d.init_record_text(enc);

                    // Names may have been re-decoded; no flag is set yet.
                    d.flagged = d.osc.clone();
                    d.series_gen += 1;
                }
                if let Some(di) = self.active {
                    self.load_record_settings(di);
                    // OnOpenDocument selects the record's channel 0 once the
                    // loader is done (`0x44f245`, `.cff` `0x44f358`).
                    let d = &mut self.docs[di];
                    if !d.osc.analog.is_empty() {
                        d.selection.insert(ChanKey::Analog(0));
                    } else if !d.osc.digital.is_empty() {
                        d.selection.insert(ChanKey::Digital(0));
                    }
                }
                self.remember_recent(path);
            }
            // The reader's exception is posted to the frame (`0x475680`,
            // message 0x40a) and reported in an MB_OK box (`0x4756e0`): its
            // text "<message> - <file>" (`0x42dfe0`, the file as detail).
            Err(fvb_core::comtrade::ComtradeError::Plain(m)) => self.dlg.push_alert(m),
            Err(ref e @ fvb_core::comtrade::ComtradeError::FormatAt(ref d)) => self.dlg.push_alert(format!("{e} - {d}")),
            Err(e) => self.dlg.push_alert(format!("{e} - {}", path.display())),
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
        let Some(path) = crate::dialogs::file_dialog()
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
        let Some(path) = crate::dialogs::file_dialog()
            .add_filter("ZIP", &["zip"])
            .set_file_name(format!("{}.zip", d.name))
            .save_file()
        else {
            return;
        };
        let (cfg, dat) = fvb_core::comtrade::write(&d.osc, d.codepage);
        let csv = Self::csv_string(d);
        let zip = crate::util::zip_store(&[
            (format!("{}.cfg", d.name), cfg),
            (format!("{}.dat", d.name), dat),
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
        if let Some(path) = crate::dialogs::file_dialog()
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
            dig_flags: d.dig_flags.clone(),
            name_raw: d.name_raw.clone(),
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
        d.dig_flags = s.dig_flags;
        d.name_raw = s.name_raw;
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
        let Some(d) = self.active_doc_mut() else {
            self.status = "Сначала загрузите осциллограмму.".into();
            return None;
        };
        // The new definitions join the list and `0x415780` evaluates the
        // whole list: the older formula channels take the current values
        // and correction too (L9.103); an old one failing aborts the batch
        // — its box, nothing added (`0x416ed6`–`0x416f69`).
        if let Err(e) = d.try_reevaluate_formulas(show_primary) {
            self.dlg.push_alert(e.to_string());
            return None;
        }
        let existing: Vec<_> = d.computed.iter().map(Computed::as_core).collect();
        match fvb_core::quick::evaluate_batch_with(&d.flagged, fvb_core::freqcorr::of(&d.osc), &existing, batch, show_primary) {
            Ok(added) => {
                let d = self.active_doc_mut()?;
                let n0 = d.computed.len();
                let names = added.iter().map(|c| c.name.clone()).collect();
                for (k, c) in added.into_iter().enumerate() {
                    let color = channel_color(&c.name, "", d.osc.analog.len() + n0 + k);
                    let mut cc = Computed::new(c.name, c.formula, Vec::new(), color);
                    cc.unit = c.unit;
                    cc.set_values(c.samples, c.phasors);
                    d.computed.push(cc);
                }
                d.channels_changed();
                Some(names)
            }
            // A computed channel's evaluation failure is a box (`0x415780`
            // → `0x475680`).
            Err(e) => {
                self.dlg.push_alert(e.to_string());
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
        // `0x446280` (from «Рассчитать», `0x417426`): with Iдиф / Iторм
        // pairs the differential readout goes on — `ViewChannelType` 1, the
        // document modified — unless it already is.
        if !g.dif_pairs.is_empty() && !self.diag.diff_values {
            self.diag.diff_values = true;
            if let Some(d) = self.active_doc_mut() {
                d.set.modified = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L9.65: a record with a «Гц» channel opens corrected; «Не
    /// корректировать» recomputes with the fixed window; the next channel
    /// change turns the correction back on.
    /// A record whose data gives no sample (an empty `.dat`; the same as a
    /// `.cff` cancelled during its split) fails with string 330 and the
    /// record's path less its extension (`0x450bc1`, `0x475680`); cancelled,
    /// the error still shows (L9.268).
    #[test]
    fn a_record_without_samples_reports_the_wrong_format() {
        let dir = std::env::temp_dir().join(format!("fv_nosamples_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = "S,D,1999\r\n1,1A,0D\r\n1,X,,,V,1,0,0,-9,9\r\n50\r\n1\r\n1000,3\r\n01/01/2024,00:00:00.000000\r\n01/01/2024,00:00:00.000000\r\nASCII\r\n1\r\n";
        std::fs::write(dir.join("rec.cfg"), cfg).unwrap();
        std::fs::write(dir.join("rec.dat"), b"").unwrap();
        for cancel in [false, true] {
            let mut app = App::new();
            app.load_comtrade_path(&dir.join("rec.cfg"));
            if cancel {
                app.loading[0].progress.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            while !app.loading.is_empty() {
                app.poll_loading();
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(app.docs.is_empty());
            assert_eq!(app.dlg.alert(), Some(format!("Неправильный формат файла - {}", dir.join("rec").display()).as_str()), "cancelled: {cancel}");
        }
    }

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
        // The rebuild turns it back on without refreshing the caches
        // (L9.103): the statistics read it live, the phasors keep theirs.
        let stale = d.analysis.phasors[3][k];
        d.channels_changed();
        assert_eq!(d.osc.freq_sel, 1);
        assert_eq!(d.analysis.phasors[3][k], stale);
        assert_eq!(d.stat_rms(3, k), on.1);
        d.recompute_signals();
        assert_eq!((d.analysis.phasors[3][k], d.rms_at(3, k)), on);
    }

    /// L9.103: an implicit selection change leaves a formula channel's
    /// samples as they were; the next «Рассчитать» (`0x415780` over the whole
    /// list) re-evaluates it with the current correction.
    #[test]
    fn a_stale_formula_channel_is_refreshed_by_the_next_calculation() {
        let cfg = DEMO_CFG.replacen("2,Ub,B,,V,", "2,Ub,B,,Гц,", 1);
        let osc = fvb_core::comtrade::parse(&cfg, DEMO_DAT).unwrap();
        let mut app = App::new();
        app.docs.push(Doc::new(osc, "f".into()));
        app.active = Some(0);
        app.docs[0].osc.freq_sel = 0;
        app.docs[0].recompute_signals();
        app.append_computed(&[("X".into(), "rms:(Ua)".into())]).unwrap();
        let k = 300;
        let uncorrected = app.docs[0].computed[0].samples[k];
        app.docs[0].channels_changed();
        assert_eq!(app.docs[0].osc.freq_sel, 1);
        assert_eq!(app.docs[0].computed[0].samples[k], uncorrected, "stale until recalculated");
        app.append_computed(&[("Y".into(), "Ua*2".into())]).unwrap();
        assert_ne!(app.docs[0].computed[0].samples[k], uncorrected, "re-evaluated with the correction");
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
        app.docks = crate::docking::Docks::one_group();
        assert!(Panes::GROUP.iter().all(|&p| app.panes.visible(p)) && app.panes.frequency);
        assert_eq!(app.panes.shown_tab(), Some(Pane::Vector), "the vector pane is the active tab");
        app.dispatch(Command::TogglePane(Pane::Vector));
        assert!(!app.panes.vector);
        assert_eq!(app.panes.shown_tab(), Some(Pane::Circle));
        assert_eq!(app.current_dimension(), Dimension::Ohm);
        app.dispatch(Command::TogglePane(Pane::Differential));
        app.dispatch(Command::TogglePane(Pane::Differential));
        assert_eq!(app.panes.shown_tab(), Some(Pane::Circle), "a re-shown tab stays behind the front one");
        app.dispatch(Command::TogglePane(Pane::Circle));
        assert_eq!(app.panes.shown_tab(), Some(Pane::Differential), "the nearest visible tab on the right");
        assert_eq!(app.current_dimension(), Dimension::Dif);
        app.dispatch(Command::TogglePane(Pane::Differential));
        app.dispatch(Command::TogglePane(Pane::Vector));
        assert_eq!(app.panes.shown_tab(), Some(Pane::Vector), "the only one shown comes to the front");
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

    /// `0x4554e0` / `0x455210`: a pair naming a channel the record lacks
    /// goes; names compare case-sensitively.
    #[test]
    fn stale_pairs_are_purged() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.rx_pairs = vec![crate::diagram::ChanPair::new("Ua", "Ia"), crate::diagram::ChanPair::new("Ua", "нет")];
        d.dif_pairs = vec![crate::diagram::ChanPair::new("ia", "Ib")];
        d.purge_pairs();
        assert_eq!(d.rx_pairs, vec![crate::diagram::ChanPair::new("Ua", "Ia")]);
        assert!(d.dif_pairs.is_empty(), "«ia» is not «Ia»");
    }

    /// «Быстрый анализ» starts from the defaults on every open (`0x48b690`
    /// → `0x411f40`, then the `.set`): combos by name (missing → «Канал
    /// отсутствует»), Uни the first channel, K0 «0.».
    #[test]
    fn quick_analysis_pages_start_from_defaults() {
        use fvb_core::addformula::NO_CHANNEL;
        let mut app = demo_app();
        app.dispatch(Command::QuickAnalysis);
        let f = &app.quick[0];
        assert_eq!((f.ia.as_str(), f.k0.as_str()), ("Ia", "0."));
        assert_eq!(f.uab, NO_CHANNEL, "no «Uab» in the record");
        let first = app.active_doc().unwrap().osc.analog[0].id.clone();
        assert_eq!(f.uni, first);
        app.quick[0].ia = "Ib".into();
        app.quick_open = false;
        app.dispatch(Command::QuickAnalysis);
        assert_eq!(app.quick[0].ia, "Ia", "not the previous session's value");
    }

    /// A formula may name a digital channel (`0x42edf0` has no kind test):
    /// its value is 1 / 0 through the accessor, inverted with the flag; its
    /// RMS is 0 (`0x4272ae`).
    #[test]
    fn formulas_read_digital_channels() {
        let mut app = demo_app();
        let d = app.active_doc().unwrap();
        let name = d.osc.digital[0].id.clone();
        let raw: Vec<f64> = d.osc.digital[0].samples.iter().map(|&b| f64::from(u8::from(b))).collect();
        assert!(app.append_computed(&[("D2".into(), format!("2*\"{name}\"")), ("DR".into(), format!("rms:(\"{name}\")"))]).is_some());
        let d = app.active_doc().unwrap();
        assert!(d.computed[0].samples.iter().zip(&raw).all(|(v, r)| *v == 2.0 * r), "2 × the state");
        assert!(d.computed[1].samples.iter().all(|v| *v == 0.0), "no RMS for a digital channel");
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Digital(0));
        app.dispatch(Command::ToggleFlag(crate::chansel::ChanFlag::Invert));
        let d = app.active_doc().unwrap();
        assert!(d.computed[0].samples.iter().zip(&raw).all(|(v, r)| *v == 2.0 * (1.0 - r)), "inverted");
    }

    /// A computed channel carries invert / DC / the display mode like a file
    /// channel (`0x428490` with bit 13): its values negated / DC-removed,
    /// its stored phasor negated by invert, a later formula reading it
    /// flagged, the flags kept across re-evaluation; RMS mode draws the
    /// one-period RMS.
    #[test]
    fn computed_channels_carry_the_value_flags() {
        use crate::chansel::ChanFlag;
        let mut app = demo_app();
        let k = 300usize;
        let name = app.active_doc().unwrap().osc.analog[0].id.clone();
        assert!(app.append_computed(&[("x2".into(), format!("2*\"{name}\""))]).is_some());
        assert!(app.append_computed(&[("x4".into(), "2*\"x2\"".into())]).is_some());
        let d = app.active_doc().unwrap();
        let (v, z, v4) = (d.computed[0].samples[k], d.computed[0].phasors.as_ref().map(|p| p[k]), d.computed[1].samples[k]);
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Computed(0));
        assert!(app.active_doc().unwrap().flag_state(ChanFlag::Invert).enabled);
        app.dispatch(Command::ToggleFlag(ChanFlag::Invert));
        let d = app.active_doc().unwrap();
        assert!((d.computed[0].samples[k] + v).abs() < 1e-9, "negated");
        if let (Some(z), Some(p)) = (z, d.computed[0].phasors.as_ref()) {
            assert!((p[k] + z).norm() < 1e-9, "the stored phasor too");
        }
        assert!((d.computed[1].samples[k] + v4).abs() < 1e-9, "a later formula reads it flagged");
        app.dispatch(Command::ToggleFlag(ChanFlag::Invert));
        app.dispatch(Command::ToggleFlag(ChanFlag::RemoveDc));
        let d = app.active_doc().unwrap();
        let mean = whole_period_mean(&d.computed[0].raw, d.osc.samples_per_cycle().max(2));
        assert!((d.computed[0].samples[k] - (v - mean)).abs() < 1e-9, "minus the whole-period mean");
        app.dispatch(Command::SetValueMode(ValueMode::Rms));
        let d = app.active_doc().unwrap();
        assert!(d.mode_state(ValueMode::Rms).checked);
        let n = d.osc.samples_per_cycle().max(2);
        let want = fvb_core::freqcorr::rms_at(fvb_core::freqcorr::of(&d.osc), fvb_core::freqcorr::OTHER, &d.computed[0].samples, k, n, 1);
        assert!((d.computed[0].shown(k) - want).abs() < 1e-9, "RMS mode shows the one-period RMS");
        assert!(d.computed[0].remove_dc, "kept across the re-evaluation");
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

    /// A channel pasted from a record at twice the rate survives
    /// «Объединить» (VM: the original keeps it, the record at the higher
    /// rate).
    #[test]
    fn a_channel_pasted_at_another_rate_survives_the_merge() {
        let mut app = demo_app();
        let mut osc = app.active_doc().unwrap().osc.clone();
        osc.sample_rate *= 2.0;
        osc.time.iter_mut().for_each(|t| *t /= 2.0);
        app.docs.push(Doc::new(osc, "fast".into()));
        let (base, fast) = (app.active.unwrap(), app.docs.len() - 1);
        app.active = Some(fast);
        app.docs[fast].selection.extend([ChanKey::Analog(0)]);
        app.dispatch(Command::Copy);
        app.active = Some(base);
        let n = app.docs[base].osc.analog.len();
        app.dispatch(Command::Paste);
        assert!(!app.docs[base].pending.is_empty(), "pasted");
        let picked = app.docs[base].selection.clone();
        app.merge_records();
        let d = &app.docs[base];
        assert_eq!(d.osc.analog.len(), n + 1, "the pasted channel merged: {:?}", d.osc.analog.iter().map(|c| &c.id).collect::<Vec<_>>());
        // The pasted lane was selected: its merged channel is (VM).
        assert!(picked.iter().any(|k| matches!(k, ChanKey::Computed(_))), "the paste selects its lane");
        let merged = d.osc.analog.iter().position(|c| c.id.contains("(вст.)")).expect("the merged channel");
        assert!(d.selection.contains(&ChanKey::Analog(merged)), "still selected after the merge");
        assert!(!d.selection.iter().any(|k| matches!(k, ChanKey::Computed(_))));
    }

    /// The ribbon's update rules (`0x45c8c0`, `0x42fc60`, `0x475750`,
    /// `0x45bf60`, `0x42ffb0`).
    #[test]
    fn update_rules_as_read() {
        let empty = App::new();
        assert_eq!(empty.zoom_enabled(), [false; 4], "no document: zoom greyed");
        assert!(!empty.can_save());
        let mut app = demo_app();
        app.view_width = 1000.0;
        app.y_analog = 500.0;
        let d = app.active_doc_mut().unwrap();
        d.xscale = Some(1.0);
        assert_eq!(app.zoom_enabled(), [false, true, false, true], "at the limits: YAnalogSize 500, f = 1");
        app.y_analog = 70.0;
        let fit = crate::oscgeom::fit_scale(1000.0, app.active_doc().unwrap().osc.sample_count());
        app.active_doc_mut().unwrap().xscale = Some(fit);
        assert_eq!(app.zoom_enabled(), [true, false, true, false], "70 px and the fit scale");
        let d = app.active_doc_mut().unwrap();
        assert!(!d.mode_state(ValueMode::Instant).enabled, "no selection: modes greyed");
        assert!(d.stats_row_state(0).enabled);
        assert!(!d.can_copy());
        d.selection.insert(ChanKey::Digital(0));
        assert!(d.can_copy(), "a digital channel is copied");
        assert!(d.mode_state(ValueMode::Instant).enabled);
        assert_eq!(d.primary_shown(), Some(false));
        d.chans.clear();
        assert_eq!(d.primary_shown(), None, "no analog channel: neither checked");
        app.active_doc_mut().unwrap().path = Some("x/REC.STO".into());
        assert!(!app.can_save(), ".sto: Save greyed");
        app.active_doc_mut().unwrap().path = Some("x/rec.cfg".into());
        assert!(app.can_save());
    }

    /// Invert / DC / modes on a digital channel (`0x42f4b0`, `0x42fc60`,
    /// `0x428490`): enabled by a digital selection alone; invert flips the
    /// displayed state; the merge bakes it and keeps the flag.
    #[test]
    fn digital_channels_carry_the_value_flags() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        let raw: Vec<bool> = d.osc.digital[0].samples.clone();
        d.selection.insert(ChanKey::Digital(0));
        let st = d.flag_state(crate::chansel::ChanFlag::Invert);
        assert!(st.enabled && !st.checked, "a digital selection enables 32823");
        app.dispatch(Command::ToggleFlag(crate::chansel::ChanFlag::Invert));
        let d = app.active_doc().unwrap();
        assert!(d.dig(0).invert && d.flag_state(crate::chansel::ChanFlag::Invert).checked);
        assert!((0..raw.len()).all(|k| d.dig_on(0, k) != raw[k]), "drawn inverted");
        app.dispatch(Command::SetValueMode(ValueMode::Rms));
        let d = app.active_doc().unwrap();
        assert_eq!(d.dig(0).mode, ValueMode::Rms);
        assert!(d.mode_state(ValueMode::Rms).checked, "the bits are kept for the check");
        // «Объединить»: the inverted values baked, the flag kept — shown
        // as the raw record again.
        app.active_doc_mut().unwrap().selection = BTreeSet::from([ChanKey::Analog(0)]);
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        app.merge_records();
        let d = app.active_doc().unwrap();
        assert!(d.dig(0).invert);
        assert!(d.osc.digital[0].samples.iter().zip(&raw).take(10).all(|(m, r)| *m != *r), "baked");
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

    /// `0x42b520` / `0x42b0c0`: phase field first, then name substrings in
    /// order, then the 10-colour table by vector place.
    #[test]
    fn channel_colours_by_phase_name_and_place() {
        assert_eq!(phase_class("X", "A"), 1);
        assert_eq!(phase_class("X", "б"), 2);
        assert_eq!(phase_class("X", "CA"), 7);
        assert_eq!(phase_class("Uab", ""), 5, "uab before ua");
        assert_eq!(phase_class("IC БЛОКА", ""), 3);
        assert_eq!(phase_class("3I0", ""), 4);
        assert_eq!(phase_class("Uп", ""), 4, "Cyrillic п for n");
        assert_eq!(phase_class("Pin 7", ""), 4, "any «in» left over");
        assert_eq!(phase_class("F", ""), 0);
        assert_eq!(channel_color("F", "", 13), egui::Color32::from_rgb(0, 111, 122), "table entry 3");
        assert_eq!(channel_color("Ia", "", 13), egui::Color32::from_rgb(192, 192, 0));
    }

    /// The block state lives in the document's `.set` tree (`0x408cf0` /
    /// `0x409030`): saved on an edit, picked and loaded on a refresh.
    #[test]
    fn block_state_is_the_documents() {
        let mut app = demo_app();
        assert_eq!(app.library.blocks[app.block_idx].name, NO_BLOCK, "no section: «Нет»");
        app.block_idx = 1;
        app.ensure_block_state();
        let blk = app.library.blocks[1].clone();
        let ch = &blk.characteristics[0];
        let p = ch.parameters[0].name.clone();
        let st = app.chars.get_mut(&(1, 0)).unwrap();
        st.settings.insert(p.clone(), "1+1".into());
        st.visible = false;
        app.active_doc_mut().unwrap().set.modified = false;
        app.save_block();
        let d = app.active_doc().unwrap();
        assert!(d.set.modified);
        let sec = d.set.tree.section("Circle Chart").unwrap();
        assert_eq!(sec.get_str("SelectedBlock", ""), blk.name);
        assert!(!sec.attrs.contains_key("SightsOnly"), "the section emptied first");
        // A second document with no section: «Нет»; back: the block and its values.
        let osc = fvb_core::comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        app.add_doc(Doc::new(osc, "b".into()));
        app.refresh_blocks();
        assert_eq!(app.block_idx, 0);
        app.chars.clear();
        app.active = Some(0);
        app.refresh_blocks();
        assert_eq!(app.block_idx, 1);
        let st = &app.chars[&(1, 0)];
        assert_eq!(st.settings[&p], "1+1");
        assert!(!st.visible);
        assert_eq!(app.block_text, None);
        // A saved name the library lacks: block 0, the cell shows the name
        // (`0x407814`, `0x436d3b`).
        app.active_doc_mut().unwrap().set.tree.section_mut("Circle Chart").set_str("SelectedBlock", "Неизвестный");
        app.refresh_blocks();
        assert_eq!((app.block_idx, app.block_text.as_deref()), (0, Some("Неизвестный")));
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
        assert_eq!(names(&app), ["Y", "Z"], "the new channel at the analog-kind block's end (`0x42ea36`)");
        assert_eq!(d.computed[0].formula, "Ub*3");
        assert_eq!(d.computed[0].color, y_color, "updated in place");
        // «Отмена» keeps a deletion.
        app.dispatch(Command::QuickAnalysis);
        app.formula_page.selected = Some(0);
        app.fp_delete();
        app.quick_open = false;
        assert_eq!(names(&app), ["Z"], "row 0 is Y, in record order");
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

    /// A `.sto` (`0x4563b0`): never the `.cfg` beside it; string 124 with
    /// the name ending in a lowercase ".sto", shown as it is.
    #[test]
    fn a_sto_record_reports_124_alone() {
        let mut app = App::new();
        let p = std::path::Path::new("/r/Запись.STO");
        let r = open_file(p, Encoding::Cp1251, None);
        assert!(matches!(&r, Err(fvb_core::comtrade::ComtradeError::Plain(m)) if m == "Ошибка чтения файла /r/Запись.sto"));
        app.finish_load(p, r);
        assert_eq!(app.dlg.alert(), Some("Ошибка чтения файла /r/Запись.sto"), "no \" - <file>\" detail");
        assert!(app.docs.is_empty());
    }

    /// Phase 2 of the open (`0x428490` under `0x44f5b0`) runs on the
    /// loader's thread: the worker hands over a document whose derived
    /// arrays are built.
    #[test]
    fn the_loader_builds_the_derived_arrays() {
        let dir = std::env::temp_dir().join(format!("fv_open_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("r.cfg");
        std::fs::write(&cfg, "S,1,1999\n1,1A,0D\n1,Ia,A,,A,1,0,0,-1000,1000\n50\n1\n1000,40\n01/01/2000,00:00:00.000000\n01/01/2000,00:00:00.000000\nASCII\n").unwrap();
        let rows: String = (0..40).map(|k| format!("{},{},{}\n", k + 1, k * 1000, ((k as f64) * std::f64::consts::PI / 10.0).sin() * 100.0)).collect();
        std::fs::write(dir.join("r.dat"), rows).unwrap();
        let (doc, _) = open_file(&cfg, Encoding::Cp1251, None).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(doc.name, "r");
        assert_eq!(doc.rms.len(), 1);
        assert_eq!(doc.rms[0].len(), 40);
        assert!(doc.rms[0][39] > 60.0, "{}", doc.rms[0][39]);
    }

    /// `0x416ed6`: quick analysis evaluates the whole list; an old formula
    /// that fails aborts it with its box and nothing is added.
    #[test]
    fn a_failing_old_formula_aborts_quick_analysis() {
        let mut app = demo_app();
        assert!(app.append_computed(&[("X".into(), "Ua*2".into())]).is_some());
        app.active_doc_mut().unwrap().computed[0].formula = "Нет*2".into();
        assert!(app.append_computed(&[("Y".into(), "Ub*2".into())]).is_none());
        assert_eq!(app.dlg.alert(), Some("Имя канала задано неправильно."), "its box (0x162, no formula: VM)");
        assert!(app.active_doc().unwrap().computed.iter().all(|c| c.name != "Y"), "nothing added");
    }

    #[test]
    fn bad_setting_reports_the_formula_and_keeps_old_geometry() {
        let mut app = demo_app();
        app.dispatch(Command::ActivatePane(Pane::Circle));
        app.block_idx = 1;
        app.ensure_block_state();
        let key = (app.block_idx, 0);
        let before = app.chars[&key].figs.clone();
        let name = app.library.blocks[app.block_idx].characteristics[0].parameters[0].name.clone();
        app.chars.get_mut(&key).unwrap().settings.insert(name, "1+".into());
        app.recompute_block();
        assert_eq!(app.dlg.alert(), Some("1+: Синтаксическая ошибка в формуле"), "an MB_OK box (`0x409260` posts it)");
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
        let s = app.cursor_status().unwrap();
        assert_eq!(s.len(), 6);
        assert!(s.iter().all(|x| !x.is_empty()));
    }

    /// `0x49484d`: with a trigger sample (`.osc`) the readouts count from
    /// it — a визир on it reads 0 and sample 1.
    #[test]
    fn cursor_status_counts_from_the_trigger() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.osc.trigger_sample = 20;
        (d.main_cursor, d.aux_cursor, d.v_shown) = (20, 30, [true, true, false]);
        app.time_unit = TimeUnit::Milli;
        assert_eq!(app.cursor_status().unwrap()[0], "Верт. визир 1: 0 мс");
        app.time_unit = TimeUnit::Samples;
        let s = app.cursor_status().unwrap();
        assert_eq!((s[0].as_str(), s[1].as_str()), ("Верт. визир 1: 1", "Верт. визир 2: 11"));
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
        let s = app.cursor_status().unwrap();
        assert_eq!(s[0], format!("Верт. визир 1: {}", crate::oscgeom::time_of_day_text(46_799.5 + dt)));
        assert_eq!(s[1], "Верт. визир 2: 12:59:59.500");
        assert_eq!(s[2], format!("Разница. верт. визиров: {}", crate::oscgeom::status_abs_diff(dt)));
        // A start time without ':' (`0x494c31`): the shown V panes stay empty.
        app.active_doc_mut().unwrap().osc.stamps.start_time = "125959".into();
        let s = app.cursor_status().unwrap();
        assert!(s[0].is_empty() && s[1].is_empty() && s[2].is_empty());
    }

    /// The H panes (3171 / 3173 «… %4g %s», 3429 «… %g %s%s», `0x4960f0`):
    /// the stored value unscaled, padded to 4, the plain unit; the
    /// difference SI-prefixed; nothing of the unit in ADC mode.
    #[test]
    fn cursor_status_horizontal_panes() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        let unit = d.osc.analog[0].shown_unit().text;
        d.hviz[0] = HVizir { shown: true, chan: Some(0), value: 5.0 };
        d.hviz[1] = HVizir { shown: true, chan: Some(0), value: 2005.0 };
        let s = app.cursor_status().unwrap();
        assert_eq!(s[3], format!("Гор. визир 1:    5 {unit}"));
        assert_eq!(s[4], format!("Гор. визир 2: 2005 {unit}"));
        assert_eq!(s[5], format!("Разница г. визиров: 2 к{unit}"));
        app.active_doc_mut().unwrap().chans[0].adc_counts = true;
        let s = app.cursor_status().unwrap();
        assert_eq!((s[3].as_str(), s[5].as_str()), ("Гор. визир 1:    5 ", "Разница г. визиров: 2000 "));
        // No document: the panes keep what they showed.
        assert!(App::new().cursor_status().is_none());
    }
}
