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
    const COL: f32 = 18.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), Color32::PLACEHOLDER);
    let size = Vec2::new(COL + galley.size().x + 10.0, (galley.size().y + 4.0).max(ui.spacing().interact_size.y));
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let row = Rect::from_x_y_ranges(ui.max_rect().x_range(), rect.y_range());
    if enabled && resp.hovered() {
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
fn items<C: Clone>(ui: &mut egui::Ui, entries: &[Entry<C>], open: &mut Option<usize>, sub_rect: &mut Option<Rect>, root: bool) -> Option<C> {
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
                let r = mark_item(ui, label, *enabled, glyph(*check));
                if root && r.hovered() {
                    *open = None;
                }
                if r.clicked() {
                    chosen = Some(cmd.clone());
                }
            }
            Entry::Sub { label, .. } => {
                let r = ui.add(egui::Button::new(format!("{label}    ▸")).selected(*open == Some(i)));
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
    let mut open = menu.sub;
    let mut sub_rect = None;
    let mut chosen = None;
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
                if let Some(c) = items(ui, entries, &mut open, &mut sub_rect, true) {
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
                        if let Some(c) = items(ui, sub, &mut none, &mut r, false) {
                            chosen = Some(c);
                        }
                    })
                });
            rects.push(area.response.rect);
        }
    }
    menu.sub = open;
    let (esc, outside) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Escape),
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rects.iter().any(|r| r.contains(p))),
        )
    });
    Outcome { close: chosen.is_some() || esc || outside, chosen, eat: outside }
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
}
