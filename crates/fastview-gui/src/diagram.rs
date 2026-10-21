//! Pure model of the diagram windows (ledger L9, `UI_BEHAVIOUR_SPEC` §6):
//! the circle / differential plane's zoom-pan state and scale
//! (`CircleDiagramWnd` `0x438b50`, `0x4393b0`, `0x437ae0`, `0x437b90`,
//! `0x437ca0`), the characteristic palette (`0x43a4a0`), diagram eligibility
//! by unit (`0x430650`, `0x43d220`, `0x437150`), the context-menu contents
//! (MENU 387 / 388 / 355 / 351) and the vector / frequency layout rules.
//! Painting lives in `diagram_ui.rs`.

use crate::popmenu::{Entry, Mark};

/// Grid cell of the circle / differential plane: `LOGPIXELSX · scale / 5`
/// (`0x438b50`), 19 px at 96 dpi.
pub const CELL_PX: i32 = 96 / 5;

/// Pixel extent of the shown characteristics above which zoom-in is refused
/// (`0x4379e0`) and above which painting resets the zoom to 1 (`0x43b570`).
pub const ZOOM_IN_LIMIT_PX: f64 = 400.0;
pub const ZOOM_RESET_PX: f64 = 800.0;

/// A characteristic's colours: (operate, reset, fill) by `index mod 9`
/// (`0x43a4a0`; the original indexes by characteristic field `+0x68`, whose
/// meaning was not decoded — we use the characteristic's ordinal).
pub const CHAR_PALETTE: [[(u8, u8, u8); 3]; 9] = [
    [(1, 37, 255), (88, 112, 253), (25, 245, 253)],
    [(202, 1, 252), (225, 119, 251), (202, 1, 252)],
    [(34, 177, 76), (50, 230, 29), (34, 177, 76)],
    [(254, 0, 0), (254, 159, 159), (252, 66, 101)],
    [(0, 162, 232), (153, 217, 234), (0, 162, 232)],
    [(163, 73, 164), (200, 191, 231), (163, 73, 164)],
    [(63, 72, 204), (112, 146, 190), (63, 72, 204)],
    [(163, 73, 164), (255, 201, 14), (163, 73, 164)],
    [(136, 0, 21), (185, 122, 87), (136, 0, 21)],
];

/// Alpha of the operate-region fill (GDI+ fill at 50/255).
pub const FILL_ALPHA: u8 = 50;

/// A channel pair of the circle diagram (R | X, `doc+0x884`) or of the
/// differential diagram (Iдиф | Iторм, `doc+0x878`): two channel names and
/// the «enabled» byte (`+8`) toggled from the Каналы submenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChanPair {
    pub a: String,
    pub b: String,
    pub on: bool,
}

impl ChanPair {
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> ChanPair {
        ChanPair { a: a.into(), b: b.into(), on: true }
    }

    /// Menu text (`0x43d220`): «a | b».
    pub fn menu_label(&self) -> String {
        format!("{} | {}", self.a, self.b)
    }

    /// Legend text (`0x438210`): the two names joined by one space.
    pub fn legend_label(&self) -> String {
        format!("{} {}", self.a, self.b)
    }
}

// ---- eligibility by unit -------------------------------------------------

/// Vector diagram (CMDUI 32850 in `0x430650`, pane list `0x492a00`): units
/// "A", "V", «ном» (string 162) or none.
pub fn vector_unit_ok(unit: &str) -> bool {
    ["A", "V"].iter().any(|u| unit.eq_ignore_ascii_case(u)) || unit.to_lowercase() == "ном" || unit.is_empty()
}

/// Circle-diagram flag of the oscillogram's channel menu (CMDUI 32851 in
/// `0x430650`): "Ohm" (string 3060), "W", "VA" or "var".
pub fn circle_flag_unit_ok(unit: &str) -> bool {
    ["Ohm", "W", "VA", "var"].iter().any(|u| unit.eq_ignore_ascii_case(u))
}

/// The circle pane's channel list, «Показать все» and the attach-time clear
/// (`0x43d220`, `0x437150`, `0x436b30`): "Ohm" or "VA" only.
pub fn circle_pane_unit_ok(unit: &str) -> bool {
    ["Ohm", "VA"].iter().any(|u| unit.eq_ignore_ascii_case(u))
}

/// A circle-diagram locus is on the impedance scale when its unit is "Ohm"
/// (`0x438b50`), else on the power scale.
pub fn is_ohm(unit: &str) -> bool {
    unit == "Ohm"
}

// ---- circle / differential plane ----------------------------------------

/// Zoom and pan of a circle / differential window (`+0x4a8` z, `+0x4a0` ox,
/// `+0x4a4` oy, pans in grid cells; `oy` positive moves the origin down,
/// `ox` positive right).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneView {
    pub z: f64,
    pub ox: i32,
    pub oy: i32,
    /// The differential window's one-shot initial pan to the lower left
    /// (ctor flag `+0x4c0`), done after its first paint.
    pub initial_pending: bool,
}

impl Default for PlaneView {
    fn default() -> Self {
        PlaneView { z: 1.0, ox: 0, oy: 0, initial_pending: false }
    }
}

/// The scale of one paint (`0x438b50`): grid cell, the number of cells from
/// the centre to the nearer edge, and per scale (`[0]` Ω, `[1]` power) the
/// units per cell and pixels per unit (`None` when the extent is 0: the
/// original then divides by zero and draws nothing but the grid).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneScale {
    pub cell: i32,
    pub cells: i32,
    pub upc: [f64; 2],
    pub ppu: [Option<f64>; 2],
}

/// One significant digit, rounded up, sign kept (`0x438b50`).
pub fn nice_up(v: f64) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return 0.0;
    }
    let p = 10f64.powf(v.abs().log10().floor());
    let r = (v.abs() / p).ceil() * p;
    if v < 0.0 {
        -r
    } else {
        r
    }
}

/// `cells = round(min(w/2, h/2)) / cell` (integer division) and
/// `upc = nice(max / (z · cells))` for both scales.
pub fn plane_scale(w: i32, h: i32, z: f64, max: [f64; 2]) -> PlaneScale {
    let cell = CELL_PX;
    let cells = ((w / 2).min(h / 2)) / cell;
    let upc = max.map(|m| if cells > 0 { nice_up(m / (z * f64::from(cells))) } else { 0.0 });
    let ppu = upc.map(|u| (u > 0.0).then(|| f64::from(cell) / u));
    PlaneScale { cell, cells, upc, ppu }
}

/// The pan limit of one axis (`0x437b90`): `round((span / cell) · z / 2)`
/// with the plot rect's width (x) or height (y).
pub fn max_pan(span: i32, z: f64) -> i32 {
    (f64::from(span / CELL_PX) * z * 0.5).round() as i32
}

/// What a wheel notch does (`0x437ae0`): exactly Ctrl zooms ×1.5 / ×⅔,
/// exactly Shift pans x by 2 cells, anything else pans y by 2 (notch up = +2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelAct {
    Zoom(bool),
    PanX(i32),
    PanY(i32),
}

pub fn wheel_action(up: bool, ctrl: bool, shift: bool) -> WheelAct {
    let step = if up { 2 } else { -2 };
    match (ctrl, shift) {
        (true, false) => WheelAct::Zoom(up),
        (false, true) => WheelAct::PanX(step),
        _ => WheelAct::PanY(step),
    }
}

impl PlaneView {
    /// Pan one axis by `delta` cells, refused when `|o + delta| > limit`.
    pub fn pan(&mut self, x_axis: bool, delta: i32, limit: i32) -> bool {
        let o = if x_axis { &mut self.ox } else { &mut self.oy };
        if (*o + delta).abs() > limit {
            return false;
        }
        *o += delta;
        true
    }

    /// Zoom ×1.5 (in) or ×⅔ (out) about the origin (`0x43d580`). Zoom-in is
    /// refused when the characteristics would exceed 400 px
    /// (`char_max · ppuΩ(z') ≥ 400`, `ppu_at` gives the Ω pixels per unit at
    /// a zoom); zoom-out is unlimited; at `z ≤ 1` the pans reset to 0.
    pub fn zoom(&mut self, zoom_in: bool, char_max: f64, ppu_at: impl Fn(f64) -> Option<f64>) -> bool {
        let z = if zoom_in { self.z * 1.5 } else { self.z / 1.5 };
        if zoom_in {
            // An empty plane (no extent) has no finite scale: NaN < 400 fails.
            let Some(ppu) = ppu_at(z) else { return false };
            let px = char_max * ppu;
            if px.is_nan() || px >= ZOOM_IN_LIMIT_PX {
                return false;
            }
        }
        self.z = z;
        if z <= 1.0 {
            self.ox = 0;
            self.oy = 0;
        }
        true
    }

    /// The differential window's initial view: the origin pushed to the
    /// lower left in 2-cell steps (`0x445e00` tail), showing quadrant I.
    pub fn push_lower_left(&mut self, limit_x: i32, limit_y: i32) {
        while self.pan(false, 2, limit_y) {}
        while self.pan(true, -2, limit_x) {}
        self.initial_pending = false;
    }
}

/// Scroll bars of a zoomed plane (`0x437ca0`): hidden at `z ≤ 1`, else range
/// 0..10000 with page `round(10000 / z)`. Returns `M = 10000 − page`.
pub fn scroll_range(z: f64) -> Option<(i32, i32)> {
    (z > 1.0).then(|| {
        let page = (10000.0 / z).round() as i32;
        (page, 10000 - page)
    })
}

/// Thumb position for a pan: `M/2 − o·M/maxPan` (`0x437ca0`).
pub fn scroll_pos(o: i32, limit: i32, m: i32) -> i32 {
    if limit == 0 {
        m / 2
    } else {
        m / 2 - o * m / limit
    }
}

/// Pan for a thumb position: `maxPan·(M/2 − pos)·2/M` (`0x437f00` /
/// `0x437ff0`). Not the inverse of [`scroll_pos`] — the original's factor 2
/// is kept.
pub fn pan_from_scroll(pos: i32, limit: i32, m: i32) -> i32 {
    if m == 0 {
        0
    } else {
        limit * (m / 2 - pos) * 2 / m
    }
}

/// Line / page step of the thumb (±100, `0x437f00`).
pub fn scroll_step(pos: i32, forward: bool, m: i32) -> i32 {
    if forward {
        if pos + 100 < m {
            pos + 100
        } else {
            m - 1
        }
    } else if pos - 100 <= 0 {
        0
    } else {
        pos - 100
    }
}

// ---- even-odd scanline fill ----------------------------------------------

/// Horizontal spans `(y, x0, x1)` covering the even-odd interior of `rings`
/// (screen coordinates) on the pixel rows `y0..y1`, sampled at row centres —
/// the fill of a GDI region clipped to itself (`0x43a4a0`).
pub fn scanline_spans(rings: &[Vec<[f32; 2]>], y0: i32, y1: i32) -> Vec<(i32, f32, f32)> {
    let mut out = Vec::new();
    let mut xs: Vec<f32> = Vec::new();
    for y in y0..y1 {
        let yc = y as f32 + 0.5;
        xs.clear();
        for ring in rings {
            let n = ring.len();
            if n < 3 {
                continue;
            }
            for k in 0..n {
                let [ax, ay] = ring[k];
                let [bx, by] = ring[(k + 1) % n];
                if (ay <= yc) != (by <= yc) {
                    xs.push(ax + (yc - ay) / (by - ay) * (bx - ax));
                }
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for p in xs.as_chunks::<2>().0 {
            if p[1] > p[0] {
                out.push((y, p[0], p[1]));
            }
        }
    }
    out
}

// ---- vector and frequency layout ------------------------------------------

/// Number of concentric circles of the vector grid (`0x490360`):
/// `n = ⌊R/50⌋` → 4 when n < 4, 5 when 4..9, 10 from 10.
pub fn vector_circles(radius: f32) -> usize {
    match (radius / 50.0).floor() as i64 {
        n if n < 4 => 4,
        n if n < 10 => 5,
        _ => 10,
    }
}

/// Frequency x label (`0x46f5a0`): `"%-.0f"` when integral, else `"%-.1f"`.
pub fn freq_x_label(hz: f64) -> String {
    if hz.fract() == 0.0 {
        format!("{hz:.0}")
    } else {
        format!("{hz:.1}")
    }
}

// ---- context menus --------------------------------------------------------

/// A command of a diagram window, by its original id.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagCmd {
    /// 33027 «Создать»: the pair dialog 554.
    CreatePairs,
    /// 32845 / 32846 «Показать все» / «Скрыть все».
    ShowAll(bool),
    /// 35000 + i: toggle channel i's diagram flag (analog then computed).
    ToggleChan(usize),
    /// 35000 + nCh + j (circle) / 35000 + j (differential): toggle pair j.
    TogglePair(usize),
    /// 32865 / 33016 «Отображать характеристики возврата».
    ShowReset,
    /// 32843 / 33015 «Диапазон между визирами».
    BetweenCursors,
    /// 33008 / 33017 (1), 33009 / 33018 (2): readout mode, the checked one
    /// returning to 0; 33028 on the differential diagram toggles.
    Readout(u8),
    /// 32864 / 33019 «Перезагрузить базы характеристик».
    Reload,
    /// 33020 / 33021 zoom ×1.5 / ×⅔.
    Zoom(bool),
    /// 33022..33025: (x axis, delta) — ↑ oy −2, ↓ oy +2, ← ox −2, → ox +2.
    Pan(bool, i32),
    /// 33014 / 33010: pop the Каналы submenu at the pointer.
    ChannelsMenu,
    /// 33011: pop the Отсчет углов submenu at the pointer.
    AngleMenu,
    /// 36000 «Абсолютный угол» (None) / 36001 + i: the angle reference.
    AngleRef(Option<String>),
    /// 32844 / 33012 «Показывать значения» (vector).
    ShowValues,
    /// 32842 / 33013 «Только направление».
    DirectionOnly,
    /// 32863 «Настройки» (frequency): DIALOG 353.
    FreqSettings,
}

/// A channel as a diagram menu lists it: index (analog, then computed),
/// name, unit, the diagram flag.
#[derive(Debug, Clone)]
pub struct MenuChan {
    pub idx: usize,
    pub name: String,
    pub unit: String,
    pub on: bool,
}

/// Insert `e` at `pos` like `InsertMenu(MF_BYPOSITION)`: past the end appends.
fn insert_at(list: &mut Vec<Entry<DiagCmd>>, pos: usize, e: Entry<DiagCmd>) {
    let p = pos.min(list.len());
    list.insert(p, e);
}

fn check(label: impl Into<String>, on: bool, cmd: DiagCmd) -> Entry<DiagCmd> {
    Entry::item(label, true, Mark::Check(on), cmd)
}

/// The Каналы submenu of MENU 387 (`0x43d220`): Создать, Показать все,
/// Скрыть все, ─, then each eligible (Ohm / VA) channel inserted at position
/// `i + 3` and each R | X pair at `nCh + 4 + j` — positions past the end
/// append, as the original's `InsertMenu` calls do.
pub fn circle_channels_menu(doc_open: bool, chans: &[MenuChan], n_ch: usize, pairs: &[ChanPair]) -> Vec<Entry<DiagCmd>> {
    let mut m = vec![
        Entry::item("Создать", doc_open, Mark::None, DiagCmd::CreatePairs),
        Entry::item("Показать все", true, Mark::None, DiagCmd::ShowAll(true)),
        Entry::item("Скрыть все", true, Mark::None, DiagCmd::ShowAll(false)),
        Entry::Sep,
    ];
    for c in chans.iter().filter(|c| circle_pane_unit_ok(&c.unit)) {
        insert_at(&mut m, c.idx + 3, check(c.name.clone(), c.on, DiagCmd::ToggleChan(c.idx)));
    }
    for (j, p) in pairs.iter().enumerate() {
        insert_at(&mut m, n_ch + 4 + j, check(p.menu_label(), p.on, DiagCmd::TogglePair(j)));
    }
    m
}

/// The Каналы submenu of MENU 388: the Iдиф | Iторм pairs (35000 + j)
/// inserted at position 4 + j, after the separator (`0x446d70`).
pub fn diff_channels_menu(doc_open: bool, pairs: &[ChanPair]) -> Vec<Entry<DiagCmd>> {
    let mut m = vec![
        Entry::item("Создать", doc_open, Mark::None, DiagCmd::CreatePairs),
        Entry::item("Показать все", true, Mark::None, DiagCmd::ShowAll(true)),
        Entry::item("Скрыть все", true, Mark::None, DiagCmd::ShowAll(false)),
        Entry::Sep,
    ];
    for (j, p) in pairs.iter().enumerate() {
        insert_at(&mut m, j + 4, check(p.menu_label(), p.on, DiagCmd::TogglePair(j)));
    }
    m
}

/// MENU 387 (circle) or 388 (differential) in resource order.
pub fn plane_menu(diff: bool, channels: Vec<Entry<DiagCmd>>, show_reset: bool, between: bool, readout: u8) -> Vec<Entry<DiagCmd>> {
    let mut m = vec![
        Entry::sub("Каналы", channels),
        Entry::Sep,
        check("Отображать характеристики возврата", show_reset, DiagCmd::ShowReset),
        check("Диапазон между визирами", between, DiagCmd::BetweenCursors),
    ];
    if diff {
        m.push(check("Показывать значения", readout != 0, DiagCmd::Readout(1)));
    } else {
        m.push(check("Показывать вещественную и мнимую части", readout == 1, DiagCmd::Readout(1)));
        // The resource's own spelling.
        m.push(check("Показывавать действующее значение и угол", readout == 2, DiagCmd::Readout(2)));
    }
    m.push(Entry::Sep);
    m.push(Entry::item("Перезагрузить базы характеристик", true, Mark::None, DiagCmd::Reload));
    m
}

/// The Каналы submenu of MENU 355 / 351: Показать все, Скрыть все, ─ and the
/// listed channels inserted at position `3 + i`, after the separator
/// (`0x492a00`, `0x46f2e0`; a skipped channel leaves no hole).
pub fn simple_channels_menu(chans: &[MenuChan]) -> Vec<Entry<DiagCmd>> {
    let mut m = vec![
        Entry::item("Показать все", true, Mark::None, DiagCmd::ShowAll(true)),
        Entry::item("Скрыть все", true, Mark::None, DiagCmd::ShowAll(false)),
        Entry::Sep,
    ];
    for c in chans {
        insert_at(&mut m, c.idx + 3, check(c.name.clone(), c.on, DiagCmd::ToggleChan(c.idx)));
    }
    m
}

/// «Отсчет углов» (MENU 355): Абсолютный угол (36000), ─, then the
/// vector-flagged channels (36001 + i) as a radio group.
pub fn angle_menu(flagged: &[String], current: Option<&str>) -> Vec<Entry<DiagCmd>> {
    let mut m = vec![
        Entry::item("Абсолютный угол", true, Mark::Radio(current.is_none()), DiagCmd::AngleRef(None)),
        Entry::Sep,
    ];
    for n in flagged {
        m.push(Entry::item(n.clone(), true, Mark::Radio(current == Some(n.as_str())), DiagCmd::AngleRef(Some(n.clone()))));
    }
    m
}

/// MENU 355 with «Углы в радианах» (32841) removed at run time (`0x48fc80`).
pub fn vector_menu(channels: Vec<Entry<DiagCmd>>, angles: Vec<Entry<DiagCmd>>, show_values: bool, dir_only: bool) -> Vec<Entry<DiagCmd>> {
    vec![
        Entry::sub("Каналы", channels),
        Entry::Sep,
        Entry::sub("Отсчет углов", angles),
        Entry::Sep,
        check("Показывать значения", show_values, DiagCmd::ShowValues),
        check("Только направление", dir_only, DiagCmd::DirectionOnly),
    ]
}

/// MENU 351 (frequency).
pub fn freq_menu(channels: Vec<Entry<DiagCmd>>) -> Vec<Entry<DiagCmd>> {
    vec![Entry::sub("Каналы", channels), Entry::Sep, Entry::item("Настройки", true, Mark::None, DiagCmd::FreqSettings)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(m: &[Entry<DiagCmd>]) -> Vec<String> {
        m.iter()
            .map(|e| match e {
                Entry::Item { label, .. } => label.clone(),
                Entry::Sub { label, .. } => format!("{label} ▸"),
                Entry::Sep => "─".into(),
            })
            .collect()
    }

    #[test]
    fn nice_rounds_one_digit_up() {
        assert_eq!(nice_up(0.0), 0.0);
        assert_eq!(nice_up(3.2), 4.0);
        assert_eq!(nice_up(41.0), 50.0);
        assert_eq!(nice_up(0.0123), 0.02);
        assert_eq!(nice_up(-7.5), -8.0);
        assert_eq!(nice_up(100.0), 100.0);
    }

    #[test]
    fn scale_cells_from_the_nearer_edge() {
        // 400×300 plot: min(200, 150) / 19 = 7 cells; max 30 Ω → 30/7 → 5.
        let s = plane_scale(400, 300, 1.0, [30.0, 0.0]);
        assert_eq!((s.cell, s.cells), (19, 7));
        assert_eq!(s.upc, [5.0, 0.0]);
        assert_eq!(s.ppu, [Some(19.0 / 5.0), None]);
        // Zoomed ×1.5: 30/10.5 = 2.86 → 3.
        assert_eq!(plane_scale(400, 300, 1.5, [30.0, 0.0]).upc[0], 3.0);
    }

    #[test]
    fn wheel_modifiers_and_pan_limits() {
        assert_eq!(wheel_action(true, true, false), WheelAct::Zoom(true));
        assert_eq!(wheel_action(false, false, true), WheelAct::PanX(-2));
        assert_eq!(wheel_action(true, true, true), WheelAct::PanY(2), "Ctrl+Shift is not exactly Ctrl");
        assert_eq!(max_pan(400, 1.0), 11); // (400/19 = 21) · 0.5 → 10.5 → 11
        let mut v = PlaneView::default();
        assert!(v.pan(false, 2, 3));
        assert!(!v.pan(false, 2, 3), "|4| > 3 refused");
        assert_eq!(v.oy, 2);
    }

    #[test]
    fn zoom_limits_and_pan_reset() {
        let mut v = PlaneView { ox: 4, oy: -2, ..Default::default() };
        // 10 Ω of characteristics at 30 px/Ω = 300 px < 400: accepted.
        assert!(v.zoom(true, 10.0, |_| Some(30.0)));
        assert_eq!(v.z, 1.5);
        // 10 Ω at 45 px/Ω = 450 px: refused.
        assert!(!v.zoom(true, 10.0, |_| Some(45.0)));
        assert!(!v.zoom(true, 0.0, |_| None), "no finite scale: refused");
        assert!(v.zoom(false, 10.0, |_| None));
        assert_eq!((v.z, v.ox, v.oy), (1.0, 0, 0), "z ≤ 1 resets the pans");
    }

    #[test]
    fn differential_initial_view_goes_lower_left() {
        let mut v = PlaneView { initial_pending: true, ..Default::default() };
        v.push_lower_left(7, 5);
        assert_eq!((v.ox, v.oy, v.initial_pending), (-6, 4, false));
    }

    #[test]
    fn scrollbar_mapping_keeps_the_original_factor() {
        assert_eq!(scroll_range(1.0), None);
        let (page, m) = scroll_range(2.0).unwrap();
        assert_eq!((page, m), (5000, 5000));
        assert_eq!(scroll_pos(0, 10, m), 2500);
        assert_eq!(scroll_pos(2, 10, m), 1500);
        assert_eq!(pan_from_scroll(1500, 10, m), 4, "pos → o doubles");
        assert_eq!(scroll_step(50, false, m), 0);
        assert_eq!(scroll_step(4950, true, m), 4999);
    }

    #[test]
    fn scanline_fill_punches_holes() {
        let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let hole = vec![[3.0, 3.0], [7.0, 3.0], [7.0, 7.0], [3.0, 7.0]];
        let spans = scanline_spans(&[outer.clone(), hole], 0, 10);
        assert_eq!(spans.iter().filter(|s| s.0 == 1).count(), 1);
        let row5: Vec<_> = spans.iter().filter(|s| s.0 == 5).map(|s| (s.1, s.2)).collect();
        assert_eq!(row5, vec![(0.0, 3.0), (7.0, 10.0)]);
        assert!(scanline_spans(&[outer], 10, 12).is_empty());
    }

    #[test]
    fn vector_circle_rule() {
        assert_eq!(vector_circles(120.0), 4);
        assert_eq!(vector_circles(200.0), 5);
        assert_eq!(vector_circles(499.0), 5);
        assert_eq!(vector_circles(500.0), 10);
        assert_eq!(freq_x_label(50.0), "50");
        assert_eq!(freq_x_label(16.666), "16.7");
    }

    #[test]
    fn units_select_the_diagrams() {
        assert!(vector_unit_ok("A") && vector_unit_ok("") && vector_unit_ok("ном") && !vector_unit_ok("Ohm"));
        assert!(circle_flag_unit_ok("var") && circle_flag_unit_ok("W") && !circle_flag_unit_ok("A"));
        assert!(circle_pane_unit_ok("VA") && !circle_pane_unit_ok("W"));
    }

    #[test]
    fn circle_channel_items_follow_insert_menu_positions() {
        let ch = |idx: usize, name: &str, unit: &str| MenuChan { idx, name: name.into(), unit: unit.into(), on: false };
        // Channels 0 and 1 eligible: inserted at 3 and 4, before the separator.
        let m = circle_channels_menu(true, &[ch(0, "Z1", "Ohm"), ch(1, "S", "VA")], 2, &[ChanPair::new("R", "X")]);
        assert_eq!(labels(&m), ["Создать", "Показать все", "Скрыть все", "Z1", "S", "─", "R | X"]);
        // Only channel 5 eligible: position 8 is past the end → appended.
        let m = circle_channels_menu(true, &[ch(0, "Ia", "A"), ch(5, "Z", "Ohm")], 6, &[]);
        assert_eq!(labels(&m), ["Создать", "Показать все", "Скрыть все", "─", "Z"]);
    }

    #[test]
    fn menus_in_resource_order() {
        let m = plane_menu(false, vec![], true, false, 2);
        assert_eq!(
            labels(&m),
            [
                "Каналы ▸",
                "─",
                "Отображать характеристики возврата",
                "Диапазон между визирами",
                "Показывать вещественную и мнимую части",
                "Показывавать действующее значение и угол",
                "─",
                "Перезагрузить базы характеристик"
            ]
        );
        let m = plane_menu(true, vec![], true, false, 1);
        assert_eq!(labels(&m)[4], "Показывать значения");
        let v = vector_menu(vec![], angle_menu(&["Ua".into()], Some("Ua")), true, false);
        assert_eq!(labels(&v), ["Каналы ▸", "─", "Отсчет углов ▸", "─", "Показывать значения", "Только направление"]);
        assert!(!labels(&v).iter().any(|l| l.contains("радиан")), "32841 removed");
    }
}
