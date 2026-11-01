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
    /// An item's image in the column (the toolbar image the original's
    /// CMFCPopupMenu finds for its command).
    Image(MenuImage),
}

/// Our drawings of the menu images (VM: 16 × 16, blue).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuImage {
    /// «Показать все»: the eye with a «+».
    EyePlus,
    /// «Скрыть все»: the eye with a «−».
    EyeMinus,
}

/// Paint `img` into its 16 px square: a (52, 152, 200) frame round a
/// (140, 212, 240) face, a white eye (outline and pupil) in its upper
/// half, the «+» / «−» at its lower left (VM, the original's images).
fn paint_image(p: &egui::Painter, r: Rect, img: MenuImage) {
    // Stretched with the menus' images at > 100 %.
    let side = crate::dpi::img(16.0);
    let k = side / 16.0;
    let r = Rect::from_min_size(r.min.round(), Vec2::splat(side));
    p.rect_filled(r, 0.0, Color32::from_rgb(52, 152, 200));
    p.rect_filled(r.shrink(1.0), 0.0, Color32::from_rgb(140, 212, 240));
    let white = Stroke::new(1.0f32, Color32::WHITE);
    p.add(egui::Shape::ellipse_stroke(Pos2::new(r.left() + 9.0 * k, r.top() + 6.5 * k), Vec2::new(5.5, 2.5) * k, white));
    p.rect_filled(Rect::from_min_size(Pos2::new(r.left() + 8.0 * k, r.top() + 5.0 * k), Vec2::splat(2.0 * k)), 0.0, Color32::WHITE);
    let (y, x0, x1) = (r.bottom() - crate::dpi::half(3.5), r.left() + 1.0, r.left() + 6.0 * k);
    p.hline(x0..=x1, y, white);
    if img == MenuImage::EyePlus {
        p.vline(r.left() + crate::dpi::half(3.5), y - 2.5 * k..=y + 2.5 * k, white);
    }
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
    /// A context menu's pixel pads above and below its rows; a ribbon
    /// button's menu has none (VM: «Стиль»).
    pub pad: bool,
}

impl PopMenu {
    pub fn new(serial: u64, pos: Pos2) -> Self {
        PopMenu { serial, pos, sub: None, hot: None, sub_hot: None, in_sub: false, pad: true }
    }

    /// A ribbon button's menu: no pads.
    pub fn unpadded(mut self) -> Self {
        self.pad = false;
        self
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
/// The image column's width: the 16 px image + 8, stretched at > 100 %.
fn col_w() -> f32 {
    crate::dpi::img(16.0) + 8.0
}

/// A menu row's pitch: 22 px, 24 / 28 at 125 / 150 % (VM: the rows of
/// the view's menu; the images 20 / 24 px there).
pub fn row_pitch() -> f32 {
    crate::dpi::measured(22.0, 24.0, 28.0)
}

/// A popup menu's frame and image column around `add` (the rows).
pub fn mfc_frame<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    mfc_frame_with(ui, true, true, add)
}

/// [`mfc_frame`], with or without the image column (a menu of large
/// images and descriptions has none), and with a context menu's pixel
/// clear above and below its rows (`pad`) or without, as a ribbon
/// panel menu has it (VM: the status bar's and the QAT's).
pub fn mfc_frame_with<R>(ui: &mut egui::Ui, column: bool, pad: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    use crate::oscgeom::PixelLines;
    // The (134) border on the frame's own outer pixels, the inner line
    // just inside it (VM: the border at the menu's point, then (245));
    // egui's stroke would sit a pixel further out.
    let frame = egui::Frame::none()
        .fill(MFC_GROUND)
        .inner_margin(egui::Margin::same(2.0))
        .shadow(egui::epaint::Shadow { offset: Vec2::new(3.0, 3.0), blur: 3.0, spread: 0.0, color: Color32::from_black_alpha(60) });
    frame
        .show(ui, |ui| {
            menu_style(ui.style_mut());
            let under = ui.painter().add(egui::Shape::Noop);
            // A pixel clear above the first row and below the last (VM).
            let r = ui
                .with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    if pad {
                        ui.allocate_exact_size(Vec2::new(1.0, 0.0), egui::Sense::hover());
                    }
                    let r = add(ui);
                    if pad {
                        ui.allocate_exact_size(Vec2::new(1.0, 1.0), egui::Sense::hover());
                    }
                    r
                })
                .inner;
            let c = ui.min_rect();
            let p = ui.painter();
            p.px_rect_stroke(c.expand(1.0), 0.0, Stroke::new(1.0f32, MFC_INNER));
            p.px_rect_stroke(c.expand(2.0), 0.0, Stroke::new(1.0f32, Color32::from_gray(134)));
            if column {
                let col = Rect::from_min_max(Pos2::new(c.left(), c.top() + 1.0), Pos2::new(c.left() + col_w(), c.bottom() - 1.0));
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
    let size = crate::dpi::font(12.0);
    let tw = ui.fonts(|f| crate::lanes::bold_font(f, size).map(|b| f.layout_no_wrap(title.to_string(), b, MFC_TEXT).size().x)).unwrap_or(0.0);
    ui.set_min_width(tw + 12.0);
    // The band 17 px, then the rule and its light line; the rows from
    // the band's top + 20 (VM, the status bar's menu).
    let bh = crate::dpi::s(17.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, bh + 2.0), egui::Sense::hover());
    let w = ui.max_rect().x_range();
    let band = Rect::from_x_y_ranges(w, r.top()..=r.top() + bh);
    ui.painter().rect_filled(band, 0.0, Color32::from_rgb(222, 232, 239));
    crate::lanes::bold_text(ui.painter(), Pos2::new(band.left() + 5.0, band.top() + 2.0), title, egui::FontId::proportional(size), MFC_TEXT);
    ui.painter().px_hline(w, band.bottom(), Stroke::new(1.0f32, MFC_RULE));
    ui.painter().px_hline(w, band.bottom() + 1.0, Stroke::new(1.0f32, MFC_INNER));
}

/// A large item: its 32 px image, a bold title and the description under
/// it (a ribbon menu's entries, VM).
pub fn large_item(ui: &mut egui::Ui, label: &str, desc: &str, enabled: bool, icon: RowIcon) -> egui::Response {
    use crate::dpi::{img, s};
    let side = img(32.0);
    let font = egui::FontId::proportional(crate::dpi::font(12.0));
    let wrap = ui.max_rect().width() - side - 21.0;
    let gal = ui.painter().layout(desc.to_string(), font.clone(), Color32::from_rgb(21, 66, 139), wrap);
    let h = (s(22.0) + gal.size().y + 6.0).max(side + 16.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, h), egui::Sense::hover());
    let row = Rect::from_x_y_ranges(ui.max_rect().x_range(), rect.y_range());
    let resp = ui.interact(row, ui.id().with(("fv_large_item", label)), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    if enabled && ui.rect_contains_pointer(row) {
        hot_bar(ui.painter(), row);
    }
    icon(ui.painter(), Rect::from_min_size(Pos2::new(row.left() + 6.0, row.top() + 3.0), Vec2::splat(side)), if enabled { Color32::from_rgb(40, 60, 130) } else { MFC_GREY });
    let ink = if enabled { MFC_TEXT } else { MFC_GREY };
    crate::lanes::bold_text(ui.painter(), Pos2::new(row.left() + side + 11.0, row.top() + 4.0), label, font, ink);
    ui.painter().galley(Pos2::new(row.left() + side + 11.0, row.top() + s(20.0)), gal, ink);
    resp
}

/// A menu's separator: 7 px with a (197, 197, 197) / (245, 245, 245) rule
/// from the text's column to the edge (VM).
pub fn separator(ui: &mut egui::Ui) {
    use crate::oscgeom::PixelLines;
    // No width of its own (a popup sizes to its widest row): drawn across
    // the menu's width.
    // 8 px with the item spacing: three clear, the rule, its light line,
    // three clear (VM).
    let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, 7.0), egui::Sense::hover());
    let w = ui.max_rect();
    let x = (w.left() + col_w() + 5.0)..=w.right();
    ui.painter().px_hline(x.clone(), r.top() + 3.0, Stroke::new(1.0f32, MFC_RULE));
    ui.painter().px_hline(x, r.top() + 4.0, Stroke::new(1.0f32, MFC_INNER));
}

/// A ribbon panel menu's separator: 4 px with the item spacing — the
/// rule, its light line, two clear (VM, the status bar's menu).
pub fn panel_separator(ui: &mut egui::Ui) {
    use crate::oscgeom::PixelLines;
    let (r, _) = ui.allocate_exact_size(Vec2::new(1.0, 3.0), egui::Sense::hover());
    let w = ui.max_rect();
    let x = (w.left() + col_w() + 5.0)..=w.right();
    ui.painter().px_hline(x.clone(), r.top(), Stroke::new(1.0f32, MFC_RULE));
    ui.painter().px_hline(x, r.top() + 1.0, Stroke::new(1.0f32, MFC_INNER));
}

/// The hot item's bar: 22 px, a thin amber border and a two-part gradient
/// (VM).
pub(crate) fn hot_bar(p: &egui::Painter, r: Rect) {
    use crate::oscgeom::PixelLines;
    let inner = r.shrink(1.0);
    let mid = inner.top() + crate::dpi::s(10.0);
    crate::lanes::gradient(p, Rect::from_min_max(inner.min, Pos2::new(inner.right(), mid)), Color32::from_rgb(255, 253, 235), Color32::from_rgb(255, 235, 172));
    crate::lanes::gradient(p, Rect::from_min_max(Pos2::new(inner.left(), mid), Pos2::new(inner.right(), inner.bottom() - 1.0)), Color32::from_rgb(255, 213, 99), Color32::from_rgb(255, 227, 145));
    p.px_hline(inner.x_range(), inner.bottom() - 1.0, Stroke::new(1.0f32, Color32::from_rgb(255, 248, 181)));
    p.px_hline(r.x_range(), r.top(), Stroke::new(1.0f32, Color32::from_rgb(219, 206, 153)));
    p.px_hline(r.x_range(), r.bottom() - 1.0, Stroke::new(1.0f32, Color32::from_rgb(215, 208, 179)));
    p.px_vline(r.left(), r.y_range(), Stroke::new(1.0f32, Color32::from_rgb(195, 170, 121)));
    p.px_vline(r.right() - 1.0, r.y_range(), Stroke::new(1.0f32, Color32::from_rgb(195, 170, 121)));
}

/// A disabled item's bar under the keyboard: the hot bar
/// in greys (VM: a (212) top, (195) sides, 250 to 231 above the middle,
/// 210 to 224 below, a (230) inner bottom line, a (222) bottom).
pub(crate) fn grey_bar(p: &egui::Painter, r: Rect) {
    use crate::oscgeom::PixelLines;
    let inner = r.shrink(1.0);
    let mid = inner.top() + crate::dpi::s(10.0);
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
    // The check / image column: the label 31 px in from the menu's edge
    // (VM); the column, the row and the image grow at > 100 %.
    let col_x = col_w() + 5.0;
    let pitch = row_pitch();
    let side = crate::dpi::img(16.0);
    let font = egui::TextStyle::Button.resolve(ui.style());
    // «&» marks the mnemonic: shown underlined (CMFCPopupMenu draws it
    // always, VM: the pane menu opened by the mouse).
    let (label, ul) = split_mnemonic(label);
    let label = label.as_str();
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), Color32::PLACEHOLDER);
    let keys_w = keys.map_or(0.0, |k| ui.painter().layout_no_wrap(k.to_string(), font.clone(), Color32::PLACEHOLDER).size().x + 40.0);
    let size = Vec2::new(col_x + galley.size().x + 10.0 + keys_w, pitch - 1.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let row = Rect::from_x_y_ranges(ui.max_rect().x_range(), rect.y_range());
    // A disabled row: the grey bar under the keyboard only, nothing under
    // the pointer (VM).
    let bar = Rect::from_min_max(row.min, Pos2::new(row.right(), row.top() + pitch));
    if enabled && (resp.hovered() || hot) {
        hot_bar(ui.painter(), bar);
    } else if !enabled && hot {
        grey_bar(ui.painter(), bar);
    }
    let col = if enabled { MFC_TEXT } else { MFC_GREY };
    let bx = Rect::from_min_size(Pos2::new(row.left() + 2.0, rect.top() + 1.0), Vec2::splat(pitch - 2.0));
    if let Some(m) = mark {
        // A checked item: its mark (or image) in a 20 px amber box.
        ui.painter().rect(bx.shrink(0.5), 0.0, Color32::from_rgb(255, 227, 149), Stroke::new(1.0f32, Color32::from_rgb(242, 149, 54)));
        if icon.is_none() {
            ui.painter().text(bx.center(), egui::Align2::CENTER_CENTER, m, font.clone(), col);
        }
    }
    if let Some(paint) = icon {
        paint(ui.painter(), Rect::from_center_size(bx.center(), Vec2::splat(side)), if enabled { Color32::from_rgb(40, 60, 130) } else { MFC_GREY });
    }
    let at = Pos2::new(rect.left() + col_x, rect.center().y - galley.size().y / 2.0);
    if let Some(i) = ul {
        let w = |t: String| ui.painter().layout_no_wrap(t, font.clone(), col).size().x;
        let x0 = at.x + w(label.chars().take(i).collect());
        let x1 = x0 + w(label.chars().skip(i).take(1).collect());
        let y = (at.y + galley.size().y - 2.0).round();
        ui.painter().rect_filled(Rect::from_min_max(Pos2::new(x0.round(), y), Pos2::new(x1.round(), y + 1.0)), 0.0, col);
    }
    ui.painter().galley(at, galley, col);
    match keys {
        // A submenu's arrow: a solid triangle (VM).
        Some("▸") => {
            let c = Pos2::new(row.right() - crate::dpi::s(12.0), rect.center().y);
            ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(-2.0, -4.0), c + Vec2::new(2.0, 0.0), c + Vec2::new(-2.0, 4.0)], col, Stroke::NONE));
        }
        Some(k) => {
            ui.painter().text(Pos2::new(row.right() - crate::dpi::s(14.0), rect.center().y), egui::Align2::RIGHT_CENTER, k, font, col);
        }
        None => {}
    }
    resp
}

/// A label without its «&» and the place of the letter it marks («&&» is
/// an ampersand).
pub fn split_mnemonic(label: &str) -> (String, Option<usize>) {
    let (mut out, mut ul, mut chars) = (String::new(), None, label.chars().peekable());
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.next() {
                Some('&') => out.push('&'),
                Some(n) => {
                    ul = ul.or(Some(out.chars().count()));
                    out.push(n);
                }
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    (out, ul)
}

/// The entry of `entries` whose mnemonic is `typed` (case-insensitive),
/// when exactly one has it.
fn mnemonic_entry<C>(entries: &[Entry<C>], typed: &str) -> Option<usize> {
    let key = typed.to_lowercase();
    let hits: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            let label = match e {
                Entry::Item { label, .. } | Entry::Sub { label, .. } => label,
                Entry::Sep => return false,
            };
            let (shown, ul) = split_mnemonic(label);
            ul.and_then(|i| shown.chars().nth(i)).is_some_and(|c| c.to_lowercase().to_string() == key)
        })
        .map(|(i, _)| i)
        .collect();
    (hits.len() == 1).then(|| hits[0])
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
fn items<C: Clone>(ui: &mut egui::Ui, entries: &[Entry<C>], open: &mut Option<usize>, sub_rect: &mut Option<Rect>, root: bool, hot: Option<usize>, pad: bool) -> Option<C> {
    let mut chosen = None;
    let shown = shown_seps(entries);
    for (i, e) in entries.iter().enumerate() {
        match e {
            Entry::Sep => {
                // A ribbon button's menu has the panel's 4 px separator.
                if shown[i] && pad {
                    separator(ui);
                } else if shown[i] {
                    panel_separator(ui);
                }
            }
            Entry::Item { label, enabled, check, cmd } => {
                let r = match check {
                    Mark::Image(img) => {
                        let img = *img;
                        mark_item_icon(ui, label, None, *enabled, None, hot == Some(i), Some(&move |p: &egui::Painter, r: Rect, _| paint_image(p, r, img)))
                    }
                    _ => mark_item_hot(ui, label, *enabled, glyph(*check), hot == Some(i)),
                };
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

/// Where a popup menu of `size` opens for the point `at` on `screen` (CMFCPopupMenu's layout, mfc120u about
/// `0x100dd75e`–`0x100dd7b4`; the clone at 125 %): at the point when it fits below it; otherwise upward from
/// the point, its bottom edge at the point, clamped to the top of the monitor's work area (the lane menu near
/// the foot opened at y 0). Horizontally the original moves a menu past the right edge to 1 px inside the work
/// area; `constrain` leaves it flush (1 px). Differences: ours is confined to the window's client (the work
/// area when maximised), and a menu taller than the area is not cut and scrolled.
pub fn menu_origin(at: Pos2, size: Vec2, screen: Rect) -> Pos2 {
    if at.y + size.y > screen.bottom() {
        Pos2::new(at.x, (at.y - size.y).max(screen.top()))
    } else {
        at
    }
}

/// [`menu_origin`] for the area `id`, whose height is last pass's (an area's first pass is a hidden
/// sizing one, so the menu is never seen at the wrong place).
pub fn menu_pos(ctx: &egui::Context, id: egui::Id, at: Pos2) -> Pos2 {
    let size = ctx.memory(|m| m.area_rect(id)).map_or(Vec2::ZERO, |r| r.size());
    menu_origin(at, size, ctx.screen_rect())
}

/// Show an open menu for one frame.
pub fn show<C: Clone>(ctx: &egui::Context, base: &str, menu: &mut PopMenu, entries: &[Entry<C>]) -> Outcome<C> {
    let mut chosen = None;
    // The keys of a Win32 popup: ↑ / ↓ over the selectable items, → into a
    // submenu, ← back out, Enter picks, Esc leaves a submenu first.
    // The open popup owns these keys (Win32's menu loop): taken from the
    // input, so a dialog the chosen item opens in this pass does not see
    // the Enter that chose it (VM: the frequency «Настройки» by keys).
    let (up, down, right, left, enter, esc_key) = ctx.input_mut(|i| {
        let mut k = |key| i.consume_key(egui::Modifiers::NONE, key);
        (k(egui::Key::ArrowUp), k(egui::Key::ArrowDown), k(egui::Key::ArrowRight), k(egui::Key::ArrowLeft), k(egui::Key::Enter), k(egui::Key::Escape))
    });
    let sub_entries = |i: Option<usize>| match i.and_then(|i| entries.get(i)) {
        Some(Entry::Sub { entries, .. }) => Some(entries.as_slice()),
        _ => None,
    };
    // A mnemonic's letter picks its item (or opens its submenu) when one
    // item of the level has it (Win32's menu loop).
    let typed: Option<String> = ctx.input(|i| i.events.iter().find_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }));
    if let Some(t) = typed {
        if menu.in_sub {
            if let Some(Entry::Item { cmd, enabled: true, .. }) = mnemonic_entry(sub_entries(menu.sub).unwrap_or(&[]), &t).and_then(|h| sub_entries(menu.sub).and_then(|s| s.get(h))) {
                chosen = Some(cmd.clone());
            }
        } else if let Some(h) = mnemonic_entry(entries, &t) {
            match &entries[h] {
                Entry::Item { cmd, enabled: true, .. } => chosen = Some(cmd.clone()),
                Entry::Sub { entries: sub, .. } => {
                    (menu.hot, menu.sub, menu.in_sub) = (Some(h), Some(h), true);
                    menu.sub_hot = step(sub, None, true);
                }
                _ => {}
            }
        }
    }
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
    let pad = menu.pad;
    let frame_ui = |ui: &mut egui::Ui, f: &mut dyn FnMut(&mut egui::Ui)| {
        mfc_frame_with(ui, true, pad, |ui| {
            f(ui);
            // Unpadded, the last row's 22 px bar still ends the menu.
            if !pad {
                ui.allocate_exact_size(Vec2::new(1.0, 0.0), egui::Sense::hover());
            }
        });
    };
    let root_id = area_id(base, menu.serial);
    let root = egui::Area::new(root_id)
        .kind(egui::UiKind::Menu)
        .order(egui::Order::Foreground)
        .fixed_pos(menu_pos(ctx, root_id, menu.pos))
        .constrain(true)
        .show(ctx, |ui| {
            frame_ui(ui, &mut |ui| {
                if let Some(c) = items(ui, entries, &mut open, &mut sub_rect, true, menu.hot, pad) {
                    chosen = Some(c);
                }
            })
        });
    let mut rects = vec![root.response.rect];
    if let (Some(i), Some(at)) = (open, sub_rect) {
        if let Some(Entry::Sub { entries: sub, .. }) = entries.get(i) {
            // CMFCPopupMenu: a submenu that would run past the bottom opens
            // upward, its bottom at its item's (VM: the frequency pane's
            // «Каналы» near the screen's foot). Its height is last frame's.
            let sub_id = area_id(base, menu.serial).with(("sub", i));
            let h = ctx.memory(|m| m.area_rect(sub_id)).map_or(0.0, |r| r.height());
            // Its border a pixel inside the parent's right one and a pixel
            // above its item (VM: «Каналы» of the vector pane).
            let y = if at.top() - 1.0 + h > ctx.screen_rect().bottom() { at.bottom() + 1.0 - h } else { at.top() - 1.0 };
            let area = egui::Area::new(sub_id)
                .kind(egui::UiKind::Menu)
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::new(at.right(), y))
                .constrain(true)
                .show(ctx, |ui| {
                    frame_ui(ui, &mut |ui| {
                        let (mut none, mut r) = (None, None);
                        if let Some(c) = items(ui, sub, &mut none, &mut r, false, if menu.in_sub { menu.sub_hot } else { None }, pad) {
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
    fn a_mnemonic_is_split_out_and_picks_its_item() {
        assert_eq!(split_mnemonic("&Скрыть"), ("Скрыть".into(), Some(0)));
        assert_eq!(split_mnemonic("R && X"), ("R & X".into(), None));
        assert_eq!(split_mnemonic("Плава&ющее"), ("Плавающее".into(), Some(5)));
        let e = vec![Entry::item("&Плавающее", true, Mark::None, 1), Entry::Sep, Entry::item("&Скрыть", true, Mark::None, 2)];
        assert_eq!((mnemonic_entry(&e, "с"), mnemonic_entry(&e, "С"), mnemonic_entry(&e, "x")), (Some(2), Some(2), None));
    }

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

    /// A menu that does not fit below its point opens upward from it, its bottom edge at the point and
    /// clamped to the screen's top (the original, on a clone at 125 %), at every scale; one that fits stays.
    #[test]
    fn a_menu_that_does_not_fit_below_opens_upward_from_the_point() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0));
        for k in [1.0f32, 1.25, 1.5] {
            let h = (22.0 * 9.0 * k).round();
            let size = Vec2::new(200.0 * k, h);
            let near = Pos2::new(300.0, 800.0 - h / 3.0);
            assert_eq!(menu_origin(near, size, screen), Pos2::new(300.0, near.y - h), "{k}");
            let fits = Pos2::new(300.0, 800.0 - h);
            assert_eq!(menu_origin(fits, size, screen), fits, "{k}: exactly fitting");
            let high = Pos2::new(40.0, 123.0);
            assert_eq!(menu_origin(high, size, screen), high, "{k}: plenty of room");
            let tall = Vec2::new(200.0, 900.0);
            assert_eq!(menu_origin(Pos2::new(5.0, 790.0), tall, screen).y, 0.0, "{k}: taller than the screen: the top");
        }
    }

    /// The same through an open menu: after its sizing pass the menu's bottom edge lies at the point.
    #[test]
    fn an_open_menu_near_the_screens_foot_ends_at_its_point() {
        let entries: Vec<Entry<u8>> = (0..12).map(|n| Entry::item(format!("item {n}"), true, Mark::None, n)).collect();
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0));
        let at = Pos2::new(400.0, 780.0);
        let mut m = PopMenu::new(1, at);
        let mut bottom = 0.0;
        for _ in 0..4 {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                show(ctx, "t", &mut m, &entries);
                bottom = ctx.memory(|mem| mem.area_rect(area_id("t", 1))).unwrap().bottom();
            });
        }
        assert!((bottom - at.y).abs() < 0.5, "the menu's bottom {bottom} at the point {}", at.y);
    }
}
