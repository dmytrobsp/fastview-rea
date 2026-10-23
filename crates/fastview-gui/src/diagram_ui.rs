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
    pub max: [f64; 2],
    pub char_max: f64,
}

impl PlaneGeom {
    fn ppu_at(&self, z: f64) -> Option<f64> {
        dg::plane_scale(self.w, self.h, z, self.max).ppu[0]
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
}

/// DIALOG 353 «Настройки частотной диаграммы».
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreqDialog {
    pub percent: bool,
    pub use_rms: bool,
    pub harmonics: usize,
    pub periods: usize,
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
    Stroke::new(w, c)
}

thread_local! {
    /// While a diagram page prints (`0x441fe0`): the integer scale
    /// `s = ⌊LOGPIXELSY(printer) / LOGPIXELSY(screen)⌋`; `None` on screen.
    static PRINT_S: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
    /// On a real print (`m_bPrinting`; not the preview's memory DC): the
    /// page origin relative to the diagram rect, in points.
    static PRINT_PAGE: std::cell::Cell<Option<Vec2>> = const { std::cell::Cell::new(None) };
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

/// A print pen `dev` printer pixels wide (truncated; 0 draws one pixel), in
/// points.
fn dev_pen(dev: f32, c: Color32) -> Stroke {
    let s = print_s().unwrap_or(1.0);
    Stroke::new(dev.floor().max(1.0) / s, c)
}

fn rgb((r, g, b): (u8, u8, u8)) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Tahoma 13 on screen; printing passes Arial `⌊52·s/5⌋` (`0x441fe0`),
/// 0.8 of the screen size.
fn font() -> FontId {
    FontId::proportional(if print_s().is_some() { 12.0 * 0.8 } else { 12.0 })
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
    color: Color32,
    analog: Option<usize>,
    computed: Option<usize>,
}

fn doc_chans(d: &Doc) -> Vec<DChan> {
    let mut v: Vec<DChan> = d
        .osc
        .analog
        .iter()
        .enumerate()
        // The canonical unit, none in ADC mode (`0x414c30`).
        .map(|(i, c)| DChan { name: c.id.clone(), unit: if d.chans[i].adc_counts { String::new() } else { c.unit.clone() }, color: d.chans[i].color, analog: Some(i), computed: None })
        .collect();
    v.extend(d.computed.iter().enumerate().map(|(j, c)| DChan {
        name: c.name.clone(),
        unit: c.unit.trim().to_string(),
        color: c.color,
        analog: None,
        computed: Some(j),
    }));
    v
}

/// Instantaneous value of a channel by name (`0x426020`).
/// A channel's value per sample.
type Series<'a> = Box<dyn Fn(usize) -> f64 + 'a>;

fn inst_by_name<'a>(d: &'a Doc, name: &str) -> Option<(Series<'a>, Color32)> {
    if let Some(i) = d.osc.analog.iter().position(|c| c.id == name) {
        return Some((Box::new(move |k| d.sample(i, k)), d.chans[i].color));
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
        _ => false,
    }
}

fn set_chan_flag(d: &mut Doc, w: Window, idx: usize, v: bool) {
    let na = d.chans.len();
    if idx < na {
        let c = &mut d.chans[idx];
        match w {
            Window::Vector => c.on_vector = v,
            Window::Freq => c.on_freq = v,
            _ => c.on_circle = v,
        }
    } else if let Some(c) = d.computed.get_mut(idx - na) {
        match w {
            Window::Vector => c.on_vector = v,
            Window::Circle => c.on_circle = v,
            _ => {}
        }
    }
}

/// Is a channel listed in a window's Каналы submenu.
fn listed(w: Window, c: &DChan) -> bool {
    match w {
        Window::Vector => dg::vector_unit_ok(&c.unit),
        // Our computed channels without a unit (the generator sets only
        // S0 = VA) are kept on the circle list.
        Window::Circle => dg::circle_pane_unit_ok(&c.unit) || (c.computed.is_some() && c.unit.is_empty()),
        Window::Freq => c.analog.is_some(),
        Window::Diff => false,
    }
}

/// One locus of a plane: name (legend), colour, power scale, the samples and
/// the values at cursor 1 / cursor 2.
struct Locus {
    name: String,
    unit: String,
    color: Color32,
    power: bool,
    pts: Vec<[f64; 2]>,
    c1: [f64; 2],
    c2: [f64; 2],
}

/// Sample range of the loci (`0x438b50`): N..end, or [min(c1,c2),
/// max(c1,c2)] with «Диапазон между визирами» (which replaces the N start),
/// both clamped to the record.
fn locus_range(d: &Doc, between: bool) -> std::ops::RangeInclusive<usize> {
    let len = d.osc.sample_count();
    let last = len.saturating_sub(1);
    let (mut a, mut b) = (d.osc.samples_per_cycle().max(1), last);
    if between {
        a = d.main_cursor.min(d.aux_cursor);
        b = d.main_cursor.max(d.aux_cursor);
    }
    a.min(last)..=b.min(last)
}

fn plane_loci(d: &Doc, diff: bool, between: bool) -> Vec<Locus> {
    let range = locus_range(d, between);
    let last = d.osc.sample_count().saturating_sub(1);
    let (k1, k2) = (d.main_cursor.min(last), d.aux_cursor.min(last));
    let n = d.osc.samples_per_cycle().max(2);
    let mut out = Vec::new();
    let mut push = |name: String, unit: String, color: Color32, power: bool, f: &dyn Fn(usize) -> [f64; 2]| {
        out.push(Locus { name, unit, color, power, pts: range.clone().map(f).collect(), c1: f(k1), c2: f(k2) });
    };
    if diff {
        // (Iторм, Iдиф) of every enabled pair, instantaneous.
        for p in d.dif_pairs.iter().filter(|p| p.on) {
            if let (Some((dif, col)), Some((tor, _))) = (inst_by_name(d, &p.a), inst_by_name(d, &p.b)) {
                push(p.legend_label(), String::new(), col, false, &|k| [tor(k), dif(k)]);
            }
        }
        return out;
    }
    for (i, ch) in d.osc.analog.iter().enumerate() {
        if d.chans[i].on_circle {
            let ph = &d.analysis.phasors[i];
            let unit = ch.unit.trim().to_string();
            let power = !dg::is_ohm(&unit);
            push(ch.id.clone(), unit, d.chans[i].color, power, &|k| ph.get(k).map_or([0.0, 0.0], |z| [z.re, z.im]));
        }
    }
    for c in d.computed.iter().filter(|c| c.on_circle) {
        let series = |k: usize| {
            let z = match &c.phasors {
                Some(p) => p.get(k).copied().unwrap_or_default(),
                None => fvb_core::freqcorr::phasor_at(fvb_core::freqcorr::of(&d.osc), fvb_core::freqcorr::OTHER, &c.samples, k, n),
            };
            [z.re, z.im]
        };
        // An empty unit (not modelled by our generator) is taken as Ohm.
        let unit = c.unit.trim().to_string();
        let power = !unit.is_empty() && !dg::is_ohm(&unit);
        push(c.name.clone(), unit, c.color, power, &series);
    }
    for p in d.rx_pairs.iter().filter(|p| p.on) {
        if let (Some((r, col)), Some((x, _))) = (inst_by_name(d, &p.a), inst_by_name(d, &p.b)) {
            push(p.legend_label(), "Ohm".into(), col, false, &|k| [r(k), x(k)]);
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
/// a 1 px frame (`FrameRgn`).
fn paint_region(p: &egui::Painter, rings: &[Vec<[f32; 2]>], fill: Option<Color32>, frame: Color32, clip: Rect) {
    if let Some(fc) = fill {
        let (y0, y1) = (clip.top().floor() as i32, clip.bottom().ceil() as i32);
        let mut mesh = egui::Mesh::default();
        for (y, x0, x1) in dg::scanline_spans(rings, y0, y1) {
            let (x0, x1) = (x0.max(clip.left()), x1.min(clip.right()));
            if x1 > x0 {
                mesh.add_colored_rect(Rect::from_min_max(Pos2::new(x0, y as f32), Pos2::new(x1, y as f32 + 1.0)), fc);
            }
        }
        p.add(egui::Shape::mesh(mesh));
    }
    for ring in rings {
        p.add(egui::Shape::closed_line(ring.iter().map(|q| Pos2::new(q[0], q[1])).collect(), stroke(1.0, frame)));
    }
}

fn dotted(p: &egui::Painter, a: Pos2, b: Pos2, color: Color32, width: f32) {
    p.extend(egui::Shape::dashed_line(&[a, b], stroke(width, color), 2.0, 2.0));
}

/// A tool button glyph of the pane toolbars (our own line art; the
/// original's toolbar bitmaps are not redistributed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
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

fn paint_tool(p: &egui::Painter, r: Rect, t: Tool, c: Color32) {
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

/// One toolbar button: (glyph, tooltip, command, checked state).
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
            Window::Vector => vec![
                Some((Tool::Channels, "Каналы", DiagCmd::ChannelsMenu, None)),
                None,
                Some((Tool::Angles, "Отсчет углов", DiagCmd::AngleMenu, None)),
                None,
                Some((Tool::Values, "Показывавать действующее значение и угол", DiagCmd::ShowValues, Some(self.vec_show_values))),
                Some((Tool::Direction, "Только направление", DiagCmd::DirectionOnly, Some(self.vec_direction_only))),
            ],
            Window::Circle | Window::Diff => {
                let (between, reset, readout) = self.plane_opts(diff);
                let mut v = vec![
                    Some((Tool::Channels, "Каналы", DiagCmd::ChannelsMenu, None)),
                    None,
                    Some((Tool::Range, "Диапазон между визирами", DiagCmd::BetweenCursors, Some(between))),
                    Some((Tool::Reset, "Характеристики возврата", DiagCmd::ShowReset, Some(reset))),
                    None,
                ];
                if diff {
                    v.push(Some((Tool::Values, "Показывать значения", DiagCmd::Readout(1), Some(readout != 0))));
                } else {
                    v.push(Some((Tool::ReIm, "Показывать вещественную и мнимую части", DiagCmd::Readout(1), Some(readout == 1))));
                    v.push(Some((Tool::Polar, "Показывавать действующее значение и угол", DiagCmd::Readout(2), Some(readout == 2))));
                }
                v.extend([
                    None,
                    Some((Tool::Reload, "Перезагрузить базы характеристик", DiagCmd::Reload, None)),
                    None,
                    Some((Tool::Plus, "Увеличить масштаб", DiagCmd::Zoom(true), None)),
                    Some((Tool::Minus, "Уменьшить масштаб", DiagCmd::Zoom(false), None)),
                    None,
                    Some((Tool::Up, "Сдвиг вверх", DiagCmd::Pan(false, -2), None)),
                    Some((Tool::Down, "Сдвиг вниз", DiagCmd::Pan(false, 2), None)),
                    None,
                    Some((Tool::Left, "Сдвиг влево", DiagCmd::Pan(true, -2), None)),
                    Some((Tool::Right, "Сдвиг вправо", DiagCmd::Pan(true, 2), None)),
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
        egui::SidePanel::right(egui::Id::new(("pane_toolbar", w as u8))).resizable(false).exact_width(26.0).show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            // Buttons that do not fit are cut off (a short toolbar), never
            // grow the pane past its slot.
            for b in &buttons {
                if ui.available_height() < 22.0 {
                    break;
                }
                let Some((tool, tip, cmd, checked)) = b else {
                    ui.add_space(3.0);
                    ui.separator();
                    continue;
                };
                // The vector toolbar is enabled with a document (CMDUI 0x48f440).
                let enabled = w != Window::Vector || doc;
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
                let vis = ui.visuals();
                if checked == &Some(true) {
                    ui.painter().rect(rect, 2.0, vis.selection.bg_fill.gamma_multiply(0.5), stroke(1.0, vis.selection.stroke.color));
                } else if resp.hovered() && enabled {
                    ui.painter().rect_filled(rect, 2.0, vis.widgets.hovered.weak_bg_fill);
                }
                let col = if enabled { vis.text_color() } else { vis.weak_text_color() };
                paint_tool(ui.painter(), rect, *tool, col);
                let resp = resp.on_hover_text(*tip);
                if resp.clicked() {
                    fire = Some(cmd.clone());
                }
            }
        });
        if let Some(c) = fire {
            let at = ui.ctx().pointer_latest_pos().unwrap_or_default();
            self.diag_cmd(w, c, at);
        }
    }

    fn open_diag_menu(&mut self, win: Window, kind: MenuKind, pos: Pos2) {
        self.diag.serial += 1;
        self.diag.menu = Some(OpenMenu { win, kind, pm: PopMenu { serial: self.diag.serial, pos, sub: None } });
    }

    fn menu_chans(&self, w: Window) -> Vec<MenuChan> {
        let Some(d) = self.active_doc() else { return Vec::new() };
        doc_chans(d)
            .iter()
            .enumerate()
            .filter(|(_, c)| w == Window::Circle || listed(w, c))
            .map(|(idx, c)| {
                // A computed channel without a unit is listed as an Ohm one
                // (see `listed`).
                let unit = if w == Window::Circle && c.computed.is_some() && c.unit.is_empty() { "Ohm".into() } else { c.unit.clone() };
                MenuChan { idx, name: c.name.clone(), unit, on: chan_flag(d, w, c) }
            })
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
                if let Some(d) = self.active_doc() {
                    let pairs = if diff { d.dif_pairs.clone() } else { d.rx_pairs.clone() };
                    self.diag.pair_dlg = Some(PairDialog { diff, pairs, a: None, b: None, sel: None });
                }
            }
            DiagCmd::ShowAll(on) => {
                let Some(d) = self.active_doc_mut() else { return };
                match w {
                    Window::Diff => d.dif_pairs.iter_mut().for_each(|p| p.on = on),
                    _ => {
                        // «Показать все» sets the flag on every listed channel
                        // (`0x437150`), «Скрыть все» clears it on all.
                        let chans = doc_chans(d);
                        for (idx, c) in chans.iter().enumerate() {
                            if !on || listed(w, c) {
                                set_chan_flag(d, w, idx, on);
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
                    set_chan_flag(d, w, idx, v);
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
                    let limit = dg::max_pan(if x_axis { g.w } else { g.h }, z);
                    self.plane_view(diff).pan(x_axis, delta, limit);
                }
            }
            DiagCmd::AngleRef(r) => self.vec_angle_ref = r,
            DiagCmd::ShowValues => self.vec_show_values = !self.vec_show_values,
            DiagCmd::DirectionOnly => self.vec_direction_only = !self.vec_direction_only,
            DiagCmd::FreqSettings => {
                self.diag.freq_dlg = Some(FreqDialog {
                    percent: self.freq_percent,
                    use_rms: self.freq_use_rms,
                    harmonics: self.freq_harmonics,
                    periods: self.freq_periods,
                })
            }
        }
    }

    /// 32864 «Перезагрузить базы характеристик» (`0x436080` → `0x40c7e0`):
    /// load the `*.fvb` bases again, keeping the selected block by name.
    pub fn reload_bases(&mut self) {
        let block = self.library.blocks.get(self.block_idx).map(|b| b.name.clone());
        let (lib, name) = crate::app::load_libraries();
        self.full_library = lib;
        self.source_name = name;
        self.diagrams = [None, None];
        self.library = self.full_library.for_dimension(self.diagram_dim);
        self.block_idx = block.and_then(|n| self.library.blocks.iter().position(|b| b.name == n)).unwrap_or(0);
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
        if resp.secondary_clicked() && !self.diag.menu_eat {
            if let Some(p) = resp.interact_pointer_pos() {
                self.open_diag_menu(w, MenuKind::Full, p);
            }
        }
    }

    // ---- circle / differential plane ----

    /// The circle (`Ohm`) or differential (`Dif`) plane below the grid.
    pub fn plane_ui(&mut self, ui: &mut egui::Ui) {
        let diff = self.current_dimension() == fvb_core::model::Dimension::Dif;
        let win = if diff { Window::Diff } else { Window::Circle };
        let gi = usize::from(diff);
        let full = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(full, egui::Sense::click());
        let (between, show_reset, readout) = self.plane_opts(diff);
        let view = *self.plane_view(diff);
        // Scroll bars take client area while zoomed (`0x437ca0`).
        let sb = dg::scroll_range(view.z);
        let client = if sb.is_some() { Rect::from_min_max(full.min, full.max - Vec2::splat(SCROLL_W)) } else { full };
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, Color32::WHITE);
        self.window_menu_click(&resp, win);
        if client.width() < 121.0 || client.height() < 121.0 {
            return;
        }
        // Characteristics: extent of every visible one (operate and reset).
        let b = self.block_idx;
        let mut draws = Vec::new();
        let mut char_ext = 0.0f64;
        if let Some(block) = self.library.blocks.get(b) {
            for (ci, ch) in block.characteristics.iter().enumerate() {
                let Some(st) = self.chars.get(&(b, ci)) else { continue };
                if !st.visible {
                    continue;
                }
                let Some(geo) = &st.geo else { continue };
                if let Some((x0, y0, x1, y1)) = geo.bounds {
                    char_ext = char_ext.max(x0.abs()).max(x1.abs()).max(y0.abs()).max(y1.abs());
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
        let mut max = [char_ext, 0.0];
        for l in &loci {
            let s = usize::from(l.power && !diff);
            for p in &l.pts {
                for v in p {
                    if v.is_finite() {
                        max[s] = max[s].max(v.abs());
                    }
                }
            }
        }
        // Layout: client − 6 px, top + 5 (splitter), bottom − legend (20)
        // or − the readout table when there is room for a 120 px plot.
        let text_h = 14.0f32;
        let mut rect = client.shrink(6.0);
        rect.min.y += 5.0;
        let rows = loci.len() as f32;
        let table_h = (text_h + 6.0) * rows + 12.0;
        let table = readout != 0 && rect.height() - table_h >= 120.0;
        rect.max.y -= if table { table_h } else { 20.0 };
        let (w, h) = (rect.width() as i32, rect.height() as i32);
        let mut view = view;
        let mut sc = dg::plane_scale(w, h, view.z, max);
        if sc.ppu[0].is_some_and(|ppu| char_ext * ppu > dg::ZOOM_RESET_PX) {
            view.z = 1.0;
            sc = dg::plane_scale(w, h, 1.0, max);
        }
        let cell = sc.cell as f32;
        let c = Pos2::new(rect.left() + (w / 2) as f32, rect.top() + (h / 2) as f32);
        let origin = c + Vec2::new(view.ox as f32 * cell, view.oy as f32 * cell);
        let (nx, ny) = ((w / 2) / sc.cell, (h / 2) / sc.cell);
        // Grid (`0x4393b0`): lines every cell outward from the centre, the
        // frame at a whole number of cells, axes through the origin.
        let (fx, fy) = (nx as f32 * cell, ny as f32 * cell);
        let frame = Rect::from_min_max(c - Vec2::new(fx, fy), c + Vec2::new(fx, fy));
        // Printed pens (`0x439486`): grid (150,150,150) ⌊s/2⌋, frame
        // (50,50,50) and axes black ⌊2s/3⌋.
        let (gs, fs, axs) = match print_s() {
            Some(s) => (dev_pen(s / 2.0, AXES), dev_pen(2.0 * s / 3.0, Color32::from_rgb(50, 50, 50)), dev_pen(2.0 * s / 3.0, Color32::BLACK)),
            None => (stroke(1.0, GRID), stroke(1.0, FRAME), stroke(1.0, AXES)),
        };
        for k in 0..ny {
            for y in [c.y + k as f32 * cell, c.y - k as f32 * cell] {
                painter.line_segment([Pos2::new(frame.left(), y), Pos2::new(frame.right(), y)], gs);
            }
        }
        for k in 0..nx {
            for x in [c.x + k as f32 * cell, c.x - k as f32 * cell] {
                painter.line_segment([Pos2::new(x, frame.top()), Pos2::new(x, frame.bottom())], gs);
            }
        }
        painter.rect_stroke(frame, 0.0, fs);
        if (rect.left()..=rect.right()).contains(&origin.x) {
            painter.line_segment([Pos2::new(origin.x, frame.top()), Pos2::new(origin.x, frame.bottom())], axs);
        }
        if (rect.top()..=rect.bottom()).contains(&origin.y) {
            painter.line_segment([Pos2::new(frame.left(), origin.y), Pos2::new(frame.right(), origin.y)], axs);
        }
        // Edge labels: the coordinate at each frame edge (pan included).
        let edge = |n: i32, o: i32, s: usize, sign: f64| (sign * f64::from(n) - f64::from(o)) * sc.upc[s];
        let edge_y = |n: i32, s: usize, sign: f64| (f64::from(view.oy) + sign * f64::from(n)) * sc.upc[s];
        let fmt2 = |axis: &str, a: f64, b: f64, ub: &str| {
            let ((ta, pa), (tb, pb)) = (g_si(a), g_si(b));
            format!(" {axis}: {ta} {pa}Ω, {tb} {pb}{ub}")
        };
        let fmt1 = |a: f64| {
            let (t, p) = g_si(a);
            format!(" {t} {p}")
        };
        let labels: [(String, Pos2, egui::Align2); 4] = if diff {
            [
                (fmt1(edge(nx, view.ox, 0, 1.0)), Pos2::new(frame.right() - 3.0, c.y - 3.0), egui::Align2::RIGHT_BOTTOM),
                (fmt1(edge(nx, view.ox, 0, -1.0)), Pos2::new(frame.left() + 3.0, c.y - 3.0), egui::Align2::LEFT_BOTTOM),
                (fmt1(edge_y(ny, 0, 1.0)), Pos2::new(c.x + 3.0, frame.top() + 3.0), egui::Align2::LEFT_TOP),
                (fmt1(edge_y(ny, 0, -1.0)), Pos2::new(c.x + 3.0, frame.bottom() - 3.0), egui::Align2::LEFT_BOTTOM),
            ]
        } else {
            [
                (fmt2("R", edge(nx, view.ox, 0, 1.0), edge(nx, view.ox, 1, 1.0), "W"), Pos2::new(frame.right() - 3.0, c.y - 3.0), egui::Align2::RIGHT_BOTTOM),
                (fmt2("R", edge(nx, view.ox, 0, -1.0), edge(nx, view.ox, 1, -1.0), "W"), Pos2::new(frame.left() + 3.0, c.y - 3.0), egui::Align2::LEFT_BOTTOM),
                (fmt2("X", edge_y(ny, 0, 1.0), edge_y(ny, 1, 1.0), "var"), Pos2::new(c.x + 3.0, frame.top() + 3.0), egui::Align2::LEFT_TOP),
                (fmt2("X", edge_y(ny, 0, -1.0), edge_y(ny, 1, -1.0), "var"), Pos2::new(c.x + 3.0, frame.bottom() - 3.0), egui::Align2::LEFT_BOTTOM),
            ]
        };
        for (t, at, align) in &labels {
            painter.text(*at, *align, t, font(), TEXT);
        }
        // Scale box: one cell filled (240,240,240) and the value of a cell.
        let bx = Rect::from_min_size(Pos2::new(frame.left() + cell, frame.bottom() - 2.0 * cell), Vec2::splat(cell));
        painter.rect(bx, 0.0, Color32::from_gray(240), stroke(1.0, AXES));
        let scale_text = if diff {
            fmt1(sc.upc[0])
        } else {
            let ((ta, pa), (tb, pb)) = (g_si(sc.upc[0]), g_si(sc.upc[1]));
            format!(" {ta} {pa}Ω, {tb} {pb}W")
        };
        painter.text(Pos2::new(bx.right() + 3.0, bx.center().y), egui::Align2::LEFT_CENTER, scale_text, font(), TEXT);
        let to_px = |x: f64, y: f64, ppu: f64| [origin.x + (x * ppu) as f32, origin.y - (y * ppu) as f32];
        let clip = painter.with_clip_rect(client);
        // Characteristics (`0x43a4a0`), last first so the first ends on top,
        // only while their extent stays under 800 px.
        if let Some(ppu) = sc.ppu[0].filter(|ppu| char_ext * ppu < dg::ZOOM_RESET_PX) {
            for d in draws.iter().rev() {
                let [op, rs, fl] = dg::CHAR_PALETTE[d.pal];
                let rings = |shapes: &[Shape]| -> Vec<Vec<Vec<[f32; 2]>>> {
                    shapes
                        .iter()
                        .map(|Shape::Region(rr)| rr.iter().filter(|r| r.len() >= 3).map(|r| r.iter().map(|p| to_px(p[0], p[1], ppu)).collect()).collect())
                        .collect()
                };
                if show_reset {
                    for r in rings(&d.rs) {
                        paint_region(&clip, &r, None, rgb(rs), client);
                    }
                }
                let (r, g, bb) = fl;
                for r_ in rings(&d.op) {
                    paint_region(&clip, &r_, Some(Color32::from_rgba_unmultiplied(r, g, bb, dg::FILL_ALPHA)), rgb(op), client);
                }
            }
        }
        // Loci (`0x43a880`): a polyline per channel / pair, "+" at cursor 1
        // (red) and cursor 2 (blue).
        for l in &loci {
            let Some(ppu) = sc.ppu[usize::from(l.power && !diff)] else { continue };
            let mut run: Vec<Pos2> = Vec::new();
            let flush = |run: &mut Vec<Pos2>| {
                if run.len() >= 2 {
                    clip.add(egui::Shape::line(std::mem::take(run), stroke(1.0, l.color)));
                }
                run.clear();
            };
            for p in &l.pts {
                if p[0].is_finite() && p[1].is_finite() {
                    let q = to_px(p[0], p[1], ppu);
                    run.push(Pos2::new(q[0], q[1]));
                } else {
                    flush(&mut run);
                }
            }
            flush(&mut run);
            for (p, col) in [(l.c1, Color32::RED), (l.c2, Color32::BLUE)] {
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
        let lim = (dg::max_pan(w, view.z), dg::max_pan(h, view.z));
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
        // Wheel (`0x437ae0`).
        if resp.hovered() && self.diag.menu.is_none() {
            let (raw, mods) = ui.input(|i| (i.raw_scroll_delta, i.modifiers));
            let delta = raw.x + raw.y;
            if delta != 0.0 {
                let g = PlaneGeom { w, h, max, char_max: char_ext };
                match dg::wheel_action(delta > 0.0, mods.ctrl || mods.command, mods.shift) {
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
        if diff && view.initial_pending {
            view.push_lower_left(lim.0, lim.1);
            ui.ctx().request_repaint();
        }
        *self.plane_view(diff) = view;
        self.diag.geom[gi] = Some(PlaneGeom { w, h, max, char_max: char_ext });
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
        let n = d.osc.samples_per_cycle().max(2);
        let phasor_of = |name: &str| -> Option<Complex64> {
            if let Some(i) = d.osc.analog.iter().position(|c| c.id == name) {
                return d.analysis.phasors.get(i).and_then(|p| p.get(k)).copied();
            }
            d.computed.iter().find(|c| c.name == name).map(|c| match &c.phasors {
                Some(p) => p.get(k).copied().unwrap_or_default(),
                None => fvb_core::freqcorr::phasor_at(fvb_core::freqcorr::of(&d.osc), fvb_core::freqcorr::OTHER, &c.samples, k, n),
            })
        };
        let shown: Vec<(String, String, Complex64, Color32)> = doc_chans(d)
            .into_iter()
            .filter(|c| chan_flag(d, Window::Vector, c))
            .map(|c| {
                let z = fvb_core::views::finite_or_zero(phasor_of(&c.name).unwrap_or_default());
                (c.name, c.unit, z, c.color)
            })
            .collect();
        let reference = angle_ref.as_deref().and_then(phasor_of).map(fvb_core::views::finite_or_zero);
        let is_current = |unit: &str| unit == "A";
        let group_max = |current: bool| shown.iter().filter(|s| is_current(&s.1) == current).map(|s| s.2.norm()).fold(0.0, f64::max);
        let (i_scale, u_scale) = (fvb_core::views::nice_scale(group_max(true)), fvb_core::views::nice_scale(group_max(false)));
        // Header «U max» / «I max» [I: placement].
        let (us, up) = g_si(u_scale);
        let (is, ip) = g_si(i_scale);
        let header = format!("U max = {us} {up}V    I max = {is} {ip}A");
        painter.text(full.left_top() + Vec2::new(6.0, 4.0), egui::Align2::LEFT_TOP, header, font(), TEXT);
        let area = Rect::from_min_max(full.min + Vec2::new(0.0, 22.0), full.max);
        let r = (area.width().min(area.height()) / 2.0 - 10.0).max(10.0);
        let c = area.center();
        // The circle count takes R in device pixels (`⌊R/50⌋`, unscaled), so
        // a printed page nearly always has 10; printed pens are solid
        // (100,100,100) and (50,50,50), ⌊s/3⌋ wide (`0x490360`).
        let ps = print_s();
        let nc = dg::vector_circles(r * ps.unwrap_or(1.0));
        for j in 1..=nc {
            let rr = r * j as f32 / nc as f32;
            if let Some(s) = ps {
                let col = if j == nc { Color32::from_rgb(50, 50, 50) } else { Color32::from_rgb(100, 100, 100) };
                painter.circle_stroke(c, rr, dev_pen(s / 3.0, col));
            } else if j == nc {
                painter.circle_stroke(c, rr, stroke(1.0, AXES));
            } else {
                let pts: Vec<Pos2> = (0..=120).map(|t| c + Vec2::angled(t as f32 / 120.0 * std::f32::consts::TAU) * rr).collect();
                painter.extend(egui::Shape::dashed_line(&pts, stroke(1.0, FRAME), 2.0, 2.0));
            }
        }
        for j in 0..12 {
            let a = j as f32 * std::f32::consts::PI / 6.0;
            let end = c + Vec2::new(a.cos(), -a.sin()) * r;
            match ps {
                Some(s) => {
                    painter.line_segment([c, end], dev_pen(s / 3.0, Color32::from_rgb(100, 100, 100)));
                }
                None => dotted(&painter, c, end, FRAME, 1.0),
            }
        }
        let mut lines = Vec::new();
        for (name, unit, z, color) in &shown {
            let ang = fvb_core::views::vector_angle(*z, reference);
            let scale = if is_current(unit) { i_scale } else { u_scale };
            let len = if dir_only { r } else { (z.norm() / scale) as f32 * r };
            let dir = Vec2::new(ang.cos() as f32, -(ang.sin() as f32));
            let tip = c + dir * len;
            // A pen s wide: solid for V, PS_DOT otherwise — which GDI draws
            // solid once wider than 1, i.e. on paper.
            let stroke = stroke(1.0, *color);
            if unit == "V" || ps.is_some_and(|s| s > 1.0) {
                painter.line_segment([c, tip], stroke);
            } else {
                painter.extend(egui::Shape::dashed_line(&[c, tip], stroke, 3.0, 2.0));
            }
            // Filled triangular arrowhead: min(18·s, L/2) long, a fifth of
            // that to each side.
            if len > 1.0 {
                let head = (len / 2.0).min(18.0);
                let back = tip - dir * head;
                let nrm = Vec2::new(-dir.y, dir.x) * (head / 5.0);
                painter.add(egui::Shape::convex_polygon(vec![tip, back + nrm, back - nrm], *color, Stroke::NONE));
            }
            if show_values {
                let (v, p) = g_si(z.norm());
                lines.push((format!("{name}  {v} {p}{unit}{}", fvb_core::views::angle_text(ang, radians)), *color));
            }
        }
        // «Показывать значения»: magnitude and angle per phasor [I: layout].
        for (i, (t, col)) in lines.iter().enumerate() {
            painter.text(area.left_top() + Vec2::new(6.0, 4.0 + 15.0 * i as f32), egui::Align2::LEFT_TOP, t, font(), *col);
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
        let chans: Vec<usize> = (0..d.osc.analog.len()).filter(|i| d.chans[*i].on_freq).collect();
        let s = print_s().unwrap_or(1.0);
        let si = s as i32;
        let ink = Color32::from_gray(20);
        let h = (painter.layout_no_wrap("A".into(), font(), ink).size().y * s).round() as i32;
        let dev = |v: f32| (v * s).round() as i32;
        let pt = |x: i32, y: i32| Pos2::new(x as f32 / s, y as f32 / s);
        let Some(g) = dg::freq_geom((dev(full.left()), dev(full.top()), dev(full.right()), dev(full.bottom())), si, chans.len(), k_max, h) else { return };
        let width = |t: &str, fid: FontId| (painter.layout_no_wrap(t.into(), fid, ink).size().x * s).round() as i32;
        let rot_font = FontId::proportional(12.0);
        let corr = fvb_core::freqcorr::of(&d.osc);
        // In print the panels tile the page by the screen pane's aspect.
        let screen = self.pane_canvas.iter().find(|(p, _)| *p == crate::app::Pane::Frequency).map_or((0, 0), |(_, r)| (r.width() as i32, r.height() as i32));
        let tl = if print_s().is_some() { g.print_tiling(chans.len(), k_max, screen) } else { g.screen_tiling(chans.len()) };
        // Pass 1: swatch and name under each panel's row; the x labels
        // (with several rows their rects are inverted: none).
        for (pi, &i) in chans.iter().enumerate() {
            let (c, r) = tl.cell(pi);
            let yb = tl.row_bottom(&g, r);
            let name = format!("{}{}", d.osc.analog[i].id, if percent { ", %" } else { "" });
            let x0 = tl.swatch_x(&g, c, width(&name, font()));
            let sw = Rect::from_min_max(pt(x0, yb + 3), pt(x0 + 10 * si, yb + 3 + 10 * si));
            painter.rect_filled(sw, 0.0, d.chans[i].color);
            painter.rect_stroke(sw.shrink(0.5), 0.0, Stroke::new(1.0f32, Color32::BLACK));
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
        for (pi, &i) in chans.iter().enumerate() {
            let (c, r) = tl.cell(pi);
            let (row_top, yb) = (g.top + r * tl.row_h, tl.row_bottom(&g, r));
            let samples = &d.flagged.analog[i].samples;
            let mine = corr.and_then(|cr| cr.for_chan(i));
            let sp = fvb_core::freqcorr::spectrum(mine, samples, pos, n, periods, k_max.max(periods), d.osc.harmonic1_gain);
            let max = sp[..=k_max].iter().copied().fold(0.0, f64::max);
            let max = if max == 0.0 { 1.0 } else { max };
            let norm = if use_rms { fvb_core::freqcorr::rms_at(corr, i, samples, pos, n, periods) } else { sp.get(periods).copied().unwrap_or(0.0) };
            let color = d.chans[i].color;
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
                    painter.rect_stroke(bar.shrink(0.5), 0.0, Stroke::new(1.0f32, color));
                }
                let val = if percent {
                    if norm == 0.0 { 0.0 } else { 100.0 * v / norm }
                } else {
                    v
                };
                let label = fvb_core::views::freq_label(val).trim().to_string();
                if 6 * width(&label, font()) / 5 <= tl.slot {
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
        let axis = match print_s() {
            Some(s) => dev_pen(s / 2.0, Color32::from_rgb(50, 50, 50)),
            None => stroke(1.0, AXES),
        };
        for c in 0..chans.len() as i32 {
            let x = g.left + 9 * si + c * tl.panel_w;
            painter.line_segment([pt(x, g.top + 12 * si + g.h), pt(x, g.bottom - 6 * si - 1)], axis);
        }
        for idx in 0..tl.rows {
            let y = g.b - 1 - 18 * si - idx * tl.row_h;
            painter.line_segment([pt(g.left + 6 * si, y), pt(g.right - 6 * si, y)], axis);
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
        egui::Window::new(title).collapsible(false).resizable(false).show(ctx, |ui| {
            egui::Grid::new("pair_combos").num_columns(2).show(ui, |ui| {
                ui.label(ca);
                ui.label(cb);
                ui.end_row();
                for (slot, id) in [(&mut dlg.a, "pair_a"), (&mut dlg.b, "pair_b")] {
                    egui::ComboBox::from_id_salt(id).width(180.0).selected_text(slot.clone().unwrap_or_default()).show_ui(ui, |ui| {
                        for n in &names {
                            ui.selectable_value(slot, Some(n.clone()), n);
                        }
                    });
                }
                ui.end_row();
            });
            ui.horizontal(|ui| {
                // «Добавить» needs both channels; the new row is selected.
                if ui.add_enabled(dlg.a.is_some() && dlg.b.is_some(), egui::Button::new("Добавить")).clicked() {
                    dlg.pairs.push(ChanPair::new(dlg.a.clone().unwrap_or_default(), dlg.b.clone().unwrap_or_default()));
                    dlg.sel = Some(dlg.pairs.len() - 1);
                }
                // «Удалить» removes the selected row and selects the next (or last).
                if ui.add_enabled(dlg.sel.is_some(), egui::Button::new("Удалить")).clicked() {
                    if let Some(s) = dlg.sel {
                        dlg.pairs.remove(s);
                        dlg.sel = if dlg.pairs.is_empty() { None } else { Some(s.min(dlg.pairs.len() - 1)) };
                    }
                }
            });
            egui::Frame::none().stroke(ui.visuals().widgets.noninteractive.bg_stroke).show(ui, |ui| {
                ui.set_min_size(Vec2::new(380.0, 120.0));
                egui::Grid::new("pair_list").num_columns(2).min_col_width(185.0).striped(true).show(ui, |ui| {
                    ui.strong(ca);
                    ui.strong(cb);
                    ui.end_row();
                    for (i, p) in dlg.pairs.iter().enumerate() {
                        let sel = dlg.sel == Some(i);
                        if ui.selectable_label(sel, &p.a).clicked() | ui.selectable_label(sel, &p.b).clicked() {
                            dlg.sel = Some(i);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    done = Some(true);
                }
                if ui.button("Отмена").clicked() {
                    done = Some(false);
                }
            });
        });
        match done {
            None => self.diag.pair_dlg = Some(dlg),
            Some(false) => {}
            Some(true) => {
                if let Some(d) = self.active_doc_mut() {
                    if dlg.diff {
                        d.dif_pairs = dlg.pairs;
                    } else {
                        d.rx_pairs = dlg.pairs;
                    }
                }
            }
        }
    }

    /// DIALOG 353 «Настройки частотной диаграммы» (`0x46ec30`); the
    /// normalisation radios are disabled with absolute values (`0x46ee20`).
    fn freq_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.diag.freq_dlg.take() else { return };
        let mut done: Option<bool> = None;
        egui::Window::new("Настройки частотной диаграммы").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.group(|ui| {
                ui.label("Значения величин");
                ui.radio_value(&mut dlg.percent, true, "В процентах (нормализовано)");
                ui.radio_value(&mut dlg.percent, false, "Абсолютные величины");
            });
            ui.add_enabled_ui(dlg.percent, |ui| {
                ui.group(|ui| {
                    ui.label("Нормализация");
                    ui.radio_value(&mut dlg.use_rms, false, "По основной гармонике");
                    ui.radio_value(&mut dlg.use_rms, true, "По среднеквадратичному значению");
                });
            });
            ui.group(|ui| {
                ui.label("Расчет значений");
                egui::Grid::new("freq_calc").num_columns(2).show(ui, |ui| {
                    ui.label("Число гармоник:");
                    ui.add(egui::DragValue::new(&mut dlg.harmonics).range(1..=200));
                    ui.end_row();
                    ui.label("Число обрабатываемых периодов:");
                    ui.add(egui::DragValue::new(&mut dlg.periods).range(1..=50));
                    ui.end_row();
                });
            });
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    done = Some(true);
                }
                if ui.button("Отмена").clicked() {
                    done = Some(false);
                }
            });
        });
        match done {
            None => self.diag.freq_dlg = Some(dlg),
            Some(false) => {}
            Some(true) => self.apply_freq_dialog(dlg),
        }
    }

    /// OK of DIALOG 353.
    fn apply_freq_dialog(&mut self, dlg: FreqDialog) {
        self.freq_percent = dlg.percent;
        self.freq_use_rms = dlg.use_rms;
        self.freq_harmonics = dlg.harmonics.max(1);
        self.freq_periods = dlg.periods.max(1);
    }
}

/// The legend strip (`0x438210`): bottom 20 px, a centred row of loci only —
/// 10 px colour square, 6 px gap, name, 6 px gap.
fn legend(p: &egui::Painter, ui: &egui::Ui, rect: Rect, loci: &[Locus]) {
    let gals: Vec<_> = loci.iter().map(|l| (ui.painter().layout_no_wrap(l.name.clone(), font(), TEXT), l.color)).collect();
    let total: f32 = gals.iter().map(|(g, _)| g.size().x + 22.0).sum();
    let mut x = rect.left() + ((rect.width() - total) / 2.0).max(0.0);
    for (g, col) in gals {
        let sq = Rect::from_min_size(Pos2::new(x, rect.bottom() + 3.0), Vec2::splat(10.0));
        p.rect(sq, 0.0, col, stroke(1.0, col));
        let wdt = g.size().x;
        p.galley(Pos2::new(x + 16.0, rect.bottom() + 1.0), g, TEXT);
        x += wdt + 22.0;
    }
}

/// The readout table (`0x43bbf0`): one ruled row per locus — swatch, name,
/// value at cursor 1 (mode 1 Re ± jIm `%.3f`, mode 2 `%.3f` and an angle
/// column `%.2fº`); centred when narrower than the window. The differential
/// diagram's boolean readout shows (Iторм; Iдиф) [I].
fn readout_table(p: &egui::Painter, ui: &egui::Ui, rect: Rect, text_h: f32, loci: &[Locus], mode: u8, diff: bool) {
    let row_h = text_h * 1.5;
    let cells: Vec<Vec<String>> = loci
        .iter()
        .map(|l| {
            let [re, im] = l.c1;
            if diff {
                return vec![l.name.clone(), format!("{re:.3}; {im:.3}")];
            }
            let z = Complex64::new(re, im);
            let (_, pre) = fvb_core::omp::si(z.norm());
            let k = fvb_core::omp::si(z.norm()).0 / z.norm().max(1e-300);
            let k = if z.norm() == 0.0 { 1.0 } else { k };
            if mode == 1 {
                let sign = if im < 0.0 { "-" } else { "+" };
                vec![l.name.clone(), format!("{:.3}{sign}j{:.3} {pre}{}", re * k, (im * k).abs(), l.unit)]
            } else {
                vec![l.name.clone(), format!("{:.3} {pre}{}", z.norm() * k, l.unit), format!("{:.2}º", im.atan2(re).to_degrees())]
            }
        })
        .collect();
    let ncol = cells.first().map_or(2, |r| r.len());
    let mut widths = vec![0.0f32; ncol];
    for r in &cells {
        for (c, t) in r.iter().enumerate() {
            widths[c] = widths[c].max(ui.painter().layout_no_wrap(t.clone(), font(), TEXT).size().x + 12.0);
        }
    }
    let total = text_h + 12.0 + widths.iter().sum::<f32>();
    let left = if total < rect.width() { rect.center().x - total / 2.0 } else { rect.left() };
    let top = rect.bottom() + 6.0;
    let ruled = stroke(1.0, AXES);
    for (ri, (r, l)) in cells.iter().zip(loci).enumerate() {
        let y = top + row_h * ri as f32;
        p.line_segment([Pos2::new(left, y), Pos2::new(left + total, y)], ruled);
        let sq = Rect::from_min_size(Pos2::new(left + 6.0, y + (row_h - text_h) / 2.0), Vec2::splat(text_h));
        p.rect_filled(sq, 0.0, l.color);
        let mut x = left + text_h + 12.0;
        for (c, t) in r.iter().enumerate() {
            p.text(Pos2::new(x + 6.0, y + row_h / 2.0), egui::Align2::LEFT_CENTER, t, font(), TEXT);
            x += widths[c];
        }
    }
    p.line_segment([Pos2::new(left, top + row_h * cells.len() as f32), Pos2::new(left + total, top + row_h * cells.len() as f32)], ruled);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Command, Pane};

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

    #[test]
    fn circle_wheel_pans_and_zooms_without_egui_plot() {
        let mut h = H::new(Pane::Circle);
        assert!(h.app.diag.geom[0].is_some(), "the circle plane painted");
        h.wheel(1.0, egui::Modifiers::NONE);
        assert_eq!(h.app.diag.circle.oy, 2, "plain wheel up: oy + 2");
        h.wheel(-1.0, egui::Modifiers::SHIFT);
        assert_eq!(h.app.diag.circle.ox, -2, "Shift+wheel down: ox − 2");
        h.wheel(1.0, egui::Modifiers::CTRL);
        assert_eq!(h.app.diag.circle.z, 1.5, "Ctrl+wheel up: ×1.5");
        h.wheel(-1.0, egui::Modifiers::CTRL);
        assert_eq!((h.app.diag.circle.z, h.app.diag.circle.ox, h.app.diag.circle.oy), (1.0, 0, 0), "back to z = 1 resets the pans");
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

    #[test]
    fn frequency_settings_dialog_edits_a_copy_until_ok() {
        let mut h = H::new(Pane::Vector);
        assert!(h.app.freq_percent, "percent is the default");
        h.app.diag_cmd(Window::Freq, DiagCmd::FreqSettings, Pos2::ZERO);
        h.app.diag.freq_dlg.as_mut().unwrap().percent = false;
        h.frame(vec![], Default::default());
        assert!(h.app.diag.freq_dlg.is_some() && h.app.freq_percent, "not applied before OK");
        let dlg = h.app.diag.freq_dlg.take().unwrap();
        h.app.apply_freq_dialog(FreqDialog { periods: 0, ..dlg });
        assert!(!h.app.freq_percent);
        assert_eq!(h.app.freq_periods, 1, "at least one period");
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
        app.add_doc(Doc::new(osc, "units".into()));
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
