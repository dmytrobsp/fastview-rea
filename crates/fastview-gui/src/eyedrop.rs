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
    /// True when the capture was taken and something else has it now.
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
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetActiveWindow, GetCapture, ReleaseCapture, SetCapture};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, LoadCursorW, SetCursor, IDC_CROSS};

    /// GDI's screen DC and the mouse capture on our frame.
    #[derive(Default)]
    pub struct WinScreen {
        /// The window holding our capture (0: none taken).
        held: isize,
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
            // SAFETY: our own frame (or this thread's active window).
            unsafe {
                let hwnd: HWND = crate::minsize::frame().unwrap_or_else(|| GetActiveWindow());
                if !hwnd.is_null() {
                    SetCapture(hwnd);
                    self.held = hwnd as isize;
                }
            }
        }

        fn release(&mut self) {
            if self.held != 0 {
                // SAFETY: only when the capture is still ours.
                unsafe {
                    if GetCapture() as isize == self.held {
                        ReleaseCapture();
                    }
                }
                self.held = 0;
            }
        }

        fn capture_lost(&self) -> bool {
            // SAFETY: a query.
            self.held != 0 && unsafe { GetCapture() as isize != self.held }
        }

        fn show_cursor(&mut self) {
            // SAFETY: a system cursor.
            unsafe {
                SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_CROSS));
            }
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
