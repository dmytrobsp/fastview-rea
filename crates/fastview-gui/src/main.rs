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
mod finder;
mod formula_dlg;
mod icons;
mod instance;
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

use app::App;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([866.0, 500.0])
            .with_title("Механотроника FastView")
            .with_drag_and_drop(true),
        ..Default::default()
    };
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
    /// The tabbed-MDI strip: OneNote-style tabs coloured per document, the
    /// close button on the active tab only, and the ▾ document list.
    fn doc_tabs(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            for i in 0..self.docs.len() {
                let active = self.active == Some(i);
                let title = self.docs[i].name.clone();
                let font = egui::FontId::proportional(13.0);
                let text_w = ui.fonts(|f| f.layout_no_wrap(title.clone(), font.clone(), egui::Color32::BLACK).size().x);
                let w = text_w + 34.0 + if active { 20.0 } else { 0.0 };
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), egui::Sense::click());
                let base = tab_color(i);
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
                if resp.clicked() && close.is_none() {
                    activate = Some(i);
                }
                ui.add_space(-6.0);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("▾", |ui| {
                    for i in 0..self.docs.len() {
                        if ui.radio(self.active == Some(i), self.docs[i].name.clone()).clicked() {
                            activate = Some(i);
                            ui.close_menu();
                        }
                    }
                });
            });
        });
        if let Some(i) = activate {
            self.dispatch(app::Command::ActivateDoc(i));
        }
        if let Some(i) = close {
            self.dispatch(app::Command::CloseDoc(i));
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.skin.palette();
        let mut s = self.cursor_status();
        // In print preview the page goes to the first pane (MFC's
        // OnDisplayPageNumber writes the message bar's pane 0).
        if let Some(pv) = &self.print.preview {
            s[0] = pv.page_text();
        }
        ui.horizontal(|ui| {
            // Six fixed-width panes (27 / 40-character templates, `0x4729b0`).
            let char_w = 6.2;
            for (i, text) in s.iter().enumerate() {
                let w = if i == 2 || i == 5 { 40.0 } else { 27.0 } * char_w;
                ui.add_sized([w, 16.0], egui::Label::new(egui::RichText::new(text).small().color(p.text)).truncate());
                ui.separator();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("НТЦ Механотроника").small().weak());
            });
        });
        self.status_toast(ui.ctx());
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

/// Tab colours of the documents, in order (MFC OneNote auto colours; the
/// first one is the yellow of the help screenshots, the order of the rest is
/// not determined).
fn tab_color(i: usize) -> egui::Color32 {
    const C: [(u8, u8, u8); 8] = [
        (255, 219, 117),
        (148, 175, 230),
        (189, 205, 159),
        (240, 158, 159),
        (186, 166, 225),
        (154, 191, 180),
        (247, 182, 131),
        (216, 171, 192),
    ];
    let (r, g, b) = C[i % C.len()];
    egui::Color32::from_rgb(r, g, b)
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
            Some(d) => format!("Механотроника FastView - {}", d.name),
            None => "Механотроника FastView".to_string(),
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
        } else {
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
            .frame(egui::Frame::none().fill(pal.group_bg).inner_margin(egui::Margin::symmetric(6.0, 3.0)))
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
        for events in batches {
            let t = ctx.input(|i| i.time) + 0.05;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(t), events, ..Default::default() };
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
        use app::{Command as C, Pane, View};
        sweep(&mut App::new());
        sweep(&mut demo(&[]));
        sweep(&mut demo(&[C::Show(View::Analysis)]));
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

    /// A whole window at the default size with the demo document.
    struct Win {
        ctx: egui::Context,
        app: App,
    }

    impl Win {
        fn new() -> Win {
            let mut w = Win { ctx: egui::Context::default(), app: demo(&[]) };
            w.idle(3);
            w
        }

        fn run(&mut self, events: Vec<egui::Event>) {
            frames(&mut self.app, &self.ctx, Vec2::new(1320.0, 860.0), vec![events]);
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
            let tabs = PanelState::load(&ctx, egui::Id::new("pane_tabs")).expect("the tab strip").rect;
            // Either it ends above the tabs or it is too short for a button.
            assert!(bar.bottom() <= tabs.top() + 0.5 || bar.height() < 22.0, "h {h}: toolbar {bar:?} over tabs {tabs:?}");
        }
    }

    /// A narrow window collapses ribbon panels from the right instead of
    /// wrapping their labels; the ribbon keeps its height and a collapsed
    /// panel opens below its button.
    #[test]
    fn narrow_ribbon_collapses_panels_and_keeps_its_height() {
        use egui::containers::panel::PanelState;
        let height = |w: f32| {
            let ctx = egui::Context::default();
            let mut app = demo(&[]);
            frames(&mut app, &ctx, Vec2::new(w, 860.0), vec![vec![]; 3]);
            let collapsed: Vec<bool> = ["Объединение", "Диаграммы", "Видимость", "Вычисления", " "]
                .iter()
                .map(|t| ctx.data(|d| d.get_temp::<bool>(egui::Id::new(("fv_ribbon_group_collapsed", *t)))).unwrap_or(false))
                .collect();
            (PanelState::load(&ctx, egui::Id::new("top")).unwrap().rect.height(), collapsed, ctx, app)
        };
        let (h_wide, c_wide, ..) = height(1320.0);
        assert_eq!(c_wide, vec![false; 5], "everything fits at the default size");
        let (h_narrow, c_narrow, ctx, mut app) = height(700.0);
        assert!((h_narrow - h_wide).abs() < 1.0, "ribbon height {h_narrow} vs {h_wide}");
        assert!(c_narrow[3] && !c_narrow[0], "collapsed from the right: {c_narrow:?}");
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
        assert_eq!(w.app.copy_focus, Some(app::Pane::Vector));
        w.app.dispatch(app::Command::Copy);
        assert_eq!(w.app.clip.picture(), Some((canvas.width().round() as usize, canvas.height().round() as usize)));
        assert_eq!(w.app.clip.get(), None, "a diagram copy leaves no document");
        // Back to the view.
        w.press(Primary, stats, stats);
        assert_eq!(w.app.copy_focus, None);
        w.app.dispatch(app::Command::Copy);
        assert!(w.app.clip.get().is_some());
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
        use app::{Command as C, Pane, RibbonTab, View};
        let cmds = match std::env::var("FV_WINDOW_STATE").unwrap_or_default().as_str() {
            "analysis" => vec![C::Show(View::Analysis)],
            "circle" => vec![C::ActivatePane(Pane::Circle)],
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
        app.mru = vec!["/data/osc/2024-05-01 Подстанция 110 кВ.cfg".into(), "/data/osc/short.cff".into()];
        let size = pos("FV_WINDOW_SIZE").map_or(Vec2::new(1320.0, 860.0), |p| p.to_vec2());
        let ev = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let passes = vec![vec![], vec![], vec![egui::Event::PointerMoved(at), ev(true)], vec![ev(false)], vec![egui::Event::PointerMoved(hover)], vec![], vec![], vec![]];
        let fonts = app_fonts();
        let c = raster::render_input(size, 1.0, Some(&fonts), passes, |ctx| app.frame_ui(ctx));
        let rgba: Vec<u8> = c.px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        std::fs::write(path, util::png_rgba(c.w as u32, c.h as u32, &rgba)).unwrap();
    }
}
