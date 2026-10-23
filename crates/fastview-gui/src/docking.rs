//! The diagram panes as MFC docking panes (`CDockablePane`, ledger L9.92):
//! FastView creates all four with pane style 0xF — float, close, resize and
//! auto-hide allowed (`0x474e10`). Ours has two dockable units, the
//! Векторная | Круговая | Дифференциальная tab group (a double-click on a
//! grouped pane acts on the whole group) and Частотная.

use eframe::egui::{Pos2, Rect, Vec2};

/// The two dockable units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// Векторная | Круговая | Дифференциальная.
    Group,
    Freq,
}

/// Where a unit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Docked,
    Floating,
    /// Pinned off: a button on its edge's auto-hide strip.
    AutoHide,
}

/// A caption drag detaches past the system drag distance (SM_CXDRAG, 4 px)
/// once the pointer has left the pane (`0x10086f3a`).
pub const DRAG_PX: f32 = 4.0;
/// Hovering a strip button this long shows the pane (timer 0xec00).
pub const HOVER_S: f64 = 0.4;
/// The slid-out pane is checked this often and slides back when the
/// pointer is outside it and its button (`0x10087a4a`).
pub const CHECK_S: f64 = 0.7;
/// The slide moves 1/12 of the extent per step (`0x10088459`).
pub const SLIDE_STEPS: f32 = 12.0;

/// One dockable unit's state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dockable {
    pub place: Place,
    /// `RectRecentFloat` (`+0x1d8`): where it floats; None until it first
    /// floats — then the create rect at the frame's client top-left.
    pub float_rect: Option<Rect>,
    /// The create size (500 × 500, Частотная 200 × 200).
    pub create: Vec2,
    /// Auto-hide: shown (sliding out) or not, the slide 0…1, the hover
    /// start on its button and the next hide check.
    pub out: bool,
    pub slide: f32,
    hover_since: Option<f64>,
    next_check: f64,
}

impl Dockable {
    pub fn new(create: Vec2) -> Dockable {
        Dockable { place: Place::Docked, float_rect: None, create, out: false, slide: 0.0, hover_since: None, next_check: 0.0 }
    }

    /// The recent floating rect, or the create rect at `origin`.
    pub fn float_at(&self, origin: Pos2) -> Rect {
        self.float_rect.unwrap_or_else(|| Rect::from_min_size(origin, self.create))
    }

    /// A caption double-click (`0x100886dd`): floating → back to its dock,
    /// docked → floating at its recent floating rect; ignored auto-hidden.
    pub fn double_click(&mut self, origin: Pos2) {
        match self.place {
            Place::Docked => {
                self.float_rect = Some(self.float_at(origin));
                self.place = Place::Floating;
            }
            Place::Floating => self.place = Place::Docked,
            Place::AutoHide => {}
        }
    }

    /// The pin (`0x100871c7`): docked → auto-hide, auto-hide → docked.
    pub fn pin(&mut self) {
        self.place = match self.place {
            Place::Docked => Place::AutoHide,
            Place::AutoHide => Place::Docked,
            p => p,
        };
        (self.out, self.slide, self.hover_since) = (false, 0.0, None);
    }

    /// A caption drag of a docked unit: it detaches when the pointer has
    /// moved past [`DRAG_PX`] from `press` and left `pane`; then it floats
    /// with the caption still under the pointer. True when it detached.
    pub fn drag_detach(&mut self, press: Pos2, now: Pos2, pane: Rect, origin: Pos2) -> bool {
        if self.place != Place::Docked || (now - press).length() <= DRAG_PX || pane.contains(now) {
            return false;
        }
        let size = self.float_at(origin).size();
        let grab = Vec2::new((press.x - pane.left()).min(size.x - 20.0).max(0.0), (press.y - pane.top()).clamp(0.0, 20.0));
        self.float_rect = Some(Rect::from_min_size(now - grab, size));
        self.place = Place::Floating;
        true
    }

    /// One frame of an auto-hidden unit: hovering its button 400 ms or a
    /// click shows it; every 700 ms a shown one slides back when the pointer
    /// is outside it and its button (not while a menu is up); the slide
    /// moves 1/12 per frame. Returns true while it is moving.
    pub fn auto_hide_frame(&mut self, now: f64, on_button: bool, clicked: bool, in_pane: bool, menu_up: bool) -> bool {
        if self.place != Place::AutoHide {
            return false;
        }
        if on_button {
            let since = *self.hover_since.get_or_insert(now);
            if !self.out && (clicked || now - since >= HOVER_S) {
                self.out = true;
                self.next_check = now + CHECK_S;
            }
        } else {
            self.hover_since = None;
        }
        if self.out && now >= self.next_check {
            self.next_check = now + CHECK_S;
            if !in_pane && !on_button && !menu_up {
                self.out = false;
            }
        }
        let target = if self.out { 1.0 } else { 0.0 };
        let step = 1.0 / SLIDE_STEPS;
        self.slide = if self.slide < target { (self.slide + step).min(1.0) } else { (self.slide - step).max(0.0) };
        self.slide != target || (on_button && !self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit() -> Dockable {
        Dockable::new(Vec2::new(500.0, 500.0))
    }

    #[test]
    fn double_click_floats_and_redocks() {
        let mut d = unit();
        d.double_click(Pos2::new(10.0, 80.0));
        assert_eq!(d.place, Place::Floating);
        assert_eq!(d.float_rect, Some(Rect::from_min_size(Pos2::new(10.0, 80.0), Vec2::new(500.0, 500.0))), "first float: the create rect");
        d.float_rect = Some(Rect::from_min_size(Pos2::new(300.0, 300.0), Vec2::new(400.0, 200.0)));
        d.double_click(Pos2::ZERO);
        assert_eq!(d.place, Place::Docked);
        d.double_click(Pos2::ZERO);
        assert_eq!(d.float_rect.unwrap().min, Pos2::new(300.0, 300.0), "then its recent floating rect");
        d.place = Place::AutoHide;
        d.double_click(Pos2::ZERO);
        assert_eq!(d.place, Place::AutoHide, "ignored auto-hidden");
    }

    #[test]
    fn a_drag_detaches_past_the_threshold_outside_the_pane() {
        let pane = Rect::from_min_size(Pos2::new(800.0, 100.0), Vec2::new(500.0, 600.0));
        let mut d = unit();
        let press = Pos2::new(900.0, 110.0);
        assert!(!d.drag_detach(press, Pos2::new(903.0, 110.0), pane, Pos2::ZERO), "inside the drag distance");
        assert!(!d.drag_detach(press, Pos2::new(850.0, 110.0), pane, Pos2::ZERO), "still over the pane");
        assert!(d.drag_detach(press, Pos2::new(700.0, 105.0), pane, Pos2::ZERO));
        assert_eq!(d.place, Place::Floating);
        let r = d.float_rect.unwrap();
        assert!(r.contains(Pos2::new(700.0, 105.0)), "the caption stays under the pointer: {r:?}");
    }

    #[test]
    fn auto_hide_hover_slide_and_check() {
        let mut d = unit();
        d.pin();
        assert_eq!(d.place, Place::AutoHide);
        d.auto_hide_frame(0.0, true, false, false, false);
        d.auto_hide_frame(0.3, true, false, false, false);
        assert!(!d.out, "under 400 ms");
        d.auto_hide_frame(0.45, true, false, false, false);
        assert!(d.out);
        for k in 0..12 {
            d.auto_hide_frame(0.5 + f64::from(k) * 0.016, false, false, true, false);
        }
        assert_eq!(d.slide, 1.0, "out in 12 steps");
        // In the pane at the check: stays; outside at the next check: back.
        d.auto_hide_frame(1.2, false, false, true, false);
        assert!(d.out);
        d.auto_hide_frame(1.5, false, false, false, false);
        assert!(d.out, "not yet the next check");
        d.auto_hide_frame(1.95, false, false, false, false);
        assert!(!d.out);
        d.pin();
        assert_eq!((d.place, d.slide), (Place::Docked, 0.0));
    }
}
