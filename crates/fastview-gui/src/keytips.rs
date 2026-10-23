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
        Target::AppItem(_) | Target::PrintMain => Pos2::new(r.left() + 32.0 + 4.0 - size.x, r.bottom() - size.y - 4.0),
        Target::PrintMenu => Pos2::new(r.right() - half(size.x), r.bottom() - size.y - 4.0),
        // A text-only menu row: at its left, centred on its middle (ours).
        Target::Recent(_) => Pos2::new(r.left() + 4.0, cy - half(size.y)),
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
        Level::Tabs => vec![("F", Target::AppButton), ("1", Target::Qat(0)), ("2", Target::Qat(1)), ("3", Target::Qat(2)), ("A", Target::Help)],
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

/// The keytip state machine (OnSysKeyDown / Up `0x100ff240` /
/// `0x100ff383`, the 200 ms timer `0x100fcc2b`, ProcessKey `0x100fee05`).
#[derive(Debug, Clone, Default)]
pub struct Keytips {
    /// The shown level, if any.
    pub level: Option<Level>,
    /// Shown by holding Alt, before the ribbon took the focus.
    by_hold: bool,
    /// When Alt went down, and whether another key came since.
    alt_since: Option<f64>,
    alt_clean: bool,
    /// The first letter of a longer keytip typed so far.
    prefix: Option<char>,
}

/// Alt held this long shows the tab-level keytips (timer 0xec1a).
pub const ALT_HOLD_S: f64 = 0.2;

impl Keytips {
    pub fn shown(&self) -> bool {
        self.level.is_some()
    }

    pub fn exit(&mut self) {
        self.level = None;
        self.by_hold = false;
        self.prefix = None;
    }

    fn show_tabs(&mut self, by_hold: bool) {
        self.level = Some(Level::Tabs);
        self.by_hold = by_hold;
        self.prefix = None;
    }

    /// One frame of the Alt key: `down` its state, `other` whether any
    /// other key was pressed this frame.
    pub fn alt(&mut self, down: bool, other: bool, now: f64) {
        match (self.alt_since, down) {
            (None, true) => {
                self.alt_since = Some(now);
                self.alt_clean = !other;
            }
            (Some(t), true) => {
                self.alt_clean &= !other;
                if self.alt_clean && self.level.is_none() && now - t >= ALT_HOLD_S {
                    self.show_tabs(true);
                }
            }
            (Some(_), false) => {
                self.alt_since = None;
                if self.alt_clean && !other {
                    // The release focuses the ribbon, which shows the
                    // keytips; already focused, it hands the focus back.
                    if self.level.is_some() && !self.by_hold {
                        self.exit();
                    } else {
                        self.show_tabs(false);
                    }
                } else if self.by_hold {
                    self.exit();
                }
            }
            (None, false) => {}
        }
    }

    /// F10 (without Shift or Ctrl) toggles the same way.
    pub fn f10(&mut self) {
        if self.level.is_some() && !self.by_hold {
            self.exit();
        } else {
            self.show_tabs(false);
        }
    }

    /// Esc: back to the parent level, out from the tab level.
    pub fn escape(&mut self) -> Outcome {
        match self.level {
            Some(Level::AppMenu) => {
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

    #[test]
    fn alt_tap_hold_and_f10() {
        let mut k = Keytips::default();
        k.alt(true, false, 0.0);
        k.alt(false, false, 0.05);
        assert_eq!(k.level, Some(Level::Tabs), "a tap focuses the ribbon");
        k.alt(true, false, 1.0);
        k.alt(false, false, 1.05);
        assert!(!k.shown(), "a second tap hands the focus back");
        k.alt(true, false, 2.0);
        k.alt(true, false, 2.1);
        assert!(!k.shown());
        k.alt(true, false, 2.25);
        assert!(k.shown(), "held 200 ms");
        k.alt(false, false, 2.3);
        assert!(k.shown(), "the release keeps them, focused");
        k.alt(true, false, 3.0);
        k.alt(true, true, 3.1);
        k.alt(false, false, 3.2);
        assert!(k.shown(), "Alt with another key: no toggle");
        k.f10();
        assert!(!k.shown());
        k.f10();
        assert!(k.shown());
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
