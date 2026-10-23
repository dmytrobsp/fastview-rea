//! A Win32-style popup menu with one level of submenus, for the diagram
//! windows' context menus and their toolbar drop-downs (MENU 387 / 388 / 355
//! / 351). Each opening gets its own areas (fresh ids per serial, sized to the
//! items) — egui's shared context-menu area remembers its size across
//! menus (see ledger L9.1).

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

/// How an item shows its state (SetCheck vs SetRadio of the CMDUI handler).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    None,
    Check(bool),
    Radio(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry<C> {
    Item { label: String, enabled: bool, check: Mark, cmd: C },
    Sub { label: String, entries: Vec<Entry<C>> },
    Sep,
}

impl<C> Entry<C> {
    pub fn item(label: impl Into<String>, enabled: bool, check: Mark, cmd: C) -> Self {
        Entry::Item { label: label.into(), enabled, check, cmd }
    }

    pub fn sub(label: impl Into<String>, entries: Vec<Entry<C>>) -> Self {
        Entry::Sub { label: label.into(), entries }
    }
}

/// An open popup menu: its serial (area ids), position and open submenu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopMenu {
    pub serial: u64,
    pub pos: Pos2,
    pub sub: Option<usize>,
    /// The keyboard's item in the menu and in its open submenu (none until
    /// an arrow key is pressed, as a popup opened by the mouse); `in_sub`
    /// = the keys act on the submenu.
    pub hot: Option<usize>,
    pub sub_hot: Option<usize>,
    pub in_sub: bool,
}

impl PopMenu {
    pub fn new(serial: u64, pos: Pos2) -> Self {
        PopMenu { serial, pos, sub: None, hot: None, sub_hot: None, in_sub: false }
    }
}

/// An item the keyboard can land on: an enabled item or a submenu.
fn selectable<C>(e: &Entry<C>) -> bool {
    matches!(e, Entry::Item { enabled: true, .. } | Entry::Sub { .. })
}

/// The next (or previous) selectable entry from `from`, wrapping.
pub fn step<C>(entries: &[Entry<C>], from: Option<usize>, down: bool) -> Option<usize> {
    let n = entries.len();
    if n == 0 {
        return None;
    }
    let mut i = from.unwrap_or(if down { n - 1 } else { 0 });
    for _ in 0..n {
        i = if down { (i + 1) % n } else { (i + n - 1) % n };
        if selectable(&entries[i]) {
            return Some(i);
        }
    }
    None
}

/// What one frame of an open menu produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome<C> {
    /// The command picked (the menu closes).
    pub chosen: Option<C>,
    /// The menu is to close (an item, Escape or a press outside).
    pub close: bool,
    /// The closing press was outside the menu: swallow it.
    pub eat: bool,
}

pub fn area_id(base: &str, serial: u64) -> egui::Id {
    egui::Id::new(base).with(serial)
}

fn menu_style(style: &mut egui::Style) {
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
    style.spacing.button_padding = Vec2::new(2.0, 0.0);
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
}

/// A menu row as Win32 draws it: a check-mark column (a tick for a
/// checked SetCheck item, a bullet for a SetRadio one, nothing otherwise),
/// then the label.
pub fn mark_item(ui: &mut egui::Ui, label: &str, enabled: bool, mark: Option<&str>) -> egui::Response {
    mark_item_hot(ui, label, enabled, mark, false)
}

/// [`mark_item`], highlighted as the keyboard's item when `hot`.
pub fn mark_item_hot(ui: &mut egui::Ui, label: &str, enabled: bool, mark: Option<&str>, hot: bool) -> egui::Response {
    const COL: f32 = 18.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), Color32::PLACEHOLDER);
    let size = Vec2::new(COL + galley.size().x + 10.0, (galley.size().y + 4.0).max(ui.spacing().interact_size.y));
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let row = Rect::from_x_y_ranges(ui.max_rect().x_range(), rect.y_range());
    if enabled && (resp.hovered() || hot) {
        ui.painter().rect_filled(row, 2.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let col = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
    if let Some(m) = mark {
        ui.painter().text(Pos2::new(rect.left() + COL / 2.0, rect.center().y), egui::Align2::CENTER_CENTER, m, font, col);
    }
    ui.painter().galley(Pos2::new(rect.left() + COL, rect.center().y - galley.size().y / 2.0), galley, col);
    resp
}

/// The glyph of a mark (none when unchecked).
pub fn glyph(mark: Mark) -> Option<&'static str> {
    match mark {
        Mark::Check(true) => Some("✓"),
        Mark::Radio(true) => Some("•"),
        _ => None,
    }
}

/// MFC copies a menu into its popup dropping a separator that comes first,
/// follows another one or comes last (`0x100e2fdd`–`0x100e3010`).
pub fn shown_seps<C>(entries: &[Entry<C>]) -> Vec<bool> {
    let mut prev_sep = true;
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| match e {
            Entry::Sep => {
                let show = !prev_sep && i + 1 < entries.len();
                prev_sep |= show;
                show
            }
            _ => {
                prev_sep = false;
                true
            }
        })
        .collect()
}

/// Lay out `entries`; returns the picked command and, for submenu rows, their
/// index and rect (hovered or clicked → `open`).
fn items<C: Clone>(ui: &mut egui::Ui, entries: &[Entry<C>], open: &mut Option<usize>, sub_rect: &mut Option<Rect>, root: bool, hot: Option<usize>) -> Option<C> {
    let mut chosen = None;
    let shown = shown_seps(entries);
    for (i, e) in entries.iter().enumerate() {
        match e {
            Entry::Sep => {
                if shown[i] {
                    ui.separator();
                }
            }
            Entry::Item { label, enabled, check, cmd } => {
                let r = mark_item_hot(ui, label, *enabled, glyph(*check), hot == Some(i));
                if root && r.hovered() {
                    *open = None;
                }
                if r.clicked() {
                    chosen = Some(cmd.clone());
                }
            }
            Entry::Sub { label, .. } => {
                let r = ui.add(egui::Button::new(format!("{label}    ▸")).selected(*open == Some(i) || hot == Some(i)));
                if r.hovered() || r.clicked() {
                    *open = Some(i);
                }
                if *open == Some(i) {
                    *sub_rect = Some(r.rect);
                }
            }
        }
    }
    chosen
}

/// Show an open menu for one frame.
pub fn show<C: Clone>(ctx: &egui::Context, base: &str, menu: &mut PopMenu, entries: &[Entry<C>]) -> Outcome<C> {
    let mut chosen = None;
    // The keys of a Win32 popup: ↑ / ↓ over the selectable items, → into a
    // submenu, ← back out, Enter picks, Esc leaves a submenu first.
    let (up, down, right, left, enter, esc_key) = ctx.input(|i| {
        let k = |key| i.key_pressed(key);
        (k(egui::Key::ArrowUp), k(egui::Key::ArrowDown), k(egui::Key::ArrowRight), k(egui::Key::ArrowLeft), k(egui::Key::Enter), k(egui::Key::Escape))
    });
    let sub_entries = |i: Option<usize>| match i.and_then(|i| entries.get(i)) {
        Some(Entry::Sub { entries, .. }) => Some(entries.as_slice()),
        _ => None,
    };
    let mut esc_close = esc_key;
    if menu.in_sub {
        let sub = sub_entries(menu.sub).unwrap_or(&[]);
        if up || down {
            menu.sub_hot = step(sub, menu.sub_hot, down);
        }
        if left || esc_key {
            (menu.in_sub, menu.sub_hot) = (false, None);
            if left {
                menu.sub = None;
            }
            esc_close = false;
        } else if enter {
            if let Some(Entry::Item { cmd, enabled: true, .. }) = menu.sub_hot.and_then(|h| sub.get(h)) {
                chosen = Some(cmd.clone());
            }
        }
    } else {
        if up || down {
            menu.hot = step(entries, menu.hot, down);
        }
        let into_sub = (right || enter) && sub_entries(menu.hot).is_some();
        if into_sub {
            menu.sub = menu.hot;
            menu.in_sub = true;
            menu.sub_hot = step(sub_entries(menu.hot).unwrap_or(&[]), None, true);
        } else if enter {
            if let Some(Entry::Item { cmd, enabled: true, .. }) = menu.hot.and_then(|h| entries.get(h)) {
                chosen = Some(cmd.clone());
            }
        }
    }
    let mut open = menu.sub;
    let mut sub_rect = None;
    let frame_ui = |ui: &mut egui::Ui, f: &mut dyn FnMut(&mut egui::Ui)| {
        egui::Frame::menu(ui.style()).show(ui, |ui| {
            menu_style(ui.style_mut());
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| f(ui));
        });
    };
    let root = egui::Area::new(area_id(base, menu.serial))
        .kind(egui::UiKind::Menu)
        .order(egui::Order::Foreground)
        .fixed_pos(menu.pos)
        .constrain(true)
        .show(ctx, |ui| {
            frame_ui(ui, &mut |ui| {
                if let Some(c) = items(ui, entries, &mut open, &mut sub_rect, true, menu.hot) {
                    chosen = Some(c);
                }
            })
        });
    let mut rects = vec![root.response.rect];
    if let (Some(i), Some(at)) = (open, sub_rect) {
        if let Some(Entry::Sub { entries: sub, .. }) = entries.get(i) {
            let area = egui::Area::new(area_id(base, menu.serial).with(("sub", i)))
                .kind(egui::UiKind::Menu)
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::new(at.right(), at.top() - 3.0))
                .constrain(true)
                .show(ctx, |ui| {
                    frame_ui(ui, &mut |ui| {
                        let (mut none, mut r) = (None, None);
                        if let Some(c) = items(ui, sub, &mut none, &mut r, false, if menu.in_sub { menu.sub_hot } else { None }) {
                            chosen = Some(c);
                        }
                    })
                });
            rects.push(area.response.rect);
        }
    }
    menu.sub = open;
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rects.iter().any(|r| r.contains(p))));
    Outcome { close: chosen.is_some() || esc_close || outside, chosen, eat: outside }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `0x100e2fdd`–`0x100e3010`: no separator first, after another one,
    /// or last.
    #[test]
    fn separators_as_mfc_copies_them() {
        let i = |n: u8| Entry::item(n.to_string(), true, Mark::None, n);
        let e = vec![Entry::Sep, i(1), Entry::Sep, Entry::Sep, i(2), Entry::Sep];
        assert_eq!(shown_seps(&e), vec![false, true, true, false, true, false]);
        let only: Vec<Entry<u8>> = vec![i(1), Entry::Sep];
        assert_eq!(shown_seps(&only), vec![true, false], "a list that ends in a separator");
        assert_eq!(glyph(Mark::Check(true)), Some("✓"));
        assert_eq!((glyph(Mark::Check(false)), glyph(Mark::Radio(true))), (None, Some("•")));
    }

    /// A Win32 popup's keys: ↓ lands on the first selectable item (a
    /// disabled one and separators skipped), → opens a submenu on its first
    /// item, Esc leaves the submenu only, Enter picks.
    #[test]
    fn popups_follow_the_keyboard() {
        let e = vec![
            Entry::item("off", false, Mark::None, 0u8),
            Entry::item("a", true, Mark::None, 1),
            Entry::Sep,
            Entry::sub("sub", vec![Entry::item("x", true, Mark::None, 7), Entry::item("y", true, Mark::None, 8)]),
        ];
        assert_eq!(step(&e, None, true), Some(1));
        assert_eq!(step(&e, Some(1), true), Some(3), "the separator skipped");
        assert_eq!(step(&e, Some(3), true), Some(1), "wraps past the disabled item");
        let ctx = egui::Context::default();
        let mut m = PopMenu::new(1, Pos2::new(10.0, 10.0));
        let frame = |m: &mut PopMenu, keys: &[egui::Key]| {
            let events = keys.iter().map(|&key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }).collect();
            let mut out = None;
            let _ = ctx.run(egui::RawInput { events, ..Default::default() }, |ctx| out = Some(show(ctx, "t", m, &e)));
            out.unwrap()
        };
        frame(&mut m, &[egui::Key::ArrowDown]);
        frame(&mut m, &[egui::Key::ArrowDown]);
        assert_eq!(m.hot, Some(3));
        frame(&mut m, &[egui::Key::ArrowRight]);
        assert!(m.in_sub && m.sub == Some(3) && m.sub_hot == Some(0));
        let o = frame(&mut m, &[egui::Key::Escape]);
        assert!(!o.close && !m.in_sub, "Esc leaves the submenu only");
        frame(&mut m, &[egui::Key::Enter]);
        frame(&mut m, &[egui::Key::ArrowDown]);
        let o = frame(&mut m, &[egui::Key::Enter]);
        assert_eq!(o.chosen, Some(8), "Enter on the submenu's second item");
    }
}
