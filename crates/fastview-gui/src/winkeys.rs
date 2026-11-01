//! Keys egui-winit turns into something else before the app sees them.
//!
//! On Windows egui-winit 0.29 answers Ctrl+C / Ctrl+Insert with
//! `Event::Copy`, Ctrl+V / Shift+Insert with `Event::Paste` (only with text
//! on the OS clipboard) and sends no key event for them. The original's one
//! accelerator table has Ctrl+C and Ctrl+V and no Insert entries (L9.282), and
//! its Ctrl+V acts at the key press whatever the clipboard holds (L9.130).
//! The main window's subclass (`minsize`) notes those key presses as the
//! messages arrive, and [`filter`] corrects the pass's input: an Insert's
//! copy / paste is dropped, a Ctrl+V that came as no event is a key press.

#![cfg_attr(not(windows), allow(dead_code))]

use eframe::egui;
use std::sync::atomic::{AtomicU8, Ordering};

/// What the messages since the last pass showed.
static SEEN: AtomicU8 = AtomicU8::new(0);
const PASTE_KEY: u8 = 1;
const CTRL_INSERT: u8 = 2;
const SHIFT_INSERT: u8 = 4;

/// A key-down message (`WM_KEYDOWN` / `WM_SYSKEYDOWN`): the virtual key, the
/// message's repeat flag, and the modifiers held.
pub fn note_key_down(vk: u32, repeat: bool, ctrl: bool, shift: bool, alt: bool) {
    const VK_INSERT: u32 = 0x2D;
    const VK_V: u32 = 0x56;
    let bit = match vk {
        VK_V if ctrl && !shift && !alt && !repeat => PASTE_KEY,
        VK_INSERT if ctrl && !shift && !alt => CTRL_INSERT,
        VK_INSERT if shift && !ctrl && !alt => SHIFT_INSERT,
        _ => return,
    };
    SEEN.fetch_or(bit, Ordering::Relaxed);
}

/// The pass's raw input corrected by the key presses noted since the last.
pub fn filter(raw: &mut egui::RawInput) {
    apply(raw, SEEN.swap(0, Ordering::Relaxed));
}

fn apply(raw: &mut egui::RawInput, seen: u8) {
    if seen & CTRL_INSERT != 0 {
        if let Some(at) = raw.events.iter().position(|e| matches!(e, egui::Event::Copy)) {
            raw.events.remove(at);
        }
    }
    if seen & SHIFT_INSERT != 0 {
        if let Some(at) = raw.events.iter().position(|e| matches!(e, egui::Event::Paste(_))) {
            raw.events.remove(at);
        }
    }
    // Ctrl+V with no text on the clipboard comes as no event at all.
    if seen & PASTE_KEY != 0 && !raw.events.iter().any(|e| matches!(e, egui::Event::Paste(_))) {
        let modifiers = egui::Modifiers { ctrl: true, command: true, ..Default::default() };
        raw.events.push(egui::Event::Key { key: egui::Key::V, physical_key: Some(egui::Key::V), pressed: true, repeat: false, modifiers });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput { events, ..Default::default() }
    }

    /// Insert's copy and paste are dropped (the original has no such
    /// entries); a Ctrl+C or Ctrl+V beside them stays (#26).
    #[test]
    fn an_inserts_copy_and_paste_are_dropped() {
        let mut r = raw(vec![egui::Event::Copy]);
        apply(&mut r, CTRL_INSERT);
        assert!(r.events.is_empty(), "Ctrl+Insert");
        let mut r = raw(vec![egui::Event::Paste("t".into())]);
        apply(&mut r, SHIFT_INSERT);
        assert!(r.events.is_empty(), "Shift+Insert");
        let mut r = raw(vec![egui::Event::Copy]);
        apply(&mut r, 0);
        assert_eq!(r.events.len(), 1, "Ctrl+C");
        let mut r = raw(vec![egui::Event::Paste("t".into())]);
        apply(&mut r, 0);
        assert_eq!(r.events.len(), 1, "Ctrl+V");
    }

    /// Ctrl+V with nothing for egui to paste is a key press when it is
    /// pressed, not at its release (#26).
    #[test]
    fn ctrl_v_without_text_is_a_key_press() {
        let mut r = raw(vec![]);
        apply(&mut r, PASTE_KEY);
        assert!(matches!(r.events[..], [egui::Event::Key { key: egui::Key::V, pressed: true, .. }]));
        let mut r = raw(vec![egui::Event::Paste("t".into())]);
        apply(&mut r, PASTE_KEY);
        assert_eq!(r.events.len(), 1, "a paste egui announced is not doubled");
    }

    #[test]
    fn only_the_accelerators_are_noted() {
        SEEN.store(0, Ordering::Relaxed);
        note_key_down(0x56, false, true, true, false);
        note_key_down(0x56, true, true, false, false);
        note_key_down(0x2D, false, true, true, false);
        assert_eq!(SEEN.load(Ordering::Relaxed), 0);
        note_key_down(0x56, false, true, false, false);
        note_key_down(0x2D, false, false, true, false);
        assert_eq!(SEEN.swap(0, Ordering::Relaxed), PASTE_KEY | SHIFT_INSERT);
    }
}
