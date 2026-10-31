//! The diagram windows painted and driven as the original's (ledger L9,
//! `UI_BEHAVIOUR_SPEC` §6): the circle / differential plane (`0x43b570`
//! draw, `0x4393b0` grid, `0x43a4a0` characteristics, `0x43a880` loci,
//! `0x438210` legend, `0x43bbf0` readout), the vector diagram (`0x490360`),
//! the frequency diagram (`0x46f5a0`), the vertical pane toolbars 550 / 552 /
//! 553, the context menus 387 / 388 / 355 / 351, the pair dialog 554 and the
//! frequency settings dialog 353. Painted with egui's painter: no free pan,
//! box zoom or egui legend — only the original's (z, ox, oy) model.

use crate::app::{App, Doc};
use crate::diagram::{self as dg, ChanPair, DiagCmd, MenuChan, PlaneView, WheelAct};
use crate::popmenu::{self, Entry, PopMenu};
use crate::oscgeom::PixelLines;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use fvb_core::geom::Shape;
use num_complex::Complex64;

/// A diagram window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    Vector,
    Circle,
    Diff,
    Freq,
}

/// Which menu is open: the whole context menu, or one submenu popped from
/// a toolbar drop-down (33014 / 33010 Каналы, 33011 Отсчет углов).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Full,
    Channels,
    Angles,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenMenu {
    pub win: Window,
    pub kind: MenuKind,
    pub pm: PopMenu,
}

/// What the last paint of a plane measured, for the toolbar's zoom / pan
/// commands (which the original also checks against the current layout).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneGeom {
    pub w: i32,
    pub h: i32,
    /// The span the pan limit counts in (`0x443620` / `0x437b90`): client
    /// − 6, top + 5, bottom − 20 always — the readout table not subtracted.
    pub pan: (i32, i32),
    pub max: [f64; 2],
    pub char_max: f64,
    /// The zoom gate's own rectangle (`0x4379e0` / `0x443470`): the client
    /// deflated by 8 px left / right and 16 px top / bottom
    /// (`InflateRect(−8, −16)`, `0x437a2f`–`0x437a37`) — not the painted
    /// plane.
    pub gate: (i32, i32),
}

impl PlaneGeom {
    /// The gate's Ω pixels per unit at zoom `z`.
    fn ppu_at(&self, z: f64) -> Option<f64> {
        dg::plane_scale(self.gate.0, self.gate.1, z, self.max).ppu[0]
    }
}

/// DIALOG 554 (ChanPairDlg): the pair list being edited.
#[derive(Debug, Clone, PartialEq)]
pub struct PairDialog {
    pub diff: bool,
    pub pairs: Vec<ChanPair>,
    pub a: Option<String>,
    pub b: Option<String>,
    pub sel: Option<usize>,
    /// The list holds the focus: its selection (0, 120, 215), else (240)
    /// (LVS_SHOWSELALWAYS; VM: after «Добавить» / «Удалить»).
    pub list_focus: bool,
}

/// DIALOG 353 «Настройки частотной диаграммы».
#[derive(Debug, Clone, PartialEq)]
pub struct FreqDialog {
    pub percent: bool,
    pub use_rms: bool,
    /// The two int edits as typed (0x3fe NumHarmonics, 0x401 NumPeriods).
    pub harmonics: String,
    pub periods: String,
    /// The field a refused OK left the focus in (0 periods, 1 harmonics).
    pub fail: Option<u8>,
    /// The enable state of 1026 / 1027 (the two «Нормализация» radios):
    /// OnInitDialog (`0x46ee20`) disables them in absolute mode; every
    /// click (`0x46f070`, nCode 0) sets them — and the frame 1061, which
    /// looks the same either way (VM) — from 1023's check.
    pub radios_on: bool,
}

/// DIALOG 353's exchange on OK (`0x46ee70`): 0x401 (periods) an int in
/// [1, N div (R div trunc f)], then 0x3fe (harmonics) an int in [1,
/// trunc(R/f · 0.5) · periods] — N the record's samples, R rec+0x228, f
/// the network frequency; the first failure is MFC's message and the
/// dialog stays (AFX_IDP_PARSE_INT / AFX_IDP_PARSE_INT_RANGE).
#[cfg(test)]
pub fn freq_dialog_exchange(periods: &str, harmonics: &str, n: usize, r: f64, f: f64) -> Result<(usize, usize), String> {
    freq_exchange_field(periods, harmonics, n, r, f).map_err(|e| e.1)
}

/// [`freq_dialog_exchange`] with the failing field (0 periods, 1 harmonics).
pub fn freq_exchange_field(periods: &str, harmonics: &str, n: usize, r: f64, f: f64) -> Result<(usize, usize), (u8, String)> {
    let int = |s: &str| crate::dialogs::ddx_int(s).ok_or_else(|| "Введите целое число.".to_string());
    let range = |v: i32, max: i64| {
        if v < 1 || i64::from(v) > max {
            Err(format!("Введите целое число от 1 до {max}."))
        } else {
            Ok(v as usize)
        }
    };
    let spp = (r as u32).checked_div(f as u32).filter(|&s| s > 0).unwrap_or(1);
    let p = int(periods).and_then(|v| range(v, (n as u32 / spp) as i64)).map_err(|m| (0u8, m))?;
    let h = int(harmonics).and_then(|v| range(v, i64::from((r / f * 0.5) as i32) * p as i64)).map_err(|m| (1u8, m))?;
    Ok((p, h))
}

pub struct DiagState {
    pub circle: PlaneView,
    pub diff: PlaneView,
    /// The differential window's own options (`Diff Chart`): SightsOnly,
    /// ShowOffCharacteristics (on), ViewChannelType (boolean).
    pub diff_between: bool,
    pub diff_show_reset: bool,
    pub diff_values: bool,
    pub geom: [Option<PlaneGeom>; 2],
    /// The circle / differential panes' toolbars: a wheel over one reaches
    /// the plane (the pane's window procedure `0x436220` posts every
    /// WM_MOUSEWHEEL on to its diagram window; the toolbar has no wheel
    /// handler, so Windows hands its wheel to the pane).
    pub toolbar: [Option<Rect>; 2],
    pub menu: Option<OpenMenu>,
    pub serial: u64,
    /// A press that dismissed a menu is swallowed until the buttons are up.
    pub menu_eat: bool,
    pub pair_dlg: Option<PairDialog>,
    pub freq_dlg: Option<FreqDialog>,
}

impl Default for DiagState {
    fn default() -> Self {
        DiagState {
            circle: PlaneView::default(),
            diff: PlaneView { initial_pending: true, ..Default::default() },
            diff_between: false,
            diff_show_reset: true,
            diff_values: false,
            geom: [None, None],
            toolbar: [None, None],
            menu: None,
            serial: 0,
            menu_eat: false,
            pair_dlg: None,
            freq_dlg: None,
        }
    }
}

const MENU_ID: &str = "fv_diag_menu";
const TEXT: Color32 = Color32::from_rgb(50, 50, 50);
const GRID: Color32 = Color32::from_rgb(230, 230, 230);
const FRAME: Color32 = Color32::from_rgb(200, 200, 200);
const AXES: Color32 = Color32::from_rgb(150, 150, 150);
const SCROLL_W: f32 = 16.0;

fn stroke(w: f32, c: Color32) -> Stroke {
    Stroke::new(w.max(PIXEL.with(std::cell::Cell::get)), c)
}

thread_local! {
    /// While a diagram page prints (`0x441fe0`): the integer scale
    /// `s = ⌊LOGPIXELSY(printer) / LOGPIXELSY(screen)⌋`; `None` on screen.
    static PRINT_S: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
    /// On a real print (`m_bPrinting`; not the preview's memory DC): the
    /// page origin relative to the diagram rect, in points.
    static PRINT_PAGE: std::cell::Cell<Option<Vec2>> = const { std::cell::Cell::new(None) };
    /// One pixel of the device drawn on, in points, while a page renders:
    /// GDI draws a pen thinner than a device pixel one pixel wide in its
    /// full colour (the preview's pens of a few printer pixels).
    static PIXEL: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
}

/// Run `f` with no pen thinner than `px` points (a device pixel).
pub fn with_pixel<R>(px: f32, f: impl FnOnce() -> R) -> R {
    let old = PIXEL.with(|c| c.replace(px));
    let r = f();
    PIXEL.with(|c| c.set(old));
    r
}

/// Run `f` with the print scale `s` set (the diagram painters' print
/// branches) and, on a real print, R's offset on the page in points.
pub fn with_print_scale<R>(s: Option<f32>, page: Option<Vec2>, f: impl FnOnce() -> R) -> R {
    let old = (PRINT_S.with(|c| c.replace(s)), PRINT_PAGE.with(|c| c.replace(page)));
    let r = f();
    PRINT_S.with(|c| c.set(old.0));
    PRINT_PAGE.with(|c| c.set(old.1));
    r
}

fn print_s() -> Option<f32> {
    PRINT_S.with(std::cell::Cell::get)
}

/// The print look's scale: the painters pick their print pens and the
/// frequency page layout by s ≠ 1, not by printing (`0x490495`,
/// `0x439486`, `0x46f63b` test `s == 1`) — a printer under 192 dpi gets
/// the screen look (L9.118).
fn print_look() -> Option<f32> {
    print_s().filter(|&s| s != 1.0)
}

/// `v` device pixels in points: the painters keep a few offsets in device
/// pixels unscaled on paper (the angle labels, the legend swatch, the
/// plane's top and its 120 px thresholds).
fn dpx(v: f32) -> f32 {
    v / print_s().unwrap_or(1.0)
}

/// A print pen `dev` printer pixels wide (truncated; 0 draws one pixel), in
/// points.
fn dev_pen(dev: f32, c: Color32) -> Stroke {
    let s = print_s().unwrap_or(1.0);
    stroke(dev.floor().max(1.0) / s, c)
}

fn rgb((r, g, b): (u8, u8, u8)) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Tahoma 13 on screen; printing passes Arial `⌊52·s/5⌋` (`0x441fe0`),
/// 0.8 of the screen size.
/// The diagrams' font: Tahoma with a 13 px cell (`0x441b80`, the
/// DiagramWnd base), an 11 px em.
fn font() -> FontId {
    FontId::proportional(if print_s().is_some() { 11.0 * 0.8 } else { 11.0 })
}

/// `%g` with an SI prefix: (text, prefix).
fn g_si(v: f64) -> (String, &'static str) {
    let (s, p) = fvb_core::omp::si(v);
    (fvb_core::views::fmt_g(s), p)
}

/// A channel of the active document as the diagrams list it: analog
/// channels then computed ones (our computed channels stand for the
/// original's appended analog channels).
struct DChan {
    name: String,
    unit: String,
    /// The unit as the readouts show it (`0x42bc50`).
    shown: fvb_core::units::Shown,
    color: Color32,
    analog: Option<usize>,
    computed: Option<usize>,
    /// A pasted channel waiting for «Объединить» (bit 14).
    pending: bool,
}

/// The analog-kind records in vector order (a pasted digital channel is
/// kind 1, past `doc+0x1d8`, L9.188).
fn doc_chans(d: &Doc) -> Vec<DChan> {
    d.kind0_order()
        .into_iter()
        .map(|k| match k {
            // The canonical unit, none in ADC mode (`0x414c30`).
            crate::app::ChanKey::Analog(i) | crate::app::ChanKey::Digital(i) => {
                let c = &d.osc.analog[i];
                let unit = if d.chans[i].adc_counts { String::new() } else { c.unit.clone() };
                let shown = fvb_core::units::Shown::file(&unit, c.per_second);
                DChan { name: c.id.clone(), unit, shown, color: d.chans[i].color, analog: Some(i), computed: None, pending: false }
            }
            crate::app::ChanKey::Computed(j) => {
                let c = &d.computed[j];
                DChan {
                    name: c.name.clone(),
                    unit: c.unit.trim().to_string(),
                    shown: fvb_core::units::Shown::computed(c.unit.trim()),
                    color: c.color,
                    analog: None,
                    computed: Some(j),
                    pending: d.pending_of(j).is_some(),
                }
            }
        })
        .collect()
}

/// Instantaneous value of a channel by name (`0x426020`).
/// A channel's value per sample.
type Series<'a> = Box<dyn Fn(usize) -> f64 + 'a>;

fn inst_by_name<'a>(d: &'a Doc, name: &str) -> Option<(Series<'a>, Color32)> {
    if let Some(i) = d.osc.analog.iter().position(|c| c.id == name) {
        let kf = d.shown_k(i);
        return Some((Box::new(move |k| d.sample(i, k) * kf), d.chans[i].color));
    }
    let c = d.computed.iter().find(|c| c.name == name)?;
    Some((Box::new(move |k| c.samples.get(k).copied().unwrap_or(f64::NAN)), c.color))
}

/// The diagram flag of a channel for a window.
fn chan_flag(d: &Doc, w: Window, c: &DChan) -> bool {
    match (c.analog, c.computed, w) {
        (Some(i), _, Window::Vector) => d.chans[i].on_vector,
        (Some(i), _, Window::Freq) => d.chans[i].on_freq,
        (Some(i), _, _) => d.chans[i].on_circle,
        (_, Some(j), Window::Vector) => d.computed[j].on_vector,
        (_, Some(j), Window::Circle) => d.computed[j].on_circle,
        (_, Some(j), Window::Freq) => d.computed[j].on_freq,
        _ => false,
    }
}

fn set_chan_flag(d: &mut Doc, w: Window, c: &DChan, v: bool) {
    if let Some(i) = c.analog {
        let c = &mut d.chans[i];
        match w {
            Window::Vector => c.on_vector = v,
            Window::Freq => c.on_freq = v,
            _ => c.on_circle = v,
        }
    } else if let Some(c) = c.computed.and_then(|j| d.computed.get_mut(j)) {
        match w {
            Window::Vector => c.on_vector = v,
            Window::Circle => c.on_circle = v,
            Window::Freq => c.on_freq = v,
            _ => {}
        }
    }
}

/// Is a channel listed in a window's Каналы submenu.
fn listed(w: Window, c: &DChan) -> bool {
    match w {
        Window::Vector => dg::vector_unit_ok(&c.unit),
        // Formula channels by their walked unit like any other (L9.83).
        Window::Circle => dg::circle_pane_unit_ok(&c.unit),
        // Every analog-kind record not pending a merge (`0x46f2e0`).
        Window::Freq => c.analog.is_some() || c.computed.is_some() && !c.pending,
        Window::Diff => false,
    }
}

/// One locus of a plane: name (legend), colour, power scale, the samples and
/// the values at cursor 1 / cursor 2.
struct Locus {
    name: String,
    /// The unit as the readout table shows it (`0x42bc50`).
    unit: fvb_core::units::Shown,
    color: Color32,
    power: bool,
    pts: Vec<[f64; 2]>,
    c1: [f64; 2],
    c2: [f64; 2],
    /// Gets the cursor "+" marks (not an R/X pair, `0x43a880`).
    markers: bool,
    /// A differential pair's units, (Iдиф, Iторм): its table shows each
    /// value in its own channel's unit (`0x446460`).
    pair_units: Option<[fvb_core::units::Shown; 2]>,
}

/// The unit a channel's readout shows (`0x42bc50`), by name.
fn shown_by_name(d: &Doc, name: &str) -> fvb_core::units::Shown {
    if let Some(c) = d.osc.analog.iter().find(|c| c.id == name) {
        return c.shown_unit();
    }
    d.computed.iter().find(|c| c.name == name).map(|c| fvb_core::units::Shown::computed(&c.unit)).unwrap_or_default()
}

/// The circle diagram's range (`0x43a880`, `0x43a9ed`–`0x43aa41`): N..len,
/// or [min(c1,c2), max(c1,c2)] with «Диапазон между визирами»; both bounds
/// raised to N, then capped at the sample count itself — so the default
/// range ends one past the samples, at a (0, 0) point.
/// A channel's bins in the frequency diagram (`0x426510`): its window is
/// round(own rate / 50) with an own rate (`ch+0x108`), else the record's, and
/// the correction runs at that rate too.
pub(crate) fn freq_bins(d: &Doc, samples: &[f64], file: Option<usize>, own_rate: f64, pos: usize, periods: usize, k_max: usize) -> Vec<f64> {
    let corr = fvb_core::freqcorr::of(&d.osc);
    let mine = file.and_then(|i| corr.and_then(|cr| cr.for_chan(i))).map(|c| c.at_rate(own_rate));
    fvb_core::freqcorr::spectrum(mine, samples, pos, d.osc.period_of(own_rate).max(2), periods, k_max, d.osc.harmonic1_gain)
}

fn circle_range(d: &Doc, between: bool) -> std::ops::RangeInclusive<usize> {
    let len = d.osc.sample_count();
    let n = d.osc.samples_per_cycle().max(1);
    let (a, b) = if between { (d.main_cursor.min(d.aux_cursor), d.main_cursor.max(d.aux_cursor)) } else { (n, len) };
    a.max(n).min(len)..=b.max(n).min(len)
}

/// A point of the circle diagram: a non-finite part or a sample past the
/// values gives the origin (`0x43ab37`–`0x43ab96`).
/// The differential scale's range (`0x4441b0`, `0x44425d`–`0x4442b8`):
/// N..=len, or with «Диапазон между визирами» [min(c1,c2), max(c1,c2)]
/// NOT raised to N — each bound only capped at the sample count — while
/// the curves keep the circle's range.
fn diff_scale_range(d: &Doc, between: bool) -> std::ops::RangeInclusive<usize> {
    let len = d.osc.sample_count();
    if !between {
        return circle_range(d, false);
    }
    let (a, b) = (d.main_cursor.min(d.aux_cursor), d.main_cursor.max(d.aux_cursor));
    a.min(len)..=b.min(len)
}

/// The largest |value| of the enabled differential pairs over `range`
/// (`0x444391`–`0x44441b`: NaN skipped, 0 past the values).
fn diff_scale_max(d: &Doc, range: std::ops::RangeInclusive<usize>) -> f64 {
    let len = d.osc.sample_count();
    let mut m = 0.0f64;
    for p in d.dif_pairs.iter().filter(|p| p.on) {
        if let (Some((dif, _)), Some((tor, _))) = (inst_by_name(d, &p.a), inst_by_name(d, &p.b)) {
            for k in range.clone().filter(|&k| k < len) {
                for v in [tor(k), dif(k)] {
                    if !v.is_nan() {
                        m = m.max(v.abs());
                    }
                }
            }
        }
    }
    m
}

fn finite_or_origin(p: [f64; 2]) -> [f64; 2] {
    if p[0].is_finite() && p[1].is_finite() {
        p
    } else {
        [0.0, 0.0]
    }
}

fn plane_loci(d: &Doc, diff: bool, between: bool) -> Vec<Locus> {
    let mut out = Vec::new();
    let range = circle_range(d, between);
    // The marks use the cursors as they are: past the values, the origin.
    let (k1, k2) = (d.main_cursor, d.aux_cursor);
    let len = d.osc.sample_count();
    // `0x426020`: 0 past the values or for NaN.
    let inst = |s: &dyn Fn(usize) -> f64, k: usize| if k < len { Some(s(k)).filter(|v| v.is_finite()).unwrap_or(0.0) } else { 0.0 };
    if diff {
        // `0x4457a0`: (Iторм, Iдиф) of every enabled pair, the same range
        // as the circle's, the Iдиф channel's colour, one scale.
        for p in d.dif_pairs.iter().filter(|p| p.on) {
            if let (Some((dif, col)), Some((tor, _))) = (inst_by_name(d, &p.a), inst_by_name(d, &p.b)) {
                let f = |k| [inst(&tor, k), inst(&dif, k)];
                let pair_units = Some([shown_by_name(d, &p.a), shown_by_name(d, &p.b)]);
                // The table reads the raw values (a non-finite one empties
                // the cells).
                let raw = |k: usize| if k < len { [tor(k), dif(k)] } else { [0.0, 0.0] };
                out.push(Locus { name: p.legend_label(), unit: Default::default(), color: col, power: false, pts: range.clone().map(f).collect(), c1: raw(k1), c2: raw(k2), markers: true, pair_units });
            }
        }
        return out;
    }
    let mut push = |name: String, unit: fvb_core::units::Shown, color: Color32, power: bool, markers: bool, f: &dyn Fn(usize) -> [f64; 2]| {
        let g = |k| finite_or_origin(f(k));
        out.push(Locus { name, unit, color, power, pts: range.clone().map(g).collect(), c1: f(k1), c2: f(k2), markers, pair_units: None });
    };
    for (i, ch) in d.osc.analog.iter().enumerate() {
        if d.chans[i].on_circle {
            // The phasor cache holds the values read through the shown factor (`0x428490`).
            let ph = &d.analysis.phasors[i];
            // The scale by the canonical unit, "" in ADC mode (→ power).
            let canon = if d.chans[i].adc_counts { "" } else { ch.unit.trim() };
            let unit = fvb_core::units::Shown::file(canon, ch.per_second);
            push(ch.id.clone(), unit, d.chans[i].color, !dg::is_ohm(canon), true, &|k| ph.get(k).map_or([0.0, 0.0], |z| [z.re, z.im]));
        }
    }
    for (j, c) in d.computed.iter().enumerate().filter(|(_, c)| c.on_circle) {
        let series = |k: usize| {
            if k >= len {
                return [0.0, 0.0];
            }
            let z = match &c.phasors {
                Some(p) => p.get(k).copied().unwrap_or_default(),
                None => fvb_core::freqcorr::phasor_at(d.computed_corr(j), fvb_core::freqcorr::OTHER, &c.samples, k, d.computed_period(j)),
            };
            [z.re, z.im]
        };
        // The canonical unit decides the scale, "" → power (`0x43a880`).
        let unit = c.unit.trim();
        let power = !dg::is_ohm(unit);
        push(c.name.clone(), fvb_core::units::Shown::computed(unit), c.color, power, true, &series);
    }
    // R/X pairs: instantaneous values (`0x426020`: 0 past the values or
    // NaN, per coordinate), the Ω scale, the R channel's unit, no marks.
    for p in d.rx_pairs.iter().filter(|p| p.on) {
        if let (Some((r, col)), Some((x, _))) = (inst_by_name(d, &p.a), inst_by_name(d, &p.b)) {
            push(p.legend_label(), shown_by_name(d, &p.a), col, false, false, &|k| [inst(&r, k), inst(&x, k)]);
        }
    }
    out
}

/// A characteristic as painted: palette index and its regions.
struct CharDraw {
    pal: usize,
    op: Vec<Shape>,
    rs: Vec<Shape>,
}

/// Paint one region: even-odd fill (alpha 50) clipped to the region, then
/// a 1 px frame (`FrameRgn`): the region's own edge pixels, solid (VM: a
/// shallow edge 1–2 rows of the frame colour, not a thin line).
fn paint_region(p: &egui::Painter, rings: &[Vec<[f32; 2]>], fill: Option<Color32>, frame: Color32, clip: Rect) {
    let (y0, y1) = (clip.top().floor() as i32 - 1, clip.bottom().ceil() as i32 + 1);
    let spans = dg::scanline_spans(rings, y0, y1);
    if let Some(fc) = fill {
        let mut mesh = egui::Mesh::default();
        for (y, a, b) in dg::region_runs(&spans) {
            let r = Rect::from_min_max(Pos2::new(a as f32, y as f32), Pos2::new(b as f32, y as f32 + 1.0)).intersect(clip);
            if r.is_positive() {
                mesh.add_colored_rect(r, fc);
            }
        }
        p.add(egui::Shape::mesh(mesh));
    }
    let mut mesh = egui::Mesh::default();
    for (y, a, b) in dg::frame_runs(&spans) {
        let r = Rect::from_min_max(Pos2::new(a as f32, y as f32), Pos2::new(b as f32, y as f32 + 1.0)).intersect(clip);
        if r.is_positive() {
            mesh.add_colored_rect(r, frame);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

fn dotted(p: &egui::Painter, a: Pos2, b: Pos2, color: Color32, width: f32) {
    p.extend(egui::Shape::dashed_line(&[a, b], stroke(width, color), 2.0, 2.0));
}

/// GDI's cosmetic PS_DOT over whole pixels (VM: 3 on, 3 off, from the
/// figure's first pixel): pixel (x, y) is the square from (x, y).
fn gdi_dots(p: &egui::Painter, px: &[(i32, i32)], color: Color32) {
    gdi_pixels(p, px, color, true);
}

/// `px` lit in `color`: every one, or PS_DOT's three of each six.
fn gdi_pixels(p: &egui::Painter, px: &[(i32, i32)], color: Color32, dots: bool) {
    let mut mesh = egui::Mesh::default();
    for (k, &(x, y)) in px.iter().enumerate() {
        if !dots || k % 6 < 3 {
            mesh.add_colored_rect(Rect::from_min_size(Pos2::new(x as f32, y as f32), Vec2::splat(1.0)), color);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

/// The pixels LineTo lights from `a` towards `b` (Bresenham, `b` left out).
fn line_pixels(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
    let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
    let mut out = Vec::new();
    while (x, y) != b {
        out.push((x, y));
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    out
}

/// Ellipse's pixels for a circle of radius `r` about pixel `c` (its box
/// from c − r to c + r, the right and bottom edges out), counter-clockwise
/// from the right.
fn circle_pixels(c: (i32, i32), r: i32) -> Vec<(i32, i32)> {
    let n = (8 * r).max(8);
    let (cx, cy, rr) = (c.0 as f32 - 0.5, c.1 as f32 - 0.5, r as f32 - 0.5);
    let mut out: Vec<(i32, i32)> = Vec::new();
    for i in 0..n {
        let t = i as f32 / n as f32 * std::f32::consts::TAU;
        let q = ((cx + rr * t.cos()).round() as i32, (cy - rr * t.sin()).round() as i32);
        if out.last() != Some(&q) && out.first() != Some(&q) {
            out.push(q);
        }
    }
    out
}

/// A tool button glyph of the pane toolbars (our own line art; the
/// original's toolbar bitmaps are not redistributed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    Channels,
    Angles,
    Range,
    Reset,
    ReIm,
    Polar,
    Values,
    Direction,
    Reload,
    Plus,
    Minus,
    Up,
    Down,
    Left,
    Right,
}

pub(crate) fn paint_tool(p: &egui::Painter, r: Rect, t: Tool, c: Color32) {
    let r = r.shrink(4.0);
    let s = stroke(1.4, c);
    let pt = |x: f32, y: f32| Pos2::new(r.left() + x * r.width(), r.top() + y * r.height());
    let tri = |a: Pos2, b: Pos2, d: Pos2| egui::Shape::convex_polygon(vec![a, b, d], c, Stroke::NONE);
    match t {
        Tool::Channels => {
            for y in [0.2, 0.5, 0.8] {
                p.circle_filled(pt(0.12, y), 1.6, c);
                p.line_segment([pt(0.3, y), pt(1.0, y)], s);
            }
        }
        Tool::Angles => {
            p.line_segment([pt(0.0, 1.0), pt(1.0, 1.0)], s);
            p.line_segment([pt(0.0, 1.0), pt(0.85, 0.15)], s);
            p.add(egui::Shape::line((0..=6).map(|k| {
                let a = k as f32 / 6.0 * 0.78;
                Pos2::new(pt(0.0, 1.0).x + 0.55 * r.width() * a.cos(), pt(0.0, 1.0).y - 0.55 * r.height() * a.sin())
            }).collect(), s));
        }
        Tool::Range => {
            p.line_segment([pt(0.1, 0.0), pt(0.1, 1.0)], s);
            p.line_segment([pt(0.9, 0.0), pt(0.9, 1.0)], s);
            p.line_segment([pt(0.2, 0.5), pt(0.8, 0.5)], s);
            p.add(tri(pt(0.2, 0.5), pt(0.38, 0.35), pt(0.38, 0.65)));
            p.add(tri(pt(0.8, 0.5), pt(0.62, 0.35), pt(0.62, 0.65)));
        }
        Tool::Reset => {
            let q = Rect::from_min_max(pt(0.05, 0.05), pt(0.95, 0.95));
            for (a, b) in [(q.left_top(), q.right_top()), (q.right_top(), q.right_bottom()), (q.right_bottom(), q.left_bottom()), (q.left_bottom(), q.left_top())] {
                dotted(p, a, b, c, 1.4);
            }
        }
        Tool::ReIm | Tool::Values => {
            let txt = if t == Tool::ReIm { "Re" } else { "12" };
            p.text(r.center(), egui::Align2::CENTER_CENTER, txt, FontId::proportional(r.height() * 0.85), c);
        }
        Tool::Polar => {
            p.circle_stroke(r.center(), r.width() * 0.45, s);
            p.line_segment([r.center(), pt(0.85, 0.2)], s);
        }
        Tool::Direction => {
            p.line_segment([pt(0.1, 0.9), pt(0.85, 0.15)], s);
            p.add(tri(pt(0.95, 0.05), pt(0.55, 0.2), pt(0.8, 0.45)));
        }
        Tool::Reload => {
            p.add(egui::Shape::line((0..=10).map(|k| {
                let a = 0.6 + k as f32 / 10.0 * 4.6;
                r.center() + Vec2::new(a.cos(), -a.sin()) * r.width() * 0.42
            }).collect(), s));
            p.add(tri(pt(0.85, 0.05), pt(1.0, 0.45), pt(0.6, 0.35)));
        }
        Tool::Plus | Tool::Minus => {
            p.line_segment([pt(0.1, 0.5), pt(0.9, 0.5)], stroke(2.0, c));
            if t == Tool::Plus {
                p.line_segment([pt(0.5, 0.1), pt(0.5, 0.9)], stroke(2.0, c));
            }
        }
        Tool::Up => {
            p.add(tri(pt(0.5, 0.1), pt(0.1, 0.8), pt(0.9, 0.8)));
        }
        Tool::Down => {
            p.add(tri(pt(0.5, 0.9), pt(0.1, 0.2), pt(0.9, 0.2)));
        }
        Tool::Left => {
            p.add(tri(pt(0.1, 0.5), pt(0.8, 0.1), pt(0.8, 0.9)));
        }
        Tool::Right => {
            p.add(tri(pt(0.9, 0.5), pt(0.2, 0.1), pt(0.2, 0.9)));
        }
    }
}

/// One toolbar button: (glyph, its command's prompt «description\ntitle»
/// — the tooltip's, `tips` —, command, checked state).
type ToolButton = (Tool, &'static str, DiagCmd, Option<bool>);

impl App {
    fn plane_opts(&self, diff: bool) -> (bool, bool, u8) {
        if diff {
            (self.diag.diff_between, self.diag.diff_show_reset, u8::from(self.diag.diff_values))
        } else {
            (self.circle_between_cursors, self.show_reset, self.circle_readout)
        }
    }

    fn plane_view(&mut self, diff: bool) -> &mut PlaneView {
        if diff {
            &mut self.diag.diff
        } else {
            &mut self.diag.circle
        }
    }

    /// The buttons of TOOLBAR 550 / 552 / 553, top to bottom; `None` marks a
    /// separator.
    fn toolbar_buttons(&self, w: Window) -> Vec<Option<ToolButton>> {
        let diff = w == Window::Diff;
        match w {
            // 33012 / 33013 (`0x48f490` / `0x48f4f0`): enabled with a
            // document, checked only with one and the flag.
            Window::Vector => {
                let doc = self.active_doc().is_some();
                vec![
                    // 33010 / 33011 / 33012 / 33013.
                    Some((Tool::Channels, "Настройка каналов\nКаналы", DiagCmd::ChannelsMenu, None)),
                    None,
                    Some((Tool::Angles, "Отсчет углов\nОтсчет углов", DiagCmd::AngleMenu, None)),
                    None,
                    Some((Tool::Values, "Показывавать действующее значение и угол\nПоказывавать действующее значение и угол", DiagCmd::ShowValues, Some(doc && self.vec_show_values))),
                    Some((Tool::Direction, "Отображать на диаграмме только направление\nТолько направление", DiagCmd::DirectionOnly, Some(doc && self.vec_direction_only))),
                ]
            }
            Window::Circle | Window::Diff => {
                let (between, reset, readout) = self.plane_opts(diff);
                let mut v = vec![
                    // 33014 … 33025, 33028.
                    Some((Tool::Channels, "Настройка каналов\nКаналы", DiagCmd::ChannelsMenu, None)),
                    None,
                    Some((Tool::Range, "Диапазон между визирами\nДиапазон между визирами", DiagCmd::BetweenCursors, Some(between))),
                    Some((Tool::Reset, "Отображать характеристики возврата\nХарактеристики возврата", DiagCmd::ShowReset, Some(reset))),
                    None,
                ];
                if diff {
                    v.push(Some((Tool::Values, "Показывать значения", DiagCmd::Readout(1), Some(readout != 0))));
                } else {
                    v.push(Some((Tool::ReIm, "Показывать вещественную и мнимую части\nПоказывать вещественную и мнимую части", DiagCmd::Readout(1), Some(readout == 1))));
                    v.push(Some((Tool::Polar, "Показывавать действующее значение и угол\nПоказывавать действующее значение и угол", DiagCmd::Readout(2), Some(readout == 2))));
                }
                v.extend([
                    None,
                    Some((Tool::Reload, "Перезагрузить базы характеристик\nПерезагрузить базы характеристик", DiagCmd::Reload, None)),
                    None,
                    Some((Tool::Plus, "Увеличить масштаб\nУвеличить масштаб", DiagCmd::Zoom(true), None)),
                    Some((Tool::Minus, "Уменьшить масштаб\nУменьшить масштаб", DiagCmd::Zoom(false), None)),
                    None,
                    Some((Tool::Up, "Сдвиг вверх\nСдвиг вверх", DiagCmd::Pan(false, -2), None)),
                    Some((Tool::Down, "Сдвиг вниз\nСдвиг вниз", DiagCmd::Pan(false, 2), None)),
                    None,
                    Some((Tool::Left, "Сдвиг влево\nСдвиг влево", DiagCmd::Pan(true, -2), None)),
                    Some((Tool::Right, "Сдвиг вправо\nСдвиг вправо", DiagCmd::Pan(true, 2), None)),
                ]);
                v
            }
            Window::Freq => Vec::new(),
        }
    }

    /// The pane's vertical toolbar on its right edge (`0x436270`), 16×16
    /// buttons. Call before the window's contents.
    pub fn pane_toolbar(&mut self, ui: &mut egui::Ui, w: Window) {
        let buttons = self.toolbar_buttons(w);
        let doc = self.active_doc().is_some();
        let mut fire = None;
        // A 24 px strip, a horizontal gradient (VM: per style, Blue's (227,
        // 239, 255) flat for 6 px, then shading over 16 to (177, 211, 255),
        // flat for the last 2).
        // At > 100 % its images stretched (`dpi::img`), the buttons and the
        // strip with them (VM, 150 %: 30 px buttons).
        let side = crate::dpi::img(16.0) + 6.0;
        let bar = egui::SidePanel::right(egui::Id::new(("pane_toolbar", w as u8))).resizable(false).show_separator_line(false).frame(egui::Frame::none()).exact_width(side + 2.0).show_inside(ui, |ui| {
            let r = ui.max_rect();
            let mut mesh = egui::Mesh::default();
            let [l, rr] = self.skin.palette().bands.map_or([Color32::from_rgb(227, 239, 255), Color32::from_rgb(177, 211, 255)], |b| b.toolbar);
            for (k, (x0, x1, c0, c1)) in [(0.0, 6.0, l, l), (6.0, side, l, rr), (side, r.width(), rr, rr)].into_iter().enumerate() {
                let (a, b) = (r.left() + x0, r.left() + x1);
                for (x, y, c) in [(a, r.top(), c0), (b, r.top(), c1), (b, r.bottom(), c1), (a, r.bottom(), c0)] {
                    mesh.colored_vertex(egui::pos2(x, y), c);
                }
                let i = 4 * k as u32;
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i, i + 2, i + 3);
            }
            ui.painter().add(egui::Shape::mesh(mesh));
            // CMFCToolBar's vertical layout (VM): 22 px buttons touching,
            // 8 px separators — a (118, 161, 218) line 3 px down with a
            // white one under it, 14 px long from 6 px in.
            ui.add_space(1.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            // Buttons that do not fit are cut off (a short toolbar), never
            // grow the pane past its slot.
            for b in &buttons {
                if ui.available_height() < side {
                    break;
                }
                let Some((tool, tip, cmd, checked)) = b else {
                    use crate::oscgeom::PixelLines;
                    let (sep, _) = ui.allocate_exact_size(Vec2::new(side + 2.0, 8.0), egui::Sense::hover());
                    let x = sep.left() + 6.0..=sep.left() + side - 3.0;
                    ui.painter().px_hline(x.clone(), sep.top() + 3.0, stroke(1.0, Color32::from_rgb(118, 161, 218)));
                    ui.painter().px_hline(x, sep.top() + 4.0, stroke(1.0, Color32::WHITE));
                    continue;
                };
                // The vector toolbar is enabled with a document (CMDUI 0x48f440).
                let enabled = w != Window::Vector || doc;
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
                let vis = ui.visuals();
                // Checked: CMFCToolBar's checked face (VM).
                if checked == &Some(true) {
                    crate::ribbon::toolbar_checked(ui.painter(), rect, &self.skin.palette());
                } else if resp.hovered() && enabled {
                    // The ribbon's hot face (VM, the gold gradient).
                    crate::ribbon::hot_small(ui.painter(), rect, &self.skin.palette());
                }
                let col = if enabled { vis.text_color() } else { vis.weak_text_color() };
                paint_tool(ui.painter(), rect, *tool, col);
                // A prompt without «\n» is both title and description.
                let (desc, title) = tip.split_once('\n').unwrap_or((tip, tip));
                crate::tips::pointer_tip(ui, &resp, crate::tips::TipImage::Tool(*tool), title, desc, false);
                if resp.clicked() {
                    fire = Some(cmd.clone());
                }
            }
        });
        match w {
            Window::Circle => self.diag.toolbar[0] = Some(bar.response.rect),
            Window::Diff => self.diag.toolbar[1] = Some(bar.response.rect),
            _ => {}
        }
        if let Some(c) = fire {
            let at = ui.ctx().pointer_latest_pos().unwrap_or_default();
            self.diag_cmd(w, c, at);
        }
    }

    pub(crate) fn open_diag_menu(&mut self, win: Window, kind: MenuKind, pos: Pos2) {
        self.diag.serial += 1;
        self.diag.menu = Some(OpenMenu { win, kind, pm: PopMenu::new(self.diag.serial, pos) });
    }

    fn menu_chans(&self, w: Window) -> Vec<MenuChan> {
        let Some(d) = self.active_doc() else { return Vec::new() };
        doc_chans(d)
            .iter()
            .enumerate()
            // The circle's list (`0x43d2a5`) skips pending (bit 14) channels.
            .filter(|(_, c)| if w == Window::Circle { !c.computed.is_some_and(|j| d.computed[j].formula == crate::merge_ui::PENDING) } else { listed(w, c) })
            .map(|(idx, c)| MenuChan { idx, name: c.name.clone(), unit: c.unit.clone(), on: chan_flag(d, w, c) })
            .collect()
    }

    /// The Каналы submenu of a window.
    fn channels_entries(&self, w: Window) -> Vec<Entry<DiagCmd>> {
        let d = self.active_doc();
        match w {
            Window::Circle => {
                let pairs = d.map(|d| d.rx_pairs.clone()).unwrap_or_default();
                let n = d.map_or(0, |d| d.chans.len() + d.computed.len());
                dg::circle_channels_menu(d.is_some(), &self.menu_chans(w), n, &pairs)
            }
            Window::Diff => dg::diff_channels_menu(d.is_some(), &d.map(|d| d.dif_pairs.clone()).unwrap_or_default()),
            _ => dg::simple_channels_menu(&self.menu_chans(w)),
        }
    }

    fn angle_entries(&self) -> Vec<Entry<DiagCmd>> {
        let flagged: Vec<String> = self
            .active_doc()
            .map(|d| doc_chans(d).iter().filter(|c| chan_flag(d, Window::Vector, c)).map(|c| c.name.clone()).collect())
            .unwrap_or_default();
        dg::angle_menu(&flagged, self.vec_angle_ref.as_deref())
    }

    fn menu_entries(&self, m: &OpenMenu) -> Vec<Entry<DiagCmd>> {
        match m.kind {
            MenuKind::Channels => self.channels_entries(m.win),
            MenuKind::Angles => self.angle_entries(),
            MenuKind::Full => match m.win {
                Window::Circle | Window::Diff => {
                    let diff = m.win == Window::Diff;
                    let (between, reset, readout) = self.plane_opts(diff);
                    dg::plane_menu(diff, self.channels_entries(m.win), reset, between, readout)
                }
                Window::Vector => dg::vector_menu(self.channels_entries(m.win), self.angle_entries(), self.vec_show_values, self.vec_direction_only),
                Window::Freq => dg::freq_menu(self.channels_entries(m.win)),
            },
        }
    }

    /// Run a diagram command (`at` = where a drop-down pops up).
    pub fn diag_cmd(&mut self, w: Window, cmd: DiagCmd, at: Pos2) {
        let diff = w == Window::Diff;
        let gi = usize::from(diff);
        match cmd {
            DiagCmd::ChannelsMenu => self.open_diag_menu(w, MenuKind::Channels, at),
            DiagCmd::AngleMenu => self.open_diag_menu(w, MenuKind::Angles, at),
            DiagCmd::CreatePairs => {
                if let Some(d) = self.active_doc_mut() {
                    d.purge_pairs();
                }
                if let Some(d) = self.active_doc() {
                    let pairs = if diff { d.dif_pairs.clone() } else { d.rx_pairs.clone() };
                    // Both combos open on their first channel (`0x4216b0`,
                    // CB_SETCURSEL 0), so «Добавить» is usable at once.
                    let first = doc_chans(d).into_iter().next().map(|c| c.name);
                    self.diag.pair_dlg = Some(PairDialog { diff, pairs, a: first.clone(), b: first, sel: None, list_focus: false });
                }
            }
            DiagCmd::ShowAll(on) => {
                let Some(d) = self.active_doc_mut() else { return };
                match w {
                    Window::Diff => d.dif_pairs.iter_mut().for_each(|p| p.on = on),
                    _ => {
                        // «Показать все» sets the flag on every listed channel
                        // — the circle's skips one pending a merge (bit 14,
                        // `0x437150`) —, «Скрыть все» clears it on all.
                        let chans = doc_chans(d);
                        for c in &chans {
                            if !on || listed(w, c) && !(w == Window::Circle && c.pending) {
                                set_chan_flag(d, w, c, on);
                            }
                        }
                        if w == Window::Circle {
                            d.rx_pairs.iter_mut().for_each(|p| p.on = on);
                        }
                    }
                }
                // Every list command marks the document modified.
                d.set.modified = true;
            }
            DiagCmd::ToggleChan(idx) => {
                let Some(d) = self.active_doc_mut() else { return };
                let chans = doc_chans(d);
                if let Some(c) = chans.get(idx) {
                    let v = !chan_flag(d, w, c);
                    set_chan_flag(d, w, c, v);
                    d.set.modified = true;
                }
            }
            DiagCmd::TogglePair(j) => {
                let Some(d) = self.active_doc_mut() else { return };
                let list = if diff { &mut d.dif_pairs } else { &mut d.rx_pairs };
                if let Some(p) = list.get_mut(j) {
                    p.on = !p.on;
                    d.set.modified = true;
                }
            }
            DiagCmd::ShowReset => {
                if diff {
                    self.diag.diff_show_reset = !self.diag.diff_show_reset;
                } else {
                    self.show_reset = !self.show_reset;
                }
            }
            DiagCmd::BetweenCursors => {
                if diff {
                    self.diag.diff_between = !self.diag.diff_between;
                } else {
                    self.circle_between_cursors = !self.circle_between_cursors;
                }
            }
            DiagCmd::Readout(m) => {
                if diff {
                    self.diag.diff_values = !self.diag.diff_values;
                } else {
                    // Mutually exclusive; clicking the checked one turns it off.
                    self.circle_readout = if self.circle_readout == m { 0 } else { m };
                }
            }
            DiagCmd::Reload => self.reload_bases(),
            DiagCmd::Zoom(zin) => {
                if let Some(g) = self.diag.geom[gi] {
                    self.plane_view(diff).zoom(zin, g.char_max, |z| g.ppu_at(z));
                }
            }
            DiagCmd::Pan(x_axis, delta) => {
                if let Some(g) = self.diag.geom[gi] {
                    let z = self.plane_view(diff).z;
                    let limit = dg::max_pan(if x_axis { g.pan.0 } else { g.pan.1 }, z);
                    self.plane_view(diff).pan(x_axis, delta, limit);
                }
            }
            // 36000+ (`0x48f8d0`): saved and modified even when unchanged.
            DiagCmd::AngleRef(r) => {
                self.vec_angle_ref = r;
                if let Some(d) = self.active_doc_mut() {
                    d.set.modified = true;
                }
            }
            DiagCmd::ShowValues => self.vec_show_values = !self.vec_show_values,
            DiagCmd::DirectionOnly => self.vec_direction_only = !self.vec_direction_only,
            DiagCmd::FreqSettings => {
                self.diag.freq_dlg = Some(FreqDialog {
                    percent: self.freq_percent,
                    use_rms: self.freq_use_rms,
                    harmonics: self.freq_harmonics.to_string(),
                    periods: self.freq_periods.to_string(),
                    fail: None,
                    radios_on: self.freq_percent,
                })
            }
        }
    }

    /// 32864 «Перезагрузить базы характеристик» (`0x436080` → `0x40c7e0`):
    /// load the `*.fvb` bases again; the loader ends on «Нет».
    pub fn reload_bases(&mut self) {
        let (lib, name) = crate::app::load_libraries();
        self.full_library = lib;
        self.source_name = name;
        self.diagrams = [None, None];
        self.library = crate::app::plane_library(&self.full_library, self.diagram_dim);
        self.block_idx = 0;
        self.chars.clear();
        self.selected_char = None;
        self.ensure_block_state();
    }

    /// The open diagram menu and the diagram dialogs, once per frame.
    pub fn diagram_overlays(&mut self, ctx: &egui::Context) {
        if let Some(mut m) = self.diag.menu {
            let entries = self.menu_entries(&m);
            let out = popmenu::show(ctx, MENU_ID, &mut m.pm, &entries);
            self.diag.menu = (!out.close).then_some(m);
            if out.eat {
                self.diag.menu_eat = true;
            }
            if let Some(c) = out.chosen {
                self.diag_cmd(m.win, c, m.pm.pos);
            }
        }
        if self.diag.menu_eat && !ctx.input(|i| i.pointer.any_down()) {
            self.diag.menu_eat = false;
        }
        self.pair_dialog_ui(ctx);
        self.freq_dialog_ui(ctx);
    }

    /// Right-click / wheel bookkeeping shared by the windows: opens the
    /// whole menu on a right click unless a press is being swallowed.
    fn window_menu_click(&mut self, resp: &egui::Response, w: Window) {
        // The vector (`0x48fc80`) and frequency (`0x46f2e0`) windows show
        // no menu without a document.
        if matches!(w, Window::Vector | Window::Freq) && self.active_doc().is_none() {
            return;
        }
        if resp.secondary_clicked() && !self.diag.menu_eat {
            if let Some(p) = resp.interact_pointer_pos() {
                self.open_diag_menu(w, MenuKind::Full, p);
            }
        }
    }

    // ---- circle / differential plane ----

    /// The circle (`Ohm`) or differential (`Dif`) plane below the grid.
    pub fn plane_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(d) = self.active_doc_mut() {
            d.purge_pairs();
        }
        let diff = self.current_dimension() == fvb_core::model::Dimension::Dif;
        let win = if diff { Window::Diff } else { Window::Circle };
        let gi = usize::from(diff);
        let full = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(full, egui::Sense::click());
        let (between, show_reset, readout) = self.plane_opts(diff);
        let view = *self.plane_view(diff);
        // Scroll bars take client area while zoomed (`0x437ca0`).
        let sb = dg::scroll_range(view.z);
        let client = if sb.is_some() { Rect::from_min_max(full.min, full.max - Vec2::splat(crate::dpi::s(SCROLL_W))) } else { full };
        let painter = ui.painter_at(full);
        // White below the splitter's 5 rows, which the window keeps.
        painter.rect_filled(Rect::from_min_max(full.min + Vec2::new(0.0, dpx(5.0)), full.max), 0.0, Color32::WHITE);
        self.window_menu_click(&resp, win);
        if client.width() < dpx(121.0) || client.height() < dpx(121.0) {
            // Too small to paint, the painter still runs its tail
            // (`0x445eb2` → `0x44611f`): the differential window's initial
            // push happens on its first paint however small, with this
            // client's limits (VM, L9.403: a first run at the default frame
            // pushes x only).
            if diff && view.initial_pending && print_s().is_none() {
                let mut rect = client.shrink(6.0);
                rect.min.y += dpx(5.0);
                let mut v = view;
                v.push_lower_left(dg::max_pan(rect.width() as i32, v.z), dg::max_pan((rect.height() - 20.0) as i32, v.z));
                *self.plane_view(diff) = v;
            }
            return;
        }
        // Characteristics: extent of every visible one (operate and reset).
        let b = self.block_idx;
        let mut draws = Vec::new();
        // The circle splits them by dimension (`0x4387d0`): string 3060
        // «Ohm» upper-cased (MakeUpper, `0x43897c`), then an exact compare —
        // the shipped «OHM» ones go to the Ω extent, any other text
        // (a literal «Ohm» too) to the power one.
        let mut char_ext = [0.0f64; 2];
        if let Some(block) = self.library.blocks.get(b) {
            for (ci, ch) in block.characteristics.iter().enumerate() {
                let Some(st) = self.chars.get(&(b, ci)) else { continue };
                if !st.visible {
                    continue;
                }
                let Some(geo) = &st.geo else { continue };
                if geo.bounds.is_some() {
                    let s = usize::from(!diff && ch.dimension_text != "OHM");
                    char_ext[s] = char_ext[s].max(st.extent);
                }
                // Open polypoints are closed against the visible area.
                let in_view = match self.char_view {
                    Some(v) if fvb_core::geom::depends_on_view(ch) => Some(fvb_core::geom::build_geometry(ch, &st.figs, Some(v))),
                    _ => None,
                };
                let geo = in_view.as_ref().unwrap_or(geo);
                draws.push(CharDraw { pal: ci % 9, op: geo.operate.shapes.clone(), rs: geo.reset.shapes.clone() });
            }
        }
        let loci = self.active_doc().map(|d| plane_loci(d, diff, between)).unwrap_or_default();
        let mut max = char_ext;
        let char_ext = char_ext[0].max(char_ext[1]);
        if diff {
            // The differential's scale over its own range (`0x4441b0`).
            if let Some(d) = self.active_doc() {
                max[0] = max[0].max(diff_scale_max(d, diff_scale_range(d, between)));
            }
        } else {
            for l in &loci {
                let s = usize::from(l.power);
                for p in &l.pts {
                    for v in p {
                        if v.is_finite() {
                            max[s] = max[s].max(v.abs());
                        }
                    }
                }
            }
        }
        // Layout: client − 6 px, top + 5 (splitter), bottom − legend (20)
        // or − the readout table when there is room for a 120 px plot (the
        // 5 and the 120 in device pixels on paper too, `0x43b679` /
        // `0x43b6b2`).
        // cy: the "Ay" extent in the pane font (`0x446320` / `0x43ba80`) —
        // Tahoma's 13 px cell on screen, Arial's ⌊52s/5⌋ device px in print.
        let text_h = match print_s() {
            Some(s) => (52.0 * s / 5.0).floor() / s,
            None => 13.0f32,
        };
        let mut rect = client.shrink(6.0);
        rect.min.y += dpx(5.0);
        let pan = (rect.width() as i32, (rect.height() - 20.0) as i32);
        let rows = loci.len() as f32;
        let table_h = (text_h + 6.0) * rows + 12.0;
        let table = readout != 0 && rect.height() - table_h >= dpx(120.0);
        rect.max.y -= if table { table_h } else { 20.0 };
        let (w, h) = (rect.width() as i32, rect.height() as i32);
        let gate = (client.width() as i32 - 16, client.height() as i32 - 32);
        let mut view = view;
        let ps = print_s().unwrap_or(1.0);
        let mut sc = dg::plane_scale_s(w, h, view.z, max, ps);
        // Device px against 800·s (`0x43b72f`) — in points against 800.
        if dg::zoom_resets(char_ext, sc.ppu[0]) {
            view.z = 1.0;
            sc = dg::plane_scale_s(w, h, 1.0, max, ps);
        }
        let cell = sc.cell_pt;
        let c = Pos2::new(rect.left() + (w / 2) as f32, rect.top() + (h / 2) as f32);
        let origin = c + Vec2::new(view.ox as f32 * cell, view.oy as f32 * cell);
        let (nx, ny) = sc.half_cells;
        // Grid (`0x4393b0`): lines every cell outward from the centre, the
        // frame at a whole number of cells, axes through the origin.
        let (fx, fy) = (nx as f32 * cell, ny as f32 * cell);
        let frame = Rect::from_min_max(c - Vec2::new(fx, fy), c + Vec2::new(fx, fy));
        // Printed pens (`0x439486`): grid (150,150,150) ⌊s/2⌋, frame
        // (50,50,50) and axes black ⌊2s/3⌋.
        let (gs, fs, axs) = match print_look() {
            Some(s) => (dev_pen(s / 2.0, AXES), dev_pen(2.0 * s / 3.0, Color32::from_rgb(50, 50, 50)), dev_pen(2.0 * s / 3.0, Color32::BLACK)),
            None => (stroke(1.0, GRID), stroke(1.0, FRAME), stroke(1.0, AXES)),
        };
        for k in 0..ny {
            for y in [c.y + k as f32 * cell, c.y - k as f32 * cell] {
                painter.px_segment([Pos2::new(frame.left(), y), Pos2::new(frame.right(), y)], gs);
            }
        }
        for k in 0..nx {
            for x in [c.x + k as f32 * cell, c.x - k as f32 * cell] {
                painter.px_segment([Pos2::new(x, frame.top()), Pos2::new(x, frame.bottom())], gs);
            }
        }
        // The frame as four aligned 1 px lines (a stroked rect straddles
        // its outline and blurs over two pixels; VM: one crisp row). GDI's
        // Rectangle (`0x444b73`, `0x4398ff`) leaves out its right and bottom
        // edges: those lines are a device pixel in.
        let (fr, fb) = (frame.right() - dpx(1.0), frame.bottom() - dpx(1.0));
        painter.px_hline(frame.left()..=fr, frame.top(), fs);
        painter.px_hline(frame.left()..=fr, fb, fs);
        painter.px_vline(frame.left(), frame.top()..=fb, fs);
        painter.px_vline(fr, frame.top()..=fb, fs);
        if (rect.left()..=rect.right()).contains(&origin.x) {
            painter.px_segment([Pos2::new(origin.x, frame.top()), Pos2::new(origin.x, frame.bottom())], axs);
        }
        if (rect.top()..=rect.bottom()).contains(&origin.y) {
            painter.px_segment([Pos2::new(frame.left(), origin.y), Pos2::new(frame.right(), origin.y)], axs);
        }
        // Edge labels: the coordinate at each frame edge (pan included).
        let edge = |n: i32, o: i32, s: usize, sign: f64| (sign * f64::from(n) - f64::from(o)) * sc.upc[s];
        let edge_y = |n: i32, s: usize, sign: f64| (f64::from(view.oy) + sign * f64::from(n)) * sc.upc[s];
        // `" R: %g %s%s, %g %s%s"`: the Ω value goes to `0x43d960` with the
        // unit «Ω», which it never prefixes; the power one with "W"; the
        // unit texts are "Ohm" / "W" / "var" as shown (`0x42b740`).
        // C's `%g` keeps a zero's sign: with no loci (scale 0) the left
        // edge, −(n·u) − ox·u, is −0 (VM: «R: -0 Ом, -0 Вт» on the left,
        // «0» on the other three edges, `0x439b3a`–`0x439d7d`).
        let g_signed = |v: f64| if v == 0.0 && v.is_sign_negative() { "-0".to_string() } else { fvb_core::views::fmt_g(v) };
        let fmt2 = |axis: &str, a: f64, b: f64, ub: &str| {
            let (sb, pb) = fvb_core::omp::si(b);
            format!(" {axis}: {} {}, {} {pb}{}", g_signed(a), fvb_core::units::ru("Ohm"), g_signed(sb), fvb_core::units::ru(ub))
        };
        // The circle plane's edges in the original's order of operations:
        // ±(n·u) − ox·u across, oy·u ± n·u up and down.
        let cedge = |s: usize, sign: f64| sign * (f64::from(nx) * sc.upc[s]) - f64::from(view.ox) * sc.upc[s];
        let cedge_y = |s: usize, sign: f64| f64::from(view.oy) * sc.upc[s] + sign * (f64::from(ny) * sc.upc[s]);
        // The differential plane: strings 163 « Iторм: %g %s%s» (left /
        // right) and 161 « Iдиф: %g %s%s» (top / bottom), unit string 162
        // «ном», never prefixed.
        let fmt1 = |axis: &str, a: f64| format!(" {axis}: {} ном", fvb_core::views::fmt_g(a));
        let labels: [(String, Pos2, egui::Align2); 4] = if diff {
            [
                (fmt1("Iторм", edge(nx, view.ox, 0, 1.0)), Pos2::new(frame.right() - 3.0, c.y - 3.0), egui::Align2::RIGHT_BOTTOM),
                (fmt1("Iторм", edge(nx, view.ox, 0, -1.0)), Pos2::new(frame.left() + 3.0, c.y - 3.0), egui::Align2::LEFT_BOTTOM),
                (fmt1("Iдиф", edge_y(ny, 0, 1.0)), Pos2::new(c.x + 3.0, frame.top() + 3.0), egui::Align2::LEFT_TOP),
                (fmt1("Iдиф", edge_y(ny, 0, -1.0)), Pos2::new(c.x + 3.0, frame.bottom() - 3.0), egui::Align2::LEFT_BOTTOM),
            ]
        } else {
            [
                (fmt2("R", cedge(0, 1.0), cedge(1, 1.0), "W"), Pos2::new(frame.right() - 3.0, c.y - 3.0), egui::Align2::RIGHT_BOTTOM),
                (fmt2("R", cedge(0, -1.0), cedge(1, -1.0), "W"), Pos2::new(frame.left() + 3.0, c.y - 3.0), egui::Align2::LEFT_BOTTOM),
                (fmt2("X", cedge_y(0, 1.0), cedge_y(1, 1.0), "var"), Pos2::new(c.x + 3.0, frame.top() + 3.0), egui::Align2::LEFT_TOP),
                (fmt2("X", cedge_y(0, -1.0), cedge_y(1, -1.0), "var"), Pos2::new(c.x + 3.0, frame.bottom() - 3.0), egui::Align2::LEFT_BOTTOM),
            ]
        };
        // The DC keeps its default OPAQUE mode here: each text cell is
        // painted white first, hiding the grid under it.
        let opaque = |at: Pos2, align: egui::Align2, t: &str| {
            let gal = painter.layout_no_wrap(t.to_string(), font(), TEXT);
            let r = align.anchor_size(at, gal.size());
            painter.rect_filled(r, 0.0, Color32::WHITE);
            painter.galley(r.min, gal, TEXT);
        };
        for (t, at, align) in &labels {
            opaque(*at, *align, t);
        }
        // Scale box: one cell filled (240,240,240) and the value of a cell.
        let bx = Rect::from_min_size(Pos2::new(frame.left() + cell, frame.bottom() - 2.0 * cell), Vec2::splat(cell));
        // Outlined with the axes pen (`0x444630`) — black ⌊2s/3⌋ on paper.
        painter.rect(bx, 0.0, Color32::from_gray(240), axs);
        // `" %g %s%s, %g %s%s"` (Ω never prefixed); the differential plane
        // `" %g %S%s"` with «ном».
        let scale_text = if diff {
            format!(" {} ном", fvb_core::views::fmt_g(sc.upc[0]))
        } else {
            let (tb, pb) = g_si(sc.upc[1]);
            format!(" {} {}, {tb} {pb}{}", fvb_core::views::fmt_g(sc.upc[0]), fvb_core::units::ru("Ohm"), fvb_core::units::ru("W"))
        };
        opaque(Pos2::new(bx.right() + 3.0, bx.center().y), egui::Align2::LEFT_CENTER, &scale_text);
        let to_px = |x: f64, y: f64, ppu: f64| [origin.x + (x * ppu) as f32, origin.y - (y * ppu) as f32];
        let clip = painter.with_clip_rect(client);
        // Characteristics (`0x43a4a0`), last first so the first ends on top,
        // only while their extent stays under 800 px.
        if let Some(ppu) = sc.ppu[0].filter(|ppu| char_ext * ppu < dg::ZOOM_RESET_PX) {
            for d in draws.iter().rev() {
                let [op, rs, fl, _] = dg::CHAR_PALETTE[d.pal];
                // Polygon's whole-pixel points: the scaled offsets
                // truncated (VM: the slopes' pixels as the original's).
                let rings = |shapes: &[Shape]| -> Vec<Vec<Vec<[f32; 2]>>> {
                    shapes
                        .iter()
                        .map(|Shape::Region(rr)| rr.iter().filter(|r| r.len() >= 3).map(|r| r.iter().map(|p| [origin.x + (p[0] * ppu).trunc() as f32, origin.y - (p[1] * ppu).trunc() as f32]).collect()).collect())
                        .collect()
                };
                if show_reset {
                    for r in rings(&d.rs) {
                        paint_region(&clip, &r, None, rgb(rs), client);
                    }
                }
                let (r, g, bb) = fl;
                for r_ in rings(&d.op) {
                    paint_region(&clip, &r_, Some(crate::theme::alpha(r, g, bb, dg::FILL_ALPHA)), rgb(op), client);
                }
            }
        }
        // Loci (`0x43a880`): the channels' polylines, then their "+" at
        // cursor 1 (200, 0, 0) and cursor 2 (0, 0, 200), then the R/X
        // pairs' polylines.
        // (`0x4457ea` / `0x445831` for the differential plane: the same.)
        let (red, blue) = (Color32::from_rgb(200, 0, 0), Color32::from_rgb(0, 0, 200));
        let ordered = loci.iter().filter(|l| l.markers).map(|l| (l, false)).chain(loci.iter().filter(|l| l.markers).map(|l| (l, true))).chain(loci.iter().filter(|l| !l.markers).map(|l| (l, false)));
        for (l, marks) in ordered {
            let Some(ppu) = sc.ppu[usize::from(l.power && !diff)] else { continue };
            let mut run: Vec<Pos2> = Vec::new();
            let flush = |run: &mut Vec<Pos2>| {
                if run.len() >= 2 {
                    clip.add(egui::Shape::line(std::mem::take(run), stroke(1.0, l.color)));
                }
                run.clear();
            };
            if !marks {
                for p in &l.pts {
                    if p[0].is_finite() && p[1].is_finite() {
                        let q = to_px(p[0], p[1], ppu);
                        run.push(Pos2::new(q[0], q[1]));
                    } else {
                        flush(&mut run);
                    }
                }
                flush(&mut run);
                continue;
            }
            for (p, col) in [(l.c1, red), (l.c2, blue)] {
                if p[0].is_finite() && p[1].is_finite() {
                    let q = to_px(p[0], p[1], ppu);
                    let q = Pos2::new(q[0], q[1]);
                    // Arms ±4·s; the pens are function-static (made at the
                    // first, screen, call), so a print keeps them 1 px.
                    let w = 1.0 / print_s().unwrap_or(1.0);
                    clip.line_segment([q - Vec2::new(4.0, 0.0), q + Vec2::new(4.0, 0.0)], stroke(w, col));
                    clip.line_segment([q - Vec2::new(0.0, 4.0), q + Vec2::new(0.0, 4.0)], stroke(w, col));
                }
            }
        }
        // Real print only (`0x43b832` / `0x446078`): four white
        // FillSolidRect(x, y, width, height) measured from the PAGE origin
        // around the frame inflated by ⌊2s/3⌋ + 1 — (0, 0, R, T),
        // (0, 0, L, B), (R, 0, L, B), (0, B, L + R, T) in page terms — crop
        // what overflows it, oddly sized.
        if let (Some(s), Some(o)) = (print_s(), PRINT_PAGE.with(std::cell::Cell::get)) {
            let f = frame.expand(((2.0 * s / 3.0).floor() + 1.0) / s);
            let (l, t, r, b) = (f.left() + o.x, f.top() + o.y, f.right() + o.x, f.bottom() + o.y);
            for (x, y, w, h) in [(0.0, 0.0, r, t), (0.0, 0.0, l, b), (r, 0.0, l, b), (0.0, b, l + r, t)] {
                painter.rect_filled(Rect::from_min_size(Pos2::new(x - o.x, y - o.y), Vec2::new(w, h)), 0.0, Color32::WHITE);
            }
        }
        if table {
            readout_table(&painter, ui, rect, text_h, &loci, readout, diff);
        } else {
            legend(&painter, ui, rect, &loci);
        }
        // World view of the plot for open polypoints.
        if let Some(ppu) = sc.ppu[0] {
            let wx = |x: f32| f64::from(x - origin.x) / ppu;
            let wy = |y: f32| f64::from(origin.y - y) / ppu;
            let next = Some((wx(rect.left()), wy(rect.bottom()), wx(rect.right()), wy(rect.top())));
            if next != self.char_view {
                self.char_view = next;
                ui.ctx().request_repaint();
            }
        }
        // Scroll bars.
        let lim = (dg::max_pan(pan.0, view.z), dg::max_pan(pan.1, view.z));
        if let Some((page, m)) = sb {
            let hb = Rect::from_min_max(Pos2::new(full.left(), client.bottom()), Pos2::new(client.right(), full.bottom()));
            let vb = Rect::from_min_max(Pos2::new(client.right(), full.top()), Pos2::new(full.right(), client.bottom()));
            if let Some(pos) = scrollbar(ui, hb, true, dg::scroll_pos(view.ox, lim.0, m), page, m, (MENU_ID, "h", gi)) {
                view.ox = if view.z <= 1.0 { 0 } else { dg::pan_from_scroll(pos, lim.0, m) };
            }
            if let Some(pos) = scrollbar(ui, vb, false, dg::scroll_pos(view.oy, lim.1, m), page, m, (MENU_ID, "v", gi)) {
                view.oy = if view.z <= 1.0 { 0 } else { dg::pan_from_scroll(pos, lim.1, m) };
            }
        }
        // Wheel (`0x437ae0`): one step per WM_MOUSEWHEEL; wParam is
        // exactly MK_CONTROL / MK_SHIFT only with no mouse button held.
        let over_bar = ui.ctx().pointer_hover_pos().is_some_and(|p| {
            self.diag.toolbar[gi].is_some_and(|r| r.contains(p)) && ui.ctx().layer_id_at(p) == Some(ui.layer_id())
        });
        if (resp.hovered() || over_bar) && self.diag.menu.is_none() {
            let (wheels, mods, buttons) = ui.input(|i| (crate::lanes::wheel_deltas(&i.events), i.modifiers, i.pointer.any_down()));
            for delta in wheels {
                let g = PlaneGeom { w, h, pan, max, char_max: char_ext, gate };
                match dg::wheel_action(delta > 0.0, (mods.ctrl || mods.command) && !buttons, mods.shift && !buttons) {
                    WheelAct::Zoom(zin) => {
                        view.zoom(zin, char_ext, |z| g.ppu_at(z));
                    }
                    WheelAct::PanX(d) => {
                        view.pan(true, d, lim.0);
                    }
                    WheelAct::PanY(d) => {
                        view.pan(false, d, lim.1);
                    }
                }
            }
        }
        // The differential window's initial view, once after its first paint.
        // The initial push (`0x445e00` tail, `0x44612d`–`0x44619e`) has no
        // print test: a print consumes it too, after its page is drawn, with
        // the limits from the live window's client (`0x443620`).
        let printing = print_s().is_some();
        if diff && view.initial_pending {
            let live = if printing { self.diag.geom[gi].map(|g| (dg::max_pan(g.pan.0, view.z), dg::max_pan(g.pan.1, view.z))) } else { Some(lim) };
            if let Some(l) = live {
                view.push_lower_left(l.0, l.1);
                ui.ctx().request_repaint();
            }
        }
        *self.plane_view(diff) = view;
        // The page's geometry is not the live window's.
        if !printing {
            self.diag.geom[gi] = Some(PlaneGeom { w, h, pan, max, char_max: char_ext, gate });
        }
    }

    // ---- vector diagram ----

    /// Vector diagram (`0x490360`): no zoom, pan or wheel; options from the
    /// toolbar and MENU 355.
    pub fn vector_ui(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(full, egui::Sense::click());
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, Color32::WHITE);
        self.window_menu_click(&resp, Window::Vector);
        let (radians, dir_only, show_values) = (self.vec_radians, self.vec_direction_only, self.vec_show_values);
        let angle_ref = self.vec_angle_ref.clone();
        let Some(d) = self.active_doc() else { return };
        let k = d.main_cursor;
        // The table's value cells stay empty until В1 has been placed (VM:
        // a record just opened, also after a quick-analysis run, shows the
        // rows with empty cells; a click, the 32813 toggle or the keys
        // place it and the values stay when it is hidden again).
        let values = d.v1_placed || d.v_shown[0];
        let phasor_of = |name: &str| -> Option<Complex64> {
            if let Some(i) = d.osc.analog.iter().position(|c| c.id == name) {
                return d.analysis.phasors.get(i).and_then(|p| p.get(k)).copied();
            }
            d.computed.iter().enumerate().find(|(_, c)| c.name == name).map(|(j, c)| {
                // A pasted channel: its own cache by own time (`0x490120`),
                // 0 for a digital one.
                if let Some((t, _)) = d.pasted_times(j) {
                    return t.phasor_at(k, &c.pending_phasors);
                }
                match &c.phasors {
                    Some(p) => p.get(k).copied().unwrap_or_default(),
                    None => fvb_core::freqcorr::phasor_at(d.computed_corr(j), fvb_core::freqcorr::OTHER, &c.samples, k, d.computed_period(j)),
                }
            })
        };
        let shown: Vec<(String, String, Complex64, Color32, fvb_core::units::Shown)> = doc_chans(d)
            .into_iter()
            .filter(|c| chan_flag(d, Window::Vector, c))
            .map(|c| {
                let z = fvb_core::views::finite_or_zero(phasor_of(&c.name).unwrap_or_default());
                (c.name, c.unit, z, c.color, c.shown)
            })
            .collect();
        let reference = angle_ref.as_deref().and_then(phasor_of).map(fvb_core::views::finite_or_zero);
        let is_current = |unit: &str| unit == "A";
        let group_max = |current: bool| shown.iter().filter(|s| is_current(&s.1) == current).map(|s| s.2.norm()).fold(0.0, f64::max);
        let (i_scale, u_scale) = (fvb_core::views::nice_scale(group_max(true)), fvb_core::views::nice_scale(group_max(false)));
        // Paint order (`0x490360`): the bottom band, the header, circles,
        // radials and angle labels, then each phasor with its table row.
        let s = print_s().unwrap_or(1.0);
        let ps = print_look();
        let ink = Color32::from_rgb(20, 20, 20);
        let measure = |t: &str| painter.layout_no_wrap(t.to_string(), font(), ink).size();
        let m = 6.0;
        let cy = measure("Ay").y;
        let h = (3.0 * cy * s / 2.0).floor() / s;
        let sq = (4.0 * cy * s / 5.0).floor() / s;
        let rect = (full.left(), full.top(), full.right(), full.bottom());
        let off = vector_header_off(&painter, full, [u_scale, i_scale]);
        let lay = dg::vector_layout(rect, m, cy, h, off, shown.len(), show_values);
        let black = dev_pen(s, Color32::BLACK);
        let swatch = |r: Rect, col: Color32| {
            painter.rect_filled(r, 0.0, col);
            painter.px_rect_stroke(r, 0.0, black);
        };
        let grid = match ps {
            Some(s) => dev_pen(s / 2.0, Color32::from_rgb(100, 100, 100)),
            None => stroke(1.0, Color32::from_rgb(150, 150, 150)),
        };
        // The table's columns: names, magnitudes, angles.
        let name_w = shown.iter().map(|c| measure(&c.0).x).fold(0.0, f32::max) + sq + 5.0 * m;
        let val_w = measure("-0.0000000").x + 4.0 * m;
        let tw = name_w + 2.0 * val_w;
        let xoff = if tw <= full.width() { ((full.width() - tw) / 2.0).floor() } else { m };
        if lay.table {
            let (x0, x1) = (full.left() + xoff, full.left() + xoff + tw);
            let y1 = lay.b + shown.len() as f32 * h;
            painter.px_hline(x0..=x1, lay.b, grid);
            for (row, c) in shown.iter().enumerate() {
                let y = lay.b + row as f32 * h;
                painter.px_hline(x0..=x1, y + h, grid);
                let third = (cy / 3.0).floor();
                swatch(Rect::from_min_max(Pos2::new(x0 + third, y + third), Pos2::new(x0 + third + cy - 1.0, y + h - third)), c.3);
                painter.text(Pos2::new(x0 + cy + (cy / 2.0).floor(), y + cy / 4.0), egui::Align2::LEFT_TOP, &c.0, font(), ink);
            }
            for x in [x0, x0 + name_w, x1 - val_w, x1] {
                painter.px_vline(x, lay.b..=y1, grid);
            }
        } else {
            // The legend: one centred row (from x = m when it does not fit).
            let total: f32 = shown.iter().map(|c| measure(&c.0).x + 2.0 * m + sq).sum();
            let mut x = dg::vector_legend_x(full.left(), full.width(), total, m);
            for c in &shown {
                swatch(Rect::from_min_size(Pos2::new(x, lay.b + dpx(2.0)), Vec2::splat(sq)), c.3);
                painter.text(Pos2::new(x + sq + m, lay.b), egui::Align2::LEFT_TOP, &c.0, font(), ink);
                x += measure(&c.0).x + 2.0 * m + sq;
            }
        }
        vector_header(&painter, full, [u_scale, i_scale]);
        if lay.r <= 0.0 {
            return;
        }
        let (r, c) = (lay.r, Pos2::new(lay.cx, lay.cy));
        // Circles k·R/n from the outer one in; printed pens are solid
        // (100,100,100) and (50,50,50), ⌊s/3⌋ wide; the count takes R in
        // device pixels.
        let nc = dg::vector_circles(r * s);
        let inner = |a: Pos2, b: Pos2| match ps {
            Some(s) => {
                painter.line_segment([a, b], dev_pen(s / 3.0, Color32::from_rgb(100, 100, 100)));
            }
            // GDI's dotted pen (VM: one pixel column, 3 on, 3 off).
            None => gdi_dots(&painter, &line_pixels((a.x as i32, a.y as i32), (b.x as i32, b.y as i32)), FRAME),
        };
        for j in (1..=nc).rev() {
            let rr = ((r * s) as usize * j / nc) as f32 / s;
            if let Some(s) = ps {
                let col = if j == nc { Color32::from_rgb(50, 50, 50) } else { Color32::from_rgb(100, 100, 100) };
                painter.circle_stroke(c, rr, dev_pen(s / 3.0, col));
            } else if j == nc {
                // Ellipse's box (VM: 2070…2523 about 2297).
                gdi_pixels(&painter, &circle_pixels((c.x as i32, c.y as i32), rr as i32), AXES, false);
            } else {
                gdi_dots(&painter, &circle_pixels((c.x as i32, c.y as i32), rr as i32), FRAME);
            }
        }
        let rhu = |v: f32| crate::oscgeom::round_half_up(f64::from(v)) as f32;
        for j in 0..12 {
            let a = j as f32 * std::f32::consts::PI / 6.0;
            inner(c, Pos2::new(rhu(c.x + r * a.cos()), rhu(c.y + r * a.sin())));
        }
        // 0° up, 180° down, 90° left, −90° right — inside the circle (the
        // offsets in device pixels on paper too, `0x491bfc`–`0x491c55`).
        let lbl = |deg: &str, rad: &str| if radians { rad.to_string() } else { deg.to_string() };
        painter.text(Pos2::new(c.x + dpx(2.0), c.y - r), egui::Align2::LEFT_TOP, lbl("0º", "0"), font(), ink);
        painter.text(Pos2::new(c.x, c.y + r - dpx(2.0)), egui::Align2::LEFT_BOTTOM, lbl("180º", "π"), font(), ink);
        painter.text(Pos2::new(c.x - r + dpx(7.0), c.y), egui::Align2::LEFT_BOTTOM, lbl("90º", "π/2"), font(), ink);
        painter.text(Pos2::new(c.x + r - dpx(2.0), c.y), egui::Align2::RIGHT_BOTTOM, lbl("-90º", "-π/2"), font(), ink);
        for (row, (name, unit, z, color, unit_text)) in shown.iter().enumerate() {
            let is_ref = angle_ref.as_deref() == Some(name.as_str());
            let ang = if is_ref { 0.0 } else { fvb_core::views::vector_angle(*z, reference) };
            let scale = if is_current(unit) { i_scale } else { u_scale };
            let len = if scale <= 0.0 {
                0.0
            } else if dir_only {
                r
            } else {
                rhu((f64::from(r) * z.norm() / scale) as f32)
            };
            // The arrow in the up-pointing frame, turned by θ = −angle.
            let hl = (18.0f32).min((len * s / 2.0).floor() / s);
            let w = ((hl * s) as i32 / 5) as f32 / s;
            let (sin, cos) = ((-ang) as f32).sin_cos();
            let rot = |x: f32, y: f32| Pos2::new(rhu(x * cos - y * sin + c.x), rhu(x * sin + y * cos + c.y));
            let (p0, p1, p2, p3) = (rot(0.0, 0.0), rot(0.0, -len), rot(w, hl - len), rot(-w, hl - len));
            // A pen s wide: solid for V, PS_DOT otherwise — which GDI draws
            // solid once wider than 1, i.e. on paper.
            let pen = dev_pen(s, *color);
            if unit == "V" || s > 1.0 {
                painter.line_segment([p0, p1], pen);
            } else {
                painter.extend(egui::Shape::dashed_line(&[p0, p1], pen, 3.0, 2.0));
            }
            painter.add(egui::Shape::convex_polygon(vec![p1, p2, p3], *color, pen));
            if lay.table && values {
                let y = lay.b + row as f32 * h + cy / 4.0;
                painter.text(Pos2::new(full.left() + xoff + name_w + m, y), egui::Align2::LEFT_TOP, fvb_core::views::vector_value_text(z.norm(), unit_text), font(), ink);
                painter.text(Pos2::new(full.right() - xoff - val_w + m, y), egui::Align2::LEFT_TOP, fvb_core::views::angle_text(ang, radians), font(), ink);
            }
        }
    }

    // ---- frequency diagram ----

    /// Frequency diagram (`0x46f5a0`, ledger L9.67): one panel per
    /// frequency-flagged channel side by side, K + 1 bar slots each; bars
    /// normalised to the channel's largest bin (bin 0 included), labels
    /// in percent of the fundamental (or RMS) or absolute, no grid, no zoom
    /// or pan. Geometry in device pixels at the print scale `s`.
    pub fn frequency_ui(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(full, egui::Sense::click());
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, Color32::WHITE);
        self.window_menu_click(&resp, Window::Freq);
        let (periods, percent, use_rms) = (self.freq_periods.max(1), self.freq_percent, self.freq_use_rms);
        let Some(d) = self.active_doc() else { return };
        let n = d.osc.samples_per_cycle().max(2);
        let pos = d.main_cursor;
        // NumHarmonics is capped at the Nyquist bin of the window.
        let k_max = self.freq_harmonics.min(n * periods / 2);
        let f = d.osc.line_freq;
        // Only the diagram's own flag counts (`0x46fb1c`), not the lane's
        // visibility on the oscillogram.
        // Formula channels too, after the file's, in vector order
        // (`0x46fb1c` walks the whole vector by bit 1, L9.188).
        struct FreqChan<'a> {
            name: &'a str,
            color: Color32,
            samples: &'a [f64],
            /// The file channel (its frequency correction and k).
            file: Option<usize>,
            /// Its own rate (`ch+0x108`): the window and the correction's rate.
            own_rate: f64,
        }
        let chans: Vec<FreqChan> = (0..d.osc.analog.len())
            .filter(|i| d.chans[*i].on_freq)
            .map(|i| FreqChan { name: &d.osc.analog[i].id, color: d.chans[i].color, samples: d.shown_samples(i), file: Some(i), own_rate: d.osc.analog[i].own_rate })
            .chain(d.computed.iter().enumerate().filter(|(j, c)| c.on_freq && d.pending_of(*j).is_none()).map(|(_, c)| FreqChan { name: &c.name, color: c.color, samples: &c.samples, file: None, own_rate: c.own_rate }))
            .collect();
        let s = print_s().unwrap_or(1.0);
        let si = s as i32;
        let ink = Color32::from_gray(20);
        let h = (painter.layout_no_wrap("A".into(), font(), ink).size().y * s).round() as i32;
        let dev = |v: f32| (v * s).round() as i32;
        let pt = |x: i32, y: i32| Pos2::new(x as f32 / s, y as f32 / s);
        let Some(g) = dg::freq_geom((dev(full.left()), dev(full.top()), dev(full.right()), dev(full.bottom())), si, chans.len(), k_max, h) else { return };
        let width = |t: &str, fid: FontId| (painter.layout_no_wrap(t.into(), fid, ink).size().x * s).round() as i32;
        // The rotated label's font: the same 13 px-cell Tahoma (`0x46f7b7`).
        let rot_font = font();
        let corr = fvb_core::freqcorr::of(&d.osc);
        // In print the panels tile the page by the screen pane's aspect.
        let screen = self.pane_canvas.iter().find(|(p, _)| *p == crate::app::Pane::Frequency).map_or((0, 0), |(_, r)| (r.width() as i32, r.height() as i32));
        let tl = if print_look().is_some() { g.print_tiling(chans.len(), k_max, screen) } else { g.screen_tiling(chans.len()) };
        // Pass 1: swatch and name under each panel's row; the x labels
        // (with several rows their rects are inverted: none).
        for (pi, ch) in chans.iter().enumerate() {
            let (c, r) = tl.cell(pi);
            let yb = tl.row_bottom(&g, r);
            let name = format!("{}{}", ch.name, if percent { ", %" } else { "" });
            let x0 = tl.swatch_x(&g, c, width(&name, font()));
            let sw = Rect::from_min_max(pt(x0, yb + 3), pt(x0 + 10 * si, yb + 3 + 10 * si));
            painter.rect_filled(sw, 0.0, ch.color);
            painter.px_rect_stroke(sw.shrink(0.5), 0.0, Stroke::new(1.0f32, Color32::BLACK));
            painter.text(pt(x0 + 16 * si, yb), egui::Align2::LEFT_TOP, name, font(), ink);
            if tl.rows > 1 {
                continue;
            }
            for k in 0..=k_max {
                let t = dg::freq_x_label(k as f64 * f / periods as f64);
                if width(&t, font()) <= tl.slot1 {
                    let l = g.left + tl.label_left(&g, c, k);
                    painter.text(pt(l, g.b - 18 * si) + Vec2::new(tl.slot1 as f32 / s / 2.0, 0.0), egui::Align2::CENTER_TOP, t, font(), ink);
                }
            }
        }
        // Pass 2: the bars and their value labels.
        for (pi, ch) in chans.iter().enumerate() {
            let (c, r) = tl.cell(pi);
            let (row_top, yb) = (g.top + r * tl.row_h, tl.row_bottom(&g, r));
            let samples = ch.samples;
            let n = d.osc.period_of(ch.own_rate).max(2);
            // The spectrum and the norm sum the samples as `0x426020` returns them (times k, each one).
            let sp: Vec<f64> = freq_bins(d, samples, ch.file, ch.own_rate, pos, periods, k_max.max(periods));
            let max = sp[..=k_max].iter().copied().fold(0.0, f64::max);
            let max = if max == 0.0 { 1.0 } else { max };
            let norm = if use_rms { fvb_core::freqcorr::rms_at(ch.file.and(corr).map(|c| c.at_rate(ch.own_rate)), ch.file.unwrap_or(0), samples, pos, n, periods) } else { sp.get(periods).copied().unwrap_or(0.0) };
            let color = ch.color;
            if g.plot_h() - 30 * si - g.h <= 0 {
                continue;
            }
            for (k, &v) in sp[..=k_max].iter().enumerate() {
                let t = tl.bar_offset(&g, v, max);
                let sl = g.left + tl.slot_left(&g, c, k);
                let bl = sl + tl.inset;
                if v != 0.0 {
                    let bar = Rect::from_min_max(pt(bl, row_top + 12 * si + g.h + t), pt(bl + tl.bar, yb - 18 * si));
                    // GradientFill (channel colour at the top to white), then
                    // the outline in the channel colour.
                    let mut mesh = egui::Mesh::default();
                    for (p, col) in [(bar.left_top(), color), (bar.right_top(), color), (bar.right_bottom(), Color32::WHITE), (bar.left_bottom(), Color32::WHITE)] {
                        mesh.colored_vertex(p, col);
                    }
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    painter.add(egui::Shape::mesh(mesh));
                    painter.px_rect_stroke(bar.shrink(0.5), 0.0, Stroke::new(1.0f32, color));
                }
                // The fundamental's own bar reads 100 (an improvement, #60); `use_rms` has its own norm.
                let val = if percent { fvb_core::views::percent_bar_value(v, norm, k == periods && !use_rms) } else { v };
                let label = fvb_core::views::freq_label(val).trim().to_string();
                // Measured in the diagrams' Tahoma 13 (`0x4705e6`): its
                // digits advance 6 px, «.» 3 — numbers only reach here.
                let w = dg::tahoma13_width(&label).map_or_else(|| width(&label, font()), |w| (w as f32 * s).round() as i32);
                if 6 * w / 5 <= tl.slot {
                    let at = pt(sl, row_top + t + 6 * si) + Vec2::new(tl.slot as f32 / s / 2.0, 0.0);
                    painter.text(at, egui::Align2::CENTER_TOP, label, font(), ink);
                } else {
                    let wr = width(&label, rot_font.clone());
                    let at = pt(bl, row_top + 6 * si + (t + g.h).max(wr)) - Vec2::new(2.0 / s, 0.0);
                    let gal = painter.layout_no_wrap(label, rot_font.clone(), ink);
                    painter.add(egui::epaint::TextShape::new(at, gal, ink).with_angle(-std::f32::consts::FRAC_PI_2));
                }
            }
        }
        // The axis pen: n separators down the whole height (in print past
        // the columns, too) and one baseline per row, `rem` below the bars
        // when the rows do not divide H.
        let axis = match print_look() {
            Some(s) => dev_pen(s / 2.0, Color32::from_rgb(50, 50, 50)),
            None => stroke(1.0, AXES),
        };
        for c in 0..chans.len() as i32 {
            let x = g.left + 9 * si + c * tl.panel_w;
            painter.px_segment([pt(x, g.top + 12 * si + g.h), pt(x, g.bottom - 6 * si - 1)], axis);
        }
        for idx in 0..tl.rows {
            let y = g.b - 1 - 18 * si - idx * tl.row_h;
            painter.px_segment([pt(g.left + 6 * si, y), pt(g.right - 6 * si, y)], axis);
        }
    }

    // ---- dialogs ----

    /// DIALOG 554 (ChanPairDlg, 33027): the R / X pairs of the circle
    /// diagram or the Iдиф / Iторм pairs of the differential diagram.
    fn pair_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.diag.pair_dlg.take() else { return };
        let names: Vec<String> = self.active_doc().map(|d| doc_chans(d).into_iter().map(|c| c.name).collect()).unwrap_or_default();
        let (title, ca, cb) = if dlg.diff {
            ("Задать соответствие между дифференциальным током и током торможения", "Дифференциальный ток", "Ток торможения")
        } else {
            ("Задать соответствие между активным и реактивным сопротивлениями", "Активное сопротивление", "Реактивное сопротивление")
        };
        let mut done: Option<bool> = None;
        // DIALOG 554: 393 × 367 DLU (590 × 596 px), every control at its
        // rectangle (VM).
        crate::dialogs::dlg(ctx, title).margin(0.0).collapsible(false).resizable(false).show(ctx, |ui| {
            use crate::dialogs::{dlu, win_button_en, win_combo_list, dlg_font};
            use egui::{Color32, Pos2, Rect, Stroke};
            let (client, _) = ui.allocate_exact_size(crate::dialogs::dlu_size(393, 367), egui::Sense::hover());
            let r = |x, y, w, h| dlu(client, x, y, w, h);
            // The two SS_CENTER labels over the two lists.
            for (lr, t) in [(r(18, 18, 168, 8), ca), (r(204, 17, 168, 8), cb)] {
                ui.painter().text(Pos2::new(lr.center().x, lr.top()), egui::Align2::CENTER_TOP, t, dlg_font(), Color32::BLACK);
            }
            // The template's order is the tab order: OK, Отмена, the right
            // combo, the left one, «Добавить», the list, «Удалить».
            if win_button_en(ui, r(252, 348, 50, 14), "OK", true, true).clicked() {
                done = Some(true);
            }
            if win_button_en(ui, r(324, 348, 50, 14), "Отмена", false, true).clicked() {
                done = Some(false);
            }
            for (k, (cr, slot)) in [(1, (r(204, 36, 167, 30), &mut dlg.b)), (0, (r(18, 36, 168, 30), &mut dlg.a))] {
                let cur = slot.as_ref().and_then(|v| names.iter().position(|n| n == v)).unwrap_or(0);
                if let Some(pick) = win_combo_list(ui, cr, egui::Id::new(("pair_combo", k)), &names, cur, !names.is_empty()) {
                    *slot = Some(names[pick].clone());
                }
            }
            // «Добавить» needs both channels; the new row is selected.
            if win_button_en(ui, r(324, 60, 49, 14), "Добавить", false, dlg.a.is_some() && dlg.b.is_some()).clicked() {
                dlg.pairs.push(ChanPair::new(dlg.a.clone().unwrap_or_default(), dlg.b.clone().unwrap_or_default()));
                dlg.sel = Some(dlg.pairs.len() - 1);
            }
            // The list (VM): a (130, 135, 144) border; the header MFC draws
            // — a white row, (247, 250, 252) shading to (212, 221, 233) over
            // 23 rows, a (158, 182, 206) line below and at each column's
            // right edge − 2; columns 265 and 263 px; 17 px rows ruled in
            // (240) down the whole list; the selection (0, 120, 215).
            let lr = r(18, 90, 354, 222);
            let inner = lr.shrink(1.0);
            let p = ui.painter().clone();
            p.rect_filled(lr, 0.0, Color32::from_rgb(130, 135, 144));
            p.rect_filled(inner, 0.0, Color32::WHITE);
            // At > 100 % the header and rows from the font's height (25 /
            // 17 px at 100 %), the columns at the base units.
            let by = crate::dpi::base_units().1 as f32;
            let (hdr_h, row_h) = (by + 12.0, by + 4.0);
            let hdr = Rect::from_min_size(inner.min, Vec2::new(inner.width(), hdr_h));
            let mut mesh = egui::Mesh::default();
            let g = Rect::from_min_max(Pos2::new(hdr.left(), hdr.top() + 1.0), Pos2::new(hdr.right(), hdr.bottom() - 1.0));
            for (pt, c) in [(g.left_top(), (247, 250, 252)), (g.right_top(), (247, 250, 252)), (g.right_bottom(), (212, 221, 233)), (g.left_bottom(), (212, 221, 233))] {
                mesh.colored_vertex(pt, Color32::from_rgb(c.0, c.1, c.2));
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            p.add(mesh);
            let edge = Stroke::new(1.0f32, Color32::from_rgb(158, 182, 206));
            p.hline(hdr.x_range(), hdr.bottom() - 0.5, edge);
            let grid = Stroke::new(1.0f32, Color32::from_gray(240));
            let (c1, c2) = (crate::dpi::dx(265.0), crate::dpi::dx(528.0));
            let cols = [(0.0, c1, ca), (c1, c2 - c1, cb)];
            for (x, w, t) in cols {
                let c = Rect::from_min_size(Pos2::new(inner.left() + x, hdr.top()), Vec2::new(w, hdr_h));
                p.vline(c.right() - 1.5, c.y_range(), edge);
                p.vline(c.right() - 0.5, c.bottom()..=inner.bottom(), grid);
                p.with_clip_rect(c).text(Pos2::new(c.left() + 7.0, c.center().y), egui::Align2::LEFT_CENTER, t, dlg_font(), Color32::BLACK);
            }
            let mut y = hdr.bottom() + row_h - 0.5;
            while y < inner.bottom() {
                p.hline(inner.x_range(), y, grid);
                y += row_h;
            }
            // The list is a tab stop; a press on it gives it the focus.
            let list_id = egui::Id::new("pair_list");
            ui.interact(Rect::from_min_max(Pos2::new(inner.left(), hdr.bottom()), inner.max), list_id, egui::Sense::click());
            dlg.list_focus = crate::dialogs::tab_stop(ui, list_id, crate::dialogs::StopKind::Other);
            for (i, pair) in dlg.pairs.iter().enumerate() {
                let top = hdr.bottom() + row_h * i as f32;
                if top + row_h > inner.bottom() {
                    break;
                }
                let row = Rect::from_min_size(Pos2::new(inner.left(), top), Vec2::new(cols[1].0 + cols[1].1 - 1.0, row_h - 1.0));
                let sel = dlg.sel == Some(i);
                let lit = sel && dlg.list_focus;
                if sel {
                    p.rect_filled(row, 0.0, if lit { Color32::from_rgb(0, 120, 215) } else { Color32::from_gray(240) });
                }
                for ((x, w, _), t) in cols.into_iter().zip([&pair.a, &pair.b]) {
                    let c = Rect::from_min_size(Pos2::new(inner.left() + x, top), Vec2::new(w - 1.0, row_h - 1.0));
                    p.with_clip_rect(c).text(Pos2::new(c.left() + 6.0, c.center().y), egui::Align2::LEFT_CENTER, t, dlg_font(), if lit { Color32::WHITE } else { Color32::BLACK });
                }
                if ui.interact(row, ui.id().with(("pair_row", i)), egui::Sense::click()).clicked() {
                    dlg.sel = Some(i);
                }
            }
            // «Удалить» removes the selected row and selects the next (or last).
            if win_button_en(ui, r(324, 318, 50, 14), "Удалить", false, dlg.sel.is_some()).clicked() {
                if let Some(s) = dlg.sel {
                    dlg.pairs.remove(s);
                    dlg.sel = if dlg.pairs.is_empty() { None } else { Some(s.min(dlg.pairs.len() - 1)) };
                }
            }
        });
        let done = done.or_else(|| self.dialog_keys(ctx));
        match done {
            None => self.diag.pair_dlg = Some(dlg),
            Some(false) => {}
            // The callers (`0x447041`, `0x43d6f1`) copy the list and mark the
            // document modified on every IDOK.
            Some(true) => {
                if let Some(d) = self.active_doc_mut() {
                    if dlg.diff {
                        d.dif_pairs = dlg.pairs;
                    } else {
                        d.rx_pairs = dlg.pairs;
                    }
                    d.set.modified = true;
                }
            }
        }
    }

    /// DIALOG 353 «Настройки частотной диаграммы» (`0x46ec30`); the
    /// normalisation radios are disabled with absolute values (`0x46ee20`).
    fn freq_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.diag.freq_dlg.take() else { return };
        // After the message box: the failing field, its text selected.
        let focus = dlg.fail.filter(|_| self.dlg.alerts.is_empty());
        if focus.is_some() {
            dlg.fail = None;
        }
        let mut done: Option<bool> = None;
        crate::dialogs::dlg(ctx, "Настройки частотной диаграммы").margin(0.0).collapsible(false).resizable(false).show(ctx, |ui| {
            use crate::dialogs::{dlu, win_button, win_edit, win_group, win_label, win_radio_en};
            // The 226 × 222 DLU template (339 × 361 px). The dialog opens
            // with the first edit focused, its text selected.
            let (client, _) = ui.allocate_exact_size(crate::dialogs::dlu_size(226, 222), egui::Sense::hover());
            let first = ui.memory(|m| m.focused().is_none());
            win_group(ui, dlu(client, 12, 12, 204, 54), "Расчет значений");
            win_label(ui, dlu(client, 24, 30, 121, 8), "Число обрабатываемых периодов:", true, true);
            win_edit(ui, dlu(client, 150, 30, 42, 12), egui::Id::new("fv_freq_periods"), &mut dlg.periods, focus == Some(0) || (first && focus.is_none()));
            win_label(ui, dlu(client, 84, 50, 60, 8), "Число гармоник:", true, true);
            win_edit(ui, dlu(client, 150, 48, 42, 12), egui::Id::new("fv_freq_harmonics"), &mut dlg.harmonics, focus == Some(1));
            let mut clicked = false;
            win_group(ui, dlu(client, 12, 72, 204, 54), "Значения величин");
            if win_radio_en(ui, dlu(client, 24, 88, 95, 12), "Абсолютные величины", !dlg.percent, true) {
                dlg.percent = false;
                clicked = true;
            }
            if win_radio_en(ui, dlu(client, 24, 106, 178, 12), "В процентах (нормализовано)", dlg.percent, true) {
                dlg.percent = true;
                clicked = true;
            }
            win_group(ui, dlu(client, 12, 132, 204, 60), "Нормализация");
            if win_radio_en(ui, dlu(client, 24, 150, 126, 12), "По основной гармонике", !dlg.use_rms, dlg.radios_on) {
                dlg.use_rms = false;
                clicked = true;
            }
            if win_radio_en(ui, dlu(client, 24, 168, 150, 12), "По среднеквадратичному значению", dlg.use_rms, dlg.radios_on) {
                dlg.use_rms = true;
                clicked = true;
            }
            if win_button(ui, dlu(client, 114, 200, 50, 14), "OK", true).clicked() {
                done = Some(true);
                clicked = true;
            }
            if win_button(ui, dlu(client, 168, 200, 50, 14), "Отмена", false).clicked() {
                done = Some(false);
            }
            if clicked {
                dlg.radios_on = dlg.percent;
            }
        });
        let done = done.or_else(|| self.dialog_keys(ctx));
        match done {
            None => self.diag.freq_dlg = Some(dlg),
            Some(false) => {}
            Some(true) => self.apply_freq_dialog(dlg),
        }
    }

    /// OK of DIALOG 353: the exchange as read, or the message with the
    /// dialog kept.
    fn apply_freq_dialog(&mut self, mut dlg: FreqDialog) {
        let (n, r, f) = self.active_doc().map_or((usize::MAX, 0.0, 50.0), |d| (d.osc.sample_count(), d.osc.rate_r, d.osc.line_freq));
        match freq_exchange_field(&dlg.periods, &dlg.harmonics, n, r, f) {
            Ok((p, h)) => {
                self.freq_percent = dlg.percent;
                self.freq_use_rms = dlg.use_rms;
                self.freq_periods = p;
                self.freq_harmonics = h;
            }
            Err((field, m)) => {
                self.dlg.push_alert(m);
                dlg.fail = Some(field);
                self.diag.freq_dlg = Some(dlg);
            }
        }
    }
}

/// The legend strip (`0x438210` circle, `0x443b90` differential): below
/// the plot's bottom b (raised by 20m by the caller), a row centred by a
/// signed halving and never clamped (a wide row overflows both sides); per
/// item a Rectangle with a black pen of width m in the item's colour from
/// (x, b + 3) — 3 device px, not scaled in print — 10m square, the name at
/// (x + 16m, b) in (50, 50, 50) on a transparent background, then x +=
/// text width + 22m (L9.82).
fn legend(p: &egui::Painter, ui: &egui::Ui, rect: Rect, loci: &[Locus]) {
    let gals: Vec<_> = loci.iter().map(|l| (ui.painter().layout_no_wrap(l.name.clone(), font(), TEXT), l.color)).collect();
    let total: f32 = gals.iter().map(|(g, _)| g.size().x + 22.0).sum();
    let mut x = rect.left() + ((rect.width() - total) / 2.0).trunc();
    let px = 1.0 / print_s().unwrap_or(1.0);
    for (g, col) in gals {
        let sq = Rect::from_min_size(Pos2::new(x, rect.bottom() + 3.0 * px), Vec2::splat(10.0));
        p.rect(sq, 0.0, col, stroke(1.0, Color32::BLACK));
        let wdt = g.size().x;
        p.galley(Pos2::new(x + 16.0, rect.bottom()), g, TEXT);
        x += wdt + 22.0;
    }
}

/// The circle diagram's value cells at cursor 1 (`0x43bbf0`): mode 1
/// `"%s%sj%s %s%s"` of Re and |Im| (`%.3f`, scaled by the prefix chosen on
/// |z|; both «0.0» when |z| is 0), mode 2 `"%.3f %s%s"` of |z| and
/// `"%.2fº"`; a non-finite part leaves the cells empty.
fn circle_cells(l: &Locus, mode: u8) -> [String; 2] {
    let [re, im] = l.c1;
    if !(re.is_finite() && im.is_finite()) {
        return Default::default();
    }
    // A differential pair (`0x446460`): Iдиф then Iторм, each `"%.3f %s%s"`
    // in its own channel's prefix and unit, an exact 0 as «0.0».
    if let Some([ud, ut]) = &l.pair_units {
        let cell = |v: f64, u: &fvb_core::units::Shown| {
            let (s, unit) = fvb_core::views::si_scaled(v, u);
            if v == 0.0 {
                // VM: «0.0» alone, no unit.
                "0.0".to_string()
            } else {
                format!("{s:.3} {unit}")
            }
        };
        return [cell(im, ud), cell(re, ut)];
    }
    let z = Complex64::new(re, im);
    let (scaled, unit) = fvb_core::views::si_scaled(z.norm(), &l.unit);
    let k = if z.norm() == 0.0 { 1.0 } else { scaled / z.norm() };
    if mode == 1 {
        let sign = if im < 0.0 { "-" } else { "+" };
        let (a, b) = if z.norm() == 0.0 { ("0.0".to_string(), "0.0".to_string()) } else { (format!("{:.3}", re * k), format!("{:.3}", (im * k).abs())) };
        [format!("{a}{sign}j{b} {unit}"), String::new()]
    } else {
        [format!("{:.3} {unit}", z.norm() * k), format!("{:.2}º", im.atan2(re).to_degrees())]
    }
}

/// The circle diagram's readout table (`0x43bbf0`), in units of the
/// print multiplier m: no header; the name column the widest name + 30 +
/// ⌊4cy/5⌋, two value columns of the width of "-0.0000000" + 24 (both
/// reserved in mode 1 too); centred when it fits, else 6 from the left; top
/// at the plot's bottom; rows ⌊3cy/2⌋ with the swatch a Rectangle (black
/// pen, the colour's fill) from (⌊cy/3⌋, ⌊cy/3⌋) to (⌊cy/3⌋ + cy − 1,
/// ⌊3cy/2⌋ − ⌊cy/3⌋), the name at ⌊cy/2⌋ + cy, values 6 into their column,
/// text ⌊cy/4⌋ down; rules at the top, under every row, at both edges,
/// after the name column and (mode 2) between the values, pen (150, 150,
/// 150) 1 px on screen, (100, 100, 100) ⌊m/2⌋ in print. No rows: nothing.
fn circle_table(p: &egui::Painter, ui: &egui::Ui, rect: Rect, cy: f32, loci: &[Locus], mode: u8) {
    if loci.is_empty() {
        return;
    }
    let width = |t: &str| ui.painter().layout_no_wrap(t.to_string(), font(), TEXT).size().x;
    let names = loci.iter().map(|l| width(&l.name)).fold(0.0, f32::max);
    let name_col = names + 30.0 + (4.0 * cy / 5.0).floor();
    let val_col = width("-0.0000000") + 24.0;
    let total = name_col + 2.0 * val_col;
    let left = if total < rect.width() { rect.left() + ((rect.width() - total) / 2.0).floor() } else { rect.left() + 6.0 };
    let top = rect.bottom();
    let row_h = (1.5 * cy).floor();
    let rule = match print_s() {
        Some(s) => stroke((s / 2.0).floor() / s, Color32::from_gray(100)),
        None => stroke(1.0, Color32::from_gray(150)),
    };
    let third = (cy / 3.0).floor();
    let px = 1.0 / print_s().unwrap_or(1.0);
    for (ri, l) in loci.iter().enumerate() {
        let y = top + row_h * ri as f32;
        let sw = Rect::from_min_max(Pos2::new(left + third, y + third), Pos2::new(left + third + cy - 1.0, y + row_h - third));
        p.rect(sw, 0.0, l.color, stroke(px, Color32::BLACK));
        let ty = y + (cy / 4.0).floor();
        p.text(Pos2::new(left + (cy / 2.0).floor() + cy, ty), egui::Align2::LEFT_TOP, &l.name, font(), TEXT);
        for (c, t) in circle_cells(l, mode).iter().enumerate() {
            p.text(Pos2::new(left + name_col + 6.0 + c as f32 * val_col, ty), egui::Align2::LEFT_TOP, t, font(), TEXT);
        }
    }
    let bottom = top + row_h * loci.len() as f32;
    for ri in 0..=loci.len() {
        let y = top + row_h * ri as f32;
        p.px_segment([Pos2::new(left, y), Pos2::new(left + total, y)], rule);
    }
    let mut xs = vec![left, left + name_col, left + total];
    if mode == 2 {
        xs.push(left + name_col + val_col);
    }
    for x in xs {
        p.px_segment([Pos2::new(x, top), Pos2::new(x, bottom)], rule);
    }
}

/// The readout table: the circle diagram's (`0x43bbf0`) and the
/// differential diagram's (`0x446460`, rows «Iдиф Iторм», a value column
/// each) share the layout, [`circle_table`].
fn readout_table(p: &egui::Painter, ui: &egui::Ui, rect: Rect, text_h: f32, loci: &[Locus], mode: u8, diff: bool) {
    // The differential one is on / off with both value columns ruled.
    circle_table(p, ui, rect, text_h, loci, if diff { 2 } else { mode });
}

/// A plane scroll bar (range 0..10000, page `round(10000/z)`): a click on
/// the track steps ±100, the thumb drags. Returns a new position.
fn scrollbar(ui: &mut egui::Ui, track: Rect, horizontal: bool, pos: i32, page: i32, m: i32, id: impl std::hash::Hash) -> Option<i32> {
    let resp = ui.interact(track, egui::Id::new(id), egui::Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(track, 0.0, Color32::from_gray(235));
    let len = if horizontal { track.width() } else { track.height() };
    let thumb_len = (len * page as f32 / 10000.0).max(12.0).min(len);
    let free = (len - thumb_len).max(1.0);
    let pos = pos.clamp(0, m.max(0));
    let start = free * pos as f32 / m.max(1) as f32;
    let thumb = if horizontal {
        Rect::from_min_size(Pos2::new(track.left() + start, track.top() + 2.0), Vec2::new(thumb_len, track.height() - 4.0))
    } else {
        Rect::from_min_size(Pos2::new(track.left() + 2.0, track.top() + start), Vec2::new(track.width() - 4.0, thumb_len))
    };
    p.rect_filled(thumb, 2.0, Color32::from_gray(190));
    let at = |q: Pos2| if horizontal { q.x - track.left() } else { q.y - track.top() };
    if resp.dragged() {
        let q = resp.interact_pointer_pos()?;
        let v = ((at(q) - thumb_len / 2.0) / free * m as f32).round() as i32;
        return Some(v.clamp(0, m.max(0)));
    }
    if resp.clicked() {
        let q = resp.interact_pointer_pos()?;
        let forward = at(q) > start + thumb_len;
        if forward || at(q) < start {
            return Some(dg::scroll_step(pos, forward, m));
        }
    }
    None
}

fn header_texts([u, i]: [f64; 2]) -> (String, String) {
    (format!("{} V", fvb_core::views::vector_header_value(u)), format!("{} A", fvb_core::views::vector_header_value(i)))
}

/// The header box's x offset from the window's left:
/// `max(0, W − L − Wv − m − 10)`.
fn vector_header_off(painter: &egui::Painter, full: Rect, scales: [f64; 2]) -> f32 {
    let s = print_s().unwrap_or(1.0);
    let measure = |t: &str| painter.layout_no_wrap(t.to_string(), font(), Color32::BLACK).size().x;
    let (ut, it) = header_texts(scales);
    let m = 6.0;
    let l = measure("U max") + 2.0 * m;
    let w = measure("0.0000").max(measure(&ut)).max(measure(&it)) + 2.0 * m;
    (full.width() - l - w - m - 10.0 / s).max(0.0)
}

/// The vector diagram's header (`0x4910c3`–`0x4917fc`): a box at the top
/// right, (254,250,220) under the window pen, split into two rows and two
/// columns — «U max» / «I max» and the group scales `"%.1f V"` / `"%.1f A"`
/// (`vector_header_value`). Both rows are always drawn. Device offsets 10
/// and m = 6·s, h = ⌊3·cy/2⌋ with cy the height of "Ay"; the label column
/// fits "U max" only, the value column the wider of "0.0000" and both texts.
fn vector_header(painter: &egui::Painter, full: Rect, [u, i]: [f64; 2]) {
    let s = print_s().unwrap_or(1.0);
    let ink = Color32::from_rgb(20, 20, 20);
    let measure = |t: &str| painter.layout_no_wrap(t.to_string(), font(), ink).size();
    let (ut, it) = header_texts([u, i]);
    let (m, ten) = (6.0, 10.0 / s);
    let cy = measure("Ay").y;
    let h = (3.0 * cy * s / 2.0).floor() / s;
    let l = measure("U max").x + 2.0 * m;
    let w = measure("0.0000").x.max(measure(&ut).x).max(measure(&it).x) + 2.0 * m;
    let off = vector_header_off(painter, full, [u, i]);
    let x0 = full.left() + off;
    let top = full.top() + ten;
    let pen = match print_s() {
        Some(s) => dev_pen(s / 2.0, Color32::from_rgb(100, 100, 100)),
        None => stroke(1.0, Color32::from_rgb(150, 150, 150)),
    };
    let rect = Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x0 + l + w + m, top + 2.0 * h));
    painter.rect_filled(rect, 0.0, Color32::from_rgb(254, 250, 220));
    painter.px_rect_stroke(rect, 0.0, pen);
    painter.px_vline(x0 + l, top..=top + 2.0 * h, pen);
    painter.px_hline(rect.x_range(), top + h, pen);
    for (row, (label, value)) in [("U max", ut), ("I max", it)].into_iter().enumerate() {
        let y = top + cy / 4.0 + row as f32 * h;
        painter.text(Pos2::new(x0 + m, y), egui::Align2::LEFT_TOP, label, font(), ink);
        painter.text(Pos2::new(x0 + l + m, y), egui::Align2::LEFT_TOP, value, font(), ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Command, Pane};

    /// The zoom gate scales over the client less 8 / 16 px (`0x4379e0`),
    /// not over the painted plane.
    #[test]
    fn the_zoom_gate_measures_its_own_rectangle() {
        let g = PlaneGeom { w: 100, h: 100, pan: (0, 0), max: [10.0, 10.0], char_max: 10.0, gate: (400, 400) };
        assert_eq!(g.ppu_at(1.0), dg::plane_scale(400, 400, 1.0, [10.0, 10.0]).ppu[0]);
        assert_ne!(g.ppu_at(1.0), dg::plane_scale(100, 100, 1.0, [10.0, 10.0]).ppu[0]);
    }

    struct H {
        app: App,
        ctx: egui::Context,
        t: f64,
    }

    const SCREEN: egui::Vec2 = egui::vec2(1320.0, 860.0);

    impl H {
        fn new(pane: Pane) -> H {
            let mut app = App::new();
            app.dispatch(Command::Demo);
            app.dispatch(Command::ActivatePane(pane));
            let mut h = H { app, ctx: egui::Context::default(), t: 0.0 };
            for _ in 0..3 {
                h.frame(vec![], Default::default());
            }
            h
        }

        fn frame(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
            self.t += 0.05;
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
                time: Some(self.t),
                events,
                modifiers,
                ..Default::default()
            };
            let app = &mut self.app;
            let _ = self.ctx.run(raw, |ctx| {
                app.diagram_panes(ctx);
                app.render_tab(ctx);
            });
        }

        /// A point inside the plane / vector window of the right column
        /// (the pane is 500 px wide on the right; the grid takes its top).
        fn plane_point(&self) -> Pos2 {
            Pos2::new(SCREEN.x - 260.0, 380.0)
        }

        fn wheel(&mut self, notches: f32, m: egui::Modifiers) {
            let at = self.plane_point();
            let ev = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: Vec2::new(0.0, notches), modifiers: m };
            self.frame(vec![egui::Event::PointerMoved(at), ev], m);
            self.frame(vec![], Default::default());
        }

        fn right_click(&mut self, at: Pos2) {
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
            self.frame(vec![egui::Event::PointerMoved(at), ev(true)], Default::default());
            self.frame(vec![ev(false)], Default::default());
            self.frame(vec![], Default::default());
        }
    }

    /// Follow-up of #7 (VM): the vector table's value cells are empty
    /// until В1 has been placed; placed and hidden again, they stay.
    #[test]
    fn the_vector_table_values_wait_for_the_first_vizir() {
        let mut app = App::new();
        app.dispatch(crate::app::Command::Demo);
        app.vec_show_values = true;
        app.active_doc_mut().unwrap().chans[3].on_vector = true;
        let texts = |app: &mut App| -> Vec<String> {
            let ctx = egui::Context::default();
            let out = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 500.0))), ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.vector_ui(ui));
            });
            out.shapes.iter().filter_map(|c| if let egui::Shape::Text(t) = &c.shape { Some(t.galley.text().to_string()) } else { None }).collect()
        };
        // The row's angle cell, «%8.2fº» (the axis labels have no blanks).
        let value = |t: &Vec<String>| t.iter().any(|s| s.starts_with(' ') && s.ends_with('º'));
        let t = texts(&mut app);
        assert!(t.iter().any(|s| s == "Ia") && !value(&t), "{t:?}");
        let d = app.active_doc_mut().unwrap();
        // 32813 places В1; moved onto sample 0 and hidden again by 32813,
        // the values stay.
        d.toggle_vizir(0, 500.0, 0.0);
        assert!(d.v_shown[0] && d.v1_placed);
        let t = texts(&mut app);
        assert!(value(&t), "{t:?}");
        let d = app.active_doc_mut().unwrap();
        d.main_cursor = 0;
        d.toggle_vizir(0, 500.0, 0.0);
        assert!(!d.v_shown[0]);
        assert!(value(&texts(&mut app)), "hidden again: the values stay");
    }

    #[test]
    fn the_vector_grid_lies_on_gdis_pixels() {
        // VM: the 227 px ring about pixel (2297, 802) spans 2070…2523 and
        // 575…1028; a spoke starts on the centre and leaves out its end.
        let ring = super::circle_pixels((2297, 802), 227);
        let xs = ring.iter().map(|p| p.0);
        let ys = ring.iter().map(|p| p.1);
        assert_eq!((xs.clone().min(), xs.max(), ys.clone().min(), ys.max()), (Some(2070), Some(2523), Some(575), Some(1028)));
        assert_eq!(ring[0], (2523, 802), "counter-clockwise from the right");
        let up = super::line_pixels((2297, 802), (2297, 575));
        assert_eq!((up.len(), up[0], up[up.len() - 1]), (227, (2297, 802), (2297, 576)));
    }

    #[test]
    fn a_circles_box_leaves_out_its_right_and_bottom_edges() {
        // VM: «БМРЗ - Защита генераторов (с 2025 г.в.)» on an 8-cell plane
        // reads X ±64, an 8 Ом cell: its largest circle's `Ellipse` box
        // reaches 65 on the right, which GDI leaves out of the path.
        let mut h = H::new(Pane::Circle);
        let b = h.app.library.blocks.iter().position(|b| b.name == "БМРЗ - Защита генераторов (с 2025 г.в.)").expect("the block");
        h.app.block_idx = b;
        h.app.ensure_block_state();
        h.frame(vec![], Default::default());
        let g = h.app.diag.geom[0].as_ref().expect("the circle plane painted");
        assert_eq!(g.max[0], 64.0);
    }

    #[test]
    fn the_shipped_ohm_characteristics_take_the_impedance_scale() {
        // VM: «БМРЗ - Защита генераторов» on the circle plane, no loci: the
        // edges read ±66 Ом and 0 Вт, the circles drawn (`0x4387d0`
        // upper-cases «Ohm» before its exact compare with «OHM»).
        let mut h = H::new(Pane::Circle);
        let b = h.app.library.blocks.iter().position(|b| b.name == "БМРЗ - Защита генераторов").expect("the block");
        h.app.block_idx = b;
        h.app.ensure_block_state();
        h.frame(vec![], Default::default());
        let g = h.app.diag.geom[0].as_ref().expect("the circle plane painted");
        assert!(g.max[0] > 0.0 && g.char_max == g.max[0], "the Ω extent: {:?}", g.max);
        assert_eq!(g.max[1], 0.0, "nothing on the power scale");
    }

    #[test]
    fn circle_wheel_pans_and_zooms_without_egui_plot() {
        let mut h = H::new(Pane::Circle);
        assert!(h.app.diag.geom[0].is_some(), "the circle plane painted");
        h.wheel(1.0, egui::Modifiers::NONE);
        assert_eq!(h.app.diag.circle.oy, 2, "plain wheel up: oy + 2");
        h.wheel(-1.0, egui::Modifiers::SHIFT);
        assert_eq!(h.app.diag.circle.ox, -2, "Shift+wheel down: ox − 2");
        // No block and no Ω locus: the Ω scale is infinite, no zoom
        // (`0x438b50`).
        h.wheel(1.0, egui::Modifiers::CTRL);
        assert_eq!(h.app.diag.circle.z, 1.0, "no Ω locus: Ctrl+wheel cannot zoom");
        let d = h.app.active_doc_mut().unwrap();
        let n = d.osc.sample_count();
        // Large enough that the characteristics fit the zoomed Ω scale.
        let wave: Vec<f64> = (0..n).map(|k| 1e6 * (k as f64 * 0.3).sin()).collect();
        let mut z = crate::app::Computed::new("Z".into(), "Zf: ()".into(), wave, Color32::RED);
        (z.unit, z.on_circle) = ("Ohm".into(), true);
        d.computed.push(z);
        h.frame(vec![], Default::default());
        h.wheel(1.0, egui::Modifiers::CTRL);
        assert_eq!(h.app.diag.circle.z, 1.5, "Ctrl+wheel up: ×1.5");
        h.wheel(-1.0, egui::Modifiers::CTRL);
        assert_eq!((h.app.diag.circle.z, h.app.diag.circle.ox, h.app.diag.circle.oy), (1.0, 0, 0), "back to z = 1 resets the pans");
    }

    #[test]
    fn a_wheel_over_the_pane_toolbar_reaches_the_plane() {
        let mut h = H::new(Pane::Circle);
        let at = h.app.diag.toolbar[0].expect("the circle toolbar drawn").center();
        let ev = egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: Vec2::new(0.0, 1.0), modifiers: Default::default() };
        h.frame(vec![egui::Event::PointerMoved(at), ev], Default::default());
        h.frame(vec![], Default::default());
        assert_eq!(h.app.diag.circle.oy, 2, "posted on to the plane (`0x436220`)");
    }

    #[test]
    fn pair_dialog_opens_on_the_first_channel() {
        let mut h = H::new(Pane::Circle);
        h.app.diag_cmd(Window::Circle, DiagCmd::CreatePairs, Pos2::ZERO);
        let first = doc_chans(h.app.active_doc().unwrap()).into_iter().next().map(|c| c.name);
        let dlg = h.app.diag.pair_dlg.as_ref().unwrap();
        assert!(first.is_some());
        assert_eq!((&dlg.a, &dlg.b), (&first, &first));
    }

    #[test]
    fn differential_starts_in_quadrant_one() {
        let h = H::new(Pane::Differential);
        let v = h.app.diag.diff;
        assert!(!v.initial_pending);
        assert!(v.oy > 0 && v.ox < 0, "origin pushed lower-left: {v:?}");
    }

    #[test]
    fn right_click_opens_the_original_menus() {
        let mut h = H::new(Pane::Circle);
        let at = h.plane_point();
        h.right_click(at);
        let m = h.app.diag.menu.expect("MENU 387 open");
        assert_eq!((m.win, m.kind), (Window::Circle, MenuKind::Full));
        let e = h.app.menu_entries(&m);
        assert!(matches!(&e[0], Entry::Sub { label, .. } if label == "Каналы"));
        // Escape closes it.
        h.frame(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }], Default::default());
        assert!(h.app.diag.menu.is_none());
        let mut h = H::new(Pane::Vector);
        h.right_click(at);
        let m = h.app.diag.menu.expect("MENU 355 open");
        assert_eq!(m.win, Window::Vector);
    }

    #[test]
    fn menu_commands_toggle_the_options() {
        let mut h = H::new(Pane::Circle);
        let at = Pos2::ZERO;
        h.app.diag_cmd(Window::Circle, DiagCmd::Readout(2), at);
        assert_eq!(h.app.circle_readout, 2);
        h.app.diag_cmd(Window::Circle, DiagCmd::Readout(2), at);
        assert_eq!(h.app.circle_readout, 0, "the checked readout turns off");
        let reset = h.app.show_reset;
        h.app.diag_cmd(Window::Circle, DiagCmd::ShowReset, at);
        assert_eq!(h.app.show_reset, !reset);
        // Circle «Показать все»: only Ohm / VA channels (the demo has A / V).
        h.app.diag_cmd(Window::Circle, DiagCmd::ShowAll(true), at);
        assert!(h.app.active_doc().unwrap().chans.iter().all(|c| !c.on_circle));
        h.app.diag_cmd(Window::Vector, DiagCmd::ShowAll(false), at);
        assert!(h.app.active_doc().unwrap().chans.iter().all(|c| !c.on_vector));
        h.app.diag_cmd(Window::Vector, DiagCmd::ToggleChan(0), at);
        assert!(h.app.active_doc().unwrap().chans[0].on_vector);
    }

    #[test]
    fn rx_pairs_draw_as_loci_and_extend_the_ohm_scale() {
        let mut h = H::new(Pane::Circle);
        let names: Vec<String> = h.app.active_doc().unwrap().osc.analog.iter().map(|c| c.id.clone()).collect();
        h.app.diag_cmd(Window::Circle, DiagCmd::CreatePairs, Pos2::ZERO);
        let dlg = h.app.diag.pair_dlg.as_mut().expect("dialog 554");
        assert!(!dlg.diff);
        dlg.pairs.push(ChanPair::new(names[0].clone(), names[1].clone()));
        let pairs = dlg.pairs.clone();
        h.app.diag.pair_dlg = None;
        h.app.active_doc_mut().unwrap().rx_pairs = pairs;
        let d = h.app.active_doc().unwrap();
        let loci = plane_loci(d, false, false);
        let l = loci.iter().find(|l| l.name == format!("{} {}", names[0], names[1])).expect("R X locus");
        assert!(!l.power && !l.pts.is_empty());
        for _ in 0..2 {
            h.frame(vec![], Default::default());
        }
        assert!(h.app.diag.geom[0].unwrap().max[0] > 0.0);
        // Disabled from the Каналы submenu: gone.
        h.app.diag_cmd(Window::Circle, DiagCmd::TogglePair(0), Pos2::ZERO);
        let label = format!("{} {}", names[0], names[1]);
        assert!(plane_loci(h.app.active_doc().unwrap(), false, false).iter().all(|l| l.name != label));
    }

    /// A formula channel is listed on the circle pane by its unit like any
    /// other (`0x43d220`): none → not listed (L9.83).
    /// The frequency diagram lists every analog-kind channel not pending a
    /// merge, formula channels included (`0x46f2e0`, L9.188); its flag
    /// round-trips through the formula channel.
    #[test]
    fn formula_channels_reach_the_frequency_diagram() {
        let ch = |pending: bool| DChan { name: "F".into(), unit: String::new(), shown: Default::default(), color: Color32::RED, analog: None, computed: Some(0), pending };
        assert!(listed(Window::Freq, &ch(false)));
        assert!(!listed(Window::Freq, &ch(true)), "pending a merge");
        let mut app = App::new();
        app.dispatch(crate::app::Command::Demo);
        let d = app.active_doc_mut().unwrap();
        d.computed.push(crate::app::Computed::new("F".into(), "Ua".into(), vec![0.0; d.osc.sample_count()], Color32::RED));
        let idx = d.chans.len();
        let chans = doc_chans(d);
        set_chan_flag(d, Window::Freq, &chans[idx], true);
        assert!(d.computed[0].on_freq);
        let chans = doc_chans(d);
        assert!(chan_flag(d, Window::Freq, &chans[idx]));
    }

    #[test]
    fn unitless_formula_channels_are_not_circle_channels() {
        let mk = |unit: &str| DChan { name: "Z".into(), unit: unit.into(), shown: Default::default(), color: Color32::RED, analog: None, computed: Some(0), pending: false };
        assert!(!listed(Window::Circle, &mk("")));
        assert!(listed(Window::Circle, &mk("Ohm")) && listed(Window::Circle, &mk("VA")));
    }

    /// The circle range (`0x43a9ed`–`0x43aa41`): N..=len, both bounds
    /// raised to N and capped at the sample count; points past the values
    /// or non-finite are the origin; R/X pairs get no cursor marks.
    #[test]
    fn circle_range_and_points_as_read() {
        let mut h = H::new(Pane::Circle);
        let d = h.app.active_doc_mut().unwrap();
        let (len, n) = (d.osc.sample_count(), d.osc.samples_per_cycle());
        assert_eq!(circle_range(d, false), n..=len);
        (d.main_cursor, d.aux_cursor) = (len + 50, 1);
        assert_eq!(circle_range(d, true), n..=len, "both bounds ≥ N, ≤ len");
        // The differential's scale range is not raised to N (`0x44425d`).
        assert_eq!(diff_scale_range(d, true), 1..=len);
        (d.main_cursor, d.aux_cursor) = (3, 1);
        assert_eq!((diff_scale_range(d, true), circle_range(d, true)), (1..=3, n..=n));
        assert_eq!(diff_scale_range(d, false), n..=len);
        assert_eq!(finite_or_origin([f64::NAN, 1.0]), [0.0, 0.0]);
        let names: Vec<String> = d.osc.analog.iter().take(2).map(|c| c.id.clone()).collect();
        d.rx_pairs.push(crate::diagram::ChanPair::new(names[0].clone(), names[1].clone()));
        let loci = plane_loci(d, false, false);
        let pair = loci.last().unwrap();
        assert!(!pair.markers && !pair.power);
        assert_eq!(pair.unit, d.osc.analog[0].shown_unit(), "the R channel's unit");
        assert_eq!(pair.pts.last(), Some(&[0.0, 0.0]));
    }

    /// A «P» record shown «Вторичные» is drawn in secondary values on the
    /// diagrams too: the phasor cache and `0x426020` carry k (L9.69) — and the
    /// original reads every sample through it before it sums (#60), so the
    /// phasor, the RMS, the first harmonic and the spectrum are sums of
    /// `x·k`, not `k` times a sum of `x` (the two differ in the last bits).
    #[test]
    fn diagrams_carry_the_display_factor() {
        use fvb_core::signal::{phasor_at, rms_at, spectrum};
        let mut h = H::new(Pane::Circle);
        let d = h.app.active_doc_mut().unwrap();
        (d.osc.analog[0].primary, d.osc.analog[0].secondary, d.osc.analog[0].is_primary) = (1234.0, 1.0, true);
        // Synthetic values whose per-sample and after-the-sum orders give different bits.
        let mut z: u64 = 0x2545_f491_4f6c_dd1d;
        for v in d.osc.analog[0].samples.iter_mut() {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            *v = 1000.0 + (z % 1_000_000) as f64 * 0.37;
        }
        d.chans[0].on_circle = true;
        d.recompute(false);
        let n = d.osc.samples_per_cycle().max(2);
        let k = d.osc.sample_count() - 1;
        assert_eq!(d.shown_k(0), 1.0 / 1234.0);
        let kf = 1.0 / 1234.0;
        let scaled: Vec<f64> = d.osc.analog[0].samples.iter().map(|v| v * kf).collect();
        let (per_sample, after) = (phasor_at(&scaled, k, n), phasor_at(&d.osc.analog[0].samples, k, n) * kf);
        assert_ne!(per_sample, after, "the synthetic record tells the two orders apart");
        assert_eq!(d.analysis.phasors[0][k], per_sample, "the phasor cache sums the scaled samples");
        assert_eq!(d.rms_at(0, k).to_bits(), rms_at(&scaled, k, n).to_bits());
        assert_eq!(d.stat_rms(0, k).to_bits(), rms_at(&scaled, k, n).to_bits());
        assert_eq!(d.harmonic_at(0, k).to_bits(), per_sample.norm().to_bits());
        assert_eq!(spectrum(d.shown_samples(0), k, n, 2, 6, 1.0), spectrum(&scaled, k, n, 2, 6, 1.0));
        let id = d.osc.analog[0].id.clone();
        // The point of sample k: the range's end is one past it (0x43a880).
        let at = |d: &Doc| {
            let pts = plane_loci(d, false, false).into_iter().find(|l| l.name == id).unwrap().pts;
            assert_eq!(pts.last(), Some(&[0.0, 0.0]), "the locus ends at the origin");
            pts[pts.len() - 2]
        };
        assert_eq!(at(d), [per_sample.re, per_sample.im]);
        d.chans[0].primary = true;
        d.recompute(true);
        let raw = phasor_at(&d.osc.analog[0].samples, k, n);
        assert_eq!(at(d), [raw.re, raw.im], "shown primary: the stored values");
        d.chans[0].adc_counts = true;
        assert_eq!(d.shown_k(0), 1.0, "ADC codes are taken as is");
    }

    /// With «Удалить постоянную» the accessor `0x426020` returns `x·k − dc·k` (negated by «Инвертировать»),
    /// not `(x − dc)·k`: the sums read the former.
    #[test]
    fn remove_dc_follows_the_accessors_order() {
        use fvb_core::signal::{phasor_at, rms_at};
        let mut h = H::new(Pane::Circle);
        let d = h.app.active_doc_mut().unwrap();
        (d.osc.analog[0].primary, d.osc.analog[0].secondary, d.osc.analog[0].is_primary) = (1234.0, 1.0, true);
        let mut z: u64 = 0x9e37_79b9_7f4a_7c15;
        for v in d.osc.analog[0].samples.iter_mut() {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            *v = 700.0 + (z % 900_000) as f64 * 0.013;
        }
        d.chans[0].remove_dc = true;
        d.chans[0].invert = true;
        d.recompute(false);
        let n = d.osc.samples_per_cycle().max(2);
        let k = 1.0 / 1234.0;
        let dc = crate::app::whole_period_mean(&d.osc.analog[0].samples, n);
        let want: Vec<f64> = d.osc.analog[0].samples.iter().map(|x| -(x * k - dc * k)).collect();
        let other: Vec<f64> = d.osc.analog[0].samples.iter().map(|x| -((x - dc) * k)).collect();
        assert_ne!(want, other, "the synthetic record tells the two orders apart");
        let i = d.osc.sample_count() - 1;
        assert_eq!(d.shown_samples(0), &want[..]);
        assert_eq!(d.analysis.phasors[0][i], phasor_at(&want, i, n));
        assert_eq!(d.stat_rms(0, i).to_bits(), rms_at(&want, i, n).to_bits());
    }

    #[test]
    fn frequency_settings_dialog_edits_a_copy_until_ok() {
        let mut h = H::new(Pane::Vector);
        assert!(h.app.freq_percent, "percent is the default");
        h.app.diag_cmd(Window::Freq, DiagCmd::FreqSettings, Pos2::ZERO);
        h.app.diag.freq_dlg.as_mut().unwrap().percent = false;
        h.frame(vec![], Default::default());
        assert!(h.app.diag.freq_dlg.is_some() && h.app.freq_percent, "not applied before OK");
        let dlg = h.app.diag.freq_dlg.take().unwrap();
        h.app.apply_freq_dialog(FreqDialog { periods: "0".into(), ..dlg });
        let o = &h.app.active_doc().unwrap().osc;
        let max = o.sample_count() / (o.rate_r as usize / o.line_freq as usize);
        assert_eq!(h.app.dlg.alerts.front().map(|a| a.0.as_str()), Some(format!("Введите целое число от 1 до {max}.").as_str()), "the record's whole periods");
        assert!(h.app.diag.freq_dlg.is_some() && h.app.freq_percent, "kept, nothing applied");
        let dlg = h.app.diag.freq_dlg.take().unwrap();
        h.app.apply_freq_dialog(FreqDialog { periods: "2".into(), harmonics: "20".into(), ..dlg });
        assert!(!h.app.freq_percent);
        assert_eq!((h.app.freq_periods, h.app.freq_harmonics), (2, 20));
        // Opened in absolute mode (`0x46ee20`): the two radios greyed.
        h.app.diag_cmd(Window::Freq, DiagCmd::FreqSettings, Pos2::ZERO);
        assert!(!h.app.diag.freq_dlg.as_ref().unwrap().radios_on);
    }

    /// `0x46ee70`'s checks: MFC's int parse, then the two ranges.
    #[test]
    fn frequency_dialog_exchange_as_read() {
        assert_eq!(freq_dialog_exchange(" 3 ", "+30", 1000, 1000.0, 50.0), Ok((3, 30)));
        assert_eq!(freq_dialog_exchange("2.5", "1", 1000, 1000.0, 50.0), Ok((2, 1)), "scanf %d stops at the point");
        assert_eq!(freq_dialog_exchange(".5", "1", 1000, 1000.0, 50.0), Err("Введите целое число.".into()));
        assert_eq!(freq_dialog_exchange("51", "1", 1000, 1000.0, 50.0), Err("Введите целое число от 1 до 50.".into()));
        assert_eq!(freq_dialog_exchange("2", "21", 1000, 1000.0, 50.0), Err("Введите целое число от 1 до 20.".into()), "trunc(10) × 2 periods");
        assert_eq!(freq_dialog_exchange("x", "x", 1000, 1000.0, 50.0), Err("Введите целое число.".into()), "the first failure");
    }

    /// Records whose units carry SI prefixes and Cyrillic letters («кВ»,
    /// «кА») — as the user's: units are normalised on load (`0x422b10`), so
    /// these channels are listed on the vector diagram, «Скрыть все» can be
    /// undone with «Показать все» or a tick, the menu command 32850 and the
    /// «Каналы» column are enabled; a Latin «B» stays ineligible.
    #[test]
    fn prefixed_cyrillic_units_reach_the_vector_diagram() {
        use crate::app::{ChanKey, Doc};
        use crate::chansel::ChanFlag;
        let cfg = include_str!("../../../assets/demo/demo_fault.cfg")
            .replacen(",,V,", ",,кВ,", 2)
            .replacen(",,V,", ",,B,", 1)
            .replace(",,A,", ",,кА,");
        let osc = fvb_core::comtrade::parse(&cfg, include_bytes!("../../../assets/demo/demo_fault.dat")).unwrap();
        assert_eq!((osc.analog[0].unit.as_str(), osc.analog[0].unit_mult), ("V", 1000.0));
        assert_eq!(osc.analog[3].raw_unit, "кА");
        assert_eq!(osc.analog[2].unit, "B", "a Latin B is no volt");
        let mut app = App::new();
        let mut doc = Doc::new(osc, "units".into());
        for c in &mut doc.chans {
            c.on_vector = true;
        }
        app.add_doc(doc);
        app.refresh_blocks();
        let names = |app: &App| -> Vec<String> {
            app.menu_chans(Window::Vector).into_iter().map(|c| c.name).collect()
        };
        assert_eq!(names(&app), ["Ua", "Ub", "Ia", "Ib", "Ic"]);
        let on = |app: &App| app.active_doc().unwrap().chans.iter().map(|c| c.on_vector).collect::<Vec<_>>();
        assert_eq!(on(&app), [true, true, false, true, true, true], "the ineligible channel is cleared on attach");
        let at = Pos2::ZERO;
        app.diag_cmd(Window::Vector, DiagCmd::ShowAll(false), at);
        assert!(on(&app).iter().all(|v| !v));
        assert!(app.active_doc().unwrap().set.modified);
        app.diag_cmd(Window::Vector, DiagCmd::ShowAll(true), at);
        assert_eq!(on(&app), [true, true, false, true, true, true], "«Показать все» brings them back");
        app.diag_cmd(Window::Vector, DiagCmd::ToggleChan(3), at);
        app.diag_cmd(Window::Vector, DiagCmd::ToggleChan(3), at);
        assert!(on(&app)[3], "a tick in the list brings one back");
        // 32850 on a «кА» channel, the «Каналы» vector column.
        let d = app.active_doc_mut().unwrap();
        d.selection = [ChanKey::Analog(3)].into_iter().collect();
        assert!(d.flag_state(ChanFlag::OnVector).enabled);
        assert!(!d.flag_state(ChanFlag::OnCircle).enabled, "no power or impedance unit");
        assert!(crate::chandlg::cell(d, ChanKey::Analog(3), 3).is_some());
        assert!(crate::chandlg::cell(d, ChanKey::Analog(2), 3).is_none());
        // Values are in base units: kV scaled by 1000.
        let raw = fvb_core::comtrade::parse(include_str!("../../../assets/demo/demo_fault.cfg"), include_bytes!("../../../assets/demo/demo_fault.dat")).unwrap();
        assert!((d.osc.analog[1].samples[1] - 1000.0 * raw.analog[1].samples[1]).abs() < 1e-6);
    }
}
