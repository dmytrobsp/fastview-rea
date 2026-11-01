//! System dialogs (file open / save / folder) outside the egui frame (#190).
//!
//! winit 0.30's WM_PAINT handler re-requests a paint after every paint while
//! it is inside a handler. A system dialog opened from our frame runs a modal
//! loop inside that handler: the loop never goes idle, the UI thread spins
//! repainting under the dialog (and a dialog that shows itself on idle never
//! appears — «Свойства...», 0.7.0). So a dialog is run from a message posted to
//! the frame, after the frame has returned (on Windows), owned by our active window;
//! the caller leaves a continuation that [`App::poll_sysdlg`] runs, in a later
//! frame, with the choice. Where the platform has no such trouble (not Windows)
//! the dialog runs where it is asked and the choice is delivered the same way,
//! so every caller has one path.

use crate::app::App;
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

/// A chosen file and, where the dialog reports it, the chosen file type (0-based).
pub type Picked = Option<(PathBuf, Option<usize>)>;

/// What a dialog is asked to be.
#[cfg_attr(test, allow(dead_code))]
pub enum Job {
    Open(rfd::FileDialog),
    Save(rfd::FileDialog),
    Folder(rfd::FileDialog),
    /// Another system call that shows dialogs of its own (the shell's delete): answers `Some` when it returns true.
    Task(Box<dyn FnOnce() -> bool + Send>),
    /// A save dialog that reports the chosen file type: `filters` are (description, extension).
    SaveTyped { filters: Vec<(String, String)>, dir: Option<String>, name: String, owner: isize },
}

/// The dialog in progress and what runs when it ends.
type Then = Box<dyn FnOnce(&mut App, &egui::Context, Picked)>;

pub struct Pending {
    rx: Receiver<Picked>,
    then: Then,
}

/// The context to wake when a dialog ends (set by the frame).
static WAKE: std::sync::Mutex<Option<egui::Context>> = std::sync::Mutex::new(None);

pub fn set_wake(ctx: &egui::Context) {
    if let Ok(mut w) = WAKE.lock() {
        if w.is_none() {
            *w = Some(ctx.clone());
        }
    }
}

fn wake() {
    if let Some(c) = WAKE.lock().ok().and_then(|w| w.clone()) {
        c.request_repaint();
    }
}

/// The window that owns a dialog, taken where the command runs (0 = none).
pub fn owner() -> isize {
    #[cfg(windows)]
    {
        crate::dialogs::Owner::active().map_or(0, |o| o.hwnd() as isize)
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Show `job` and wait for it (on whichever thread this is).
#[cfg_attr(test, allow(dead_code))]
pub fn run(job: Job) -> Picked {
    match job {
        Job::Open(d) => d.pick_file().map(|p| (p, None)),
        Job::Save(d) => d.save_file().map(|p| (p, None)),
        Job::Folder(d) => d.pick_folder().map(|p| (p, None)),
        Job::Task(f) => f().then(|| (PathBuf::new(), None)),
        Job::SaveTyped { filters, dir, name, owner } => save_typed(&filters, dir, &name, owner),
    }
}

/// comdlg32's save dialog on Windows, which reports the chosen file type
/// (OPENFILENAME's nFilterIndex); elsewhere rfd's, which does not.
#[cfg_attr(test, allow(dead_code))]
fn save_typed(filters: &[(String, String)], dir: Option<String>, name: &str, owner: isize) -> Picked {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OFN_EXPLORER, OFN_HIDEREADONLY, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW};
        let wide = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        let mut filter: Vec<u16> = Vec::new();
        for (desc, ext) in filters {
            filter.extend(wide(desc));
            filter.push(0);
            filter.extend(wide(&format!("*.{ext}")));
            filter.push(0);
        }
        filter.push(0);
        let mut file = vec![0u16; 1024];
        let n = wide(name);
        file[..n.len().min(1023)].copy_from_slice(&n[..n.len().min(1023)]);
        let dir_w: Option<Vec<u16>> = dir.map(|d| wide(&d).into_iter().chain(Some(0)).collect());
        // SAFETY: every pointer outlives the call; the file buffer's size is
        // given.
        unsafe {
            let mut ofn: OPENFILENAMEW = std::mem::zeroed();
            ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
            // Owned by the frame (`dialogs::file_dialog`).
            ofn.hwndOwner = owner as *mut std::ffi::c_void;
            ofn.lpstrFilter = filter.as_ptr();
            ofn.nFilterIndex = 1;
            ofn.lpstrFile = file.as_mut_ptr();
            ofn.nMaxFile = file.len() as u32;
            ofn.lpstrInitialDir = dir_w.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
            ofn.Flags = OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_NOCHANGEDIR;
            if GetSaveFileNameW(&mut ofn) == 0 {
                return None;
            }
            let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
            Some((PathBuf::from(String::from_utf16_lossy(&file[..len])), Some(ofn.nFilterIndex.max(1) as usize - 1)))
        }
    }
    #[cfg(not(windows))]
    {
        let _ = owner;
        let mut dlg = crate::dialogs::file_dialog();
        for (desc, ext) in filters {
            dlg = dlg.add_filter(desc.as_str(), &[ext.as_str()]);
        }
        if let Some(d) = dir {
            dlg = dlg.set_directory(d);
        }
        dlg.set_file_name(name).save_file().map(|p| (p, None))
    }
}

// Dialogs waiting for the UI thread's next turn (Windows).
#[cfg(windows)]
thread_local! {
    static QUEUE: std::cell::RefCell<std::collections::VecDeque<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

/// The posted message's turn (the frame's subclass, [`crate::minsize`]): runs the queued dialogs.
/// Here the thread is in its message loop, not inside winit's event handler, so the dialog's
/// modal loop dispatches paints as any window's would and the loop goes idle.
#[cfg(windows)]
pub fn run_queued() {
    while let Some(job) = QUEUE.with(|q| q.borrow_mut().pop_front()) {
        job();
    }
}

/// Where the dialog runs. Windows: on the UI thread, but from a message posted to the frame, so
/// outside winit's handler — a dialog run on a worker thread owned by our window made its keyboard
/// input crawl (clone, SendKeys into the file name). Tests: a thread. Elsewhere: here.
fn start(runner: Box<dyn FnOnce() -> Picked + Send>) -> Receiver<Picked> {
    let (tx, rx) = channel();
    #[cfg(all(windows, not(test)))]
    {
        QUEUE.with(|q| {
            q.borrow_mut().push_back(Box::new(move || {
                let _ = tx.send(runner());
                wake();
            }))
        });
        if !crate::minsize::post_run() {
            // No frame to post to: asked now, here.
            run_queued();
        }
        rx
    }
    #[cfg(test)]
    {
        std::thread::spawn(move || {
            let _ = tx.send(runner());
            wake();
        });
        rx
    }
    #[cfg(all(not(windows), not(test)))]
    {
        let _ = tx.send(runner());
        wake();
        rx
    }
}

#[cfg(test)]
pub mod testing {
    //! Tests never show a real dialog: `pick` hands the job to this gate.
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// What the next dialog answers once the test lets it (process-wide: only a test that holds `gate()`
    /// may reach `pick`; the others get the cancel).
    pub static ANSWER: Mutex<Option<std::sync::mpsc::Receiver<Picked>>> = Mutex::new(None);
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// Serialises the tests that use the gate; the next dialog waits for `send`.
    pub fn gate() -> (MutexGuard<'static, ()>, std::sync::mpsc::Sender<Picked>) {
        let g = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let (tx, rx) = channel();
        *ANSWER.lock().unwrap() = Some(rx);
        (g, tx)
    }
}

impl App {
    /// Ask for a system dialog; `then` runs in a later frame with the choice
    /// (`None`: cancelled). One dialog at a time: a second request while one
    /// is up is dropped (the first holds the window's input).
    pub fn pick(&mut self, job: Job, then: impl FnOnce(&mut App, &egui::Context, Picked) + 'static) {
        if self.sysdlg.is_some() {
            return;
        }
        #[cfg(test)]
        let runner: Box<dyn FnOnce() -> Picked + Send> = {
            drop(job);
            let rx = testing::ANSWER.lock().unwrap().take();
            // A timeout answers as a cancel: a test that blocks on the dialog fails on its assertions, it does not hang CI.
            Box::new(move || rx.and_then(|rx| rx.recv_timeout(std::time::Duration::from_secs(10)).ok()).flatten())
        };
        #[cfg(not(test))]
        let runner: Box<dyn FnOnce() -> Picked + Send> = Box::new(move || run(job));
        self.sysdlg = Some(Pending { rx: start(runner), then: Box::new(then) });
    }

    /// A system dialog is up (its worker has not answered yet).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn sysdlg_busy(&self) -> bool {
        self.sysdlg.is_some()
    }

    /// Called every frame: runs the continuation of a dialog that ended.
    pub fn poll_sysdlg(&mut self, ctx: &egui::Context) {
        let Some(p) = self.sysdlg.take() else { return };
        match p.rx.try_recv() {
            Ok(r) => (p.then)(self, ctx, r),
            // The worker died without an answer: as a cancel.
            Err(std::sync::mpsc::TryRecvError::Disconnected) => (p.then)(self, ctx, None),
            Err(std::sync::mpsc::TryRecvError::Empty) => self.sysdlg = Some(p),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Command, Doc};

    fn poll_until(app: &mut App, ctx: &egui::Context, what: &str, done: impl Fn(&App) -> bool) {
        for _ in 0..400 {
            app.poll_sysdlg(ctx);
            app.poll_loading();
            if done(app) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("timed out: {what}");
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fv_sysdlg_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A command that asks for a system file dialog returns at once (the dialog is run outside the
    /// frame, #190) and does its work in a later frame, once the dialog has answered; a second dialog
    /// asked for meanwhile is dropped.
    #[test]
    fn a_file_dialog_does_not_block_the_frame_that_asks_for_it() {
        let (_guard, answer) = testing::gate();
        let ctx = egui::Context::default();
        let dir = temp_dir("open");
        let cfg = dir.join("rec.cfg");
        std::fs::write(&cfg, crate::app::DEMO_CFG).unwrap();
        std::fs::write(dir.join("rec.dat"), crate::app::DEMO_DAT).unwrap();
        let mut app = App::new();
        app.persist = false;
        app.dispatch(Command::Open);
        assert!(app.sysdlg_busy(), "the dialog is up");
        assert!(app.docs.is_empty(), "nothing opened before the dialog answered");
        app.poll_sysdlg(&ctx);
        assert!(app.sysdlg_busy() && app.docs.is_empty(), "frames while it is up wait");
        app.dispatch(Command::OpenFvb);
        answer.send(Some((cfg, None))).unwrap();
        poll_until(&mut app, &ctx, "the record opens", |a| !a.docs.is_empty());
        assert!(!app.sysdlg_busy());
        assert_eq!(app.docs.len(), 1, "the dropped request opened nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A cancelled dialog does nothing.
    #[test]
    fn a_cancelled_file_dialog_does_nothing() {
        let (_guard, answer) = testing::gate();
        let ctx = egui::Context::default();
        let mut app = App::new();
        app.persist = false;
        app.dispatch(Command::Demo);
        let n = app.docs.len();
        app.dispatch(Command::ExportCsv);
        assert!(app.sysdlg_busy());
        answer.send(None).unwrap();
        poll_until(&mut app, &ctx, "the dialog ends", |a| !a.sysdlg_busy());
        assert_eq!(app.docs.len(), n);
        assert!(!app.status.starts_with("Экспортировано"));
    }

    /// «Да» to the save question of a record with no file asks for one; the window closes only once
    /// the dialog has answered with a file, and not at all when it is cancelled.
    #[test]
    fn closing_after_a_save_as_waits_for_the_dialog() {
        let ctx = egui::Context::default();
        let dir = temp_dir("close");
        let mut app = App::new();
        app.persist = false;
        app.dispatch(Command::Demo);
        let osc = fvb_core::comtrade::parse(crate::app::DEMO_CFG, crate::app::DEMO_DAT).unwrap();
        app.add_doc(Doc::new(osc, "synthetic".into()));
        let i = app.docs.len() - 1;
        assert!(app.docs[i].path.is_none());
        app.docs[i].set.modified = true;
        let before = app.docs.len();
        for (n, save) in [false, true].into_iter().enumerate() {
            let (_guard, answer) = testing::gate();
            app.request_close(i);
            assert!(app.setfile.prompt.is_some());
            app.answer_save_prompt(&ctx, Some(true));
            assert!(app.sysdlg_busy(), "the Save As dialog is up");
            assert_eq!(app.docs.len(), before, "still open while the dialog is up");
            answer.send(save.then(|| (dir.join("saved.cfg"), Some(0)))).unwrap();
            poll_until(&mut app, &ctx, "the dialog answers", |a| !a.sysdlg_busy());
            assert_eq!(app.docs.len(), if save { before - 1 } else { before }, "round {n}");
        }
        assert!(dir.join("saved.cfg").exists(), "the record was written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// «Закрыть окна» with «Да» on a record that needs Save As: the frame painted while the dialog is
    /// up must not go on closing the next selected window, and the answer closes the record the
    /// question was about (by serial), not whatever the index now names.
    #[test]
    fn closing_windows_waits_for_a_save_as_and_closes_the_right_ones() {
        let ctx = egui::Context::default();
        let dir = temp_dir("windows");
        let mut app = App::new();
        app.persist = false;
        app.dispatch(Command::Demo);
        let osc = fvb_core::comtrade::parse(crate::app::DEMO_CFG, crate::app::DEMO_DAT).unwrap();
        for n in ["a", "b", "c"] {
            app.add_doc(Doc::new(osc.clone(), n.into()));
        }
        let serial = |app: &App, name: &str| app.docs.iter().find(|d| d.name == name).unwrap().serial;
        let (a, b) = (serial(&app, "a"), serial(&app, "b"));
        app.docs.iter_mut().find(|d| d.serial == b).unwrap().set.modified = true;
        app.open_windows_dialog();
        {
            let dlg = app.windows_dlg.as_mut().unwrap();
            dlg.closing = vec![b, a];
            dlg.refresh = true;
        }
        let frame = |app: &mut App| {
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.windows_dialog(ctx));
        };
        let (_guard, answer) = testing::gate();
        frame(&mut app);
        assert!(app.setfile.prompt.is_some(), "b asks first");
        app.answer_save_prompt(&ctx, Some(true));
        assert!(app.sysdlg_busy());
        frame(&mut app);
        frame(&mut app);
        assert_eq!(app.docs.len(), 4, "nothing closed while the dialog is up");
        answer.send(Some((dir.join("b.cfg"), Some(0)))).unwrap();
        poll_until(&mut app, &ctx, "the dialog answers", |a| !a.sysdlg_busy());
        frame(&mut app);
        frame(&mut app);
        let names: Vec<&str> = app.docs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["demo_fault", "c"], "b (saved) and a closed, c kept");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
