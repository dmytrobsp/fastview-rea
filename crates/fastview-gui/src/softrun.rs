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

/// Whether `FV_SOFTLOG` names a file (the trace lines are only worth building then).
fn tracing() -> bool {
    std::env::var_os("FV_SOFTLOG").is_some()
}

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
        crate::minsize::install_on(&window);
        crate::device_window(&self.ctx, window.scale_factor() as f32);
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
                if tracing() && !matches!(event, WindowEvent::CursorMoved { .. }) {
                    trace(&format!("event {:.70}", format!("{event:?}")));
                }
                if matches!(event, WindowEvent::CursorMoved { .. } | WindowEvent::CursorEntered { .. } | WindowEvent::CursorLeft { .. } | WindowEvent::MouseInput { .. }) {
                    trace(&format!("event {event:?}"));
                }
                // A Win32 click carries where it happened; egui-winit drops a
                // button while it has seen no cursor move (a window shown
                // under a still pointer: VM runs whose pointer was already
                // there missed their clicks). The OS cursor first.
                if let WindowEvent::MouseInput { device_id, .. } = &event {
                    if let Some(position) = cursor_in(window) {
                        let _ = state.on_window_event(window, &WindowEvent::CursorMoved { device_id: *device_id, position });
                    }
                }
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
        let wheels: Vec<String> = raw.events.iter().filter_map(|e| if let egui::Event::MouseWheel { delta, modifiers, .. } = e { Some(format!("{:.1}{}", delta.y, if modifiers.ctrl { "^" } else { "" })) } else { None }).collect();
        if !wheels.is_empty() {
            trace(&format!("wheel {wheels:?}, frame mods ctrl {}", raw.modifiers.ctrl));
        }
        raw.viewports.insert(egui::ViewportId::ROOT, self.info.clone());
        crate::device_screen(&self.ctx, &mut raw);
        // A close request (WM_CLOSE: Alt+F4, the taskbar) closes the window
        // unless the app cancels it this frame, as eframe does.
        let mut closing = self.info.events.iter().any(|e| matches!(e, egui::ViewportEvent::Close));
        self.info.events.clear();
        // Quick key presses each in a pass of their own; the last pass's
        // picture is shown, every pass's textures kept.
        let mut textures = egui::TexturesDelta::default();
        let mut last = None;
        for mut raw in crate::one_press_per_pass(raw) {
            crate::dialogs::take_dialog_tabs(&self.ctx, &mut raw);
            let app = &mut self.app;
            let mut out = self.ctx.run(raw, |ctx| {
                app.frame_ui(ctx);
                app.splash(ctx);
            });
            if tracing() {
                let delay = out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay);
                trace(&format!("pass: repaint in {delay:?} causes {:?}", self.ctx.repaint_causes().iter().map(|c| c.to_string()).collect::<Vec<_>>()));
            }
            state.handle_platform_output(window, std::mem::take(&mut out.platform_output));
            textures.append(std::mem::take(&mut out.textures_delta));
            if let Some(vp) = out.viewport_output.get(&egui::ViewportId::ROOT) {
                let mut actions = egui::ahash::HashSet::default();
                egui_winit::process_viewport_commands(&self.ctx, &mut self.info, vp.commands.iter().cloned(), window, &mut actions);
                if vp.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Close)) {
                    event_loop.exit();
                    return;
                }
                if vp.commands.iter().any(|c| matches!(c, egui::ViewportCommand::CancelClose)) {
                    closing = false;
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
        if closing {
            event_loop.exit();
            return;
        }
        let Some(out) = last else { return };
        let size = window.inner_size();
        let t0 = Instant::now();
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let canvas = self.painter.paint(&prims, &textures, size.width as usize, size.height as usize, out.pixels_per_point);
        blit(window, &canvas);
        trace(&format!("frame {}×{} in {:?}; dragged {:?}, down {}, top {:?}", size.width, size.height, t0.elapsed(), self.ctx.dragged_id(), self.ctx.input(|i| i.pointer.any_down()), self.ctx.pointer_latest_pos().and_then(|p| self.ctx.layer_id_at(p))));
    }
}

/// The OS cursor in the window's client coordinates (physical pixels).
fn cursor_in(window: &Window) -> Option<winit::dpi::PhysicalPosition<f64>> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::Win32(h) = handle.as_raw() else { return None };
    let hwnd = h.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: plain queries into a local POINT for our own window.
    let ok = unsafe { GetCursorPos(&mut p) != 0 && ScreenToClient(hwnd, &mut p) != 0 };
    ok.then(|| winit::dpi::PhysicalPosition::new(p.x as f64, p.y as f64))
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
