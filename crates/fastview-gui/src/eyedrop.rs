//! «Выбор...» of the colour dialog: pick a colour off the screen.
//!
//! The original (CMFCColorDialog's pick mode, `mfc120u` 0x75d64 – 0x75fde;
//! L9.258, L9.337): the button brings the dialog to the front, captures the
//! mouse on it and sets the picker cursor, then runs its own message loop.
//! Each mouse move reads the screen pixel under the pointer (a screen DC,
//! GetPixel) and makes it the new colour — the swatch and the hexagon's ring
//! follow the pointer. The left button's release ends the mode keeping the
//! colour; so do the right and middle buttons' press and Enter; Esc ends it
//! and puts back the colour the dialog had when the mode began; every other
//! key is ignored; the capture is released at the end. The click that ends
//! the pick therefore reaches no other window.
//!
//! Ours takes the mouse capture on our window through `ScreenSource` (so the
//! pointer's buttons and moves arrive from anywhere on the screen and the
//! cross shows everywhere), reads the ending clicks and keys from the
//! window's own input events (`input_events`: a quick click or a late key is
//! never missed), and reads the pixel under the pointer through the same
//! trait, so a test can feed it. Where the platform has no reliable screen
//! read (everything but Windows) the button stays greyed.

use eframe::egui::{self, Color32};

/// What ends the pick mode, from the window's input events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ev {
    /// The left (primary) button's release.
    LeftUp,
    /// The right or the middle button's press.
    OtherDown,
    Enter,
    Esc,
}

/// The mode-ending events among `events`, in order (every other event,
/// every other key, is ignored: the original's loop drops them).
pub fn input_events(events: &[egui::Event]) -> Vec<Ev> {
    use egui::{Event, Key, PointerButton};
    events
        .iter()
        .filter_map(|e| match e {
            Event::PointerButton { button: PointerButton::Primary, pressed: false, .. } => Some(Ev::LeftUp),
            Event::PointerButton { button: PointerButton::Secondary | PointerButton::Middle, pressed: true, .. } => Some(Ev::OtherDown),
            Event::Key { key: Key::Enter, pressed: true, .. } => Some(Ev::Enter),
            Event::Key { key: Key::Escape, pressed: true, .. } => Some(Ev::Esc),
            _ => None,
        })
        .collect()
}

/// The screen and the mouse capture, as the picker sees them.
pub trait ScreenSource {
    /// Whether this platform lets the program read the screen.
    fn available(&self) -> bool;
    /// The colour of the screen pixel under the pointer; `None` when it
    /// cannot be read (GetPixel's CLR_INVALID, a failed read).
    fn pixel(&mut self) -> Option<Color32>;
    /// Takes the mouse capture on our window (the mode began).
    fn capture(&mut self) {}
    /// Gives the capture back (the mode ended, however).
    fn release(&mut self) {}
    /// Called every frame of the mode: takes the capture again if winit's
    /// own release at a button's up dropped it before the up reached us.
    fn keep_capture(&mut self) {}
    /// True when the capture was taken and another window has it now.
    fn capture_lost(&self) -> bool {
        false
    }
    /// Shows the picker's cursor (the system's cross; the original's own
    /// picker artwork is not reused) wherever the pointer is.
    fn show_cursor(&mut self) {}
}

/// How a frame of picking ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Still picking; the new colour is this.
    Preview(Color32),
    /// Picking ended; the new colour is this.
    Done(Color32),
    /// Picking was cancelled; the colour the dialog began with.
    Restore(Color32),
}

/// The pick mode's state.
#[derive(Clone, Copy)]
pub struct Eyedrop {
    start: Color32,
}

impl Eyedrop {
    /// Begins the mode over a dialog whose new colour is `start`.
    pub fn new(start: Color32) -> Self {
        Eyedrop { start }
    }

    /// One frame: the ending `events` of the frame in order, the `pixel`
    /// under the pointer now, `current` the dialog's new colour, `lost` the
    /// capture's loss. The first ending event decides; with none, the colour
    /// follows the pointer.
    pub fn step(&mut self, events: &[Ev], pixel: Option<Color32>, current: Color32, lost: bool) -> Step {
        let now = pixel.unwrap_or(current);
        match events.first() {
            Some(Ev::Esc) => Step::Restore(self.start),
            Some(_) => Step::Done(now),
            // Something took the mouse away (another window): the mode
            // cannot go on; cancelled as Esc does.
            None if lost => Step::Restore(self.start),
            None => Step::Preview(now),
        }
    }
}

/// The platform's screen source.
pub fn system() -> Box<dyn ScreenSource> {
    #[cfg(windows)]
    {
        Box::new(win::WinScreen::default())
    }
    #[cfg(not(windows))]
    {
        Box::new(NoScreen)
    }
}

/// No screen read (Wayland forbids it; no reliable read elsewhere here).
#[cfg(not(windows))]
struct NoScreen;

#[cfg(not(windows))]
impl ScreenSource for NoScreen {
    fn available(&self) -> bool {
        false
    }
    fn pixel(&mut self) -> Option<Color32> {
        None
    }
}

#[cfg(windows)]
mod win {
    use super::ScreenSource;
    use eframe::egui::Color32;
    use windows_sys::Win32::Foundation::{HWND, POINT};
    use windows_sys::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC, CLR_INVALID};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetActiveWindow, GetCapture, ReleaseCapture, SetCapture};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetSystemMetrics, LoadCursorW, RegisterClassW, SetCursor, ShowWindow, IDC_CROSS, SM_CXVIRTUALSCREEN,
        SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    /// Whether the capture, now held by `current` (0: nobody), counts as
    /// taken from us (`held`: our frame). Nobody holding it is not a loss:
    /// winit releases the capture itself when a button goes up, before the
    /// up reaches our passes.
    pub fn lost(held: isize, current: isize) -> bool {
        held != 0 && current != 0 && current != held
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The popup's class: no background, no painting, the cross as its class
    /// cursor (MFC's helper window of the pick mode, 0x7602a: its class holds
    /// the picker cursor).
    fn register() -> Vec<u16> {
        let name = wide("FastViewEyedropShield");
        // SAFETY: a class of ours with the default window procedure; a second
        // registration just fails, which is fine.
        unsafe {
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(DefWindowProcW),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: GetModuleHandleW(std::ptr::null()),
                hIcon: std::ptr::null_mut(),
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_CROSS),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: name.as_ptr(),
            };
            RegisterClassW(&wc);
        }
        name
    }

    /// GDI's screen DC, the mouse capture on our frame and MFC's helper
    /// popup over the screen.
    #[derive(Default)]
    pub struct WinScreen {
        /// The window holding our capture (0: none taken).
        held: isize,
        /// The popup of our thread over the whole virtual screen (0: none).
        shield: isize,
    }

    impl ScreenSource for WinScreen {
        fn available(&self) -> bool {
            true
        }

        fn pixel(&mut self) -> Option<Color32> {
            let mut pt = POINT { x: 0, y: 0 };
            // SAFETY: plain user32 / GDI calls on the screen DC, released at once.
            unsafe {
                if GetCursorPos(&mut pt) == 0 {
                    return None;
                }
                let dc = GetDC(std::ptr::null_mut());
                if dc.is_null() {
                    return None;
                }
                let c = GetPixel(dc, pt.x, pt.y);
                ReleaseDC(std::ptr::null_mut(), dc);
                // COLORREF 0x00BBGGRR.
                (c != CLR_INVALID).then(|| Color32::from_rgb(c as u8, (c >> 8) as u8, (c >> 16) as u8))
            }
        }

        fn capture(&mut self) {
            // SAFETY: our own frame (or this thread's active window) and a
            // popup of ours. SetCapture takes a press that began in another
            // program's window only while the pointer is over a window of this
            // thread: the popup, topmost over the whole screen (the taskbar
            // too), makes every press one, and the capture hands it to the
            // frame — MFC's way (0x75da5 – 0x75ddc).
            unsafe {
                let hwnd: HWND = crate::minsize::frame().unwrap_or_else(|| GetActiveWindow());
                if hwnd.is_null() {
                    return;
                }
                if self.shield == 0 {
                    let class = register();
                    let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
                    let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
                    let pop = CreateWindowExW(
                        WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                        class.as_ptr(),
                        std::ptr::null(),
                        WS_POPUP,
                        x,
                        y,
                        w,
                        h,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        GetModuleHandleW(std::ptr::null()),
                        std::ptr::null(),
                    );
                    if !pop.is_null() {
                        ShowWindow(pop, SW_SHOWNOACTIVATE);
                        self.shield = pop as isize;
                    }
                }
                SetCapture(hwnd);
                self.held = hwnd as isize;
            }
        }

        fn release(&mut self) {
            // SAFETY: only the capture that is still ours; our own popup.
            unsafe {
                if self.held != 0 {
                    if GetCapture() as isize == self.held {
                        ReleaseCapture();
                    }
                    self.held = 0;
                }
                if self.shield != 0 {
                    DestroyWindow(self.shield as HWND);
                    self.shield = 0;
                }
            }
        }

        fn keep_capture(&mut self) {
            // SAFETY: nobody holds the capture: take it again.
            unsafe {
                if self.held != 0 && GetCapture().is_null() {
                    SetCapture(self.held as HWND);
                }
            }
        }

        fn capture_lost(&self) -> bool {
            // SAFETY: a query.
            lost(self.held, unsafe { GetCapture() } as isize)
        }

        fn show_cursor(&mut self) {
            // SAFETY: a system cursor.
            unsafe {
                SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_CROSS));
            }
        }
    }

    impl Drop for WinScreen {
        fn drop(&mut self) {
            self.release();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(r: u8, g: u8, b: u8) -> Option<Color32> {
        Some(Color32::from_rgb(r, g, b))
    }

    fn key(key: egui::Key, pressed: bool) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE }
    }

    fn button(button: egui::PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos: egui::Pos2::ZERO, button, pressed, modifiers: egui::Modifiers::NONE }
    }

    #[test]
    fn preview_follows_the_pixel_under_the_pointer() {
        let mut e = Eyedrop::new(Color32::BLACK);
        assert_eq!(e.step(&[], rgb(1, 2, 3), Color32::BLACK, false), Step::Preview(Color32::from_rgb(1, 2, 3)));
        // An unreadable pixel leaves the colour as it is.
        assert_eq!(e.step(&[], None, Color32::from_rgb(9, 8, 7), false), Step::Preview(Color32::from_rgb(9, 8, 7)));
    }

    #[test]
    fn left_release_enter_and_other_buttons_keep_the_pixel() {
        for ev in [Ev::LeftUp, Ev::Enter, Ev::OtherDown] {
            let mut e = Eyedrop::new(Color32::BLACK);
            assert_eq!(e.step(&[ev], rgb(12, 22, 32), Color32::BLACK, false), Step::Done(Color32::from_rgb(12, 22, 32)), "{ev:?}");
            assert_eq!(e.step(&[ev], None, Color32::from_rgb(5, 5, 5), false), Step::Done(Color32::from_rgb(5, 5, 5)), "{ev:?} unreadable");
        }
    }

    #[test]
    fn escape_puts_the_starting_colour_back() {
        let start = Color32::from_rgb(40, 50, 60);
        let mut e = Eyedrop::new(start);
        assert_eq!(e.step(&[Ev::Esc], rgb(1, 1, 1), Color32::from_rgb(1, 1, 1), false), Step::Restore(start));
    }

    #[test]
    fn the_first_ending_event_of_a_frame_decides() {
        let start = Color32::from_rgb(40, 50, 60);
        let mut e = Eyedrop::new(start);
        assert_eq!(e.step(&[Ev::Esc, Ev::LeftUp], rgb(1, 1, 1), start, false), Step::Restore(start));
        assert_eq!(e.step(&[Ev::LeftUp, Ev::Esc], rgb(1, 1, 1), start, false), Step::Done(Color32::from_rgb(1, 1, 1)));
    }

    #[test]
    fn a_lost_capture_cancels_unless_an_event_ended_it() {
        let start = Color32::from_rgb(40, 50, 60);
        let mut e = Eyedrop::new(start);
        assert_eq!(e.step(&[], rgb(1, 1, 1), start, true), Step::Restore(start));
        // The left release comes with winit's own release of the capture.
        assert_eq!(e.step(&[Ev::LeftUp], rgb(1, 1, 1), start, true), Step::Done(Color32::from_rgb(1, 1, 1)));
    }

    #[test]
    fn only_the_ending_events_are_read_from_the_input() {
        use egui::{Key, PointerButton as B};
        // A press and its release in one frame are both there (a quick tap).
        let tap = [button(B::Primary, true), button(B::Primary, false)];
        assert_eq!(input_events(&tap), vec![Ev::LeftUp]);
        let evs = [
            egui::Event::PointerMoved(egui::Pos2::ZERO),
            key(Key::A, true),
            key(Key::Tab, true),
            key(Key::Enter, false),
            button(B::Secondary, true),
            button(B::Secondary, false),
            button(B::Middle, true),
            key(Key::Escape, true),
        ];
        assert_eq!(input_events(&evs), vec![Ev::OtherDown, Ev::OtherDown, Ev::Esc]);
        assert_eq!(input_events(&[key(Key::Enter, true)]), vec![Ev::Enter]);
    }

    /// Nobody holding the capture (winit let go at the button's up) is not a
    /// loss; another window holding it is.
    #[cfg(windows)]
    #[test]
    fn windows_capture_loss_is_another_holder_only() {
        assert!(!win::lost(10, 10));
        assert!(!win::lost(10, 0), "winit's own release at a button's up");
        assert!(win::lost(10, 20));
        assert!(!win::lost(0, 20), "never taken");
    }

    /// The real screen read (Windows): a desktop pixel comes back. Needs an
    /// interactive desktop: on a locked session or a service account GetPixel
    /// answers CLR_INVALID and this fails for a reason that is not ours.
    #[cfg(windows)]
    #[test]
    fn windows_reads_a_screen_pixel() {
        let mut src = system();
        assert!(src.available());
        assert!(src.pixel().is_some(), "GetPixel on the screen DC read nothing");
        // Giving back a capture never taken is harmless.
        src.release();
        assert!(!src.capture_lost());
    }
}
