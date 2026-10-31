//! Printing (ledger L9.22, spec §7.11, backlog 36), from `OnPreparePrinting`
//! `0x4053f0`, `OnBeginPrinting` `0x4054b0`, `OnPrint` `0x4057e0` →
//! `0x405820`, the oscillogram slice `0x405970`, the header `0x405c20`, the
//! footer `0x406590`, the diagram print `0x441fe0` and page setup
//! `0x448a90`.
//!
//! A job is a queue of pages: the oscillogram cut into `N` slices (when
//! «Осциллограмма» is ticked), then one page each for the ticked Частотная,
//! Векторная, Круговая and Дифференциальная diagrams. The oscillogram is
//! painted as on screen — the current horizontal window, all visible lanes
//! stacked from the top — fit to the printable width, with a time ruler at
//! the bottom of every page; each page gets the header above and the footer
//! below its margin rectangle. Pages are painted off screen with egui and
//! rasterised (`raster`).

use crate::app::{App, CopyFocus, Pane};
use crate::lanes::{self, OscView, Row};
use crate::oscgeom as g;
use crate::raster::{self, Canvas};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};

/// Screen resolution the original's sizes are in (LOGPIXELSX of the view).
pub const SCREEN_DPI: f32 = 96.0;
/// The sheet (paper selection is disabled in page setup: PSD_DISABLEPAPER).
pub const A4_MM: (f32, f32) = (210.0, 297.0);

/// «Print Parameters» (`LoadCustomState` `0x448c00` / `SaveCustomState`
/// `0x448da0`): margins in 0.01 mm (default 2000 = 20 mm) and the
/// orientation (1 portrait, 2 landscape).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSetup {
    /// Left, top, right, bottom.
    pub margins: [i32; 4],
    pub portrait: bool,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup { margins: [2000; 4], portrait: true }
    }
}

const KEYS: [&str; 4] = ["Left Margin", "Top Margin", "Right Margin", "Bottom Margin"];

impl PageSetup {
    pub fn load(v: &std::collections::HashMap<String, String>) -> PageSetup {
        let mut s = PageSetup::default();
        for (m, k) in s.margins.iter_mut().zip(KEYS) {
            if let Some(x) = v.get(&format!("Print Parameters/{k}")).and_then(|x| x.parse().ok()) {
                *m = x;
            }
        }
        if let Some(o) = v.get("Print Parameters/Page Orientation").and_then(|x| x.parse::<i32>().ok()) {
            s.portrait = o != 2;
        }
        s
    }

    pub fn store(&self, v: &mut std::collections::HashMap<String, String>) {
        for (m, k) in self.margins.iter().zip(KEYS) {
            v.insert(format!("Print Parameters/{k}"), m.to_string());
        }
        v.insert("Print Parameters/Page Orientation".into(), if self.portrait { "1" } else { "2" }.into());
    }

    /// The A4 sheet in mm, oriented.
    pub fn sheet_mm(&self) -> (f32, f32) {
        self.oriented(A4_MM)
    }

    /// Portrait hardware margins (l, t, r, b) as the oriented page sees
    /// them: landscape turns the sheet a quarter counter-clockwise, its
    /// left the portrait bottom (INFERRED: GDI's landscape PHYSICALOFFSET).
    pub fn oriented_margins(&self, [l, t, r, b]: [f32; 4]) -> [f32; 4] {
        if self.portrait {
            [l, t, r, b]
        } else {
            [b, l, t, r]
        }
    }

    /// A portrait paper size turned by the orientation.
    pub fn oriented(&self, (w, h): (f32, f32)) -> (f32, f32) {
        if self.portrait {
            (w, h)
        } else {
            (h, w)
        }
    }
}

/// A print target: the printable area in device pixels, its resolution and
/// where it sits on the sheet (PDF placement).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Device {
    pub w: usize,
    pub h: usize,
    pub dpi: f32,
    pub sheet_mm: (f32, f32),
    pub offset: (usize, usize),
    /// The height of «A» in the printer DC's stock font, in screen px:
    /// the header's T on the first printed page and in the preview
    /// (`0x405ce1` measures before selecting its own font). `None`: 12.
    pub stock_t: Option<f32>,
}

impl Device {
    /// The whole sheet as the printable area (PDF, the CUPS spooler and the
    /// preview).
    pub fn sheet(setup: &PageSetup, dpi: f32) -> Device {
        Device::paper(setup.sheet_mm(), dpi)
    }

    /// A whole sheet of `(w, h)` mm, already oriented, as the printable area.
    pub fn paper((w, h): (f32, f32), dpi: f32) -> Device {
        Device { w: (w / 25.4 * dpi).round() as usize, h: (h / 25.4 * dpi).round() as usize, dpi, sheet_mm: (w, h), offset: (0, 0), stock_t: None }
    }

    /// A sheet of `sheet` mm (oriented) whose printable area is inside the
    /// hardware margins `m` (l, t, r, b mm, oriented): the page is laid out
    /// on that area (the printer DC's origin, GDI's HORZRES / VERTRES).
    pub fn printable(sheet: (f32, f32), m: [f32; 4], dpi: f32) -> Device {
        let px = |mm: f32| (mm / 25.4 * dpi).round() as usize;
        let (l, t) = (px(m[0]), px(m[1]));
        let (w, h) = (px(sheet.0).saturating_sub(l + px(m[2])).max(1), px(sheet.1).saturating_sub(t + px(m[3])).max(1));
        Device { w, h, dpi, sheet_mm: sheet, offset: (l, t), stock_t: None }
    }

    /// The margin rectangle R in device pixels (`0x405820`: margins in
    /// 0.01 mm inside the printable area).
    pub fn margin_rect(&self, s: &PageSetup) -> Rect {
        let px = |hmm: i32| hmm as f32 / 100.0 / 25.4 * self.dpi;
        let [l, t, r, b] = s.margins.map(px);
        Rect::from_min_max(Pos2::new(l.round(), t.round()), Pos2::new((self.w as f32 - r).round(), (self.h as f32 - b).round()))
    }

    /// Printer pixels per screen pixel (`k` = LOGPIXELSX ratio).
    pub fn k(&self) -> f32 {
        self.dpi / SCREEN_DPI
    }
}

/// The diagram pages' scale (`0x441fe0`): ⌊LOGPIXELSY(printer) /
/// LOGPIXELSY(screen)⌋, an integer — 6 at 600 dpi, 1 at 150 dpi.
pub fn diagram_scale(dpi: f32) -> f32 {
    (dpi / SCREEN_DPI).floor().max(1.0)
}

/// What one page shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageKind {
    /// Oscillogram slice `i` (0-based).
    Osc(usize),
    Diagram(Pane),
}

/// The pages of a job (`0x4054b0`): the checkboxes are Осциллограмма,
/// Частотная, Векторная, Круговая, Дифференциальная.
pub fn page_queue(ticks: [bool; 5], osc_pages: usize) -> Vec<PageKind> {
    let mut q: Vec<PageKind> = if ticks[0] { (0..osc_pages).map(PageKind::Osc).collect() } else { Vec::new() };
    for (t, p) in ticks[1..].iter().zip([Pane::Frequency, Pane::Vector, Pane::Circle, Pane::Differential]) {
        if *t {
            q.push(PageKind::Diagram(p));
        }
    }
    q
}

/// The oscillogram's page count (`0x4600b0`): visible lanes packed into
/// pages of `p` client px, each keeping `band` (30, or 75 with the
/// difference band) for its ruler; a lane that reaches the band starts the
/// next page.
pub fn osc_page_count(heights: &[f32], band: f32, p: f32) -> usize {
    let (mut pages, mut acc) = (1usize, 0.0f32);
    for &h in heights {
        acc += h;
        if acc >= p - band {
            pages += 1;
            acc = h;
        }
    }
    pages
}

/// Lane positions on the stacked print image (`0x45fbe0` / `0x462ff0`).
#[derive(Debug, Clone, PartialEq)]
pub struct Paged {
    /// Top of each lane's waveform and stats; a lane that would reach the
    /// band moves to the next page's top − 1 after a ruler is drawn there.
    pub tops: Vec<f32>,
    /// Where each lane's background is drawn: checked after drawing, so the
    /// background of the lane that moves stays behind on the old page.
    pub bg_tops: Vec<f32>,
    /// A ruler drawn (bottom at this y) just before lane `i`.
    pub ruler_before: Vec<Option<f32>>,
}

pub fn paged_tops(heights: &[f32], band: f32, p: f32) -> Paged {
    let mut out = Paged { tops: Vec::new(), bg_tops: Vec::new(), ruler_before: Vec::new() };
    let (mut y, mut limit) = (0.0f32, p);
    for &h in heights {
        let mut ruler = None;
        if y + h > limit - band {
            ruler = Some(limit);
            y = limit - 1.0;
            limit += p;
        }
        out.tops.push(y);
        out.ruler_before.push(ruler);
        y += h;
    }
    let (mut y, mut limit) = (0.0f32, p);
    for &h in heights {
        out.bg_tops.push(y);
        y += h;
        if y > limit - band {
            y = limit - 1.0 + h;
            limit += p;
        }
    }
    out
}

/// The oscillogram part of a job: the view's client width, the page height
/// in client px (`P_int`) and the number of slices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OscPlan {
    pub client_w: f32,
    pub page_h: f32,
    pub pages: usize,
}

/// A print job: the document, the device, the page setup, the page queue
/// and the oscillogram plan.
#[derive(Debug, Clone)]
pub struct Job {
    pub di: usize,
    pub dev: Device,
    /// The resolution the diagram pages take their scale from — the
    /// printer's (`m_hAttribDC`, `0x442012`), also in the preview.
    pub attrib_dpi: f32,
    pub setup: PageSetup,
    pub pages: Vec<PageKind>,
    pub plan: OscPlan,
}

/// The header's record name (`0x405c20`): the path after its last
/// separator with exactly four characters cut off the end (the extension
/// with its dot, whatever its length); empty without a path.
pub fn header_name(path: Option<&std::path::Path>) -> String {
    let Some(name) = path.and_then(|p| p.file_name()) else { return String::new() };
    let chars: Vec<char> = name.to_string_lossy().chars().collect();
    chars[..chars.len().saturating_sub(4)].iter().collect()
}

/// Header and footer strings (148–152, 145–147).
const S_NAME: &str = "Наименование осциллограммы: ";
const S_START: &str = "Начало записи: ";
const S_DURATION: &str = "Длительность: ";
const S_STATION: &str = "Станция: ";
const S_BLOCK: &str = "Номер регистратора: ";
const S_REPORT: &str = "Отчет сформирован программой FastView";
const S_COMPANY: &str = "ООО «НТЦ «Механотроника»";
const S_URL: &str = "www.mtrele.ru";

/// The printed slice's bitmap resolution (`0x405970`, VM).
const SLICE_PPI: f32 = 136.0;

/// The header/footer font: «Arial» 12 screen px cell height.
const DECOR_FONT: FontId = FontId::proportional(10.5);
/// The header/footer line pitch `T` (≈ the cell height of that font).
const T: f32 = 12.0;

impl App {
    /// The oscillogram plan of document `di` for a device (`0x4054b0`).
    pub fn osc_plan(&self, di: usize, dev: &Device, setup: &PageSetup) -> OscPlan {
        let d = &self.docs[di];
        let rmar = dev.margin_rect(setup);
        let k = dev.k();
        let client_w = if self.view_width > 1.0 { self.view_width } else { 1000.0 };
        let (w_scr, h_scr) = ((rmar.width() / k).floor().max(1.0), (rmar.height() / k).floor().max(1.0));
        let r = w_scr / client_w;
        let page_h = (h_scr / r).floor().max(1.0);
        let band = g::RULER_H + if d.diff_band { g::DIFF_BAND_H } else { 0.0 };
        let rows = lanes::lane_layout(d, self.y_analog);
        let heights: Vec<f32> = rows.iter().map(|r| r.h).collect();
        OscPlan { client_w, page_h, pages: osc_page_count(&heights, band, page_h) }
    }

    /// The job of document `di` for its print ticks.
    pub fn print_job(&self, di: usize, dev: Device, setup: PageSetup) -> Job {
        let plan = self.osc_plan(di, &dev, &setup);
        Job { di, dev, attrib_dpi: dev.dpi, setup, pages: page_queue(self.docs[di].print_ticks, plan.pages), plan }
    }

    /// Paint oscillogram slice `slice` of the stacked print image (`0x405970`
    /// with `Draw` in print mode) on a `client_w` × `page_h` screen.
    fn paint_osc_slice(&self, ctx: &egui::Context, di: usize, plan: &OscPlan, slice: usize) {
        let doc = &self.docs[di];
        let painter = ctx.layer_painter(egui::LayerId::background());
        let dy = -(slice as f32) * plan.page_h;
        let total = plan.pages as f32 * plan.page_h;
        let rows: Vec<Row> = lanes::lane_layout(doc, self.y_analog);
        let abs_time = self.time_unit == crate::app::TimeUnit::Absolute;
        let (label_col, left_w) = ctx.fonts(|fo| lanes::stats_columns(fo, doc, &rows, abs_time));
        let n = doc.osc.sample_count();
        let f = doc.xscale.unwrap_or(1.0);
        let (x0, right) = (0.0, plan.client_w);
        let left = x0 + left_w;
        let visible = g::visible_count(right - x0, left_w, f);
        let first = doc.first.min(g::max_first(n, visible));
        let band = g::RULER_H + if doc.diff_band { g::DIFF_BAND_H } else { 0.0 };
        let heights: Vec<f32> = rows.iter().map(|r| r.h).collect();
        let paged = paged_tops(&heights, band, plan.page_h);
        let image = Rect::from_min_max(Pos2::new(x0, dy), Pos2::new(right, dy + total));
        let lp = painter.with_clip_rect(image);
        let pp = painter.with_clip_rect(Rect::from_min_max(Pos2::new(left, dy), Pos2::new(right, dy + total)));
        painter.rect_filled(image, 0.0, Color32::WHITE);
        // Backgrounds by parity only — no selected highlight in print.
        for (li, r) in rows.iter().enumerate() {
            if li % 2 == 1 {
                let lane = Rect::from_min_size(Pos2::new(x0, dy + paged.bg_tops[li]), Vec2::new(right - x0, r.h));
                lanes::gradient(&lp, lane, Color32::from_rgb(214, 255, 255), Color32::from_rgb(243, 255, 255));
            }
            lp.hline(x0..=right, dy + paged.tops[li] + r.h - 1.0, Stroke::new(1.0f32, Color32::from_rgb(200, 200, 200)));
        }
        let grid_bottom = dy + total - g::RULER_H;
        if self.show_grid {
            let mut x = left + 50.0;
            while x < right {
                painter.vline(x, dy..=grid_bottom, Stroke::new(1.0f32, Color32::from_rgb(230, 230, 230)));
                x += 50.0;
            }
        }
        painter.vline(left, dy..=grid_bottom, Stroke::new(1.0f32, Color32::from_rgb(100, 100, 100)));
        let ov = OscView { doc, unit: self.time_unit, f, first, n, left, right, x0, label_col };
        for (li, r) in rows.iter().enumerate() {
            if let Some(bottom) = paged.ruler_before[li] {
                ov.ruler(&painter, dy + bottom, right, g::RULER_H);
            }
            let range = lanes::row_range(doc, r);
            ov.row(&lp, &pp, r, dy + paged.tops[li], range);
        }
        ov.ruler(&painter, dy + total, right, g::RULER_H + if doc.diff_band { g::DIFF_BAND_H } else { 0.0 });
        let ruler_top = dy + total - g::RULER_H;
        if doc.diff_band {
            ov.band(&painter, ruler_top, right, self.show_grid);
        }
        painter.vline(left, dy..=ruler_top, Stroke::new(1.0f32, Color32::from_rgb(100, 100, 100)));
        let v_bottom = if doc.diff_band { ruler_top - 25.0 } else { ruler_top };
        ov.vizirs(&painter, dy, v_bottom, true);
        let tops: Vec<f32> = paged.tops.iter().map(|t| t + dy).collect();
        let ranges: Vec<(f64, f64)> = rows.iter().map(|r| lanes::row_range(doc, r)).collect();
        // Horizontal визиры: the screen Draw `0x45f950` calls `0x493110` in
        // print as well (lines and the out-of-view labels).
        let ys = [0, 1].map(|j| {
            let hv = &doc.hviz[j];
            let y = hv.chan.filter(|_| hv.value.is_finite()).and_then(|ch| rows.iter().position(|r| r.has(crate::app::ChanKey::Analog(ch)))).map(|li| g::value_y(doc.hviz[j].value, tops[li], rows[li].h, ranges[li].0, ranges[li].1));
            (hv.shown, y)
        });
        lanes::paint_hvizirs(&painter, (x0, left, right), (dy, v_bottom), ys);
        // Comments over the whole image's plot area.
        let x_of = |k: f64| ov.x_of(k);
        let lane_of = |k: crate::app::ChanKey| {
            rows.iter()
                .position(|r| r.has(k))
                .map(|li| (tops[li], rows[li].h))
        };
        // As on screen: the line's own channel range, nothing outside it.
        let level_y = |k: crate::app::ChanKey, v: f64| lanes::level_place(doc, (&rows, &tops, &ranges), k, v);
        let t0 = doc.osc.time.first().copied().unwrap_or(0.0);
        let time_at = |k: usize| doc.osc.time.get(k).copied().unwrap_or(0.0) - t0;
        let us_at = |k: usize| crate::oscgeom::micros(time_at(k));
        let abs_at = |dt: i64| fvb_core::comtrade::moment_time_text(&doc.osc.stamps, doc.osc.rev_year, dt);
        let cgeo = crate::comments::Geo { x_of: &x_of, lane: &lane_of, value_y: &level_y, top: dy, v_bottom, us: &us_at, abs: &abs_at };
        // The element under the pointer prints selected (the same Draw).
        crate::comments::paint(&pp, &doc.comments, doc.comment_hot, &cgeo);
    }

    /// The header (`0x405c20`) and footer (`0x406590`) of page `idx` of the
    /// job, painted around the margin rectangle `r` (screen px).
    fn paint_decor(&self, ctx: &egui::Context, job: &Job, r: Rect, idx: usize, preview: bool) {
        let d = &self.docs[job.di];
        let (k, cur, count) = (job.dev.k(), idx + 1, job.pages.len());
        let p = ctx.layer_painter(egui::LayerId::background());
        let black = Color32::BLACK;
        // Logo slot 50 × 42 just above R (bitmap 341 is the original's
        // artwork: ours draws its emblem), the light band behind the header.
        let (logo_w, logo_h) = (50.0, 42.0);
        let logo_top = r.top() - logo_h - 0.5;
        p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), logo_top), Pos2::new(r.right(), logo_top + logo_h - 1.0 / k)), 0.0, Color32::from_rgb(243, 246, 250));
        // Our emblem in the logo's slot (VM: the original prints its logo
        // there).
        let slot = Rect::from_min_size(Pos2::new(r.left(), logo_top), egui::vec2(logo_w, logo_h));
        crate::ribbon::paint_logo(&p, Rect::from_center_size(slot.center(), egui::vec2(logo_h * 0.84, logo_h * 0.7)), Color32::from_rgb(36, 96, 155), 3.0);
        let x = r.left() + logo_w + 100.0 / k;
        let stem = header_name(d.path.as_deref());
        let t = &d.osc.time;
        let dur = t.last().copied().unwrap_or(0.0) - t.first().copied().unwrap_or(0.0);
        let s = &d.osc.stamps;
        // Three single-line DrawTexts, each clipped to its own 0.9T rect.
        // T: on the first printed page and in the preview the «A» of the
        // printer DC's stock font (the header measures before selecting its
        // Arial and never restores the font); later pages 12.
        let th = if idx == 0 || preview { job.dev.stock_t.unwrap_or(T) } else { T };
        let line = |i: f32| p.with_clip_rect(Rect::from_min_max(Pos2::new(x, logo_top + i * 0.9 * th), Pos2::new(r.right(), logo_top + (i + 1.0) * 0.9 * th)));
        lanes::bold_text(&line(0.0), Pos2::new(x, logo_top), &format!("{S_NAME}\"{stem}\""), DECOR_FONT, black);
        let line2 = format!("{S_START}{}, {}   {S_DURATION}{} c.", s.start_date, s.start_time, fvb_core::views::fmt_g(dur));
        line(1.0).text(Pos2::new(x, logo_top + 0.9 * th), Align2::LEFT_TOP, line2, DECOR_FONT, black);
        let line3 = format!("{S_STATION}\"{}\"   {S_BLOCK}\"{}\"", d.osc.station.trim_end(), d.osc.device);
        line(2.0).text(Pos2::new(x, logo_top + 1.8 * th), Align2::LEFT_TOP, line3, DECOR_FONT, black);
        // Footer: the page number only in jobs of two or more pages (at the
        // right), then
        // the three-piece company line centred, 30 device px apart.
        if count > 1 {
            // At R's right edge (VM).
            p.text(Pos2::new(r.right(), r.bottom() + T), Align2::RIGHT_TOP, cur.to_string(), DECOR_FONT, black);
        }
        let width = |s: &str| ctx.fonts(|f| f.layout_no_wrap(s.to_string(), DECOR_FONT, black).size().x);
        // The company in bold on paper: measured in the face it is drawn in.
        let company_w = ctx.fonts(|f| {
            let font = if preview { None } else { lanes::bold_font(f, DECOR_FONT.size) };
            f.layout_no_wrap(S_COMPANY.to_string(), font.unwrap_or(DECOR_FONT), black).size().x
        });
        let whole = width(S_REPORT) + company_w + width(S_URL) + 2.0 * width("  ");
        let gap = 30.0 / k;
        let mut x = r.center().x - whole / 2.0;
        let y = r.bottom() + 2.0 * T;
        p.text(Pos2::new(x, y), Align2::LEFT_TOP, S_REPORT, DECOR_FONT, black);
        x += width(S_REPORT) + gap;
        if preview {
            p.text(Pos2::new(x, y), Align2::LEFT_TOP, S_COMPANY, DECOR_FONT, black);
        } else {
            lanes::bold_text(&p, Pos2::new(x, y), S_COMPANY, DECOR_FONT, black);
        }
        x += company_w + gap;
        let blue = Color32::from_rgb(0, 0, 200);
        let rect = p.text(Pos2::new(x, y), Align2::LEFT_TOP, S_URL, DECOR_FONT, blue);
        if !preview {
            p.hline(rect.x_range(), rect.bottom() - 1.0, Stroke::new(1.0f32 / k.max(1.0), blue));
        }
    }

    /// Render page `idx` of a job.
    pub fn render_page(&mut self, fonts: &egui::FontDefinitions, job: &Job, idx: usize, preview: bool) -> Canvas {
        let (dev, plan) = (&job.dev, &job.plan);
        let k = dev.k();
        let rdev = dev.margin_rect(&job.setup);
        let r_scr = Rect::from_min_max(rdev.min / k, rdev.max / k);
        let page_scr = Vec2::new(dev.w as f32 / k, dev.h as f32 / k);
        let this = &*self;
        let mut canvas = raster::render(page_scr, k, Some(fonts), |ctx| this.paint_decor(ctx, job, r_scr, idx, preview));
        let (rw, rh) = (rdev.width().max(1.0) as usize, rdev.height().max(1.0) as usize);
        match job.pages[idx] {
            PageKind::Osc(slice) => {
                let ppp = rw as f32 / plan.client_w;
                let size = Vec2::new(plan.client_w, plan.page_h);
                // Shrunk below a pixel a point (the preview): drawn at about
                // a pixel a point and shrunk keeping ink. GDI draws every
                // line at the scale shown in its full colour; a pixel-centre
                // test would drop the thin ones in a beat pattern, an
                // average would pale them.
                // At `k` px a point, every 1 px line kept in its colour.
                let at = |k: f32| {
                    let (w, h) = (((size.x * k).round() as usize).max(1), ((size.y * k).round() as usize).max(1));
                    if k < 1.0 {
                        let ss = (1.0 / k).ceil();
                        raster::render(size, k * ss, Some(fonts), |ctx| this.paint_osc_slice(ctx, job.di, plan, slice)).shrink_ink(w, h)
                    } else {
                        raster::render(size, k, Some(fonts), |ctx| this.paint_osc_slice(ctx, job.di, plan, slice))
                    }
                };
                // A real print (`0x405970`): the view's painter into a memory
                // bitmap (MM_ISOTROPIC, about 136 px an inch — VM: A4 at 600
                // dpi, 910 px across R) stretched onto R, so a line is a
                // bitmap pixel wide on paper (VM: 4.4 printer px) and the
                // text as blocky.
                let img = if job.dev.dpi > SLICE_PPI && !preview {
                    let k = rw as f32 / job.dev.dpi * SLICE_PPI / plan.client_w;
                    at(k).enlarge_nearest(((size.x * ppp).round() as usize).max(1), ((size.y * ppp).round() as usize).max(1))
                } else {
                    at(ppp)
                };
                canvas.blit(&Canvas::white(rw, rh), rdev.min.x as i64, rdev.min.y as i64);
                let img = if img.h > rh { crop(&img, rw, rh) } else { img };
                canvas.blit(&img, rdev.min.x as i64, rdev.min.y as i64);
                frame(&mut canvas, rdev);
            }
            PageKind::Diagram(pane) => {
                // `0x441fe0`: the painter at the integer scale ⌊printer dpi /
                // screen dpi⌋ into all of R, with its print branches.
                // The preview takes the printer's scale too (its attribute DC
                // is the printer): the page laid out as on paper, then shown
                // at the preview's resolution.
                let s = diagram_scale(job.attrib_dpi);
                let to_printer = job.attrib_dpi / dev.dpi;
                let page = (!preview).then(|| rdev.min.to_vec2() * to_printer / s);
                let size = Vec2::new(rw as f32, rh as f32) * to_printer / s;
                let kd = s / to_printer;
                // GDI's pens: none under a device pixel, unfeathered.
                let img = crate::diagram_ui::with_print_scale(Some(s), page, || crate::diagram_ui::with_pixel(1.0 / kd, || raster::crisp(|| self.render_diagram(fonts, job.di, pane, size, kd))));
                canvas.blit(&img, rdev.min.x as i64, rdev.min.y as i64);
                // At s = 1 the circle / differential painter's screen branch
                // also paints its 5 px splitter strip after RestoreDC: a
                // PatBlt at the DC's (0, 0), as wide as the live window
                // (`0x438110`, a solid brush by the look: `Skin::splitter`).
                if s == 1.0 && matches!(pane, Pane::Circle | Pane::Differential) {
                    let live = self.pane_canvas.iter().find(|(p, _)| *p == pane).map_or(0.0, |(_, r)| r.width());
                    let (r, g, b, _) = self.skin.splitter().to_tuple();
                    let w = ((live / to_printer) as usize).clamp(1, canvas.w.max(1));
                    canvas.blit(&Canvas { w, h: 5, px: vec![[r, g, b]; w * 5] }, 0, 0);
                }
            }
        }
        canvas
    }

    /// Where a page's lines are 1-pixel pens to keep when the preview
    /// shrinks it (the diagram's rect R on its sheet; an oscillogram slice
    /// is already shrunk keeping ink).
    pub fn ink_rect(job: &Job, idx: usize) -> Option<Rect> {
        matches!(job.pages.get(idx), Some(PageKind::Diagram(_))).then(|| job.dev.margin_rect(&job.setup).translate(Vec2::new(job.dev.offset.0 as f32, job.dev.offset.1 as f32)))
    }

    /// A diagram page (`0x441fe0`): the pane's own painter into R, at the
    /// printer/screen scale.
    /// Copy's picture of the oscillogram view (`0x405160`): the screen
    /// painter over the view's client rect (no scroll bars, no popups) at
    /// the screen's resolution — what the original blits into its EMF.
    pub fn view_picture(&mut self) -> Option<Canvas> {
        let (full, client) = self.view_rects?;
        let fonts = self.print.fonts.clone();
        let saved = (self.lane_menu.take(), self.rename_edit.take(), self.level_edit.take());
        let ppp = self.ppp;
        let img = raster::render(full.size(), ppp, Some(&fonts), |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| self.lanes_ui(ui));
        });
        (self.lane_menu, self.rename_edit, self.level_edit) = saved;
        self.view_rects = Some((full, client));
        Some(crop(&img, (client.width() * ppp).round() as usize, (client.height() * ppp).round() as usize))
    }

    /// Copy's picture of a diagram (`0x441e60`): its window (the canvas,
    /// no toolbar or property grid) as last drawn, at the screen's
    /// resolution.
    pub fn diagram_picture(&mut self, pane: Pane) -> Option<Canvas> {
        let rect = self.pane_canvas.iter().chain(&self.canvas_memo).find(|(p, _)| *p == pane)?.1;
        let (di, fonts, ppp) = (self.active?, self.print.fonts.clone(), self.ppp);
        Some(self.render_diagram(&fonts, di, pane, rect.size(), ppp))
    }

    /// A press in a diagram window gives its pane the focus, a press in a
    /// circle / differential property grid gives the grid the focus, a
    /// press in the oscillogram view takes it back (copy follows the
    /// focus).
    pub fn track_copy_focus(&mut self, ctx: &egui::Context) {
        if self.modal_up {
            return;
        }
        let Some(p) = ctx.input(|i| if i.pointer.any_pressed() { crate::lanes::press_pos(i) } else { None }) else { return };
        // A slid-out pane lies over the rest: a press on it (egui's top
        // layer there) reaches only its own windows; a press on a menu or
        // another popup reaches no pane under it; elsewhere the topmost
        // pane window (floating ones drawn after the docked).
        let top = ctx.layer_id_at(p);
        let slid = top.and_then(crate::views::slid_pane_unit);
        let popup = slid.is_none() && top.is_some_and(|l| l.order >= egui::Order::Foreground);
        let editor = top.is_some_and(|l| {
            let at = |name: &str, i: usize| egui::LayerId::new(egui::Order::Foreground, egui::Id::new(name).with(i));
            self.rename_edit.as_ref().is_some_and(|(i, _)| l == at("fv_rename", *i)) || self.level_edit.as_ref().is_some_and(|(i, _)| l == at("fv_level_edit", *i))
        });
        let [c0, c1, g0, g1] = match slid {
            Some(u) => self.slid_ranges[u as usize].unwrap_or([0; 4]),
            None if popup => [0; 4],
            None => [0, self.pane_canvas.len(), 0, self.pane_grid.len()],
        };
        // A floating pane's window on top there: its own diagram or grid,
        // else (caption, tab strip, toolbar) the press keeps the focus
        // where it is — it is a window of its own, not the view under it.
        let on_float = if slid.is_none() && !popup { crate::docking::UNITS.into_iter().find(|&u| top == Some(crate::views::floating_pane_layer(u))) } else { None };
        // The frame's own windows lie under any pane window or popup: a
        // press on the merge bar's combo box or buttons or the tab list's
        // button gives that window the focus (no untitled child: copy takes
        // the view's picture and the document); on a document's tab, the
        // view's (clone).
        if slid.is_none() && !popup && on_float.is_none() {
            if self.focus_other.iter().any(|r| r.contains(p)) {
                self.copy_focus = CopyFocus::Other;
                return;
            }
            if self.doc_tab_rects.iter().any(|r| r.contains(p)) {
                self.copy_focus = CopyFocus::View;
                return;
            }
        }
        let own = |pane: Pane| on_float.is_none_or(|u| self.docks.members(u).any(|m| m == pane));
        if let Some(&(pane, _)) = self.pane_canvas[c0..c1].iter().rev().find(|(q, r)| own(*q) && r.contains(p)) {
            self.copy_focus = CopyFocus::Diagram(pane);
        } else if self.pane_grid[g0..g1].iter().any(|r| r.contains(p) && on_float.is_none_or(|u| self.docks[u].float_rect.is_some_and(|f| f.contains_rect(*r)))) {
            // The grid's group: the slid-out one, else the floating window
            // or docked group it lies in (floating ones over the docked).
            let floating = |u: crate::docking::Unit| self.docks[u].float_rect.filter(|_| self.docks[u].place == crate::docking::Place::Floating);
            let owner = slid
                .or_else(|| crate::docking::UNITS.into_iter().find(|&u| floating(u).is_some_and(|r| r.contains(p))))
                .or_else(|| crate::docking::UNITS.into_iter().find(|&u| self.dock_rects[u as usize].is_some_and(|r| r.contains(p))));
            // The grid is its group's front pane's (circle or differential).
            if let Some(pane) = owner.and_then(|u| self.panes.front(self.docks.members(u))) {
                self.copy_focus = CopyFocus::Grid(pane);
            }
        } else if slid.is_none() && on_float.is_none() && (!popup || editor) && self.view_rects.is_some_and(|(full, _)| full.contains(p)) {
            // A menu or a combo box's list takes no focus (the window that
            // has it keeps it); the lane's rename box and the level editor
            // are windows of the view.
            self.copy_focus = CopyFocus::View;
        }
    }

    /// The group holding the keyboard focus, if a pane's window or one in
    /// it has it.
    pub fn focus_group(&self) -> Option<crate::docking::Unit> {
        match self.copy_focus {
            CopyFocus::View | CopyFocus::Other => None,
            CopyFocus::Diagram(u) | CopyFocus::Grid(u) | CopyFocus::Pane(u) => Some(self.docks.host(u)),
        }
    }

    fn render_diagram(&mut self, fonts: &egui::FontDefinitions, di: usize, pane: Pane, size: Vec2, k: f32) -> Canvas {
        let active = self.active;
        let dim = self.diagram_dim;
        self.active = Some(di);
        let img = raster::render(size, k, Some(fonts), |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::WHITE)).show(ctx, |ui| match pane {
                Pane::Frequency => self.frequency_ui(ui),
                Pane::Vector => self.vector_ui(ui),
                Pane::Circle | Pane::Differential => {
                    // That plane's characteristics with its dimension.
                    self.switch_diagram(if pane == Pane::Circle { fvb_core::model::Dimension::Ohm } else { fvb_core::model::Dimension::Dif });
                    self.plane_ui(ui);
                }
            });
        });
        self.switch_diagram(dim);
        self.active = active;
        img
    }
}

fn crop(c: &Canvas, w: usize, h: usize) -> Canvas {
    let mut out = Canvas::white(w.min(c.w), h.min(c.h));
    for y in 0..out.h {
        out.px[y * out.w..(y + 1) * out.w].copy_from_slice(&c.px[y * c.w..y * c.w + out.w]);
    }
    out
}

/// The 1-device-px black frame around R (`0x405970`, Rectangle with the
/// null brush).
fn frame(c: &mut Canvas, r: Rect) {
    let (x0, y0) = (r.min.x.max(0.0) as usize, r.min.y.max(0.0) as usize);
    let (x1, y1) = ((r.max.x as usize).min(c.w).saturating_sub(1), (r.max.y as usize).min(c.h).saturating_sub(1));
    for x in x0..=x1 {
        c.px[y0 * c.w + x] = [0; 3];
        c.px[y1 * c.w + x] = [0; 3];
    }
    for y in y0..=y1 {
        c.px[y * c.w + x0] = [0; 3];
        c.px[y * c.w + x1] = [0; 3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Command;

    #[test]
    fn page_count_packs_whole_lanes_and_keeps_the_band() {
        // P = 300, band 30: 93 + 93 + 93 = 279 ≥ 270 → the third lane opens page 2.
        assert_eq!(osc_page_count(&[93.0, 93.0, 93.0], 30.0, 300.0), 2);
        assert_eq!(osc_page_count(&[93.0, 93.0], 30.0, 300.0), 1);
        assert_eq!(osc_page_count(&[], 75.0, 300.0), 1);
        let p = paged_tops(&[93.0, 93.0, 93.0, 20.0], 30.0, 300.0);
        assert_eq!(p.tops, [0.0, 93.0, 299.0, 392.0], "the third lane moves to the next page top − 1");
        assert_eq!(p.ruler_before, [None, None, Some(300.0), None]);
        assert_eq!(p.bg_tops, [0.0, 93.0, 186.0, 392.0], "its background stays behind; the next one is in step");
    }

    #[test]
    fn queue_order_and_setup_persistence() {
        assert_eq!(page_queue([true, true, false, true, true], 2), [PageKind::Osc(0), PageKind::Osc(1), PageKind::Diagram(Pane::Frequency), PageKind::Diagram(Pane::Circle), PageKind::Diagram(Pane::Differential)]);
        assert!(page_queue([false; 5], 3).is_empty());
        let mut v = std::collections::HashMap::new();
        let s = PageSetup { margins: [1000, 1500, 2000, 2500], portrait: false };
        s.store(&mut v);
        assert_eq!(v["Print Parameters/Page Orientation"], "2");
        assert_eq!(PageSetup::load(&v), s);
        assert_eq!(PageSetup::load(&Default::default()), PageSetup::default());
        // A4 at 96 dpi with 20 mm margins: R = (76, 76) .. (718, 1047).
        let dev = Device::sheet(&PageSetup::default(), 96.0);
        assert_eq!((dev.w, dev.h), (794, 1123));
        assert_eq!(dev.margin_rect(&PageSetup::default()), Rect::from_min_max(Pos2::new(76.0, 76.0), Pos2::new(718.0, 1047.0)));
    }

    /// `0x438110`: a circle page printed at s = 1 carries the live
    /// window's 5 px splitter strip at the page origin; at s ≥ 2 none.
    #[test]
    fn a_circle_page_at_scale_one_carries_the_splitter_strip() {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        app.docs[0].print_ticks = [false, false, false, true, false];
        app.pane_canvas.push((crate::app::Pane::Circle, Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))));
        let fonts = egui::FontDefinitions::default();
        let (r, g, b, _) = app.skin.splitter().to_tuple();
        for (dpi, strip) in [(150.0, true), (300.0, false)] {
            let setup = PageSetup::default();
            let job = app.print_job(0, Device::sheet(&setup, dpi), setup);
            let c = app.render_page(&fonts, &job, 0, false);
            assert_eq!(c.get(3, 2) == [r, g, b], strip, "{dpi} dpi");
        }
    }

    #[test]
    fn renders_an_oscillogram_page_with_frame_and_header() {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        let setup = PageSetup::default();
        let dev = Device::sheet(&setup, 48.0);
        let job = app.print_job(0, dev, setup);
        assert!(matches!(job.pages[0], PageKind::Osc(0)));
        let fonts = egui::FontDefinitions::default();
        let c = app.render_page(&fonts, &job, 0, false);
        assert_eq!((c.w, c.h), (dev.w, dev.h));
        let r = dev.margin_rect(&setup);
        assert_eq!(c.get(r.min.x as usize + 5, r.min.y as usize), [0, 0, 0], "the frame");
        assert_eq!(c.get(2, 2), [255, 255, 255], "the paper margin");
        let header = (r.min.x as usize..r.max.x as usize).filter(|&x| c.get(x, r.min.y as usize - 6) != [255, 255, 255]).count();
        assert!(header > 10, "the header band above R");
        let ink = (r.min.y as usize + 2..r.max.y as usize - 2).filter(|&y| c.get(r.min.x as usize + 40, y) != [255, 255, 255]).count();
        assert!(ink > 0, "lanes are drawn inside R");
        // A diagram page renders without panicking.
        app.docs[0].print_ticks = [false, true, true, true, true];
        let job = app.print_job(0, dev, setup);
        assert_eq!(job.pages.len(), 4);
        for i in 0..job.pages.len() {
            let c = app.render_page(&fonts, &job, i, true);
            assert_eq!((c.w, c.h), (dev.w, dev.h));
        }
    }
}

#[cfg(test)]
mod hviz_tests {
    use super::*;
    use crate::app::Command;

    /// A shown ГВ prints on its lane (`0x45f950` → `0x493110` in print too).
    #[test]
    fn horizontal_vizirs_are_printed() {
        let reddest_row = |app: &mut App| {
            let setup = PageSetup::default();
            let dev = Device::sheet(&setup, 100.0);
            let job = app.print_job(0, dev, setup);
            let c = app.render_page(&egui::FontDefinitions::default(), &job, 0, false);
            let near = |p: [u8; 3]| i32::from(p[0]) > i32::from(p[1]) + 40 && i32::from(p[0]) > i32::from(p[2]) + 40 && (i32::from(p[1]) - i32::from(p[2])).abs() < 30;
            (0..c.h).map(|y| (0..c.w).filter(|&x| near(c.get(x, y))).count()).max().unwrap_or(0)
        };
        let mut app = App::new();
        app.dispatch(Command::Demo);
        let without = reddest_row(&mut app);
        let d = &mut app.docs[0];
        let (lo, hi) = crate::lanes::chan_range(d, 0);
        d.hviz[0] = crate::app::HVizir { shown: true, chan: Some(0), value: (lo + hi) / 2.0 };
        let with = reddest_row(&mut app);
        assert!(with > without + 200, "a red line across the plot: {with} vs {without}");
    }
}

#[cfg(test)]
mod dump {
    use super::*;
    use crate::app::Command;

    #[test]
    fn header_name_cuts_four_characters() {
        use std::path::Path;
        assert_eq!(header_name(Some(Path::new("/data/DOC_1999.cfg"))), "DOC_1999");
        assert_eq!(header_name(Some(Path::new("/data/rec.osc2"))), "rec.", "four characters, not the extension");
        assert_eq!(header_name(Some(Path::new("/data/a.b"))), "");
        assert_eq!(header_name(None), "", "an unsaved document");
    }

    #[test]
    fn diagram_pages_use_the_integer_scale() {
        assert_eq!([150.0, 300.0, 600.0, 1200.0].map(diagram_scale), [1.0, 3.0, 6.0, 12.0]);
        // The vector diagram counts circles from R in printer pixels.
        assert_eq!(crate::diagram::vector_circles(150.0), 4);
        assert_eq!(crate::diagram::vector_circles(150.0 * diagram_scale(600.0)), 10);
    }

    /// `FV_PRINT_DUMP=<file.pdf> cargo test print_dump` renders the demo job.
    #[test]
    fn print_dump() {
        let Ok(path) = std::env::var("FV_PRINT_DUMP") else { return };
        let mut app = App::new();
        app.dispatch(Command::Demo);
        app.docs[0].print_ticks = [true, true, true, true, true];
        app.docs[0].main_cursor = 300;
        // A bottom-docked frequency pane as on screen (its aspect tiles the page).
        app.pane_canvas.push((crate::app::Pane::Frequency, Rect::from_min_size(Pos2::ZERO, Vec2::new(1300.0, 200.0))));
        let setup = PageSetup::default();
        let dev = Device::sheet(&setup, 150.0);
        let job = app.print_job(0, dev, setup);
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf") {
            fonts.font_data.insert("cyr".into(), egui::FontData::from_owned(bytes));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "cyr".into());
        }
        let imgs: Vec<Canvas> = (0..job.pages.len()).map(|i| app.render_page(&fonts, &job, i, false)).collect();
        std::fs::write(path, crate::spool::pdf_bytes(&imgs, &dev)).unwrap();
    }
}
