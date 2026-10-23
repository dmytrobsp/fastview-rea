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

impl Unit {
    pub fn other(self) -> Unit {
        match self {
            Unit::Group => Unit::Freq,
            Unit::Freq => Unit::Group,
        }
    }
}

/// A frame edge, or a side of a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];

    /// Left / right edges hold a column (their extent is a width).
    pub fn vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }

    pub fn opposite(self) -> Edge {
        match self {
            Edge::Left => Edge::Right,
            Edge::Right => Edge::Left,
            Edge::Top => Edge::Bottom,
            Edge::Bottom => Edge::Top,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Edge::Left => "left",
            Edge::Right => "right",
            Edge::Top => "top",
            Edge::Bottom => "bottom",
        }
    }

    fn from_name(s: &str) -> Option<Edge> {
        Edge::ALL.into_iter().find(|e| e.name() == s)
    }
}

/// Where a docked unit sits (smart docking's drops, L9.101).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    /// Its own container on a frame edge — `outer` when an edge marker put
    /// it there (outermost), else the document area's group (innermost).
    Edge { edge: Edge, outer: bool },
    /// Beside the other unit, inside its container, on that side of it.
    Beside(Edge),
    /// A tab of the other unit's tabbed group.
    Tab,
}

/// What a smart-docking marker does on the drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drop {
    /// A frame-edge marker (outer) or a side arrow over the document area.
    Edge(Edge, bool),
    /// A side arrow of the five-way group over the other unit.
    Beside(Edge),
    /// The five-way group's centre: tab into the other unit.
    Tab,
}

/// The VS2008 smart-docking markers (`0x1013444f`; bitmaps 17117…17126):
/// side markers 36 × 35 (left / right) and 35 × 36 (top / bottom), the
/// centre 31 × 30, the five-way group 110 × 110.
pub const MARKER_LR: Vec2 = Vec2::new(36.0, 35.0);
pub const MARKER_TB: Vec2 = Vec2::new(35.0, 36.0);
pub const MARKER_CENTRE: Vec2 = Vec2::new(31.0, 30.0);
pub const GROUP_SIDE: f32 = 110.0;

fn marker_size(e: Edge) -> Vec2 {
    if e.vertical() {
        MARKER_LR
    } else {
        MARKER_TB
    }
}

/// The four edge markers (`AdjustPos` `0x101347e0`): 16 px in from the
/// frame client's edges, centred along them; FastView allows all four
/// (`EnableAutoHidePanes(0xF000)`).
pub fn edge_markers(client: Rect) -> [(Rect, Drop); 4] {
    let (cx, cy) = ((client.left() + client.right()) / 2.0, (client.top() + client.bottom()) / 2.0);
    Edge::ALL.map(|e| {
        let s = marker_size(e);
        let (hw, hh) = ((s.x / 2.0).trunc(), (s.y / 2.0).trunc());
        let min = match e {
            Edge::Left => Pos2::new(client.left() + 16.0, cy - hh),
            Edge::Right => Pos2::new(client.right() - s.x - 16.0, cy - hh),
            Edge::Top => Pos2::new(cx - hw, client.top() + 16.0),
            Edge::Bottom => Pos2::new(cx - hw, client.bottom() - s.y - 16.0),
        };
        (Rect::from_min_size(min, s), Drop::Edge(e, true))
    })
}

/// The five-way group centred on `host` (`0x101350f5`, `0x1013598a`):
/// side markers at the 110 px square's edges, the centre in its middle.
/// Over the other unit the sides dock beside it and the centre tabs into
/// it; over the document area the sides dock on that
/// frame edge innermost and there is no centre (`over_pane` false).
pub fn five_way(host: Rect, over_pane: bool) -> Vec<(Rect, Drop)> {
    let t = GROUP_SIDE;
    let o = Pos2::new(((host.left() + host.right() - t) / 2.0).trunc(), ((host.top() + host.bottom() - t) / 2.0).trunc());
    let mut v: Vec<(Rect, Drop)> = Edge::ALL
        .iter()
        .map(|&e| {
            let s = marker_size(e);
            let at = match e {
                Edge::Left => Vec2::new(0.0, ((t - s.y) / 2.0).trunc()),
                Edge::Right => Vec2::new(t - s.x, ((t - s.y) / 2.0).trunc()),
                Edge::Top => Vec2::new(((t - s.x) / 2.0).trunc(), 0.0),
                Edge::Bottom => Vec2::new(((t - s.x) / 2.0).trunc(), t - s.y),
            };
            (Rect::from_min_size(o + at, s), if over_pane { Drop::Beside(e) } else { Drop::Edge(e, false) })
        })
        .collect();
    if over_pane {
        let c = MARKER_CENTRE;
        v.push((Rect::from_min_size(o + Vec2::new(((t - c.x) / 2.0).trunc(), ((t - c.y) / 2.0).trunc()), c), Drop::Tab));
    }
    v
}

/// The marker under the pointer: the centre first, then the sides, then
/// the edges (`0x10136b50` tests from the centre down).
pub fn hit_marker(markers: &[(Rect, Drop)], p: Pos2) -> Option<Drop> {
    let rank = |d: &Drop| match d {
        Drop::Tab => 0,
        Drop::Beside(_) | Drop::Edge(_, false) => 1,
        Drop::Edge(_, true) => 2,
    };
    let mut hit: Vec<&(Rect, Drop)> = markers.iter().filter(|(r, _)| r.contains(p)).collect();
    hit.sort_by_key(|(_, d)| rank(d));
    hit.first().map(|(_, d)| *d)
}

/// Dock `unit` by `drop` relative to `other` (beside / into it only while
/// it is docked in its own container), `extent` its docked size
/// ([`dock_extent`]).
pub fn drop_on(unit: &mut Dockable, other: &Dockable, drop: Drop, extent: f32) {
    let in_container = other.place == Place::Docked && matches!(other.spot, Spot::Edge { .. });
    unit.spot = match drop {
        Drop::Edge(edge, outer) => Spot::Edge { edge, outer },
        Drop::Beside(e) if in_container => Spot::Beside(e),
        Drop::Tab if in_container => Spot::Tab,
        _ => return,
    };
    unit.place = Place::Docked;
    if drop != Drop::Tab {
        (unit.extent, unit.gen) = (Some(extent), unit.gen + 1);
    }
}

/// The docked size a drop gives, along the drop's axis
/// (`CalcExpectedDockedRect` `0x1008c45d`, `DockToFrameWindow`
/// `0x10088f61`, the split `0x100d35e8`): on a frame edge the floating
/// size capped at half the area (`AdjustRectToClientArea` `0x1008c750`,
/// 50 %); beside a pane min(the floating size, half the pane). `size` the
/// floating pane's, `area` the frame client's or the target pane's.
pub fn dock_extent(drop: Drop, size: Vec2, area: Rect) -> f32 {
    match drop {
        Drop::Edge(e, _) | Drop::Beside(e) => {
            let (s, a) = (if e.vertical() { size.x } else { size.y }, if e.vertical() { area.width() } else { area.height() });
            s.min(a / 2.0)
        }
        Drop::Tab => 0.0,
    }
}

/// `unit` leaves its container (floats or is pinned off): a unit beside it
/// takes the container, and `unit` keeps the mirrored place beside that
/// one, so a redock rebuilds the same container (MFC's recent dock info).
/// A unit tabbed into it goes along (the tabbed pane moves whole).
pub fn leave(unit: &mut Dockable, other: &mut Dockable) {
    if let (Spot::Edge { edge, .. }, Spot::Beside(side)) = (unit.spot, other.spot) {
        // The container keeps its size; `unit` keeps the share `other` had.
        let size = |d: &Dockable, vertical: bool| d.extent.unwrap_or(if vertical { d.create.x } else { d.create.y });
        let (container, share) = (size(unit, edge.vertical()), size(other, side.vertical()));
        other.spot = unit.spot;
        unit.spot = Spot::Beside(side.opposite());
        (other.extent, other.gen) = (Some(container), other.gen + 1);
        (unit.extent, unit.gen) = (Some(share), unit.gen + 1);
    }
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
    /// Where it sits while docked.
    pub spot: Spot,
    /// Its docked size across its edge (or its share beside the other)
    /// from the last drop; `None` the create size. `gen` counts the drops
    /// (a new panel takes the new size).
    pub extent: Option<f32>,
    pub gen: u32,
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
    pub fn new(create: Vec2, spot: Spot) -> Dockable {
        Dockable { place: Place::Docked, spot, extent: None, gen: 0, float_rect: None, create, out: false, slide: 0.0, hover_since: None, next_check: 0.0 }
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

/// The Feature Pack's workspace (`CWinAppEx::SaveState` `0x101a9425` when
/// the main frame closes, `LoadState` `0x101a8eea` when it loads —
/// `m_bSaveState` is on and FastView never clears it): each pane's place
/// and recent floating rect under `Workspace/<unit>/…`.
pub fn store(unit: &str, d: &Dockable, v: &mut std::collections::HashMap<String, String>) {
    let place = match d.place {
        Place::Docked => "docked",
        Place::Floating => "floating",
        Place::AutoHide => "autohide",
    };
    v.insert(format!("Workspace/{unit}/Place"), place.into());
    let spot = match d.spot {
        Spot::Edge { edge, outer } => format!("edge {} {}", edge.name(), if outer { "outer" } else { "inner" }),
        Spot::Beside(e) => format!("beside {}", e.name()),
        Spot::Tab => "tab".into(),
    };
    v.insert(format!("Workspace/{unit}/Spot"), spot);
    match d.extent {
        Some(e) => v.insert(format!("Workspace/{unit}/RecentDockedSize"), e.to_string()),
        None => v.remove(&format!("Workspace/{unit}/RecentDockedSize")),
    };
    match d.float_rect {
        Some(r) => v.insert(format!("Workspace/{unit}/RectRecentFloat"), format!("{} {} {} {}", r.left(), r.top(), r.right(), r.bottom())),
        None => v.remove(&format!("Workspace/{unit}/RectRecentFloat")),
    };
}

/// Read back what [`store`] wrote (an auto-hidden pane comes back slid in).
pub fn restore(unit: &str, d: &mut Dockable, v: &std::collections::HashMap<String, String>) {
    d.place = match v.get(&format!("Workspace/{unit}/Place")).map(String::as_str) {
        Some("floating") => Place::Floating,
        Some("autohide") => Place::AutoHide,
        _ => Place::Docked,
    };
    d.float_rect = v.get(&format!("Workspace/{unit}/RectRecentFloat")).and_then(|s| parse_rect(s));
    d.extent = v.get(&format!("Workspace/{unit}/RecentDockedSize")).and_then(|s| s.parse().ok());
    let words: Vec<&str> = v.get(&format!("Workspace/{unit}/Spot")).map_or(Vec::new(), |s| s.split_whitespace().collect());
    match words[..] {
        ["edge", e, o] => {
            if let Some(edge) = Edge::from_name(e) {
                d.spot = Spot::Edge { edge, outer: o == "outer" };
            }
        }
        ["beside", e] => {
            if let Some(e) = Edge::from_name(e) {
                d.spot = Spot::Beside(e);
            }
        }
        ["tab"] => d.spot = Spot::Tab,
        _ => {}
    }
    if d.place == Place::Floating && d.float_rect.is_none() {
        d.place = Place::Docked;
    }
}

/// "l t r b".
pub fn parse_rect(s: &str) -> Option<Rect> {
    let n: Vec<f32> = s.split_whitespace().map(str::parse).collect::<Result<_, _>>().ok()?;
    let [l, t, r, b] = n[..] else { return None };
    (r > l && b > t).then(|| Rect::from_min_max(Pos2::new(l, t), Pos2::new(r, b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_round_trip() {
        let mut v = std::collections::HashMap::new();
        let mut d = unit();
        d.double_click(Pos2::new(40.0, 60.0));
        store("Group", &d, &mut v);
        let mut back = unit();
        restore("Group", &mut back, &v);
        assert_eq!((back.place, back.float_rect), (Place::Floating, d.float_rect));
        d.double_click(Pos2::ZERO);
        d.pin();
        store("Group", &d, &mut v);
        restore("Group", &mut back, &v);
        assert_eq!((back.place, back.out), (Place::AutoHide, false));
        assert_eq!(back.float_rect, d.float_rect, "the recent floating rect is kept docked too");
        restore("Freq", &mut back, &v);
        assert_eq!((back.place, back.float_rect), (Place::Docked, None), "nothing stored: the defaults");
    }

    fn unit() -> Dockable {
        Dockable::new(Vec2::new(500.0, 500.0), Spot::Edge { edge: Edge::Right, outer: true })
    }

    /// `0x101347e0` / `0x101350f5`: the markers' places; the hot one.
    #[test]
    fn smart_dock_markers_and_drops() {
        let client = Rect::from_min_max(Pos2::new(0.0, 100.0), Pos2::new(1000.0, 700.0));
        let e = edge_markers(client);
        assert_eq!(e[0].0, Rect::from_min_size(Pos2::new(16.0, 383.0), MARKER_LR), "left: 16 px in, centred");
        assert_eq!(e[1].0.min, Pos2::new(1000.0 - 36.0 - 16.0, 383.0));
        assert_eq!(e[2].0.min, Pos2::new(483.0, 116.0));
        assert_eq!(e[3].0.min, Pos2::new(483.0, 700.0 - 36.0 - 16.0));
        let host = Rect::from_min_max(Pos2::new(600.0, 100.0), Pos2::new(1000.0, 500.0));
        let g = five_way(host, true);
        // The square at (745, 245): left (0, 37), top (37, 0), centre (39, 40).
        assert_eq!(g[0].0.min, Pos2::new(745.0, 282.0));
        assert_eq!(g[2].0.min, Pos2::new(782.0, 245.0));
        assert_eq!(g[4], (Rect::from_min_size(Pos2::new(784.0, 285.0), MARKER_CENTRE), Drop::Tab));
        assert_eq!(hit_marker(&g, Pos2::new(800.0, 300.0)), Some(Drop::Tab));
        assert_eq!(hit_marker(&g, Pos2::new(750.0, 300.0)), Some(Drop::Beside(Edge::Left)));
        assert_eq!(five_way(host, false).len(), 4, "over the documents: no centre");
        assert_eq!(five_way(host, false)[1].1, Drop::Edge(Edge::Right, false));

        // Freq beside the group; the group floats: Freq takes its column.
        let mut group = unit();
        let mut freq = Dockable::new(Vec2::new(200.0, 200.0), Spot::Beside(Edge::Bottom));
        group.place = Place::Floating;
        leave(&mut group, &mut freq);
        assert_eq!(freq.spot, Spot::Edge { edge: Edge::Right, outer: true });
        assert_eq!(group.spot, Spot::Beside(Edge::Top), "a redock goes back above it");
        assert_eq!((freq.extent, group.extent), (Some(500.0), Some(200.0)), "the column keeps its width");
        // Dropped into Freq's tabs.
        drop_on(&mut group, &freq, Drop::Tab, 0.0);
        assert_eq!((group.place, group.spot), (Place::Docked, Spot::Tab));
        drop_on(&mut group, &freq, Drop::Edge(Edge::Left, true), 400.0);
        assert_eq!((group.spot, group.extent, group.gen), (Spot::Edge { edge: Edge::Left, outer: true }, Some(400.0), 2));
        // Sizes: the floating width, at most half the frame; beside, at
        // most half the pane.
        let frame = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0));
        assert_eq!(dock_extent(Drop::Edge(Edge::Left, true), Vec2::new(420.0, 300.0), frame), 420.0);
        assert_eq!(dock_extent(Drop::Edge(Edge::Right, true), Vec2::new(700.0, 300.0), frame), 500.0);
        assert_eq!(dock_extent(Drop::Edge(Edge::Bottom, false), Vec2::new(700.0, 200.0), frame), 200.0);
        assert_eq!(dock_extent(Drop::Beside(Edge::Top), Vec2::new(300.0, 400.0), frame), 300.0);
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
