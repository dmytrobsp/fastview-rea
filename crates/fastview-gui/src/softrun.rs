//! The software runner (W4.1c): for Windows without OpenGL 2 or a
//! Direct3D 12 adapter (the VM's basic display driver, where eframe cannot
//! start), as the original runs on plain GDI. A winit window, egui-winit's
//! input and viewport handling, egui's output rasterised by `raster` and
//! blitted into the window with GDI. Slower than the GPU path, but the same
//! frames: chosen by `FV_SOFTWARE=1` or when eframe fails to start.

use crate::app::App;
use crate::raster::Painter;
use eframe::egui;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

/// A line in the file `FV_SOFTLOG` names (a release build has no console).
pub fn trace(msg: &str) {
    if let Some(p) = std::env::var_os("FV_SOFTLOG") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{:?} {msg}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default());
        }
    }
}

/// Run `app` in a window made from `viewport` until it closes.
pub fn run(app: App, viewport: egui::ViewportBuilder, ctx: egui::Context) -> Result<(), String> {
    trace("run");
    // Immediate viewports (the splash) are drawn inside the main window.
    ctx.set_embed_viewports(true);
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut soft = Soft { app, ctx, viewport, window: None, state: None, info: egui::ViewportInfo::default(), painter: Painter::default(), wake: None };
    event_loop.run_app(&mut soft).map_err(|e| e.to_string())
}

struct Soft {
    app: App,
    ctx: egui::Context,
    viewport: egui::ViewportBuilder,
    window: Option<Window>,
    state: Option<egui_winit::State>,
    info: egui::ViewportInfo,
    painter: Painter,
    /// When egui asked to be repainted next.
    wake: Option<Instant>,
}

impl ApplicationHandler for Soft {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = match egui_winit::create_window(&self.ctx, event_loop, &self.viewport) {
            Ok(w) => w,
            Err(e) => {
                trace(&format!("no window: {e}"));
                event_loop.exit();
                return;
            }
        };
        // The builder's size and position, applied after creation, take a
        // window out of maximised on Windows: maximised again.
        if self.viewport.maximized == Some(true) {
            window.set_maximized(true);
        }
        trace(&format!("window {:?} visible {:?}", window.inner_size(), window.is_visible()));
        let state = egui_winit::State::new(self.ctx.clone(), egui::ViewportId::ROOT, &window, Some(window.scale_factor() as f32), None, Some(16384));
        egui_winit::update_viewport_info(&mut self.info, &self.ctx, &window, true);
        window.request_redraw();
        self.window = Some(window);
        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let (Some(window), Some(state)) = (self.window.as_ref(), self.state.as_mut()) else { return };
        match event {
            WindowEvent::RedrawRequested => self.frame(event_loop),
            WindowEvent::CloseRequested => {
                // As eframe: the app sees the close request in a frame.
                self.info.events.push(egui::ViewportEvent::Close);
                window.request_redraw();
            }
            event => {
                let r = state.on_window_event(window, &event);
                if r.repaint {
                    window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.wake {
            Some(t) if t <= Instant::now() => {
                self.wake = None;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Some(t) => event_loop.set_control_flow(ControlFlow::WaitUntil(t)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}

impl Soft {
    /// One frame: the input, the app, the platform and viewport output, the
    /// picture.
    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(window), Some(state)) = (self.window.as_ref(), self.state.as_mut()) else { return };
        egui_winit::update_viewport_info(&mut self.info, &self.ctx, window, false);
        let mut raw = state.take_egui_input(window);
        raw.viewports.insert(egui::ViewportId::ROOT, self.info.clone());
        self.info.events.clear();
        // Quick key presses each in a pass of their own; the last pass's
        // picture is shown, every pass's textures kept.
        let mut textures = egui::TexturesDelta::default();
        let mut last = None;
        for raw in crate::one_press_per_pass(raw) {
            let app = &mut self.app;
            let mut out = self.ctx.run(raw, |ctx| {
                app.frame_ui(ctx);
                app.splash(ctx);
            });
            state.handle_platform_output(window, std::mem::take(&mut out.platform_output));
            textures.append(std::mem::take(&mut out.textures_delta));
            if let Some(vp) = out.viewport_output.get(&egui::ViewportId::ROOT) {
                let mut actions = egui::ahash::HashSet::default();
                egui_winit::process_viewport_commands(&self.ctx, &mut self.info, vp.commands.iter().cloned(), window, &mut actions);
                if vp.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Close)) {
                    event_loop.exit();
                    return;
                }
                self.wake = if vp.repaint_delay.is_zero() {
                    window.request_redraw();
                    None
                } else {
                    Instant::now().checked_add(vp.repaint_delay)
                };
            }
            last = Some(out);
        }
        let Some(out) = last else { return };
        let size = window.inner_size();
        let t0 = Instant::now();
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let canvas = self.painter.paint(&prims, &textures, size.width as usize, size.height as usize, out.pixels_per_point);
        blit(window, &canvas);
        trace(&format!("frame {}×{} in {:?}", size.width, size.height, t0.elapsed()));
    }
}

/// The picture into the window's client area (SetDIBitsToDevice, a
/// top-down 32-bit DIB).
fn blit(window: &Window, c: &crate::raster::Canvas) {
    use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC, SetDIBitsToDevice, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS};
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::Win32(h) = handle.as_raw() else { return };
    let hwnd = h.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    let bgra: Vec<u8> = c.px.iter().flat_map(|p| [p[2], p[1], p[0], 0]).collect();
    // SAFETY: the window's own DC, released; the bits outlive the call and
    // match the header's size.
    unsafe {
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = c.w as i32;
        info.bmiHeader.biHeight = -(c.h as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let dc = GetDC(hwnd);
        if !dc.is_null() {
            SetDIBitsToDevice(dc, 0, 0, c.w as u32, c.h as u32, 0, 0, 0, c.h as u32, bgra.as_ptr().cast(), &info, DIB_RGB_COLORS);
            ReleaseDC(hwnd, dc);
        }
    }
}
