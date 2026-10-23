//! Ribbon keytips — MFC's `CMFCRibbonBar` keyboard navigation as the
//! original gets it (ledger L9.90). Only elements whose text carries keys
//! after a `\n` have a keytip (no automatic assignment): the application
//! button «F», the QAT buttons «1» «2» «3» (RebuildKeys numbers them), the
//! Help split button «A»; no tab has one, so a category's own keytips
//! (Вставить «V», Копировать «C») are never reached. In the application
//! menu the items take the letter after their `&` (Cyrillic — INFERRED) and
//! Печать «P» / «W»; typed keys are Latin whatever the layout (`0x100add31`),
//! so only P / W can be activated there.

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
    /// next key must be its second; anything else ends keytip mode.
    pub fn key(&mut self, c: char) -> Outcome {
        let Some(level) = self.level else { return Outcome::None };
        let c = c.to_uppercase().next().unwrap_or(c);
        let list = tips(level);
        if let Some(p) = self.prefix.take() {
            let hit = list.iter().find(|(k, _)| {
                let mut ch = k.chars().flat_map(char::to_uppercase);
                ch.next() == Some(p) && ch.next() == Some(c)
            });
            return match hit {
                Some(&(_, t)) => self.activate(t),
                None => {
                    self.exit();
                    Outcome::Exit
                }
            };
        }
        let up = |k: &str| k.to_uppercase();
        if let Some(&(_, t)) = list.iter().find(|(k, _)| up(k).chars().count() == 1 && up(k).starts_with(c)) {
            return self.activate(t);
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
        k.f10();
        assert_eq!(k.key('2'), Outcome::Activate(Target::Qat(1)));
        assert!(!k.shown(), "an activated button ends keytip mode");
        k.f10();
        assert_eq!(k.key('f'), Outcome::Activate(Target::AppButton));
        assert_eq!(k.level, Some(Level::AppMenu));
        assert_eq!(k.key('O'), Outcome::Exit, "Latin O is not the Cyrillic «О»");
        k.f10();
        k.key('F');
        assert_eq!(k.key('w'), Outcome::Activate(Target::PrintMenu));
        assert_eq!(k.level, Some(Level::AppMenu));
        assert_eq!(k.escape(), Outcome::Back);
        assert_eq!(k.level, Some(Level::Tabs));
        assert_eq!(k.escape(), Outcome::Exit);
        assert!(!k.shown());
        k.f10();
        assert_eq!(k.key('Q'), Outcome::Exit, "an unmatched key ends it");
    }
}
