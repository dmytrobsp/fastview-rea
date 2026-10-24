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
/// The keyboard's stops: every item, disabled ones too (VM: ↓ lands on a
/// greyed item with a grey bar), and the submenus; not the separators.
fn selectable<C>(e: &Entry<C>) -> bool {
    matches!(e, Entry::Item { .. } | Entry::Sub { .. })
}

/// The next (or previous) item or submenu from `from`, wrapping.
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

// CMFCPopupMenu in the Office 2007 looks (VM, every context menu of the
// original — the view's, the panes', the diagrams'): a (134, 134, 134)
// border and a (245, 245, 245) line inside it round a (250, 250, 250)
// ground, a 24 px (233, 238, 238) image column ending in a (197, 197, 197)
// / (245, 245, 245) pair, navy text, and a 3 px shadow.
const MFC_GROUND: Color32 = Color32::from_rgb(250, 250, 250);
const MFC_INNER: Color32 = Color32::from_rgb(245, 245, 245);
const MFC_COLUMN: Color32 = Color32::from_rgb(233, 238, 238);
const MFC_RULE: Color32 = Color32::from_rgb(197, 197, 197);
const MFC_TEXT: Color32 = Color32::from_rgb(0, 21, 110);
const MFC_GREY: Color32 = Color32::from_rgb(141, 141, 141);
/// The image column's width.
const MFC_COL_W: f32 = 24.0;

/// A popup menu's frame and image column around `add` (the rows).
pub fn mfc_frame<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    mfc_frame_with(ui, true, add)
}

/// [`mfc_frame`], with or without the image column (a menu of large
/// images and descriptions has none).
pub fn mfc_frame_with<R>(ui: &mut egui::Ui, column: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    use crate::oscgeom::PixelLines;
    let frame = egui::Frame::none()
        .fill(MFC_GROUND)
        .stroke(Stroke::new(1.0f32, Color32::from_gray(134)))
        .inner_margin(egui::Margin::same(2.0))
        .shadow(egui::epaint::Shadow { offset: Vec2::new(3.0, 3.0), blur: 3.0, spread: 0.0, color: Color32::from_black_alpha(60) });
    frame
        .show(ui, |ui| {
            menu_style(ui.style_mut());
            let under = ui.painter().add(egui::Shape::Noop);
            let r = ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), add).inner;
            let c = ui.min_rect();
            let p = ui.painter();
            p.px_rect_stroke(c.expand(1.0), 0.0, Stroke::new(1.0f32, MFC_INNER));
            if column {
                let col = Rect::from_min_max(Pos2::new(c.left(), c.top() + 1.0), Pos2::new(c.left() + MFC_COL_W, c.bottom() - 1.0));
                p.set(under, egui::Shape::rect_filled(col, 0.0, MFC_COLUMN));
                p.px_vline(col.right(), col.y_range(), Stroke::new(1.0f32, MFC_RULE));
                p.px_vline(col.right() + 1.0, col.y_range(), Stroke::new(1.0f32, MFC_INNER));
            }
            r
        })
        .inner
}

/// A menu's title band (a ribbon menu's caption, VM): 17 px of (222, 232,
/// 239), the title bold, a rule under it.
pub fn title_band(ui: &mut egui::Ui, title: &str) {
    use crate::oscgeom::PixelLines;
    // The menu at least as wide as its title.
    let tw = ui.fonts(|f| crate::lanes::bold_font(f, 12.0).map(|b| f.layout_no_wrap(title.to_string(), b, MFC_TEXT).size().x)).unwrap_or(0.0);
    ui.set_min_width(tw + 12.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, 19.0), egui::Sense::hover());
    let w = ui.max_rect().x_range();
    let band = Rect::from_x_y_ranges(w, r.top()..=r.top() + 17.0);
    ui.painter().rect_filled(band, 0.0, Color32::from_rgb(222, 232, 239));
    crate::lanes::bold_text(ui.painter(), Pos2::new(band.left() + 5.0, band.top() + 2.0), title, egui::FontId::proportional(12.0), MFC_TEXT);
    ui.painter().px_hline(w, band.bottom(), Stroke::new(1.0f32, MFC_RULE));
    ui.painter().px_hline(w, band.bottom() + 1.0, Stroke::new(1.0f32, MFC_INNER));
}

/// A large item: its 32 px image, a bold title and the description under
/// it (a ribbon menu's entries, VM).
pub fn large_item(ui: &mut egui::Ui, label: &str, desc: &str, enabled: bool, icon: RowIcon) -> egui::Response {
    let wrap = ui.max_rect().width() - 53.0;
    let gal = ui.painter().layout(desc.to_string(), egui::FontId::proportional(12.0), Color32::from_rgb(21, 66, 139), wrap);
    let h = (22.0 + gal.size().y + 6.0).max(48.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, h), egui::Sense::hover());
    let row = Rect::from_x_y_ranges(ui.max_rect().x_range(), rect.y_range());
    let resp = ui.interact(row, ui.id().with(("fv_large_item", label)), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    if enabled && ui.rect_contains_pointer(row) {
        hot_bar(ui.painter(), row);
    }
    icon(ui.painter(), Rect::from_min_size(Pos2::new(row.left() + 6.0, row.top() + 3.0), Vec2::splat(32.0)), if enabled { Color32::from_rgb(40, 60, 130) } else { MFC_GREY });
    let ink = if enabled { MFC_TEXT } else { MFC_GREY };
    crate::lanes::bold_text(ui.painter(), Pos2::new(row.left() + 43.0, row.top() + 4.0), label, egui::FontId::proportional(12.0), ink);
    ui.painter().galley(Pos2::new(row.left() + 43.0, row.top() + 20.0), gal, ink);
    resp
}

/// A menu's separator: 7 px with a (197, 197, 197) / (245, 245, 245) rule
/// from the text's column to the edge (VM).
pub fn separator(ui: &mut egui::Ui) {
    use crate::oscgeom::PixelLines;
    // No width of its own (a popup sizes to its widest row): drawn across
    // the menu's width.
    let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, 6.0), egui::Sense::hover());
    let w = ui.max_rect();
    let x = (w.left() + MFC_COL_W + 5.0)..=w.right();
    ui.painter().px_hline(x.clone(), r.top() + 2.0, Stroke::new(1.0f32, MFC_RULE));
    ui.painter().px_hline(x, r.top() + 3.0, Stroke::new(1.0f32, MFC_INNER));
}

/// The hot item's bar: 22 px, a thin amber border and a two-part gradient
/// (VM).
pub(crate) fn hot_bar(p: &egui::Painter, r: Rect) {
    use crate::oscgeom::PixelLines;
    let inner = r.shrink(1.0);
    let mid = inner.top() + 10.0;
    crate::lanes::gradient(p, Rect::from_min_max(inner.min, Pos2::new(inner.right(), mid)), Color32::from_rgb(255, 253, 235), Color32::from_rgb(255, 235, 172));
    crate::lanes::gradient(p, Rect::from_min_max(Pos2::new(inner.left(), mid), Pos2::new(inner.right(), inner.bottom() - 1.0)), Color32::from_rgb(255, 213, 99), Color32::from_rgb(255, 227, 145));
    p.px_hline(inner.x_range(), inner.bottom() - 1.0, Stroke::new(1.0f32, Color32::from_rgb(255, 248, 181)));
    p.px_hline(r.x_range(), r.top(), Stroke::new(1.0f32, Color32::from_rgb(219, 206, 153)));
    p.px_hline(r.x_range(), r.bottom() - 1.0, Stroke::new(1.0f32, Color32::from_rgb(215, 208, 179)));
    p.px_vline(r.left(), r.y_range(), Stroke::new(1.0f32, Color32::from_rgb(195, 170, 121)));
    p.px_vline(r.right() - 1.0, r.y_range(), Stroke::new(1.0f32, Color32::from_rgb(195, 170, 121)));
}

/// A disabled item's bar under the pointer or the keyboard: the hot bar
/// in greys (VM: a (212) top, (195) sides, 250 to 231 above the middle,
/// 210 to 224 below, a (230) inner bottom line, a (222) bottom).
pub(crate) fn grey_bar(p: &egui::Painter, r: Rect) {
    use crate::oscgeom::PixelLines;
    let inner = r.shrink(1.0);
    let mid = inner.top() + 10.0;
    crate::lanes::gradient(p, Rect::from_min_max(inner.min, Pos2::new(inner.right(), mid)), Color32::from_gray(250), Color32::from_gray(231));
    crate::lanes::gradient(p, Rect::from_min_max(Pos2::new(inner.left(), mid), Pos2::new(inner.right(), inner.bottom() - 1.0)), Color32::from_gray(210), Color32::from_gray(224));
    p.px_hline(inner.x_range(), inner.bottom() - 1.0, Stroke::new(1.0f32, Color32::from_gray(230)));
    p.px_hline(r.x_range(), r.top(), Stroke::new(1.0f32, Color32::from_gray(212)));
    p.px_hline(r.x_range(), r.bottom() - 1.0, Stroke::new(1.0f32, Color32::from_gray(222)));
    p.px_vline(r.left(), r.y_range(), Stroke::new(1.0f32, Color32::from_gray(195)));
    p.px_vline(r.right() - 1.0, r.y_range(), Stroke::new(1.0f32, Color32::from_gray(195)));
}

/// The look of a Win32 popup menu's rows (also the view's own menus).
pub fn menu_style(style: &mut egui::Style) {
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
    style.spacing.button_padding = Vec2::new(2.0, 0.0);
    // Rows 21 px a pixel apart: the 22 px pitch of the original's menus,
    // and with egui's 6 px separator its 29 px across one (VM).
    style.spacing.item_spacing.y = 1.0;
    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    style.visuals.override_text_color = Some(MFC_TEXT);
}

/// A menu row as Win32 draws it: a check-mark column (a tick for a
/// checked SetCheck item, a bullet for a SetRadio one, nothing otherwise),
/// then the label.
/// Highlighted as the keyboard's item when `hot`.
pub fn mark_item_hot(ui: &mut egui::Ui, label: &str, enabled: bool, mark: Option<&str>, hot: bool) -> egui::Response {
    mark_item_keys(ui, label, None, enabled, mark, hot)
}

/// An item's image in the menu's column, painted into a 16 px square.
pub type RowIcon<'a> = &'a dyn Fn(&egui::Painter, Rect, Color32);

/// [`mark_item_hot`] with the accelerator's text right-aligned (the
/// original's «Ctrl+C» beside an item, VM).
pub fn mark_item_keys(ui: &mut egui::Ui, label: &str, keys: Option<&str>, enabled: bool, mark: Option<&str>, hot: bool) -> egui::Response {
    mark_item_icon(ui, label, keys, enabled, mark, hot, None)
}

/// [`mark_item_keys`] with the item's image in the column: a checked item
/// shows its image in the amber box instead of a tick (VM).
pub fn mark_item_icon(ui: &mut egui::Ui, label: &str, keys: Option<&str>, enabled: bool, mark: Option<&str>, hot: bool, icon: Option<RowIcon>) -> egui::Response {
    // The check / image column: the label 31 px in from the menu's edge (VM).
    const COL: f32 = 29.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), Color32::PLACEHOLDER);
    let keys_w = keys.map_or(0.0, |k| ui.painter().layout_no_wrap(k.to_string(), font.clone(), Color32::PLACEHOLDER).size().x + 40.0);
    let size = Vec2::new(COL + galley.size().x + 10.0 + keys_w, 21.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let row = Rect::from_x_y_ranges(ui.max_rect().x_range(), rect.y_range());
    if resp.hovered() || hot {
        let bar = Rect::from_min_max(row.min, Pos2::new(row.right(), row.top() + 22.0));
        if enabled {
            hot_bar(ui.painter(), bar);
        } else {
            grey_bar(ui.painter(), bar);
        }
    }
    let col = if enabled { MFC_TEXT } else { MFC_GREY };
    let bx = Rect::from_min_size(Pos2::new(row.left() + 2.0, rect.top() + 1.0), Vec2::splat(20.0));
    if let Some(m) = mark {
        // A checked item: its mark (or image) in a 20 px amber box.
        ui.painter().rect(bx.shrink(0.5), 0.0, Color32::from_rgb(255, 227, 149), Stroke::new(1.0f32, Color32::from_rgb(242, 149, 54)));
        if icon.is_none() {
            ui.painter().text(bx.center(), egui::Align2::CENTER_CENTER, m, font.clone(), col);
        }
    }
    if let Some(paint) = icon {
        paint(ui.painter(), Rect::from_center_size(bx.center(), Vec2::splat(16.0)), if enabled { Color32::from_rgb(40, 60, 130) } else { MFC_GREY });
    }
    ui.painter().galley(Pos2::new(rect.left() + COL, rect.center().y - galley.size().y / 2.0), galley, col);
    match keys {
        // A submenu's arrow: a solid triangle (VM).
        Some("▸") => {
            let c = Pos2::new(row.right() - 12.0, rect.center().y);
            ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(-2.0, -4.0), c + Vec2::new(2.0, 0.0), c + Vec2::new(-2.0, 4.0)], col, Stroke::NONE));
        }
        Some(k) => {
            ui.painter().text(Pos2::new(row.right() - 14.0, rect.center().y), egui::Align2::RIGHT_CENTER, k, font, col);
        }
        None => {}
    }
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
                    separator(ui);
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
                let r = mark_item_keys(ui, label, Some("▸"), true, None, *open == Some(i) || hot == Some(i));
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
        mfc_frame(ui, |ui| f(ui));
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

    /// A popup's keys (CMFCPopupMenu, VM): ↓ / ↑ stop on every item, a
    /// disabled one too (Enter there picks nothing), separators skipped; →
    /// opens a submenu on its first item, Esc leaves the submenu only,
    /// Enter picks.
    #[test]
    fn popups_follow_the_keyboard() {
        let e = vec![
            Entry::item("off", false, Mark::None, 0u8),
            Entry::item("a", true, Mark::None, 1),
            Entry::Sep,
            Entry::sub("sub", vec![Entry::item("x", true, Mark::None, 7), Entry::item("y", true, Mark::None, 8)]),
        ];
        assert_eq!(step(&e, None, true), Some(0), "the disabled item is a stop");
        assert_eq!(step(&e, Some(1), true), Some(3), "the separator skipped");
        assert_eq!(step(&e, Some(3), true), Some(0), "wraps to the disabled item");
        let ctx = egui::Context::default();
        let mut m = PopMenu::new(1, Pos2::new(10.0, 10.0));
        let frame = |m: &mut PopMenu, keys: &[egui::Key]| {
            let events = keys.iter().map(|&key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }).collect();
            let mut out = None;
            let _ = ctx.run(egui::RawInput { events, ..Default::default() }, |ctx| out = Some(show(ctx, "t", m, &e)));
            out.unwrap()
        };
        frame(&mut m, &[egui::Key::ArrowDown]);
        let o = frame(&mut m, &[egui::Key::Enter]);
        assert!(m.hot == Some(0) && o.chosen.is_none() && !o.close, "Enter on a disabled item: nothing");
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
