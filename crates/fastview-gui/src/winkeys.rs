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

/// What the messages since the last pass showed: the presses of Ctrl+V
/// (not auto-repeats), of Ctrl+Insert and of Shift+Insert (repeats too: each
/// makes an event of its own).
static PASTE_KEYS: AtomicU8 = AtomicU8::new(0);
static CTRL_INSERTS: AtomicU8 = AtomicU8::new(0);
static SHIFT_INSERTS: AtomicU8 = AtomicU8::new(0);

/// The Insert-made events of the pass kept for a text edit (copy, paste):
/// the frame's own copy / paste leaves them alone.
pub const KEPT_FOR_EDIT: &str = "fv_insert_events_for_edit";

/// A key-down message (`WM_KEYDOWN` / `WM_SYSKEYDOWN`): the virtual key, the
/// message's repeat flag, and the modifiers held.
pub fn note_key_down(vk: u32, repeat: bool, ctrl: bool, shift: bool, alt: bool) {
    const VK_INSERT: u32 = 0x2D;
    const VK_V: u32 = 0x56;
    let count = match vk {
        VK_V if ctrl && !shift && !alt && !repeat => &PASTE_KEYS,
        VK_INSERT if ctrl && !shift && !alt => &CTRL_INSERTS,
        VK_INSERT if shift && !ctrl && !alt => &SHIFT_INSERTS,
        _ => return,
    };
    // A saturating count by compare-and-swap (`fetch_update` is renamed in newer toolchains).
    let mut n = count.load(Ordering::Relaxed);
    while n < u8::MAX {
        match count.compare_exchange_weak(n, n + 1, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(now) => n = now,
        }
    }
}

/// What the messages noted.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Seen {
    paste_keys: u8,
    ctrl_inserts: u8,
    shift_inserts: u8,
}

/// The pass's raw input corrected by the key presses noted since the last.
/// A text edit (a Win32 EDIT in the original) pastes on Shift+Insert and
/// copies on Ctrl+Insert itself: with a widget holding the keyboard focus (as of the
/// last pass — a text edit, as far as copy and paste go) the events stay for it, and `KEPT_FOR_EDIT` tells the frame's
/// copy / paste to ignore them; otherwise they are dropped (the original's
/// accelerator table has no Insert entries).
pub fn filter(ctx: &egui::Context, raw: &mut egui::RawInput) {
    let seen = Seen { paste_keys: PASTE_KEYS.swap(0, Ordering::Relaxed), ctrl_inserts: CTRL_INSERTS.swap(0, Ordering::Relaxed), shift_inserts: SHIFT_INSERTS.swap(0, Ordering::Relaxed) };
    let kept = apply(raw, seen, ctx.wants_keyboard_input());
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(KEPT_FOR_EDIT), kept));
}

/// Returns how many copy and paste events were left for an edit.
fn apply(raw: &mut egui::RawInput, seen: Seen, edit_has_keys: bool) -> (u8, u8) {
    let mut kept = (0, 0);
    let mut copies = seen.ctrl_inserts;
    let mut pastes = seen.shift_inserts;
    raw.events.retain(|e| match e {
        egui::Event::Copy if copies > 0 => {
            copies -= 1;
            if edit_has_keys {
                kept.0 += 1;
            }
            edit_has_keys
        }
        egui::Event::Paste(_) if pastes > 0 => {
            pastes -= 1;
            if edit_has_keys {
                kept.1 += 1;
            }
            edit_has_keys
        }
        _ => true,
    });
    // Ctrl+V with no text on the clipboard comes as no event at all.
    if seen.paste_keys > 0 && !raw.events.iter().any(|e| matches!(e, egui::Event::Paste(_))) {
        let modifiers = egui::Modifiers { ctrl: true, command: true, ..Default::default() };
        raw.events.push(egui::Event::Key { key: egui::Key::V, physical_key: Some(egui::Key::V), pressed: true, repeat: false, modifiers });
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput { events, ..Default::default() }
    }

    /// The tests share the global counters: one at a time.
    static COUNTERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const CI: Seen = Seen { paste_keys: 0, ctrl_inserts: 1, shift_inserts: 0 };
    const SI: Seen = Seen { paste_keys: 0, ctrl_inserts: 0, shift_inserts: 1 };

    /// Insert's copy and paste are dropped for the frame (the original has no
    /// such entries); a Ctrl+C or Ctrl+V beside them stays (#26).
    #[test]
    fn an_inserts_copy_and_paste_are_dropped() {
        let mut r = raw(vec![egui::Event::Copy]);
        apply(&mut r, CI, false);
        assert!(r.events.is_empty(), "Ctrl+Insert");
        let mut r = raw(vec![egui::Event::Paste("t".into())]);
        apply(&mut r, SI, false);
        assert!(r.events.is_empty(), "Shift+Insert");
        let mut r = raw(vec![egui::Event::Copy]);
        apply(&mut r, Seen::default(), false);
        assert_eq!(r.events.len(), 1, "Ctrl+C");
        let mut r = raw(vec![egui::Event::Paste("t".into())]);
        apply(&mut r, Seen::default(), false);
        assert_eq!(r.events.len(), 1, "Ctrl+V");
    }

    /// Every auto-repeated Insert event of a slow pass goes, not the first
    /// only; a Ctrl+C of the pass stays when fewer Insert presses were noted.
    #[test]
    fn every_repeated_insert_event_is_dropped() {
        let mut r = raw(vec![egui::Event::Paste("a".into()), egui::Event::Paste("a".into()), egui::Event::Paste("a".into())]);
        apply(&mut r, Seen { shift_inserts: 3, ..Default::default() }, false);
        assert!(r.events.is_empty());
        let mut r = raw(vec![egui::Event::Copy, egui::Event::Copy]);
        apply(&mut r, CI, false);
        assert_eq!(r.events.len(), 1, "only the noted one");
    }

    /// With a widget holding the keyboard focus (a text edit) the events stay for it (a Win32
    /// EDIT copies on Ctrl+Insert and pastes on Shift+Insert itself), counted
    /// so that the frame's copy / paste can ignore exactly those.
    #[test]
    fn a_text_edit_keeps_its_insert_events() {
        let mut r = raw(vec![egui::Event::Copy]);
        assert_eq!(apply(&mut r, CI, true), (1, 0));
        assert_eq!(r.events.len(), 1);
        let mut r = raw(vec![egui::Event::Paste("t".into())]);
        assert_eq!(apply(&mut r, SI, true), (0, 1));
        assert_eq!(r.events.len(), 1);
    }

    /// Through `filter`: Insert's copy and paste are kept while a widget has
    /// the keyboard focus (a text edit then copies / pastes itself) and
    /// dropped when none has.
    #[test]
    fn filter_keeps_insert_events_only_while_a_widget_has_the_focus() {
        let _g = COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
        let ctx = egui::Context::default();
        let focus = std::cell::Cell::new(false);
        let run = |events: Vec<egui::Event>| {
            let _ = ctx.run(egui::RawInput { events, ..Default::default() }, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut t = String::new();
                    let r = ui.text_edit_singleline(&mut t);
                    if focus.get() {
                        r.request_focus();
                    } else {
                        r.surrender_focus();
                    }
                });
            });
        };
        run(vec![]);
        run(vec![]);
        let take = |ctx: &egui::Context, key_down: u32, ev: egui::Event| {
            let _ = PASTE_KEYS.swap(0, Ordering::Relaxed);
            let _ = CTRL_INSERTS.swap(0, Ordering::Relaxed);
            let _ = SHIFT_INSERTS.swap(0, Ordering::Relaxed);
            note_key_down(key_down, false, false, true, false);
            let mut raw = raw(vec![ev]);
            filter(ctx, &mut raw);
            raw.events.len()
        };
        assert_eq!(take(&ctx, 0x2D, egui::Event::Paste("t".into())), 0, "no widget focused: dropped");
        focus.set(true);
        run(vec![]);
        run(vec![]);
        assert_eq!(take(&ctx, 0x2D, egui::Event::Paste("t".into())), 1, "an edit focused: kept for it");
    }

    /// Ctrl+V with nothing for egui to paste is a key press when it is
    /// pressed, not at its release (#26).
    #[test]
    fn ctrl_v_without_text_is_a_key_press() {
        let mut r = raw(vec![]);
        apply(&mut r, Seen { paste_keys: 1, ..Default::default() }, false);
        assert!(matches!(r.events[..], [egui::Event::Key { key: egui::Key::V, pressed: true, .. }]));
        let mut r = raw(vec![egui::Event::Paste("t".into())]);
        apply(&mut r, Seen { paste_keys: 1, ..Default::default() }, false);
        assert_eq!(r.events.len(), 1, "a paste egui announced is not doubled");
    }

    #[test]
    fn only_the_accelerators_are_noted() {
        let _g = COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
        let take = || Seen { paste_keys: PASTE_KEYS.swap(0, Ordering::Relaxed), ctrl_inserts: CTRL_INSERTS.swap(0, Ordering::Relaxed), shift_inserts: SHIFT_INSERTS.swap(0, Ordering::Relaxed) };
        take();
        note_key_down(0x56, false, true, true, false);
        note_key_down(0x56, true, true, false, false);
        note_key_down(0x2D, false, true, true, false);
        assert_eq!(take(), Seen::default());
        note_key_down(0x56, false, true, false, false);
        note_key_down(0x2D, false, false, true, false);
        note_key_down(0x2D, true, false, true, false);
        assert_eq!(take(), Seen { paste_keys: 1, ctrl_inserts: 0, shift_inserts: 2 });
    }
}
