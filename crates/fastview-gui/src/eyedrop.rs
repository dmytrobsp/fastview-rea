//! «Выбор...» of the colour dialog: pick a colour off the screen.
//!
//! The original (CMFCColorDialog's pick mode, `mfc120u` 0x75d64 – 0x75fde;
//! L9.258, L9.337): the button starts a modal message loop with the
//! mouse captured and the picker cursor set. Each mouse move reads the screen
//! pixel under the pointer (the screen DC, GetPixel) and makes it the new
//! colour — the swatch and the hexagon's ring follow the pointer. The left
//! button's release ends the mode keeping the colour; so do the right and
//! middle buttons' press and Enter; Esc ends it and puts back the colour the
//! dialog had when the mode began.
//!
//! Ours polls the pointer, the buttons and the keys once per frame through
//! `ScreenSource` (a trait, so a test can feed it). Where the platform has no
//! reliable screen read (everything but Windows) the button stays greyed.

use eframe::egui::Color32;

/// What one poll of the screen and the input devices found.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Sample {
    /// The colour of the screen pixel under the pointer; `None` when it
    /// cannot be read (GetPixel's CLR_INVALID, a failed read).
    pub pixel: Option<Color32>,
    pub left: bool,
    /// The right or the middle button.
    pub other_button: bool,
    pub enter: bool,
    pub esc: bool,
}

/// The screen and the input devices, as the picker sees them.
pub trait ScreenSource {
    /// Whether this platform lets the program read the screen.
    fn available(&self) -> bool;
    /// The state now.
    fn sample(&mut self) -> Sample;
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
    left_was_down: bool,
    /// A key or button held when the mode began (the press that started it)
    /// counts only after it was seen released.
    keys_armed: bool,
}

impl Eyedrop {
    /// Begins the mode over a dialog whose new colour is `start`.
    pub fn new(start: Color32) -> Self {
        Eyedrop { start, left_was_down: false, keys_armed: false }
    }

    /// One frame: `current` is the dialog's new colour now.
    pub fn step(&mut self, s: Sample, current: Color32) -> Step {
        if !(s.enter || s.esc || s.other_button) {
            self.keys_armed = true;
        }
        if self.keys_armed {
            if s.esc {
                return Step::Restore(self.start);
            }
            if s.enter || s.other_button {
                return Step::Done(current);
            }
        }
        // The release of the left button ends the mode, after the move that
        // brought the pointer there; a press seen before it is required, so
        // the release of the click that began the mode does not end it.
        if self.left_was_down && !s.left {
            return Step::Done(s.pixel.unwrap_or(current));
        }
        self.left_was_down |= s.left;
        Step::Preview(s.pixel.unwrap_or(current))
    }
}

/// The platform's screen source.
pub fn system() -> Box<dyn ScreenSource> {
    #[cfg(windows)]
    {
        Box::new(win::WinScreen)
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
    fn sample(&mut self) -> Sample {
        Sample::default()
    }
}

#[cfg(windows)]
mod win {
    use super::{Sample, ScreenSource};
    use eframe::egui::Color32;
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC, CLR_INVALID};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_RETURN};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, LoadCursorW, SetCursor, IDC_CROSS};

    /// GDI's screen DC and the async key state.
    pub struct WinScreen;

    fn down(vk: u16) -> bool {
        // The high bit: down now.
        unsafe { GetAsyncKeyState(i32::from(vk)) < 0 }
    }

    impl ScreenSource for WinScreen {
        fn available(&self) -> bool {
            true
        }

        fn sample(&mut self) -> Sample {
            let mut pt = POINT { x: 0, y: 0 };
            let pixel = unsafe {
                if GetCursorPos(&mut pt) == 0 {
                    None
                } else {
                    let dc = GetDC(std::ptr::null_mut());
                    if dc.is_null() {
                        None
                    } else {
                        let c = GetPixel(dc, pt.x, pt.y);
                        ReleaseDC(std::ptr::null_mut(), dc);
                        // COLORREF 0x00BBGGRR.
                        (c != CLR_INVALID).then(|| Color32::from_rgb(c as u8, (c >> 8) as u8, (c >> 16) as u8))
                    }
                }
            };
            Sample { pixel, left: down(VK_LBUTTON), other_button: down(VK_RBUTTON) || down(VK_MBUTTON), enter: down(VK_RETURN), esc: down(VK_ESCAPE) }
        }

        fn show_cursor(&mut self) {
            unsafe {
                SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_CROSS));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: u8, g: u8, b: u8) -> Sample {
        Sample { pixel: Some(Color32::from_rgb(r, g, b)), ..Sample::default() }
    }

    /// A screen that plays a script of samples.
    struct Script(std::vec::IntoIter<Sample>);
    impl ScreenSource for Script {
        fn available(&self) -> bool {
            true
        }
        fn sample(&mut self) -> Sample {
            self.0.next().unwrap_or_default()
        }
    }

    /// Runs the machine over a script from `start`; the colour after the
    /// last frame and how it ended (`None`: still picking).
    fn run(start: Color32, script: Vec<Sample>) -> (Color32, Option<Step>) {
        let mut src = Script(script.into_iter());
        let mut e = Eyedrop::new(start);
        let mut cur = start;
        for _ in 0..64 {
            match e.step(src.sample(), cur) {
                Step::Preview(c) => cur = c,
                end @ (Step::Done(c) | Step::Restore(c)) => return (c, Some(end)),
            }
        }
        (cur, None)
    }

    #[test]
    fn preview_follows_the_pixel_under_the_pointer() {
        let mut e = Eyedrop::new(Color32::BLACK);
        assert_eq!(e.step(px(1, 2, 3), Color32::BLACK), Step::Preview(Color32::from_rgb(1, 2, 3)));
        assert_eq!(e.step(px(9, 8, 7), Color32::from_rgb(1, 2, 3)), Step::Preview(Color32::from_rgb(9, 8, 7)));
        // An unreadable pixel leaves the colour as it is.
        assert_eq!(e.step(Sample::default(), Color32::from_rgb(9, 8, 7)), Step::Preview(Color32::from_rgb(9, 8, 7)));
    }

    #[test]
    fn left_release_after_a_press_keeps_the_colour_under_the_pointer() {
        let down = Sample { left: true, ..px(10, 20, 30) };
        let (c, end) = run(Color32::BLACK, vec![px(5, 5, 5), down, Sample { left: true, ..px(11, 21, 31) }, px(12, 22, 32)]);
        assert_eq!(end, Some(Step::Done(Color32::from_rgb(12, 22, 32))));
        assert_eq!(c, Color32::from_rgb(12, 22, 32));
    }

    #[test]
    fn the_release_of_the_starting_click_does_not_end_it() {
        // The mode begins on the button's click, i.e. after its release: no
        // press is seen first, so nothing ends until a new click's release.
        let (_, end) = run(Color32::BLACK, vec![px(1, 1, 1), px(2, 2, 2), px(3, 3, 3)]);
        assert_eq!(end, None);
    }

    #[test]
    fn escape_puts_the_starting_colour_back() {
        let start = Color32::from_rgb(40, 50, 60);
        let (c, end) = run(start, vec![px(1, 1, 1), px(2, 2, 2), Sample { esc: true, ..px(3, 3, 3) }]);
        assert_eq!(end, Some(Step::Restore(start)));
        assert_eq!(c, start);
    }

    #[test]
    fn enter_and_the_other_buttons_end_keeping_the_preview() {
        for end_key in [Sample { enter: true, ..Sample::default() }, Sample { other_button: true, ..Sample::default() }] {
            let (c, end) = run(Color32::BLACK, vec![px(1, 1, 1), px(2, 2, 2), end_key]);
            assert_eq!(end, Some(Step::Done(Color32::from_rgb(2, 2, 2))));
            assert_eq!(c, Color32::from_rgb(2, 2, 2));
        }
    }

    #[test]
    fn a_key_held_from_the_start_counts_only_after_its_release() {
        // Enter held (it pressed the button): ignored until released, and
        // then a fresh press ends.
        let held = Sample { enter: true, ..px(1, 1, 1) };
        let (_, end) = run(Color32::BLACK, vec![held, held, px(2, 2, 2), Sample { enter: true, ..px(3, 3, 3) }]);
        assert_eq!(end, Some(Step::Done(Color32::from_rgb(2, 2, 2))));
        let (_, end) = run(Color32::BLACK, vec![held, held, px(2, 2, 2)]);
        assert_eq!(end, None);
    }

    /// The real screen read (Windows): a desktop pixel comes back.
    #[cfg(windows)]
    #[test]
    fn windows_reads_a_screen_pixel() {
        let mut src = system();
        assert!(src.available());
        let s = src.sample();
        assert!(s.pixel.is_some(), "GetPixel on the screen DC read nothing");
    }
}
