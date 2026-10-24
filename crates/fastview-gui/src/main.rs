//! FastView — a cross-platform reconstruction of НТЦ «Механотроника» Fast View 5,
//! the relay-protection oscillogram/characteristic analysis tool. Office-2007
//! ribbon shell, MDI documents and a lane-based oscillogram view. Builds and runs
//! on Linux, Windows and macOS.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod chandlg;
mod chansel;
mod comments;
mod diagram;
mod diagram_ui;
mod dialogs;
mod docking;
mod finder;
mod formula_dlg;
mod formula_page;
mod icons;
mod instance;
mod keytips;
mod sysclip;
mod lanes;
mod merge_ui;
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
mod spool;
mod theme;
mod util;
mod views;
mod windows_dlg;

use app::App;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1320.0, 860.0])
        .with_min_inner_size([866.0, 500.0])
        .with_title(app::APP_TITLE)
        .with_drag_and_drop(true);
    // The ribbon draws the caption row (L9.150): no system decorations,
    // unless `FV_SYSTEM_TITLEBAR` asks for them.
    let own_caption = std::env::var_os("FV_SYSTEM_TITLEBAR").is_none();
    viewport = viewport.with_decorations(!own_caption);
    // `LoadState` restores the frame's saved window placement.
    if let Some((r, max)) = window_placement(&persist::Store::load()) {
        viewport = viewport.with_inner_size(r.size()).with_position(r.min).with_maximized(max);
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
    eframe::run_native(
        "FastView",
        options,
        Box::new(|cc| {
            let fonts = app_fonts();
            cc.egui_ctx.set_fonts(fonts.clone());
            let mut app = App::new().with_persistence();
            app.own_caption = own_caption;
            app.print.fonts = fonts;
            let ctx = cc.egui_ctx.clone();
            app.instance = inst_dir.as_deref().and_then(|d| instance::serve(d, move || ctx.request_repaint()));
            if let Some(dir) = &args.dst {
                app.dst_dir = Some(dir.clone());
            }
            if args.find {
                app.dispatch(app::Command::ToggleExplorer);
            }
            for f in &args.files {
                app.load_comtrade_path(f);
            }
            Ok(Box::new(app))
        }),
    )
}

/// `Workspace/WindowPlacement` ("l t r b maximised") as saved on exit.
fn window_placement(st: &persist::Store) -> Option<(egui::Rect, bool)> {
    let s = st.view.get("Workspace/WindowPlacement")?;
    let (rect, max) = s.rsplit_once(' ')?;
    Some((docking::parse_rect(rect)?, max == "1"))
}

/// The window's fonts: the first system font found that covers Cyrillic,
/// in front of egui's own.
fn app_fonts() -> egui::FontDefinitions {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "C:/Windows/Fonts/segoeui.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
    ];
    let mut fonts = egui::FontDefinitions::default();
    if let Some(bytes) = CANDIDATES.iter().find_map(|p| std::fs::read(p).ok()) {
        fonts.font_data.insert("cyr".to_owned(), egui::FontData::from_owned(bytes));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().insert(0, "cyr".to_owned());
        }
    }
    fonts
}

impl App {
    /// The tabbed-MDI strip (`EnableMDITabbedGroups` `0x47293b` with
    /// OneNote tabs, auto colour, the document menu, the close button on the
    /// active tab, tab swap left on): tabs coloured per document, the ▾
    /// document list; a tab dragged over another swaps with it
    /// (`0x100690b4`).
    fn doc_tabs(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        let mut windows = false;
        let mut dragged = None;
        let mut tabs: Vec<egui::Rect> = Vec::new();
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            for i in 0..self.docs.len() {
                if self.docs[i].tab_color.is_none() {
                    let prev = i.checked_sub(1).and_then(|j| self.docs[j].tab_color);
                    self.docs[i].tab_color = Some(self.tab_auto.assign(i, prev));
                }
                let active = self.active == Some(i);
                let title = self.docs[i].title();
                let font = egui::FontId::proportional(13.0);
                let text_w = ui.fonts(|f| f.layout_no_wrap(title.clone(), font.clone(), egui::Color32::BLACK).size().x);
                let w = text_w + 34.0 + if active { 20.0 } else { 0.0 };
                // An id per document: the drag follows the tab when it moves.
                let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::hover());
                let resp = ui.interact(rect, egui::Id::new(("fv_doc_tab", self.docs[i].serial)), egui::Sense::click_and_drag());
                tabs.push(rect);
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
                ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, &title, font.clone(), egui::Color32::BLACK);
                ui.painter().text(text_pos + egui::vec2(0.5, 0.0), egui::Align2::LEFT_CENTER, &title, font, egui::Color32::BLACK);
                if active {
                    let xr = egui::Rect::from_center_size(egui::pos2(rect.right() - 12.0, rect.center().y), egui::vec2(14.0, 14.0));
                    let xresp = ui.interact(xr, ui.id().with(("tab_close", i)), egui::Sense::click());
                    let col = if xresp.hovered() { egui::Color32::from_rgb(200, 0, 0) } else { egui::Color32::from_gray(90) };
                    ui.painter().text(xr.center(), egui::Align2::CENTER_CENTER, "✕", egui::FontId::proportional(11.0), col);
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
                ui.add_space(-6.0);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("▾", |ui| {
                    for i in 0..self.docs.len() {
                        if ui.radio(self.active == Some(i), self.docs[i].title()).clicked() {
                            activate = Some(i);
                            ui.close_menu();
                        }
                    }
                    // `EnableWindowsDialog(131, 132, always)`: «Окна...» last.
                    ui.separator();
                    if ui.button("Окна...").clicked() {
                        windows = true;
                        ui.close_menu();
                    }
                });
            });
        });
        self.doc_tab_rects.clone_from(&tabs);
        if let Some(i) = activate {
            self.dispatch(app::Command::ActivateDoc(i));
        }
        // Past the other tab's middle, so tabs of unequal widths do not swap
        // back and forth.
        if let (Some(i), Some(at)) = (dragged, ui.ctx().input(|inp| inp.pointer.latest_pos())) {
            let own = tabs[i].center().x;
            if let Some(j) = tabs.iter().position(|r| r.contains(at) && (r.center().x > own) == (at.x > r.center().x)).filter(|&j| j != i) {
                self.docs.swap(i, j);
                self.active = Some(j);
            }
        }
        if let Some(i) = close {
            self.dispatch(app::Command::CloseDoc(i));
        }
        if windows {
            self.open_windows_dialog();
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
        ui.horizontal(|ui| {
            // Six fixed-width panes (27 / 40-character templates, `0x4729b0`).
            let char_w = 6.2;
            for (i, text) in s.iter().enumerate() {
                let w = if i == 2 || i == 5 { 40.0 } else { 27.0 } * char_w;
                ui.add_sized([w, 16.0], egui::Label::new(egui::RichText::new(text).small().color(p.status_text)).truncate());
                ui.separator();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("НТЦ Механотроника").small().color(p.status_text.gamma_multiply(0.6f32)));
            });
        });
        self.status_toast(ui.ctx());
        self.resize_edges(ui.ctx());
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

fn startup_args(args: impl Iterator<Item = String>) -> StartupArgs {
    let mut out = StartupArgs::default();
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        if a.eq_ignore_ascii_case("-dst") {
            out.dst = args.next().map(Into::into);
        } else if a.eq_ignore_ascii_case("-find") {
            out.find = true;
        } else {
            out.files.push(a.into());
        }
    }
    out
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_ui(ctx);
    }
}

impl App {
    /// One frame of the whole window (kept apart from `eframe::App::update`
    /// so headless tests can drive it).
    pub fn frame_ui(&mut self, ctx: &egui::Context) {
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
            dialogs::modal_backdrop(ctx, "fv_modal_backdrop", egui::Order::PanelResizeLine);
            self.keytips.exit();
        } else if !self.keytips_input(ctx) && self.print.preview.is_none() {
            // Print preview swaps table 128 for MFC's 30997 (`0x102474d9`).
            self.handle_keys(ctx);
        }

        let pal = self.skin.palette();
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::none().fill(pal.tab_active))
            .show(ctx, |ui| {
                self.ribbon_ui(ui);
                // Caption bars docked under the ribbon: the ОМП result.
                self.omp_banner(ui);
                if !self.docs.is_empty() {
                    self.doc_tabs(ui);
                }
            });
        // 142: the parameters bar, a 75 px strip under the ribbon.
        // MFC hides the docking panes during print preview.
        let previewing = self.print.preview.is_some();
        if self.dlg.param_bar && !previewing {
            egui::TopBottomPanel::top("parambar")
                .exact_height(75.0)
                .frame(egui::Frame::none().fill(pal.group_bg))
                .show(ctx, |ui| self.param_bar_ui(ui));
        }

        // The merge bar (CMergeBar, 70 px), shown after a paste.
        if self.merge.is_some() && !previewing {
            egui::TopBottomPanel::top("mergebar")
                .exact_height(70.0)
                .frame(egui::Frame::none().fill(pal.group_bg).inner_margin(egui::Margin::symmetric(15.0, 10.0)))
                .show(ctx, |ui| self.merge_bar_ui(ui));
        }

        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none().fill(pal.status_bg).inner_margin(egui::Margin::symmetric(6.0, 3.0)))
            .show(ctx, |ui| self.status_bar(ui));


        // The print preview takes the place of the views.
        if self.print.preview.is_some() {
            self.preview_ui(ctx);
        } else {
            self.diagram_panes(ctx);
            self.render_tab(ctx);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_switches() {
        let a = startup_args(["-Dst", "/d", "-FIND", "x.cfg"].into_iter().map(String::from));
        assert_eq!(a, StartupArgs { dst: Some("/d".into()), find: true, files: vec!["x.cfg".into()] });
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
    fn frames_held(app: &mut App, ctx: &egui::Context, size: Vec2, batches: Vec<Vec<egui::Event>>, modifiers: egui::Modifiers) {
        for events in batches {
            let t = ctx.input(|i| i.time) + 0.05;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(t), events, modifiers, ..Default::default() };
            let _ = ctx.run(raw, |ctx| app.frame_ui(ctx));
        }
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

        fn run(&mut self, events: Vec<egui::Event>) {
            frames_held(&mut self.app, &self.ctx, Vec2::new(1320.0, 860.0), vec![events], self.held);
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
            self.run(vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
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
        let at = Pos2::new(grid.right() - 60.0, grid.top() + 8.0 + 30.0);
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

    /// The parameters bar is a message-mode caption bar (`0x474dc5`): its
    /// close button at the top right hides it.
    #[test]
    fn the_parameters_bar_closes_from_its_button() {
        use egui::PointerButton::Primary;
        let mut w = Win::new();
        assert!(w.app.dlg.param_bar);
        w.press(Primary, Pos2::new(400.0, 220.0), Pos2::new(400.0, 220.0));
        assert!(w.app.dlg.param_bar, "a click on the bar's text does nothing");
        let mut y = 186.0;
        while w.app.dlg.param_bar && y < 240.0 {
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
        let win = w.ctx.memory(|m| m.area_rect(egui::Id::new("Цвет"))).expect("the colour dialog");
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

    /// The «Файл» menu is two columns wide, not the window's width; hovering
    /// «Печать» puts its sub-items in the right column.
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
        assert!(menu.width() > 500.0 && menu.width() < 600.0, "{menu:?}");
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
        // Across to the first sub-item, «Параметры страницы».
        let target = Pos2::new(menu.left() + 300.0, menu.top() + 45.0);
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

    /// A narrow window collapses ribbon panels from the right instead of
    /// wrapping their labels (MFC's reduction steps, L9.91); the ribbon keeps its height and a collapsed
    /// panel opens below its button.
    #[test]
    fn narrow_ribbon_collapses_panels_and_keeps_its_height() {
        use egui::containers::panel::PanelState;
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
        let (h_narrow, c_narrow, ctx, mut app) = height(700.0);
        assert!((h_narrow - h_wide).abs() < 1.0, "ribbon height {h_narrow} vs {h_wide}");
        // MFC's order (L9.91): Вычисления and Диаграммы step through their
        // layouts first, «Вычисления» collapses (step 3); with the read
        // collapsed-button width (~30 px for the untitled panel, L9.114)
        // the single-button " " panel is past 1.5 × it and collapses too.
        assert!(c_narrow[3] == 3 && c_narrow[1] > 0, "steps: {c_narrow:?}");
        // The first width that needs a step: the panel with the most steps
        // left takes it (ties: the rightmost).
        let first = (0..30).map(|k| height(1300.0 - 20.0 * k as f32).1).find(|c| c.iter().any(|&s| s > 0)).expect("a step before 700 px");
        assert_eq!(first, vec![0, 0, 0, 1, 0], "first step: the panel with the most steps left, rightmost");
        // «Вычисления»'s button opens the panel as a popup.
        let popup = egui::Id::new(("fv_ribbon_group_popup", "Вычисления"));
        for x in (300..700).step_by(10) {
            let at = Pos2::new(x as f32, 100.0);
            let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frames(&mut app, &ctx, Vec2::new(700.0, 860.0), vec![vec![egui::Event::PointerMoved(at)], vec![ev(true)], vec![ev(false)], vec![]]);
            if ctx.memory(|m| m.is_popup_open(popup)) {
                return;
            }
            frames(&mut app, &ctx, Vec2::new(700.0, 860.0), vec![vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }], vec![]]);
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
        double(&mut w, Pos2::new(docked.left() + 60.0, docked.top() - 22.0));
        assert_eq!(w.app.docks[Unit::Vector].place, Place::Floating);
        let floating = canvas(&w).expect("drawn in its window");
        double(&mut w, Pos2::new(floating.left() + 60.0, floating.top() - 22.0));
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
        drag(&mut w, Pos2::new(f.left() + 80.0, f.top() - 22.0), &centre);
        assert_eq!((w.app.docks[Unit::Frequency].place, w.app.docks[Unit::Frequency].spot), (Place::Docked, Spot::Tab(Unit::Vector)));
        assert_eq!(w.app.dock_views_shown(), 1, "one tabbed group");
        // The group's caption out to the left edge marker.
        let v = canvas(&w, app::Pane::Vector).or_else(|| canvas(&w, app::Pane::Frequency)).expect("the group's pane");
        let left = |a: &App| docking::edge_markers(a.dock_client)[0].0.center();
        drag(&mut w, Pos2::new(v.left() + 80.0, v.top() - 22.0), &left);
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
        w.held = egui::Modifiers::CTRL;
        w.run(vec![egui::Event::Copy]);
        assert!(w.app.clip.available(), "Ctrl+C copies");
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
        // The MDI keys: Ctrl+Tab / Ctrl+Shift+Tab cycle, Ctrl+F4 closes.
        w.app.dispatch(app::Command::Demo);
        w.idle(2);
        assert_eq!((w.app.docs.len(), w.app.active), (2, Some(1)));
        w.key(egui::Key::Tab);
        assert_eq!(w.app.active, Some(0), "Ctrl+Tab: the next, wrapping");
        w.held = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
        w.key(egui::Key::F6);
        assert_eq!(w.app.active, Some(1), "Ctrl+Shift+F6: the previous");
        w.held = egui::Modifiers::CTRL;
        w.app.docs[1].set.modified = false;
        w.key(egui::Key::F4);
        w.idle(2);
        assert_eq!(w.app.docs.len(), 1, "Ctrl+F4 closes the active document");
    }

    /// MFC's pane menu on a caption right-click (`0x100ce72c`): Плавающее
    /// floats the pane, Закрепленное docks it back; В закладке is greyed.
    #[test]
    fn the_pane_menu_floats_and_docks() {
        use docking::{Place, Unit};
        let mut w = Win::new();
        let f = w.app.pane_canvas.iter().find(|(q, _)| *q == app::Pane::Frequency).map(|(_, r)| *r).expect("Частотная drawn");
        w.press(egui::PointerButton::Secondary, Pos2::new(f.left() + 80.0, f.top() - 22.0), Pos2::new(f.left() + 80.0, f.top() - 22.0));
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
    /// out of the strip it stays where it is (no new tab group: FastView
    /// keeps MFC's `OnShowMDITabContextMenu`, slot 117).
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
        assert_eq!(serials(&w), [before[1], before[0]], "out of the strip: no change");
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
        while !app.loading.is_empty() {
            app.poll_loading();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let ctx = egui::Context::default();
        frames(&mut app, &ctx, Vec2::new(1320.0, 860.0), vec![vec![]; 3]);
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
            "explorer" => vec![C::ToggleExplorer],
            "parambar" => vec![C::ToggleParamBar],
            "vizirs" => vec![C::ToggleVizir(0), C::ToggleVizir(1), C::ToggleVizir(2)],
            // `skin<k>`: the k-th look of `Skin::ALL`.
            s if s.starts_with("skin") => s[4..].parse::<usize>().ok().and_then(|k| theme::Skin::ALL.get(k).copied()).map(C::SetSkin).into_iter().collect(),
            _ => vec![],
        };
        // `FV_WINDOW_RECORD=<file.cfg>`: that record instead of the demo.
        let mut app = match std::env::var("FV_WINDOW_RECORD") {
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
        // `FV_WINDOW_STATE=windows`: «Окна» over two documents.
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "windows") {
            let osc = fvb_core::comtrade::parse(app::DEMO_CFG, app::DEMO_DAT).unwrap();
            app.add_doc(app::Doc::new(osc, "Подстанция 2".into()));
            app.open_windows_dialog();
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
        if std::env::var("FV_WINDOW_STATE").is_ok_and(|s| s == "analysis") {
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
        let fonts = app_fonts();
        let c = raster::render_input(size, 1.0, Some(&fonts), passes, |ctx| app.frame_ui(ctx));
        let rgba: Vec<u8> = c.px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        std::fs::write(path, util::png_rgba(c.w as u32, c.h as u32, &rgba)).unwrap();
    }
}
