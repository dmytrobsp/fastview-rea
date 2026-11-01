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
mod dpi;
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
mod sysdlg;
mod theme;
mod tips;
mod util;
mod views;
mod winframe;
mod windows_dlg;
mod winkeys;

use app::App;
use eframe::egui;

/// The frame's minimum tracking size (CMainFrame::OnGetMinMaxInfo
/// `0x4728a0`: 866 × 500, always, after MFC's base handler) is the window
/// rect with Windows 11's invisible borders; what shows of it (VM, by a
/// drag of an edge) is 852 × 493 — ours, with no system frame, as its
/// client. Physical pixels: the original is system-DPI aware.
const MIN_VISIBLE: [f32; 2] = [852.0, 493.0];

/// The size grip of a normal (not maximised) window, at 100 % in px, as the original's last status pane
/// holds it: 20 px beyond the extended pane's own width (VM: «Гор. визир 2:» shows from 988 px of client,
/// not 984).
const STATUS_GRIP: f32 = 20.0;
/// What a normal window reserves at its right edge for the grip when the extended pane is hidden: the
/// sixth pane shows from 1078 px of client, not 1074 (VM; 1048 of panes + 26…30).
/// At 125 % 32 px (fifth pane from 1010 px of client, not 1009: clone) and at 150 % 36…39 (the fourth from
/// between 954 and 958 px; 38 taken); the 100 % value is this constant.
const STATUS_GRIP_ALONE: f32 = 28.0;

/// The status bar's right end at `right` (the extended pane's width `ext` at this scale, its grip
/// not included): `(the extended pane's width with its grip, where the panes must end)`.
fn status_reserve(right: f32, ext: f32, maximised: bool, ext_hidden: bool) -> (f32, f32) {
    let (grip, alone) = if maximised { (0.0, 0.0) } else { (dpi::s(STATUS_GRIP), dpi::measured(STATUS_GRIP_ALONE, 32.0, 38.0)) };
    if ext_hidden {
        (ext, right - alone)
    } else {
        (ext + grip, right - ext - grip)
    }
}

/// The dots' pitch in px at the system DPI (measured at 150 %: 4).
fn status_grip_pitch() -> f32 {
    4.0
}

/// The status bar's text size in px. The original's is MFC's UI font (Segoe UI, the one the pane
/// templates are measured in); ours the nearest Tahoma by width: against the original's «Верт. визир 1:»
/// (73 / 86 / 107 px of ink on a clone at 100 / 125 / 150 %) 11 / 13 / 17 px (accepted difference #52).
fn status_font_px() -> f32 {
    dpi::measured(11.0, 13.0, 17.0)
}

/// The size grip's dots (the visual style's gripper, the original on a clone at 150 %): a triangle of 2 × 2
/// px dots on a 4 px pitch at the client's bottom-right corner, the lowest row and the rightmost column
/// longest; each dot has a light shadow one pixel down and right. `right` / `bottom` are the client's edges;
/// the dots as `(top-left x, y)`.
fn status_grip_dots(right: f32, bottom: f32, pitch: f32) -> Vec<egui::Pos2> {
    let mut out = Vec::new();
    for j in 0..3 {
        for i in 0..(3 - j) {
            out.push(egui::pos2(right - 3.0 - pitch * i as f32, bottom - 3.0 - pitch * j as f32));
        }
    }
    out
}

/// The resize direction a press at `p` starts, if any: the frameless window's edges (5 px, corners 12 px
/// along each edge) and, in a normal window, the status bar's size grip box as a south-east corner (the
/// original's grip resizes: a clone drag from its top dot).
fn resize_dir_at(own_caption: bool, grip: Option<egui::Rect>, screen: egui::Rect, p: egui::Pos2) -> Option<egui::viewport::ResizeDirection> {
    use egui::viewport::ResizeDirection as D;
    if grip.is_some_and(|g| g.contains(p)) {
        return Some(D::SouthEast);
    }
    if !own_caption {
        return None;
    }
    let (b, c) = (5.0, 12.0);
    let (l, r, t, bt) = (p.x - screen.left() < b, screen.right() - p.x < b, p.y - screen.top() < b, screen.bottom() - p.y < b);
    let (lc, rc, tc, bc) = (p.x - screen.left() < c, screen.right() - p.x < c, p.y - screen.top() < c, screen.bottom() - p.y < c);
    match () {
        _ if (t && lc) || (l && tc) => Some(D::NorthWest),
        _ if (t && rc) || (r && tc) => Some(D::NorthEast),
        _ if (bt && lc) || (l && bc) => Some(D::SouthWest),
        _ if (bt && rc) || (r && bc) => Some(D::SouthEast),
        _ if t => Some(D::North),
        _ if bt => Some(D::South),
        _ if l => Some(D::West),
        _ if r => Some(D::East),
        _ => None,
    }
}

/// A status pane's width: its template of 27 (40 for the wide ones) "_" in the UI font plus 18 px; the "_" is
/// 5 / 6 / 7 px at 100 / 125 / 150 % (clone: panes 153 / 180 / 207 and 218 / 258 / 298 px apart), the 18 not scaled.
fn status_pane_w(i: usize) -> f32 {
    let n = if i == 2 || i == 5 { 40.0 } else { 27.0 };
    n * dpi::measured(5.0, 6.0, 7.0) + 18.0
}

/// The extended pane's width: its text's width in the UI font + 27 (clone: 135 / 157 / 189 px at 100 / 125 /
/// 150 %); the older looks' 155 at 100 % keep their 20 more.
fn status_ext_w(bands: bool) -> f32 {
    dpi::measured(135.0, 157.0, 189.0) + if bands { 0.0 } else { 20.0 }
}

/// A pane's text starts this far into it (clone: ink 10 / 10 / 11 px from the pane's edge at 100 / 125 / 150 %).
fn status_text_inset() -> f32 {
    dpi::measured(9.0, 9.0, 10.0)
}

/// The status bar's six panes from `left` to `room` (the first that does not fit ends the row, a hidden
/// pane leaves no gap): `(index, x, width)` of each shown one.
fn status_pane_places(left: f32, room: f32, hidden: &[bool; 7]) -> Vec<(usize, f32, f32)> {
    let mut x = left;
    let mut out = Vec::new();
    for i in (0..6).filter(|&i| !hidden[i]) {
        let w = status_pane_w(i);
        if x + w > room {
            break;
        }
        out.push((i, x, w));
        x += w;
    }
    out
}

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

/// The saved window placement the first frame sends again: only where
/// the frame lays out in device pixels (`device_pixels` is its one reader;
/// elsewhere the runner's own placement in points stands, and the rect
/// would only sit in the app unused).
fn placement_to_resend(saved: Option<egui::Rect>, device_pixels: bool) -> Option<egui::Rect> {
    saved.filter(|_| device_pixels)
}

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

/// The document tabs' font size: MFC's tab font is the 8 pt one, whole
/// pixels at the system DPI (11, 13, 16 px; VM at 150 %: «WIN_1999.cfg»
/// bold 112 px wide, ours at 16 px 114, at 16.5 118), where the ribbon's
/// 9 pt text is [`dpi::font`].
fn tab_font_px() -> f32 {
    dpi::dlg_font(11.0)
}

/// A dpi (None: the call is missing or failed) as a display scale (dpi / 96).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn dpi_scale(dpi: Option<u32>) -> Option<f32> {
    dpi.filter(|&d| d > 0).map(|d| d as f32 / 96.0)
}

/// The DPI calls resolved at run time, as winit does: Windows 10 before
/// 1607 has no `GetDpiForSystem` / `GetDpiForWindow` /
/// `SetProcessDpiAwarenessContext` and Windows before 8.1 no shcore's
/// `GetDpiForMonitor`; a static import would stop the exe from starting
/// there ("entry point not found"). A missing one is `None`: egui's native
/// scale, as before they were used.
#[cfg(windows)]
mod dyn_dpi {
    use windows_sys::Win32::Foundation::{HWND, POINT};
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

    /// `name` (NUL-terminated) in `module` (NUL-terminated), loaded if need be.
    fn lookup(module: &[u8], name: &[u8]) -> Option<unsafe extern "system" fn() -> isize> {
        debug_assert!(module.ends_with(&[0]) && name.ends_with(&[0]));
        // SAFETY: NUL-terminated names; a loaded module stays loaded.
        unsafe {
            let m = LoadLibraryA(module.as_ptr());
            if m.is_null() {
                return None;
            }
            GetProcAddress(m, name.as_ptr())
        }
    }

    #[cfg(test)]
    pub fn find(module: &[u8], name: &[u8]) -> bool {
        lookup(module, name).is_some()
    }

    /// SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2), when there is one.
    pub fn set_per_monitor_aware() {
        type F = unsafe extern "system" fn(isize) -> i32;
        if let Some(f) = lookup(b"user32.dll\0", b"SetProcessDpiAwarenessContext\0") {
            // SAFETY: the documented signature; -4 is DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2.
            unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, F>(f)(-4) };
        }
    }

    /// GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI).x
    pub fn monitor_dpi(monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR) -> Option<u32> {
        type F = unsafe extern "system" fn(windows_sys::Win32::Graphics::Gdi::HMONITOR, i32, *mut u32, *mut u32) -> i32;
        let f = lookup(b"shcore.dll\0", b"GetDpiForMonitor\0")?;
        let (mut x, mut y) = (0u32, 0u32);
        // SAFETY: the documented signature; two local u32s.
        let hr = unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, F>(f)(monitor, 0, &mut x, &mut y) };
        (hr == 0).then_some(x)
    }

    pub fn system_dpi() -> Option<u32> {
        type F = unsafe extern "system" fn() -> u32;
        let f = lookup(b"user32.dll\0", b"GetDpiForSystem\0")?;
        // SAFETY: the documented signature.
        Some(unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, F>(f)() })
    }

    pub fn window_dpi(hwnd: HWND) -> Option<u32> {
        type F = unsafe extern "system" fn(HWND) -> u32;
        let f = lookup(b"user32.dll\0", b"GetDpiForWindow\0")?;
        // SAFETY: the documented signature; our own live window.
        Some(unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, F>(f)(hwnd) })
    }

    #[allow(dead_code)]
    pub fn point_origin() -> POINT {
        POINT { x: 0, y: 0 }
    }
}

/// The primary monitor's display scale (its effective dpi / 96, as winit
/// sizes a new window by), the process made per-monitor DPI aware first —
/// as winit makes it — so that Windows reports device pixels (the work
/// area too).
#[cfg(windows)]
fn system_scale() -> Option<f32> {
    use windows_sys::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};
    dyn_dpi::set_per_monitor_aware();
    // SAFETY: a query with a plain point.
    let monitor = unsafe { MonitorFromPoint(dyn_dpi::point_origin(), MONITOR_DEFAULTTOPRIMARY) };
    dpi_scale(dyn_dpi::monitor_dpi(monitor))
}

#[cfg(not(windows))]
fn system_scale() -> Option<f32> {
    None
}

/// The runner's screen in whole points when they are device pixels: at a
/// scale with no exact [`device_zoom`] (custom ones, 151 %) the pixels
/// per point are 1.0000001 and the window 1280 px wide 1279.9999 points.
fn device_screen(ctx: &egui::Context, raw: &mut egui::RawInput) {
    device_screen_on(DEVICE_PIXELS, ctx, raw);
}

/// [`device_screen`] for a platform that does (`device`) or does not paint
/// in device pixels.
fn device_screen_on(device: bool, ctx: &egui::Context, raw: &mut egui::RawInput) {
    let native = raw.viewports.get(&egui::ViewportId::ROOT).and_then(|v| v.native_pixels_per_point).or_else(|| ctx.input(|i| i.viewport().native_pixels_per_point));
    if let (true, Some(native), Some(r)) = (device, native, raw.screen_rect) {
        if ((ctx.zoom_factor() * native) - 1.0).abs() < 1e-6 {
            raw.screen_rect = Some(egui::Rect::from_min_max(r.min.round(), r.max.round()));
        }
    }
}

/// Device pixels from the first frame at the system `scale` (each frame
/// keeps them at the window's monitor's, `App::device_pixels`). No zoom by
/// Ctrl + / − / 0: the original has none.
#[cfg(test)]
fn device_options(ctx: &egui::Context, scale: Option<f32>) {
    device_options_on(DEVICE_PIXELS, ctx, scale);
}

fn device_options_on(device: bool, ctx: &egui::Context, scale: Option<f32>) {
    ctx.options_mut(|o| {
        o.zoom_factor = scale.filter(|_| device).map_or(1.0, device_zoom);
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
    device_window_on(DEVICE_PIXELS, ctx, native);
}

#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn device_window_on(device: bool, ctx: &egui::Context, native: f32) {
    if device {
        ctx.options_mut(|o| o.zoom_factor = device_zoom(native));
    }
}

/// A frame's zoom for the monitor the window is on (`native` pixels per
/// point there): one point one device pixel where the platform paints in
/// them, else egui's own (a Retina screen's 2) untouched.
fn follow_monitor(device: bool, ctx: &egui::Context, native: f32) {
    if device {
        let zoom = device_zoom(native);
        if ctx.zoom_factor() != zoom {
            ctx.set_zoom_factor(zoom);
        }
    }
}

/// The display scale of the monitor eframe's window is on (its dpi / 96).
#[cfg(windows)]
fn window_scale(w: &impl winit::raw_window_handle::HasWindowHandle) -> Option<f32> {
    use winit::raw_window_handle::RawWindowHandle;
    let RawWindowHandle::Win32(h) = w.window_handle().ok()?.as_raw() else { return None };
    dpi_scale(dyn_dpi::window_dpi(h.hwnd.get() as windows_sys::Win32::Foundation::HWND))
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
    // the work area less its borders (`first_run_client`; clone: the shown
    // 1906 × 1016 of a 2560 × 1392 work area at 100 %).
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

/// The system DPI / 96 (GetDpiForSystem: the primary monitor's at
/// sign-in), what a system-DPI-aware process — the original — lays its
/// chrome out for, whichever monitor its window is on and after a live
/// scale change until the next sign-in.
#[cfg(windows)]
fn system_dpi_scale() -> Option<f32> {
    dpi_scale(dyn_dpi::system_dpi())
}

#[cfg(not(windows))]
fn system_dpi_scale() -> Option<f32> {
    None
}

/// MFC's chrome at the system DPI (`dpi`), where ours paints in device
/// pixels (Windows); elsewhere egui's native scale already grows it all.
/// Fixed for the process, as the original's: on a monitor at another
/// scale ours keeps its content 1:1 (`device_window`) and the chrome at
/// the system DPI — the original's picture without Windows' stretch
/// (`docs/IMPROVEMENTS.md`).
#[cfg(test)]
fn chrome_dpi(scale: Option<f32>) {
    chrome_dpi_on(DEVICE_PIXELS, scale);
}

fn chrome_dpi_on(device: bool, scale: Option<f32>) {
    if let Some(s) = scale.filter(|_| device) {
        dpi::set((s * 96.0).round() as u32);
    }
}

/// What `make_app` does about scaling: egui's pixels per point from the
/// window's monitor `scale` ([`device_options_on`]), the chrome from the
/// system DPI (`system`, GetDpiForSystem) where there is one, else from
/// that same monitor scale.
fn start_scaling(device: bool, ctx: &egui::Context, scale: Option<f32>, system: Option<f32>) {
    device_options_on(device, ctx, scale);
    chrome_dpi_on(device, system.or(scale));
}

/// The application as both runners start it: the fonts, the persisted
/// state, the single-instance server, the command line.
fn make_app(ctx: &egui::Context, args: &StartupArgs, inst_dir: Option<&std::path::Path>, own_caption: bool, scale: Option<f32>, placement: Option<egui::Rect>) -> App {
    let fonts = app_fonts();
    ctx.set_fonts(fonts.clone());
    start_scaling(DEVICE_PIXELS, ctx, scale, system_dpi_scale());
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
    app.placement_px = placement_to_resend(placement, DEVICE_PIXELS);
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

/// A first run's frame on Windows (CW_USEDEFAULT) as the original's frame client
/// (#147): the window is ¾ of the work area wide and `¾ · height` less a cut high — 21, 25, 29, 34
/// px at 96, 120, 144, 168 dpi (clone, rects of the original's first-run frame: 960 × 543, 530, 517,
/// 503 of work areas 1280 × 752, 740, 728, 716); no formula in the metrics fits all four, so the
/// cut is that table, 4 px per 24 dpi between and beyond —, its client that less its sizing
/// borders (`WinMetrics::border` on each side, and below; the ribbon draws the caption row, so none
/// above): 944 × 535 at 100 %, 942 × 521, 938 × 506, 936 × 491. Ours is frameless, so this is what
/// the egui area takes, and the MDI area derived from it is the original's (230 × 140, 230 × 113,
/// 229 × 86 on a first run). Physical pixels, as the original (system-DPI aware).
#[cfg_attr(not(windows), allow(dead_code))]
fn first_run_client(work: [f32; 2], dpi: u32) -> [f32; 2] {
    let m = oscgeom::WinMetrics::at(dpi);
    let cut = match dpi {
        96 => 21.0,
        120 => 25.0,
        144 => 29.0,
        168 => 34.0,
        // 4 px per 24 dpi from the nearest measured end.
        d if d < 168 => (21.0 + (d as f32 - 96.0) / 6.0).round(),
        d => (34.0 + (d as f32 - 168.0) / 6.0).round(),
    };
    [(work[0] * 0.75).floor() - 2.0 * m.border(), (work[1] * 0.75).floor() - cut - m.border()]
}

#[cfg(windows)]
fn default_frame_size() -> Option<[f32; 2]> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: SPI_GETWORKAREA fills the RECT passed.
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut r as *mut RECT).cast(), 0) } != 0;
    let (w, h) = ((r.right - r.left) as f32, (r.bottom - r.top) as f32);
    let dpi = system_dpi_scale().map_or(96, |s| (s * 96.0).round() as u32);
    (ok && w > 0.0 && h > 0.0).then(|| first_run_client([w, h], dpi))
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
            // At > 100 % the strip, its font and the tabs' parts grow with
            // the system DPI (`dpi`; VM: the strip 22 / 31 / 35 px).
            let tab_h = strip.height() - 1.0;
            ui.spacing_mut().interact_size.y = tab_h;
            ui.spacing_mut().button_padding.y = 0.0;
            ui.add_space(dpi::s(6.0));
            for (k, &i) in members.iter().enumerate() {
                if self.docs[i].tab_color.is_none() {
                    let prev = k.checked_sub(1).and_then(|j| self.docs[members[j]].tab_color);
                    self.docs[i].tab_color = Some(self.tab_auto.assign(k, prev));
                }
                let active = self.active == Some(i);
                let title = self.docs[i].title();
                // MFC's tab font, 11 px (VM: «WIN_1999.cfg» 68 px); the
                // active document's in the bold face (`0x1013b543`).
                let size = tab_font_px();
                let font = if bold == Some(i) { ui.fonts(|f| crate::lanes::bold_font(f, size)).unwrap_or(egui::FontId::proportional(size)) } else { egui::FontId::proportional(size) };
                let text_w = ui.fonts(|f| f.layout_no_wrap(title.clone(), font.clone(), egui::Color32::BLACK).size().x);
                let w = text_w + dpi::s(34.0) + if active { dpi::s(20.0) } else { 0.0 };
                // An id per document: the drag follows the tab when it moves.
                // 19 px, over the row's bottom line (VM).
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, tab_h), egui::Sense::hover());
                let resp = ui.interact(rect, egui::Id::new(("fv_doc_tab", self.docs[i].serial)), egui::Sense::click_and_drag());
                tabs.push((i, rect));
                let (r, g, b) = app::TAB_COLORS[self.docs[i].tab_color.unwrap_or(0)];
                let base = egui::Color32::from_rgb(r, g, b);
                let top = base.lerp_to_gamma(egui::Color32::WHITE, if active { 0.45 } else { 0.25 });
                // Slanted left edge, square right edge (OneNote style).
                let pts = [rect.left_bottom(), rect.left_top() + egui::vec2(dpi::s(16.0), 0.0), rect.right_top(), rect.right_bottom()];
                let mut mesh = egui::Mesh::default();
                for (p, c) in pts.iter().zip([base, top, top, base]) {
                    mesh.colored_vertex(*p, if active { c } else { c.gamma_multiply(0.85) });
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(mesh));
                ui.painter().add(egui::Shape::line(pts.to_vec(), egui::Stroke::new(1.0f32, egui::Color32::from_gray(140))));
                let text_pos = egui::pos2(rect.left() + dpi::s(20.0), rect.center().y);
                ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, &title, font, egui::Color32::BLACK);
                if active {
                    let xr = egui::Rect::from_center_size(egui::pos2(rect.right() - dpi::s(12.0), rect.center().y), egui::vec2(dpi::s(14.0), dpi::s(14.0)));
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
                    let (dx, dy) = (3.5 * dpi::k(), 3.0 * dpi::k());
                    ui.painter().line_segment([c + egui::vec2(-dx, -dy), c + egui::vec2(dx, dy)], st);
                    ui.painter().line_segment([c + egui::vec2(dx, -dy), c + egui::vec2(-dx, dy)], st);
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
                ui.add_space(-dpi::s(6.0));
            }
            // The document menu's button: a bare (86, 125, 177) triangle,
            // 9 px across its top row 8 px down the strip, 5 rows to its
            // point, its right end 7 px in from the strip's (VM).
            let x0 = strip.right() - dpi::s(15.0);
            let y0 = strip.top() + 8.0 + ((strip.height() - 20.0) / 2.0).floor();
            let hit = egui::Rect::from_min_max(egui::pos2(x0 - 3.0, strip.top()), egui::pos2(x0 - 3.0 + dpi::s(15.0), strip.bottom()));
            let tri = egui::Color32::from_rgb(86, 125, 177);
            for k in 0..5 {
                let row = egui::Rect::from_min_max(egui::pos2(x0 + k as f32, y0 + k as f32), egui::pos2(x0 + 9.0 - k as f32, y0 + k as f32 + 1.0));
                ui.painter().rect_filled(row, 0.0, tri);
            }
            // The documents sorted by title, each with a small window
            // image, none marked; no «Окна...» (VM: MFC adds it to a
            // «Window» menu only, which this ribbon has not). An MFC popup
            // at the button's left under it; the button lit 15 px, hot
            // (255, 244, 202) → (255, 220, 122), pressed or open (252, 151, 61)
            // → (254, 181, 91) (VM).
            let list_id = ui.id().with("fv_doc_list");
            let btn = ui.interact(hit, list_id.with("btn"), egui::Sense::click());
            self.focus_other.push(hit);
            if let Some(b) = self.list_buttons.get_mut(g) {
                *b = hit;
            }
            // A window of the frame: a press gives it the keyboard focus,
            // where Space and Enter (egui's click by key) open the list (clone) and the arrows do
            // nothing; Esc only closes the list.
            if btn.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
                btn.request_focus();
            }
            if btn.has_focus() {
                ui.memory_mut(|m| m.set_focus_lock_filter(btn.id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }));
            }
            let mut open: Option<u64> = ui.data(|d| d.get_temp(list_id));
            // The list is an MFC popup menu: with it open the arrows walk it and Enter
            // picks the row (an Enter is no click on the button then).
            let hot_id = list_id.with("hot");
            let mut hot: Option<usize> = ui.data(|d| d.get_temp(hot_id));
            let list_keys = if open.is_some() && btn.has_focus() { ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown), i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))) } else { (false, false, false) };
            if btn.clicked() && !list_keys.2 {
                open = if open.is_some() { None } else { Some(ui.ctx().cumulative_pass_nr()) };
            }
            let lit = egui::Rect::from_min_size(egui::pos2(x0 - 3.0, (y0 + 2.0 - (dpi::s(15.0) / 2.0).ceil()).round()), egui::vec2(dpi::s(15.0), dpi::s(15.0)));
            // Lit (15 px) when hot, pressed or open (L9.265's residual, VM).
            if let Some((top, bottom)) = doc_menu_fill(open.is_some(), btn.is_pointer_button_down_on(), btn.hovered()) {
                crate::lanes::gradient(ui.painter(), lit, top, bottom);
                for k in 0..5 {
                    let row = egui::Rect::from_min_max(egui::pos2(x0 + k as f32, y0 + k as f32), egui::pos2(x0 + 9.0 - k as f32, y0 + k as f32 + 1.0));
                    ui.painter().rect_filled(row, 0.0, tri);
                }
            }
            if let Some(serial) = open {
                let mut order = members.clone();
                order.sort_by_key(|&i| self.docs[i].title().to_lowercase());
                let window = |p: &egui::Painter, r: egui::Rect, _c: egui::Color32| {
                    let w = egui::Rect::from_center_size(r.center(), egui::vec2(12.0, 10.0));
                    p.rect_stroke(w, 0.0, egui::Stroke::new(1.0f32, egui::Color32::from_gray(110)));
                    p.rect_filled(egui::Rect::from_min_max(w.left_top(), egui::pos2(w.left() + 3.0, w.bottom())), 0.0, egui::Color32::from_rgb(70, 130, 110));
                };
                let n = order.len();
                if n > 0 && (list_keys.0 || list_keys.1) {
                    hot = Some(match (hot, list_keys.1) {
                        (None, true) => 0,
                        (None, false) => n - 1,
                        (Some(h), true) => (h + 1) % n,
                        (Some(h), false) => (h + n - 1) % n,
                    });
                }
                if list_keys.2 {
                    activate = hot.and_then(|h| order.get(h).copied());
                }
                let area = egui::Area::new(list_id.with(serial)).kind(egui::UiKind::Menu).order(egui::Order::Foreground).fixed_pos(lit.left_bottom() - egui::vec2(0.0, 1.0)).constrain_to(crate::popmenu::bounds(ui.ctx())).show(ui.ctx(), |ui| {
                    crate::popmenu::mfc_frame(ui, |ui| {
                        crate::popmenu::cut_to_screen(ui, list_id.with(serial), |ui| {
                            for (k, i) in order.iter().copied().enumerate() {
                                if crate::popmenu::mark_item_icon(ui, &self.docs[i].title(), None, true, None, hot == Some(k), Some(&window)).clicked() {
                                    activate = Some(i);
                                }
                            }
                        })
                    })
                });
                let outside = ui.input(|inp| inp.pointer.any_pressed() && inp.pointer.interact_pos().is_some_and(|q| !area.response.rect.contains(q) && !hit.contains(q)));
                if activate.is_some() || outside || ui.input(|inp| inp.key_pressed(egui::Key::Escape)) {
                    open = None;
                }
            }
            self.doc_list_open |= open.is_some();
            ui.data_mut(|d| match hot.filter(|_| open.is_some()) {
                Some(h) => d.insert_temp(hot_id, h),
                None => d.remove::<usize>(hot_id),
            });
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
        // with no separators — 27 / 40 × "_" templates (`0x4729b0`), each
        // `status_pane_w` wide (the "_" 5 / 6 / 7 px, + 18) with the text
        // `status_text_inset` in, left-aligned — and the extended element,
        // `status_ext_w` wide (135 / 157 / 189 px, 20 more on the older
        // looks) on a darker band behind a light edge, its text 16 px in.
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
        // A bitmap of rows, uniform along x: row i's colour at r.top() + i
        // (stretched over a taller bar at > 100 %, `dpi::stretch_row`).
        let rows = |painter: &egui::Painter, r: egui::Rect, rows: &[u32; 24]| {
            let h = r.height().round() as usize;
            for i in 0..h {
                let c = rows[dpi::stretch_row(i, h, rows.len())];
                let col = egui::Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
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
        // The panes' widths (`status_pane_w`, `status_ext_w`) and the font
        // (`status_font_px`) at the system DPI. The ribbon status bar's font is MFC's UI font, ours the nearest Tahoma (clone, 100 / 125 / 150 %: «Верт. визир 1:» 73 / 86 /
        // 107 px wide: 11 / 13 / 17 px in ours; the ribbon's 12 / 15 / 18 is wider).
        let font = egui::FontId::proportional(status_font_px());
        // A normal (not maximised) window's last pane also holds the size grip
        // (see [`status_reserve`]).
        let maximised = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        let (ext_w, room) = status_reserve(r.right(), status_ext_w(p.bands.is_some()), maximised, self.status_hidden[6]);
        let hidden = self.status_hidden;
        // A hidden pane leaves no gap: the rest move left (VM). A pane that
        // does not fit before the extended one is not shown (VM, 150 % at
        // 1280 px: «Гор. визир 2:» and on gone).
        for (i, x, w) in status_pane_places(r.left(), room, &hidden) {
            let text = &s[i];
            let clip = egui::Rect::from_min_max(egui::pos2(x, r.top()), egui::pos2(x + w - 2.0, r.bottom()));
            painter.with_clip_rect(clip).text(egui::pos2(x + status_text_inset(), r.center().y), egui::Align2::LEFT_CENTER, text, font.clone(), p.status_text);
        }
        // The last pane: 135 px at the right (VM), its own rows and a light
        // and a dark column at its left in the Office 2007 styles.
        let ext = egui::Rect::from_min_max(egui::pos2(r.right() - ext_w, r.top() + 1.0), r.right_bottom());
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
            // Its text starts 16 px in (clone: ink 17 px from the pane's edge at every scale).
            painter.text(egui::pos2(ext.left() + 16.0, ext.center().y), egui::Align2::LEFT_CENTER, "НТЦ Механотроника", font, p.status_text);
        }
        // The size grip, in a normal window only (the visual style's gripper).
        self.status_grip = (!maximised).then(|| egui::Rect::from_min_max(egui::pos2(r.right() - 11.0, r.bottom() - 11.0), r.right_bottom()));
        if !maximised {
            let (dark, light) = if p.bands.is_some() && p.status_bg == egui::Color32::from_rgb(186, 212, 247) {
                (egui::Color32::from_rgb(69, 93, 128), egui::Color32::from_rgb(177, 201, 232))
            } else {
                (egui::Color32::from_gray(100), egui::Color32::WHITE)
            };
            for d in status_grip_dots(r.right(), r.bottom(), status_grip_pitch()) {
                let dot = egui::Rect::from_min_size(d, egui::vec2(2.0, 2.0));
                painter.rect_filled(dot.translate(egui::vec2(1.0, 1.0)), 0.0, light);
                painter.rect_filled(dot, 0.0, dark);
            }
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
        let pos = crate::popmenu::menu_pos(ctx, id, at);
        let mut toggle = None;
        let area = egui::Area::new(id).kind(egui::UiKind::Menu).order(egui::Order::Foreground).fixed_pos(pos).constrain_to(crate::popmenu::bounds(ctx)).show(ctx, |ui| {
            crate::popmenu::mfc_frame_with(ui, true, false, |ui| {
                crate::popmenu::title_band(ui, "Конфигурация строки состояния");
                crate::popmenu::cut_to_screen(ui, id, |ui| {
                    for (i, t) in texts.iter().map(String::as_str).chain(Some("НТЦ Механотроника")).enumerate() {
                        if i == 6 {
                            crate::popmenu::panel_separator(ui);
                        }
                        if crate::popmenu::mark_item_keys(ui, "", Some(t), true, (!self.status_hidden[i]).then_some("✓"), false).clicked() {
                            toggle = Some(i);
                        }
                    }
                });
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
        follow_monitor(DEVICE_PIXELS, ctx, native);
        if DEVICE_PIXELS {
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
        // A dialog disables the original's frame: no resizing from its border or its grip while one is up.
        if self.modal_up || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let screen = ctx.screen_rect();
        let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return };
        let dir = resize_dir_at(self.own_caption, self.status_grip, screen, p);
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
        winkeys::filter(ctx, raw);
        dialogs::take_dialog_tabs(ctx, raw);
    }

    /// An opaque clear colour: the DWM frame is extended into the client (`winframe`), where a
    /// translucent one could show through.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        let c = visuals.panel_fill;
        [f32::from(c.r()) / 255.0, f32::from(c.g()) / 255.0, f32::from(c.b()) / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(windows)]
        {
            minsize::install_on(_frame);
            if self.own_caption {
                winframe::install_on(_frame, ctx);
            }
        }
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
        // The picture is static: one wake, at the end, destroys it (the original's timer). A
        // repaint every 100 ms kept the software runner at ~12 frames a second for those 2 s (#33).
        let lead = f64::from(ctx.input(|i| i.predicted_dt));
        ctx.request_repaint_after(std::time::Duration::from_secs_f64((until - ctx.input(|i| i.time) + lead).max(0.0)));
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
        self.frame_ui_inner(ctx);
        // The channel drag's cursor is decided last: the targets have
        // said by then whether the pointer is over one (#73).
        self.finish_drag_cursor(ctx);
    }

    fn frame_ui_inner(&mut self, ctx: &egui::Context) {
        dialogs::track_input(ctx);
        dialogs::route_combo_keys(ctx);
        self.device_pixels(ctx);
        sysdlg::set_wake(ctx);
        self.poll_sysdlg(ctx);
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
        self.focus_other.clear();
        let was_modal = std::mem::replace(&mut self.modal_up, dialogs::window_up(ctx));
        winframe::set_modal(self.modal_up);
        // A closed dialog gives the activation back to the frame, which
        // hands the focus to its view (clone: a pane's or a window's focus
        // is the view's after the dialog; copy asks the focus window).
        if was_modal && !self.modal_up {
            self.copy_focus = app::CopyFocus::View;
        }
        // An MDI activation — a document picked from the ▾ list, opened or
        // closed — focuses the activated child's view (clone: a document's
        // tab, the list's pick, an Open). A system file dialog closed
        // unanswered leaves the focus where it was (clone: Ctrl+O, Esc).
        let doc = self.active_doc().map(|d| d.serial);
        if doc != self.focus_doc {
            self.copy_focus = app::CopyFocus::View;
        }
        self.focus_doc = doc;
        if self.modal_up {
            dialogs::backdrop(ctx, "fv_modal_backdrop", egui::Order::PanelResizeLine);
            self.keytips.exit();
        } else if !self.keytips_input(ctx) && self.print.preview.is_none() {
            // Print preview swaps table 128 for MFC's 30997 (`0x102474d9`).
            self.handle_keys(ctx);
        }

        let pal = self.skin.palette();
        // The auto-hide strips of the left, right and top edges take the
        // frame's outermost space, before the ribbon and the bars.
        self.strips_outer(ctx);
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
        // The original's ribbon is 2 px taller in a normal window than in a maximised one (clone,
        // the ribbon bar 148 / 215 / 154 px against 146 / 213 visible at 100 % / 150 %: the stack
        // above the MDI area is 223 / 290 normal, 221 / 288 maximised): ours puts them here.
        let normal_extra = if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) { 0.0 } else { 2.0 };
        egui::TopBottomPanel::top("fv_under_ribbon").exact_height(6.0 + normal_extra).show_separator_line(false).frame(egui::Frame::none().fill(strip)).show(ctx, |ui| {
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
            egui::TopBottomPanel::top("fv_ompbar").exact_height(omp_ui::omp_bar_h()).show_separator_line(false).frame(egui::Frame::none().fill(strip)).show(ctx, |ui| self.omp_banner(ui));
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


        // The bottom edge's strip is the last row, under the status row.
        self.strip_bottom(ctx);
        egui::TopBottomPanel::bottom("status")
            .exact_height(dpi::s(24.0))
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

/// The document menu button's face (VM): hot (255, 244, 202) → (255, 220,
/// 122) under the pointer, (252, 151, 61) → (254, 181, 91) while pressed or
/// while its list is open; none otherwise.
fn doc_menu_fill(open: bool, down: bool, hot: bool) -> Option<(egui::Color32, egui::Color32)> {
    let c = egui::Color32::from_rgb;
    if open || down {
        Some((c(252, 151, 61), c(254, 181, 91)))
    } else if hot {
        Some((c(255, 244, 202), c(255, 220, 122)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    /// #79 (VM, 100 %): in a narrow window the panes that fit before the last one stay, in order, and
    /// the rest are gone; a normal window's last pane holds the size grip (20 px beyond its own 135, and 28 when
    /// that pane is hidden), so «Гор. визир 2:» shows from 988 px of client (the original's 1004 px frame; not
    /// from 984, its 1000 px one) and «Разница г. визиров:» from 1078 with the last pane hidden (not 1074);
    /// a maximised window has no grip.
    #[test]
    fn a_narrow_status_bar_keeps_the_panes_that_fit() {
        let none = [false; 7];
        let count = |w: f32, maximised: bool, ext_hidden: bool| {
            let mut h = none;
            h[6] = ext_hidden;
            let (_, room) = status_reserve(w, 135.0, maximised, ext_hidden);
            status_pane_places(0.0, room, &h).len()
        };
        assert_eq!(count(1280.0, true, false), 6, "wide: all six");
        assert_eq!(count(850.0, false, false), 4, "the narrowest window: four");
        assert_eq!(count(984.0, false, false), 4, "the original's 1000 px frame (984 visible): the fifth is not there");
        assert_eq!(count(988.0, false, false), 5, "its 1004 px frame (988 visible): it is");
        assert_eq!(count(964.0, true, false), 4);
        assert_eq!(count(966.0, true, false), 5, "a maximised window has no grip");
        assert_eq!(count(1074.0, false, true), 5, "the last pane hidden: the grip is still there (1090 px frame)");
        assert_eq!(count(1078.0, false, true), 6, "its 1094 px frame");
        assert_eq!(count(1048.0, true, true), 6, "maximised: no grip");
        // A hidden pane leaves no gap.
        let mut hid = none;
        hid[1] = true;
        let (_, room) = status_reserve(850.0, 135.0, false, false);
        assert_eq!(status_pane_places(0.0, room, &hid)[1].1, 153.0);
    }

    use super::*;

    /// #139 (clone, 125 % / 150 %): the panes are 27 / 40 "_" templates of 5 / 6 / 7 px plus 18 px (153 / 180 /
    /// 207 and 218 / 258 / 298), the extended pane 135 / 157 / 189; at 125 % the fifth pane shows from 1160 px
    /// of client in a normal window and not from 1159 (the grip's reserve 25 px, bracketed on the clone).
    #[test]
    fn the_status_panes_follow_the_uis_underscore_at_every_scale() {
        for (d, a, b, ext) in [(96, 153.0, 218.0, 135.0), (120, 180.0, 258.0, 157.0), (144, 207.0, 298.0, 189.0)] {
            dpi::set(d);
            assert_eq!((status_pane_w(0), status_pane_w(2), status_ext_w(true)), (a, b, ext), "{d}");
        }
        dpi::set(120);
        let count = |w: f32| {
            let (_, room) = status_reserve(w, status_ext_w(true), false, false);
            status_pane_places(0.0, room, &[false; 7]).len()
        };
        assert_eq!((count(1159.0), count(1160.0)), (4, 5), "the fifth pane's bracket at 125 %");
        // The extended pane hidden: the reserve alone (clone, 125 %: from 1010 px of client, not 1009; 150 %: the
        // fourth pane from between 954 and 958).
        let alone = |w: f32| {
            let mut h = [false; 7];
            h[6] = true;
            let (_, room) = status_reserve(w, status_ext_w(true), false, true);
            status_pane_places(0.0, room, &h).len()
        };
        assert_eq!((alone(1009.0), alone(1010.0)), (4, 5), "the hidden-pane bracket at 125 %");
        dpi::set(144);
        assert_eq!((alone(953.0), alone(958.0)), (3, 4), "the hidden-pane bracket at 150 %");
        dpi::set(96);
    }

    /// #78: the document tabs' font is whole pixels at every scale.
    #[test]
    fn the_tab_font_is_whole_pixels() {
        for (d, px) in [(96, 11.0), (120, 13.0), (144, 16.0)] {
            dpi::set(d);
            assert_eq!(tab_font_px(), px, "{d}");
        }
        dpi::set(96);
    }

    /// A missing or failed DPI call is no scale (egui's native one stays).
    #[test]
    fn missing_dpi_call_is_no_scale() {
        assert_eq!(dpi_scale(None), None);
        assert_eq!(dpi_scale(Some(0)), None);
        assert_eq!(dpi_scale(Some(96)), Some(1.0));
        assert_eq!(dpi_scale(Some(144)), Some(1.5));
    }

    /// The runtime resolver finds what exists and answers `None` for what
    /// does not (an old Windows lacks the DPI calls), not a crash.
    #[cfg(windows)]
    #[test]
    fn dyn_dpi_resolver_falls_back() {
        assert!(dyn_dpi::find(b"user32.dll\0", b"GetWindowRect\0"));
        assert!(!dyn_dpi::find(b"user32.dll\0", b"NoSuchDpiCall\0"));
        assert!(!dyn_dpi::find(b"no-such-module.dll\0", b"GetDpiForSystem\0"));
    }

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
    /// scale (the next frame on). The chrome stays at a 96 dpi system here
    /// (`dpi`): its growth with the system DPI is
    /// `the_chrome_grows_with_the_system_dpi`.
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

    /// #12: started at 125 % / 150 % (the system DPI), MFC's chrome grows
    /// as the original's does on the VM — the ribbon body's bottom line at
    /// row 178 / 210 (143 at 100 %), the status bar 30 / 36 px (24), the
    /// pane captions 29 / 33 (20) — and at 150 % on a 1280 px window
    /// «Диаграммы» takes its small buttons and «Вычисления» its icon-only
    /// column, nothing collapses at 125 %.
    #[test]
    fn the_chrome_grows_with_the_system_dpi() {
        let px = Vec2::new(1280.0, 722.0);
        let fonts = app_fonts();
        let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
        let (line, status, caption) = (rgb(egui::Color32::from_rgb(157, 179, 209)), rgb(egui::Color32::from_rgb(0x56, 0x7D, 0xB0)), rgb(egui::Color32::from_rgb(184, 207, 233)));
        for (native, body, bar, cap, steps) in [(1.0, 143, 24, 20, [0, 0, 0, 0]), (1.25, 178, 30, 29, [0, 0, 0, 0]), (1.5, 210, 36, 33, [0, 1, 0, 1])] {
            chrome_dpi(Some(native));
            let mut app = demo(&[]);
            let mut got = [9usize; 4];
            let c = raster::render_native(px, native, |ctx| device_options(ctx, Some(native)), device_screen, Some(&fonts), vec![vec![]; 5], |ctx| {
                app.frame_ui(ctx);
                for (k, t) in ["Объединение", "Диаграммы", "Видимость", "Вычисления"].into_iter().enumerate() {
                    got[k] = ctx.data(|d| d.get_temp::<usize>(ribbon::group_step_id(t))).unwrap_or(9);
                }
            });
            let at = |x: usize, y: usize| c.px[y * c.w + x];
            let first = |x: usize, k: [u8; 3]| (0..c.h).find(|&y| at(x, y) == k);
            assert_eq!(first(1270, line), Some(body), "{native}: the ribbon's bottom line");
            let near = |p: [u8; 3], k: [u8; 3]| p.iter().zip(k).all(|(a, b)| a.abs_diff(b) <= 2);
            assert_eq!((0..c.h).rev().find(|&y| near(at(2, y), status)).map(|y| c.h - y), Some(bar), "{native}: the status bar");
            // A pane caption: its top line, then its face.
            let top = first(1100, caption).expect("a pane caption");
            let face = (top..c.h).take_while(|&y| at(1100, y) == caption).count();
            assert_eq!(face + 1, cap, "{native}: a pane caption");
            assert_eq!(got, steps, "{native}: the ribbon's reduction steps");
        }
        dpi::set(96);
    }

    /// #12: the ОМП result bar is CMFCCaptionBar's own height (created with
    /// −1, `0x474cbd`), 33 px at 100 % and its font's height more above —
    /// 41 at 150 % — its face's lines on its first and last rows.
    #[test]
    fn the_omp_bar_takes_its_fonts_height() {
        let px = Vec2::new(1280.0, 722.0);
        let fonts = app_fonts();
        let line = [157, 189, 216];
        for (native, h) in [(1.0, 33), (1.25, 37), (1.5, 41)] {
            dpi::set(96);
            chrome_dpi(Some(native));
            let mut app = demo(&[]);
            (app.omp_ui.bar_doc, app.omp_ui.bar) = (app.active, Some("    КЗ".into()));
            let c = raster::render_native(px, native, |ctx| device_options(ctx, Some(native)), device_screen, Some(&fonts), vec![vec![]; 5], |ctx| app.frame_ui(ctx));
            // The first two face lines down column 600: the bar's.
            let rows: Vec<usize> = (0..c.h).filter(|&y| c.px[y * c.w + 600] == line).take(2).collect();
            assert_eq!(rows.len(), 2, "{native}");
            assert_eq!(rows[1] - rows[0] + 1, h, "{native}: the ОМП bar");
        }
        dpi::set(96);
    }

    /// #139 (clone, 100 / 125 / 150 %): the status bar's text is the Tahoma size nearest the original's width,
    /// 11 / 13 / 17 px («Верт. визир 1:» 73 / 86 / 107 px of ink in the original; at 150 % 16 px is 5 px short
    /// and the ribbon's 18 px 10 over).
    #[test]
    fn the_status_bar_text_matches_the_originals_width() {
        for (d, px) in [(96, 11.0), (120, 13.0), (144, 17.0)] {
            dpi::set(d);
            assert_eq!(status_font_px(), px, "{d}");
        }
        // Through the frame at 150 %: the first pane's ink is within 3 px of the original's 107.
        dpi::set(96);
        chrome_dpi(Some(1.5));
        let fonts = app_fonts();
        let mut app = demo(&[]);
        let c = raster::render_native(Vec2::new(1280.0, 722.0), 1.5, |ctx| device_options(ctx, Some(1.5)), device_screen, Some(&fonts), vec![vec![]; 5], |ctx| app.frame_ui(ctx));
        let ink = |x: usize, y: usize| {
            let p = c.px[y * c.w + x];
            p[0] < 110 && p[2] > p[0] + 30
        };
        let xs: Vec<usize> = (0..150).filter(|&x| (c.h - 27..c.h).any(|y| ink(x, y))).collect();
        let width = (xs.last().unwrap() - xs.first().unwrap() + 1) as f32;
        assert!((width - 107.0).abs() <= 3.0, "ink {width} against the original's 107");
        dpi::set(96);
    }

    /// #138 (clone: a drag from the original's grip top dot resizes the window): a press on the grip's box
    /// resizes south-east, whatever the window's frame; a maximised window has no grip box.
    #[test]
    fn a_press_on_the_size_grip_resizes_south_east() {
        use egui::viewport::ResizeDirection as D;
        let screen = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0));
        let grip = Some(egui::Rect::from_min_max(Pos2::new(989.0, 577.0), Pos2::new(1000.0, 588.0)));
        assert_eq!(resize_dir_at(false, grip, screen, Pos2::new(992.0, 580.0)), Some(D::SouthEast), "the top dot's corner, 8 px from both edges");
        assert_eq!(resize_dir_at(false, grip, screen, Pos2::new(900.0, 580.0)), None);
        assert_eq!(resize_dir_at(false, None, screen, Pos2::new(992.0, 580.0)), None);
        assert_eq!(resize_dir_at(true, None, screen, Pos2::new(0.0, 300.0)), Some(D::West));
        let mut app = demo(&[]);
        let fonts = app_fonts();
        for (maximised, some) in [(false, true), (true, false)] {
            let hook = move |ctx: &egui::Context, raw: &mut egui::RawInput| {
                device_screen(ctx, raw);
                raw.viewports.entry(egui::ViewportId::ROOT).or_default().maximized = Some(maximised);
            };
            raster::render_native(Vec2::new(1280.0, 722.0), 1.0, |ctx| device_options(ctx, Some(1.0)), hook, Some(&fonts), vec![vec![]; 3], |ctx| app.frame_ui(ctx));
            assert_eq!(app.status_grip.is_some(), some, "maximised {maximised}");
        }
        assert!(app.status_grip.is_none());
    }

    /// #138 (clone, 150 %): a normal window's status bar draws the size grip, a triangle of six 2 px dots at
    /// its bottom-right corner on a 4 px pitch (the lowest row longest), each with a light shadow; a
    /// maximised window draws none. Drawn at 125 / 150 % through the whole frame.
    #[test]
    fn a_normal_window_draws_the_size_grip_and_a_maximised_one_does_not() {
        let dots = status_grip_dots(100.0, 50.0, 4.0);
        assert_eq!(dots.len(), 6);
        assert_eq!((dots[0], dots[2], dots[3], dots[5]), (egui::pos2(97.0, 47.0), egui::pos2(89.0, 47.0), egui::pos2(97.0, 43.0), egui::pos2(97.0, 39.0)));
        let dark = [69, 93, 128];
        let fonts = app_fonts();
        for (native, maximised) in [(1.25, false), (1.5, false), (1.5, true), (1.0, false)] {
            dpi::set(96);
            chrome_dpi(Some(native));
            let mut app = demo(&[]);
            let hook = move |ctx: &egui::Context, raw: &mut egui::RawInput| {
                device_screen(ctx, raw);
                raw.viewports.entry(egui::ViewportId::ROOT).or_default().maximized = Some(maximised);
            };
            let px = Vec2::new(1280.0, 722.0);
            let c = raster::render_native(px, native, |ctx| device_options(ctx, Some(native)), hook, Some(&fonts), vec![vec![]; 5], |ctx| app.frame_ui(ctx));
            let found: Vec<(usize, usize)> = (c.h - 60..c.h).flat_map(|y| (c.w - 14..c.w).map(move |x| (x, y))).filter(|&(x, y)| c.px[y * c.w + x] == dark).collect();
            if maximised {
                assert!(found.is_empty(), "{native}: a maximised window has no grip");
                continue;
            }
            assert_eq!(found.len(), 24, "{native}: six 2 × 2 dots");
            let (right, bottom) = (found.iter().map(|p| p.0).max().unwrap() + 2, found.iter().map(|p| p.1).max().unwrap() + 2);
            assert_eq!(right, c.w, "{native}: the dots end at the client's right edge");
            for d in status_grip_dots(right as f32, bottom as f32, status_grip_pitch()) {
                assert!(found.contains(&(d.x as usize, d.y as usize)), "{native}: a dot at {d:?}");
            }
        }
        dpi::set(96);
    }

    /// #138 / the final review: a press on a normal (decorated) window's size grip begins a south-east resize;
    /// while a dialog is up the original's frame is disabled and the press does nothing.
    #[test]
    fn the_size_grip_resizes_only_while_no_dialog_is_up() {
        let resizes = |dialog: bool| {
            let mut w = Win::new();
            w.app.own_caption = false;
            w.idle(3);
            if dialog {
                w.app.color_dialog = Some(egui::Color32::RED);
                w.idle(3);
            }
            assert_eq!(w.app.modal_up, dialog);
            let grip = w.app.status_grip.expect("a normal window has the grip");
            let at = grip.center();
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            let mut begun = false;
            for events in [vec![egui::Event::PointerMoved(at)], vec![ev(true)], vec![ev(false)]] {
                let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0))), time: Some(w.ctx.input(|i| i.time) + 0.05), events, ..Default::default() };
                let out = w.ctx.run(raw, |ctx| w.app.frame_ui(ctx));
                begun |= out.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::BeginResize(egui::viewport::ResizeDirection::SouthEast))));
            }
            begun
        };
        assert!(resizes(false), "the grip resizes a normal window");
        assert!(!resizes(true), "a dialog disables the frame");
    }

    /// #12: «Параметры ОМП» places its tree and grid as OnInitDialog
    /// (`0x4a7c50`) does, in pixels from its client: at 150 % (client 1069 ×
    /// 653) from row 55 under the 34 px toolbar, 563 px tall — down past
    /// its OK / Отмена row as on the VM — the grid from 356, 702 wide.
    #[test]
    fn the_omp_parameters_lay_out_from_their_client() {
        for (dpi, client, tree, grid) in [
            (96, (713.0, 447.0), (10.0, 44.0, 217.0, 357.0), (237.0, 44.0, 465.0, 357.0)),
            (120, (831.0, 550.0), (10.0, 50.0, 257.0, 460.0), (277.0, 50.0, 544.0, 460.0)),
            (144, (1069.0, 653.0), (10.0, 55.0, 336.0, 563.0), (356.0, 55.0, 702.0, 563.0)),
        ] {
            dpi::set(dpi);
            let size = dialogs::dlu_size(475, 275);
            assert_eq!((size.x, size.y), client, "{dpi}");
            let (t, g) = omp_ui::params_layout(egui::Rect::from_min_size(Pos2::ZERO, size));
            assert_eq!((t.left(), t.top(), t.width(), t.height()), tree, "{dpi}: the tree");
            assert_eq!((g.left(), g.top(), g.width(), g.height()), grid, "{dpi}: the grid");
        }
        dpi::set(96);
    }

    /// #12 on a monitor at another scale than the system's: the content
    /// 1:1 there (`device_window`), the chrome at the system DPI — the
    /// ribbon's bottom line on row 210 for a 150 % system on a 100 %
    /// monitor, 143 for a 100 % system on a 150 % one.
    #[test]
    fn the_chrome_keeps_the_system_dpi_on_another_monitor() {
        let px = Vec2::new(1280.0, 722.0);
        let fonts = app_fonts();
        let line = [157, 179, 209];
        for (system, monitor, body) in [(1.5, 1.0, 210), (1.0, 1.5, 143), (1.25, 1.5, 178)] {
            dpi::set(96);
            chrome_dpi(Some(system));
            let mut app = demo(&[]);
            let mut ppp = 0.0;
            let setup = |ctx: &egui::Context| {
                device_options(ctx, Some(system));
                device_window(ctx, monitor);
            };
            let c = raster::render_native(px, monitor, setup, device_screen, Some(&fonts), vec![vec![]; 5], |ctx| {
                app.frame_ui(ctx);
                ppp = ctx.pixels_per_point();
            });
            assert_eq!(ppp, 1.0, "system {system}, monitor {monitor}");
            let row = (0..c.h).find(|&y| c.px[y * c.w + 1270] == line);
            assert_eq!(row, Some(body), "system {system}, monitor {monitor}: the ribbon's bottom line");
        }
        dpi::set(96);
    }

    /// #80: off Windows (`DEVICE_PIXELS` false: a Mac's Retina 2×, a Linux
    /// HiDPI screen) egui keeps its native scale and the chrome its 100 %
    /// sizes: no device zoom, the screen not rounded, no chrome dpi, the
    /// window's monitor changing nothing. On Windows (true) the same calls
    /// give one pixel per point. `DEVICE_PIXELS` is true in the test build,
    /// so the other branch is reached through the `_on` functions.
    #[test]
    fn off_windows_egui_keeps_its_native_scale() {
        let px = Vec2::new(1100.0, 700.0);
        for native in [1.25, 2.0] {
            dpi::set(96);
            let ctx = egui::Context::default();
            start_scaling(false, &ctx, Some(native), Some(native));
            assert_eq!((ctx.zoom_factor(), dpi::dpi()), (1.0, 96), "{native}: nothing started");
            device_window_on(false, &ctx, native);
            follow_monitor(false, &ctx, native);
            assert_eq!(ctx.zoom_factor(), 1.0, "{native}: nothing followed");
            let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(549.7, 350.3))), ..Default::default() };
            raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(native);
            device_screen_on(false, &ctx, &mut raw);
            assert_eq!(raw.screen_rect, Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(549.7, 350.3))), "{native}: the screen as given");
            // A frame: egui's own pixels per point.
            let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, px / native)), ..Default::default() };
            raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(native);
            let ppp = ctx.run(raw, |_| {}).pixels_per_point;
            assert_eq!(ppp, native, "{native}: egui's scale");
            // The same calls on Windows: one pixel per point, the chrome grown.
            start_scaling(true, &ctx, Some(native), Some(native));
            assert_eq!(ctx.zoom_factor(), device_zoom(native), "{native}: device pixels");
            assert_eq!(dpi::dpi(), (native * 96.0).round() as u32);
        }
        dpi::set(96);
    }

    /// #80: `make_app`'s scaling — device pixels at the window's monitor's
    /// scale, the chrome at GetDpiForSystem's (the system's, whichever
    /// monitor the window opens on), the monitor's own when the call is
    /// missing (Windows 10 before 1607) and 100 % when both are.
    #[test]
    fn make_app_takes_the_chrome_dpi_from_the_system() {
        for (monitor, system, dpi_want, zoom_of) in [
            (Some(1.0), Some(1.5), 144, 1.0),
            (Some(1.75), Some(1.25), 120, 1.75),
            (Some(1.25), None, 120, 1.25),
            (None, Some(1.5), 144, 0.0),
            (None, None, 96, 0.0),
        ] {
            dpi::set(96);
            let ctx = egui::Context::default();
            start_scaling(true, &ctx, monitor, system);
            assert_eq!(dpi::dpi(), dpi_want, "monitor {monitor:?}, system {system:?}");
            let zoom = if zoom_of == 0.0 { 1.0 } else { device_zoom(zoom_of) };
            assert_eq!(ctx.zoom_factor(), zoom, "monitor {monitor:?}, system {system:?}: the zoom follows the monitor");
        }
        dpi::set(96);
    }

    /// #80: a live change — the window dragged to a monitor at another
    /// scale (or the scale changed), the runner reporting the new native
    /// scale and Windows resizing the window to keep its size in points —
    /// settles within a frame to device pixels again (one pixel per
    /// point), the chrome staying at the system DPI (fixed for the
    /// process: the ribbon's bottom line on row 143 / 178 / 210), and
    /// stays there: a further frame paints the same picture. Headless this
    /// is the egui side only: the real WM_DPICHANGED resize and a
    /// monitor's own rendering need hardware.
    #[test]
    fn a_live_monitor_change_settles_to_device_pixels_and_the_system_chrome() {
        let px = Vec2::new(1100.0, 700.0);
        let fonts = app_fonts();
        let line = [157, 179, 209];
        for (system, from, to, body) in [(1.0, 1.0, 1.5, 143), (1.25, 1.25, 1.75, 178), (1.5, 1.5, 1.0, 210), (1.0, 1.5, 1.25, 143)] {
            dpi::set(96);
            chrome_dpi(Some(system));
            let after = (px * (to / from)).round();
            let shot = |frames_after: usize| {
                let setup = |ctx: &egui::Context| {
                    device_options(ctx, Some(system));
                    device_window(ctx, from);
                };
                let passes: Vec<(f32, Vec2, Vec<egui::Event>)> = [(from, px), (from, px)].into_iter().chain(std::iter::repeat_n((to, after), frames_after)).map(|(n, p)| (n, p, vec![])).collect();
                let (mut ppp, mut app) = (0.0, demo(&[]));
                let c = raster::render_natives(setup, device_screen, Some(&fonts), passes, |ctx| {
                    app.frame_ui(ctx);
                    ppp = ctx.pixels_per_point();
                });
                (c, ppp)
            };
            let (c, ppp) = shot(3);
            assert_eq!(ppp, 1.0, "{from} -> {to}: device pixels again");
            assert_eq!((c.w, c.h), (after.x as usize, after.y as usize));
            let row = (0..c.h).find(|&y| c.px[y * c.w + c.w - 10] == line);
            assert_eq!(row, Some(body), "system {system}, {from} -> {to}: the ribbon's bottom line");
            let (later, _) = shot(6);
            assert!(later.px == c.px, "system {system}, {from} -> {to}: the picture moves after it settled");
        }
        dpi::set(96);
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

    /// The saved placement is kept for the first frame only where device
    /// pixels are used; elsewhere nothing reads it (#83).
    #[test]
    fn the_saved_placement_is_kept_only_where_it_is_read() {
        let r = Rect::from_min_size(Pos2::new(40.0, 30.0), Vec2::new(900.0, 700.0));
        assert_eq!(placement_to_resend(Some(r), true), Some(r));
        assert_eq!(placement_to_resend(Some(r), false), None);
        assert_eq!(placement_to_resend(None, true), None);
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

    /// The Help button's key tip «A» opens its drop-down with the first row
    /// highlighted (MFC posts Home; clone: «Помощь» lit, ↓ then «О программе...»,
    /// Home back) (#26).
    #[test]
    fn the_help_menu_opened_by_its_key_tip_has_its_first_row_lit() {
        let mut app = demo(&[]);
        let ctx = egui::Context::default();
        let size = Vec2::new(1320.0, 860.0);
        let key = |key| vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }];
        frames(&mut app, &ctx, size, vec![vec![], key(egui::Key::F10), vec![]]);
        frames(&mut app, &ctx, size, vec![key(egui::Key::A), vec![], vec![]]);
        let (_, menu) = app.top_menu.expect("the Help menu is open");
        assert_eq!(menu.hot, Some(0), "the first row is lit");
        frames(&mut app, &ctx, size, vec![key(egui::Key::ArrowDown), vec![]]);
        assert_eq!(app.top_menu.unwrap().1.hot, Some(2), "↓ goes to «О программе...»");
        frames(&mut app, &ctx, size, vec![key(egui::Key::Home), vec![]]);
        assert_eq!(app.top_menu.unwrap().1.hot, Some(0), "Home back to the first");
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
    /// end too (VM, L9.398: the first tick past a record's end is labelled).
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
        w.app.active_doc_mut().unwrap().set.modified = false;
        w.press(Primary, at, at);
        w.key(egui::Key::F4);
        w.idle(1);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::Enter);
        w.idle(2);
        // Nothing is saved for a keyed-in block: the document stays unmodified
        // (clone: closed right after, no save prompt) — #24's SelectedBlock.
        assert!(!w.app.active_doc().unwrap().set.modified);
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

    /// A row selected by a key comes into view (CMFCPropertyGridCtrl's
    /// EnsureVisible; the grid's rows scroll by whole rows): End on a grid
    /// taller than its window scrolls to the last row, Home back (#24).
    #[test]
    fn a_key_selected_grid_row_scrolls_into_view() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.app.store.view.insert("Circle Diagram\\Properties Height".into(), "68".into());
        w.idle(2);
        let b = (0..w.app.library.blocks.len()).find(|&b| w.app.library.blocks[b].characteristics.len() >= 2 && !w.app.library.blocks[b].characteristics[0].parameters.is_empty()).expect("a block with two characteristics");
        w.app.block_idx = b;
        w.app.ensure_block_state();
        w.idle(3);
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        let at = Pos2::new(grid.left() + 60.0, grid.top() + 1.0 + 8.0);
        w.press(Primary, at, at);
        let sid = egui::Id::new(("char_grid_scroll", app::Pane::Circle));
        let state = |w: &Win| w.ctx.data(|d| d.get_temp::<(f32, f32)>(sid)).expect("the grid's scroll state");
        let (_, content) = state(&w);
        assert!(content > grid.height(), "the rows overflow the window: {content} in {}", grid.height());
        w.key(egui::Key::End);
        w.idle(3);
        let (off, content) = state(&w);
        assert!(off + grid.height() >= content - 17.0, "the last row is in view: offset {off}, rows {content}, window {}", grid.height());
        w.key(egui::Key::Home);
        w.idle(3);
        assert_eq!(state(&w).0, 0.0, "Home scrolls back to the first row");
    }

    /// PgDn / PgUp in the parameter grid move the selection and the window by
    /// a page, P - 1 rows with P in view; the wheel and the bar reach
    /// N - P + 2 rows down (clone, a block's grid: PgDn from the top went ten
    /// rows with eleven in view and scrolled to the range's end, the second
    /// PgDn to the last row, PgUp back to the top) (#24).
    #[test]
    fn page_keys_and_the_scroll_range_of_the_grid() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.app.store.view.insert("Circle Diagram\\Properties Height".into(), "120".into());
        w.idle(2);
        let b = (0..w.app.library.blocks.len()).find(|&b| w.app.library.blocks[b].characteristics.len() >= 2 && !w.app.library.blocks[b].characteristics[0].parameters.is_empty()).expect("a block with two characteristics");
        w.app.block_idx = b;
        w.app.ensure_block_state();
        w.idle(3);
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        let at = Pos2::new(grid.left() + 60.0, grid.top() + 1.0 + 8.0);
        w.press(Primary, at, at);
        let sid = egui::Id::new(("char_grid_scroll", app::Pane::Circle));
        let state = |w: &Win| w.ctx.data(|d| d.get_temp::<(f32, f32)>(sid)).expect("the grid's scroll state");
        let rows_in_view = w.ctx.data(|d| d.get_temp::<i32>(egui::Id::new(("fv_grid_rows_in_view", app::Pane::Circle)))).expect("the rows in view") as f32;
        let (_, content) = state(&w);
        let row = 17.0;
        let max = content - (rows_in_view - 2.0) * row;
        assert!(content > rows_in_view * row);
        w.key(egui::Key::PageDown);
        w.idle(3);
        assert_eq!(state(&w).0, ((rows_in_view - 1.0) * row).min(max), "a page down scrolls a page");
        for _ in 0..80 {
            w.key(egui::Key::PageDown);
        }
        w.idle(3);
        assert_eq!(state(&w).0, max, "the end of the range lies N - P + 2 rows down");
        w.key(egui::Key::PageUp);
        w.idle(3);
        assert_eq!(state(&w).0, (max - (rows_in_view - 1.0) * row).max(0.0), "a page up");
        // The wheel reaches the same end.
        for _ in 0..40 {
            w.wheel(Pos2::new(grid.left() + 60.0, grid.top() + 30.0));
        }
        w.idle(2);
        assert_eq!(state(&w).0, max, "the wheel's end");
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

    /// #147: a first run's frame client (ours has no frame, so its egui area) is the original's
    /// at every display scale — ¾ of the work area less the sizing borders, VM clones: the client
    /// 942 × 521 at 125 % (work area 1280 × 740), 938 × 506 at 150 % (1280 × 728), 936 × 491 at 175 %.
    #[test]
    fn a_first_runs_frame_is_the_originals_client_at_every_scale() {
        assert_eq!(first_run_client([1280.0, 752.0], 96), [944.0, 535.0]);
        assert_eq!(first_run_client([1280.0, 740.0], 120), [942.0, 521.0]);
        assert_eq!(first_run_client([1280.0, 728.0], 144), [938.0, 506.0]);
        assert_eq!(first_run_client([1280.0, 716.0], 168), [936.0, 491.0]);
        // 200 %: the cut 34 + 4 = 38 px, the borders 5 + 8 = 13.
        assert_eq!(first_run_client([1280.0, 704.0], 192), [934.0, 477.0]);
        // The MDI area that follows: a quarter of it less 6, rounded up (230, 230, 229 on the VM).
        for (dpi, w) in [(96, 230.0), (120, 230.0), (144, 229.0)] {
            let c = first_run_client([1280.0, 740.0], dpi)[0];
            assert_eq!((c / 4.0 - 6.0).ceil(), w, "dpi {dpi}");
        }
    }

    /// #147: on a first run's frame ours' MDI area is the original's at every display scale (clone,
    /// the original's MDIClient from its first-run frame: 230 × 140 at 100 %, 230 × 113 at 125 %,
    /// 229 × 86 at 150 %; its top 223 / 290 below a ribbon 2 px taller than a maximised one's).
    #[test]
    fn a_first_runs_mdi_area_is_the_originals_at_every_scale() {
        for (dpi, work, want) in [(96u32, [1280.0, 752.0], (230.0, 140.0)), (120, [1280.0, 740.0], (230.0, 113.0)), (144, [1280.0, 728.0], (229.0, 86.0))] {
            dpi::set(dpi);
            let ctx = egui::Context::default();
            let mut app = demo(&[]);
            app.own_caption = true;
            let [w, h] = first_run_client(work, dpi);
            frames(&mut app, &ctx, Vec2::new(w, h), vec![vec![]; 4]);
            assert_eq!((app.mdi_area.width(), app.mdi_area.height()), want, "dpi {dpi}: {:?}", app.mdi_area);
        }
    }

    /// #147: restored maximised after the first run, ours' MDI area is the original's (clone:
    /// 566 × 359, 568 × 334, 571 × 310 at 100 / 125 / 150 %; the maximised frame's client the work
    /// area, Частотная keeping the half of the first-run span).
    #[test]
    fn a_maximised_frames_mdi_area_is_the_originals_at_every_scale() {
        for (dpi, work, want) in [(96u32, [1280.0, 752.0], (566.0, 359.0)), (120, [1280.0, 740.0], (568.0, 334.0)), (144, [1280.0, 728.0], (571.0, 310.0))] {
            dpi::set(dpi);
            let ctx = egui::Context::default();
            let mut app = demo(&[]);
            app.own_caption = true;
            let first = first_run_client(work, dpi);
            frames(&mut app, &ctx, Vec2::new(first[0], first[1]), vec![vec![]; 4]);
            for _ in 0..4 {
                let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(work[0], work[1]))), time: Some(ctx.input(|i| i.time) + 0.05), ..Default::default() };
                raw.viewports.insert(raw.viewport_id, egui::ViewportInfo { maximized: Some(true), ..Default::default() });
                let _ = ctx.run(raw, |ctx| app.frame_ui(ctx));
            }
            assert_eq!((app.mdi_area.width(), app.mdi_area.height()), want, "dpi {dpi}: {:?}", app.mdi_area);
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
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(10);
        assert!(w.app.docks[Unit::Vector].out, "the hover slid it out");
        w.idle(14);
        assert!(canvas(&w).is_some(), "drawn over the content");
    }

    /// A floating pane is an ordinary tool window, not a dialog (#70): the
    /// frame keeps its shortcuts and its view takes input, and a press on
    /// the window's caption, tab strip or toolbar over the view does not
    /// hand the focus to the view.
    #[test]
    fn a_floating_pane_is_no_dialog() {
        use docking::{Place, Unit};
        let mut w = Win::new();
        let canvas = |w: &Win| w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Vector).map(|(_, r)| *r);
        let docked = canvas(&w).expect("the vector pane is drawn");
        let at = Pos2::new(docked.left() + 60.0, docked.top() - 10.0);
        let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, modifiers: Default::default(), pressed };
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(true), ev(false)]);
        w.run(vec![ev(true), ev(false)]);
        w.idle(3);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating);
        assert!(!w.app.modal_up, "a floating pane is no dialog");
        let win = w.ctx.memory(|m| m.area_rect(egui::Id::new(("fv_floating_pane", Unit::Vector)))).expect("the floating window");
        // The view takes input beside the window.
        let free = [Pos2::new(700.0, 470.0), Pos2::new(700.0, 330.0), Pos2::new(420.0, 470.0), Pos2::new(560.0, 330.0), Pos2::new(250.0, 600.0)]
            .into_iter()
            .find(|p| !win.expand(6.0).contains(*p))
            .unwrap_or_else(|| panic!("a point of the view beside the window {win:?}"));
        let before = w.view_state();
        w.poke(free);
        assert_ne!(w.view_state(), before, "input reaches the view at {free:?} (window {win:?})");
        // A press on the window's caption, tab strip or toolbar over the
        // view leaves the focus off the view.
        let tab = w.app.pane_tab_rects.iter().map(|(_, r)| r.center()).next().expect("the tab strip");
        let toolbar = Pos2::new(win.right() - 12.0, win.center().y);
        let caption = Pos2::new(win.left() + 60.0, win.top() + 10.0);
        w.press(egui::PointerButton::Primary, caption, caption);
        assert_eq!(w.app.copy_focus, app::CopyFocus::Pane(Unit::Vector), "the caption gives the pane the focus");
        for (what, p) in [("tab strip", tab), ("toolbar", toolbar)] {
            w.press(egui::PointerButton::Primary, p, p);
            assert_eq!(w.app.copy_focus, app::CopyFocus::Pane(Unit::Vector), "{what}: the press leaves the focus on the pane");
        }
        // The same from the view's focus: the window's caption takes it.
        w.press(egui::PointerButton::Primary, free, free);
        assert_eq!(w.app.copy_focus, app::CopyFocus::View);
        w.press(egui::PointerButton::Primary, caption, caption);
        assert_ne!(w.app.copy_focus, app::CopyFocus::View);
    }

    /// A docked pane's caption drag floats it once the pointer is past the
    /// drag distance from the press — 4 px at 100 %, grown with the display
    /// scale — wherever the pointer is, with the four-way move cursor from
    /// the press on (#90); the caption's buttons never start a drag.
    #[test]
    fn a_short_caption_drag_floats_a_docked_pane_with_the_move_cursor() {
        use docking::{Place, Unit};
        use egui::CursorIcon::{Default as Arrow, Move};
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        for dpi in [96, 120, 144] {
            dpi::set(dpi);
            let reach = dpi::s(docking::DRAG_PX);
            let fresh = || {
                let mut w = Win::new();
                w.idle(3);
                let pane = w.app.dock_rects[Unit::Vector as usize].expect("the docked group");
                (w, pane)
            };
            // The press on the caption body, 60 px in from the pane's left.
            let (mut w, pane) = fresh();
            let at = Pos2::new(pane.left() + 60.0, pane.top() + 8.0);
            assert_eq!(w.run(vec![egui::Event::PointerMoved(at)]), Arrow, "{dpi}: hovering the caption leaves the arrow");
            assert_eq!(w.run(vec![ev(at, true)]), Move, "{dpi}: the press arms the drag: move cursor");
            // Up to the distance: still docked, still armed.
            assert_eq!(w.run(vec![egui::Event::PointerMoved(at + Vec2::new(reach, 0.0))]), Move);
            assert_eq!(w.app.docks[Unit::Vector].place, Place::Docked, "{dpi}: {reach} px is not past the distance");
            // One more px, the pointer still over the pane: it floats.
            let to = at + Vec2::new(reach + 1.0, 0.0);
            assert!(w.app.dock_rects[Unit::Vector as usize].is_some_and(|r| r.contains(to)));
            assert_eq!(w.run(vec![egui::Event::PointerMoved(to)]), Move, "{dpi}: the cursor goes on while dragging");
            w.idle(1);
            assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating, "{dpi}: past the drag distance");
            assert_eq!(w.run(vec![egui::Event::PointerMoved(to + Vec2::new(3.0, 3.0))]), Move, "{dpi}: moving the floated pane");
            w.run(vec![ev(to, false)]);
            w.idle(3);
            assert_eq!(w.run(vec![egui::Event::PointerMoved(Pos2::new(5.0, 5.0))]), Arrow, "{dpi}: released: the arrow");
            // A vertical drag counts the same.
            let (mut w, pane) = fresh();
            let at = Pos2::new(pane.left() + 60.0, pane.top() + 8.0);
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.run(vec![ev(at, true)]);
            w.run(vec![egui::Event::PointerMoved(at + Vec2::new(0.0, reach))]);
            assert_eq!(w.app.docks[Unit::Vector].place, Place::Docked);
            w.run(vec![egui::Event::PointerMoved(at + Vec2::new(0.0, reach + 1.0))]);
            w.idle(1);
            assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating, "{dpi}: down past the distance");
            // The caption's buttons (close at 11 px from the right edge, pin
            // at 26) never start a drag, however far the pointer goes.
            for from_right in [11.0, 26.0] {
                let (mut w, pane) = fresh();
                let btn = Pos2::new(pane.right() - dpi::s(from_right), pane.top() + dpi::s(10.0));
                w.run(vec![egui::Event::PointerMoved(btn)]);
                assert_eq!(w.run(vec![ev(btn, true)]), Arrow, "{dpi}: a press on a button: the arrow");
                for k in 1..=6 {
                    w.run(vec![egui::Event::PointerMoved(btn + Vec2::new(-20.0 * k as f32, 3.0 * k as f32))]);
                }
                assert_eq!(w.app.docks[Unit::Vector].place, Place::Docked, "{dpi}: a drag from the button at {from_right} did not float it");
            }
        }
        dpi::set(96);
    }

    /// Only the left button drags a caption (`0x10086f3a` reads the left
    /// button): a right-drag floats nothing, shows no move cursor and
    /// moves no floating window.
    #[test]
    fn a_right_drag_on_a_caption_does_nothing() {
        use docking::{Place, Unit};
        use egui::CursorIcon::Move;
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
        let mut w = Win::new();
        let pane = w.app.dock_rects[Unit::Vector as usize].expect("docked");
        let at = Pos2::new(pane.left() + 60.0, pane.top() + 8.0);
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(at, true)]);
        for k in 1..=6 {
            assert_ne!(w.run(vec![egui::Event::PointerMoved(at + Vec2::new(10.0 * k as f32, 0.0))]), Move);
        }
        w.run(vec![ev(at + Vec2::new(60.0, 0.0), false)]);
        w.idle(2);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Docked, "a right-drag floats nothing");
        w.app.pane_menu = None;
        w.app.docks.0[Unit::Vector as usize].double_click(Pos2::ZERO);
        w.idle(3);
        let win = w.app.docks[Unit::Vector].float_rect.expect("floating");
        let at = Pos2::new(win.left() + 60.0, win.top() + 10.0);
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(at, true)]);
        for k in 1..=6 {
            assert_ne!(w.run(vec![egui::Event::PointerMoved(at + Vec2::new(10.0 * k as f32, 5.0))]), Move);
        }
        w.run(vec![ev(at + Vec2::new(60.0, 5.0), false)]);
        w.idle(2);
        assert_eq!(w.app.docks[Unit::Vector].float_rect, Some(win), "a right-drag moves no window");
    }

    /// The new drag rule leaves the other captions alone (#90): a floating
    /// pane's caption press shows the move cursor and a click on it moves
    /// nothing; a slid-out pane's caption keeps the arrow and is never
    /// floated by a drag.
    #[test]
    fn floating_and_slid_out_captions_under_the_short_drag_rule() {
        use docking::{Place, Unit};
        use egui::CursorIcon::Move;
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        // Slid out.
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out);
        let area = w.app.dock_areas[Unit::Vector as usize].expect("slid out");
        let at = Pos2::new(area.left() + 60.0, area.top() + 8.0);
        w.run(vec![egui::Event::PointerMoved(at)]);
        assert_ne!(w.run(vec![ev(at, true)]), Move, "a slid-out caption keeps the arrow");
        w.run(vec![egui::Event::PointerMoved(at + Vec2::new(30.0, 10.0))]);
        w.run(vec![ev(at + Vec2::new(30.0, 10.0), false)]);
        w.idle(2);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::AutoHide, "no float from a drag");
        // Floating: a click (press and release in place) changes nothing,
        // the press shows the move cursor.
        let mut w = Win::new();
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Vector).map(|(_, r)| *r).expect("drawn");
        let dbl = Pos2::new(canvas.left() + 60.0, canvas.top() - 10.0);
        w.run(vec![egui::Event::PointerMoved(dbl)]);
        w.run(vec![ev(dbl, true), ev(dbl, false)]);
        w.run(vec![ev(dbl, true), ev(dbl, false)]);
        w.idle(3);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating);
        let win = w.app.docks[Unit::Vector].float_rect.expect("floating");
        let at = Pos2::new(win.left() + 60.0, win.top() + 10.0);
        w.run(vec![egui::Event::PointerMoved(at)]);
        assert_eq!(w.run(vec![ev(at, true)]), Move);
        w.run(vec![ev(at, false)]);
        w.idle(2);
        assert_eq!(w.app.docks[Unit::Vector].float_rect, Some(win), "a click moves nothing");
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating);
    }

    /// An auto-hidden group slid out by hovering its strip button stays
    /// out while the pointer rests there (issue #4): the slid-out pane
    /// keeps to its rect beside the strip, so the button stays hovered
    /// every frame and the 700 ms hide check finds the pointer on it — on
    /// every edge, for a tabbed group, the pane beside it and that pane on
    /// an edge of its own. Moved off both, it slides back; back on the
    /// button while it slides in, the button is hovered at once.
    #[test]
    fn auto_hidden_pane_stays_out_while_its_button_is_hovered() {
        use docking::{Edge, Spot, Unit};
        let edge = |edge| Some(Spot::Edge { edge, outer: true });
        let cases = [(Unit::Vector, edge(Edge::Right)), (Unit::Vector, edge(Edge::Left)), (Unit::Vector, edge(Edge::Top)), (Unit::Vector, edge(Edge::Bottom)), (Unit::Frequency, None), (Unit::Frequency, edge(Edge::Left))];
        for (unit, spot) in cases {
            let case = format!("{unit:?} {spot:?}");
            let mut w = Win::new();
            if let Some(spot) = spot {
                w.app.docks.0[unit as usize].spot = spot;
                w.idle(2);
            }
            w.app.docks.0[unit as usize].pin();
            w.idle(2);
            let strip = w.app.dock_strips[unit as usize].unwrap_or_else(|| panic!("{case}: its strip"));
            // The top strip lies under the caption's glass and the pane
            // slides out over it, from the window's top (VM): the pane
            // covers its buttons, so the hover ends once it is out.
            let top = w.app.docks.edge(unit) == docking::Edge::Top;
            let area = |w: &Win| w.ctx.memory(|m| m.area_rect(egui::Id::new(("fv_slide", unit))));
            // On the first button.
            let on = w.app.strip_hits[unit as usize].buttons[0].center();
            w.run(vec![egui::Event::PointerMoved(on)]);
            let mut first = None;
            for k in 0..80 {
                w.run(vec![]);
                let d = w.app.docks[unit];
                assert!(d.on_button() || (top && d.out), "{case}: frame {k}: the button lost its hover: layer {:?} at {on:?}", w.ctx.layer_id_at(on));
                if d.out {
                    first.get_or_insert(k);
                    // The pane keeps to its rect: the strip stays uncovered.
                    let (a, r) = (area(&w).expect("the slid-out pane"), w.app.dock_areas[unit as usize].expect("its rect"));
                    assert!(r.expand(0.5).contains_rect(a) && (top || !a.intersects(strip.shrink(0.5))), "{case}: frame {k}: area {a:?}, rect {r:?}, strip {strip:?}");
                } else {
                    assert!(first.is_none(), "{case}: frame {k}: it went back while the button was hovered");
                }
                if first.is_some_and(|f| k > f + 12) {
                    assert_eq!(d.slide, 1.0, "{case}: frame {k}: out, it rests");
                }
            }
            let first = first.unwrap_or_else(|| panic!("{case}: the hover slid it out"));
            assert!(first * 50 <= 600, "{case}: out after {} ms", first * 50);
            // Into the pane: still out; then away from both: it slides back.
            let rect = w.app.dock_areas[unit as usize].expect("its rect");
            w.run(vec![egui::Event::PointerMoved(rect.center())]);
            w.idle(30);
            assert!(w.app.docks[unit].out, "{case}: the pointer in the pane keeps it out");
            let view = w.app.view_rects.expect("the view").0.shrink(10.0);
            let away = [view.left_top(), view.right_top(), view.left_bottom(), view.right_bottom()].into_iter().find(|&q| !rect.contains(q) && !strip.contains(q)).expect("a point off both");
            w.run(vec![egui::Event::PointerMoved(away)]);
            for _ in 0..30 {
                if !w.app.docks[unit].out {
                    break;
                }
                w.run(vec![]);
            }
            // Sliding in, back on the button: hovered at once, out again
            // (not under the top pane, which covers it).
            w.idle(3);
            assert!(!w.app.docks[unit].out && w.app.docks[unit].slide > 0.0, "{case}: sliding in");
            if !top {
                w.run(vec![egui::Event::PointerMoved(on)]);
                assert!(w.app.docks[unit].on_button(), "{case}: the button is hovered while the pane slides in");
                w.idle(20);
                assert!(w.app.docks[unit].out, "{case}: the hover slid it out again");
            }
            w.run(vec![egui::Event::PointerMoved(away)]);
            w.idle(30);
            let pane = w.app.dock_areas[unit as usize];
            assert!(!w.app.docks[unit].out && pane.is_none(), "{case}: away from both it slid back ({pane:?})");
        }
    }

    /// Auto-hide asks for frames only when it needs them — every frame of
    /// a slide, the hover's open time, the hide check's 700 ms timer — and
    /// keeps its timing with nothing else waking the window: frames run
    /// only when the last one asked (the event loop with no input).
    #[test]
    fn auto_hide_wakes_only_for_its_slide_and_timers() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        // One frame with `events`, then frames only as asked, for `secs`;
        // the frames' times and delays.
        let run = |w: &mut Win, events: Vec<egui::Event>, secs: f64| {
            let mut log = Vec::new();
            let (start, mut events) = (w.ctx.input(|i| i.time) + 0.05, events);
            let mut t = start;
            while t < start + secs && log.len() < 400 {
                let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0))), time: Some(t), events: std::mem::take(&mut events), ..Default::default() };
                let out = w.ctx.run(raw, |ctx| w.app.frame_ui(ctx));
                let delay = out.viewport_output.get(&egui::ViewportId::ROOT).map_or(f64::INFINITY, |v| v.repaint_delay.as_secs_f64());
                log.push((t - start, w.app.docks[Unit::Vector].out, w.app.docks[Unit::Vector].slide, delay));
                // The delay less the frame time egui expects (a 60 Hz
                // display): the next frame comes that much later.
                t += (delay + 1.0 / 60.0).min(10.0);
            }
            log
        };
        // On the button: out at 400 ms, with no frames between.
        let button = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        let log = run(&mut w, vec![egui::Event::PointerMoved(button)], 0.6);
        let open = log.iter().find(|f| f.1).expect("the hover slid it out");
        assert!((0.4..0.45).contains(&open.0), "out at {:.3} s: {log:?}", open.0);
        assert!(log.len() < 5 + 12, "frames while hovering: {log:?}");
        // Out and resting, the pointer on the button: one frame per check.
        let log = run(&mut w, vec![], 3.0);
        assert!(log.iter().all(|f| f.1), "it stays out");
        let resting: Vec<_> = log.iter().filter(|f| f.2 == 1.0).collect();
        assert!(resting.iter().skip(1).all(|f| f.3 > 0.1), "no 16 ms frames at rest: {log:?}");
        assert!(log.len() < 12 + 6, "frames in 3 s: {}", log.len());
        // Away from both: back at the next check, within 700 ms.
        let view = w.app.view_rects.expect("the view").0;
        let log = run(&mut w, vec![egui::Event::PointerMoved(view.left_bottom() + Vec2::new(20.0, -20.0))], 1.5);
        let back = log.iter().find(|f| !f.1).expect("it slid back");
        assert!(back.0 <= 0.75, "back at {:.3} s: {log:?}", back.0);
        assert!(log.last().is_some_and(|f| f.2 == 0.0), "slid in: {log:?}");
    }

    /// A slid-out pane holding the focus stays out with the pointer
    /// elsewhere (the check returns while its active flag is set,
    /// `0x10087a4a` / `0x10089d7d`; VM: a click in the slid-out vector
    /// diagram, the pointer away 4.5 s, still out); a press in the
    /// oscillogram view takes the focus and it slides back at the next
    /// check.
    #[test]
    fn a_focused_slid_out_pane_stays_out() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out, "the hover slid it out");
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Vector).map(|(_, r)| *r).expect("the slid-out vector diagram");
        let away = w.app.view_rects.expect("the view").0.left_top() + Vec2::new(40.0, 40.0);
        let press = |w: &mut Win, at: Pos2| {
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.run(vec![ev(true)]);
            w.run(vec![ev(false)]);
        };
        press(&mut w, canvas.center());
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Vector));
        w.run(vec![egui::Event::PointerMoved(away)]);
        w.idle(90);
        assert!(w.app.docks[Unit::Vector].out, "the focus in the pane keeps it out");
        press(&mut w, away);
        assert_eq!(w.app.copy_focus, app::CopyFocus::View);
        w.idle(30);
        assert!(!w.app.docks[Unit::Vector].out && w.app.docks[Unit::Vector].slide == 0.0, "the focus gone, it slid back");
    }

    /// A press on the strip button gives the pane the focus (`0x1005f612`;
    /// VM: clicked out, the pointer over the view 4.5 s, still out); the
    /// focus leaves a pane that hides — pinned to its edge and back in its
    /// strip it slides back as soon as the pointer is away.
    #[test]
    fn a_pane_clicked_out_holds_the_focus_until_it_hides() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        let on = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        let away = w.app.view_rects.expect("the view").0.left_top() + Vec2::new(40.0, 40.0);
        let ev = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        w.run(vec![egui::Event::PointerMoved(on), ev(on, true), ev(on, false)]);
        w.idle(2);
        assert!(w.app.docks[Unit::Vector].out, "the click slid it out at once");
        w.run(vec![egui::Event::PointerMoved(away)]);
        w.idle(90);
        assert!(w.app.docks[Unit::Vector].out, "the focus in the pane keeps it out");
        // Pinned to its edge and back: hidden, the focus is gone.
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        assert_eq!(w.app.copy_focus, app::CopyFocus::View, "the focus left the hidden pane");
        w.run(vec![egui::Event::PointerMoved(on)]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out, "the hover slid it out");
        w.run(vec![egui::Event::PointerMoved(away)]);
        w.idle(30);
        assert!(!w.app.docks[Unit::Vector].out, "no focus: away, it slid back");
    }

    /// The slid-out pane only slides: from its first frame out it is
    /// painted opaque (no fade-in).
    #[test]
    fn a_sliding_pane_is_opaque_from_its_first_frame() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        w.app.docks.0[Unit::Vector as usize].out = true;
        let (mut seen, mut out_frames) = (0, 0);
        for k in 0..14 {
            let t = w.ctx.input(|i| i.time) + 0.016;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0))), time: Some(t), events: if k == 0 { vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())] } else { vec![] }, ..Default::default() };
            let app = &mut w.app;
            let out = w.ctx.run(raw, |ctx| app.frame_ui(ctx));
            let Some(rect) = w.app.dock_areas[Unit::Vector as usize] else { continue };
            out_frames += 1;
            let fill = w.ctx.style().visuals.panel_fill;
            // The pane's background: the shape filling its rect.
            let bg = out.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if (r.rect.min - rect.min).length() < 1.0 && (r.rect.max - rect.max).length() < 1.0 => Some(r.fill),
                _ => None,
            });
            if let Some(f) = bg {
                assert_eq!(f, fill, "frame {k}: the pane faded");
                seen += 1;
            }
        }
        assert_eq!(seen, out_frames, "the pane's background painted on every frame it is out");
    }

    /// A press on a pane's caption gives that pane the focus (`0x10086d23`,
    /// `0x10086f01`): the slid-out pane's own holds it out; another pane's
    /// takes the focus, and the slid-out pane slides back.
    #[test]
    fn a_caption_press_gives_its_pane_the_focus() {
        use docking::{Edge, Spot, Unit};
        let mut w = Win::new();
        // Частотная on the left edge, clear of the slid-out vector pane.
        w.app.docks.0[Unit::Frequency as usize].spot = Spot::Edge { edge: Edge::Left, outer: true };
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        let away = w.app.view_rects.expect("the view").0.center();
        let canvas = |w: &Win, p: app::Pane| w.app.pane_canvas.iter().rev().find(|(q, _)| *q == p).map(|(_, r)| *r);
        let press = |w: &mut Win, at: Pos2, button| {
            let ev = |pressed| egui::Event::PointerButton { pos: at, button, pressed, modifiers: Default::default() };
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.run(vec![ev(true)]);
            w.run(vec![ev(false)]);
        };
        let freq = canvas(&w, app::Pane::Frequency).expect("Частотная docked");
        for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
            w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
            w.idle(30);
            let v = canvas(&w, app::Pane::Vector).expect("the slid-out vector diagram");
            assert!(w.app.dock_areas[Unit::Vector as usize].is_some_and(|a| !a.contains(away) && !a.contains(freq.center())), "away and Частотная clear of the pane");
            press(&mut w, Pos2::new(v.left() + 60.0, v.top() - 10.0), button);
            w.run(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
            w.run(vec![egui::Event::PointerMoved(away)]);
            w.idle(60);
            assert!(w.app.docks[Unit::Vector].out, "{button:?}: its caption pressed, it stays out");
            // Частотная's caption: the focus goes there.
            press(&mut w, Pos2::new(freq.left() + 60.0, freq.top() - 10.0), button);
            w.run(vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
            w.run(vec![egui::Event::PointerMoved(away)]);
            w.idle(30);
            assert!(!w.app.docks[Unit::Vector].out, "{button:?}: another caption pressed, it slid back");
        }
    }

    /// An auto-hidden group whose panes are all hidden while it is out is
    /// back in its strip when they show again (not fully out, sliding in).
    #[test]
    fn a_group_hidden_while_out_shows_in_its_strip() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out);
        let shown: Vec<app::Pane> = [app::Pane::Vector, app::Pane::Circle, app::Pane::Differential].into_iter().filter(|&p| w.app.panes.visible(p)).collect();
        // (The ribbon's toggle does not hide an auto-hidden pane: closed here
        // as the pane's caption does.)
        for &p in &shown {
            w.app.panes.set(p, false);
        }
        w.idle(2);
        for &p in &shown {
            w.app.panes.set(p, true);
        }
        w.run(vec![egui::Event::PointerMoved(w.app.view_rects.expect("the view").0.center())]);
        let d = w.app.docks[Unit::Vector];
        assert!(!d.out && d.slide == 0.0, "in its strip: {d:?}");
    }

    /// Each edge's auto-hide strip is the frame's outermost row or column
    /// (#61; VM, maximised): a side strip runs from under the caption row
    /// to the window's bottom, beside the ribbon, the bars and the status
    /// row (which stop at it); the bottom one is under the status row; the
    /// top one is the first row, the ribbon pushed down by it. A slid-out
    /// side pane spans the window's whole height, a top / bottom one its
    /// whole width, against its strip.
    #[test]
    fn auto_hide_strips_are_the_outermost_on_every_edge() {
        use docking::{Edge, Spot, Unit};
        let (screen, cap) = (Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0)), ribbon::caption_h());
        for edge in Edge::ALL {
            let mut w = Win::new();
            w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge, outer: true };
            w.idle(2);
            let base = w.app.dock_client;
            w.app.docks.0[Unit::Vector as usize].pin();
            w.idle(2);
            let strip = w.app.dock_strips[Unit::Vector as usize].unwrap_or_else(|| panic!("{edge:?}: its strip"));
            let after = w.app.dock_client;
            match edge {
                Edge::Right => {
                    assert_eq!((strip.right(), strip.top(), strip.bottom(), strip.width()), (screen.right(), cap, screen.bottom(), 23.0), "{edge:?}: {strip:?}");
                    assert!((base.right() - after.right() - 23.0).abs() < 0.5, "{edge:?}: the bars and panes stop at it: {base:?} → {after:?}");
                }
                Edge::Left => {
                    assert_eq!((strip.left(), strip.top(), strip.bottom(), strip.width()), (0.0, cap, screen.bottom(), 23.0), "{edge:?}: {strip:?}");
                    assert!((after.left() - base.left() - 23.0).abs() < 0.5, "{edge:?}: {base:?} → {after:?}");
                }
                Edge::Bottom => {
                    assert_eq!((strip.left(), strip.right(), strip.bottom(), strip.height()), (0.0, screen.right(), screen.bottom(), 21.0), "{edge:?}: {strip:?}");
                    assert!((base.bottom() - after.bottom() - 21.0).abs() < 0.5, "{edge:?}: the status row is above it: {base:?} → {after:?}");
                }
                Edge::Top => {
                    assert_eq!((strip.left(), strip.top(), strip.right(), strip.height()), (0.0, 0.0, screen.right(), 21.0), "{edge:?}: {strip:?}");
                    assert!((after.top() - base.top() - 21.0).abs() < 0.5, "{edge:?}: the ribbon is pushed down: {base:?} → {after:?}");
                }
            }
            // Out: the pane against its strip, over the whole extent along it,
            // from below the caption row (not from the window's top, as the
            // original's: its caption's buttons would lie under the window's).
            let button = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
            w.run(vec![egui::Event::PointerMoved(button)]);
            w.idle(30);
            let pane = w.app.dock_areas[Unit::Vector as usize].unwrap_or_else(|| panic!("{edge:?}: slid out"));
            match edge {
                Edge::Right => assert_eq!((pane.right(), pane.top(), pane.bottom()), (strip.left(), cap, screen.bottom()), "{edge:?}: {pane:?}"),
                Edge::Left => assert_eq!((pane.left(), pane.top(), pane.bottom()), (strip.right(), cap, screen.bottom()), "{edge:?}: {pane:?}"),
                Edge::Bottom => assert_eq!((pane.left(), pane.right(), pane.bottom()), (0.0, screen.right(), strip.top()), "{edge:?}: {pane:?}"),
                Edge::Top => assert_eq!((pane.left(), pane.right(), pane.top()), (0.0, screen.right(), cap), "{edge:?}: {pane:?}"),
            }
        }
    }

    /// The window's own buttons stay whole at the window's edge beside a
    /// right strip and under a top strip (VM; the review of #61): a click
    /// at the right edge in the caption row's top, middle and bottom closes
    /// the window — not clipped by the strip's or the ribbon's panel.
    #[test]
    fn the_window_buttons_stay_live_beside_a_right_and_a_top_strip() {
        use docking::{Edge, Spot, Unit};
        let screen = Vec2::new(1320.0, 860.0);
        for edge in [Edge::Right, Edge::Top] {
            let mut w = Win::new();
            w.app.own_caption = true;
            w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge, outer: true };
            w.app.docks.0[Unit::Vector as usize].pin();
            w.idle(3);
            for y in [2.0, 15.0, ribbon::caption_h() - 3.0] {
                for dx in [2.0, 10.0] {
                    let at = Pos2::new(screen.x - dx, y);
                    let mut closed = false;
                    for events in [vec![egui::Event::PointerMoved(at)], vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }], vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]] {
                        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)), time: Some(w.ctx.input(|i| i.time) + 0.05), events, ..Default::default() };
                        let out = w.ctx.run(raw, |ctx| w.app.frame_ui(ctx));
                        closed |= out.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Close)));
                    }
                    assert!(closed, "{edge:?}: a click at {at:?} closes the window");
                    w.idle(2);
                }
            }
        }
    }

    /// Beside a right strip the caption row has no dead band: the ribbon's panel is narrower by the strip's
    /// width and the window's buttons lie at the window's edge, so the free space reserves the buttons' width
    /// less that inset. A drag anywhere in the row up to the buttons moves the window, as elsewhere in the row.
    #[test]
    fn the_caption_row_beside_a_right_strip_has_no_dead_band() {
        use docking::{Edge, Spot, Unit};
        let screen = Vec2::new(1320.0, 860.0);
        let mut w = Win::new();
        w.app.own_caption = true;
        w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge: Edge::Right, outer: true };
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        let ev = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        // The strip is 23 px wide: the band was [W - 161, W - 138].
        for dx in [160.0, 150.0, 140.0, 139.0] {
            let at = Pos2::new(screen.x - dx, 10.0);
            let mut dragged = false;
            for events in [vec![egui::Event::PointerMoved(at)], vec![ev(at, true)], vec![egui::Event::PointerMoved(at + Vec2::new(12.0, 0.0))], vec![egui::Event::PointerMoved(at + Vec2::new(24.0, 0.0))], vec![ev(at + Vec2::new(24.0, 0.0), false)]] {
                let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)), time: Some(w.ctx.input(|i| i.time) + 0.05), events, ..Default::default() };
                let out = w.ctx.run(raw, |ctx| w.app.frame_ui(ctx));
                dragged |= out.viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::StartDrag)));
            }
            assert!(dragged, "a press at {dx} px from the right edge drags the window");
            w.idle(2);
        }
    }

    /// An auto-hidden tabbed group has a strip button per visible tab (#62;
    /// VM, three tabs): the front pane's full length, the others' stubs of
    /// 24 px, the buttons laid out in the tab order from 4 px under the
    /// window's top (so the first ones' ends lie under the caption glass);
    /// hovering a stub slides the group out with that pane alone, no tab
    /// strip, and the pane becomes the front one with the full length.
    #[test]
    fn an_auto_hidden_tabbed_group_has_a_full_front_button_and_stubs() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        let long_stub = |w: &Win| {
            let b = &w.app.strip_hits[Unit::Vector as usize].buttons;
            (b.iter().filter(|r| r.height() > 100.0).count(), b.iter().filter(|r| r.height() <= 24.5).count(), b.clone())
        };
        let (long, stubs, buttons) = long_stub(&w);
        assert_eq!((long, stubs), (1, buttons.len() - 1), "one full button, the rest stubs: {buttons:?}");
        assert!(buttons.windows(2).all(|b| (b[0].bottom() - b[1].top()).abs() < 0.5), "one after the other");
        let shown = |w: &Win| w.app.pane_canvas.iter().filter(|(p, _)| *p != app::Pane::Frequency).map(|(p, _)| *p).collect::<Vec<_>>();
        assert!(shown(&w).is_empty(), "in its strip nothing of it is drawn");
        // A stub: its pane in front, alone in the pane, the button long now.
        let front = w.app.panes.front(w.app.docks.members(Unit::Vector));
        let stub_rect = *buttons.iter().find(|r| r.height() <= 24.5).expect("a visible stub");
        let (front_slot, stub) = (*buttons.iter().find(|r| r.height() > 100.0).expect("the front's button"), stub_rect.center());
        w.run(vec![egui::Event::PointerMoved(stub)]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out);
        let now = w.app.panes.front(w.app.docks.members(Unit::Vector));
        assert_ne!(now, front, "the stub's pane came to the front");
        assert!(w.app.pane_tab_rects.is_empty(), "no tab strip in the slid-out pane: {:?}", w.app.pane_tab_rects);
        assert!(shown(&w).iter().all(|&p| Some(p) == now), "only the front pane is drawn: {:?}", shown(&w));
        let (long, _, after) = long_stub(&w);
        assert_eq!(long, 1, "one full button again: {after:?}");
        let long_rect = after.iter().find(|r| r.height() > 100.0).expect("the front's button");
        // The column starts 4 px under the window's top and runs in tab order: the front pane was first
        // (its button 4 px + its length down), so with the hovered pane in front the former front keeps
        // its place as a 24 px stub and the new long button follows it. (The new button does not start
        // where the hovered stub lay: the former front shrank above it.)
        let len = |p: app::Pane, front: bool| docking::strip_button_len(views::pane_title(p).chars().count(), front, true);
        assert!((front_slot.bottom() - (4.0 + len(front.unwrap(), true))).abs() < 0.5, "the front pane was first in the column: {front_slot:?}");
        assert!((long_rect.bottom() - (4.0 + len(front.unwrap(), false) + len(now.unwrap(), true))).abs() < 0.5, "the former front's slot is a 24 px stub, the new long button after it: {long_rect:?}");
        assert!((stub_rect.top() - front_slot.bottom()).abs() < 0.5, "the hovered stub lay right after the former front's button");
        let stub_after = after.iter().find(|r| r.height() <= 24.5 && r.top() >= long_rect.bottom() - 0.5).expect("the stub that followed");
        assert!((stub_after.top() - long_rect.bottom()).abs() < 0.5 && stub_after.height() == 24.0, "the stubs follow the long button: {stub_after:?}");
    }

    /// A divider beside a slid-out pane (#63; VM, `0x10087a4a`): the pointer
    /// on it counts as inside, a drag on it resizes the pane and the size
    /// stays; a visible dock-bar tooltip of the pane holds it out too.
    #[test]
    fn a_slid_out_pane_has_a_divider_that_resizes_it_and_holds_it_out() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        let button = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        w.run(vec![egui::Event::PointerMoved(button)]);
        w.idle(30);
        let (pane, div) = (w.app.dock_areas[Unit::Vector as usize].expect("its rect"), w.app.dock_dividers[Unit::Vector as usize].expect("its divider"));
        assert_eq!((div.right(), div.width(), div.top(), div.bottom()), (pane.left(), docking::DIVIDER_PX, pane.top(), pane.bottom()), "against the pane on the side facing the view: {div:?} {pane:?}");
        // The pointer on the divider alone: still out through the checks.
        w.run(vec![egui::Event::PointerMoved(div.center())]);
        w.idle(40);
        assert!(w.app.docks[Unit::Vector].out, "the divider counts as inside");
        // Dragged, it resizes the pane; the size stays for the next time.
        let before = pane.width();
        let ev = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        w.run(vec![egui::Event::PointerMoved(div.center())]);
        w.run(vec![ev(div.center(), true)]);
        let to = div.center() - Vec2::new(120.0, 0.0);
        w.run(vec![egui::Event::PointerMoved(div.center() - Vec2::new(60.0, 0.0))]);
        w.run(vec![egui::Event::PointerMoved(to)]);
        w.idle(2);
        w.run(vec![ev(to, false)]);
        w.idle(3);
        let wider = w.app.dock_areas[Unit::Vector as usize].expect("its rect").width();
        assert!((wider - before - 120.0).abs() <= 2.0, "wider by the drag: {before} → {wider}");
        w.run(vec![egui::Event::PointerMoved(Pos2::new(300.0, 400.0))]);
        w.idle(30);
        assert!(!w.app.docks[Unit::Vector].out);
        let button = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        w.run(vec![egui::Event::PointerMoved(button)]);
        w.idle(30);
        let again = w.app.dock_areas[Unit::Vector as usize].expect("its rect").width();
        assert!((again - wider).abs() <= 1.0, "the size stays: {wider} → {again}");
        // A visible tooltip of the pane holds it out with the pointer away
        // (the anchor the tips module records for the check).
        let inside = w.app.dock_areas[Unit::Vector as usize].expect("its rect").shrink(20.0);
        w.run(vec![egui::Event::PointerMoved(Pos2::new(300.0, 400.0))]);
        for _ in 0..40 {
            let pass = w.ctx.cumulative_pass_nr();
            w.ctx.data_mut(|d| d.insert_temp(egui::Id::new("fv_tip_anchor"), (inside, pass)));
            w.run(vec![]);
        }
        assert!(w.app.docks[Unit::Vector].out, "its tooltip holds it out");
        w.idle(40);
        assert!(!w.app.docks[Unit::Vector].out, "the tooltip gone, it slides back");
    }

    /// The ribbon's pane toggle on an auto-hidden group (#64; VM, the
    /// original): the ribbon button never checks for it, slid out or not;
    /// the toggle shows the pane — in its strip it slides out, out it stays
    /// — and never closes or docks it; the pointer elsewhere, the next hide
    /// check slides it back.
    #[test]
    fn the_ribbon_toggle_shows_an_auto_hidden_pane_and_never_closes_it() {
        use docking::{Place, Unit};
        let mut w = Win::new();
        assert!(w.app.pane_checked(app::Pane::Vector), "docked, its button is checked");
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        let away = w.app.view_rects.expect("the view").0.center();
        w.run(vec![egui::Event::PointerMoved(away)]);
        assert!(!w.app.pane_checked(app::Pane::Vector) && !w.app.pane_checked(app::Pane::Differential), "auto-hidden, its buttons do not check");
        assert!(w.app.pane_checked(app::Pane::Frequency), "a docked pane's still does");
        // In its strip: the toggle slides it out, the check takes it back.
        w.app.dispatch(app::Command::TogglePane(app::Pane::Vector));
        w.idle(3);
        assert!(w.app.docks[Unit::Vector].out, "the toggle slid it out");
        assert!(w.app.panes.visible(app::Pane::Vector) && w.app.docks[Unit::Vector].place == Place::AutoHide, "not closed, not docked");
        assert!(!w.app.pane_checked(app::Pane::Vector), "slid out, still unchecked");
        w.idle(30);
        assert!(!w.app.docks[Unit::Vector].out, "taken back by the next check");
        // The pointer on the pane keeps it out.
        w.app.dispatch(app::Command::TogglePane(app::Pane::Vector));
        w.idle(4);
        let rect = w.app.dock_areas[Unit::Vector as usize].expect("its rect");
        w.run(vec![egui::Event::PointerMoved(rect.center())]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out, "the pointer in the pane holds it");
        // Out, toggled again: still out, still in its strip afterwards.
        w.app.dispatch(app::Command::TogglePane(app::Pane::Vector));
        w.idle(3);
        assert!(w.app.docks[Unit::Vector].out && w.app.panes.visible(app::Pane::Vector));
        w.run(vec![egui::Event::PointerMoved(away)]);
        w.idle(30);
        assert!(!w.app.docks[Unit::Vector].out && w.app.panes.visible(app::Pane::Vector), "back in its strip, open");
        // The real click on the ribbon's button, the pane out from a hover:
        // taken back with the pointer left on the ribbon.
        let button = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        w.run(vec![egui::Event::PointerMoved(button)]);
        w.idle(20);
        assert!(w.app.docks[Unit::Vector].out, "hovered out");
        let ribbon = Pos2::new(240.0, 90.0);
        w.press(egui::PointerButton::Primary, ribbon, ribbon);
        w.idle(30);
        assert!(!w.app.docks[Unit::Vector].out && w.app.panes.visible(app::Pane::Vector), "taken back with the pointer on the ribbon");
        // A tab of the group other than the front one: shown in front.
        w.app.dispatch(app::Command::TogglePane(app::Pane::Differential));
        w.idle(3);
        assert!(w.app.docks[Unit::Vector].out);
        assert_eq!(w.app.panes.front(w.app.docks.members(Unit::Vector)), Some(app::Pane::Differential), "the toggled pane in front");
    }

    /// Toggled out from a hover, the pane is taken back with frames only as
    /// asked (the event loop): the check's timer keeps the pane's frames
    /// coming while the pointer rests on the ribbon.
    #[test]
    fn a_toggled_out_pane_is_taken_back_by_the_timer_alone() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        let run = |w: &mut Win, events: Vec<egui::Event>, secs: f64| {
            let (start, mut events) = (w.ctx.input(|i| i.time) + 0.05, events);
            let mut t = start;
            let mut log = Vec::new();
            while t < start + secs && log.len() < 400 {
                let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0))), time: Some(t), events: std::mem::take(&mut events), ..Default::default() };
                let out = w.ctx.run(raw, |ctx| w.app.frame_ui(ctx));
                let delay = out.viewport_output.get(&egui::ViewportId::ROOT).map_or(f64::INFINITY, |v| v.repaint_delay.as_secs_f64());
                log.push((t - start, w.app.docks[Unit::Vector].out));
                t += (delay + 1.0 / 60.0).min(10.0);
            }
            log
        };
        let button = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        run(&mut w, vec![egui::Event::PointerMoved(button)], 1.0);
        assert!(w.app.docks[Unit::Vector].out);
        let at = Pos2::new(240.0, 90.0);
        let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let log = run(&mut w, vec![egui::Event::PointerMoved(at), ev(true), ev(false)], 2.0);
        assert!(log[0].1, "still out right after the press (the toggle shows it, never hides it): {log:?}");
        assert!(log.iter().any(|f| !f.1), "taken back within 2 s: {log:?}");
        assert!(w.app.panes.visible(app::Pane::Vector) && w.app.docks[Unit::Vector].place == docking::Place::AutoHide, "still open and auto-hidden");
    }

    /// A strip press gives the pane window the focus, the one focus there
    /// is (VM: after it → leaves В1 where it was, Copy holds a metafile and
    /// no document; after a press in the view → moves В1): the view's keys
    /// stop, Copy gives the pane's diagram; the pane opens on the press.
    #[test]
    fn a_pane_focus_takes_the_keys_and_copy_from_the_view() {
        use docking::Unit;
        let mut w = Win::new();
        let view = w.app.view_rects.expect("the view").0;
        let at = view.left_top() + Vec2::new(300.0, 60.0);
        w.press(egui::PointerButton::Primary, at, at);
        let b1 = |w: &Win| w.app.active_doc().unwrap().main_cursor;
        let before = b1(&w);
        w.key(egui::Key::ArrowRight);
        assert_ne!(b1(&w), before, "the view has the keys");
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        let on = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        let ev = |pressed| egui::Event::PointerButton { pos: on, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        w.run(vec![egui::Event::PointerMoved(on), ev(true)]);
        w.idle(1);
        assert!(w.app.docks[Unit::Vector].out, "out on the press");
        w.run(vec![ev(false)]);
        w.idle(14);
        assert_eq!(w.app.copy_focus, app::CopyFocus::Pane(Unit::Vector));
        let before = b1(&w);
        w.key(egui::Key::ArrowRight);
        assert_eq!(b1(&w), before, "the pane has the keys");
        w.app.dispatch(app::Command::Copy);
        let canvas = w.app.pane_canvas.iter().rev().find(|(p, _)| *p == app::Pane::Vector).map(|(_, r)| *r).expect("the vector diagram");
        assert_eq!(w.app.clip.picture(), Some((canvas.width().round() as usize, canvas.height().round() as usize)), "the pane's diagram");
        assert_eq!(w.app.clip.get(), None, "no document");
    }

    /// The focus stays with a pane that is still shown when its group
    /// changes — Частотная focused, then tabbed into the vector group.
    #[test]
    fn the_focus_follows_a_pane_into_another_group() {
        use docking::{Spot, Unit};
        let mut w = Win::new();
        let freq = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Frequency).map(|(_, r)| *r).expect("Частотная");
        w.press(egui::PointerButton::Primary, freq.center(), freq.center());
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Frequency));
        w.app.docks.0[Unit::Frequency as usize].spot = Spot::Tab(Unit::Vector);
        w.idle(3);
        assert_eq!(w.app.docks.host(Unit::Frequency), Unit::Vector);
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Frequency), "still shown: the focus stays");
    }

    /// Two slid-out groups over one another: a press goes to the one on
    /// top (egui's order — the one shown last), not the other.
    #[test]
    fn a_press_goes_to_the_slid_out_pane_on_top() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.app.docks.0[Unit::Frequency as usize].pin();
        w.idle(3);
        let button = |w: &Win, u: Unit| {
            w.app.strip_hits[u as usize].buttons[0].center()
        };
        // Частотная clicked out (it keeps the focus), then the vector group
        // hovered out over it.
        let f = button(&w, Unit::Frequency);
        let ev = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        w.run(vec![egui::Event::PointerMoved(f), ev(f, true), ev(f, false)]);
        w.idle(14);
        let v = button(&w, Unit::Vector);
        w.run(vec![egui::Event::PointerMoved(v)]);
        w.idle(30);
        assert!(w.app.docks[Unit::Frequency].out && w.app.docks[Unit::Vector].out, "both out: {:?} {:?} f {f:?} v {v:?} {:?}", w.app.docks[Unit::Frequency], w.app.docks[Unit::Vector], w.app.dock_strips);
        let canvas = |w: &Win, p: app::Pane| w.app.pane_canvas.iter().rev().find(|(q, _)| *q == p).map(|(_, r)| *r);
        let over = canvas(&w, app::Pane::Vector).expect("the vector diagram").intersect(canvas(&w, app::Pane::Frequency).expect("Частотная"));
        assert!(over.is_positive(), "they overlap");
        w.press(egui::PointerButton::Primary, over.center(), over.center());
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Vector), "the one on top");
    }

    /// Presses inside a slid-out pane stay in it: its property grid holds it
    /// out; a press on it outside its windows reaches neither the docked
    /// diagram nor the view under it.
    #[test]
    fn presses_in_a_slid_out_pane_stay_in_it() {
        use docking::{Edge, Spot, Unit};
        // Out, the circle diagram in front: its property grid.
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        let on = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        let away = w.app.view_rects.expect("the view").0.left_top() + Vec2::new(40.0, 40.0);
        w.run(vec![egui::Event::PointerMoved(on)]);
        w.idle(30);
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.run(vec![egui::Event::PointerMoved(w.app.dock_areas[Unit::Vector as usize].expect("out").center())]);
        w.idle(3);
        let [_, _, g0, g1] = w.app.slid_ranges[Unit::Vector as usize].expect("drawn out");
        assert!(g1 > g0, "its grid drawn");
        let grid = w.app.pane_grid[g0];
        w.press(egui::PointerButton::Primary, grid.center(), grid.center());
        assert_eq!(w.app.copy_focus, app::CopyFocus::Grid(app::Pane::Circle));
        w.run(vec![egui::Event::PointerMoved(away)]);
        w.idle(60);
        assert!(w.app.docks[Unit::Vector].out, "the grid's focus holds it out");
        w.press(egui::PointerButton::Primary, away, away);
        w.idle(30);
        // Over the docked Частотная: a press on the pane off its diagram.
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        w.run(vec![egui::Event::PointerMoved(on)]);
        w.idle(30);
        let area = w.app.dock_areas[Unit::Vector as usize].expect("out");
        let [c0, c1, ..] = w.app.slid_ranges[Unit::Vector as usize].expect("drawn out");
        let freq = w.app.pane_canvas[..c0].iter().find(|(p, _)| *p == app::Pane::Frequency).map(|(_, r)| *r).expect("Частотная docked");
        let own = w.app.pane_canvas[c0..c1].to_vec();
        let off = (0..200)
            .flat_map(|i| (0..200).map(move |j| Pos2::new(freq.left() + freq.width() * i as f32 / 200.0, freq.top() + freq.height() * j as f32 / 200.0)))
            .find(|&q| area.shrink(4.0).contains(q) && q.y > area.top() + 30.0 && !own.iter().any(|(_, r)| r.expand(2.0).contains(q)))
            .expect("a point of the pane, off its diagram, over Частотная");
        w.press(egui::PointerButton::Primary, off, off);
        assert_eq!(w.app.copy_focus, app::CopyFocus::View, "not Частотная's under it");
        // Over the view: Частотная on the left edge, the pane over the view.
        let mut w = Win::new();
        w.app.docks.0[Unit::Frequency as usize].spot = Spot::Edge { edge: Edge::Left, outer: true };
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(30);
        let area = w.app.dock_areas[Unit::Vector as usize].expect("out");
        let [c0, c1, ..] = w.app.slid_ranges[Unit::Vector as usize].expect("drawn out");
        let own = w.app.pane_canvas[c0..c1].to_vec();
        let view = w.app.view_rects.expect("the view").0;
        let canvas = own.iter().find(|(p, _)| *p == app::Pane::Vector).map(|(_, r)| *r).expect("the vector diagram");
        w.press(egui::PointerButton::Primary, canvas.center(), canvas.center());
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Vector));
        let off = (0..200)
            .flat_map(|i| (0..200).map(move |j| Pos2::new(area.left() + area.width() * i as f32 / 200.0, area.top() + area.height() * j as f32 / 200.0)))
            .find(|&q| view.contains(q) && area.shrink(4.0).contains(q) && q.y > area.top() + 30.0 && !own.iter().any(|(_, r)| r.expand(2.0).contains(q)))
            .expect("a point of the pane, off its diagram, over the view");
        w.press(egui::PointerButton::Primary, off, off);
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Vector), "not the view under it");
    }

    /// The focus leaves a standalone pane that closes (#77; clone: Частотная's
    /// «×» with its diagram focused, the frame's view holds the focus): a
    /// focused diagram's, and a focused property grid's (the start-up
    /// layout's Дифференциальная), so the view's arrow keys work again at
    /// once.
    #[test]
    fn closing_the_pane_that_holds_the_focus_returns_it_to_the_view() {
        use app::{Command, CopyFocus, Pane};
        // A focused diagram.
        let mut w = Win::new();
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Frequency).expect("the frequency diagram").1;
        w.press(egui::PointerButton::Primary, canvas.center(), canvas.center());
        assert_eq!(w.app.copy_focus, CopyFocus::Diagram(Pane::Frequency));
        w.app.dispatch(Command::TogglePane(Pane::Frequency));
        w.idle(3);
        assert_eq!(w.app.copy_focus, CopyFocus::View, "the diagram's pane closed");
        // A focused property grid, in a pane of its own.
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.idle(3);
        let grid = *w.app.pane_grid.first().expect("the differential pane's grid");
        w.press(egui::PointerButton::Primary, grid.center(), grid.center());
        assert_eq!(w.app.copy_focus, CopyFocus::Grid(Pane::Differential));
        w.app.dispatch(Command::TogglePane(Pane::Differential));
        w.idle(3);
        assert_eq!(w.app.copy_focus, CopyFocus::View, "the grid's pane closed");
        // The view's keys work again without a press in it.
        let before = w.view_state();
        w.key(egui::Key::ArrowRight);
        assert_ne!(w.view_state(), before, "the arrow key reaches the view");
    }

    /// A pane of a tabbed group that closes keeps the focus it held (clone,
    /// the start-up layout: the vector diagram focused, its «×» pressed and
    /// the ribbon's toggle, the circle tab still there or not: the hidden
    /// pane's window is still the focus window — copy took its diagram's
    /// picture at its last size, and no document): only a pane of its
    /// own, whose control bar hides, hands the focus to the view (#112).
    #[test]
    fn a_tabbed_pane_that_closes_keeps_the_focus() {
        use app::{Command, CopyFocus, Pane};
        use egui::PointerButton::Primary;
        // The focused diagram of the front tab.
        let mut w = Win::new();
        w.app.active_doc_mut().unwrap().selection.insert(app::ChanKey::Analog(0));
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).expect("the vector diagram").1;
        w.press(Primary, canvas.center(), canvas.center());
        w.app.dispatch(Command::TogglePane(Pane::Vector));
        w.idle(3);
        assert_eq!(w.app.panes.front(w.app.docks.members(docking::Unit::Vector)), Some(Pane::Circle), "the next tab came to the front");
        assert_eq!(w.app.copy_focus, CopyFocus::Diagram(Pane::Vector), "the hidden pane keeps the focus");
        let before = w.view_state();
        w.key(egui::Key::ArrowRight);
        assert_eq!(w.view_state(), before, "the view's keys stay dead");
        w.app.dispatch(Command::Copy);
        assert_eq!(w.app.clip.picture(), Some((canvas.width().round() as usize, canvas.height().round() as usize)), "the hidden diagram's picture at its last size");
        assert_eq!(w.app.clip.get(), None, "a diagram copy leaves no document");
        // Pressing the pane's caption gave its group the focus: closing the
        // front pane leaves it on that pane.
        let mut w = Win::new();
        let at = Pos2::new(canvas.left() + 60.0, canvas.top() - 10.0);
        w.press(Primary, at, at);
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Vector));
        w.app.dispatch(Command::TogglePane(Pane::Vector));
        w.idle(3);
        assert_eq!(w.app.copy_focus, CopyFocus::Diagram(Pane::Vector), "the caption's window, hidden, keeps the focus");
        // A property grid, the whole group hidden: the grid's window keeps it and copy takes the document alone.
        let mut w = Win::new();
        w.app.active_doc_mut().unwrap().selection.insert(app::ChanKey::Analog(0));
        w.app.dispatch(Command::ActivatePane(Pane::Circle));
        w.idle(3);
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        w.press(Primary, grid.center(), grid.center());
        assert_eq!(w.app.copy_focus, CopyFocus::Grid(Pane::Circle));
        for p in [Pane::Circle, Pane::Vector, Pane::Differential] {
            w.app.dispatch(Command::TogglePane(p));
        }
        w.idle(3);
        assert_eq!(w.app.copy_focus, CopyFocus::Grid(Pane::Circle), "the group is gone, the grid's window keeps the focus");
        w.app.dispatch(Command::Copy);
        assert_eq!(w.app.clip.picture(), None);
        assert!(w.app.clip.get().is_some_and(|t| t.contains("<channels")), "the document alone");
        // Shown again, the same window: the focus is still on it.
        w.app.dispatch(Command::TogglePane(Pane::Circle));
        w.idle(3);
        assert_eq!(w.app.copy_focus, CopyFocus::Grid(Pane::Circle));
    }

    /// The same through the pane's caption «×» (#77): the press on the
    /// button first gives the pane the focus, and closing takes it back.
    #[test]
    fn closing_a_pane_with_its_caption_button_returns_the_focus_to_the_view() {
        use app::{CopyFocus, Pane};
        use docking::Unit;
        // The caption sits right over the pane's window, its «×» 11 px in from the group's right edge.
        let close_button = |w: &Win, unit: Unit, window_top: f32| {
            let r = w.app.dock_rects[unit as usize].expect("the group's rect");
            Pos2::new(r.right() - dpi::s(11.0), window_top - dpi::s(10.0))
        };
        // A focused diagram.
        let mut w = Win::new();
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Frequency).expect("the frequency diagram").1;
        w.press(egui::PointerButton::Primary, canvas.center(), canvas.center());
        assert_eq!(w.app.copy_focus, CopyFocus::Diagram(Pane::Frequency));
        let at = close_button(&w, Unit::Frequency, canvas.top());
        w.press(egui::PointerButton::Primary, at, at);
        assert!(!w.app.panes.visible(Pane::Frequency), "the caption button closed the pane");
        assert_eq!(w.app.copy_focus, CopyFocus::View, "the diagram's pane closed");
        let before = w.view_state();
        w.key(egui::Key::ArrowRight);
        assert_ne!(w.view_state(), before, "the arrow key reaches the view");
        // A focused property grid.
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.idle(3);
        let grid = *w.app.pane_grid.first().expect("the differential pane's grid");
        w.press(egui::PointerButton::Primary, grid.center(), grid.center());
        assert_eq!(w.app.copy_focus, CopyFocus::Grid(Pane::Differential));
        let at = close_button(&w, Unit::Differential, grid.top());
        w.press(egui::PointerButton::Primary, at, at);
        assert!(!w.app.panes.visible(Pane::Differential), "the caption button closed the pane");
        assert_eq!(w.app.copy_focus, CopyFocus::View, "the grid's pane closed");
        let before = w.view_state();
        w.key(egui::Key::ArrowRight);
        assert_ne!(w.view_state(), before, "the arrow key reaches the view");
    }

    /// The other ways to close a pane after a caption press gave it the
    /// focus (#77): the pane menu's «Скрыть» (the right-click that opens the
    /// menu gives the focus), and the ribbon's toggle.
    #[test]
    fn closing_a_pane_by_the_menu_or_the_ribbon_after_a_caption_press_returns_the_focus() {
        use app::{Command, CopyFocus, Pane};
        use egui::PointerButton::{Primary, Secondary};
        let caption = |w: &Win| {
            let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Frequency).expect("the frequency diagram").1;
            Pos2::new(canvas.left() + 60.0, canvas.top() - 10.0)
        };
        // The pane menu's last entry, «Скрыть».
        let mut w = Win::new();
        let at = caption(&w);
        w.press(Secondary, at, at);
        assert!(w.app.pane_menu.is_some(), "the pane menu opens");
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Frequency), "the right-click gave the pane the focus");
        for _ in 0..5 {
            w.key(egui::Key::ArrowDown);
        }
        w.key(egui::Key::Enter);
        w.idle(3);
        assert!(!w.app.panes.visible(Pane::Frequency), "the menu hid the pane");
        assert_eq!(w.app.copy_focus, CopyFocus::View, "the menu's Скрыть");
        let before = w.view_state();
        w.key(egui::Key::ArrowRight);
        assert_ne!(w.view_state(), before, "the arrow key reaches the view");
        // The ribbon's toggle.
        let mut w = Win::new();
        let at = caption(&w);
        w.press(Primary, at, at);
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Frequency));
        w.app.dispatch(Command::TogglePane(Pane::Frequency));
        w.idle(3);
        assert_eq!(w.app.copy_focus, CopyFocus::View, "the ribbon toggle");
        let before = w.view_state();
        w.key(egui::Key::ArrowRight);
        assert_ne!(w.view_state(), before, "the arrow key reaches the view");
    }

    /// The caption «×» of a slid-out auto-hidden pane closes it (the ribbon's
    /// toggle only shows such a pane); a group stays out with its next tab in
    /// front and the press's focus on its window (#64, #77, #143).
    #[test]
    fn the_caption_button_closes_a_slid_out_pane() {
        use app::{CopyFocus, Pane};
        use docking::{Edge, Spot, Unit};
        for edge in [Edge::Bottom, Edge::Right] {
            let mut w = Win::new();
            w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge, outer: true };
            w.app.docks.0[Unit::Vector as usize].pin();
            w.idle(3);
            w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
            w.idle(30);
            let area = w.app.dock_areas[Unit::Vector as usize].expect("slid out");
            let at = Pos2::new(area.right() - dpi::s(11.0), area.top() + dpi::s(10.0));
            w.press(egui::PointerButton::Primary, at, at);
            w.idle(30);
            assert!(!w.app.panes.visible(Pane::Vector), "{edge:?}: the caption button closed the pane");
            // #143 (clone): the group stays out, the next tab in front, and
            // the focus the press gave sits on the group's window.
            assert!(w.app.docks[Unit::Vector].out, "{edge:?}: the group stays slid out");
            assert!(w.app.panes.front(w.app.docks.members(Unit::Vector)).is_some_and(|p| p != Pane::Vector), "{edge:?}: another tab in front");
            assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Vector), "{edge:?}");
        }
    }

    /// A divider resize of a slid-out pane is the slid state's own (#109; VM,
    /// the original, a bottom pane): unpinned, the pane docks back at the size it
    /// had before it was pinned, and pinned again it slides out at the docked
    /// size (the resize is forgotten) — it only survives hiding and showing from
    /// the strip.
    #[test]
    fn a_divider_resize_is_forgotten_when_the_pane_docks_back() {
        use docking::{Edge, Place, Spot, Unit};
        let ev = |at, pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        for edge in [Edge::Bottom, Edge::Right] {
            // pin, slide out (resized by `drag` px or not), unpin, pin again,
            // slide out: the docked size after unpinning and the second slid
            // size, with the first one.
            let run = |drag: f32| {
                let mut w = Win::new();
                w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge, outer: true };
                // On its own: no pane beside it to share its slot.
                w.app.docks.0[Unit::Frequency as usize].spot = Spot::Edge { edge: edge.opposite(), outer: true };
                w.idle(3);
                let across = |r: Rect| if edge.vertical() { r.width() } else { r.height() };
                let pin = |w: &mut Win, pane: Rect| {
                    let at = Pos2::new(pane.right() - dpi::s(26.0), pane.top() + dpi::s(10.0));
                    w.press(egui::PointerButton::Primary, at, at);
                    w.idle(3);
                };
                let slid = |w: &mut Win| {
                    w.run(vec![egui::Event::PointerMoved(Pos2::new(300.0, 300.0))]);
                    w.idle(40);
                    let button = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
                    w.run(vec![egui::Event::PointerMoved(button)]);
                    w.idle(30);
                    w.app.dock_areas[Unit::Vector as usize].expect("slid out")
                };
                let pane = w.app.dock_rects[Unit::Vector as usize].expect("docked");
                pin(&mut w, pane);
                assert_eq!(w.app.docks[Unit::Vector].place, Place::AutoHide);
                let first = across(slid(&mut w));
                let mut area = w.app.dock_areas[Unit::Vector as usize].expect("slid out");
                if drag != 0.0 {
                    let div = w.app.dock_dividers[Unit::Vector as usize].expect("its divider");
                    let by = if edge.vertical() { Vec2::new(-drag, 0.0) } else { Vec2::new(0.0, -drag) };
                    w.run(vec![egui::Event::PointerMoved(div.center())]);
                    w.run(vec![ev(div.center(), true)]);
                    w.run(vec![egui::Event::PointerMoved(div.center() + by * 0.5)]);
                    w.run(vec![egui::Event::PointerMoved(div.center() + by)]);
                    w.idle(2);
                    w.run(vec![ev(div.center() + by, false)]);
                    w.idle(3);
                    area = w.app.dock_areas[Unit::Vector as usize].expect("slid out");
                    assert!((across(area) - first - drag).abs() <= 2.0, "{edge:?}: wider by the drag: {first} → {}", across(area));
                }
                pin(&mut w, area);
                assert_eq!(w.app.docks[Unit::Vector].place, Place::Docked, "{edge:?}: docked again");
                let back = across(w.app.dock_rects[Unit::Vector as usize].expect("docked"));
                let pane = w.app.dock_rects[Unit::Vector as usize].expect("docked");
                pin(&mut w, pane);
                (first, back, across(slid(&mut w)))
            };
            let (first, back, again) = run(0.0);
            let (first2, back2, again2) = run(100.0);
            assert_eq!(first, first2, "{edge:?}: the same first slid size");
            assert!((back2 - back).abs() <= 1.0, "{edge:?}: docked back at {back2}, as with no resize ({back})");
            assert!((again - first).abs() <= 1.0 && (again2 - first).abs() <= 1.0, "{edge:?}: pinned again it slides at {again2}, as the first time {first} ({again})");
        }
    }

    /// A slid-out side or top pane starts below the caption row, so its caption's
    /// pin and close are in reach (#110, the maintainer's decision; the original
    /// puts the pane's caption under the window's caption glass, where its
    /// buttons lie under the window's own buttons): a press on each acts, with
    /// the window's buttons present.
    #[test]
    fn a_slid_out_pane_lies_below_the_caption_row_with_its_caption_in_reach() {
        use docking::{Edge, Place, Spot, Unit};
        let cap = ribbon::caption_h();
        for edge in [Edge::Right, Edge::Left, Edge::Top] {
            for close in [false, true] {
                let case = format!("{edge:?} close={close}");
                let mut w = Win::new();
                w.app.own_caption = true;
                w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge, outer: true };
                w.app.docks.0[Unit::Vector as usize].pin();
                w.idle(3);
                w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
                w.idle(30);
                let area = w.app.dock_areas[Unit::Vector as usize].expect("slid out");
                assert!(area.top() >= cap, "{case}: below the caption row: {area:?}");
                let at = Pos2::new(area.right() - dpi::s(if close { 11.0 } else { 26.0 }), area.top() + dpi::s(10.0));
                w.press(egui::PointerButton::Primary, at, at);
                w.idle(30);
                if close {
                    assert!(!w.app.panes.visible(app::Pane::Vector), "{case}: the close acts");
                } else {
                    assert_eq!(w.app.docks[Unit::Vector].place, Place::Docked, "{case}: the pin acts");
                }
            }
        }
    }

    /// The hide check keeps the original's region (#110): a side or top pane
    /// is checked from the window's top, so the pointer resting between a top
    /// strip and its pane, or in the caption row above a side pane, keeps it out.
    #[test]
    fn the_hide_check_covers_the_band_above_a_slid_out_pane() {
        use docking::{Edge, Spot, Unit};
        for (edge, y) in [(Edge::Top, 23.0), (Edge::Right, 5.0), (Edge::Left, 5.0)] {
            let mut w = Win::new();
            w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge, outer: true };
            w.app.docks.0[Unit::Vector as usize].pin();
            w.idle(3);
            w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
            w.idle(30);
            let area = w.app.dock_areas[Unit::Vector as usize].expect("slid out");
            assert!(y < area.top(), "{edge:?}: the probe lies above the pane");
            let at = Pos2::new(area.center().x, y);
            let button = w.app.strip_hits[Unit::Vector as usize].buttons.iter().any(|b| b.contains(at));
            assert!(!button, "{edge:?}: the probe is on no button");
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.idle(60);
            assert!(w.app.docks[Unit::Vector].out, "{edge:?}: resting above the pane keeps it out");
        }
    }

    /// #128 (clone, the original): with a top strip as well, a slid-out side
    /// pane starts at the strip's top, the caption row's bottom, and its hide
    /// region with it: the pointer resting in the caption row above the pane
    /// no longer keeps it out.
    #[test]
    fn a_side_pane_beside_a_top_strip_is_not_held_from_the_windows_top() {
        use docking::{Edge, Spot, Unit};
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge: Edge::Right, outer: true };
        w.app.docks.0[Unit::Frequency as usize].spot = Spot::Edge { edge: Edge::Top, outer: true };
        w.app.docks.0[Unit::Frequency as usize].pin();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(30);
        let area = w.app.dock_areas[Unit::Vector as usize].expect("slid out");
        let at = Pos2::new(area.center().x, 5.0);
        assert!(at.y < area.top());
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.idle(60);
        assert!(!w.app.docks[Unit::Vector].out, "resting in the caption row above the pane lets it slide in");
    }

    /// A strip button lying wholly under the caption glass is hovered like any
    /// other (#110; VM, the original, a left strip with the second tab in front:
    /// the first tab's 24 px stub, 4 px under the window's top, slides its pane
    /// out when the pointer rests on it).
    #[test]
    fn a_stub_under_the_caption_glass_can_be_hovered() {
        use docking::{Edge, Spot, Unit};
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge: Edge::Left, outer: true };
        w.app.dispatch(app::Command::ActivatePane(app::Pane::Circle));
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        let cap = ribbon::caption_h();
        let stub = w.app.strip_hits[Unit::Vector as usize].buttons.iter().copied().find(|r| r.bottom() <= cap).expect("a button wholly under the glass");
        assert!(stub.height() <= 24.5, "a stub: {stub:?}");
        w.run(vec![egui::Event::PointerMoved(stub.center())]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out, "the hover slid the group out");
        assert_eq!(w.app.panes.front(w.app.docks.members(Unit::Vector)), Some(app::Pane::Vector), "with the stub's pane in front");
    }

    /// Hiding a background tab of the focused pane's group leaves the focus.
    #[test]
    fn hiding_a_background_tab_keeps_the_pane_focus() {
        use app::{Command, CopyFocus, Pane};
        let mut w = Win::new();
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).expect("the vector diagram").1;
        let at = Pos2::new(canvas.left() + 60.0, canvas.top() - 10.0);
        w.press(egui::PointerButton::Primary, at, at);
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Vector));
        w.app.dispatch(Command::TogglePane(Pane::Circle));
        w.idle(3);
        assert!(!w.app.panes.visible(Pane::Circle));
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Vector), "the shown pane keeps the focus");
    }

    /// A press on a menu or other popup over a pane gives that pane no
    /// focus.
    #[test]
    fn a_press_on_a_popup_reaches_no_pane_under_it() {
        let mut w = Win::new();
        let freq = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Frequency).map(|(_, r)| *r).expect("Частотная");
        let at = freq.center();
        let cover = Rect::from_center_size(at, Vec2::splat(60.0));
        for events in [vec![egui::Event::PointerMoved(at)], vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }], vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }], vec![]] {
            let t = w.ctx.input(|i| i.time) + 0.05;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0))), time: Some(t), events, ..Default::default() };
            let app = &mut w.app;
            let _ = w.ctx.run(raw, |ctx| {
                // A popup over Частотная, drawn before the frame's focus
                // tracking (as the menus are).
                egui::Area::new(egui::Id::new("test_popup")).order(egui::Order::Foreground).fixed_pos(cover.min).show(ctx, |ui| {
                    ui.allocate_exact_size(cover.size(), egui::Sense::click());
                });
                app.frame_ui(ctx);
            });
        }
        assert_eq!(w.app.copy_focus, app::CopyFocus::View, "the popup took the press");
    }

    /// The focus belongs to the window pressed — the slid-out pane over a
    /// docked one takes the press, and a press in a docked pane that the
    /// slid-out pane covers later does not hold that one out.
    #[test]
    fn the_focus_is_the_pressed_windows_group() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        let on = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        let away = w.app.view_rects.expect("the view").0.left_top() + Vec2::new(40.0, 40.0);
        let canvas = |w: &Win, p: app::Pane| w.app.pane_canvas.iter().rev().find(|(q, _)| *q == p).map(|(_, r)| *r);
        let press = |w: &mut Win, at: Pos2| {
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.run(vec![ev(true)]);
            w.run(vec![ev(false)]);
        };
        // Where the slid-out vector diagram will lie over the docked Частотная.
        let freq = canvas(&w, app::Pane::Frequency).expect("Частотная docked");
        w.run(vec![egui::Event::PointerMoved(on)]);
        w.idle(30);
        let over = canvas(&w, app::Pane::Vector).expect("the slid-out vector diagram").intersect(freq);
        assert!(over.is_positive(), "the slid-out pane covers Частотная: {over:?}");
        let at = over.center();
        // Pressed there while out: the vector diagram's.
        press(&mut w, at);
        assert_eq!((w.app.copy_focus, w.app.focus_group()), (app::CopyFocus::Diagram(app::Pane::Vector), Some(Unit::Vector)));
        press(&mut w, away);
        w.idle(30);
        assert!(!w.app.docks[Unit::Vector].out, "the focus in the view: back");
        // Pressed there while in: Частотная's; out again, away, it slides back.
        press(&mut w, at);
        assert_eq!((w.app.copy_focus, w.app.focus_group()), (app::CopyFocus::Diagram(app::Pane::Frequency), Some(w.app.docks.host(Unit::Frequency))));
        w.run(vec![egui::Event::PointerMoved(on)]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out && w.app.dock_areas[Unit::Vector as usize].is_some_and(|a| a.contains(at)));
        w.run(vec![egui::Event::PointerMoved(away)]);
        w.idle(30);
        assert!(!w.app.docks[Unit::Vector].out, "Частотная's focus does not hold the vector pane");
    }

    /// The hide check finds the pointer on the strip button by position
    /// (`0x10087a4a`: PtInRect on the button's rect, whatever window is on
    /// top there): with a window over the button the pane stays out,
    /// though the button no longer has the hover.
    #[test]
    fn a_window_over_the_strip_button_keeps_the_pane_out() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(2);
        let on = w.app.strip_hits[Unit::Vector as usize].buttons[0].center();
        w.run(vec![egui::Event::PointerMoved(on)]);
        w.idle(30);
        assert!(w.app.docks[Unit::Vector].out, "the hover slid it out");
        // A window over the button (as a dialog or tooltip would be).
        let cover = Rect::from_center_size(on, Vec2::splat(60.0));
        for k in 0..40 {
            let t = w.ctx.input(|i| i.time) + 0.05;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0))), time: Some(t), ..Default::default() };
            let app = &mut w.app;
            let _ = w.ctx.run(raw, |ctx| {
                app.frame_ui(ctx);
                egui::Area::new(egui::Id::new("test_cover")).order(egui::Order::Tooltip).fixed_pos(cover.min).show(ctx, |ui| {
                    ui.allocate_exact_size(cover.size(), egui::Sense::click());
                });
            });
            let d = w.app.docks[Unit::Vector];
            assert!(k < 2 || !d.on_button(), "frame {k}: the cover takes the button's hover");
            assert!(d.out, "frame {k}: the pointer on the button keeps it out");
        }
    }

    /// The slid-out pane's area claims its click rect once per frame
    /// without a clash at a fractional scale (debug builds flag a clash
    /// on screen), sliding out and back on every edge.
    #[test]
    fn slid_out_pane_claims_its_rect_without_an_id_clash() {
        use docking::{Edge, Spot, Unit};
        for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(1.25);
            let mut app = demo(&[]);
            app.docks = docking::Docks::one_group();
            app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge, outer: true };
            app.docks.0[Unit::Vector as usize].pin();
            for k in 0..60 {
                // Out from frame 3 to 29 (no pointer: the check would close it).
                let d = &mut app.docks.0[Unit::Vector as usize];
                d.out = (3..30).contains(&k);
                let t = ctx.input(|i| i.time) + 0.05;
                let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1056.0, 688.0))), time: Some(t), ..Default::default() };
                let shapes = format!("{:?}", ctx.run(raw, |ctx| app.frame_ui(ctx)).shapes);
                assert!(!shapes.contains("Double use"), "{edge:?}: frame {k}: an id clash");
                // The rect the hide check and the presses use is the one drawn.
                if let Some(r) = app.dock_areas[Unit::Vector as usize] {
                    let drawn = ctx.memory(|m| m.area_rect(egui::Id::new(("fv_slide", Unit::Vector)))).expect("drawn");
                    assert!((drawn.min - r.min).length() < 0.01 && (drawn.max - r.max).length() < 0.01, "{edge:?}: frame {k}: drawn {drawn:?}, kept {r:?}");
                }
            }
        }
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

    /// #126 (clone, the original): a slid-out pane is as large across its
    /// edge as its window was docked, the 4 px divider outside it — the
    /// bottom Частотная's pane 144 px tall slid out as docked, with no margin
    /// around its window; a tabbed group's pane is the docked size less its
    /// tab control's 3 px frame and the divider's place (the right group).
    #[test]
    fn a_slid_out_pane_is_as_large_as_it_was_docked() {
        use docking::Unit;
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.idle(4);
        let slide_out = |w: &mut Win, u: Unit| -> egui::Rect {
            // A click on the caption's pin.
            let pane = if u == Unit::Vector { app::Pane::Vector } else { app::Pane::Frequency };
            let r = w.app.pane_canvas.iter().find(|(p, _)| *p == pane).map(|(_, r)| *r).expect("the pane");
            let at = Pos2::new(w.app.dock_rects[u as usize].expect("docked").right() - 26.0, r.top() - 10.0);
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.run(vec![ev(true), ev(false)]);
            w.idle(3);
            assert_eq!(w.app.docks[u].place, docking::Place::AutoHide, "{u:?} pinned");
            let button = w.app.strip_hits[u as usize].buttons[0].center();
            w.run(vec![egui::Event::PointerMoved(button)]);
            w.idle(30);
            assert!(w.app.docks[u].out, "{u:?} slid out");
            w.app.dock_areas[u as usize].expect("its rect")
        };
        // The lone bottom pane: the docked height, the window with no inset.
        let docked = w.app.dock_rects[Unit::Frequency as usize].expect("docked");
        let area = slide_out(&mut w, Unit::Frequency);
        assert_eq!(area.height(), docked.height(), "the pane as tall as docked: {area:?} vs {docked:?}");
        let div = w.app.dock_dividers[Unit::Frequency as usize].expect("its divider");
        assert_eq!((div.height(), div.bottom()), (docking::DIVIDER_PX, area.top()), "the divider above it");
        // The right group (two tabs): the docked width less 3 + 4 px.
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.idle(4);
        let docked = w.app.dock_rects[Unit::Vector as usize].expect("docked");
        let area = slide_out(&mut w, Unit::Vector);
        assert_eq!(area.width(), docked.width() - docking::GROUP_FRAME_PX - docking::DIVIDER_PX, "the group's pane: {area:?} vs {docked:?}");
        // A group on the bottom edge: its tab strip lies across the extent
        // (clone: 48 px lower than docked at 100 %).
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.app.docks.0[Unit::Vector as usize].spot = docking::Spot::Edge { edge: docking::Edge::Bottom, outer: true };
        w.idle(4);
        let docked = w.app.dock_rects[Unit::Vector as usize].expect("docked");
        let area = slide_out(&mut w, Unit::Vector);
        assert_eq!(area.height(), docked.height() - 48.0, "the bottom group's pane: {area:?} vs {docked:?}");
    }

    /// #127: a pane that shares its docking slot with a neighbour docks back
    /// at the size it had when it was pinned and unpinned (clone: the original
    /// keeps it to within a few px; ours handed it the neighbour's share, and
    /// a bottom Частотная beside Дифференциальной grew or shrank by the
    /// difference of the two). The owner of the slot, Частотная, is the one
    /// whose share the container did not store.
    #[test]
    fn a_pane_sharing_its_slot_docks_back_at_its_size() {
        use docking::{Edge, Place, Spot, Unit};
        for (neighbour_share, resize) in [(500.0, None), (500.0, Some(280.0)), (250.0, None)] {
            let mut w = Win::new();
            w.app.docks = docking::Docks::default();
            w.app.docks.0[Unit::Differential as usize].spot = Spot::Beside(Unit::Frequency, Edge::Right);
            w.app.docks.0[Unit::Differential as usize].extent = Some(neighbour_share);
            w.idle(4);
            if let Some(e) = resize {
                w.app.docks.0[Unit::Differential as usize].extent = Some(e);
                w.idle(3);
            }
            let before = w.app.dock_rects[Unit::Frequency as usize].expect("docked").width();
            let r = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Frequency).map(|(_, r)| *r).expect("the pane");
            let at = Pos2::new(w.app.dock_rects[Unit::Frequency as usize].unwrap().right() - 26.0, r.top() - 10.0);
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            w.run(vec![egui::Event::PointerMoved(at)]);
            w.run(vec![ev(true), ev(false)]);
            w.idle(3);
            assert_eq!(w.app.docks[Unit::Frequency].place, Place::AutoHide, "pinned");
            w.app.docks.0[Unit::Frequency as usize].pin();
            w.idle(4);
            let after = w.app.dock_rects[Unit::Frequency as usize].expect("docked again").width();
            assert!((after - before).abs() < 1.0, "share {neighbour_share} {resize:?}: it docks back at {after}, was {before}");
        }
    }

    /// #127, across the block: a block whose divider was dragged keeps that
    /// size when its owner is pinned (the neighbour takes the block over) and
    /// unpinned. Real drag of the block's grip.
    #[test]
    fn a_dragged_block_keeps_its_size_through_a_pin_and_unpin() {
        use docking::{Edge, Place, Spot, Unit};
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.app.docks.0[Unit::Differential as usize].spot = Spot::Beside(Unit::Frequency, Edge::Right);
        w.app.docks.0[Unit::Differential as usize].extent = Some(500.0);
        w.idle(4);
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let h0 = w.app.dock_rects[Unit::Frequency as usize].expect("docked").height();
        let top = w.app.dock_rects[Unit::Frequency as usize].unwrap().top();
        let from = Pos2::new(100.0, top - 2.0);
        w.run(vec![egui::Event::PointerMoved(from)]);
        w.run(vec![ev(from, true)]);
        for k in 1..=6 {
            w.run(vec![egui::Event::PointerMoved(from - Vec2::new(0.0, 10.0 * k as f32))]);
        }
        w.run(vec![ev(from - Vec2::new(0.0, 60.0), false)]);
        w.idle(3);
        let h1 = w.app.dock_rects[Unit::Frequency as usize].unwrap().height();
        assert!((h1 - h0 - 60.0).abs() < 2.0, "the grip drag grew the block: {h0} -> {h1}");
        // Pin Частотная by its caption; Дифференциальная keeps the block's size.
        let r = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Frequency).map(|(_, r)| *r).expect("the pane");
        let at = Pos2::new(w.app.dock_rects[Unit::Frequency as usize].unwrap().right() - 26.0, r.top() - 10.0);
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(at, true), ev(at, false)]);
        w.idle(4);
        assert_eq!(w.app.docks[Unit::Frequency].place, Place::AutoHide);
        let d = w.app.dock_rects[Unit::Differential as usize].expect("it took the block").height();
        assert!((d - h1).abs() < 2.0, "the neighbour's block keeps the dragged size: {d} vs {h1}");
        w.app.docks.0[Unit::Frequency as usize].pin();
        w.idle(4);
        let back = w.app.dock_rects[Unit::Frequency as usize].expect("docked again").height();
        assert!((back - h1).abs() < 2.0, "it docks back at {back}, was {h1}");
    }

    /// #127: the frame shrunk and grown back leaves the blocks' stored
    /// extents alone (only a grip drag writes one).
    #[test]
    fn a_frame_shrunk_and_grown_back_keeps_the_extents() {
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.idle(4);
        let before: Vec<Option<f32>> = docking::UNITS.iter().map(|&u| w.app.docks[u].extent).collect();
        for size in [Vec2::new(420.0, 360.0), Vec2::new(1320.0, 860.0)] {
            for _ in 0..3 {
                frames_held(&mut w.app, &w.ctx, size, vec![vec![]], w.held);
            }
        }
        let after: Vec<Option<f32>> = docking::UNITS.iter().map(|&u| w.app.docks[u].extent).collect();
        assert_eq!(before, after, "no clamp is stored");
    }

    /// #126 through the pane menu: «Автоматически скрывать» shares the
    /// caption pin's rule: the slid-out pane is as large as it was docked.
    #[test]
    fn auto_hide_from_the_pane_menu_slides_out_at_the_docked_size() {
        use docking::{Place, Unit};
        let mut w = Win::new();
        w.app.docks = docking::Docks::default();
        w.app.docks.0[Unit::Frequency as usize].extent = Some(260.0);
        w.idle(4);
        let docked = w.app.dock_rects[Unit::Frequency as usize].expect("docked").height();
        let f = w.app.pane_canvas.iter().find(|(q, _)| *q == app::Pane::Frequency).map(|(_, r)| *r).expect("Частотная drawn");
        let at = Pos2::new(f.left() + 80.0, f.top() - 10.0);
        w.press(egui::PointerButton::Secondary, at, at);
        assert!(w.app.pane_menu.is_some(), "the pane menu opens");
        for _ in 0..4 {
            w.key(egui::Key::ArrowDown);
        }
        w.key(egui::Key::Enter);
        w.idle(3);
        assert_eq!(w.app.docks[Unit::Frequency].place, Place::AutoHide, "Автоматически скрывать");
        let button = w.app.strip_hits[Unit::Frequency as usize].buttons[0].center();
        w.run(vec![egui::Event::PointerMoved(button)]);
        w.idle(30);
        let area = w.app.dock_areas[Unit::Frequency as usize].expect("slid out");
        assert!((area.height() - docked).abs() < 1.0, "slid out at {} but docked {docked}", area.height());
    }

    /// #143: closing the last visible tab of a slid-out group hides the group
    /// and the view has the focus (not seen on a clone: the group is gone).
    #[test]
    fn closing_the_last_tab_of_a_slid_out_group_gives_the_view_the_focus() {
        use app::{CopyFocus, Pane};
        use docking::{Edge, Spot, Unit};
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge: Edge::Bottom, outer: true };
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(30);
        for p in [Pane::Circle, Pane::Differential] {
            if w.app.docks.members(Unit::Vector).any(|u| u as usize == p as usize) {
                w.app.dispatch(app::Command::HidePane(p));
            }
        }
        w.app.copy_focus = CopyFocus::Pane(Pane::Vector);
        w.app.close_pane(Pane::Vector);
        assert_eq!(w.app.copy_focus, CopyFocus::View);
    }

    /// #124 (clone, the original): a double-click on a tab of a docked
    /// group's strip floats that pane alone, the front one or not (mfc120u's
    /// tab double-click `0x10068d99` calls the detach `0x10068657` in mode 2
    /// when the group has more than one tab); a single click floats nothing.
    #[test]
    fn a_double_click_on_a_tab_floats_that_pane() {
        use docking::{Place, Unit};
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let tab_of = |w: &Win, p: app::Pane| w.app.pane_tab_rects.iter().find(|(q, _)| *q == p).map(|(_, r)| r.center()).expect("the tab");
        // A single click only brings the tab to the front.
        let mut w = Win::new();
        w.idle(2);
        let at = tab_of(&w, app::Pane::Circle);
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(at, true), ev(at, false)]);
        w.idle(3);
        assert_eq!((w.app.docks[Unit::Circle].place, w.app.docks[Unit::Vector].place), (Place::Docked, Place::Docked), "a click floats nothing");
        // A double-click on the other tab floats that pane; the group stays.
        let mut w = Win::new();
        w.idle(2);
        let at = tab_of(&w, app::Pane::Circle);
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(at, true), ev(at, false)]);
        w.run(vec![ev(at, true), ev(at, false)]);
        w.idle(3);
        assert_eq!((w.app.docks[Unit::Circle].place, w.app.docks[Unit::Vector].place), (Place::Floating, Place::Docked), "the double-clicked tab floats alone");
        assert_eq!(w.app.copy_focus, app::CopyFocus::Pane(app::Pane::Circle), "the focus is the floated pane's window (clone)");
        // On the front tab: the group's own pane floats, the next tab takes the group.
        let mut w = Win::new();
        w.idle(2);
        let at = tab_of(&w, app::Pane::Vector);
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(at, true), ev(at, false)]);
        w.run(vec![ev(at, true), ev(at, false)]);
        w.idle(3);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating, "the front tab floats too");
        assert_eq!(w.app.docks[Unit::Circle].place, Place::Docked);
    }

    /// A tab pulled out of a tab strip follows the caption drag's distance rule
    /// (#111; `0x10068657` reads the metric once, `>=` on either axis — the
    /// caption's rule `0x10086f3a` is a strict `>` — the distance scaled with
    /// the display as the caption's, by the maintainer's decision, L9.411) once the pointer has left the strip (the
    /// original needs that: `0x100690b4` calls the detach only outside the tab
    /// area; VM: 5 and 8 px up inside the strip float nothing, 14 px up out of it
    /// floats the pane), with the caption drag's move cursor from the press on.
    #[test]
    fn a_tab_pulled_out_follows_the_drag_distance_and_shows_the_move_cursor() {
        use docking::{Place, Unit};
        use egui::CursorIcon::{Default as Arrow, Move};
        let ev = |pos, pressed, button| egui::Event::PointerButton { pos, button, pressed, modifiers: Default::default() };
        let primary = egui::PointerButton::Primary;
        for dpi in [96, 120, 144] {
            dpi::set(dpi);
            let reach = dpi::s(docking::DRAG_PX);
            // Pressed on the tab's top row, dragged straight up out of the strip
            // (whose top is the tab's less 1 px) by `dist` px.
            let pull = |dist: f32| {
                let mut w = Win::new();
                w.idle(2);
                let tab = w.app.pane_tab_rects.iter().find(|(p, _)| *p == app::Pane::Circle).map(|(_, r)| *r).expect("the Круговая tab");
                let at = Pos2::new(tab.center().x, tab.top() + 0.5);
                assert_eq!(w.run(vec![egui::Event::PointerMoved(at)]), Arrow, "{dpi}: hovering a tab leaves the arrow");
                assert_eq!(w.run(vec![ev(at, true, primary)]), Move, "{dpi}: the press arms the drag: move cursor");
                let to = at - Vec2::new(0.0, dist);
                assert_eq!(w.run(vec![egui::Event::PointerMoved(to)]), Move, "{dpi}: still");
                w.idle(2);
                let floated = w.app.docks[Unit::Circle].place == Place::Floating;
                w.run(vec![ev(to, false, primary)]);
                w.idle(3);
                assert_eq!(w.run(vec![egui::Event::PointerMoved(Pos2::new(5.0, 5.0))]), Arrow, "{dpi}: released: the arrow");
                floated
            };
            assert!(!pull(reach - 0.5), "{dpi}: {} px up is short of the distance {reach}", reach - 0.5);
            assert!(pull(reach), "{dpi}: {reach} px up, out of the strip: floated (the rule is >=)");
        }
        dpi::set(96);
        // Inside the strip nothing floats, however far (the pointer must leave the
        // tab area), and only the left button arms the drag.
        let mut w = Win::new();
        w.idle(2);
        let tab = w.app.pane_tab_rects.iter().find(|(p, _)| *p == app::Pane::Circle).map(|(_, r)| *r).expect("the Круговая tab");
        let at = Pos2::new(tab.center().x, tab.top() + 8.0);
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(at, true, primary)]);
        w.run(vec![egui::Event::PointerMoved(at - Vec2::new(0.0, 7.0))]);
        w.idle(2);
        assert_eq!(w.app.docks[Unit::Circle].place, Place::Docked, "7 px up inside the strip floats nothing");
        w.run(vec![ev(at, false, primary)]);
        w.idle(2);
        let sec = egui::PointerButton::Secondary;
        w.app.pane_menu = None;
        w.run(vec![egui::Event::PointerMoved(at)]);
        assert_ne!(w.run(vec![ev(at, true, sec)]), Move, "a right press arms nothing");
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
        // The press the window procedure noted (`winkeys`) pastes at once, whatever the clipboard
        // holds, and Ctrl released first or not, the key-up is not a second paste (#26).
        let down = egui::Event::Key { key: egui::Key::V, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::CTRL };
        w.run(vec![down]);
        assert_eq!(pending(&w), 1, "Ctrl+V pastes at the key press");
        // (Undone first, so that a second paste would be enabled.)
        w.app.dispatch(app::Command::Undo);
        assert_eq!(pending(&w), 0);
        w.run(vec![up.clone()]);
        assert_eq!(pending(&w), 0, "the key-up pastes nothing more: the press was the paste");
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

    /// #44 at the lane menu's call site: near the screen's foot the menu's bottom edge lies at the point.
    #[test]
    fn the_lane_menu_near_the_foot_opens_upward_from_the_point() {
        let mut w = Win::new();
        let at = Pos2::new(300.0, 860.0 - 8.0);
        w.app.lane_menu_serial += 1;
        let serial = w.app.lane_menu_serial;
        w.app.lane_menu = Some(lanes::LaneMenu { serial, pos: at, kind: lanes::LaneMenuKind::Channels });
        w.idle(4);
        let menu = w.ctx.memory(|m| m.area_rect(egui::Id::new("fv_lane_menu").with(serial))).expect("the menu shown");
        assert!((menu.bottom() - at.y).abs() <= 0.5, "bottom {} at the point {}", menu.bottom(), at.y);
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
        assert!((menu.bottom() - at.y).abs() <= 0.5, "opened upward from the point: its bottom at it, not at the screen's foot");
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
        assert!(matches!(w.app.copy_focus, app::CopyFocus::Grid(_)), "{:?}", w.app.copy_focus);
        w.app.dispatch(app::Command::Copy);
        assert_eq!(w.app.clip.picture(), None, "no picture with the grid focused");
        assert!(w.app.clip.get().is_some_and(|t| t.contains("<channels")), "the document alone");
        // A press in the diagram below the grid gives the pane the focus.
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Circle).expect("the circle canvas").1;
        w.press(Primary, canvas.center(), canvas.center());
        assert_eq!(w.app.copy_focus, app::CopyFocus::Diagram(app::Pane::Circle));
    }

    /// A press on a pane's tab — the front one's too, with either button —
    /// gives the group's front pane the focus (clone: a grid focused in the
    /// circle pane, the vector tab pressed: the vector pane's window holds
    /// the focus, not the hidden circle pane's grid; #112, #65).
    #[test]
    fn a_press_on_a_tab_gives_the_front_pane_the_focus() {
        use app::{Command, CopyFocus, Pane};
        use egui::PointerButton::{Primary, Secondary};
        let tab = |w: &Win, p| w.app.pane_tab_rects.iter().find(|(q, _)| *q == p).map(|(_, r)| r.center()).expect("the tab");
        let mut w = Win::new();
        w.app.dispatch(Command::ActivatePane(Pane::Circle));
        w.idle(3);
        let grid = *w.app.pane_grid.first().expect("the circle pane's grid");
        w.press(Primary, grid.center(), grid.center());
        assert_eq!(w.app.copy_focus, CopyFocus::Grid(Pane::Circle));
        let at = tab(&w, Pane::Vector);
        w.press(Primary, at, at);
        assert_eq!(w.app.panes.front(w.app.docks.members(docking::Unit::Vector)), Some(Pane::Vector), "the tab came to the front");
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Vector), "the shown pane holds the focus");
        // Copy now takes the shown pane's diagram, not the hidden grid's document.
        w.app.dispatch(Command::Copy);
        assert!(w.app.clip.picture().is_some() && w.app.clip.get().is_none(), "the vector diagram's picture alone");
        // The front tab, pressed while a diagram of the group holds the focus.
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).expect("the vector diagram").1;
        w.press(Primary, canvas.center(), canvas.center());
        assert_eq!(w.app.copy_focus, CopyFocus::Diagram(Pane::Vector));
        let at = tab(&w, Pane::Vector);
        w.press(Primary, at, at);
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Vector), "the front tab too");
        // A right press activates the tab and gives the pane the focus.
        w.app.copy_focus = CopyFocus::View;
        let at = tab(&w, Pane::Circle);
        w.press(Secondary, at, at);
        assert_eq!(w.app.panes.front(w.app.docks.members(docking::Unit::Vector)), Some(Pane::Circle));
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Vector), "the right press");
    }

    /// Every dialog is modal, and the frame it gives the activation back to
    /// hands the focus to its view (clone: a diagram's pane focused, «Каналы»
    /// opened and closed: the view holds it; the ОМП parameters' tree held it
    /// while up): a focus held by a pane or by another window is the view's
    /// again once the dialog is gone (#65).
    #[test]
    fn a_dialog_that_closes_hands_the_focus_to_the_view() {
        use app::CopyFocus;
        for before in [CopyFocus::Diagram(app::Pane::Vector), CopyFocus::Grid(app::Pane::Circle), CopyFocus::Pane(app::Pane::Vector), CopyFocus::Other] {
            let mut w = Win::new();
            w.app.copy_focus = before;
            w.app.color_dialog = Some(egui::Color32::RED);
            w.idle(3);
            assert!(w.app.modal_up);
            assert_eq!(w.app.copy_focus, before, "{before:?}: while the dialog is up it is the dialog's, not changed");
            w.app.color_dialog = None;
            w.idle(3);
            assert!(!w.app.modal_up);
            assert_eq!(w.app.copy_focus, CopyFocus::View, "{before:?}: the dialog closed");
            let at = w.view_state();
            w.key(egui::Key::ArrowRight);
            assert_ne!(w.view_state(), at, "{before:?}: the view's keys work again");
        }
    }

    /// The frame's own windows that take the focus (clone: the merge bar's
    /// combo box and buttons, the document tabs' ▾ button — each a window
    /// without an untitled child, so copy puts the view's picture and the
    /// document, while the view's keys go to that window), and the
    /// document's tab, which gives it to the view (#65).
    #[test]
    fn the_frames_own_windows_take_the_copy_focus() {
        use app::{ChanKey, CopyFocus, Pane};
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).expect("the vector diagram").1;
        let focus_diagram = |w: &mut Win| {
            w.press(Primary, canvas.center(), canvas.center());
            assert_eq!(w.app.copy_focus, CopyFocus::Diagram(Pane::Vector));
        };
        // The tab list's button at the strip's right end.
        let strip = w.app.mdi_strips[0];
        let list = Pos2::new(strip.right() - 10.0, strip.center().y);
        focus_diagram(&mut w);
        w.press(Primary, list, list);
        assert_eq!(w.app.copy_focus, CopyFocus::Other, "the tab list's button");
        // Copy asks that window: no diagram, the view's picture and the document.
        w.app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        w.app.dispatch(app::Command::Copy);
        assert!(w.app.clip.picture().is_some() && w.app.clip.get().is_some_and(|t| t.contains("<channels")));
        let before = w.view_state();
        w.key(egui::Key::ArrowRight);
        assert_eq!(w.view_state(), before, "the arrow key goes to that window, not the view");
        // The list's popup is closed again by a press elsewhere; a document's tab gives the view the focus.
        w.key(egui::Key::Escape);
        let tab = w.app.doc_tab_rects[0].center();
        focus_diagram(&mut w);
        w.press(Primary, tab, tab);
        assert_eq!(w.app.copy_focus, CopyFocus::View, "the document's tab");
        // The merge bar after a paste: the combo box and the two buttons that are enabled.
        let mut w = Win::new();
        w.app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        w.app.copy_channels_for_merge();
        w.app.paste_channels_for_merge();
        w.idle(3);
        assert!(w.app.merge.is_some(), "the merge bar is up");
        let controls = w.app.focus_other.clone();
        assert_eq!(controls.len(), 4, "the method combo, «Синхронизация», «Объединить», the tab list's button: {controls:?}");
        // (The last one is the tab list's; «Объединить» takes the bar away.)
        for r in controls.into_iter().take(3) {
            w.app.copy_focus = CopyFocus::Diagram(Pane::Vector);
            w.press(Primary, r.center(), r.center());
            assert_eq!(w.app.copy_focus, CopyFocus::Other, "{r:?}");
        }
    }

    /// The merge bar's method combo box holding the focus takes ↑ / ↓ as a
    /// combo box does, one choice each (clone: a press on it, then ↓ / ↑
    /// change the method; Tab goes nowhere, a bar is no dialog), and the
    /// view's arrow keys stay off (#145).
    #[test]
    fn the_merge_bars_combo_takes_the_arrow_keys() {
        use app::ChanKey;
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        w.app.copy_channels_for_merge();
        w.app.paste_channels_for_merge();
        w.idle(3);
        let combo = w.app.focus_other[0];
        let method = |w: &Win| w.app.merge.as_ref().unwrap().method;
        assert_eq!(method(&w), 0);
        // The press opens the list; the click at the focus window closes it.
        w.press(Primary, combo.center(), combo.center());
        w.key(egui::Key::Escape);
        let before = w.view_state();
        w.key(egui::Key::ArrowDown);
        assert_eq!(method(&w), 1, "↓ picks the next method");
        w.key(egui::Key::ArrowDown);
        assert_eq!(method(&w), 2);
        w.key(egui::Key::ArrowDown);
        assert_eq!(method(&w), 2, "the last one stays");
        w.key(egui::Key::ArrowUp);
        assert_eq!(method(&w), 1, "↑ the previous");
        assert_eq!(w.view_state(), before, "the view's keys are off");
        // F4 drops the list (Space does nothing); Esc closes it, the focus stays.
        let open = |w: &Win| w.ctx.data(|d| d.get_temp::<bool>(egui::Id::new("merge_method").with("open"))).unwrap_or(false);
        w.key(egui::Key::Space);
        assert!(!open(&w), "Space does nothing");
        w.key(egui::Key::F4);
        assert!(open(&w), "F4 drops the list");
        w.key(egui::Key::Escape);
        assert!(!open(&w));
        w.key(egui::Key::ArrowUp);
        assert_eq!(method(&w), 0, "the focus stayed");
    }

    /// «Синхронизация» holding the focus (a press let go outside it) is
    /// pressed by Space and by Enter (clone: the status line fills with
    /// neither a click), as the tab list's ▾ button opens its list by both
    /// and takes no arrow (#145).
    #[test]
    fn the_merge_bars_buttons_and_the_tab_lists_button_take_space_and_enter() {
        use app::ChanKey;
        use egui::PointerButton::Primary;
        for key in [egui::Key::Space, egui::Key::Enter] {
            let mut w = Win::new();
            w.app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
            w.app.copy_channels_for_merge();
            w.app.paste_channels_for_merge();
            w.idle(3);
            let sync = w.app.focus_other[1];
            w.press(Primary, sync.center(), sync.center() + Vec2::new(0.0, 200.0));
            assert!(w.app.merge.as_ref().unwrap().status.is_empty(), "no click: nothing synchronised");
            w.key(key);
            assert!(!w.app.merge.as_ref().unwrap().status.is_empty(), "{key:?} presses the focused button");
        }
        // The ▾ at the document strip's end.
        for (key, opens) in [(egui::Key::Space, true), (egui::Key::Enter, true), (egui::Key::ArrowDown, false), (egui::Key::ArrowUp, false)] {
            let mut w = Win::new();
            let strip = w.app.mdi_strips[0];
            let list = Pos2::new(strip.right() - 10.0, strip.center().y);
            // The press opens the list; Esc closes it, the button keeps the focus.
            w.press(Primary, list, list);
            w.key(egui::Key::Escape);
            w.idle(2);
            let open = |w: &Win| w.app.doc_list_open;
            assert!(!open(&w), "closed");
            w.key(key);
            w.idle(2);
            assert_eq!(open(&w), opens, "{key:?}");
        }
    }

    /// A press on the ▾ of a document group that is not the active one
    /// (clone, two groups): the button holds the focus and the active
    /// document stays (the title does not change); a pick from its list
    /// activates that document, which focuses its view (#148).
    #[test]
    fn the_list_button_of_another_document_group_keeps_the_focus() {
        use app::CopyFocus;
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::Demo);
        w.idle(2);
        w.app.new_group_after(1, 0, true);
        w.idle(3);
        assert_eq!(w.app.group_count(), 2);
        let active = w.app.active;
        let strip = w.app.mdi_strips[0];
        let list = Pos2::new(strip.right() - 10.0, strip.center().y);
        w.press(Primary, list, list);
        w.idle(3);
        assert_eq!(w.app.active, active, "the press activates nothing");
        assert_eq!(w.app.copy_focus, CopyFocus::Other, "the button holds the focus");
        assert!(w.app.doc_list_open, "its list is open");
    }

    /// «Коэффициенты» (DIALOG 477): the buttons have no WS_GROUP, so with the
    /// list before them they make one span: ↑ from ОК focuses the list (clone),
    /// not Отмена (#25).
    #[test]
    fn the_buttons_of_a_dialog_share_a_span_with_the_control_before_them() {
        let mut w = Win::new();
        w.app.dispatch(app::Command::Coefficients);
        w.idle(4);
        let focused = |w: &Win| w.ctx.memory(|m| m.focused());
        let list = focused(&w);
        w.key(egui::Key::Tab);
        w.idle(2);
        let ok = focused(&w);
        w.key(egui::Key::Tab);
        w.idle(2);
        let cancel = focused(&w);
        assert!(ok.is_some() && cancel.is_some() && ok != cancel);
        w.key(egui::Key::Tab);
        w.idle(2);
        w.key(egui::Key::Tab);
        w.idle(2);
        assert_eq!(focused(&w), ok, "round the ring to ОК");
        w.key(egui::Key::ArrowUp);
        w.idle(2);
        let up = focused(&w);
        assert_ne!(up, cancel, "not round to Отмена");
        assert_ne!(up, ok);
        assert!(up.is_some() && (up == list || list.is_none()), "the list: {up:?} {list:?}");
    }

    /// The arrows' spans of the template's WS_GROUPs that mix kinds (the
    /// original's dialog manager walks a span over every control in it; clone:
    /// 537, 477). Each dialog family: print (347), the quick-analysis pages
    /// (340, 536, 538, 539, 543), the finder (558), the pair dialog (554) and
    /// the program settings (510) (#25).
    #[test]
    fn the_templates_mixed_spans_are_one_span_for_the_arrows() {
        use crate::dialogs::{arrow_spans, GroupKind::{Check, Other, Push, Radio}};
        let has = |w: &Win, title: &str, span: &[crate::dialogs::GroupKind]| {
            let spans = arrow_spans(&w.ctx, title);
            assert!(spans.iter().any(|s| s == span), "{title}: no span {span:?} in {spans:?}");
        };
        // 347 «Печать»: [Предв. просмотр] [OK, Отмена, five check boxes].
        let mut w = Win::new();
        w.app.dispatch(app::Command::Print);
        w.idle(4);
        has(&w, "Печать", &[Push, Push, Check, Check, Check, Check, Check]);
        // The quick-analysis pages.
        let page = |tab: app::AnalysisTab| {
            let mut w = Win::new();
            w.app.dispatch(app::Command::QuickAnalysis);
            w.app.analysis_tab = tab;
            w.idle(4);
            w
        };
        let w = page(app::AnalysisTab::Formula);
        has(&w, "fv_quick_sheet", &[Other, Push, Push, Push, Push, Push, Push, Push]);
        let w = page(app::AnalysisTab::Symmetric);
        has(&w, "fv_quick_sheet", &[Other, Check]);
        let w = page(app::AnalysisTab::PhasePhase);
        has(&w, "fv_quick_sheet", &[Other, Check]);
        has(&w, "fv_quick_sheet", &[Radio, Radio, Other, Other, Other, Other]);
        // 537: «Ic» with the 3Io radios; the two «Отключить» one span.
        let w = page(app::AnalysisTab::Zero);
        has(&w, "fv_quick_sheet", &[Other, Radio, Radio]);
        has(&w, "fv_quick_sheet", &[Check, Check]);
        // A disabled control takes up the span flag meant for it: with the
        // voltages off the sheet's buttons stay their own span (review).
        let mut w = page(app::AnalysisTab::Zero);
        w.app.quick[crate::quick_ui::page_index(app::AnalysisTab::Zero).unwrap()].zero_u_off = true;
        w.idle(4);
        let spans = arrow_spans(&w.ctx, "fv_quick_sheet");
        let with_push: Vec<_> = spans.iter().filter(|s| s.contains(&Push)).collect();
        assert_eq!(with_push, vec![&vec![Push, Push]], "{spans:?}");
        // The disabled «Отключить» keeps its place in the span: the other one
        // stands alone and ↑ on it changes nothing (the original's dialog manager).
        let mut w = page(app::AnalysisTab::Zero);
        let q = crate::quick_ui::page_index(app::AnalysisTab::Zero).unwrap();
        w.app.quick[q].zero_u_off = true;
        w.idle(4);
        let spans = arrow_spans(&w.ctx, "fv_quick_sheet");
        let with_check: Vec<_> = spans.iter().filter(|s| s.contains(&Check)).collect();
        assert_eq!(with_check, vec![&vec![Check]], "{spans:?}");
        let before = w.app.quick[q].zero_i;
        // Ia, Ib, Ic, «Расчетный», then the 3Io «Отключить» (the voltage side is off).
        for _ in 0..4 {
            w.key(egui::Key::Tab);
        }
        w.key(egui::Key::ArrowUp);
        w.key(egui::Key::ArrowDown);
        assert_eq!(w.app.quick[q].zero_i, before, "no arrow reached the 3Io radios");
        // A disabled combo box keeps its place too: with 3Uo «Расчитанный из Uни,
        // Uик» (Ua–Uc off) the voltage radios stay a span of their own, and ↓ on
        // the 3Io radio does not reach them.
        let mut w = page(app::AnalysisTab::Zero);
        w.app.quick[q].zero_u = 2;
        w.app.quick[q].zero_i = 2;
        w.idle(4);
        // (The 3Io combo box, enabled with the measured current, ends that span.)
        has(&w, "fv_quick_sheet", &[Radio, Radio]);
        has(&w, "fv_quick_sheet", &[Radio, Radio, Radio, Other]);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::ArrowDown);
        assert_eq!(w.app.quick[q].zero_u, 2, "no arrow reached the voltage radios");
        // 359: the tree, the name edit and Отмена are one span (OK is off).
        let mut w = page(app::AnalysisTab::Formula);
        w.app.open_formula_dlg();
        w.idle(4);
        has(&w, "fv_add_formula", &[Other, Other, Push, Push]);
        let w = page(app::AnalysisTab::Phase);
        has(&w, "fv_quick_sheet", &[Other, Radio, Radio]);
        let w = page(app::AnalysisTab::Differential);
        // «<<» is disabled on the first arm.
        has(&w, "fv_quick_sheet", &[Other, Radio, Radio, Radio, Radio, Push]);
        // 558 the finder.
        let mut w = Win::new();
        w.app.open_finder();
        w.idle(4);
        has(&w, "Поиск файлов", &[Push, Push, Push, Other, Other, Other, Other]);
        has(&w, "Поиск файлов", &[Push, Other, Check]);
        // 554 the pair dialog.
        let mut w = Win::new();
        let names: Vec<String> = vec!["A".into(), "B".into()];
        w.app.diag.pair_dlg = Some(crate::diagram_ui::PairDialog { diff: true, pairs: Vec::new(), a: Some(names[0].clone()), b: Some(names[1].clone()), sel: None, list_focus: false });
        w.idle(4);
        has(&w, "Задать соответствие между дифференциальным током и током торможения", &[Push, Push, Other, Other, Push, Other]);
        // («Удалить» is disabled with no row selected.)
        // 510 the program's settings: the folder link and «...».
        let mut w = Win::new();
        w.app.show_settings = true;
        w.idle(4);
        // (No folder set: no link, «...» alone; with a folder the link joins it.)
        has(&w, "Установка настроек программы", &[Push, Push]);
        has(&w, "Установка настроек программы", &[Push]);
    }

    /// A bar is no dialog (clone: Tab goes nowhere in the original, a push
    /// button outside a dialog takes no arrow): a frame control holding the
    /// focus keeps Tab and the arrows from walking egui's focus on to another
    /// widget, which Space would then press (#145, review).
    #[test]
    fn tab_and_arrows_do_not_walk_the_frames_controls() {
        use app::ChanKey;
        use egui::PointerButton::Primary;
        let setup = || {
            let mut w = Win::new();
            w.app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
            w.app.copy_channels_for_merge();
            w.app.paste_channels_for_merge();
            w.idle(3);
            w
        };
        let status = |w: &Win| w.app.merge.as_ref().map(|m| m.status.clone());
        // The method combo, then Tab, Space: nothing is pressed; Tab twice too.
        for tabs in [1, 2] {
            let mut w = setup();
            let combo = w.app.focus_other[0];
            w.press(Primary, combo.center(), combo.center());
            w.key(egui::Key::Escape);
            for _ in 0..tabs {
                w.key(egui::Key::Tab);
            }
            w.key(egui::Key::Space);
            assert_eq!(status(&w), Some(String::new()), "{tabs} Tab: «Синхронизация» not pressed, the bar stays");
        }
        // «Синхронизация» focused: →, ↓ and Tab move nothing, Space does not merge.
        for key in [egui::Key::ArrowRight, egui::Key::ArrowDown, egui::Key::Tab] {
            let mut w = setup();
            let sync = w.app.focus_other[1];
            w.press(Primary, sync.center(), sync.center() + Vec2::new(0.0, 200.0));
            let focus = w.ctx.memory(|m| m.focused());
            assert!(focus.is_some());
            w.key(key);
            assert_eq!(w.ctx.memory(|m| m.focused()), focus, "{key:?}");
        }
        let mut w = setup();
        let sync = w.app.focus_other[1];
        w.press(Primary, sync.center(), sync.center() + Vec2::new(0.0, 200.0));
        w.key(egui::Key::ArrowRight);
        w.key(egui::Key::Space);
        assert!(w.app.merge.is_some(), "→ then Space does not reach «Объединить»");
        // The tab list's ▾: Tab leaves the focus where it is.
        let mut w = Win::new();
        let strip = w.app.mdi_strips[0];
        let list = Pos2::new(strip.right() - 10.0, strip.center().y);
        w.press(Primary, list, list);
        w.key(egui::Key::Escape);
        let focus = w.ctx.memory(|m| m.focused());
        assert!(focus.is_some());
        w.key(egui::Key::Tab);
        assert_eq!(w.ctx.memory(|m| m.focused()), focus, "Tab on ▾");
    }

    /// The ▾ list is an MFC popup menu: opened by Space, the arrows walk it
    /// and Enter picks the row, which comes forward (#145, review).
    #[test]
    fn the_tab_lists_popup_is_walked_by_the_arrows() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.dispatch(app::Command::Demo);
        w.idle(3);
        assert_eq!(w.app.active, Some(1));
        let strip = w.app.mdi_strips[0];
        let list = Pos2::new(strip.right() - 10.0, strip.center().y);
        w.press(Primary, list, list);
        w.key(egui::Key::Escape);
        w.key(egui::Key::Space);
        assert!(w.app.doc_list_open);
        w.key(egui::Key::ArrowDown);
        w.key(egui::Key::Enter);
        w.idle(2);
        assert!(!w.app.doc_list_open, "Enter picked and closed it");
        assert_eq!(w.app.active, Some(0), "the first row (by title) is forward");
    }

    /// Insert's copy / paste kept for a text edit are the edit's own: the
    /// frame's channel copy and paste ignore them (#26, review).
    #[test]
    fn inserts_events_kept_for_an_edit_are_not_the_frames() {
        let mut w = Win::new();
        w.app.active_doc_mut().unwrap().selection.insert(app::ChanKey::Analog(0));
        w.ctx.data_mut(|d| d.insert_temp(egui::Id::new(winkeys::KEPT_FOR_EDIT), (1u8, 0u8)));
        w.run(vec![egui::Event::Copy]);
        assert!(!w.app.clip.available(), "Ctrl+Insert in an edit copies no channel");
        w.run(vec![egui::Event::Copy]);
        assert!(w.app.clip.available(), "a plain copy still does");
    }

    /// A slid-out auto-hidden pane is a child of the frame, which a modal
    /// dialog disables, and the dialog lies above it: the pane takes no
    /// input under the dialog (its caption «×», its pin, the divider) and
    /// does not cover it (final review, Important 1; #113).
    #[test]
    fn a_slid_out_pane_takes_no_input_while_a_dialog_is_up() {
        use app::{CopyFocus, Pane};
        use docking::{Edge, Spot, Unit};
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge: Edge::Right, outer: true };
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(30);
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).map(|(_, r)| *r).expect("the slid-out diagram");
        // A press in its diagram: it holds the focus, so it stays out.
        w.press(Primary, canvas.center(), canvas.center());
        assert_eq!(w.app.copy_focus, CopyFocus::Diagram(Pane::Vector));
        let away = w.app.view_rects.expect("the view").0.left_top() + Vec2::new(40.0, 40.0);
        w.run(vec![egui::Event::PointerMoved(away)]);
        w.idle(5);
        w.app.color_dialog = Some(egui::Color32::RED);
        w.idle(3);
        assert!(w.app.modal_up);
        assert!(w.app.docks[Unit::Vector].out, "the pane is still out under the dialog");
        let area = w.app.dock_areas[Unit::Vector as usize].expect("slid out");
        let close = Pos2::new(area.right() - dpi::s(11.0), area.top() + dpi::s(10.0));
        let dialog = w.ctx.memory(|m| m.area_rect(egui::Id::new("Цвета"))).expect("the colour dialog");
        // Wherever the dialog lies, it is the window on top there.
        for at in [dialog.center(), Pos2::new(dialog.right() - 3.0, dialog.top() + 3.0)] {
            let layer = w.ctx.layer_id_at(at).expect("a layer");
            assert!(!crate::views::is_slid_pane_layer(layer), "{at:?}: the dialog is above the slid-out pane");
        }
        w.press(Primary, close, close);
        assert!(w.app.panes.visible(Pane::Vector), "a press on the pane's caption button does nothing");
        let top = w.ctx.memory(|m| m.areas().top_layer_id(egui::Order::Middle));
        assert_eq!(top.map(|l| l.id), Some(egui::Id::new("Цвета")), "the dialog stays on top");
        // Its divider takes no drag either.
        // A point of it clear of the dialog (the dialog lies over its middle).
        let strip = w.app.dock_dividers[Unit::Vector as usize].expect("its divider");
        let div = [strip.top() + 30.0, strip.bottom() - 30.0].into_iter().map(|y| Pos2::new(strip.center().x, y)).find(|p| !dialog.contains(*p)).expect("a divider point clear of the dialog");
        let extent = w.app.docks[Unit::Vector].slide_extent;
        let ev = |at, pressed| egui::Event::PointerButton { pos: at, button: Primary, pressed, modifiers: Default::default() };
        w.run(vec![egui::Event::PointerMoved(div)]);
        w.run(vec![ev(div, true)]);
        w.run(vec![egui::Event::PointerMoved(div - Vec2::new(60.0, 0.0))]);
        w.run(vec![egui::Event::PointerMoved(div - Vec2::new(120.0, 0.0))]);
        w.idle(2);
        w.run(vec![ev(div - Vec2::new(120.0, 0.0), false)]);
        w.idle(3);
        assert_eq!(w.app.docks[Unit::Vector].slide_extent, extent, "a drag of the divider does nothing");
        // The dialog gone, the same press closes the pane.
        w.app.color_dialog = None;
        w.idle(3);
        w.press(Primary, close, close);
        w.idle(30);
        assert!(!w.app.panes.visible(Pane::Vector), "the dialog gone, the caption button works");
    }

    /// MFC's frame disables its floating windows while a modal dialog is up
    /// (clone: «Каналы» open, the floating pane's mini-frame reads as
    /// disabled): a press, a double-click or a drag on the floating pane's
    /// caption, or a press on its tab, does nothing until the dialog is gone
    /// (#113).
    #[test]
    fn a_floating_pane_takes_no_input_while_a_dialog_is_up() {
        use app::{CopyFocus, Pane};
        use docking::{Place, Unit};
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).expect("the vector pane").1;
        let at = Pos2::new(canvas.left() + 60.0, canvas.top() - 10.0);
        let ev = |at, pressed| egui::Event::PointerButton { pos: at, button: Primary, modifiers: Default::default(), pressed };
        w.run(vec![egui::Event::PointerMoved(at)]);
        w.run(vec![ev(at, true), ev(at, false)]);
        w.run(vec![ev(at, true), ev(at, false)]);
        w.idle(3);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating);
        let id = egui::Id::new(("fv_floating_pane", Unit::Vector));
        let win = |w: &Win| w.ctx.memory(|m| m.area_rect(id)).expect("the floating window");
        let (caption, tab) = (Pos2::new(win(&w).left() + 60.0, win(&w).top() + 10.0), w.app.pane_tab_rects.iter().find(|(p, _)| *p == Pane::Circle).map(|(_, r)| r.center()).expect("the circle tab"));
        let shown = win(&w);
        w.app.copy_focus = CopyFocus::View;
        w.app.color_dialog = Some(egui::Color32::RED);
        w.idle(3);
        assert!(w.app.modal_up);
        let dialog = w.ctx.memory(|m| m.area_rect(egui::Id::new("Цвета"))).expect("the colour dialog");
        assert!(!dialog.contains(caption) && !dialog.contains(tab), "the dialog stays clear of the window's caption and tab");
        let (rect, front) = (win(&w), w.app.panes.front(w.app.docks.members(Unit::Vector)));
        w.press(Primary, caption, caption);
        assert_eq!(w.app.copy_focus, CopyFocus::View, "the caption's press");
        w.press(Primary, tab, tab);
        assert_eq!(w.app.panes.front(w.app.docks.members(Unit::Vector)), front, "the tab's press");
        w.run(vec![egui::Event::PointerMoved(caption)]);
        w.run(vec![ev(caption, true), ev(caption, false)]);
        w.run(vec![ev(caption, true), ev(caption, false)]);
        w.idle(3);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating, "the double-click on the caption");
        w.press(Primary, caption, caption + Vec2::new(90.0, 70.0));
        assert_eq!(win(&w), rect, "a drag of the caption");
        // A press on it does not raise it over the dialog.
        let top = w.ctx.memory(|m| m.areas().top_layer_id(egui::Order::Middle));
        assert_eq!(top.map(|l| l.id), Some(egui::Id::new("Цвета")), "the dialog stays above the floating window");
        // The dialog gone, the same press is the pane's again.
        w.app.color_dialog = None;
        w.idle(3);
        w.press(Primary, caption, caption);
        assert_eq!(w.app.copy_focus, CopyFocus::Pane(Pane::Vector), "the dialog gone");
        assert_eq!(win(&w).size(), shown.size(), "the window keeps its size");
    }

    /// A channel drag let go over a floating pane's window that lies over the
    /// view drops nowhere unless it is on its diagram (clone: the dragged
    /// lane released on the mini-frame's caption over another lane of the
    /// same unit: nothing grouped; released beside the window the same drag
    /// groups them): the window under the pointer is the drop target, not
    /// the lane under it (#113).
    #[test]
    fn a_channel_dropped_on_a_floating_window_over_the_view_drops_nowhere() {
        use docking::{Place, Unit};
        use egui::PointerButton::Primary;
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: Primary, modifiers: Default::default(), pressed };
        // The lane under `to` is the first one after lane 0 of its unit.
        let setup = |floating: bool| {
            let mut w = Win::new();
            let (rows, same) = {
                let d = w.app.active_doc().unwrap();
                let rows = lanes::lane_layout(d, w.app.y_analog);
                let unit = |li: usize| match rows[li].key {
                    app::ChanKey::Analog(i) => Some(d.osc.analog[i].unit.clone()),
                    _ => None,
                };
                let same = (1..rows.len()).find(|&li| unit(li).is_some() && unit(li) == unit(0)).expect("a lane with the same unit");
                (rows, same)
            };
            let client = w.app.view_rects.expect("the view").1;
            let from = Pos2::new(client.left() + 40.0, client.top() + rows[0].h / 2.0);
            let to = Pos2::new(client.left() + 320.0, client.top() + rows[..same].iter().map(|r| r.h).sum::<f32>() + rows[same].h / 2.0);
            if floating {
                // The window's caption (its top 20 px) over `to`.
                let d = &mut w.app.docks.0[Unit::Vector as usize];
                d.float_rect = Some(Rect::from_min_size(Pos2::new(to.x - 90.0, to.y - 8.0), Vec2::new(260.0, 220.0)));
                d.place = Place::Floating;
                w.idle(3);
            }
            let (app::ChanKey::Analog(c0), app::ChanKey::Analog(cs)) = (rows[0].key, rows[same].key) else { unreachable!() };
            (w, from, to, c0, cs)
        };
        let drag = |w: &mut Win, a: Pos2, b: Pos2| {
            w.run(vec![egui::Event::PointerMoved(a), ev(a, true)]);
            let steps = (((b - a).length()) / 6.0).ceil().max(1.0) as usize;
            for k in 1..=steps {
                w.run(vec![egui::Event::PointerMoved(a + (b - a) * (k as f32 / steps as f32))]);
            }
            w.run(vec![ev(b, false)]);
            w.idle(3);
        };
        // Beside any window: the drop groups the two lanes (the geometry is right).
        let (mut w, from, to, c0, cs) = setup(false);
        drag(&mut w, from, to);
        let d = w.app.active_doc().unwrap();
        assert!(d.chans[c0].group.is_some() && d.chans[c0].group == d.chans[cs].group, "dropped on the lane");
        // On the floating window's caption lying over that lane: nothing.
        let (mut w, from, to, c0, cs) = setup(true);
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating);
        drag(&mut w, from, to);
        let d = w.app.active_doc().unwrap();
        assert!(d.chans[c0].group.is_none() && d.chans[cs].group.is_none(), "dropped on the window over the lane");
        assert!(!egui::DragAndDrop::has_any_payload(&w.ctx), "the drag is over");
    }

    /// A channel drag shows the move-drop cursor over the oscillogram view and
    /// the frequency / vector diagrams, "copy" with Ctrl inside the view's
    /// 11 px auto-scroll band, and "no-drop" everywhere else — the ribbon, the
    /// bars, a pane's caption (clone, #73).
    #[test]
    fn a_channel_drag_shows_the_drop_cursor_over_its_targets_only() {
        use app::Pane;
        use egui::CursorIcon::{Copy, NoDrop};
        let mut w = Win::new();
        let serial = w.app.active_doc().unwrap().serial;
        let client = w.app.view_rects.expect("the view").1;
        let canvas = |w: &Win, p: Pane| w.app.pane_canvas.iter().find(|(q, _)| *q == p).unwrap_or_else(|| panic!("{p:?}")).1;
        let (vector, freq) = (canvas(&w, Pane::Vector), canvas(&w, Pane::Frequency));
        egui::DragAndDrop::set_payload(&w.ctx, lanes::ChanDrag { chans: vec![0], computed: vec![], doc: serial });
        let at = |w: &mut Win, p: Pos2| {
            w.run(vec![egui::Event::PointerMoved(p)]);
            w.run(vec![])
        };
        let drop = lanes::MOVE_DROP_CURSOR;
        let mid = client.center();
        assert_eq!(at(&mut w, mid), drop, "over the view");
        assert_eq!(at(&mut w, vector.center()), drop, "over the vector diagram");
        assert_eq!(at(&mut w, freq.center()), drop, "over the frequency diagram");
        assert_eq!(at(&mut w, Pos2::new(60.0, 12.0)), NoDrop, "over the window's top rows");
        assert_eq!(at(&mut w, Pos2::new(vector.center().x, vector.top() - 12.0)), NoDrop, "over a pane's caption");
        // The auto-scroll band: Ctrl turns the cursor to "copy", only there.
        let band = Pos2::new(mid.x, client.bottom() - 4.0);
        assert_eq!(at(&mut w, band), drop, "in the band without Ctrl");
        w.held = egui::Modifiers::CTRL;
        assert_eq!(at(&mut w, mid), drop, "Ctrl beside the band changes nothing");
        assert_eq!(at(&mut w, vector.center()), drop, "Ctrl over a diagram changes nothing");
        assert_eq!(at(&mut w, Pos2::new(60.0, 12.0)), NoDrop, "Ctrl elsewhere changes nothing");
        assert_eq!(at(&mut w, band), Copy, "Ctrl in the band");
        w.held = egui::Modifiers::NONE;
        // Ctrl+Shift in the band: MFC answers "link", the drag allows none.
        w.held = egui::Modifiers { ctrl: true, shift: true, command: true, ..Default::default() };
        assert_eq!(at(&mut w, band), NoDrop, "Ctrl+Shift in the band");
        w.held = egui::Modifiers::NONE;
        egui::DragAndDrop::clear_payload(&w.ctx);
        assert_ne!(at(&mut w, Pos2::new(60.0, 12.0)), NoDrop, "no drag, no cursor");
    }

    /// The circle's drop target is its window (property grid and diagram), not
    /// the pane's toolbar around it; the differential pane has none; a
    /// floating window over the view is no part of the view (clone, #73).
    #[test]
    fn the_circle_toolbar_and_the_differential_pane_are_no_drop_targets() {
        use app::{Command, Pane};
        use docking::{Place, Unit};
        let mut w = Win::new();
        let serial = w.app.active_doc().unwrap().serial;
        // The vector and circle panes share a dock: the toolbar column right of the vector canvas is the circle pane's too.
        let toolbar = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).map(|(_, r)| Pos2::new(r.right() + 8.0, r.center().y)).expect("the vector diagram");
        w.app.dispatch(Command::ActivatePane(Pane::Circle));
        w.idle(3);
        let circle = w.app.drop_targets.iter().find(|(p, _)| *p == Pane::Circle).expect("the circle window").1;
        let payload = || lanes::ChanDrag { chans: vec![0], computed: vec![], doc: serial };
        let at = |w: &mut Win, p: Pos2| {
            egui::DragAndDrop::set_payload(&w.ctx, payload());
            w.run(vec![egui::Event::PointerMoved(p)]);
            w.run(vec![])
        };
        let (drop, no) = (lanes::MOVE_DROP_CURSOR, egui::CursorIcon::NoDrop);
        assert_eq!(at(&mut w, circle.center()), drop, "over the circle window");
        assert!(!circle.contains(toolbar), "the toolbar is outside the circle window");
        assert_eq!(at(&mut w, toolbar), no, "over the circle pane's toolbar");
        // Every point that shows the drop cursor lies in the view window (its scroll bars too) or a drop target: none in the differential pane.
        w.app.dispatch(Command::ActivatePane(Pane::Differential));
        w.idle(3);
        let view = w.app.view_rects.expect("the view").1;
        let mut seen = 0;
        for y in (20..840).step_by(41) {
            for x in (10..1310).step_by(43) {
                let p = Pos2::new(x as f32, y as f32);
                if at(&mut w, p) == drop {
                    seen += 1;
                    assert!(view.expand2(Vec2::new(20.0, 20.0)).contains(p) || w.app.drop_targets.iter().any(|(_, r)| r.contains(p)), "{p:?}");
                }
            }
        }
        assert!(seen > 0);
        egui::DragAndDrop::clear_payload(&w.ctx);
        // A floating pane's window lying over the view: no-drop on its caption.
        w.app.dispatch(Command::ActivatePane(Pane::Vector));
        let to = Pos2::new(view.left() + 200.0, view.center().y);
        let d = &mut w.app.docks.0[Unit::Vector as usize];
        d.float_rect = Some(Rect::from_min_size(Pos2::new(to.x - 90.0, to.y - 8.0), Vec2::new(260.0, 220.0)));
        d.place = Place::Floating;
        w.idle(3);
        assert_eq!(at(&mut w, to), no, "over a floating window's caption lying over the view");
        assert_eq!(at(&mut w, Pos2::new(view.left() + 60.0, view.top() + 30.0)), drop, "beside it, still the view");
    }

    /// Channels dragged out of one record and dropped on another record's
    /// view or diagram change nothing and show no message (the maintainer's
    /// decision, #74: the drag carries only channel numbers; the original
    /// applied them to the active record), and the cursor there is "no-drop".
    #[test]
    fn a_channel_dropped_on_another_records_view_does_nothing() {
        use egui::PointerButton::Primary;
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: Primary, modifiers: Default::default(), pressed };
        let mut w = Win::new();
        w.app.dispatch(app::Command::Demo);
        w.idle(3);
        w.app.mdi_vertical = true;
        let at = w.app.mdi_area.center();
        w.app.drop_doc_tab(1, at);
        w.idle(3);
        assert_eq!(w.app.group_count(), 2, "two tab groups");
        let (a, b) = (w.app.docs.iter().position(|d| d.mdi_group == 0).unwrap(), w.app.docs.iter().position(|d| d.mdi_group == 1).unwrap());
        let (sa, sb) = (w.app.docs[a].serial, w.app.docs[b].serial);
        w.app.dispatch(app::Command::ActivateDoc(b));
        w.idle(3);
        let client = w.app.view_rects.expect("the view").1;
        let (to, c0, cs) = {
            let rows = lanes::lane_layout(w.app.active_doc().unwrap(), w.app.y_analog);
            let d = w.app.active_doc().unwrap();
            let unit = |li: usize| match rows[li].key {
                app::ChanKey::Analog(i) => Some(d.osc.analog[i].unit.clone()),
                _ => None,
            };
            let same = (1..rows.len()).find(|&li| unit(li).is_some() && unit(li) == unit(0)).expect("a lane with the same unit");
            let (app::ChanKey::Analog(c0), app::ChanKey::Analog(cs)) = (rows[0].key, rows[same].key) else { unreachable!() };
            (Pos2::new(client.left() + 320.0, client.top() + rows[..same].iter().map(|r| r.h).sum::<f32>() + rows[same].h / 2.0), c0, cs)
        };
        let drop_from = |w: &mut Win, from_doc: u64| {
            egui::DragAndDrop::set_payload(&w.ctx, lanes::ChanDrag { chans: vec![c0], computed: vec![], doc: from_doc });
            w.run(vec![egui::Event::PointerMoved(to)]);
            let cursor = w.run(vec![]);
            w.run(vec![ev(to, true)]);
            w.run(vec![ev(to, false)]);
            w.idle(3);
            cursor
        };
        let grouped = |w: &Win| {
            let d = &w.app.docs[b];
            d.chans[c0].group.is_some() && d.chans[c0].group == d.chans[cs].group
        };
        // From a record onto its own view: the lanes group (the geometry is right).
        assert_eq!(drop_from(&mut w, sb), lanes::MOVE_DROP_CURSOR);
        assert!(grouped(&w), "the control: a drop on the record's own view");
        w.app.docs[b].chans[c0].group = None;
        w.app.docs[b].chans[cs].group = None;
        w.app.docs[b].set.modified = false;
        w.idle(2);
        // From the other record: nothing.
        assert_eq!(drop_from(&mut w, sa), egui::CursorIcon::NoDrop, "the cursor over another record's view");
        assert!(!grouped(&w), "no combine on the other record");
        assert!(!w.app.docs[b].set.modified, "no change");
        assert!(w.app.dlg.alert().is_none(), "no message");
        assert!(!egui::DragAndDrop::has_any_payload(&w.ctx), "the drag is over");
        // Nor on a diagram that shows the other record.
        let freq = w.app.pane_canvas.iter().find(|(p, _)| *p == app::Pane::Frequency).unwrap().1;
        w.app.docs[b].chans[c0].on_freq = false;
        egui::DragAndDrop::set_payload(&w.ctx, lanes::ChanDrag { chans: vec![c0], computed: vec![], doc: sa });
        w.run(vec![egui::Event::PointerMoved(freq.center())]);
        assert_eq!(w.run(vec![]), egui::CursorIcon::NoDrop, "the cursor over a diagram of the other record");
        w.run(vec![ev(freq.center(), true)]);
        w.run(vec![ev(freq.center(), false)]);
        w.idle(3);
        assert!(!w.app.docs[b].chans[c0].on_freq && !w.app.docs[b].set.modified, "nothing on the diagram");
    }

    /// The lane's rename box and the level editor are windows over the view
    /// (children of it): a press in one is a press in the view, which holds
    /// the focus then — not in a popup that leaves it where it was (#83).
    #[test]
    fn a_press_in_an_editor_over_the_view_gives_the_view_the_focus() {
        use app::{CopyFocus, Pane};
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        let canvas = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).expect("the vector diagram").1;
        let mut seen = 0;
        for edit in ["fv_rename", "fv_level_edit"] {
            w.press(Primary, canvas.center(), canvas.center());
            assert_eq!(w.app.copy_focus, CopyFocus::Diagram(Pane::Vector));
            if edit == "fv_rename" {
                w.app.rename_edit = Some((0, "Ia".into()));
            } else {
                w.app.rename_edit = None;
                w.app.level_edit = Some((0, "1.00".into()));
            }
            w.idle(3);
            let Some(r) = w.ctx.memory(|m| m.area_rect(egui::Id::new(edit).with(0usize))) else { continue };
            seen += 1;
            w.press(Primary, r.center(), r.center());
            assert_eq!(w.app.copy_focus, CopyFocus::View, "{edit}: the press in the editor");
            w.app.rename_edit = None;
            w.app.level_edit = None;
        }
        assert!(seen > 0, "an editor was up");
        // A menu or a combo box's list over the view takes no focus: the
        // window that holds it keeps it.
        let (full, _) = w.app.view_rects.expect("the view");
        let at = full.min + Vec2::new(180.0, 200.0);
        let cover = Rect::from_center_size(at, Vec2::splat(60.0));
        for before in [CopyFocus::Other, CopyFocus::Diagram(Pane::Vector)] {
            w.app.copy_focus = before;
            for pressed in [None, Some(true), Some(false), None] {
                let events = match pressed {
                    None => vec![egui::Event::PointerMoved(at)],
                    Some(pressed) => vec![egui::Event::PointerButton { pos: at, button: Primary, pressed, modifiers: Default::default() }],
                };
                let t = w.ctx.input(|i| i.time) + 0.05;
                let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1320.0, 860.0))), time: Some(t), events, ..Default::default() };
                let app = &mut w.app;
                let _ = w.ctx.run(raw, |ctx| {
                    egui::Area::new(egui::Id::new("test_popup")).order(egui::Order::Foreground).fixed_pos(cover.min).show(ctx, |ui| {
                        ui.allocate_exact_size(cover.size(), egui::Sense::click());
                    });
                    app.frame_ui(ctx);
                });
            }
            assert_eq!(w.app.copy_focus, before, "a popup over the view took the press");
        }
    }

    /// The merge bar's button keeps the focus when the merge hides the bar
    /// (clone: «Объединить» pressed, the bar gone: GetFocus still names the
    /// hidden button): the view's keys stay dead and copy takes the view's
    /// picture and the document (#65).
    #[test]
    fn the_focus_stays_on_the_merge_button_when_the_bar_hides() {
        use app::{ChanKey, CopyFocus};
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        w.app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        w.app.copy_channels_for_merge();
        w.app.paste_channels_for_merge();
        w.idle(3);
        let join = w.app.focus_other[2];
        w.press(Primary, join.center(), join.center());
        w.idle(3);
        assert!(w.app.merge.is_none(), "the bar is gone");
        assert_eq!(w.app.copy_focus, CopyFocus::Other, "the hidden button holds the focus");
        let before = w.view_state();
        w.key(egui::Key::ArrowRight);
        assert_eq!(w.view_state(), before, "the view's keys are dead");
        w.app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        w.app.dispatch(app::Command::Copy);
        assert!(w.app.clip.picture().is_some() && w.app.clip.get().is_some());
    }

    /// A channel let go on a floating window that lies over a docked diagram
    /// goes to the window's diagram (or nowhere, on its caption), never to
    /// the docked diagram beneath it (#113; the window under the pointer
    /// takes the drop).
    #[test]
    fn a_channel_dropped_on_a_floating_window_over_a_docked_diagram_misses_the_docked_one() {
        use app::{ChanKey, Pane};
        use docking::{Place, Unit};
        use egui::PointerButton::Primary;
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: Primary, modifiers: Default::default(), pressed };
        let run = |onto_diagram: bool| {
            let mut w = Win::new();
            let freq = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Frequency).expect("the frequency diagram").1;
            let d = &mut w.app.docks.0[Unit::Vector as usize];
            d.float_rect = Some(Rect::from_min_size(freq.min + Vec2::new(40.0, 4.0), Vec2::new(300.0, 200.0)));
            d.place = Place::Floating;
            w.idle(3);
            // The docked layout changed as the group left: place the window over where Частотная is now.
            let freq = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Frequency).expect("the frequency diagram").1;
            w.app.docks.0[Unit::Vector as usize].float_rect = Some(Rect::from_min_size(freq.min + Vec2::new(40.0, 4.0), Vec2::new(300.0, 200.0)));
            w.idle(3);
            {
                let d = w.app.active_doc_mut().unwrap();
                d.chans[0].on_freq = false;
                d.chans[0].on_vector = false;
            }
            let freq = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Frequency).expect("the frequency diagram").1;
            let win = w.ctx.memory(|m| m.area_rect(egui::Id::new(("fv_floating_pane", Unit::Vector)))).expect("the floating window");
            let to = if onto_diagram {
                let v = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Vector).expect("the floating vector diagram").1;
                Pos2::new(freq.left() + 60.0, v.center().y)
            } else {
                Pos2::new(freq.left() + 30.0, win.top() + 10.0)
            };
            assert!(freq.contains(to), "the window lies over the frequency diagram");
            let client = w.app.view_rects.expect("the view").1;
            let rows = lanes::lane_layout(w.app.active_doc().unwrap(), w.app.y_analog);
            let a = Pos2::new(client.left() + 40.0, client.top() + rows[0].h / 2.0);
            w.run(vec![egui::Event::PointerMoved(a), ev(a, true)]);
            let steps = ((to - a).length() / 6.0).ceil().max(1.0) as usize;
            for k in 1..=steps {
                w.run(vec![egui::Event::PointerMoved(a + (to - a) * (k as f32 / steps as f32))]);
            }
            w.run(vec![ev(to, false)]);
            w.idle(3);
            let d = w.app.active_doc().unwrap();
            assert!(matches!(rows[0].key, ChanKey::Analog(0)));
            (d.chans[0].on_freq, d.chans[0].on_vector)
        };
        assert_eq!(run(true), (false, true), "on the floating diagram: it takes the channel, not the frequency diagram");
        assert_eq!(run(false), (false, false), "on the floating caption: nobody does");
    }

    /// The same for a slid-out pane over a docked group on the same edge: a
    /// channel let go on the slid-out diagram goes to it, not to the docked
    /// diagram under it (#113).
    #[test]
    fn a_channel_dropped_on_a_slid_out_pane_over_a_docked_diagram_misses_the_docked_one() {
        use app::Pane;
        use docking::{Edge, Spot, Unit};
        use egui::PointerButton::Primary;
        let ev = |pos, pressed| egui::Event::PointerButton { pos, button: Primary, modifiers: Default::default(), pressed };
        let mut w = Win::new();
        w.app.docks.0[Unit::Frequency as usize].spot = Spot::Edge { edge: Edge::Right, outer: true };
        w.app.docks.0[Unit::Vector as usize].spot = Spot::Edge { edge: Edge::Right, outer: true };
        w.app.docks.0[Unit::Vector as usize].pin();
        w.idle(3);
        w.run(vec![egui::Event::PointerMoved(w.app.strip_hits[Unit::Vector as usize].buttons[0].center())]);
        w.idle(30);
        let [c0, c1, ..] = w.app.slid_ranges[Unit::Vector as usize].expect("slid out");
        let slid = w.app.pane_canvas[c0..c1].iter().find(|(p, _)| *p == Pane::Vector).expect("the slid-out vector diagram").1;
        let freq = w.app.pane_canvas.iter().find(|(p, _)| *p == Pane::Frequency).expect("the docked frequency diagram").1;
        let to = Pos2::new(freq.center().x, slid.center().y);
        {
            let d = w.app.active_doc_mut().unwrap();
            d.chans[0].on_freq = false;
            d.chans[0].on_vector = false;
        }
        assert!(freq.contains(to) && slid.contains(to), "the slid-out pane lies over the docked diagram: {freq:?} {slid:?}");
        {
            let d = w.app.active_doc_mut().unwrap();
            d.chans[0].on_freq = false;
            d.chans[0].on_vector = false;
        }
        // The pane holds the focus (so it stays out) and a drag from the lanes is in flight.
        w.press(Primary, to, to);
        egui::DragAndDrop::set_payload(&w.ctx, lanes::ChanDrag { chans: vec![0], computed: vec![], doc: w.app.active_doc().unwrap().serial });
        w.run(vec![egui::Event::PointerMoved(to)]);
        w.run(vec![ev(to, true)]);
        w.run(vec![ev(to, false)]);
        w.idle(3);
        let d = w.app.active_doc().unwrap();
        assert_eq!((d.chans[0].on_freq, d.chans[0].on_vector), (false, true), "the slid-out diagram takes it");
    }

    /// Every way the active document changes — the tab list's pick (the ▾
    /// button leaves `Other` until then), opening, closing — focuses the
    /// activated document's view (clone: MDI activation); a system file dialog
    /// closed unanswered does not (#65).
    #[test]
    fn a_new_active_document_gives_the_view_the_focus() {
        use app::{Command, CopyFocus, Pane};
        let mut w = Win::new();
        w.app.dispatch(Command::Demo);
        w.idle(3);
        assert_eq!(w.app.docs.len(), 2);
        for before in [CopyFocus::Other, CopyFocus::Diagram(Pane::Vector), CopyFocus::Pane(Pane::Vector), CopyFocus::Grid(Pane::Circle)] {
            // The tab list's pick: the other document.
            let other = 1 - w.app.active.unwrap();
            w.app.copy_focus = before;
            w.app.dispatch(Command::ActivateDoc(other));
            w.idle(2);
            assert_eq!(w.app.copy_focus, CopyFocus::View, "{before:?}: activated");
            // Closing the active one.
            w.app.copy_focus = before;
            w.app.dispatch(Command::CloseDoc(w.app.active.unwrap()));
            w.idle(2);
            assert_eq!(w.app.copy_focus, CopyFocus::View, "{before:?}: closed");
            // Opening one.
            w.app.copy_focus = before;
            w.app.dispatch(Command::Demo);
            w.idle(2);
            assert_eq!(w.app.copy_focus, CopyFocus::View, "{before:?}: opened");
            // Nothing else does it. (A system file dialog closed
            // unanswered leaves the focus where it was, clone; it cannot be
            // driven headless, so the rule is not tested here.)
            w.app.copy_focus = before;
            w.idle(3);
            assert_eq!(w.app.copy_focus, before, "{before:?}: left alone");
        }
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

    /// The delay egui reports for the next repaint after idle frames (no input, time advancing):
    /// `Duration::MAX` when nothing asks for one.
    fn idle_delays(app: &mut App, ctx: &egui::Context, size: Vec2, n: usize) -> Vec<std::time::Duration> {
        let mut out = Vec::new();
        for _ in 0..n {
            let t = ctx.input(|i| i.time) + 0.075;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(t), ..Default::default() };
            let o = ctx.run(raw, |ctx| {
                app.frame_ui(ctx);
                app.splash(ctx);
            });
            out.push(o.viewport_output.get(&egui::ViewportId::ROOT).map_or(std::time::Duration::MAX, |v| v.repaint_delay));
        }
        out
    }

    /// #33: the splash asks for one repaint, at its end (its timer), not one every 100 ms; and the
    /// window with nothing happening asks for none, the pointer resting anywhere once it has settled
    /// (a tooltip's wait, a hover's fade end).
    #[test]
    fn an_idle_window_asks_for_no_repaint() {
        let size = Vec2::new(1320.0, 860.0);
        let mut app = App::new();
        app.dispatch(app::Command::Demo);
        app.splash_until = Some(2.0);
        let ctx = egui::Context::default();
        let d = idle_delays(&mut app, &ctx, size, 8);
        let t = ctx.input(|i| i.time);
        assert!(t < 2.0, "still in the splash: {t}");
        assert!(d[4..].iter().all(|x| x.as_secs_f64() > 1.0 && x.as_secs_f64() < 2.0 - t + 1.0), "the splash's single wake at its end: {d:?}");
        idle_delays(&mut app, &ctx, size, 20);
        assert!(app.splash_until.is_none());
        let rest = idle_delays(&mut app, &ctx, size, 6);
        assert!(rest.iter().all(|x| *x == std::time::Duration::MAX), "no pointer: {rest:?}");
        for own_caption in [false, true] {
            app.own_caption = own_caption;
            for y in [10.0, 45.0, 90.0, 200.0, 420.0, 700.0, 850.0] {
                for x in [10.0, 150.0, 400.0, 700.0, 1000.0, 1300.0] {
                    let t = ctx.input(|i| i.time) + 0.075;
                    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(t), events: vec![egui::Event::PointerMoved(Pos2::new(x, y))], ..Default::default() };
                    let _ = ctx.run(raw, |ctx| app.frame_ui(ctx));
                    let d = idle_delays(&mut app, &ctx, size, 16);
                    assert!(d[10..].iter().all(|x| *x == std::time::Duration::MAX), "the pointer resting at {x},{y}: {d:?}");
                }
            }
        }
    }

    /// #34: the frameless window tells Windows where its maximise button is (`WM_NCHITTEST` answers
    /// `HTMAXBUTTON` there, the snap-layout flyout): the middle of the three caption buttons at the
    /// window's right edge, in device pixels; none while a modal dialog is up, nor under a floating
    /// pane lying over it (`the_maximise_button_is_not_published_under_a_modal_or_a_pane`).
    #[test]
    fn the_maximise_button_is_published_for_the_frame() {
        let size = Vec2::new(1280.0, 800.0);
        let mut app = demo(&[]);
        app.own_caption = true;
        let ctx = egui::Context::default();
        idle_delays(&mut app, &ctx, size, 3);
        let r = winframe::max_button().expect("the caption's maximise button is published");
        let w = dpi::s(46.0).round() as i32;
        assert_eq!(r[2] - r[0], w, "one caption button wide: {r:?}");
        assert_eq!(r[2], size.x as i32 - w, "the middle one of three at the right edge: {r:?}");
        assert!(r[1] == 0 && r[3] > 20 && r[3] < 50, "on the caption row: {r:?}");
        assert!(winframe::on_max_button(r[0] + 3, 5) && !winframe::on_max_button(r[2] + 3, 5) && !winframe::on_max_button(r[0] - 3, 5));
    }

    #[test]
    fn the_maximise_button_is_not_published_under_a_modal_or_a_pane() {
        use app::{Command, Pane};
        use docking::{Place, Unit};
        let mut w = Win::new();
        w.app.own_caption = true;
        w.idle(3);
        let r = winframe::max_button().expect("published");
        // A floating pane over the top-right corner takes the clicks there.
        w.app.dispatch(Command::ActivatePane(Pane::Vector));
        let d = &mut w.app.docks.0[Unit::Vector as usize];
        d.float_rect = Some(Rect::from_min_size(Pos2::new(r[0] as f32 - 60.0, r[1] as f32 - 2.0), Vec2::new(260.0, 220.0)));
        d.place = Place::Floating;
        w.idle(4);
        assert_eq!(winframe::max_button(), None, "a floating pane lies over the button");
        w.app.docks.0[Unit::Vector as usize].float_rect = Some(Rect::from_min_size(Pos2::new(300.0, 300.0), Vec2::new(260.0, 220.0)));
        w.idle(4);
        assert!(winframe::max_button().is_some(), "the pane moved away");
        // A modal dialog disables the frame's buttons.
        w.app.color_dialog = Some(egui::Color32::RED);
        w.idle(4);
        assert!(w.app.modal_up);
        assert_eq!(winframe::max_button(), None, "a modal dialog is up");
        w.app.color_dialog = None;
        w.idle(4);
        assert!(winframe::max_button().is_some(), "the dialog closed");
    }

    #[test]
    fn the_document_menu_button_is_lit_hot_pressed_and_open() {
        assert_eq!(doc_menu_fill(false, false, false), None);
        let hot = doc_menu_fill(false, false, true).unwrap();
        let lit = doc_menu_fill(false, true, true).unwrap();
        assert_eq!(hot.0, egui::Color32::from_rgb(255, 244, 202));
        assert_eq!(lit.1, egui::Color32::from_rgb(254, 181, 91));
        assert_eq!(doc_menu_fill(true, false, false), Some(lit));
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
        // `FV_WINDOW_NATIVE=<scale>`: started on a monitor at that display
        // scale (1.25 for 125 %) — the chrome at its DPI, the size in device
        // pixels (below).
        let native = std::env::var("FV_WINDOW_NATIVE").ok().and_then(|s| s.parse::<f32>().ok());
        chrome_dpi(native);
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
        let c = match native {
            Some(native) => raster::render_native(size, native, |ctx| device_options(ctx, Some(native)), device_screen, Some(&fonts), passes, paint),
            None => raster::render_input(size, 1.0, Some(&fonts), passes, paint),
        };
        let rgba: Vec<u8> = c.px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        std::fs::write(path, util::png_rgba(c.w as u32, c.h as u32, &rgba)).unwrap();
    }

    /// #78, #141 through the call site: a zoomed plane's scroll bar is 25 px at
    /// 150 % (the system's 26 less one; its grey track across the window's right
    /// edge), not the 100 % width scaled (24).
    #[test]
    fn a_zoomed_planes_scroll_bar_is_the_system_width() {
        let fonts = app_fonts();
        for (dpi_v, native, bar) in [(96u32, 1.0f32, 16usize), (120, 1.25, 20), (144, 1.5, 25)] {
            dpi::set(dpi_v);
            let mut app = demo(&[]);
            app.diag.circle.z = 2.0;
            let c = raster::render_native(Vec2::new(700.0, 500.0), native, |ctx| device_options_on(true, ctx, Some(native)), device_screen, Some(&fonts), vec![vec![]; 3], |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| app.plane_ui(ui));
            });
            let y = 250;
            let run = (0..c.w).rev().take_while(|&x| c.px[y * c.w + x] == [235, 235, 235] || c.px[y * c.w + x] == [190, 190, 190]).count();
            assert_eq!(run, bar, "{dpi_v}: the scroll bar's width");
        }
        dpi::set(96);
    }

    /// #78 through the strip layout: an inactive tab is its text in the
    /// whole 8 pt font (16 px at 150 %) plus 34 px scaled (51).
    #[test]
    fn a_tab_is_laid_out_in_the_whole_8_pt_font() {
        let fonts = app_fonts();
        dpi::set(144);
        let mut app = demo(&[app::Command::Demo]);
        let mut got = None;
        raster::render_native(Vec2::new(1280.0, 722.0), 1.5, |ctx| device_options_on(true, ctx, Some(1.5)), device_screen, Some(&fonts), vec![vec![]; 3], |ctx| {
            app.frame_ui(ctx);
            let other = (0..app.docs.len()).find(|&i| app.active != Some(i));
            got = other.and_then(|i| {
                let title = app.docs[i].title();
                let w = ctx.read_response(egui::Id::new(("fv_doc_tab", app.docs[i].serial)))?.rect.width();
                let text = ctx.fonts(|f| f.layout_no_wrap(title, egui::FontId::proportional(16.0), egui::Color32::BLACK).size().x);
                Some((w, text + 51.0))
            });
        });
        dpi::set(96);
        let (w, want) = got.expect("an inactive tab");
        assert_eq!(w, want, "the tab's width");
    }

    /// `make_app` takes the chrome's dpi from GetDpiForSystem
    /// (`system_dpi_scale`) before the monitor's scale; its source line is
    /// the call pinned here.
    #[test]
    fn make_app_passes_the_system_dpi_to_the_chrome() {
        let src = include_str!("main.rs");
        assert!(src.contains(concat!("start_scaling(DEVICE_PIXELS, ctx, scale, ", "system_dpi_scale());")));
    }
}
