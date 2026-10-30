//! The diagram panes as MFC docking panes (`CDockablePane`, ledger L9.92):
//! FastView creates all four with pane style 0xF — float, close, resize and
//! auto-hide allowed (`0x474e10`). Each pane is a dockable unit; a unit
//! docks on a frame edge, beside another (splitting its slot) or as a tab
//! of another's tabbed group (a caption acts on the whole group, a tab
//! pulled out of the strip floats that pane alone).

use eframe::egui::{Pos2, Rect, Vec2};

/// A dockable unit: one diagram pane.
pub use crate::app::Pane as Unit;

pub const UNITS: [Unit; 4] = [Unit::Vector, Unit::Circle, Unit::Differential, Unit::Frequency];

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
    /// Beside that unit, splitting its slot in its container, on that side
    /// of it (`DockToWindow` `0x100891c3`).
    Beside(Unit, Edge),
    /// A tab of that unit's tabbed group (`AttachToTabWnd` `0x1008a390`).
    Tab(Unit),
}

/// What a smart-docking marker does on the drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drop {
    /// A frame-edge marker (outer) or a side arrow over the document area.
    Edge(Edge, bool),
    /// A side arrow of the five-way group over a docked pane.
    Beside(Edge),
    /// The five-way group's centre: tab into that pane.
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
/// Over a docked pane the sides dock beside it and the centre tabs into
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

/// A block's size when it docks at its create size (`AdjustRectToClientArea`
/// `0x1008c750`): at least half the documents' area `span` across its edge
/// → that half, rounded down (the docking manager's 50 %).
pub fn start_extent(size: f32, span: f32) -> f32 {
    let allowed = (span / 2.0).floor();
    if size >= allowed {
        allowed
    } else {
        size
    }
}

/// Swap two tabs of a strip's order (`0x100690b4`).
pub fn swap_tabs<T: PartialEq + Copy>(order: &mut [T], a: T, b: T) {
    if let (Some(i), Some(j)) = (order.iter().position(|&x| x == a), order.iter().position(|&x| x == b)) {
        order.swap(i, j);
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
    /// Its block's place in the docking manager's list (L9.109): blocks
    /// are laid out in ascending order, each spanning the space the
    /// earlier ones left. An outer-edge drop goes before every pane block
    /// (`AddPane` `0x1008b521`), an inner one after them (`0x10061335`).
    pub order: i32,
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
        Dockable { place: Place::Docked, spot, extent: None, gen: 0, order: 0, float_rect: None, create, out: false, slide: 0.0, hover_since: None, next_check: 0.0 }
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

    /// Its panes hidden: an auto-hidden one is back in its strip.
    pub fn hide(&mut self) {
        (self.out, self.slide, self.hover_since) = (false, 0.0, None);
    }

    /// When it next needs a frame, in seconds from `now`: at once while it
    /// slides, when the hover on its button reaches the open time, at the
    /// next hide check while it is out; None when resting in.
    pub fn wake_in(&self, now: f64) -> Option<f64> {
        if self.place != Place::AutoHide {
            return None;
        }
        if self.slide != if self.out { 1.0 } else { 0.0 } {
            return Some(0.0);
        }
        let at = if self.out { Some(self.next_check) } else { self.hover_since.map(|s| s + HOVER_S) };
        at.map(|t| (t - now).max(0.0))
    }

    /// Whether the pointer was on its strip button in the last frame.
    #[cfg(test)]
    pub fn on_button(&self) -> bool {
        self.hover_since.is_some()
    }

    /// One frame of an auto-hidden unit: hovering its button 400 ms or a
    /// click shows it; every 700 ms a shown one slides back when the pointer
    /// is outside it and its button (not while `held`: a menu up or the
    /// focus in the pane); the slide moves 1/12 per frame.
    pub fn auto_hide_frame(&mut self, now: f64, on_button: bool, clicked: bool, in_pane: bool, held: bool) {
        if self.place != Place::AutoHide {
            return;
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
            if !in_pane && !on_button && !held {
                self.out = false;
            }
        }
        let target = if self.out { 1.0 } else { 0.0 };
        let step = 1.0 / SLIDE_STEPS;
        // Fully out or in, it rests there.
        if self.slide < target {
            self.slide = (self.slide + step).min(1.0);
        } else if self.slide > target {
            self.slide = (self.slide - step).max(0.0);
        }
    }
}

/// The four units' docking state, indexed by pane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Docks(pub [Dockable; 4]);

impl std::ops::Index<Unit> for Docks {
    type Output = Dockable;
    fn index(&self, u: Unit) -> &Dockable {
        &self.0[u as usize]
    }
}

impl std::ops::IndexMut<Unit> for Docks {
    fn index_mut(&mut self, u: Unit) -> &mut Dockable {
        &mut self.0[u as usize]
    }
}

impl Default for Docks {
    /// The start-up docking (`0x474e10`, seen running on the VM with no
    /// saved workspace, L9.165): Векторная docked on the right first,
    /// Круговая attached to it as a tab, Дифференциальная docked on the right
    /// next (so inside it), Частотная (200 tall) on the bottom last —
    /// spanning only what the two right blocks leave.
    fn default() -> Docks {
        let big = Vec2::splat(500.0);
        let edge = |e: Edge, size: Vec2, order: i32| Dockable { order, ..Dockable::new(size, Spot::Edge { edge: e, outer: true }) };
        Docks([
            edge(Edge::Right, big, 0),
            Dockable::new(big, Spot::Tab(Unit::Vector)),
            edge(Edge::Right, big, 1),
            edge(Edge::Bottom, Vec2::splat(200.0), 2),
        ])
    }
}

impl Docks {
    /// One tabbed group of three on the right and Частотная below it — the
    /// layout the docking tests start from.
    #[cfg(test)]
    pub fn one_group() -> Docks {
        let big = Vec2::splat(500.0);
        Docks([
            Dockable::new(big, Spot::Edge { edge: Edge::Right, outer: true }),
            Dockable::new(big, Spot::Tab(Unit::Vector)),
            Dockable::new(big, Spot::Tab(Unit::Vector)),
            Dockable::new(Vec2::splat(200.0), Spot::Beside(Unit::Vector, Edge::Bottom)),
        ])
    }

    /// The unit whose tabbed group `u` shows in: a docked tab follows its
    /// host wherever that is (a tab pulled out floats alone).
    pub fn host(&self, u: Unit) -> Unit {
        let mut x = u;
        for _ in 0..UNITS.len() {
            match self[x] {
                Dockable { place: Place::Docked, spot: Spot::Tab(h), .. } if h != u => x = h,
                _ => return x,
            }
        }
        u
    }

    /// `root`'s tabbed group, `root` among them.
    pub fn members(&self, root: Unit) -> impl Iterator<Item = Unit> + '_ {
        UNITS.into_iter().filter(move |&u| self.host(u) == root)
    }

    /// The frame edge `u`'s container is on (its own, or the unit's it
    /// sits beside / in).
    pub fn edge(&self, u: Unit) -> Edge {
        let mut x = u;
        for _ in 0..UNITS.len() {
            match self[x].spot {
                Spot::Edge { edge, .. } => return edge,
                Spot::Beside(t, _) | Spot::Tab(t) => x = t,
            }
        }
        Edge::Right
    }

    /// The group whose slot the docked group `u` splits, when that one
    /// shows docked (`shown`: its group has a visible pane); none — `u` is
    /// a container of its own — otherwise, or on a loop.
    pub fn parent(&self, u: Unit, shown: impl Fn(Unit) -> bool) -> Option<Unit> {
        let step = |x: Unit| match self[x] {
            Dockable { place: Place::Docked, spot: Spot::Beside(t, _), .. } => {
                let p = self.host(t);
                (p != x && self[p].place == Place::Docked && shown(p)).then_some(p)
            }
            _ => None,
        };
        let p = step(u)?;
        let mut q = Some(p);
        for _ in 0..UNITS.len() {
            match q {
                Some(x) if x == u => return None,
                Some(x) => q = step(x),
                None => return Some(p),
            }
        }
        None
    }

    /// Dock `u` by `drop` on `target` (a docked group; beside / into it),
    /// `extent` its docked size ([`dock_extent`]). A group tabbed in brings
    /// its tabs along (`AttachToTabWnd` moves every tab).
    pub fn drop_on(&mut self, u: Unit, drop: Drop, target: Option<Unit>, extent: f32) {
        let spot = match (drop, target) {
            (Drop::Edge(edge, outer), _) => Spot::Edge { edge, outer },
            (Drop::Beside(e), Some(t)) if t != u => Spot::Beside(t, e),
            (Drop::Tab, Some(t)) if t != u => Spot::Tab(t),
            _ => return,
        };
        if let Spot::Tab(t) = spot {
            for x in UNITS {
                if x != u && self[x].spot == Spot::Tab(u) {
                    self[x].spot = Spot::Tab(t);
                }
            }
        }
        let order = match spot {
            Spot::Edge { outer, .. } => {
                let (lo, hi) = (self.0.iter().map(|d| d.order).min().unwrap_or(0), self.0.iter().map(|d| d.order).max().unwrap_or(0));
                if outer {
                    lo - 1
                } else {
                    hi + 1
                }
            }
            _ => self[u].order,
        };
        let d = &mut self[u];
        (d.spot, d.place, d.order) = (spot, Place::Docked, order);
        if drop != Drop::Tab {
            (d.extent, d.gen) = (Some(extent), d.gen + 1);
        }
    }

    /// The group `u` leaves its slot (floats or is pinned off). When it is
    /// its container's own, a group beside it (`shown` first) takes the
    /// container, keeping its size, and `u` keeps the mirrored place beside
    /// that one, so a redock rebuilds the same container (MFC's recent dock
    /// info); groups beside a split slot go to the slot's parent. Its tabs
    /// go along (the tabbed pane moves whole).
    pub fn leave(&mut self, u: Unit, shown: impl Fn(Unit) -> bool) {
        let beside: Vec<Unit> = UNITS.into_iter().filter(|&c| c != u && matches!(self[c].spot, Spot::Beside(t, _) if self.host(t) == u)).collect();
        match self[u].spot {
            Spot::Edge { edge, .. } => {
                let docked = |c: &&Unit| self[**c].place == Place::Docked;
                let Some(&taker) = beside.iter().filter(docked).find(|&&c| shown(c)).or_else(|| beside.iter().find(docked)) else { return };
                let Spot::Beside(_, side) = self[taker].spot else { return };
                let size = |d: &Dockable, vertical: bool| d.extent.unwrap_or(if vertical { d.create.x } else { d.create.y });
                let (container, share) = (size(&self[u], edge.vertical()), size(&self[taker], side.vertical()));
                for c in beside.iter().copied().filter(|&c| c != taker) {
                    if let Spot::Beside(_, s) = self[c].spot {
                        self[c].spot = Spot::Beside(taker, s);
                    }
                }
                let (spot, order) = (self[u].spot, self[u].order);
                let t = &mut self[taker];
                (t.spot, t.extent, t.gen, t.order) = (spot, Some(container), t.gen + 1, order);
                let d = &mut self[u];
                (d.spot, d.extent, d.gen) = (Spot::Beside(taker, side.opposite()), Some(share), d.gen + 1);
            }
            Spot::Beside(p, _) => {
                for c in beside {
                    if let Spot::Beside(_, s) = self[c].spot {
                        self[c].spot = Spot::Beside(p, s);
                    }
                }
            }
            Spot::Tab(_) => {}
        }
    }

    /// `CTabbedPane::FloatTab` (`0x1013a259`): tab `m` of `root`'s group
    /// floats alone at `rect`, remembering its tab place. The group's own
    /// unit pulled out hands the group — its place, size and the units
    /// beside it — to `next`, another of its tabs.
    pub fn float_tab(&mut self, m: Unit, root: Unit, next: Unit, rect: Rect) {
        if m == root && next != root {
            let r = self[root];
            for x in UNITS.into_iter().filter(|&x| x != next) {
                match self[x].spot {
                    Spot::Tab(h) if h == root => self[x].spot = Spot::Tab(next),
                    Spot::Beside(t, s) if t == root => self[x].spot = Spot::Beside(next, s),
                    _ => {}
                }
            }
            let n = &mut self[next];
            (n.spot, n.extent, n.gen, n.place, n.out, n.slide, n.order) = (r.spot, r.extent, n.gen + 1, r.place, r.out, r.slide, r.order);
            if r.place == Place::Floating {
                n.float_rect = r.float_rect;
            }
            self[root].spot = Spot::Tab(next);
        }
        let d = &mut self[m];
        (d.place, d.float_rect, d.out, d.slide) = (Place::Floating, Some(rect), false, 0.0);
    }

    /// Every unit's workspace entries ([`store`]).
    pub fn store_all(&self, v: &mut std::collections::HashMap<String, String>) {
        for u in UNITS {
            store(&format!("{u:?}"), &self[u], v);
        }
    }

    /// Read back [`Docks::store_all`] — or an L9.98 workspace's two units
    /// (`Group` → Векторная with its tabs, `Freq` → Частотная).
    pub fn restore_all(&mut self, v: &std::collections::HashMap<String, String>) {
        if v.contains_key("Workspace/Vector/Place") || !v.contains_key("Workspace/Group/Place") {
            for u in UNITS {
                restore(&format!("{u:?}"), &mut self[u], v, None);
            }
        } else {
            restore("Group", &mut self[Unit::Vector], v, Some(Unit::Frequency));
            restore("Freq", &mut self[Unit::Frequency], v, Some(Unit::Vector));
        }
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
        Spot::Beside(u, e) => format!("beside {u:?} {}", e.name()),
        Spot::Tab(u) => format!("tab {u:?}"),
    };
    v.insert(format!("Workspace/{unit}/Spot"), spot);
    match d.extent {
        Some(e) => v.insert(format!("Workspace/{unit}/RecentDockedSize"), e.to_string()),
        None => v.remove(&format!("Workspace/{unit}/RecentDockedSize")),
    };
    v.insert(format!("Workspace/{unit}/DockOrder"), d.order.to_string());
    match d.float_rect {
        Some(r) => v.insert(format!("Workspace/{unit}/RectRecentFloat"), format!("{} {} {} {}", r.left(), r.top(), r.right(), r.bottom())),
        None => v.remove(&format!("Workspace/{unit}/RectRecentFloat")),
    };
}

/// Read back what [`store`] wrote (an auto-hidden pane comes back slid in);
/// `legacy` the other unit of an L9.98 workspace's "beside <edge>" / "tab".
pub fn restore(unit: &str, d: &mut Dockable, v: &std::collections::HashMap<String, String>, legacy: Option<Unit>) {
    d.place = match v.get(&format!("Workspace/{unit}/Place")).map(String::as_str) {
        Some("floating") => Place::Floating,
        Some("autohide") => Place::AutoHide,
        _ => Place::Docked,
    };
    d.float_rect = v.get(&format!("Workspace/{unit}/RectRecentFloat")).and_then(|s| parse_rect(s));
    d.extent = v.get(&format!("Workspace/{unit}/RecentDockedSize")).and_then(|s| s.parse().ok());
    let order = v.get(&format!("Workspace/{unit}/DockOrder")).and_then(|s| s.parse().ok());
    let words: Vec<&str> = v.get(&format!("Workspace/{unit}/Spot")).map_or(Vec::new(), |s| s.split_whitespace().collect());
    match words[..] {
        ["edge", e, o] => {
            if let Some(edge) = Edge::from_name(e) {
                d.spot = Spot::Edge { edge, outer: o == "outer" };
            }
        }
        ["beside", u, e] => {
            if let (Some(u), Some(e)) = (unit_named(u), Edge::from_name(e)) {
                d.spot = Spot::Beside(u, e);
            }
        }
        ["tab", u] => {
            if let Some(u) = unit_named(u) {
                d.spot = Spot::Tab(u);
            }
        }
        ["beside", e] => {
            if let (Some(u), Some(e)) = (legacy, Edge::from_name(e)) {
                d.spot = Spot::Beside(u, e);
            }
        }
        ["tab"] => {
            if let Some(u) = legacy {
                d.spot = Spot::Tab(u);
            }
        }
        _ => {}
    }
    // Without an order (an L9.98 workspace): outer blocks first.
    d.order = order.unwrap_or(match d.spot {
        Spot::Edge { outer: true, .. } => -1,
        Spot::Edge { outer: false, .. } => 1,
        _ => 0,
    });
    if d.place == Place::Floating && d.float_rect.is_none() {
        d.place = Place::Docked;
    }
}

fn unit_named(s: &str) -> Option<Unit> {
    UNITS.into_iter().find(|u| format!("{u:?}") == s)
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
        restore("Group", &mut back, &v, None);
        assert_eq!((back.place, back.float_rect), (Place::Floating, d.float_rect));
        d.double_click(Pos2::ZERO);
        d.pin();
        store("Group", &d, &mut v);
        restore("Group", &mut back, &v, None);
        assert_eq!((back.place, back.out), (Place::AutoHide, false));
        assert_eq!(back.float_rect, d.float_rect, "the recent floating rect is kept docked too");
        restore("Freq", &mut back, &v, None);
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

        // Частотная beside the group; the group floats: Частотная takes
        // its column.
        let all = |_: Unit| true;
        let mut d = Docks::one_group();
        d[Unit::Vector].place = Place::Floating;
        d.leave(Unit::Vector, all);
        assert_eq!(d[Unit::Frequency].spot, Spot::Edge { edge: Edge::Right, outer: true });
        assert_eq!(d[Unit::Vector].spot, Spot::Beside(Unit::Frequency, Edge::Top), "a redock goes back above it");
        assert_eq!((d[Unit::Frequency].extent, d[Unit::Vector].extent), (Some(500.0), Some(200.0)), "the column keeps its width");
        assert_eq!(d.members(Unit::Vector).collect::<Vec<_>>(), [Unit::Vector, Unit::Circle, Unit::Differential], "its tabs went along");
        // Dropped into Частотная's tabs: its tabs come too.
        d.drop_on(Unit::Vector, Drop::Tab, Some(Unit::Frequency), 0.0);
        assert_eq!((d[Unit::Vector].place, d[Unit::Vector].spot), (Place::Docked, Spot::Tab(Unit::Frequency)));
        assert_eq!(d[Unit::Circle].spot, Spot::Tab(Unit::Frequency));
        assert_eq!(d.members(Unit::Frequency).count(), 4);
        d.drop_on(Unit::Frequency, Drop::Edge(Edge::Left, true), None, 400.0);
        assert_eq!((d[Unit::Frequency].spot, d[Unit::Frequency].extent, d[Unit::Frequency].gen), (Spot::Edge { edge: Edge::Left, outer: true }, Some(400.0), 2));
        assert_eq!(d.edge(Unit::Circle), Edge::Left);
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
        d.auto_hide_frame(0.7, false, false, true, false);
        assert!(d.slide == 1.0 && d.wake_in(0.7).is_some_and(|t| t > 0.4), "out, it rests until the next check");
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

    /// `FloatTab` (`0x1013a259`): a tab pulled out floats alone; the
    /// group's own unit pulled out hands the group to another tab.
    #[test]
    fn a_tab_pulled_out_floats_alone() {
        let r = Rect::from_min_size(Pos2::new(50.0, 60.0), Vec2::splat(300.0));
        let mut d = Docks::one_group();
        d.float_tab(Unit::Circle, Unit::Vector, Unit::Circle, r);
        assert_eq!((d[Unit::Circle].place, d[Unit::Circle].spot, d.host(Unit::Circle)), (Place::Floating, Spot::Tab(Unit::Vector), Unit::Circle));
        assert_eq!(d.members(Unit::Vector).collect::<Vec<_>>(), [Unit::Vector, Unit::Differential]);
        // Векторная's own tab: Дифференциальная takes the column and
        // Частотная below it.
        d.float_tab(Unit::Vector, Unit::Vector, Unit::Differential, r);
        assert_eq!(d[Unit::Differential].spot, Spot::Edge { edge: Edge::Right, outer: true });
        assert_eq!(d[Unit::Frequency].spot, Spot::Beside(Unit::Differential, Edge::Bottom));
        assert_eq!(d[Unit::Circle].spot, Spot::Tab(Unit::Differential), "the floating tab's place follows");
        assert_eq!((d[Unit::Vector].place, d[Unit::Vector].spot), (Place::Floating, Spot::Tab(Unit::Differential)));
        // Redocked (a double-click), Векторная is a tab of that group.
        d[Unit::Vector].double_click(Pos2::ZERO);
        assert_eq!(d.host(Unit::Vector), Unit::Differential);
    }

    /// Slots split beside one another; a loop of beside places is no
    /// parent; a slot's parent hidden, it is a container of its own.
    #[test]
    fn beside_slots_nest_and_collapse() {
        let mut d = Docks::one_group();
        d.drop_on(Unit::Differential, Drop::Beside(Edge::Left), Some(Unit::Frequency), 200.0);
        let all = |_: Unit| true;
        assert_eq!(d.parent(Unit::Differential, all), Some(Unit::Frequency));
        assert_eq!(d.parent(Unit::Frequency, all), Some(Unit::Vector));
        assert_eq!(d.parent(Unit::Vector, all), None);
        assert_eq!(d.parent(Unit::Frequency, |u| u != Unit::Vector), None, "the group hidden");
        // Частотная leaves: Дифференциальная goes beside the group.
        d[Unit::Frequency].place = Place::Floating;
        d.leave(Unit::Frequency, all);
        assert_eq!(d[Unit::Differential].spot, Spot::Beside(Unit::Vector, Edge::Left));
        d[Unit::Vector].spot = Spot::Beside(Unit::Differential, Edge::Top);
        assert_eq!((d.parent(Unit::Vector, all), d.parent(Unit::Differential, all)), (None, None), "a loop");
    }

    /// An L9.98 workspace (two units) reads into the four.
    #[test]
    fn a_two_unit_workspace_reads_into_four() {
        let mut v = std::collections::HashMap::new();
        for (k, x) in [("Group/Place", "docked"), ("Group/Spot", "tab"), ("Freq/Place", "docked"), ("Freq/Spot", "edge left outer")] {
            v.insert(format!("Workspace/{k}"), x.to_string());
        }
        let mut d = Docks::one_group();
        d.restore_all(&v);
        assert_eq!((d[Unit::Vector].spot, d[Unit::Frequency].spot), (Spot::Tab(Unit::Frequency), Spot::Edge { edge: Edge::Left, outer: true }));
        assert_eq!(d.members(Unit::Frequency).count(), 4);
        let mut w = std::collections::HashMap::new();
        d.store_all(&mut w);
        let mut back = Docks::one_group();
        back.restore_all(&w);
        assert_eq!(back, d);
    }

    /// The start-up docking (`0x474e10`, L9.165): Векторная right first,
    /// Круговая its tab, Дифференциальная right inside it, Частотная bottom.
    #[test]
    fn the_start_up_layout() {
        let d = Docks::default();
        let right = Spot::Edge { edge: Edge::Right, outer: true };
        assert_eq!((d[Unit::Vector].spot, d[Unit::Vector].order), (right, 0));
        assert_eq!(d[Unit::Circle].spot, Spot::Tab(Unit::Vector));
        assert_eq!((d[Unit::Differential].spot, d[Unit::Differential].order), (right, 1));
        assert_eq!((d[Unit::Frequency].spot, d[Unit::Frequency].order), (Spot::Edge { edge: Edge::Bottom, outer: true }, 2));
        assert_eq!(d[Unit::Frequency].create, Vec2::splat(200.0));
    }

    /// Edge drops (`0x1008bc77`, `AddPane` `0x1008b521`): an outer one
    /// goes before every pane block, an inner one after them; a group
    /// taking a container takes its place in the list.
    #[test]
    fn edge_blocks_in_docking_order() {
        let mut d = Docks::one_group();
        d.drop_on(Unit::Frequency, Drop::Edge(Edge::Bottom, true), None, 200.0);
        d.drop_on(Unit::Circle, Drop::Edge(Edge::Left, false), None, 300.0);
        d.drop_on(Unit::Differential, Drop::Edge(Edge::Top, true), None, 150.0);
        let mut by: Vec<Unit> = UNITS.to_vec();
        by.sort_by_key(|&u| d[u].order);
        assert_eq!(by, [Unit::Differential, Unit::Frequency, Unit::Vector, Unit::Circle], "newest outer outermost, inner innermost");
        let mut v = std::collections::HashMap::new();
        d.store_all(&mut v);
        let mut back = Docks::one_group();
        back.restore_all(&v);
        assert!(UNITS.iter().all(|&u| back[u].order == d[u].order));
    }
}
