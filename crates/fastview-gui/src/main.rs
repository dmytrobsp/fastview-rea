//! FastView — a cross-platform reconstruction of НТЦ «Механотроника» Fast View 5,
//! the relay-protection oscillogram/characteristic analysis tool. Office-2007
//! ribbon shell, MDI documents and a lane-based oscillogram view. Builds and runs
//! on Linux, Windows and macOS.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod chandlg;
mod colordlg;
mod chansel;
mod comments;
mod diagram;
mod diagram_ui;
mod dialogs;
mod fontfix;
mod docking;
mod finder;
mod formula_dlg;
mod formula_page;
mod icons;
mod instance;
mod keytips;
mod sysclip;
mod lanes;
mod mdi;
mod merge_ui;
#[cfg(windows)]
mod minsize;
mod omp_ui;
mod oscgeom;
mod pdf;
mod persist;
mod popmenu;
mod print_ui;
mod printout;
mod quick_ui;
mod raster;
mod recdlg;
mod ribbon;
mod setfile;
#[cfg(windows)]
mod softrun;
mod spool;
mod theme;
mod tips;
mod util;
mod views;
mod windows_dlg;

use app::App;
use eframe::egui;

/// The frame's minimum tracking size (CMainFrame::OnGetMinMaxInfo
/// `0x4728a0`: 866 × 500, always, after MFC's base handler) is the window
/// rect with Windows 11's invisible borders; what shows of it (VM, by a
/// drag of an edge) is 852 × 493 — ours, with no system frame, as its
/// client. Physical pixels: the original is system-DPI aware.
const MIN_VISIBLE: [f32; 2] = [852.0, 493.0];

/// One point is one device pixel at any display scale. The original is
/// system-DPI aware (its manifest's `dpiAware`), so Windows does not
/// stretch it, and it paints its views in device pixels: their fonts are
/// cell heights (Tahoma 15 / 13, `0x404df9`) and their pens 1 px, at
/// every scale (VM at 125 %: the lanes, rulers, waveforms and diagrams
/// pixel for pixel as at 100 %). Ours lays out and paints in whole pixels
/// too (fonts sized in pixels, pixel-snapped strokes, the aliased
/// polylines): at egui's native fractional scale (1.25, 1.5) its 1 px
/// strokes came out 1 or 2 px wide and its text resampled (#5, #6).
/// Windows only: the original's platform (elsewhere egui's native scale
/// stays, a Retina screen's 2 included).
const DEVICE_PIXELS: bool = cfg!(any(windows, test));

/// The zoom that makes egui's pixels per point (zoom · native) 1: exactly
/// where an f32 does it, else just above (0.99999994 would floor a whole
/// coordinate into the pixel before, 1.0000001 floors and rounds as 1).
fn device_zoom(native: f32) -> f32 {
    // A positive float's neighbours are a bit pattern apart.
    let (up, down) = (|z: f32| f32::from_bits(z.to_bits() + 1), |z: f32| f32::from_bits(z.to_bits() - 1));
    let mut z = 1.0 / native;
    while z * native < 1.0 {
        z = up(z);
    }
    while down(z) * native >= 1.0 {
        z = down(z);
    }
    z
}

/// The primary monitor's display scale (its effective dpi / 96, as winit
/// sizes a new window by), the process made per-monitor DPI aware first —
/// as winit makes it — so that Windows reports device pixels (the work
/// area too).
#[cfg(windows)]
fn system_scale() -> Option<f32> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI};
    let (mut x, mut y) = (0u32, 0u32);
    // SAFETY: process-wide settings and queries into two local u32s.
    let ok = unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        GetDpiForMonitor(MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY), MDT_EFFECTIVE_DPI, &mut x, &mut y) == 0
    };
    (ok && x > 0).then(|| x as f32 / 96.0)
}

#[cfg(not(windows))]
fn system_scale() -> Option<f32> {
    None
}

/// The runner's screen in whole points when they are device pixels: at a
/// scale with no exact [`device_zoom`] (custom ones, 151 %) the pixels
/// per point are 1.0000001 and the window 1280 px wide 1279.9999 points.
fn device_screen(ctx: &egui::Context, raw: &mut egui::RawInput) {
    let native = raw.viewports.get(&egui::ViewportId::ROOT).and_then(|v| v.native_pixels_per_point).or_else(|| ctx.input(|i| i.viewport().native_pixels_per_point));
    if let (true, Some(native), Some(r)) = (DEVICE_PIXELS, native, raw.screen_rect) {
        if ((ctx.zoom_factor() * native) - 1.0).abs() < 1e-6 {
            raw.screen_rect = Some(egui::Rect::from_min_max(r.min.round(), r.max.round()));
        }
    }
}

/// Device pixels from the first frame at the system `scale` (each frame
/// keeps them at the window's monitor's, `App::device_pixels`). No zoom by
/// Ctrl + / − / 0: the original has none.
fn device_options(ctx: &egui::Context, scale: Option<f32>) {
    ctx.options_mut(|o| {
        o.zoom_factor = scale.filter(|_| DEVICE_PIXELS).map_or(1.0, device_zoom);
        o.zoom_with_keyboard = false;
    });
}

/// The window made on a monitor at `native` (not necessarily the
/// primary one the system scale is): device pixels there from its first
/// frame. Ours is per-monitor DPI aware, 1:1 on every monitor — the
/// original, system-DPI aware, is bitmap-stretched by Windows there
/// (`docs/IMPROVEMENTS.md`).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn device_window(ctx: &egui::Context, native: f32) {
    if DEVICE_PIXELS {
        ctx.options_mut(|o| o.zoom_factor = device_zoom(native));
    }
}

/// The display scale of the monitor eframe's window is on (its dpi / 96).
#[cfg(windows)]
fn window_scale(w: &impl winit::raw_window_handle::HasWindowHandle) -> Option<f32> {
    use winit::raw_window_handle::RawWindowHandle;
    let RawWindowHandle::Win32(h) = w.window_handle().ok()?.as_raw() else { return None };
    // SAFETY: a query on our own live window.
    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForWindow(h.hwnd.get() as windows_sys::Win32::Foundation::HWND) };
    (dpi > 0).then(|| dpi as f32 / 96.0)
}

fn main() -> eframe::Result<()> {
    // Before the work area is read (`default_frame_size`).
    let scale = system_scale();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1320.0, 860.0])
        .with_min_inner_size(MIN_VISIBLE)
        .with_title(app::APP_TITLE)
        .with_icon(app_icon())
        .with_drag_and_drop(true);
    // The ribbon draws the caption row (L9.150): no system decorations,
    // unless `FV_SYSTEM_TITLEBAR` asks for them.
    let own_caption = std::env::var_os("FV_SYSTEM_TITLEBAR").is_none();
    viewport = viewport.with_decorations(!own_caption);
    // `LoadState` restores the frame's saved window placement; on a first
    // run the frame is CW_USEDEFAULT's: placed by Windows, three quarters of
    // the work area less its invisible borders (VM: 1906 × 1016 shown of
    // a 2560 × 1392 work area).
    let mut placement = None;
    if let Some((r, max)) = window_placement(&persist::Store::load()) {
        viewport = viewport.with_inner_size(r.size()).with_position(r.min).with_maximized(max);
        placement = (!max).then_some(r);
    } else if let Some(size) = default_frame_size() {
        viewport = viewport.with_inner_size(size);
    }
    // The software runner makes its window after `make_app` has set the
    // device zoom (`device_zoom`): the builder's sizes are device pixels.
    #[cfg(windows)]
    let soft_viewport = viewport.clone();
    // eframe makes its window before the app, at zoom 1 — in points of the
    // system scale.
    if let Some(s) = scale {
        viewport.inner_size = viewport.inner_size.map(|v| v / s);
        viewport.min_inner_size = viewport.min_inner_size.map(|v| v / s);
        viewport.position = viewport.position.map(|p| (p.to_vec2() / s).to_pos2());
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    // Command line (`InitInstance` 0x4483f0): `-Dst <dir>`, `-Find` (open
    // Проводник at start-up) and the file to open.
    let args = startup_args(std::env::args().skip(1));
    // Single instance: a file argument goes to the running instance.
    let inst_dir = instance::default_dir();
    if !args.files.is_empty() && inst_dir.as_deref().is_some_and(|d| instance::hand_off(d, &args.files)) {
        return Ok(());
    }
    // No OpenGL 2 / Direct3D 12 (a basic display driver): the software
    // runner (W4.1c), also forced by `FV_SOFTWARE`.
    #[cfg(windows)]
    let soft = |why: &str| {
        softrun::trace(&format!("software rendering ({why})"));
        let ctx = egui::Context::default();
        let app = make_app(&ctx, &args, inst_dir.as_deref(), own_caption, scale, placement);
        softrun::trace("app made");
        softrun::run(app, soft_viewport.clone(), ctx).map_err(|e| eframe::Error::AppCreation(e.into()))
    };
    #[cfg(windows)]
    if std::env::var_os("FV_SOFTWARE").is_some() {
        return soft("FV_SOFTWARE");
    }
    let r = eframe::run_native(
        "FastView",
        options,
        Box::new(|cc| {
            let app = make_app(&cc.egui_ctx, &args, inst_dir.as_deref(), own_caption, scale, placement);
            #[cfg(windows)]
            if let Some(native) = window_scale(cc) {
                device_window(&cc.egui_ctx, native);
            }
            Ok(Box::new(app))
        }),
    );
    // winit makes one event loop a process: the software runner starts
    // over in a new process of its own.
    #[cfg(windows)]
    if let Err(e) = &r {
        eprintln!("fastview: {e}");
        let exe = std::env::current_exe().map_err(|e| eframe::Error::AppCreation(e.into()))?;
        let status = std::process::Command::new(exe).args(std::env::args_os().skip(1)).env("FV_SOFTWARE", "1").status();
        return status.map(|_| ()).map_err(|e| eframe::Error::AppCreation(e.into()));
    }
    r
}

/// The application as both runners start it: the fonts, the persisted
/// state, the single-instance server, the command line.
fn make_app(ctx: &egui::Context, args: &StartupArgs, inst_dir: Option<&std::path::Path>, own_caption: bool, scale: Option<f32>, placement: Option<egui::Rect>) -> App {
    let fonts = app_fonts();
    ctx.set_fonts(fonts.clone());
    device_options(ctx, scale);
    // A double-click within Windows' default GetDoubleClickTime, 500 ms
    // (egui's own is 300 ms: a hand's double-click on the coefficient list
    // was missed on the VM).
    // A press released in place is a click however long it was held, as a
    // Win32 button or menu item takes it (egui's limit is 0.8 s; VM: an
    // application menu item pressed for 1.5 s acts in the original).
    ctx.options_mut(|o| {
        o.input_options.max_double_click_delay = 0.5;
        o.input_options.max_click_duration = f64::INFINITY;
    });
    let mut app = App::new().with_persistence();
    app.own_caption = own_caption;
    app.placement_px = placement;
    app.splash_until = Some(2.0);
    app.print.fonts = fonts;
    let c = ctx.clone();
    app.instance = inst_dir.and_then(|d| instance::serve(d, move || c.request_repaint()));
    if let Some(f) = &args.dst {
        app.dst_file = Some(f.clone());
    }
    if args.find {
        app.dispatch(app::Command::ToggleExplorer);
    }
    for f in &args.files {
        app.load_comtrade_path(f);
    }
    app
}

/// A first run's frame size on Windows (CW_USEDEFAULT): ¾ of the work
/// area, less the 14 / 28 px of invisible border a framed window keeps
/// (ours has none). Physical pixels, as the original (system-DPI aware).
#[cfg(windows)]
fn default_frame_size() -> Option<[f32; 2]> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: SPI_GETWORKAREA fills the RECT passed.
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut r as *mut RECT).cast(), 0) } != 0;
    let (w, h) = ((r.right - r.left) as f32, (r.bottom - r.top) as f32);
    (ok && w > 0.0 && h > 0.0).then(|| [(w * 0.75).floor() - 14.0, (h * 0.75).floor() - 28.0])
}

#[cfg(not(windows))]
fn default_frame_size() -> Option<[f32; 2]> {
    None
}

/// The window's icon (taskbar, Alt+Tab): our logo — the orb's two
/// chevrons, blue over green — drawn into 48 × 48 RGBA (the original's
/// icon is its artwork).
fn app_icon() -> egui::IconData {
    const N: usize = 48;
    let seg = |p: (f32, f32), a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
    };
    // As `ribbon::paint_logo` over a 40 × 33 box at (4, 8), 5 px strokes.
    let (l, t, w, h) = (4.0f32, 8.0f32, 40.0f32, 33.0f32);
    let chevron = |yoff: f32| [(l, t + h - yoff - h * 0.1), (l + w / 2.0, t + yoff + h * 0.05), (l + w, t + h - yoff - h * 0.1)];
    let marks = [(chevron(0.0), [0x2E, 0x5B, 0x9E]), (chevron(h * 0.36), [0x4C, 0xAF, 0x50])];
    let mut rgba = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            for (pts, col) in &marks {
                let d = seg(p, pts[0], pts[1]).min(seg(p, pts[1], pts[2]));
                let cover = (2.5 + 0.5 - d).clamp(0.0, 1.0);
                if cover > 0.0 {
                    let i = (y * N + x) * 4;
                    let a = (cover * 255.0) as u8;
                    // The later (green) chevron over the first.
                    if a >= rgba[i + 3] || col[1] == 0xAF {
                        rgba[i..i + 3].copy_from_slice(col);
                        rgba[i + 3] = rgba[i + 3].max(a);
                    }
                }
            }
        }
    }
    egui::IconData { rgba, width: N as u32, height: N as u32 }
}

/// `Workspace/WindowPlacement` ("l t r b maximised") as saved on exit.
fn window_placement(st: &persist::Store) -> Option<(egui::Rect, bool)> {
    let s = st.view.get("Workspace/WindowPlacement")?;
    let (rect, max) = s.rsplit_once(' ')?;
    Some((docking::parse_rect(rect)?, max == "1"))
}

/// Wine's Tahoma (LGPL, `assets/fonts/LICENSE-tahoma.txt`): the widths and
/// vertical metrics of the Tahoma the original's GDI text uses.
const TAHOMA: &[u8] = include_bytes!("../../../assets/fonts/tahoma.ttf");
const TAHOMA_BOLD: &[u8] = include_bytes!("../../../assets/fonts/tahomabd.ttf");
/// DejaVu Sans (`assets/fonts/LICENSE-dejavu.txt`): the glyphs Tahoma lacks
/// (arrows, check marks), alike on every platform.
const DEJAVU: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");

/// A font whose size is its em, as GDI's: egui scales a face by its
/// ascent − descent and rounds that height to whole pixels, so Tahoma
/// (1.207 em high) could only take ems of 10.77, 11.6, 12.43 … px — never
/// 12. The copy's `hhea` ascent − descent is made one em (the rest moved
/// into the line gap, so rows keep their height) and the glyphs shifted
/// back down to their own baseline.
fn em_sized(bytes: Vec<u8>) -> egui::FontData {
    let mut b = bytes;
    let rd = |b: &[u8], o: usize| u16::from_be_bytes([b[o], b[o + 1]]);
    let table = |b: &[u8], tag: &[u8; 4]| (0..rd(b, 4) as usize).map(|i| 12 + 16 * i).find(|&r| &b[r..r + 4] == tag).map(|r| u32::from_be_bytes([b[r + 8], b[r + 9], b[r + 10], b[r + 11]]) as usize);
    let (Some(head), Some(hhea)) = (table(&b, b"head"), table(&b, b"hhea")) else { return egui::FontData::from_owned(b) };
    let upem = f32::from(rd(&b, head + 18));
    let (asc, desc, gap) = (f32::from(rd(&b, hhea + 4) as i16), f32::from(rd(&b, hhea + 6) as i16), f32::from(rd(&b, hhea + 8) as i16));
    let new_asc = (asc * upem / (asc - desc)).round();
    let (new_desc, new_gap) = (new_asc - upem, gap + (asc - desc) - upem);
    for (o, v) in [(4, new_asc), (6, new_desc), (8, new_gap)] {
        b[hhea + o..hhea + o + 2].copy_from_slice(&(v as i16).to_be_bytes());
    }
    let mut d = egui::FontData::from_owned(b);
    d.tweak.y_offset_factor = (asc - new_asc) / upem;
    d
}

/// The window's fonts: Tahoma, sized as GDI's em (`em_sized`), then DejaVu
/// Sans for the glyphs Tahoma lacks; the bold face as the family "bold".
fn app_fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("tahoma".to_owned(), em_sized(TAHOMA.to_vec()));
    // Its copied regular Cyrillic outlines made bold (`fontfix`).
    fonts.font_data.insert("tahoma-bold".to_owned(), em_sized(fontfix::embolden_copied_cyrillic(TAHOMA_BOLD, TAHOMA)));
    fonts.font_data.insert("dejavu".to_owned(), egui::FontData::from_static(DEJAVU));
    let order = ["tahoma".to_owned(), "dejavu".to_owned()];
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        for (k, name) in order.iter().enumerate() {
            list.insert(k, name.clone());
        }
    }
    let bold: Vec<String> = std::iter::once("tahoma-bold".to_owned()).chain(order.iter().cloned()).collect();
    fonts.families.insert(egui::FontFamily::Name("bold".into()), bold);
    fonts
}

impl App {
    /// The tabbed-MDI strip (`EnableMDITabbedGroups` `0x47293b` with
    /// OneNote tabs, auto colour, the document menu, the close button on the
    /// active tab, tab swap left on): tabs coloured per document, the ▾
    /// document list; a tab dragged over another swaps with it
    /// (`0x100690b4`), off the strip it goes to a group (`crate::mdi`).
    /// Group `g`'s tabs, its front document taken as `self.active`; the
    /// label bold only on `bold`, the active document (`0x1013b543`).
    fn doc_tabs(&mut self, ui: &mut egui::Ui, g: usize, bold: Option<usize>) {
        let mut activate = None;
        let mut close = None;
        let mut dragged = None;
        let mut released = None;
        let mut tabs: Vec<(usize, egui::Rect)> = Vec::new();
        let strip = ui.max_rect();
        if let Some(s) = self.mdi_strips.get_mut(g) {
            *s = strip;
        }
        let members: Vec<usize> = (0..self.docs.len()).filter(|&i| self.docs[i].mdi_group == g).collect();
        ui.horizontal(|ui| {
            // Nothing in the row taller than the tabs (the ▾ button included).
            ui.spacing_mut().interact_size.y = 19.0;
            ui.spacing_mut().button_padding.y = 0.0;
            ui.add_space(6.0);
            for (k, &i) in members.iter().enumerate() {
                if self.docs[i].tab_color.is_none() {
                    let prev = k.checked_sub(1).and_then(|j| self.docs[members[j]].tab_color);
                    self.docs[i].tab_color = Some(self.tab_auto.assign(k, prev));
                }
                let active = self.active == Some(i);
                let title = self.docs[i].title();
                // MFC's tab font, 11 px (VM: «WIN_1999.cfg» 68 px); the
                // active document's in the bold face (`0x1013b543`).
                let font = if bold == Some(i) { ui.fonts(|f| crate::lanes::bold_font(f, 11.0)).unwrap_or(egui::FontId::proportional(11.0)) } else { egui::FontId::proportional(11.0) };
                let text_w = ui.fonts(|f| f.layout_no_wrap(title.clone(), font.clone(), egui::Color32::BLACK).size().x);
                let w = text_w + 34.0 + if active { 20.0 } else { 0.0 };
                // An id per document: the drag follows the tab when it moves.
                // 19 px, over the row's bottom line (VM).
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 19.0), egui::Sense::hover());
                let resp = ui.interact(rect, egui::Id::new(("fv_doc_tab", self.docs[i].serial)), egui::Sense::click_and_drag());
                tabs.push((i, rect));
                let (r, g, b) = app::TAB_COLORS[self.docs[i].tab_color.unwrap_or(0)];
                let base = egui::Color32::from_rgb(r, g, b);
                let top = base.lerp_to_gamma(egui::Color32::WHITE, if active { 0.45 } else { 0.25 });
                // Slanted left edge, square right edge (OneNote style).
                let pts = [rect.left_bottom(), rect.left_top() + egui::vec2(16.0, 0.0), rect.right_top(), rect.right_bottom()];
                let mut mesh = egui::Mesh::default();
                for (p, c) in pts.iter().zip([base, top, top, base]) {
                    mesh.colored_vertex(*p, if active { c } else { c.gamma_multiply(0.85) });
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(mesh));
                ui.painter().add(egui::Shape::line(pts.to_vec(), egui::Stroke::new(1.0f32, egui::Color32::from_gray(140))));
                let text_pos = egui::pos2(rect.left() + 20.0, rect.center().y);
                ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, &title, font, egui::Color32::BLACK);
                if active {
                    let xr = egui::Rect::from_center_size(egui::pos2(rect.right() - 12.0, rect.center().y), egui::vec2(14.0, 14.0));
                    let xresp = ui.interact(xr, ui.id().with(("tab_close", i)), egui::Sense::click());
                    // Two 2 px diagonals in the pane buttons' blue (VM: (86,
                    // 125, 177)). Hot: a 14 px box, a (255, 189, 105) frame
                    // round a (255, 242, 197) → (255, 222, 129) face, the
                    // cross as before; MFC's plain «Close» tip (mfc120rus).
                    let col = egui::Color32::from_rgb(86, 125, 177);
                    if xresp.hovered() {
                        let mut mesh = egui::Mesh::default();
                        let face = xr.shrink(1.0);
                        let (t, b) = (egui::Color32::from_rgb(255, 242, 197), egui::Color32::from_rgb(255, 222, 129));
                        for (p, c) in [(face.left_top(), t), (face.right_top(), t), (face.right_bottom(), b), (face.left_bottom(), b)] {
                            mesh.colored_vertex(p, c);
                        }
                        mesh.add_triangle(0, 1, 2);
                        mesh.add_triangle(0, 2, 3);
                        ui.painter().add(egui::Shape::mesh(mesh));
                        ui.painter().rect_stroke(xr.shrink(0.5), 0.0, egui::Stroke::new(1.0f32, egui::Color32::from_rgb(255, 189, 105)));
                    }
                    crate::tips::pointer_tip(ui, &xresp, crate::tips::TipImage::None, "", "Закрыть", false);
                    let c = xr.center();
                    let st = egui::Stroke::new(2.0f32, col);
                    ui.painter().line_segment([c + egui::vec2(-3.5, -3.0), c + egui::vec2(3.5, 3.0)], st);
                    ui.painter().line_segment([c + egui::vec2(3.5, -3.0), c + egui::vec2(-3.5, 3.0)], st);
                    if xresp.clicked() {
                        close = Some(i);
                    }
                }
                // A press (or a drag) brings the tab to the front; so does a
                // right-click — the frame's OnContextMenu activates the tab
                // under the point, and OnShowMDITabContextMenu (not
                // overridden, `0x100bc637`) shows no menu for these tabs.
                if (resp.clicked() || resp.drag_started() || resp.secondary_clicked()) && close.is_none() {
                    activate = Some(i);
                }
                if resp.dragged() {
                    dragged = Some(i);
                }
                if resp.drag_stopped() {
                    released = Some(i);
                }
                ui.add_space(-6.0);
            }
            // The document menu's button: a bare (86, 125, 177) triangle,
            // 9 px across its top row 8 px down the strip, 5 rows to its
            // point, its right end 7 px in from the strip's (VM).
            let x0 = strip.right() - 15.0;
            let y0 = strip.top() + 8.0;
            let hit = egui::Rect::from_min_max(egui::pos2(x0 - 3.0, strip.top()), egui::pos2(x0 + 12.0, strip.bottom()));
            let tri = egui::Color32::from_rgb(86, 125, 177);
            for k in 0..5 {
                let row = egui::Rect::from_min_max(egui::pos2(x0 + k as f32, y0 + k as f32), egui::pos2(x0 + 9.0 - k as f32, y0 + k as f32 + 1.0));
                ui.painter().rect_filled(row, 0.0, tri);
            }
            // The documents sorted by title, each with a small window
            // image, none marked; no «Окна...» (VM: MFC adds it to a
            // «Window» menu only, which this ribbon has not). An MFC popup
            // at the button's left under it; the button lit (15 px, (252,
            // 151, 61) → (254, 181, 91)) while it is open (VM).
            let list_id = ui.id().with("fv_doc_list");
            let btn = ui.interact(hit, list_id.with("btn"), egui::Sense::click());
            let mut open: Option<u64> = ui.data(|d| d.get_temp(list_id));
            if btn.clicked() {
                open = if open.is_some() { None } else { Some(ui.ctx().cumulative_pass_nr()) };
            }
            let lit = egui::Rect::from_min_size(egui::pos2(x0 - 3.0, strip.top() + 1.0), egui::vec2(15.0, 15.0));
            if let Some(serial) = open {
                crate::lanes::gradient(ui.painter(), lit, egui::Color32::from_rgb(252, 151, 61), egui::Color32::from_rgb(254, 181, 91));
                for k in 0..5 {
                    let row = egui::Rect::from_min_max(egui::pos2(x0 + k as f32, y0 + k as f32), egui::pos2(x0 + 9.0 - k as f32, y0 + k as f32 + 1.0));
                    ui.painter().rect_filled(row, 0.0, tri);
                }
                let mut order = members.clone();
                order.sort_by_key(|&i| self.docs[i].title().to_lowercase());
                let window = |p: &egui::Painter, r: egui::Rect, _c: egui::Color32| {
                    let w = egui::Rect::from_center_size(r.center(), egui::vec2(12.0, 10.0));
                    p.rect_stroke(w, 0.0, egui::Stroke::new(1.0f32, egui::Color32::from_gray(110)));
                    p.rect_filled(egui::Rect::from_min_max(w.left_top(), egui::pos2(w.left() + 3.0, w.bottom())), 0.0, egui::Color32::from_rgb(70, 130, 110));
                };
                let area = egui::Area::new(list_id.with(serial)).kind(egui::UiKind::Menu).order(egui::Order::Foreground).fixed_pos(lit.left_bottom()).constrain(true).show(ui.ctx(), |ui| {
                    crate::popmenu::mfc_frame(ui, |ui| {
                        for i in order {
                            if crate::popmenu::mark_item_icon(ui, &self.docs[i].title(), None, true, None, false, Some(&window)).clicked() {
                                activate = Some(i);
                            }
                        }
                    })
                });
                let outside = ui.input(|inp| inp.pointer.any_pressed() && inp.pointer.interact_pos().is_some_and(|q| !area.response.rect.contains(q) && !hit.contains(q)));
                if activate.is_some() || outside || ui.input(|inp| inp.key_pressed(egui::Key::Escape)) {
                    open = None;
                }
            }
            ui.data_mut(|d| match open {
                Some(o) => d.insert_temp(list_id, o),
                None => d.remove::<u64>(list_id),
            });
        });
        for &(i, r) in &tabs {
            if let Some(t) = self.doc_tab_rects.get_mut(i) {
                *t = r;
            }
        }
        if let Some(i) = activate {
            self.dispatch(app::Command::ActivateDoc(i));
        }
        let at = ui.ctx().input(|inp| inp.pointer.latest_pos());
        // Past the other tab's middle, so tabs of unequal widths do not swap
        // back and forth.
        if let (Some(i), Some(at)) = (dragged, at) {
            let own = tabs.iter().find(|t| t.0 == i).map_or(0.0, |t| t.1.center().x);
            if let Some(&(j, _)) = tabs.iter().find(|(j, r)| *j != i && r.contains(at) && (r.center().x > own) == (at.x > r.center().x)) {
                self.docs.swap(i, j);
                self.active = Some(j);
            }
            // Off the strip: the outline of where it would go (4 px).
            if !strip.contains(at) {
                if let Some((_, r)) = self.drop_preview(i, at) {
                    crate::views::halftone(ui.ctx(), r, Some(4.0));
                }
            }
        }
        // Let go off the strip: taken up before the next frame's layout.
        if let (Some(i), Some(at)) = (released, at) {
            if !strip.contains(at) {
                self.mdi_drop = Some((self.docs[i].serial, at));
            }
        }
        if let Some(i) = close {
            self.dispatch(app::Command::CloseDoc(i));
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.skin.palette();
        // The panes keep their last texts with no document (L9.147). The
        // preview's page text goes to the message bar — the ribbon status
        // bar's window title, never shown (INFERRED).
        if let Some(s) = self.cursor_status() {
            self.status_panes = s;
        }
        let s = self.status_panes.clone();
        // As the VM shows it: a 1 px line on top of a light band, six panes
        // with no separators — 27 / 40 × "_" templates (`0x4729b0`) in the
        // 9 pt UI font, 153 / 218 px with the text 9 px in, left-aligned —
        // and the extended element, 155 px on a darker band behind a light
        // edge, its text centred clear of the size grip.
        let r = ui.max_rect();
        let painter = ui.painter();
        let band = |c0: egui::Color32, c1: egui::Color32, rr: egui::Rect| {
            let mut mesh = egui::Mesh::default();
            let mid = rr.top() + rr.height() * 0.45;
            let quads = [(rr.top(), mid, c0, c1), (mid, rr.bottom(), c1, c0.lerp_to_gamma(c1, 0.5))];
            for (y0, y1, a, b) in quads {
                let k = mesh.vertices.len() as u32;
                for (x, y, c) in [(rr.left(), y0, a), (rr.right(), y0, a), (rr.right(), y1, b), (rr.left(), y1, b)] {
                    mesh.colored_vertex(egui::pos2(x, y), c);
                }
                mesh.add_triangle(k, k + 1, k + 2);
                mesh.add_triangle(k, k + 2, k + 3);
            }
            mesh
        };
        // A bitmap of rows, uniform along x: row i's colour at r.top() + i.
        let rows = |painter: &egui::Painter, r: egui::Rect, rows: &[u32; 24]| {
            for (i, c) in rows.iter().enumerate() {
                let col = egui::Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, *c as u8);
                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(r.left(), r.top() + i as f32), egui::vec2(r.width(), 1.0)), 0.0, col);
            }
        };
        match p.status_flat {
            // The older looks: flat, a top line or none (VM).
            Some(line) => {
                if let Some(c) = line {
                    painter.hline(r.x_range(), r.top() + 0.5, egui::Stroke::new(1.0f32, c));
                }
            }
            None => match p.bands {
                // The Office 2007 styles: the bar's bitmap row by row (VM).
                Some(b) => rows(painter, r, &b.status),
                None => {
                    painter.add(band(p.status_bg.lerp_to_gamma(egui::Color32::WHITE, 0.45), p.status_bg, r));
                    painter.hline(r.x_range(), r.top() + 0.5, egui::Stroke::new(1.0f32, p.accent.lerp_to_gamma(p.status_bg, 0.3)));
                }
            },
        }
        let font = egui::FontId::proportional(12.0);
        let mut x = r.left();
        // A hidden pane leaves no gap: the rest move left (VM).
        for (i, text) in s.iter().enumerate().filter(|(i, _)| !self.status_hidden[*i]) {
            let w = if i == 2 || i == 5 { 218.0 } else { 153.0 };
            let clip = egui::Rect::from_min_max(egui::pos2(x, r.top()), egui::pos2(x + w - 2.0, r.bottom()));
            painter.with_clip_rect(clip).text(egui::pos2(x + 9.0, r.center().y), egui::Align2::LEFT_CENTER, text, font.clone(), p.status_text);
            x += w;
        }
        // The last pane: 135 px at the right (VM), its own rows and a light
        // and a dark column at its left in the Office 2007 styles.
        let ext = egui::Rect::from_min_max(egui::pos2(r.right() - if p.bands.is_some() { 135.0 } else { 155.0 }, r.top() + 1.0), r.right_bottom());
        let ext_shown = !self.status_hidden[6];
        if !ext_shown {
            // Hidden from the menu: the bar's own face there.
        } else if let (None, Some(b)) = (p.status_flat, p.bands) {
            let pane = egui::Rect::from_min_max(egui::pos2(ext.left() - 1.0, r.top()), r.right_bottom());
            rows(painter, pane, &b.status_pane);
            painter.vline(ext.left() - 0.5, ext.y_range(), egui::Stroke::new(1.0f32, b.pane_edge[0]));
            painter.vline(ext.left() + 0.5, ext.y_range(), egui::Stroke::new(1.0f32, b.pane_edge[1]));
        } else if p.status_flat.is_none() {
            let dark = p.status_bg.lerp_to_gamma(p.accent, 0.35);
            painter.add(band(dark.lerp_to_gamma(egui::Color32::WHITE, 0.3), dark, ext));
            painter.vline(ext.left() + 0.5, ext.y_range(), egui::Stroke::new(1.0f32, p.status_bg.lerp_to_gamma(egui::Color32::WHITE, 0.6)));
        }
        if ext_shown {
            painter.text(egui::pos2((ext.left() + ext.right() - 12.0) / 2.0, ext.center().y), egui::Align2::CENTER_CENTER, "НТЦ Механотроника", font, p.status_text);
        }
        // CMFCRibbonStatusBar's right-click: its configuration menu.
        let bar = ui.allocate_rect(r, egui::Sense::click());
        if bar.secondary_clicked() {
            if let Some(at) = bar.interact_pointer_pos() {
                self.status_menu = Some((at, ui.ctx().cumulative_pass_nr()));
            }
        }
        self.status_config_menu(ui.ctx(), &s);
        self.status_toast(ui.ctx());
        self.resize_edges(ui.ctx());
    }

    /// «Конфигурация строки состояния» (mfc120rus 17087; «Status Bar
    /// Configuration» under the VM's English Windows, as MFC's other menus):
    /// each element with its current text right-aligned, a tick while
    /// shown; a click toggles it and the menu stays; Esc or a press
    /// outside closes it. Opened upward from the point when it would run
    /// past the bottom.
    fn status_config_menu(&mut self, ctx: &egui::Context, texts: &[String; 6]) {
        let Some((at, opened)) = self.status_menu else { return };
        let id = egui::Id::new("fv_status_menu");
        let h = ctx.memory(|m| m.area_rect(id)).map_or(0.0, |r| r.height());
        let pos = if at.y + h > ctx.screen_rect().bottom() { egui::pos2(at.x, at.y - h) } else { at };
        let mut toggle = None;
        let area = egui::Area::new(id).kind(egui::UiKind::Menu).order(egui::Order::Foreground).fixed_pos(pos).constrain(true).show(ctx, |ui| {
            crate::popmenu::mfc_frame_with(ui, true, false, |ui| {
                crate::popmenu::title_band(ui, "Конфигурация строки состояния");
                for (i, t) in texts.iter().map(String::as_str).chain(Some("НТЦ Механотроника")).enumerate() {
                    if i == 6 {
                        crate::popmenu::panel_separator(ui);
                    }
                    if crate::popmenu::mark_item_keys(ui, "", Some(t), true, (!self.status_hidden[i]).then_some("✓"), false).clicked() {
                        toggle = Some(i);
                    }
                }
            })
        });
        if let Some(i) = toggle {
            self.status_hidden[i] = !self.status_hidden[i];
        }
        let fresh = ctx.cumulative_pass_nr() == opened;
        let away = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|q| !area.response.rect.contains(q)));
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || (away && !fresh) {
            self.status_menu = None;
        }
    }

    /// One point one device pixel at the window's monitor's scale
    /// ([`device_zoom`], from the next frame when the scale changes), and
    /// [`MIN_VISIBLE`] sent again then — in this frame's points, as the
    /// runner converts it. The saved placement too, once: the runner
    /// applied it at the scale of the monitor the window landed on, not
    /// necessarily the primary one it was sized for.
    fn device_pixels(&mut self, ctx: &egui::Context) {
        let Some(native) = ctx.input(|i| i.viewport().native_pixels_per_point) else { return };
        if DEVICE_PIXELS {
            let zoom = device_zoom(native);
            if ctx.zoom_factor() != zoom {
                ctx.set_zoom_factor(zoom);
            }
            if let Some(r) = self.placement_px.take() {
                let ppp = ctx.pixels_per_point();
                // Moved first: a move onto a monitor of another dpi resizes
                // the window by Windows' suggestion.
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition((r.min.to_vec2() / ppp).to_pos2()));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(r.size() / ppp));
            }
        }
        if self.min_size_ppp != native {
            self.min_size_ppp = native;
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::Vec2::from(MIN_VISIBLE) / ctx.pixels_per_point()));
        }
    }

    /// A frameless window's edges resize it (the DWM frame's sizing border):
    /// 5 px, corners 12 px along each edge; not while maximised.
    fn resize_edges(&mut self, ctx: &egui::Context) {
        use egui::viewport::ResizeDirection as D;
        if !self.own_caption || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let screen = ctx.screen_rect();
        let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return };
        let (b, c) = (5.0, 12.0);
        let (l, r, t, bt) = (p.x - screen.left() < b, screen.right() - p.x < b, p.y - screen.top() < b, screen.bottom() - p.y < b);
        let (lc, rc, tc, bc) = (p.x - screen.left() < c, screen.right() - p.x < c, p.y - screen.top() < c, screen.bottom() - p.y < c);
        let dir = match () {
            _ if (t && lc) || (l && tc) => Some(D::NorthWest),
            _ if (t && rc) || (r && tc) => Some(D::NorthEast),
            _ if (bt && lc) || (l && bc) => Some(D::SouthWest),
            _ if (bt && rc) || (r && bc) => Some(D::SouthEast),
            _ if t => Some(D::North),
            _ if bt => Some(D::South),
            _ if l => Some(D::West),
            _ if r => Some(D::East),
            _ => None,
        };
        let Some(dir) = dir else { return };
        ctx.set_cursor_icon(match dir {
            D::North | D::South => egui::CursorIcon::ResizeVertical,
            D::East | D::West => egui::CursorIcon::ResizeHorizontal,
            D::NorthWest | D::SouthEast => egui::CursorIcon::ResizeNwSe,
            D::NorthEast | D::SouthWest => egui::CursorIcon::ResizeNeSw,
        });
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }

    /// The original's status bar has no free-text pane (`0x4729b0`); our own
    /// messages appear as a short-lived toast above the bottom-right corner.
    fn status_toast(&mut self, ctx: &egui::Context) {
        const SHOW_S: f64 = 4.0;
        let now = ctx.input(|i| i.time);
        if self.toast.0 != self.status {
            self.toast = (self.status.clone(), now);
        }
        let age = now - self.toast.1;
        if self.toast.0.is_empty() || !(0.0..SHOW_S).contains(&age) {
            return;
        }
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("fv_status_toast"))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(screen.right_bottom() - egui::vec2(12.0, 34.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(&self.toast.0).small());
                });
            });
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(SHOW_S - age));
    }
}

/// The original's command line.
#[derive(Debug, Default, PartialEq)]
struct StartupArgs {
    dst: Option<std::path::PathBuf>,
    find: bool,
    files: Vec<std::path::PathBuf>,
}

/// `InitInstance` (`0x4483f0`): `-Dst <file>` is looked for, ignoring case,
/// only at argv[2] … argv[argc − 2] (`0x44872c`–`0x44876d`), its value kept
/// as given (a later one wins); `-Find` is a case-sensitive " -Find " in the
/// command line after the program name (`0x4487da` — never the first or
/// the last argument); the file is MFC's: the first argument that does not
/// start with '-' or '/', only one.
fn startup_args(args: impl Iterator<Item = String>) -> StartupArgs {
    let args: Vec<String> = args.collect();
    let mut out = StartupArgs::default();
    let inner = 1..args.len().saturating_sub(1);
    for j in inner.clone() {
        if args[j].eq_ignore_ascii_case("-dst") {
            out.dst = Some(args[j + 1].clone().into());
        }
    }
    out.find = inner.into_iter().any(|j| args[j] == "-Find");
    out.files = args.iter().find(|a| !a.starts_with('-') && !a.starts_with('/')).map(|a| vec![a.into()]).unwrap_or_default();
    out
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        device_screen(ctx, raw);
        dialogs::take_dialog_tabs(ctx, raw);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(windows)]
        minsize::install_on(_frame);
        self.frame_ui(ctx);
        self.splash(ctx);
    }
}

impl App {
    /// The splash (`0x4478a0` / `0x4479b0`): bitmap 462 in a topmost window
    /// centred in the work area (VM: 630 × 400 at (965, 496) on a 2560 ×
    /// 1440 screen with a 48 px taskbar), destroyed by its timer after
    /// 2000 ms. The picture is the original's (W6): ours is drawn.
    fn splash(&mut self, ctx: &egui::Context) {
        let Some(until) = self.splash_until else { return };
        if ctx.input(|i| i.time) > until {
            self.splash_until = None;
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        let size = egui::vec2(630.0, 400.0);
        let monitor = ctx.input(|i| i.viewport().monitor_size).unwrap_or(egui::vec2(1920.0, 1080.0));
        // The work area: the screen less the taskbar (48 px on Windows 11,
        // INFERRED for other set-ups).
        let work = egui::vec2(monitor.x, (monitor.y - if cfg!(windows) { 48.0 } else { 0.0 }).max(size.y));
        let at = ((work - size) / 2.0).max(egui::Vec2::ZERO).to_pos2();
        let builder = egui::ViewportBuilder::default()
            .with_title("FastView")
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false)
            .with_taskbar(false)
            .with_inner_size(size)
            .with_position(at);
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("fv_splash"), builder, |ctx, _| {
            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                let r = ui.max_rect();
                let p = ui.painter();
                lanes::gradient(p, r, egui::Color32::from_rgb(44, 92, 142), egui::Color32::from_rgb(60, 118, 172));
                let white = egui::Color32::WHITE;
                p.text(r.left_top() + egui::vec2(28.0, 34.0), egui::Align2::LEFT_TOP, "Fast View 5", egui::FontId::proportional(40.0), white);
                p.text(r.left_top() + egui::vec2(30.0, 96.0), egui::Align2::LEFT_TOP, "Программа для просмотра и анализа осциллограмм\nв формате COMTRADE", egui::FontId::proportional(15.0), white);
                p.text(r.left_top() + egui::vec2(30.0, 160.0), egui::Align2::LEFT_TOP, "FastView-REA — независимая реконструкция", egui::FontId::proportional(12.0), white);
            });
        });
    }
}

impl App {
    /// One frame of the whole window (kept apart from `eframe::App::update`
    /// so headless tests can drive it).
    pub fn frame_ui(&mut self, ctx: &egui::Context) {
        dialogs::track_input(ctx);
        dialogs::route_combo_keys(ctx);
        self.device_pixels(ctx);
        self.run_deferred(ctx);
        // A click or a key acts after the frame that drew it (a command is
        // dispatched once its button is drawn): one more frame shows the
        // state it left (VM: «Анализ» drawn unchecked after its run until
        // the next input).
        if ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::PointerButton { .. } | egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Copy | egui::Event::Paste(_)))) {
            ctx.request_repaint();
        }
        // Window title «Механотроника FastView - <document>» (0x45f7d0) and
        // files dropped on the window (DragAcceptFiles).
        let title = match self.active_doc() {
            Some(d) => format!("Механотроника FastView - {}", d.title()),
            None => app::APP_TITLE.to_string(),
        };
        if self.window_title != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for f in dropped {
            self.load_comtrade_path(&f);
        }
        let handed: Vec<std::path::PathBuf> = self.instance.as_ref().map(|s| s.rx.try_iter().collect()).unwrap_or_default();
        if !handed.is_empty() {
            // WM_COPYDATA `0x475530`: restored if iconic, then to the front.
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        for f in handed {
            self.load_comtrade_path(&f);
        }
        theme::apply(ctx, self.skin);
        self.ppp = ctx.pixels_per_point();
        // Every dialog is modal: while one is up, a backdrop between the
        // panels and the windows takes the pointer, and no shortcuts run.
        self.modal_up = dialogs::window_up(ctx);
        if self.modal_up {
            dialogs::backdrop(ctx, "fv_modal_backdrop", egui::Order::PanelResizeLine);
            self.keytips.exit();
        } else if !self.keytips_input(ctx) && self.print.preview.is_none() {
            // Print preview swaps table 128 for MFC's 30997 (`0x102474d9`).
            self.handle_keys(ctx);
        }

        let pal = self.skin.palette();
        egui::TopBottomPanel::top("top")
            .show_separator_line(false)
            .frame(egui::Frame::none().fill(pal.tab_active))
            .show(ctx, |ui| {
                self.ribbon_ui(ui);
                self.ribbon_wheel(ui, ui.min_rect());
                // The ribbon's bottom row (VM, per Office 2007 style).
                if let Some(b) = pal.bands {
                    let r = ui.max_rect();
                    ui.painter().hline(r.x_range(), r.bottom() - 0.5, egui::Stroke::new(1.0f32, b.under_ribbon[0]));
                }
            });
        // The frame's client area shows the splitter brush between its bars
        // (VM, Office 2007 Blue): under the ribbon two shade rows ((174, 199,
        // 232), (185, 212, 247)) and 4 px of (191, 219, 255).
        use crate::oscgeom::PixelLines;
        let strip = self.skin.splitter();
        egui::TopBottomPanel::top("fv_under_ribbon").exact_height(6.0).show_separator_line(false).frame(egui::Frame::none().fill(strip)).show(ctx, |ui| {
            let r = ui.max_rect();
            if let Some(b) = pal.bands {
                ui.painter().px_hline(r.x_range(), r.top(), egui::Stroke::new(1.0f32, b.under_ribbon[1]));
                ui.painter().px_hline(r.x_range(), r.top() + 1.0, egui::Stroke::new(1.0f32, b.under_ribbon[2]));
            }
        });
        // The ОМП result bar (221), first under the ribbon, then 8 px of the
        // brush (VM).
        self.omp_sync_bar();
        if self.omp_ui.bar.is_some() {
            egui::TopBottomPanel::top("fv_ompbar").exact_height(33.0).show_separator_line(false).frame(egui::Frame::none().fill(strip)).show(ctx, |ui| self.omp_banner(ui));
            egui::TopBottomPanel::top("fv_under_ompbar").exact_height(8.0).show_separator_line(false).frame(egui::Frame::none().fill(strip)).show(ctx, |_| {});
        }
        // A caption bar's face in the Office 2007 looks (VM): a box from 4 px
        // in to 21 px short of the window's right (its border on W − 22), the brush round it, a
        // gradient from white to (213, 228, 242) inside (157, 189, 216)
        // lines.
        let bar_face = |ui: &egui::Ui| {
            if let (None, Some(b)) = (pal.info_bar, pal.bands) {
                let m = ui.max_rect();
                ui.painter().rect_filled(m, 0.0, strip);
                let r = egui::Rect::from_min_max(m.min + egui::vec2(4.0, 0.0), egui::pos2(m.right() - 21.0, m.bottom()));
                crate::lanes::gradient(ui.painter(), r.shrink(1.0), egui::Color32::WHITE, b.bar_bottom);
                ui.painter().px_rect_stroke(r, 0.0, egui::Stroke::new(1.0f32, b.bar_line));
            }
        };
        // The merge bar (CMergeBar), shown after a paste — docked above the
        // parameters bar, 62 px with its lines, then 8 px of the brush (VM).
        if self.merge.is_some() {
            egui::TopBottomPanel::top("mergebar")
                .exact_height(62.0)
                .show_separator_line(false)
                .frame(egui::Frame::none().fill(pal.info_bar.unwrap_or(pal.group_bg)))
                .show(ctx, |ui| {
                    bar_face(ui);
                    self.merge_bar_ui(ui)
                });
            egui::TopBottomPanel::top("fv_under_mergebar").exact_height(8.0).show_separator_line(false).frame(egui::Frame::none().fill(strip)).show(ctx, |_| {});
        }
        // 142: the parameters bar, 67 px — in the Office 2007 looks a
        // gradient from white to (213, 228, 242) between (157, 189, 216)
        // lines —, then 4 px of the brush.
        if self.dlg.param_bar {
            egui::TopBottomPanel::top("parambar")
                .exact_height(67.0)
                .show_separator_line(false)
                .frame(egui::Frame::none().fill(pal.info_bar.unwrap_or(pal.group_bg)))
                .show(ctx, |ui| {
                    bar_face(ui);
                    self.param_bar_ui(ui)
                });
            egui::TopBottomPanel::top("fv_under_parambar").exact_height(4.0).show_separator_line(false).frame(egui::Frame::none().fill(strip)).show(ctx, |_| {});
        }


        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .show_separator_line(false)
            .frame(egui::Frame::none().fill(pal.status_bg))
            .show(ctx, |ui| self.status_bar(ui));


        // The print preview takes the document view's place in its window
        // (VM: the panes and strips stay).
        self.diagram_panes(ctx);
        self.render_tab(ctx);
        self.track_copy_focus(ctx);
        self.dialogs_ui(ctx);
        self.print_windows(ctx);
        // The record settings follow the active document; leaving asks
        // about modified documents.
        self.track_settings();
        self.handle_exit_request(ctx);

        // Image export (Печать): request a screenshot while pending, save it when
        // the frame is delivered.
        if self.want_screenshot.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let (Some(image), Some(path)) = (shot, self.want_screenshot.clone()) {
            self.want_screenshot = None;
            let [w, h] = image.size;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for px in &image.pixels {
                rgba.extend_from_slice(&px.to_array());
            }
            let png = util::png_rgba(w as u32, h as u32, &rgba);
            self.status = match std::fs::write(&path, png) {
                Ok(()) => format!("Изображение сохранено: {}", path.display()),
                Err(e) => format!("Ошибка сохранения изображения: {e}"),
            };
        }
    }
}

/// A frame's input as passes of at most one key press or mouse-button
/// change each, a release in a pass of its own (the software runner, whose
/// frames are slow): a window gets
/// each WM_KEYDOWN / WM_LBUTTONDOWN / …UP on its own, so quick presses each
/// act and a button change is seen where the pointer was then, as in the
/// original. egui's per-frame `key_pressed` would see quick presses as one,
/// and a press followed by the first moves of a drag in one frame lands
/// past a 5 px splitter line (egui tests the latest position; VM). A pass
/// with a key press carries that press's modifiers; dropped files and
/// viewport events (a close request) go with the first.
#[cfg_attr(not(windows), allow(dead_code))]
fn one_press_per_pass(mut raw: egui::RawInput) -> Vec<egui::RawInput> {
    let events = std::mem::take(&mut raw.events);
    let mut passes: Vec<(Vec<egui::Event>, Option<egui::Modifiers>)> = vec![(Vec::new(), None)];
    // A pass ends after its key press or button change.
    let mut closed = false;
    for e in events {
        let (boundary, mods) = match &e {
            egui::Event::Key { pressed: true, modifiers, .. } => (true, Some(*modifiers)),
            egui::Event::PointerButton { .. } => (true, None),
            _ => (false, None),
        };
        // A key's text stays with its press; a release begins a pass of
        // its own, so the moves before it are seen with the button down
        // (egui ends a drag at the start of its release frame: a splitter
        // stopped short of where it was let go, VM).
        let release = matches!(e, egui::Event::PointerButton { pressed: false, .. });
        let open = passes.last().is_some_and(|p| !p.0.is_empty());
        if (closed && !matches!(e, egui::Event::Text(_))) || (release && open) {
            passes.push((Vec::new(), None));
            closed = false;
        }
        if let Some(p) = passes.last_mut() {
            if mods.is_some() {
                p.1 = mods;
            }
            p.0.push(e);
        }
        closed |= boundary;
    }
    let n = passes.len();
    passes
        .into_iter()
        .enumerate()
        .map(|(i, (events, mods))| {
            let mut r = raw.clone();
            r.events = events;
            if let Some(m) = mods.filter(|_| n > 1) {
                r.modifiers = m;
            }
            if i > 0 {
                r.dropped_files.clear();
                r.viewports.values_mut().for_each(|v| v.events.clear());
            }
            r
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_key_presses_get_a_pass_each() {
        let key = |k, pressed, m| egui::Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers: m };
        let (none, ctrl) = (egui::Modifiers::NONE, egui::Modifiers::CTRL);
        let raw = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(egui::pos2(1.0, 1.0)),
                key(egui::Key::ArrowRight, true, none),
                key(egui::Key::ArrowRight, false, none),
                key(egui::Key::ArrowRight, true, none),
                key(egui::Key::C, true, ctrl),
                egui::Event::Text("x".into()),
            ],
            modifiers: none,
            ..Default::default()
        };
        let passes = one_press_per_pass(raw);
        assert_eq!(passes.iter().map(|p| p.events.len()).collect::<Vec<_>>(), [2, 2, 2], "a key's text with it");
        assert_eq!(passes[2].modifiers, ctrl, "the press's own modifiers");
        // A press, then the drag's first moves: the press alone, where it
        // was (VM: a splitter's 5 px line).
        let at = |y| egui::pos2(10.0, y);
        let btn = |pressed| egui::Event::PointerButton { pos: at(446.0), button: egui::PointerButton::Primary, pressed, modifiers: none };
        let drag = one_press_per_pass(egui::RawInput { events: vec![egui::Event::PointerMoved(at(446.0)), btn(true), egui::Event::PointerMoved(at(461.0)), egui::Event::PointerMoved(at(477.0))], ..Default::default() });
        assert_eq!(drag.iter().map(|p| p.events.len()).collect::<Vec<_>>(), [2, 2]);
        // A quick click: press and release apart.
        assert_eq!(one_press_per_pass(egui::RawInput { events: vec![btn(true), btn(false)], ..Default::default() }).len(), 2);
        // The drag's last moves before the release, with the button down.
        let end = one_press_per_pass(egui::RawInput { events: vec![egui::Event::PointerMoved(at(990.0)), egui::Event::PointerMoved(at(1000.0)), btn(false)], ..Default::default() });
        assert_eq!(end.iter().map(|p| p.events.len()).collect::<Vec<_>>(), [2, 1]);
        // One press or none: a single pass as it came.
        let one = one_press_per_pass(egui::RawInput { events: vec![key(egui::Key::A, true, ctrl)], modifiers: none, ..Default::default() });
        assert_eq!((one.len(), one[0].modifiers), (1, none));
    }

    #[test]
    fn device_zoom_makes_one_pixel_per_point() {
        // Windows' steps exactly, the reporter's 154 dpi among them; every
        // custom scale at 1 or just above (151 %: 145 dpi has no exact one).
        for dpi in [96, 120, 144, 154, 168, 192, 216, 240, 288, 336, 384, 480] {
            let native = dpi as f32 / 96.0;
            assert_eq!(device_zoom(native) * native, 1.0, "{dpi} dpi");
        }
        for dpi in 96..=480 {
            let native = dpi as f32 / 96.0;
            let p = device_zoom(native) * native;
            assert!((1.0..=1.0 + 2.0 * f32::EPSILON).contains(&p), "{dpi} dpi: {p}");
            // A whole coordinate stays in its pixel, a centre at its centre.
            assert_eq!(((500.0 * p).floor(), (500.5 * p).round()), (500.0, 501.0), "{dpi} dpi");
        }
    }

    #[test]
    fn command_line_switches() {
        let args = |v: &[&str]| startup_args(v.iter().map(|s| s.to_string()));
        assert_eq!(args(&["x.sto", "-dst", "d.dst", "-Find", "y.cfg"]), StartupArgs { dst: Some("d.dst".into()), find: true, files: vec!["x.sto".into()] });
        // `-Dst` first: not looked for there, its value becomes the file.
        assert_eq!(args(&["-Dst", "d.dst", "x.sto"]), StartupArgs { dst: None, find: false, files: vec!["d.dst".into()] });
        // `-Find` is case-sensitive, never the first or the last argument.
        assert!(!args(&["x.cfg", "-FIND", "y"]).find && !args(&["x.cfg", "-Find"]).find);
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    use eframe::egui::{Pos2, Rect, Vec2};

    /// Runs whole-window frames at `size`, one per event batch.
    fn frames(app: &mut App, ctx: &egui::Context, size: Vec2, batches: Vec<Vec<egui::Event>>) {
        frames_held(app, ctx, size, batches, egui::Modifiers::NONE);
    }

    /// [`frames`] with `modifiers` held throughout.
    /// Returns the last frame's cursor.
    fn frames_held(app: &mut App, ctx: &egui::Context, size: Vec2, batches: Vec<Vec<egui::Event>>, modifiers: egui::Modifiers) -> egui::CursorIcon {
        let mut cursor = egui::CursorIcon::Default;
        for events in batches {
            let t = ctx.input(|i| i.time) + 0.05;
            let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(t), events, modifiers, ..Default::default() };
            dialogs::take_dialog_tabs(ctx, &mut raw);
            cursor = ctx.run(raw, |ctx| app.frame_ui(ctx)).platform_output.cursor_icon;
        }
        cursor
    }

    /// At `size`: the pointer rests at the centre, then drags from a third
    /// to two thirds of the window with each button.
    fn exercise(app: &mut App, ctx: &egui::Context, size: Vec2) {
        let (a, b) = ((size / 3.0).to_pos2(), (size * 2.0 / 3.0).to_pos2());
        let mut batches = vec![vec![egui::Event::PointerMoved((size / 2.0).to_pos2())], vec![]];
        for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
            let ev = |pos, pressed| egui::Event::PointerButton { pos, button, pressed, modifiers: Default::default() };
            batches.push(vec![egui::Event::PointerMoved(a), ev(a, true)]);
            batches.push(vec![egui::Event::PointerMoved(b)]);
            batches.push(vec![ev(b, false)]);
            batches.push(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        }
        frames(app, ctx, size, batches);
    }

    fn sweep(app: &mut App) {
        let ctx = egui::Context::default();
        frames(app, &ctx, Vec2::new(1320.0, 860.0), vec![vec![], vec![]]);
        for &w in &[866.0, 400.0, 120.0, 20.0, 1.0] {
            for &h in &[500.0, 200.0, 60.0, 1.0] {
                exercise(app, &ctx, Vec2::new(w, h));
            }
        }
        exercise(app, &ctx, Vec2::new(1320.0, 860.0));
    }

    fn demo(cmds: &[app::Command]) -> App {
        let mut app = App::new();
        // `FV_WINDOW_CAPTION=1`: the frameless window's caption row.
        app.own_caption = std::env::var_os("FV_WINDOW_CAPTION").is_some();
        app.dispatch(app::Command::Demo);
        for c in cmds {
            app.dispatch(c.clone());
        }
        app
    }

    /// #5, #6: on a monitor at 125 % or 150 % the window paints in device
    /// pixels, as the original (system-DPI aware) does — pixel for pixel
    /// the picture at 100 %: text rasterised at its pixel size on whole
    /// pixels, 1 px lines one pixel wide, waveforms without gaps. Both from
    /// the start (`make_app`'s zoom) and after the window's monitor changes
    /// scale (the next frame on).
    #[test]
    fn fractional_display_scales_paint_device_pixels() {
        let px = Vec2::new(1100.0, 700.0);
        let fonts = app_fonts();
        let shot = |native: f32, setup: &dyn Fn(&egui::Context)| {
            let mut app = demo(&[]);
            let mut ppp = 0.0;
            let c = raster::render_native(px, native, setup, device_screen, Some(&fonts), vec![vec![]; 5], |ctx| {
                app.frame_ui(ctx);
                ppp = ctx.pixels_per_point();
            });
            (c, ppp)
        };
        let (base, _) = shot(1.0, &|ctx| device_options(ctx, Some(1.0)));
        let differ = |c: &raster::Canvas| c.px.iter().zip(&base.px).filter(|(a, b)| a != b).count();
        for native in [1.25, 1.5, 154.0 / 96.0, 145.0 / 96.0] {
            let (c, ppp) = shot(native, &|ctx| device_options(ctx, Some(native)));
            assert!((1.0..1.000001).contains(&ppp), "{native}: {ppp}");
            if native * 96.0 == 145.0 {
                // A custom 151 %: pixels per point 1.0000001 (no exact
                // zoom), a few edge pixels off by a shade (< 0.2 %).
                assert!(differ(&c) < c.px.len() / 500, "{native}: {} pixels differ from 100 %", differ(&c));
            } else {
                assert!(c.px == base.px, "{native}: {} pixels differ from 100 %", differ(&c));
            }
            // Started at another scale: the layout kept from the first
            // frame (as after a resize), the pixels per point 1 again.
            let (_, ppp) = shot(native, &|ctx| device_options(ctx, Some(1.0)));
            assert!((1.0..1.000001).contains(&ppp), "{native} after a change: {ppp}");
        }
    }

    /// An improvement over the original (`docs/IMPROVEMENTS.md`): a window
    /// on a monitor whose scale is not the system's (the primary one at
    /// sign-in) paints 1:1 in that monitor's device pixels from its first
    /// frame, where Windows bitmap-stretches the system-DPI-aware original
    /// by the ratio of the two scales.
    #[test]
    fn a_monitor_at_another_scale_than_the_systems_paints_one_to_one() {
        let px = Vec2::new(1100.0, 700.0);
        let fonts = app_fonts();
        let shot = |system: f32, monitor: f32| {
            let mut app = demo(&[]);
            let mut ppp = 0.0;
            // As the runners start: the system scale, then the window's.
            let setup = |ctx: &egui::Context| {
                device_options(ctx, Some(system));
                device_window(ctx, monitor);
            };
            let c = raster::render_native(px, monitor, setup, device_screen, Some(&fonts), vec![vec![]; 5], |ctx| {
                app.frame_ui(ctx);
                ppp = ctx.pixels_per_point();
            });
            (c, ppp)
        };
        let (base, _) = shot(1.0, 1.0);
        for (system, monitor) in [(1.0, 1.5), (1.5, 1.0), (1.25, 1.75)] {
            let (c, ppp) = shot(system, monitor);
            assert_eq!(ppp, 1.0, "system {system}, monitor {monitor}");
            assert!(c.px == base.px, "system {system}, monitor {monitor}: {} pixels differ", c.px.iter().zip(&base.px).filter(|(a, b)| a != b).count());
        }
    }

    /// The saved placement (device pixels) sent again in the first frame's
    /// points: the runner applied it at the scale of the monitor the window
    /// landed on (here 1.5, the window sized for a primary one at 1.25).
    #[test]
    fn saved_placement_lands_in_device_pixels_on_another_monitor() {
        let ctx = egui::Context::default();
        device_options(&ctx, Some(1.25));
        let mut app = App::new();
        app.placement_px = Some(Rect::from_min_size(Pos2::new(2600.0, 40.0), Vec2::new(1906.0, 1016.0)));
        let ppp = device_zoom(1.25) * 1.5;
        let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1906.0, 1016.0) / ppp)), ..Default::default() };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.5);
        let out = ctx.run(raw, |ctx| app.frame_ui(ctx));
        let cmds = &out.viewport_output[&egui::ViewportId::ROOT].commands;
        let size = cmds.iter().find_map(|c| if let egui::ViewportCommand::InnerSize(s) = c { Some(*s * ppp) } else { None }).expect("the size sent");
        let at = cmds.iter().find_map(|c| if let egui::ViewportCommand::OuterPosition(p) = c { Some(p.to_vec2() * ppp) } else { None }).expect("the position sent");
        assert!((size - Vec2::new(1906.0, 1016.0)).length() < 0.01 && (at - Vec2::new(2600.0, 40.0)).length() < 0.01, "{size:?} at {at:?}");
        assert!(app.placement_px.is_none(), "once");
    }

    /// A tiling window manager ignores the minimum size: every window size
    /// down to a sliver must lay out without a panic, in every layout.
    #[test]
    fn tiny_window_sizes_do_not_panic() {
        use app::{Command as C, Pane};
        sweep(&mut App::new());
        sweep(&mut demo(&[]));
        sweep(&mut demo(&[C::QuickAnalysis]));
        sweep(&mut demo(&[C::ToggleExplorer, C::ToggleParamBar]));
        sweep(&mut demo(&[C::ToggleRibbonMin]));
        sweep(&mut demo(&[C::TogglePane(Pane::Vector), C::TogglePane(Pane::Circle), C::TogglePane(Pane::Differential), C::TogglePane(Pane::Frequency)]));
        sweep(&mut demo(&[C::ActivatePane(Pane::Circle)]));
        sweep(&mut demo(&[C::ActivatePane(Pane::Differential)]));
        sweep(&mut demo(&[C::OmpAnalysis]));
        sweep(&mut demo(&[C::PrintPreview]));
        sweep(&mut demo(&[C::ZoomX(true), C::ZoomX(true), C::ZoomY(true)]));
        sweep(&mut demo(&[C::StatsRow(0), C::StatsRow(1), C::StatsRow(2)]));
        for dlg in [C::Settings, C::ChannelsDialog, C::Coefficients, C::Print, C::PageSetup, C::OmpParams, C::FileParams, C::ChannelMap, C::HdrComment] {
            sweep(&mut demo(&[dlg]));
        }
    }

    /// Keytips (L9.90): F10 shows them, «F» opens the application menu and
    /// descends into it, Esc closes it and backs out, Esc again leaves.
    #[test]
    fn keytips_open_the_application_menu() {
        let mut app = demo(&[]);
        let ctx = egui::Context::default();
        let size = Vec2::new(1320.0, 860.0);
        let key = |key| vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }];
        let menu_open = |app: &App, ctx: &egui::Context| {
            let (bar, ..) = app.kt_bar.expect("the application button was drawn");
            let state = egui::menu::BarState::load(ctx, bar);
            let root: &Option<egui::menu::MenuRoot> = &state;
            root.is_some()
        };
        frames(&mut app, &ctx, size, vec![vec![], key(egui::Key::F10), vec![]]);
        assert_eq!(app.keytips.level, Some(keytips::Level::Tabs));
        assert!(app.kt_rects.iter().any(|(t, r, _)| *t == keytips::Target::Qat(0) && r.width() > 0.0));
        frames(&mut app, &ctx, size, vec![key(egui::Key::F), vec![], vec![]]);
        assert_eq!(app.keytips.level, Some(keytips::Level::AppMenu));
        assert!(menu_open(&app, &ctx), "«F» opens the application menu");
        assert!(app.kt_rects.iter().any(|(t, ..)| *t == keytips::Target::PrintMain), "its rows carry keytips: {:?}", app.kt_rects);
        frames(&mut app, &ctx, size, vec![key(egui::Key::Escape), vec![]]);
        assert_eq!(app.keytips.level, Some(keytips::Level::Tabs));
        assert!(!menu_open(&app, &ctx), "Esc closes it");
        frames(&mut app, &ctx, size, vec![key(egui::Key::Escape), vec![]]);
        assert!(!app.keytips.shown());
    }

    /// A whole window at the default size with the demo document.
    struct Win {
        ctx: egui::Context,
        app: App,
        /// The modifier keys held.
        held: egui::Modifiers,
    }

    impl Win {
        fn new() -> Win {
            let mut w = Win { ctx: egui::Context::default(), app: demo(&[]), held: egui::Modifiers::NONE };
            // The docking tests start from one tabbed group (not the
            // start-up layout, which `docking::tests` pins).
            w.app.docks = docking::Docks::one_group();
            w.idle(3);
            w
        }

        fn run(&mut self, events: Vec<egui::Event>) -> egui::CursorIcon {
            frames_held(&mut self.app, &self.ctx, Vec2::new(1320.0, 860.0), vec![events], self.held)
        }

        fn idle(&mut self, n: usize) {
            for _ in 0..n {
                self.run(vec![]);
            }
        }

        fn press(&mut self, button: egui::PointerButton, from: Pos2, to: Pos2) {
            let ev = |pos, pressed| egui::Event::PointerButton { pos, button, pressed, modifiers: Default::default() };
            self.run(vec![egui::Event::PointerMoved(from)]);
            self.run(vec![ev(from, true)]);
            self.run(vec![egui::Event::PointerMoved(to)]);
            self.run(vec![ev(to, false)]);
            self.idle(2);
        }

        fn wheel(&mut self, at: Pos2) {
            self.run(vec![egui::Event::PointerMoved(at), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: Vec2::new(0.0, -1.0), modifiers: Default::default() }]);
            self.idle(1);
        }

        fn key(&mut self, key: egui::Key) {
            self.run(vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: self.held }]);
            self.idle(1);
        }

        /// The view state that pointer and key input can change.
        fn view_state(&self) -> String {
            let d = self.app.active_doc().unwrap();
            format!("{} {} {:?} {} {} {:?} {} {:?}", d.main_cursor, d.aux_cursor, d.v_shown, d.first, d.vscroll, d.selection, d.comments.len(), d.xscale)
        }

        /// Every kind of pointer and key input the view reacts to, at `at`.
        fn poke(&mut self, at: Pos2) {
            use egui::PointerButton::{Primary, Secondary};
            self.press(Primary, at, at);
            self.press(Secondary, at, at);
            self.press(Primary, at, at + Vec2::new(60.0, 30.0));
            self.wheel(at);
            self.key(egui::Key::ArrowRight);
        }
    }

    /// The parameter grid's group row (`0x4075c0` → `0x407c40`): a click in
    /// its value area flips the characteristic and marks the document
    /// modified; the second click of a double-click is dropped (`0x407640`).
    #[test]
    fn grid_group_click_toggles_once_and_marks_modified() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.idle(2);
        // A block with characteristics (the record's `.set` has none: «Нет»).
        w.app.block_idx = 1;
        w.app.ensure_block_state();
        w.idle(2);
        w.app.active_doc_mut().unwrap().set.modified = false;
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        // The first group row: «Тип блока»'s 17 px row, then its middle.
        let at = Pos2::new(grid.right() - 60.0, grid.top() + 1.0 + 17.0 + 8.0);
        let vis = |w: &Win| w.app.chars.get(&(w.app.block_idx, 0)).map(|c| c.visible);
        let before = vis(&w);
        w.press(Primary, at, at);
        assert_eq!(vis(&w), before.map(|v| !v), "one click flips");
        assert!(w.app.active_doc().unwrap().set.modified);
        let ev = |pressed| egui::Event::PointerButton { pos: at, button: Primary, pressed, modifiers: Default::default() };
        w.run(vec![egui::Event::PointerMoved(at), ev(true)]);
        w.run(vec![ev(false)]);
        w.run(vec![ev(true)]);
        w.run(vec![ev(false)]);
        w.idle(2);
        assert_eq!(vis(&w), before, "a double-click flips once");
    }

    /// The differential window's initial push runs on its first paint, a
    /// plane too small to draw included (VM, L9.403): at the 944 px first
    /// run it moves x only, and the view keeps that when the frame grows.
    #[test]
    fn the_differential_push_runs_on_a_small_first_paint() {
        let mut app = demo(&[]);
        let ctx = egui::Context::default();
        frames_held(&mut app, &ctx, Vec2::new(944.0, 560.0), vec![vec![]; 3], egui::Modifiers::NONE);
        let v = app.diag.diff;
        assert!(!v.initial_pending, "pushed on the small first paint");
        assert_eq!(v.oy, 0, "no room to push y");
        frames_held(&mut app, &ctx, Vec2::new(1280.0, 760.0), vec![vec![]; 3], egui::Modifiers::NONE);
        assert_eq!((app.diag.diff.ox, app.diag.diff.oy), (v.ox, v.oy), "kept after the frame grows");
    }

    /// A dialog taller than the work area hangs from its top (MFC's
    /// CenterWindow, VM L9.401: «Отображение каналов» at 1280 × 800).
    #[test]
    fn a_tall_dialog_keeps_its_caption_on_the_screen() {
        let mut app = demo(&[]);
        app.open_channels_dialog();
        let ctx = egui::Context::default();
        frames_held(&mut app, &ctx, Vec2::new(1280.0, 752.0), vec![vec![]; 6], egui::Modifiers::NONE);
        let list = app.dlg.channels.as_ref().and_then(|s| s.origin).expect("the dialog drawn");
        // Its caption (25 px) and the 1 px frame above the list.
        assert_eq!(list.y, 26.0, "the window from row 0");
        assert_eq!(dialogs::work_area_fit(Pos2::new(500.0, -13.0), Vec2::new(360.0, 780.0), Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 752.0))), Pos2::new(500.0, 0.0));
        assert_eq!(dialogs::work_area_fit(Pos2::new(1000.0, 600.0), Vec2::new(360.0, 200.0), Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 752.0))), Pos2::new(918.0, 550.0), "right and bottom edges inside first");
    }

    /// A short pane keeps the grid's saved height (VM, L9.399: the
    /// original's layout `0x4376e0` does not clamp it; only a drag does).
    #[test]
    fn a_short_pane_keeps_the_grids_height() {
        let mut app = demo(&[]);
        app.persist = true;
        let ctx = egui::Context::default();
        frames_held(&mut app, &ctx, Vec2::new(944.0, 560.0), vec![vec![]; 3], egui::Modifiers::NONE);
        let g = *app.pane_grid.first().expect("the differential grid");
        assert_eq!(g.height().round(), 200.0, "{g:?}");
        let saved = app.store.view.get("Diff Diagram\\Properties Height");
        assert!(!matches!(saved, Some(v) if v != "200"), "kept, not clamped: {saved:?}");
    }

    /// The ruler labels its ticks out to the plot's edge, past the record's
    /// end too (VM, L9.398: «12 288» after a 10.999 s record).
    #[test]
    fn the_ruler_labels_ticks_past_the_record_end() {
        let mut app = demo(&[]);
        let n = app.active_doc().unwrap().osc.time.len();
        // The whole record within the first 25 px.
        app.active_doc_mut().unwrap().xscale = Some(1.0 / (2.0 * n as f64));
        let ctx = egui::Context::default();
        frames_held(&mut app, &ctx, Vec2::new(1320.0, 860.0), vec![vec![]; 2], egui::Modifiers::NONE);
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0))), time: Some(ctx.input(|i| i.time) + 0.05), ..Default::default() };
        let out = ctx.run(raw, |ctx| app.frame_ui(ctx));
        let labels: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.sections.first().is_some_and(|f| f.format.color == egui::Color32::from_rgb(0, 0, 127)) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .filter(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '-'))
            .collect();
        let ms = |t: &str| t.trim_end_matches(" мс").replace(' ', "").parse::<f64>().ok();
        let last = labels.iter().filter_map(|t| ms(t)).fold(f64::MIN, f64::max);
        let dur = app.active_doc().unwrap().osc.duration() * 1000.0;
        assert!(last > dur, "a label past the record's {dur} ms: {labels:?}");
    }

    /// The start-up docking on a small frame (VM, a clone's 944 px first
    /// run, L9.394): each block capped at half the documents' area the
    /// earlier ones leave, then kept as the frame grows or shrinks.
    #[test]
    fn start_up_blocks_take_half_the_area_left_and_keep_it() {
        use docking::Unit;
        let mut app = demo(&[]);
        let ctx = egui::Context::default();
        let mut seen = Vec::new();
        for size in [Vec2::new(944.0, 560.0), Vec2::new(1280.0, 760.0), Vec2::new(852.0, 493.0)] {
            frames_held(&mut app, &ctx, size, vec![vec![]; 3], egui::Modifiers::NONE);
            let r = app.dock_rects;
            let (v, d, f) = (r[Unit::Vector as usize].expect("vector"), r[Unit::Differential as usize].expect("differential"), r[Unit::Frequency as usize].expect("frequency"));
            seen.push((v.width(), d.width(), f.height()));
            if size.x == 944.0 {
                assert_eq!(v.width(), 472.0, "⌊944 / 2⌋");
                assert_eq!(d.width(), ((944.0 - 476.0) / 2.0f32).floor(), "half of what Векторная's block leaves");
                assert_eq!(f.height(), (v.height() / 2.0).floor().min(200.0), "half the documents' height, at most 200");
                assert!(d.left() - 4.0 > 200.0, "the documents keep {} px", d.left() - 4.0);
            }
        }
        assert!(seen.windows(2).all(|w| w[0] == w[1]), "the blocks keep their sizes: {seen:?}");
    }

    /// The grid's splitter drags (VM: the original's moves with a drag
    /// from its 5 px band).
    #[test]
    fn the_grid_splitter_drags() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.idle(2);
        let g0 = *w.app.pane_grid.first().expect("the circle pane's grid");
        let at = Pos2::new(g0.center().x, g0.bottom() + 2.0);
        w.press(Primary, at, at + Vec2::new(0.0, 12.0));
        let g1 = *w.app.pane_grid.first().unwrap();
        assert!((g1.bottom() - at.y - 12.0).abs() <= 1.0, "{g0:?} → {g1:?}");
        // After a click on a pane tab too (VM: ours' splitters stopped
        // dragging after one).
        // A quick click: press and release in one (slow) frame.
        let quick = |w: &mut Win, at: Pos2| {
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: Primary, pressed, modifiers: Default::default() };
            w.run(vec![egui::Event::PointerMoved(at), ev(true), ev(false)]);
            w.idle(2);
        };
        let tab = w.app.pane_tab_rects.iter().find(|(p, _)| *p == app::Pane::Vector).map(|(_, r)| r.center()).expect("the vector tab");
        quick(&mut w, tab);
        let tab = w.app.pane_tab_rects.iter().find(|(p, _)| *p == app::Pane::Circle).map(|(_, r)| r.center()).expect("the circle tab");
        quick(&mut w, tab);
        assert_eq!(w.app.pane_grid.len(), 1, "the circle in front");
        let g1 = *w.app.pane_grid.first().unwrap();
        let at = Pos2::new(g1.center().x, g1.bottom() + 2.0);
        w.press(Primary, at, at - Vec2::new(0.0, 12.0));
        let g2 = *w.app.pane_grid.first().unwrap();
        assert!((g2.bottom() - (at.y - 12.0)).abs() <= 1.0, "after a tab click: {g1:?} → {g2:?}");
        // Its line and brush show the resize cursor; the plane under them
        // does not (VM, L9.392).
        let hover = |w: &mut Win, y: f32| {
            w.run(vec![egui::Event::PointerMoved(Pos2::new(g2.center().x, y))]);
            w.run(vec![])
        };
        for dy in [1.0, 3.0, 5.0] {
            assert_eq!(hover(&mut w, g2.bottom() + dy), egui::CursorIcon::ResizeVertical, "{dy} under the grid");
        }
        assert_ne!(hover(&mut w, g2.bottom() + 40.0), egui::CursorIcon::ResizeVertical, "on the plane");
        // The start-up layout's differential pane too.
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Differential));
        w.idle(3);
        let g = *w.app.pane_grid.first().expect("the differential pane's grid");
        for dy in [1.0, 3.0, 5.0] {
            let c = { w.run(vec![egui::Event::PointerMoved(Pos2::new(g.center().x, g.bottom() + dy))]); w.run(vec![]) };
            assert_eq!(c, egui::CursorIcon::ResizeVertical, "{dy} under the differential grid {g:?}");
        }
    }

    /// «Тип блока» alone (VM): F4 drops nothing; ↓ steps its value in an
    /// in-place edit, Enter keeps the text without loading the block, Esc
    /// puts it back.
    #[test]
    fn the_block_type_alone_steps_its_text() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.idle(2);
        assert_eq!(w.app.block_idx, 0, "«Нет»");
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        let at = Pos2::new(grid.left() + 30.0, grid.top() + 9.0);
        w.press(Primary, at, at);
        w.key(egui::Key::F4);
        w.idle(1);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::Enter);
        w.idle(2);
        let names: Vec<String> = w.app.library.blocks.iter().map(|b| b.name.clone()).collect();
        assert_eq!((w.app.block_idx, w.app.block_text.as_deref()), (0, Some(names[2].as_str())), "the text only");
        assert!(w.app.chars.is_empty(), "no block's rows");
        w.key(egui::Key::ArrowUp);
        w.key(egui::Key::Escape);
        w.idle(2);
        assert_eq!(w.app.block_text.as_deref(), Some(names[2].as_str()), "Esc puts the edit's text back");
        // A click on the value keeps the text; ↓ goes on from it.
        let value = Pos2::new(grid.left() + grid.width() * 0.75, grid.top() + 9.0);
        w.press(Primary, value, value);
        w.idle(1);
        assert_eq!(w.app.block_text.as_deref(), Some(names[2].as_str()), "the click keeps the text");
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::Enter);
        w.idle(2);
        assert_eq!((w.app.block_idx, w.app.block_text.as_deref()), (0, Some(names[3].as_str())), "↓ from the shown text");
    }

    /// The grid's keys (CMFCPropertyGridCtrl): ↓ from a group selects its
    /// first parameter, a typed character opens its edit with that
    /// character, Esc puts the text back; ← closes the group, so ↓ goes to
    /// the next group.
    #[test]
    fn grid_keys_move_edit_and_fold() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.idle(2);
        let b = (0..w.app.library.blocks.len()).find(|&b| w.app.library.blocks[b].characteristics.len() >= 2 && !w.app.library.blocks[b].characteristics[0].parameters.is_empty()).expect("a block with two characteristics");
        w.app.block_idx = b;
        w.app.ensure_block_state();
        w.idle(2);
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        let at = Pos2::new(grid.left() + 60.0, grid.top() + 1.0 + 17.0 + 8.0);
        w.press(Primary, at, at);
        assert_eq!(w.app.selected_char, Some(0));
        let p0 = w.app.library.blocks[b].characteristics[0].parameters[0].name.clone();
        let text = |w: &Win| w.app.chars.get(&(b, 0)).and_then(|c| c.settings.get(&p0).cloned()).unwrap_or_default();
        let before = text(&w);
        w.key(egui::Key::ArrowDown);
        w.run(vec![egui::Event::Text("7".into())]);
        w.idle(2);
        assert_eq!(text(&w), "7");
        assert!(w.ctx.memory(|m| m.focused().is_some()), "the edit is open");
        w.key(egui::Key::Escape);
        w.idle(2);
        assert_eq!(text(&w), before, "Esc puts the text back");
        w.key(egui::Key::ArrowUp);
        w.key(egui::Key::ArrowLeft);
        w.key(egui::Key::ArrowDown);
        w.idle(1);
        assert_eq!(w.app.selected_char, Some(1), "the closed group's parameters are skipped");
    }

    /// The parameters bar is a message-mode caption bar (`0x474dc5`): its
    /// close button at the top right hides it.
    #[test]
    fn the_parameters_bar_closes_from_its_button() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        assert!(w.app.dlg.param_bar);
        let bar = egui::containers::panel::PanelState::load(&w.ctx, egui::Id::new("parambar")).expect("the bar").rect;
        w.press(Primary, Pos2::new(400.0, bar.center().y), Pos2::new(400.0, bar.center().y));
        assert!(w.app.dlg.param_bar, "a click on the bar's text does nothing");
        let mut y = bar.top();
        while w.app.dlg.param_bar && y < bar.bottom() {
            w.press(Primary, Pos2::new(1320.0 - 12.0, y), Pos2::new(1320.0 - 12.0, y));
            y += 4.0;
        }
        assert!(!w.app.dlg.param_bar, "closed by its ✕");
    }

    /// The original's dialogs are all modal: while one is up the view
    /// (and the rest of the main window) gets no pointer or key input,
    /// whether the pointer is over the dialog or beside it.
    #[test]
    fn the_view_ignores_input_while_a_dialog_is_up() {
        let at = Pos2::new(420.0, 470.0);
        let mut w = Win::new();
        w.app.color_dialog = Some(egui::Color32::RED);
        w.idle(3);
        let win = w.ctx.memory(|m| m.area_rect(egui::Id::new("Цвета"))).expect("the colour dialog");
        assert!(!win.contains(at));
        let before = w.view_state();
        // Beside the dialog, on its colour plane, on its right margin.
        for p in [at, win.center(), Pos2::new(win.right() - 3.0, win.center().y)] {
            w.poke(p);
            assert!(w.app.color_dialog.is_some());
        }
        assert_eq!(w.view_state(), before, "input reached the view under the dialog");
        assert!(w.app.color_dialog.is_some());
        // Sanity: without the dialog the same input does reach the view.
        w.app.color_dialog = None;
        w.idle(3);
        w.poke(at);
        assert_ne!(w.view_state(), before);
    }

    /// The same for the other dialogs: input beside them never reaches the
    /// view, also with a page setup nested over the print dialog.
    #[test]
    fn the_view_ignores_input_beside_any_dialog() {
        use app::Command as C;
        for cmds in [vec![C::Print], vec![C::Print, C::PageSetup], vec![C::Settings], vec![C::ChannelsDialog], vec![C::Coefficients], vec![C::OmpParams], vec![C::FileParams]] {
            let mut w = Win::new();
            let before = w.view_state();
            for c in &cmds {
                w.app.dispatch(c.clone());
                w.idle(3);
            }
            assert!(w.app.modal_up, "{cmds:?}: a dialog is up");
            let windows: Vec<Rect> = w.ctx.memory(|m| {
                let ids = m.areas().visible_layer_ids();
                ids.iter().filter(|l| l.order >= egui::Order::Middle).filter_map(|l| m.area_rect(l.id)).filter(|r| r.width() < 1300.0).collect()
            });
            let free = [Pos2::new(420.0, 470.0), Pos2::new(300.0, 780.0), Pos2::new(560.0, 330.0), Pos2::new(250.0, 600.0)]
                .into_iter()
                .find(|p| !windows.iter().any(|r| r.expand(4.0).contains(*p)))
                .expect("a point of the view beside the dialogs");
            w.poke(free);
            assert_eq!(w.view_state(), before, "{cmds:?}: input reached the view");
            assert!(w.app.modal_up, "{cmds:?}: the dialog stays up");
        }
    }

    /// The Office 2007 round application button (45 × 45 over both rows):
    /// its lower half, in the tab row, opens the menu too.
    #[test]
    fn the_round_application_button_opens_from_its_lower_half() {
        let mut w = Win::new();
        let open = |w: &Win| w.ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Foreground && m.area_rect(l.id).is_some_and(|r| r.width() > 400.0)));
        assert!(!open(&w));
        w.press(egui::PointerButton::Primary, Pos2::new(25.0, 40.0), Pos2::new(25.0, 40.0));
        assert!(open(&w), "the menu opened from the orb's lower half");
    }

    /// The «Файл» menu is MFC's main panel, 490 px wide (VM), not the
    /// window's width; hovering «Печать» puts its sub-items in the right
    /// column.
    #[test]
    fn app_menu_is_two_columns_and_shows_print_items_on_the_right() {
        use ribbon::{app_sub_id, AppSub};
        let mut w = Win::new();
        w.app.mru = vec!["/data/a.cfg".into()];
        w.press(egui::PointerButton::Primary, Pos2::new(48.0, 12.0), Pos2::new(48.0, 12.0));
        let menu = w.ctx.memory(|m| {
            let ids = m.areas().visible_layer_ids();
            ids.iter().filter(|l| l.order == egui::Order::Foreground).filter_map(|l| m.area_rect(l.id)).max_by(|a, b| a.area().total_cmp(&b.area()))
        });
        let menu = menu.expect("the menu is open");
        assert!((489.0..=494.0).contains(&menu.width()), "{menu:?}");
        let sub = |w: &Win| w.ctx.data(|d| d.get_temp::<(Option<AppSub>, u64)>(app_sub_id())).and_then(|s| s.0);
        assert_eq!(sub(&w), None);
        let print = (0..(menu.height() as i32 / 4))
            .map(|k| Pos2::new(menu.left() + 40.0, menu.top() + 4.0 * k as f32))
            .find(|&p| {
                w.run(vec![egui::Event::PointerMoved(p)]);
                w.idle(1);
                sub(&w) == Some(AppSub::Print)
            })
            .expect("a «Печать» item");
        // Across to the first sub-item, «Параметры страницы» (under the
        // 25 px top band and the column's header).
        let target = Pos2::new(menu.left() + 300.0, menu.top() + 25.0 + 27.0 + 15.0);
        w.run(vec![egui::Event::PointerMoved(Pos2::new(menu.left() + 230.0, print.y))]);
        w.press(egui::PointerButton::Primary, target, target);
        assert!(w.app.print.setup_dlg.is_some(), "page setup opened from the right column");
    }

    /// A short tab group keeps its pane inside: the toolbar stops at the
    /// tab strip instead of growing over it (and the canvas with it).
    #[test]
    fn diagram_pane_toolbar_stays_above_the_tab_strip() {
        use egui::containers::panel::PanelState;
        for h in [860.0, 700.0, 560.0] {
            let ctx = egui::Context::default();
            let mut app = demo(&[]);
            frames(&mut app, &ctx, Vec2::new(1320.0, h), vec![vec![]; 3]);
            let bar = PanelState::load(&ctx, egui::Id::new(("pane_toolbar", diagram_ui::Window::Vector as u8))).expect("the vector toolbar").rect;
            let tabs = PanelState::load(&ctx, egui::Id::new(("pane_tabs", docking::Unit::Vector))).expect("the tab strip").rect;
            // Either it ends above the tabs or it is too short for a button.
            assert!(bar.bottom() <= tabs.top() + 0.5 || bar.height() < 22.0, "h {h}: toolbar {bar:?} over tabs {tabs:?}");
        }
    }

    /// The document tabs sit in the MDI area: under the parameters bar and
    /// left of the docked diagrams, as the original's frame lays them out.
    #[test]
    fn document_tabs_sit_in_the_mdi_area() {
        use egui::containers::panel::PanelState;
        let ctx = egui::Context::default();
        let mut app = demo(&[]);
        frames(&mut app, &ctx, Vec2::new(1320.0, 860.0), vec![vec![]; 3]);
        let bar = PanelState::load(&ctx, egui::Id::new("parambar")).expect("the parameters bar").rect;
        let tab = app.doc_tab_rects[0];
        assert!(tab.top() >= bar.bottom(), "tab {tab:?} under the bar {bar:?}");
        // The panes beside it (the frequency diagram docks below it).
        let diagrams = app.pane_canvas.iter().filter(|(p, _)| *p != app::Pane::Frequency).map(|(_, r)| r.left()).fold(f32::MAX, f32::min);
        assert!(tab.right() < diagrams, "tab {tab:?} left of the diagrams at {diagrams}");
    }

    /// A narrow window collapses ribbon panels from the right instead of
    /// wrapping their labels (MFC's reduction steps, L9.91); the ribbon keeps its height and a collapsed
    /// panel opens below its button.
    #[test]
    fn narrow_ribbon_collapses_panels_and_keeps_its_height() {
        use egui::containers::panel::PanelState;
        // A width where «Вычисления» has collapsed (the panels pack their
        // elements edge to edge, L9.187).
        const NARROW: f32 = 600.0;
        let height = |w: f32| {
            let ctx = egui::Context::default();
            let mut app = demo(&[]);
            frames(&mut app, &ctx, Vec2::new(w, 860.0), vec![vec![]; 3]);
            let collapsed: Vec<usize> = ["Объединение", "Диаграммы", "Видимость", "Вычисления", " "]
                .iter()
                .map(|t| ctx.data(|d| d.get_temp::<usize>(egui::Id::new(("fv_ribbon_group_step", *t)))).unwrap_or(0))
                .collect();
            (PanelState::load(&ctx, egui::Id::new("top")).unwrap().rect.height(), collapsed, ctx, app)
        };
        let (h_wide, c_wide, ..) = height(1320.0);
        assert_eq!(c_wide, vec![0; 5], "everything fits at the default size");
        let (h_narrow, c_narrow, ctx, mut app) = height(NARROW);
        assert!((h_narrow - h_wide).abs() < 1.0, "ribbon height {h_narrow} vs {h_wide}");
        // MFC's order (L9.91): Вычисления and Диаграммы step through their
        // layouts first, «Вычисления» collapses (step 3); with the read
        // collapsed-button width (~30 px for the untitled panel, L9.114)
        // the single-button " " panel is past 1.5 × it and collapses too.
        assert!(c_narrow[3] == 3 && c_narrow[1] > 0, "steps: {c_narrow:?}");
        // The first width that needs a step: the panel with the most steps
        // left takes it (ties: the rightmost).
        let first = (0..30).map(|k| height(1300.0 - 20.0 * k as f32).1).find(|c| c.iter().any(|&s| s > 0)).expect("a step before the narrow width");
        assert_eq!(first, vec![0, 0, 0, 1, 0], "first step: the panel with the most steps left, rightmost");
        // «Вычисления»'s button opens the panel as a popup.
        let popup = egui::Id::new(("fv_ribbon_group_popup", "Вычисления"));
        for x in (300..NARROW as i32).step_by(10) {
            let at = Pos2::new(x as f32, 100.0);
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frames(&mut app, &ctx, Vec2::new(NARROW, 860.0), vec![vec![egui::Event::PointerMoved(at)], vec![ev(true)], vec![ev(false)], vec![]]);
            if ctx.memory(|m| m.is_popup_open(popup)) {
                return;
            }
            frames(&mut app, &ctx, Vec2::new(NARROW, 860.0), vec![vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }], vec![]]);
        }
        panic!("no collapsed «Вычисления» button opened its panel");
    }

    /// The diagram panes dock, float and auto-hide (L9.92): a caption
    /// double-click floats the tab group in its own window and back; the
    /// pin puts it on the right strip, from which a 400 ms hover slides it
    /// out.
    #[test]
    fn diagram_panes_float_redock_and_auto_hide() {
        use docking::{Place, Unit};
        let mut w = Win::new();
        let canvas = |w: &Win| w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Vector).map(|(_, r)| *r);
        let double = |w: &mut Win, at: Pos2| {
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.run(vec![ev(true), ev(false)]);
            w.run(vec![ev(true), ev(false)]);
            w.idle(2);
        };
        let docked = canvas(&w).expect("the vector pane is drawn");
        double(&mut w, Pos2::new(docked.left() + 60.0, docked.top() - 10.0));
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating);
        let floating = canvas(&w).expect("drawn in its window");
        double(&mut w, Pos2::new(floating.left() + 60.0, floating.top() - 10.0));
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Docked, "a double-click on the floating caption redocks");
        // Pin: the strip on the right; hover its first button 400 ms.
        w.app.docks[Unit::Vector].pin();
        w.idle(2);
        assert!(canvas(&w).is_none(), "pinned away");
        let strip = Pos2::new(1320.0 - 12.0, w.app.view_rects.unwrap().0.top() + 40.0);
        w.run(vec![egui::Event::PointerMoved(strip)]);
        w.idle(10);
        assert!(w.app.docks[Unit::Vector].out, "the hover slid it out");
        w.idle(14);
        assert!(canvas(&w).is_some(), "drawn over the content");
    }

    /// The pin (mfc120u `0x100871c7`): a plain click slides the group shut
    /// in 12 steps; with Ctrl every shown group on the same edge goes at
    /// once, no slide, and Ctrl on an auto-hidden one brings them all back.
    #[test]
    fn ctrl_pin_takes_the_whole_edge() {
        use docking::{Place, Unit};
        let mut w = Win::new();
        let caption_pin = |w: &Win| {
            let r = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Vector).map(|(_, r)| *r).expect("the vector pane");
            let group = w.app.dock_rects[Unit::Vector as usize].expect("its group");
            Pos2::new(group.right() - 26.0, r.top() - 10.0)
        };
        let click = |w: &mut Win, at: Pos2, ctrl: bool| {
            let modifiers = egui::Modifiers { ctrl, command: ctrl, ..Default::default() };
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers };
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.held = modifiers;
            w.run(vec![ev(true), ev(false)]);
            w.held = egui::Modifiers::NONE;
        };
        let edge = w.app.docks.edge(Unit::Vector);
        let same: Vec<Unit> = docking::UNITS
            .into_iter()
            .filter(|&u| w.app.docks.host(u) == u && w.app.docks[u].place == Place::Docked && w.app.docks.edge(u) == edge && w.app.panes.front(w.app.docks.members(u)).is_some())
            .collect();
        let at = caption_pin(&w);
        click(&mut w, at, true);
        for &u in &same {
            assert_eq!((w.app.docks[u].place, w.app.docks[u].slide), (Place::AutoHide, 0.0), "{u:?} at once");
        }
        // A plain pin back, then a plain pin: it slides shut from 1.
        w.app.docks[Unit::Vector].pin();
        w.idle(3);
        let at = caption_pin(&w);
        click(&mut w, at, false);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::AutoHide);
        assert!(w.app.docks[Unit::Vector].slide > 0.0, "sliding shut");
    }

    /// Smart docking (L9.101): Частотная's caption dragged out of its pane
    /// detaches; released on the group's five-way centre it becomes a tab
    /// of the group; dragged out again (the group floats with it) and
    /// released on the frame's left edge marker the group docks there.
    #[test]
    fn smart_docking_drops_on_the_markers() {
        use docking::{Edge, Place, Spot, Unit};
        let mut w = Win::new();
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let canvas = |w: &Win, p: app::Pane| w.app.pane_canvas.iter().find(|(q, _)| *q == p).map(|(_, r)| *r);
        // Press on a caption, pull it out over the lanes (it detaches),
        // then over the marker `target` picks from the layout, release.
        let drag = |w: &mut Win, from: Pos2, target: &dyn Fn(&App) -> Pos2| {
            let out = Pos2::new(300.0, 450.0);
            w.run(vec![egui::Event::PointerMoved(from)]);
            w.run(vec![ev(from, true)]);
            for k in 1..=4 {
                w.run(vec![egui::Event::PointerMoved(from + (out - from) * (k as f32 / 4.0))]);
            }
            w.idle(2);
            let to = target(&w.app);
            w.run(vec![egui::Event::PointerMoved(to)]);
            w.idle(2);
            w.run(vec![ev(to, false)]);
            w.idle(3);
        };
        let f = canvas(&w, app::Pane::Frequency).expect("Частотная drawn");
        let centre = |a: &App| docking::five_way(a.dock_rects[Unit::Vector as usize].expect("the group's pane"), true)[4].0.center();
        drag(&mut w, Pos2::new(f.left() + 80.0, f.top() - 10.0), &centre);
        assert_eq!((w.app.docks[Unit::Frequency].place, w.app.docks[Unit::Frequency].spot), (Place::Docked, Spot::Tab(Unit::Vector)));
        assert_eq!(w.app.dock_views_shown(), 1, "one tabbed group");
        // The group's caption out to the left edge marker.
        let v = canvas(&w, app::Pane::Vector).or_else(|| canvas(&w, app::Pane::Frequency)).expect("the group's pane");
        let left = |_: &App| docking::edge_markers(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0)).expand(8.0))[0].0.center();
        drag(&mut w, Pos2::new(v.left() + 80.0, v.top() - 10.0), &left);
        assert_eq!((w.app.docks[Unit::Vector].place, w.app.docks[Unit::Vector].spot), (Place::Docked, Spot::Edge { edge: Edge::Left, outer: true }));
        assert_eq!(w.app.docks[Unit::Frequency].spot, Spot::Tab(Unit::Vector), "the tab went along");
        let g = w.app.dock_rects[Unit::Vector as usize].unwrap();
        assert!(g.left() < 20.0, "on the left edge: {g:?}");
        // The Частотная tab pulled out of the strip floats it alone (it
        // remembers its tab place); released over nothing it stays.
        let tab = w.app.pane_tab_rects.iter().find(|(p, _)| *p == app::Pane::Frequency).map(|(_, r)| r.center()).expect("the Частотная tab");
        let nowhere = |_: &App| Pos2::new(600.0, 600.0);
        drag(&mut w, tab, &nowhere);
        assert_eq!((w.app.docks[Unit::Frequency].place, w.app.docks[Unit::Frequency].spot), (Place::Floating, Spot::Tab(Unit::Vector)));
        assert_eq!((w.app.docks[Unit::Vector].place, w.app.dock_views_shown()), (Place::Docked, 2));
        // The pane's corner under the pointer, the frame 4 left, 27 up (VM).
        let at = w.app.docks[Unit::Frequency].float_rect.expect("floating").min;
        assert!((at - Pos2::new(596.0, 573.0)).length() < 1.0, "{at:?}");
        // With Ctrl held nothing docks (`0x1008c860`, `0x1008a2cd`): the
        // floating Частотная released on the group's centre marker stays
        // floating where it was let go.
        let fr = w.app.docks[Unit::Frequency].float_rect.expect("floating");
        let from = Pos2::new(fr.left() + 40.0, fr.top() + 8.0);
        w.held = egui::Modifiers::CTRL;
        w.run(vec![egui::Event::PointerMoved(from)]);
        w.run(vec![ev(from, true)]);
        w.idle(2);
        let to = centre(&w.app);
        for k in 1..=4 {
            w.run(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 4.0))]);
        }
        w.run(vec![ev(to, false)]);
        w.idle(3);
        w.held = egui::Modifiers::NONE;
        assert_eq!(w.app.docks[Unit::Frequency].place, Place::Floating, "Ctrl held: no dock");
        let moved = w.app.docks[Unit::Frequency].float_rect.unwrap();
        assert!((moved.min - (fr.min + (to - from))).length() < 2.0, "it moved with the pointer: {fr:?} → {moved:?}");
    }

    /// VM: in the start-up layout the Круговая tab pulled out of the
    /// Векторная group floats Круговая alone, its frame at the pointer
    /// − (4, 27); Векторная stays docked in front.
    #[test]
    fn a_pane_tab_pulled_out_floats_that_pane_at_the_pointer() {
        use docking::{Place, Unit};
        let mut w = Win::new();
        w.idle(2);
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let from = w.app.pane_tab_rects.iter().find(|(p, _)| *p == app::Pane::Circle).map(|(_, r)| r.center()).expect("the Круговая tab");
        let to = Pos2::new(500.0, 400.0);
        w.run(vec![egui::Event::PointerMoved(from)]);
        w.run(vec![ev(from, true)]);
        for k in 1..=10 {
            w.run(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 10.0))]);
        }
        w.run(vec![ev(to, false)]);
        w.idle(3);
        assert_eq!(w.app.docks[Unit::Circle].place, Place::Floating);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Docked);
        let at = w.app.docks[Unit::Circle].float_rect.expect("floating").min;
        assert!((at - Pos2::new(496.0, 373.0)).length() < 1.0, "{at:?}");
    }

    /// Table 128 as egui-winit delivers it: Ctrl+C arrives as `Event::Copy`
    /// and Ctrl+V as `Event::Paste` (only with text on the OS clipboard) or
    /// nothing but its key-up; Ctrl+Shift does not match; a disabled
    /// command's accelerator does nothing.
    #[test]
    fn shortcuts_as_egui_winit_sends_them() {
        let mut w = Win::new();
        let pending = |w: &Win| w.app.active_doc().unwrap().pending.len();
        w.held = egui::Modifiers::CTRL;
        w.run(vec![egui::Event::Copy]);
        assert!(!w.app.clip.available(), "no channel selected: copy disabled");
        w.app.active_doc_mut().unwrap().selection.insert(app::ChanKey::Analog(0));
        w.held = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
        w.run(vec![egui::Event::Copy]);
        assert!(!w.app.clip.available(), "Ctrl+Shift+C is not in the table");
        // A slow frame (the software runner): Ctrl already up at its end.
        w.held = egui::Modifiers::NONE;
        w.run(vec![egui::Event::Copy]);
        assert!(w.app.clip.available(), "Ctrl+C copies");
        w.held = egui::Modifiers::CTRL;
        // No text on the OS clipboard: only the key-up comes.
        let up = egui::Event::Key { key: egui::Key::V, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::CTRL };
        w.run(vec![up.clone()]);
        assert_eq!(pending(&w), 1, "Ctrl+V's key-up pastes");
        w.app.dispatch(app::Command::Undo);
        assert_eq!(pending(&w), 0);
        // With text there: the paste fires on the press, not again on the key-up.
        w.run(vec![egui::Event::Paste("x".into())]);
        w.run(vec![up]);
        assert_eq!(pending(&w), 1, "pasted once");
        // No MDI keys with tabbed documents (VM): Ctrl+Tab, Ctrl+F6 and
        // Ctrl+F4 change nothing.
        w.app.dispatch(app::Command::Demo);
        w.idle(2);
        assert_eq!((w.app.docs.len(), w.app.active), (2, Some(1)));
        w.key(egui::Key::Tab);
        w.key(egui::Key::F6);
        w.app.docs[1].set.modified = false;
        w.key(egui::Key::F4);
        w.idle(2);
        assert_eq!((w.app.docs.len(), w.app.active), (2, Some(1)), "nothing moves or closes");
    }

    /// MFC's pane menu on a caption right-click (`0x100ce72c`): Плавающее
    /// floats the pane, Закрепленное docks it back; В закладке is greyed.
    #[test]
    fn the_pane_menu_floats_and_docks() {
        use docking::{Place, Unit};
        let mut w = Win::new();
        let f = w.app.pane_canvas.iter().find(|(q, _)| *q == app::Pane::Frequency).map(|(_, r)| *r).expect("Частотная drawn");
        w.press(egui::PointerButton::Secondary, Pos2::new(f.left() + 80.0, f.top() - 10.0), Pos2::new(f.left() + 80.0, f.top() - 10.0));
        assert!(w.app.pane_menu.is_some(), "the pane menu opens");
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::Enter);
        w.idle(2);
        assert!(w.app.pane_menu.is_none());
        assert_eq!(w.app.docks[Unit::Frequency].place, Place::Floating, "Плавающее");
        // On the floating mini-frame: ↓ ↓ = Закрепленное.
        let fr = w.app.docks[Unit::Frequency].float_rect.unwrap();
        let at = Pos2::new(fr.left() + 40.0, fr.top() + 8.0);
        w.press(egui::PointerButton::Secondary, at, at);
        assert!(w.app.pane_menu.is_some());
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::Enter);
        w.idle(2);
        assert_eq!(w.app.docks[Unit::Frequency].place, Place::Docked, "Закрепленное");
    }

    /// A property grid leaves the right-click to its pane: the pane's menu
    /// opens there (VM: «Floating / Docking / …» over «Тип блока»).
    #[test]
    fn a_right_click_on_the_property_grid_opens_its_panes_menu() {
        let mut w = Win::new();
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.idle(2);
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        let at = Pos2::new(grid.left() + 40.0, grid.bottom() - 20.0);
        w.press(egui::PointerButton::Secondary, at, at);
        let (_, pane, _, menu) = w.app.pane_menu.expect("the pane menu opens");
        assert_eq!(pane, app::Pane::Circle);
        assert_eq!(menu.pos, at, "at the pointer");
    }

    /// The document strip's ▾ opens the list under it at its left (an MFC
    /// popup, VM); an entry activates its document and closes the list.
    #[test]
    fn the_document_list_opens_and_activates() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::Demo);
        w.idle(3);
        assert_eq!((w.app.docs.len(), w.app.active), (2, Some(1)));
        let tab = w.app.doc_tab_rects[0];
        let area = w.app.mdi_rects[0];
        let arrow = Pos2::new(area.right() - 10.0, tab.top() + 9.0);
        w.press(Primary, arrow, arrow);
        // The first row of the list: 2 px of frame, then 22 px rows.
        let row = Pos2::new(arrow.x + 20.0, tab.top() + 16.0 + 3.0 + 11.0);
        w.press(Primary, row, row);
        assert_eq!(w.app.active, Some(0), "the first document by title");
    }

    /// A docked container resizes from its 4 px divider strip only (MFC's
    /// pane divider, L9.109): a press just outside it, in the view, is the
    /// view's (VM: 2 px inside the oscillogram a drag is a lane drag).
    #[test]
    fn a_docked_container_resizes_from_its_strip_only() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        let g = |w: &Win| w.app.dock_rects[docking::Unit::Vector as usize].expect("the docked group");
        let r = g(&w);
        let y = r.center().y;
        w.press(Primary, Pos2::new(r.left() - 6.0, y), Pos2::new(r.left() - 106.0, y));
        assert_eq!(g(&w).width(), r.width(), "a press in the view does not resize");
        w.press(Primary, Pos2::new(r.left() - 2.0, y), Pos2::new(r.left() - 102.0, y));
        w.idle(2);
        assert_eq!(g(&w).width(), r.width() + 100.0, "the strip drags the divider");
    }

    /// «Параметры ОМП»'s keys (VM): Enter is OK — refused here with the
    /// dialog kept, the box's own Enter not taken by the dialog —, Esc
    /// cancels.
    #[test]
    fn the_omp_dialog_takes_enter_and_esc() {
        let mut w = Win::new();
        w.app.dispatch(app::Command::OmpParams);
        w.idle(2);
        w.app.omp_ui.dialog.as_mut().expect("the dialog").add_section();
        w.key(egui::Key::Enter);
        w.idle(1);
        assert!(w.app.omp_ui.message.is_some() && w.app.omp_ui.dialog.is_some(), "R = X = 0 refused, the dialog stays");
        w.key(egui::Key::Enter);
        w.idle(1);
        assert!(w.app.omp_ui.message.is_none() && w.app.omp_ui.dialog.is_some(), "the box's Enter closes only the box");
        w.key(egui::Key::Escape);
        w.idle(1);
        assert!(w.app.omp_ui.dialog.is_none());
    }

    /// Tab into a dialog's edit selects its text, so typing replaces it
    /// (VM: Tab into «Число гармоник», «0» → 0, not «100»).
    #[test]
    fn tab_into_an_edit_selects_its_text() {
        let mut w = Win::new();
        w.app.diag_cmd(diagram_ui::Window::Freq, diagram::DiagCmd::FreqSettings, Pos2::ZERO);
        w.idle(3);
        w.key(egui::Key::Tab);
        w.idle(1);
        w.run(vec![egui::Event::Text("0".into())]);
        w.idle(1);
        assert_eq!(w.app.diag.freq_dlg.as_ref().map(|d| d.harmonics.as_str()), Some("0"));
    }

    /// A popup item chosen with Enter opens its dialog, which does not take
    /// that Enter as its own OK (VM: the frequency menu's «Настройки»).
    #[test]
    fn a_menu_enter_does_not_reach_the_dialog_it_opens() {
        let mut w = Win::new();
        w.app.open_diag_menu(diagram_ui::Window::Freq, diagram_ui::MenuKind::Full, Pos2::new(300.0, 700.0));
        w.idle(1);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::Enter);
        w.idle(2);
        assert!(w.app.diag.freq_dlg.is_some(), "the dialog stays open");
    }

    /// The merge bar's close button (message-bar mode, `0x479960`): the bar
    /// goes, and with a pasted channel still pending the paste is undone.
    #[test]
    fn closing_the_merge_bar_undoes_a_pending_paste() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        let ai = w.app.active.unwrap();
        w.app.docs[ai].selection = [app::ChanKey::Analog(0)].into_iter().collect();
        w.app.copy_channels_for_merge();
        let n = w.app.docs[ai].computed.len();
        w.app.paste_channels_for_merge();
        w.idle(3);
        assert!(w.app.merge.is_some() && w.app.docs[ai].computed.len() == n + 1);
        let at = w.app.merge.as_ref().and_then(|m| m.close).expect("the close button drawn").center();
        w.press(Primary, at, at);
        w.idle(2);
        assert!(w.app.merge.is_none(), "the bar hidden");
        assert_eq!(w.app.docs[ai].computed.len(), n, "the paste undone");
    }

    /// The status bar's right-click menu (CMFCRibbonStatusBar): a click
    /// on an element's row hides it and the menu stays; Esc closes it; the
    /// workspace keeps the hidden ids, one pane's hiding all six (VM).
    #[test]
    fn the_status_bar_menu_hides_elements() {
        use egui::PointerButton::{Primary, Secondary};
        let mut w = Win::new();
        let at = Pos2::new(300.0, 860.0 - 8.0);
        w.press(Secondary, at, at);
        w.idle(2);
        assert!(w.app.status_menu.is_some());
        let menu = w.ctx.memory(|m| m.area_rect(egui::Id::new("fv_status_menu"))).expect("the menu shown");
        assert!(menu.bottom() <= 860.0 + 0.5, "opened upward from the bar");
        // The fourth element's row: under the 19 px title, 22 px rows.
        let row = Pos2::new(menu.center().x, menu.top() + 3.0 + 19.0 + 3.0 * 22.0 + 11.0);
        w.press(Primary, row, row);
        w.idle(2);
        assert_eq!(w.app.status_hidden, [false, false, false, true, false, false, false]);
        assert!(w.app.status_menu.is_some(), "the menu stays");
        w.key(egui::Key::Escape);
        w.idle(1);
        assert!(w.app.status_menu.is_none());
        // Saved as the ids (32838 for any pane): restored, all six hidden.
        w.app.status_hidden = [false; 7];
        w.app.store.view.insert("Workspace/StatusBar/Hidden".into(), "32838".into());
        w.app.restore_workspace();
        assert_eq!(w.app.status_hidden, [true, true, true, true, true, true, false]);
    }

    /// VM: opening «Быстрый анализ» and leaving it with Esc does not mark
    /// the record modified (no question at the close).
    #[test]
    fn the_quick_sheet_cancelled_leaves_the_record_unmodified() {
        let mut w = Win::new();
        w.app.docs.iter_mut().for_each(|d| d.set.modified = false);
        let ai = w.app.active.unwrap();
        w.idle(1);
        w.app.dispatch(app::Command::QuickAnalysis);
        w.idle(3);
        assert!(w.app.quick_open);
        w.key(egui::Key::Escape);
        w.idle(2);
        assert!(!w.app.quick_open);
        assert!(!w.app.docs[ai].set.modified);
    }

    /// A message box is modal: with the ОМП message up (VM, «Анализ» with
    /// nothing entered) Ctrl+C / Ctrl+V do nothing to the document.
    #[test]
    fn no_shortcuts_under_a_message_box() {
        let mut w = Win::new();
        let ai = w.app.active.unwrap();
        w.app.docs[ai].selection = [app::ChanKey::Analog(0)].into_iter().collect();
        w.app.omp_ui.message = Some("Данные для определения места повреждения не введены.".into());
        w.idle(2);
        w.run(vec![egui::Event::Copy]);
        w.idle(1);
        assert!(!w.app.clip.available(), "nothing copied");
        assert!(w.app.omp_ui.message.is_some());
    }

    /// Enter in an open drop-down picks its highlighted item and the
    /// dialog stays (VM: «Быстрый анализ»' arm combos; Enter used to run
    /// the dialog's default button).
    #[test]
    fn enter_in_an_open_drop_down_picks_and_keeps_the_dialog() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::FileParams);
        // The window settles where it is centred after its first frames.
        w.idle(5);
        let combo = w.ctx.read_response(egui::Id::new("fv_codepage")).expect("the encoding combo").rect.center();
        let before = w.app.docs[w.app.active.unwrap()].codepage;
        w.press(Primary, combo, combo);
        w.held = egui::Modifiers::NONE;
        w.key(egui::Key::ArrowUp);
        w.key(egui::Key::Enter);
        w.idle(2);
        assert!(w.app.show_file_params, "the dialog stays");
        assert_ne!(w.app.docs[w.app.active.unwrap()].codepage, before, "the item above picked");
        // Closed, Enter is the dialog's again.
        w.key(egui::Key::Enter);
        w.idle(2);
        assert!(!w.app.show_file_params);
    }

    /// The wheel over the ribbon steps its tabs and stops at either end
    /// (VM: two notches down from «Главная» → «ОМП», up twice → «Главная»).
    #[test]
    fn the_wheel_over_the_ribbon_steps_its_tabs() {
        let mut w = Win::new();
        let at = Pos2::new(300.0, 80.0);
        let notch = |w: &mut Win, d: f32| {
            w.run(vec![egui::Event::PointerMoved(at), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: Vec2::new(0.0, d), modifiers: egui::Modifiers::NONE }]);
            w.idle(1);
        };
        notch(&mut w, -2.0);
        assert_eq!(w.app.tab, app::RibbonTab::Omp, "down: the next, stopping at the last");
        notch(&mut w, 1.0);
        notch(&mut w, 1.0);
        assert_eq!(w.app.tab, app::RibbonTab::Home, "up: the previous, stopping at the first");
    }

    /// A right-click on a group's tab brings it to the front and opens the
    /// pane menu for that pane (VM).
    #[test]
    fn a_tab_right_click_opens_its_pane_menu() {
        let mut w = Win::new();
        let tab = w.app.pane_tab_rects.iter().find(|(p, _)| *p == app::Pane::Circle).map(|(_, r)| r.center()).expect("the circle tab");
        w.press(egui::PointerButton::Secondary, tab, tab);
        let menu = w.app.pane_menu.as_ref().map(|m| m.1);
        assert_eq!(menu, Some(app::Pane::Circle), "the tab's pane menu");
        assert!(w.app.pane_tab_rects.iter().any(|(p, _)| *p == app::Pane::Circle), "still tabbed");
        assert!(w.app.pane_canvas.iter().any(|(p, _)| *p == app::Pane::Circle), "in front");
    }

    /// IsDialogMessage: Enter is the default button, Esc cancels; Enter on a
    /// message box over the dialog closes only the box.
    #[test]
    fn dialogs_take_enter_and_escape() {
        let mut w = Win::new();
        w.app.diag_cmd(diagram_ui::Window::Freq, diagram::DiagCmd::FreqSettings, Pos2::ZERO);
        w.app.diag.freq_dlg.as_mut().unwrap().periods = "0".into();
        w.key(egui::Key::Enter);
        assert!(w.app.diag.freq_dlg.is_some() && w.app.dlg.alerts.len() == 1, "OK refused with a message");
        w.key(egui::Key::Enter);
        assert!(w.app.dlg.alerts.is_empty() && w.app.diag.freq_dlg.is_some(), "the box closes, the dialog stays");
        assert!(w.ctx.memory(|m| m.focused()).is_some(), "the refused field has the focus (CDataExchange::Fail)");
        w.key(egui::Key::Escape);
        assert!(w.app.diag.freq_dlg.is_none() && w.app.dlg.alerts.is_empty(), "Esc cancels");
    }

    /// The group's panes are units of their own (`0x474e10` creates four
    /// panes): Круговая's tab pulled out of the strip floats alone and
    /// docks beside the group on the five-way's left arrow; the
    /// differential tab dropped on its centre joins it — the vector,
    /// circle (behind) and differential planes then show at once.
    #[test]
    fn group_tabs_dock_as_panes_of_their_own() {
        use docking::{Edge, Place, Spot, Unit};
        let mut w = Win::new();
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let drag = |w: &mut Win, from: Pos2, target: &dyn Fn(&App) -> Pos2| {
            let out = Pos2::new(300.0, 450.0);
            w.run(vec![egui::Event::PointerMoved(from)]);
            w.run(vec![ev(from, true)]);
            for k in 1..=4 {
                w.run(vec![egui::Event::PointerMoved(from + (out - from) * (k as f32 / 4.0))]);
            }
            w.idle(2);
            let to = target(&w.app);
            w.run(vec![egui::Event::PointerMoved(to)]);
            w.idle(2);
            w.run(vec![ev(to, false)]);
            w.idle(3);
        };
        let tab = |w: &Win, p: app::Pane| w.app.pane_tab_rects.iter().find(|(q, _)| *q == p).map(|(_, r)| r.center()).expect("the tab");
        let side = |u: Unit, k: usize| move |a: &App| docking::five_way(a.dock_rects[u as usize].expect("the docked group"), true)[k].0.center();
        let from = tab(&w, app::Pane::Circle);
        drag(&mut w, from, &side(Unit::Vector, 0));
        assert_eq!((w.app.docks[Unit::Circle].place, w.app.docks[Unit::Circle].spot), (Place::Docked, Spot::Beside(Unit::Vector, Edge::Left)));
        assert_eq!(w.app.dock_views_shown(), 3, "Векторная | Дифференциальная, Круговая, Частотная");
        let rects = w.app.dock_rects;
        w.idle(4);
        assert_eq!(w.app.dock_rects, rects, "the slots keep their sizes");
        let drawn = |w: &Win, p: app::Pane| w.app.pane_canvas.iter().any(|(q, _)| *q == p);
        assert!(drawn(&w, app::Pane::Vector) && drawn(&w, app::Pane::Circle));
        let from = tab(&w, app::Pane::Differential);
        drag(&mut w, from, &side(Unit::Circle, 4));
        assert_eq!(w.app.docks[Unit::Differential].spot, Spot::Tab(Unit::Circle));
        assert_eq!(w.app.panes.active, app::Pane::Differential, "dropped in front");
        assert!(drawn(&w, app::Pane::Vector) && drawn(&w, app::Pane::Differential) && !drawn(&w, app::Pane::Circle));
        assert!(w.app.pane_tab_rects.iter().all(|(p, _)| *p != app::Pane::Vector), "Векторная alone: no strip");
        // Back to the circle tab: both planes keep their own state.
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.idle(2);
        assert_eq!(w.app.current_dimension(), fvb_core::model::Dimension::Ohm);
        assert!(drawn(&w, app::Pane::Circle));
    }

    /// Document tabs (`EnableMDITabbedGroups`, tab swap on): the second
    /// tab dragged over the first swaps with it and is in front; dragged
    /// out of the strip into the view it starts a second, vertical group
    /// (MFC's default when `OnShowMDITabContextMenu` returns FALSE, L9.263),
    /// each group showing its own document; a press in the other group's
    /// view activates its document; the tab dragged back onto the first
    /// group's strip joins it and the groups are one again.
    #[test]
    fn a_document_tab_dragged_over_another_swaps_with_it() {
        let mut w = Win { ctx: egui::Context::default(), app: demo(&[app::Command::Demo]), held: egui::Modifiers::NONE };
        w.idle(3);
        let serials = |w: &Win| w.app.docs.iter().map(|d| d.serial).collect::<Vec<_>>();
        let before = serials(&w);
        assert_eq!(w.app.doc_tab_rects.len(), 2);
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let drag = |w: &mut Win, from: Pos2, to: Pos2| {
            w.run(vec![egui::Event::PointerMoved(from), ev(from, true)]);
            for k in 1..=6 {
                w.run(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0))]);
            }
            w.run(vec![ev(to, false)]);
            w.idle(2);
        };
        let (a, b) = (w.app.doc_tab_rects[0], w.app.doc_tab_rects[1]);
        drag(&mut w, b.center(), Pos2::new(a.left() + 8.0, a.center().y));
        assert_eq!(serials(&w), [before[1], before[0]]);
        assert_eq!(w.app.active, Some(0), "the dragged tab in front");
        let a = w.app.doc_tab_rects[0];
        drag(&mut w, a.center(), a.center() + Vec2::new(0.0, 300.0));
        assert_eq!(w.app.group_count(), 2, "out of the strip: a new group");
        let moved = w.app.active.expect("the moved document active");
        assert_eq!((w.app.docs[moved].serial, w.app.docs[moved].mdi_group), (before[1], 1));
        let (g0, g1) = (w.app.mdi_rects[0], w.app.mdi_rects[1]);
        let wins = &w.app.mdi_wins;
        assert!(wins[0].right() == wins[1].left() && g0.top() == g1.top(), "side by side: {wins:?}");
        // Both views drawn: the first group's tab is laid out too.
        let other = 1 - moved;
        assert!(g0.contains(w.app.doc_tab_rects[other].center()) && g1.contains(w.app.doc_tab_rects[moved].center()));
        let p = Pos2::new(g0.center().x, g0.bottom() - 60.0);
        w.run(vec![egui::Event::PointerMoved(p), ev(p, true)]);
        w.run(vec![ev(p, false)]);
        w.idle(2);
        assert_eq!(w.app.active, Some(other), "a press in the other group's view");
        let t = w.app.doc_tab_rects[moved];
        // Left of the 40 px hot zone at the group's right edge.
        let onto = Pos2::new(w.app.mdi_strips[0].right() - 100.0, w.app.mdi_strips[0].center().y);
        drag(&mut w, t.center(), onto);
        assert_eq!(w.app.group_count(), 1, "back onto the first strip: one group");
        assert_eq!(w.app.docs[w.app.active.unwrap()].serial, before[1]);
    }

    /// A right-click on a document tab activates it and shows no menu (the
    /// frame's OnContextMenu, MFC's OnShowMDITabContextMenu).
    #[test]
    fn a_right_click_on_a_document_tab_activates_it() {
        use egui::PointerButton::Secondary;
        let mut w = Win { ctx: egui::Context::default(), app: demo(&[app::Command::Demo]), held: egui::Modifiers::NONE };
        w.idle(3);
        let other = 1 - w.app.active.unwrap();
        let at = w.app.doc_tab_rects[other].center();
        w.press(Secondary, at, at);
        assert_eq!(w.app.active, Some(other));
        assert!(!w.ctx.memory(|m| m.any_popup_open()), "no menu");
    }

    /// `0x100690b4`: a tab dragged along its strip over another swaps with
    /// it (the dragged one in front).
    #[test]
    fn a_tab_dragged_over_another_swaps_with_it() {
        let mut w = Win::new();
        let tab = |w: &Win, p: app::Pane| w.app.pane_tab_rects.iter().find(|(q, _)| *q == p).map(|(_, r)| *r).unwrap();
        let (circle, vector) = (tab(&w, app::Pane::Circle), tab(&w, app::Pane::Vector));
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let (from, to) = (circle.center(), Pos2::new(vector.left() + 6.0, vector.center().y));
        w.run(vec![egui::Event::PointerMoved(from), ev(from, true)]);
        for k in 1..=6 {
            w.run(vec![egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0))]);
        }
        w.run(vec![ev(to, false)]);
        w.idle(2);
        assert_eq!(w.app.tab_order[..2], [app::Pane::Circle, app::Pane::Vector]);
        assert_eq!(w.app.panes.active, app::Pane::Circle, "the dragged tab in front");
        assert_eq!(w.app.docks[docking::Unit::Vector].place, docking::Place::Docked, "inside the strip: no float");
    }

    /// Copy (`0x45be28` / `0x405160`): a picture of the view's client area
    /// then the clipboard document; with a diagram focused (a click in it),
    /// that diagram's picture alone.
    #[test]
    fn copy_pictures_the_view_or_the_focused_diagram() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        let (full, client) = w.app.view_rects.expect("the view was drawn");
        let stats = full.min + Vec2::new(40.0, 30.0);
        w.press(Primary, stats, stats);
        assert!(!w.app.active_doc().unwrap().selection.is_empty(), "a channel is selected");
        w.app.dispatch(app::Command::Copy);
        assert_eq!(w.app.clip.picture(), Some((client.width().round() as usize, client.height().round() as usize)));
        // `FV_COPY_DUMP=<file.png>` saves the view's picture for a look.
        if let Ok(path) = std::env::var("FV_COPY_DUMP") {
            let c = w.app.view_picture().unwrap();
            let rgba: Vec<u8> = c.px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
            std::fs::write(path, util::png_rgba(c.w as u32, c.h as u32, &rgba)).unwrap();
        }
        assert!(w.app.clip.get().is_some_and(|t| t.contains("<channels")), "the document follows the picture");
        // Focus the vector diagram.
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Vector).expect("the vector pane").1;
        w.press(Primary, canvas.center(), canvas.center());
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Vector));
        w.app.dispatch(app::Command::Copy);
        assert_eq!(w.app.clip.picture(), Some((canvas.width().round() as usize, canvas.height().round() as usize)));
        assert_eq!(w.app.clip.get(), None, "a diagram copy leaves no document");
        // Back to the view.
        w.press(Primary, stats, stats);
        assert_eq!(w.app.copy_focus, app::CopyFocus::View);
        w.app.dispatch(app::Command::Copy);
        assert!(w.app.clip.get().is_some());
        // The circle diagram's property grid: the document, no picture.
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.idle(2);
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        let at = grid.center();
        w.press(Primary, at, at);
        assert_eq!(w.app.copy_focus, app::CopyFocus::Grid);
        w.app.dispatch(app::Command::Copy);
        assert_eq!(w.app.clip.picture(), None, "no picture with the grid focused");
        assert!(w.app.clip.get().is_some_and(|t| t.contains("<channels")), "the document alone");
        // A press in the diagram below the grid gives the pane the focus.
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Circle).expect("the circle canvas").1;
        w.press(Primary, canvas.center(), canvas.center());
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Circle));
    }

    /// `FV_FRAME_BENCH=<file.cfg> cargo test --release frame_bench -- --nocapture`
    /// prints the mean whole-window frame time on that record.
    #[test]
    fn frame_bench() {
        let Ok(rec) = std::env::var("FV_FRAME_BENCH") else { return };
        let mut app = App::new();
        app.load_comtrade_path(std::path::Path::new(&rec));
        let mut ui_thread = std::time::Duration::ZERO;
        while !app.loading.is_empty() {
            let t = std::time::Instant::now();
            app.poll_loading();
            ui_thread += t.elapsed();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        eprintln!("open on the UI thread: {:.1} ms", ui_thread.as_secs_f64() * 1000.0);
        let ctx = egui::Context::default();
        let t0 = std::time::Instant::now();
        frames(&mut app, &ctx, Vec2::new(1320.0, 860.0), vec![vec![]; 1]);
        eprintln!("first frame: {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
        frames(&mut app, &ctx, Vec2::new(1320.0, 860.0), vec![vec![]; 2]);
        let n = 30;
        let t0 = std::time::Instant::now();
        frames(&mut app, &ctx, Vec2::new(1320.0, 860.0), vec![vec![egui::Event::PointerMoved(Pos2::new(400.0, 400.0))]; n]);
        eprintln!("frame: {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }

    /// `FV_WINDOW_DUMP=<file.png> cargo test window_dump` renders the window
    /// (demo document, a few recent files) after a click at
    /// `FV_WINDOW_CLICK=x,y` (default: the «Файл ▾» menu button), the
    /// pointer then resting at `FV_WINDOW_HOVER=x,y`.
    #[test]
    fn window_dump() {
        let Ok(path) = std::env::var("FV_WINDOW_DUMP") else { return };
        let pos = |var: &str| {
            let s = std::env::var(var).ok()?;
            let (x, y) = s.split_once(',')?;
            Some(Pos2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
        };
        let at = pos("FV_WINDOW_CLICK").unwrap_or(Pos2::new(48.0, 12.0));
        let hover = pos("FV_WINDOW_HOVER").unwrap_or(at);
        // `FV_WINDOW_STATE=<name>` dispatches commands first.
        use app::{Command as C, Pane, RibbonTab};
        let cmds = match std::env::var("FV_WINDOW_STATE").unwrap_or_default().as_str() {
            "analysis" => vec![C::QuickAnalysis],
            "circle" => vec![C::ActivatePane(Pane::Circle)],
            "circletable" | "circlelegend" => vec![C::ActivatePane(Pane::Circle)],
            "difftable" => vec![C::ActivatePane(Pane::Differential)],
            "diff" => vec![C::ActivatePane(Pane::Differential)],
            "omp" => vec![C::OmpAnalysis],
            "ompparams" => vec![C::OmpParams],
            "preview" => vec![C::PrintPreview],
            "print" => vec![C::Print],
            "pagesetup" => vec![C::PageSetup],
            "settings" => vec![C::Settings],
            "channels" => vec![C::ChannelsDialog],
            "coef" => vec![C::Coefficients],
            "fileparams" => vec![C::FileParams],
            "hdrcomment" => vec![C::HdrComment],
            "explorer" => vec![C::ToggleExplorer],
            "parambar" => vec![C::ToggleParamBar],
            "vizirs" => vec![C::ToggleVizir(0), C::ToggleVizir(1), C::ToggleVizir(2)],
            // `skin<k>`: the k-th look of `Skin::ALL`.
            s if s.starts_with("skin") => s[4..].parse::<usize>().ok().and_then(|k| theme::Skin::ALL.get(k).copied()).map(C::SetSkin).into_iter().collect(),
            _ => vec![],
        };
        // `FV_WINDOW_RECORD=<file.cfg>`: that record instead of the demo
        // (`none`: no document).
        let mut app = match std::env::var("FV_WINDOW_RECORD") {
            // `none`: the window with no document.
            Ok(rec) if rec == "none" => App::new(),
            Ok(rec) => {
                let mut app = App::new();
                app.load_comtrade_path(std::path::Path::new(&rec));
                while !app.loading.is_empty() {
                    app.poll_loading();
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                for c in &cmds {
                    app.dispatch(c.clone());
                }
                app
            }
            Err(_) => demo(&cmds),
        };
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "omp" || s == "ompparams") {
            app.tab = RibbonTab::Omp;
        }
        // `colordlg`: «Цвет канала» on the first channel's colour.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "colordlg") {
            app.color_dialog = app.active_doc().and_then(|d| crate::chandlg::rows(d).into_iter().next().map(|k| d.color_of(k)));
        }
        // `pairdlg`: the differential diagram's «Каналы ▸ Создать» (DIALOG 554).
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "pairdlg") {
            app.diag_cmd(diagram_ui::Window::Diff, diagram::DiagCmd::CreatePairs, Pos2::ZERO);
        }
        // `addformula`: «Быстрый анализ» with «Добавить новую формулу» over it.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "addformula") {
            app.dispatch(C::QuickAnalysis);
            app.open_formula_dlg();
            // `FV_ADDFORMULA_FORM=<dialog>`: the first tree item hosting it.
            if let Some(d) = std::env::var("FV_ADDFORMULA_FORM").ok().and_then(|v| v.parse::<u16>().ok()) {
                fn find(nodes: &[fvb_core::addformula::Node], path: &mut Vec<&'static str>, d: u16) -> Option<Vec<&'static str>> {
                    for n in nodes {
                        match *n {
                            fvb_core::addformula::Node::Leaf(l, f) if f == d => {
                                let mut p = path.clone();
                                p.push(l);
                                return Some(p);
                            }
                            fvb_core::addformula::Node::Group(l, k) => {
                                path.push(l);
                                let r = find(k, path, d);
                                path.pop();
                                if r.is_some() {
                                    return r;
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                }
                if let (Some(path), Some(st)) = (find(fvb_core::addformula::TREE, &mut Vec::new(), d), app.formula_dlg.as_mut()) {
                    st.select(path, Some(d));
                }
            }
        }
        // `ratealert`: «Параметры осциллограммы» with a refused rate's box over it.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "ratealert") {
            app.dispatch(C::FileParams);
            app.dlg.push_alert("Введите целое число.");
        }
        // `FV_WINDOW_SKIN=<win2000|officexp|winxp|office2003|vs2005|black|silver|aqua>`.
        if let Ok(name) = std::env::var("FV_WINDOW_SKIN") {
            use theme::Skin;
            let skin = match name.as_str() {
                "win2000" => Skin::Win2000,
                "officexp" => Skin::OfficeXp,
                "winxp" => Skin::WinXp,
                "office2003" => Skin::Office2003,
                "vs2005" => Skin::Vs2005,
                "black" => Skin::Black,
                "silver" => Skin::Silver,
                "aqua" => Skin::Aqua,
                _ => Skin::Blue,
            };
            app.dispatch(app::Command::SetSkin(skin));
        }
        // `FV_WINDOW_STATE=windows`: «Окна» over two documents; `twodocs`:
        // the two documents alone (the tabs' ▾ list).
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "windows" || s == "twodocs") {
            let osc = fvb_core::comtrade::parse(app::DEMO_CFG, app::DEMO_DAT).unwrap();
            app.add_doc(app::Doc::new(osc, "Подстанция 2".into()));
            if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "windows") {
                app.open_windows_dialog();
            }
        }
        // `FV_WINDOW_STATE=about`: «О программе».
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "about") {
            app.show_about = true;
        }
        // `FV_WINDOW_STATE=freqdlg`: the frequency diagram's settings.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "freqdlg") {
            app.diag_cmd(diagram_ui::Window::Freq, diagram::DiagCmd::FreqSettings, Pos2::ZERO);
        }
        // `FV_WINDOW_STATE=alert`: message 1086 over the window.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "alert") {
            app.dlg.push_alert(lanes::MSG_COMBINE_UNITS.to_string());
        }
        // `FV_WINDOW_STATE=comments`: a moment, a level line and an area —
        // and a level on the first digital lane (as a `.set` may hold).
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "comments") {
            use comments::{Comment, CommentKind};
            use app::ChanKey;
            if let Some(d) = app.active_doc_mut() {
                let mut m = Comment::new(CommentKind::Moment { pos: 60 });
                m.text = "Пуск".into();
                d.comments.push(m);
                d.comments.push(Comment::new(CommentKind::Level { chan: ChanKey::Analog(0), value: 0.0, from: 20, to: 50 }));
                d.comments.push(Comment::new(CommentKind::Area { chan_from: ChanKey::Analog(1), chan_to: ChanKey::Analog(2), from: 100, to: 160 }));
                d.comments.push(Comment::new(CommentKind::Level { chan: ChanKey::Digital(0), value: 0.3, from: 20, to: 120 }));
            }
        }
        // `FV_WINDOW_STATE=analysis`: two formula channels in the list.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "analysis") && std::env::var("FV_WINDOW_RECORD").is_err() {
            app.append_computed(&[("Uab".into(), "Ua-Ub".into()), ("P".into(), "Ua*Ia [W]".into())]);
            app.dispatch(app::Command::QuickAnalysis);
            app.formula_page.selected = Some(1);
        }
        // `FV_WINDOW_STATE=vecvalues`: the vector diagram's values table.
        // `FV_WINDOW_STATE=floating`: the diagram tab group in its own
        // window; `autohide`: pinned to the right strip and slid out;
        // `dockleft` / `dockbottom` / `docktab` / `dockmarkers` below.
        match std::env::var("FV_WINDOW_STATE").unwrap_or_default().as_str() {
            "floating" => {
                app.docks.leave(docking::Unit::Vector, |_| true);
                app.docks[docking::Unit::Vector].double_click(egui::pos2(60.0, 220.0));
            }
            // `groups`: the record open twice, the second copy's tab let go
            // over the view (a second, vertical tab group — at pass 2,
            // once the area is laid out).
            "groups" => {
                if let Some(path) = app.docs.first_mut().and_then(|d| d.path.take()) {
                    app.load_comtrade_path(&path);
                    while !app.loading.is_empty() {
                        app.poll_loading();
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    app.docs[0].path = Some(path);
                }
            }
            "autohide" => {
                app.docks.leave(docking::Unit::Vector, |_| true);
                let d = &mut app.docks[docking::Unit::Vector];
                d.pin();
                (d.out, d.slide) = (true, 1.0);
            }
            // Smart-docking arrangements (L9.101): the group on the left
            // edge; Частотная on the frame's bottom edge; Частотная tabbed
            // into the group (in front); Частотная dragged over the group
            // (the markers, the hot centre's preview — hover the pane);
            // Дифференциальная on its own beside the group (`docksplit`).
            "dockleft" => app.docks[docking::Unit::Vector].spot = docking::Spot::Edge { edge: docking::Edge::Left, outer: true },
            "dockbottom" => app.docks.drop_on(docking::Unit::Frequency, docking::Drop::Edge(docking::Edge::Bottom, true), None, 200.0),
            "docktab" => {
                app.docks[docking::Unit::Frequency].spot = docking::Spot::Tab(docking::Unit::Vector);
                app.dispatch(app::Command::ActivatePane(app::Pane::Frequency));
            }
            // An `.osc`-like trigger at sample 60 (the dotted line, the
            // ruler from it).
            "trigger" => {
                if let Some(d) = app.active_doc_mut() {
                    d.osc.trigger_sample = 60;
                }
            }
            "docksplit" => app.docks[docking::Unit::Differential].spot = docking::Spot::Beside(docking::Unit::Vector, docking::Edge::Left),
            "dockmarkers" => {
                app.docks[docking::Unit::Frequency].double_click(egui::pos2(80.0, 300.0));
                app.dock_drag = Some((docking::Unit::Frequency, egui::vec2(40.0, 10.0)));
                app.dock_drag_hold = true;
            }
            _ => {}
        }
        // `FV_WINDOW_STATE=keytips` / `keytipsmenu`: the tab-level keytips,
        // or the application menu opened by «F».
        // (Sent as keys after the click passes; a click ends keytip mode.)
        let kt_state = std::env::var("FV_WINDOW_STATE").unwrap_or_default();
        // `FV_WINDOW_STATE=difftable`: a differential pair (Iдиф the first
        // channel, Iторм the second) and its values table.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "difftable") {
            app.track_settings();
            app.diag.diff_values = true;
            if let Some(d) = app.active_doc_mut() {
                let (a, b) = (d.osc.analog[0].id.clone(), d.osc.analog[1].id.clone());
                d.dif_pairs.push(diagram::ChanPair::new(a, b));
                d.main_cursor = 300;
            }
        }
        // `FV_WINDOW_STATE=circletable`: the first channel and an R/X pair
        // of the first two on the circle diagram, the values table (mode 2);
        // `circlelegend`: the same with the legend instead.
        let circle_state = std::env::var("FV_WINDOW_STATE").unwrap_or_default();
        if circle_state == "circletable" || circle_state == "circlelegend" {
            app.track_settings();
            app.circle_readout = if circle_state == "circletable" { 2 } else { 0 };
            if let Some(d) = app.active_doc_mut() {
                d.chans[0].on_circle = true;
                let (a, b) = (d.osc.analog[0].id.clone(), d.osc.analog[1].id.clone());
                d.rx_pairs.push(diagram::ChanPair::new(a, b));
            }
        }
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "vecvalues") {
            app.track_settings();
            app.vec_show_values = true;
        }
        // `FV_WINDOW_STATE=paste`: the first analog and digital channels
        // copied and pasted back, 3 ms later (pending, bit 14).
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "paste") {
            use app::ChanKey;
            if let Some(d) = app.active_doc_mut() {
                d.selection.extend([ChanKey::Analog(0), ChanKey::Digital(0)]);
            }
            app.dispatch(app::Command::Copy);
            app.dispatch(app::Command::Paste);
            if let Some(d) = app.active_doc_mut() {
                d.set_merge_offset(0.003);
            }
        }
        // `FV_WINDOW_STATE=group`: the first three channels grouped, the
        // second with Max/Min too.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "group") {
            if let Some(d) = app.active_doc_mut() {
                for c in d.chans.iter_mut().take(3) {
                    c.group = Some(1);
                }
                if let Some(c) = d.chans.get_mut(1) {
                    c.stats.minmax = true;
                }
            }
        }
        // `FV_WINDOW_XSCALE=f`: the horizontal scale (25·f px per sample).
        if let Some(f) = std::env::var("FV_WINDOW_XSCALE").ok().and_then(|s| s.parse().ok()) {
            if let Some(d) = app.active_doc_mut() {
                d.xscale = Some(f);
            }
        }
        app.mru = vec!["/data/osc/2024-05-01 Подстанция 110 кВ.cfg".into(), "/data/osc/short.cff".into()];
        let size = pos("FV_WINDOW_SIZE").map_or(Vec2::new(1320.0, 860.0), |p| p.to_vec2());
        // `FV_WINDOW_BUTTON=right`: a right click (context menus).
        let button = if std::env::var("FV_WINDOW_BUTTON").is_ok_and(|b| b == "right") { egui::PointerButton::Secondary } else { egui::PointerButton::Primary };
        let ev = |pressed| egui::Event::PointerButton { pos: at, button, pressed, modifiers: Default::default() };
        let mut passes = vec![vec![], vec![], vec![egui::Event::PointerMoved(at), ev(true)], vec![ev(false)], vec![egui::Event::PointerMoved(hover)], vec![], vec![], vec![]];
        let key = |key| vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }];
        if kt_state == "keytips" || kt_state == "keytipsmenu" {
            passes.push(key(egui::Key::F10));
            if kt_state == "keytipsmenu" {
                passes.push(key(egui::Key::F));
            }
            passes.extend([vec![], vec![], vec![]]);
        }
        // `FV_WINDOW_KEYTIPS=tabs|p`: F10 (then P) over any state — the
        // print preview's keytips with `FV_WINDOW_LATE=preview`.
        if let Ok(k) = std::env::var("FV_WINDOW_KEYTIPS") {
            passes.extend([vec![], vec![]]);
            passes.push(key(egui::Key::F10));
            if k == "p" {
                passes.push(key(egui::Key::P));
            }
            passes.extend([vec![], vec![], vec![]]);
        }
        let fonts = app_fonts();
        // `FV_WINDOW_BLOCK=<n|name>`: the active diagram grid's block (0 =
        // «Нет»; with `FV_WINDOW_STATE=diff` the differential one), set
        // after the first passes have bound the grid to the document.
        let block = std::env::var("FV_WINDOW_BLOCK").ok();
        // `FV_WINDOW_LATE=preview`: the preview opened once the view is laid out.
        let late_preview = std::env::var("FV_WINDOW_LATE").is_ok_and(|v| v == "preview");
        let mut pass = 0;
        let paint = |ctx: &egui::Context| {
            if pass == 2 && late_preview {
                app.dispatch(app::Command::PrintPreview);
            }
            if pass == 2 && std::env::var("FV_WINDOW_STATE").is_ok_and(|v| v == "groups") && app.docs.len() > 1 {
                app.new_group_after(1, 0, true);
            }
            if pass == 2 {
                if let Some(b) = block.as_ref().and_then(|v| v.parse().ok().or_else(|| app.library.blocks.iter().position(|b| &b.name == v))) {
                    app.block_idx = b;
                    app.ensure_block_state();
                }
            }
            pass += 1;
            app.frame_ui(ctx)
        };
        // `FV_WINDOW_NATIVE=<scale>`: on a monitor at that display scale
        // (1.25 for 125 %), the size in device pixels.
        let c = match std::env::var("FV_WINDOW_NATIVE").ok().and_then(|s| s.parse::<f32>().ok()) {
            Some(native) => raster::render_native(size, native, |ctx| device_options(ctx, Some(native)), device_screen, Some(&fonts), passes, paint),
            None => raster::render_input(size, 1.0, Some(&fonts), passes, paint),
        };
        let rgba: Vec<u8> = c.px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        std::fs::write(path, util::png_rgba(c.w as u32, c.h as u32, &rgba)).unwrap();
    }
}


