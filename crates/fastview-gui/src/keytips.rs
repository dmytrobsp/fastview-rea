//! Ribbon keytips — MFC's `CMFCRibbonBar` keyboard navigation as the
//! original gets it (ledger L9.90). Only elements whose text carries keys
//! after a `\n` have a keytip (no automatic assignment): the application
//! button «F», the QAT buttons «1» «2» «3» (RebuildKeys numbers them), the
//! Help split button «A»; no tab has one, so a category's own keytips
//! (Вставить «V», Копировать «C») are never reached. In the application
//! menu the items take the letter after their `&` (Cyrillic — INFERRED) and
//! Печать «P» / «W»; typed keys are Latin whatever the layout (`0x100add31`),
//! so only P / W can be activated there.

use eframe::egui::{Color32, Pos2, Rect, Vec2};

/// A keytip level (`SetKeyboardNavigationLevel` `0x100fe087`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// The application button, the QAT and the tab-row elements.
    Tabs,
    /// Inside the open application menu.
    AppMenu,
    /// Inside MFC's print-preview category (its tab's «P»).
    Preview,
}

/// What a keytip stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    AppButton,
    /// QAT button `n` (Открыть, Сохранить, Печать).
    Qat(usize),
    Help,
    /// Application-menu item `n` (its row in the main panel).
    AppItem(usize),
    /// Печать's main part «P» and its menu part «W».
    PrintMain,
    PrintMenu,
    /// Recent document `n` (0-based): «&1»…«&9», «1&0» (`0x1011cb1e`).
    Recent(usize),
    /// Category tab `n`: «&Главная» gives «Г» (VM); «ОМП» has no `&`.
    Tab(usize),
    /// The print-preview category's tab (MFC: «p»).
    PreviewTab,
    /// A print-preview button.
    Preview(crate::print_ui::PreviewCmd),
}

/// The top-left of a keytip box of `size` on its element `r`
/// (`CMFCRibbonButton::GetKeyTipRect` `0x101033e0`; halves truncate):
/// the application button at its left + 10, centred on its middle; a QAT
/// button's box centred across it with its top at the button's middle; a
/// tab-row button (Help) at its left + 10, centred on its bottom edge; the
/// application menu's rows — large image, text always on the right — end
/// 4 px right of the 32 px icon and 4 px above the row's bottom, the menu
/// part's key centred on the row's right edge.
pub fn tip_origin(target: Target, r: Rect, size: Vec2) -> Pos2 {
    let half = |v: f32| (v / 2.0).trunc();
    let (cx, cy) = ((r.left() + r.right()) / 2.0, (r.top() + r.bottom()) / 2.0);
    let (cx, cy) = (cx.trunc(), cy.trunc());
    match target {
        Target::AppButton => Pos2::new(r.left() + 10.0, cy - half(size.y)),
        Target::Qat(_) => Pos2::new(cx - half(size.x), cy),
        Target::Help => Pos2::new(r.left() + 10.0, r.bottom() - half(size.y)),
        // A category tab: centred across it and on its bottom edge (VM).
        Target::Tab(_) => Pos2::new(cx - half(size.x), r.bottom() - half(size.y)),
        Target::AppItem(_) | Target::PrintMain => Pos2::new(r.left() + crate::dpi::img(32.0) + 4.0 - size.x, r.bottom() - size.y - 4.0),
        Target::PrintMenu => Pos2::new(r.right() - half(size.x), r.bottom() - size.y - 4.0),
        // A text-only menu row: at its left, centred on its middle (ours).
        Target::Recent(_) => Pos2::new(r.left() + 4.0, cy - half(size.y)),
        Target::PreviewTab => Pos2::new(cx - half(size.x), r.bottom() - half(size.y)),
        // The preview's large buttons: centred across, on the bottom edge;
        // the two small ones at their left + 10 on the top edge (VM).
        Target::Preview(c) => {
            use crate::print_ui::PreviewCmd as P;
            match c {
                P::Next | P::Prev => Pos2::new(r.left() + 10.0, r.top() - half(size.y)),
                _ => Pos2::new(cx - half(size.x), r.bottom() - half(size.y)),
            }
        }
    }
}

/// The keytip window is moved into the work area (`0x1011bbcb`: left /
/// right, then top / bottom).
pub fn into_screen(b: Rect, screen: Rect) -> Rect {
    let mut d = Vec2::ZERO;
    if b.right() > screen.right() {
        d.x = screen.right() - b.right();
    }
    if b.left() + d.x < screen.left() {
        d.x = screen.left() - b.left();
    }
    if b.bottom() > screen.bottom() {
        d.y = screen.bottom() - b.bottom();
    }
    if b.top() + d.y < screen.top() {
        d.y = screen.top() - b.top();
    }
    b.translate(d)
}

/// The Office 2007 visual manager's keytip box (`0x10194ae0`): the style's
/// `RIBBON_KEYTIP_BACK` bitmap — a 1 px border with clipped corners round
/// a vertical gradient — as (top, bottom, border); the text is the style's
/// tooltip text colour (76, 76, 76).
pub fn office2007_box(skin: crate::theme::Skin) -> Option<(Color32, Color32, Color32)> {
    use crate::theme::Skin;
    let c = Color32::from_rgb;
    Some(match skin {
        Skin::Blue => (c(251, 252, 253), c(208, 222, 241), c(118, 118, 118)),
        Skin::Black | Skin::Silver => (c(253, 253, 254), c(231, 231, 242), c(118, 118, 118)),
        Skin::Aqua => (c(254, 254, 254), c(208, 214, 226), c(134, 134, 134)),
        _ => return None,
    })
}

/// The keytips of a level, in the order they are scanned.
pub fn tips(level: Level) -> Vec<(&'static str, Target)> {
    match level {
        // «Г» is Cyrillic: no key types it (A–Z give Latin letters), so the
        // category level stays out of reach.
        // In print preview the category tab carries «P» (the others are not
        // shown: their keys drop out).
        Level::Tabs => vec![("F", Target::AppButton), ("1", Target::Qat(0)), ("2", Target::Qat(1)), ("3", Target::Qat(2)), ("Г", Target::Tab(0)), ("P", Target::PreviewTab), ("A", Target::Help)],
        // MFC's preview category (mfc120u's literals «p», «qi», «qo», «x»,
        // «v», «c»; the one / two pages button shows «2», VM).
        Level::Preview => {
            use crate::print_ui::PreviewCmd as P;
            vec![("P", Target::Preview(P::Print)), ("QI", Target::Preview(P::ZoomIn)), ("QO", Target::Preview(P::ZoomOut)), ("2", Target::Preview(P::NumPage)), ("X", Target::Preview(P::Next)), ("V", Target::Preview(P::Prev)), ("C", Target::Preview(P::Close))]
        }
        // Rows: Открыть, Сохранить, Сохранить как, Проводник, Печать,
        // Параметры файла, Комментарии, Загрузка настроек, Настройки,
        // Закрыть (Проводник and Комментарии have no `&`).
        Level::AppMenu => vec![
            ("О", Target::AppItem(0)),
            ("С", Target::AppItem(1)),
            ("К", Target::AppItem(2)),
            ("P", Target::PrintMain),
            ("W", Target::PrintMenu),
            ("П", Target::AppItem(5)),
            ("Н", Target::AppItem(7)),
            ("А", Target::AppItem(8)),
            ("З", Target::AppItem(9)),
            // The recent documents take the letter after their «&»
            // (`GetKeyTipSize` `0x10064ea3`).
            ("1", Target::Recent(0)),
            ("2", Target::Recent(1)),
            ("3", Target::Recent(2)),
            ("4", Target::Recent(3)),
            ("5", Target::Recent(4)),
            ("6", Target::Recent(5)),
            ("7", Target::Recent(6)),
            ("8", Target::Recent(7)),
            ("9", Target::Recent(8)),
            ("0", Target::Recent(9)),
        ],
    }
}

/// The result of a key while keytips are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing (a narrowing first letter).
    None,
    Activate(Target),
    /// Esc inside a level: back to its parent.
    Back,
    /// Keytip mode ends.
    Exit,
}

/// The keytip state machine (OnSysKeyDown `0x100ff240`, ProcessKey
/// `0x100fee05`). The ribbon's Alt handling (a tap focuses the ribbon, a
/// 200 ms hold shows the keytips) never runs: the application's
/// PreTranslateMessage `0x448e80` discards every WM_SYSKEYDOWN / UP of
/// VK_MENU first, so only F10 shows them.
#[derive(Debug, Clone, Default)]
pub struct Keytips {
    /// The shown level, if any.
    pub level: Option<Level>,
    /// The first letter of a longer keytip typed so far.
    prefix: Option<char>,
}

impl Keytips {
    pub fn shown(&self) -> bool {
        self.level.is_some()
    }

    pub fn exit(&mut self) {
        self.level = None;
        self.prefix = None;
    }

    fn show_tabs(&mut self) {
        self.level = Some(Level::Tabs);
        self.prefix = None;
    }

    /// F10 (without Shift or Ctrl) focuses the ribbon, showing the
    /// tab-level keytips; focused, it hands the focus back.
    pub fn f10(&mut self) {
        if self.level.is_some() {
            self.exit();
        } else {
            self.show_tabs();
        }
    }

    /// Esc: back to the parent level, out from the tab level.
    pub fn escape(&mut self) -> Outcome {
        match self.level {
            Some(Level::AppMenu | Level::Preview) => {
                self.level = Some(Level::Tabs);
                self.prefix = None;
                Outcome::Back
            }
            _ => {
                self.exit();
                Outcome::Exit
            }
        }
    }

    /// A typed character (upper-cased Latin): the first one-key keytip that
    /// matches activates; the first letter of a longer one narrows, the
    /// next key must be its second; anything else ends keytip mode. `avail`
    /// says whether a target is there (`Some`) and enabled: a disabled
    /// element's key does nothing and the keytips stay (`OnKey`
    /// `0x101036b6` returns FALSE, ProcessKey `0x100fef15` stays).
    pub fn key(&mut self, c: char, avail: impl Fn(Target) -> Option<bool>) -> Outcome {
        let Some(level) = self.level else { return Outcome::None };
        let c = c.to_uppercase().next().unwrap_or(c);
        let list: Vec<(&str, Target)> = tips(level).into_iter().filter(|&(_, t)| avail(t).is_some()).collect();
        let activate = |k: &mut Keytips, t: Target| if avail(t) == Some(true) { k.activate(t) } else { Outcome::None };
        if let Some(p) = self.prefix.take() {
            let hit = list.iter().find(|(k, _)| {
                let mut ch = k.chars().flat_map(char::to_uppercase);
                ch.next() == Some(p) && ch.next() == Some(c)
            });
            return match hit {
                Some(&(_, t)) => activate(self, t),
                None => {
                    self.exit();
                    Outcome::Exit
                }
            };
        }
        let up = |k: &str| k.to_uppercase();
        if let Some(&(_, t)) = list.iter().find(|(k, _)| up(k).chars().count() == 1 && up(k).starts_with(c)) {
            return activate(self, t);
        }
        if list.iter().any(|(k, _)| up(k).chars().count() > 1 && up(k).starts_with(c)) {
            self.prefix = Some(c);
            return Outcome::None;
        }
        self.exit();
        Outcome::Exit
    }

    fn activate(&mut self, t: Target) -> Outcome {
        match t {
            // The application button opens its menu and descends into it.
            Target::AppButton => self.level = Some(Level::AppMenu),
            // The preview's tab descends into its category.
            Target::PreviewTab => {
                self.level = Some(Level::Preview);
                self.prefix = None;
            }
            // The Печать menu part opens its submenu; the level stays.
            Target::PrintMenu => {}
            _ => self.exit(),
        }
        Outcome::Activate(t)
    }

    /// The text a keytip shows (upper-cased, `0x1017b177`).
    pub fn label(keys: &str) -> String {
        keys.to_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `0x101033e0` per element, `0x1011bbcb` into the work area.
    #[test]
    fn keytip_boxes_sit_per_element_type() {
        let size = Vec2::new(18.0, 17.0);
        let r = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(60.0, 22.0));
        assert_eq!(tip_origin(Target::AppButton, r, size), Pos2::new(10.0, 3.0));
        assert_eq!(tip_origin(Target::Qat(0), r, size), Pos2::new(21.0, 11.0));
        assert_eq!(tip_origin(Target::Help, r, size), Pos2::new(10.0, 14.0));
        let row = Rect::from_min_max(Pos2::new(100.0, 50.0), Pos2::new(300.0, 90.0));
        assert_eq!(tip_origin(Target::AppItem(0), row, size), Pos2::new(118.0, 69.0), "ends 4 px right of the 32 px icon");
        assert_eq!(tip_origin(Target::PrintMenu, row, size), Pos2::new(291.0, 69.0));
        let screen = Rect::from_min_max(Pos2::ZERO, Pos2::new(800.0, 600.0));
        let b = Rect::from_min_size(Pos2::new(790.0, -5.0), size);
        assert_eq!(into_screen(b, screen).min, Pos2::new(782.0, 0.0));
    }

    /// In print preview: F10, then «P» descends into the category, whose
    /// keys run its buttons; Esc goes back to the tabs.
    #[test]
    fn the_preview_category_has_its_own_level() {
        use crate::print_ui::PreviewCmd as P;
        let mut k = Keytips::default();
        // Only the preview's elements are there.
        let preview = |t: Target| match t {
            Target::Tab(_) => None,
            Target::Preview(P::ZoomOut) => Some(false),
            _ => Some(true),
        };
        k.f10();
        assert_eq!(k.key('p', preview), Outcome::Activate(Target::PreviewTab));
        assert_eq!(k.level, Some(Level::Preview));
        assert_eq!(k.key('Q', preview), Outcome::None, "the first of «QI» / «QO»");
        assert_eq!(k.key('O', preview), Outcome::None, "Zoom Out disabled: nothing, the keytips stay");
        assert_eq!(k.level, Some(Level::Preview));
        assert_eq!(k.key('Q', preview), Outcome::None);
        assert_eq!(k.key('I', preview), Outcome::Activate(Target::Preview(P::ZoomIn)));
        assert!(!k.shown());
        k.f10();
        k.key('P', preview);
        assert_eq!(k.escape(), Outcome::Back);
        assert_eq!(k.level, Some(Level::Tabs));
        k.key('P', preview);
        assert_eq!(k.key('2', preview), Outcome::Activate(Target::Preview(P::NumPage)));
    }

    #[test]
    fn f10_toggles() {
        let mut k = Keytips::default();
        k.f10();
        assert_eq!(k.level, Some(Level::Tabs));
        k.f10();
        assert!(!k.shown());
    }

    #[test]
    fn typing_activates_descends_and_backs_out() {
        let mut k = Keytips::default();
        let all = |_: Target| Some(true);
        k.f10();
        assert_eq!(k.key('2', |t| Some(t != Target::Qat(1))), Outcome::None, "disabled: nothing, the keytips stay");
        assert!(k.shown());
        assert_eq!(k.key('2', all), Outcome::Activate(Target::Qat(1)));
        assert!(!k.shown(), "an activated button ends keytip mode");
        k.f10();
        assert_eq!(k.key('f', all), Outcome::Activate(Target::AppButton));
        assert_eq!(k.level, Some(Level::AppMenu));
        assert_eq!(k.key('O', all), Outcome::Exit, "Latin O is not the Cyrillic «О»");
        k.f10();
        k.key('F', all);
        assert_eq!(k.key('w', all), Outcome::Activate(Target::PrintMenu));
        assert_eq!(k.level, Some(Level::AppMenu));
        assert_eq!(k.escape(), Outcome::Back);
        assert_eq!(k.level, Some(Level::Tabs));
        assert_eq!(k.escape(), Outcome::Exit);
        assert!(!k.shown());
        k.f10();
        assert_eq!(k.key('Q', all), Outcome::Exit, "an unmatched key ends it");
        // The recent documents: 1…9, 0 for the tenth; absent ones unmatched.
        k.f10();
        k.key('F', all);
        let two = |t: Target| match t {
            Target::Recent(n) => (n < 2).then_some(true),
            _ => Some(true),
        };
        assert_eq!(k.key('2', two), Outcome::Activate(Target::Recent(1)));
        k.f10();
        k.key('F', all);
        assert_eq!(k.key('5', two), Outcome::Exit);
    }
}
