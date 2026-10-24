//! MDI tab groups (`EnableMDITabbedGroups` with tab swap, L9.263). The
//! frame keeps MFC's `OnShowMDITabContextMenu` (`0x100bc637`), which
//! returns FALSE, so a document tab let go off its strip takes MFC's
//! default (`0x100b537d`): into the group under it (appended, activated),
//! or — over its own group, with another tab left there — into a new group
//! after it, vertical unless the groups already run horizontally. Over the
//! last 40 px of a group's right (bottom) edge the drag offers that half
//! as a new group (`0x100b5028`). An emptied group's room goes to the one
//! before it (the next, if it was first; `0x100b5848`). A tab that changes
//! group is a new tab there and takes a new colour (L9.63's pick).

use crate::app::App;
use eframe::egui::{Pos2, Rect};

/// The resize edge every group but the last carries inside its window
/// (`0x1013d5dd`): a 1 px line, then this much of the face; its child area
/// stops short of it.
pub const SPLIT: f32 = 4.0;

/// The hot zone along a group's right (bottom) edge, and the least a group
/// keeps by its edge (`+0x29b8`, `+0x29b4`: 40).
const ZONE: f32 = 40.0;

/// Where a dragged tab would go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drop {
    /// Into group `g`.
    Join(usize),
    /// A new group after group `g`, on its far half (`vertical`: the right
    /// half, else the bottom one).
    Split { g: usize, vertical: bool },
}

impl App {
    /// The number of tab groups (one with no document).
    pub fn group_count(&self) -> usize {
        self.docs.iter().map(|d| d.mdi_group + 1).max().unwrap_or(1)
    }

    /// Stamp the active document as its group's front one.
    pub fn touch_front(&mut self) {
        if let Some(d) = self.active.and_then(|a| self.docs.get_mut(a)) {
            if d.front_stamp != self.front_counter || d.front_stamp == 0 {
                self.front_counter += 1;
                d.front_stamp = self.front_counter;
            }
        }
    }

    /// The document group `g` shows: the active one if it is there, else
    /// the group's most recently active.
    pub fn group_front(&self, g: usize) -> Option<usize> {
        if let Some(a) = self.active.filter(|&a| self.docs.get(a).is_some_and(|d| d.mdi_group == g)) {
            return Some(a);
        }
        (0..self.docs.len()).filter(|&i| self.docs[i].mdi_group == g).max_by_key(|&i| self.docs[i].front_stamp)
    }

    fn group_len(&self, g: usize) -> usize {
        self.docs.iter().filter(|d| d.mdi_group == g).count()
    }

    /// Document `i` as the last tab of group `g`, in front and active.
    pub fn move_to_group(&mut self, i: usize, g: usize) {
        let mut d = self.docs.remove(i);
        d.mdi_group = g;
        d.tab_color = None;
        self.docs.push(d);
        self.active = Some(self.docs.len() - 1);
        // Indices shift (as a close does).
        self.omp_ui.bar_doc = None;
        self.setfile.sync = None;
        self.touch_front();
        self.prune_groups();
    }

    /// Document `i` alone in a new group after group `g`, which gives it
    /// the far half of its child area — the window less its edge and 4 px
    /// of borders (`0x100b56f6` with the area at `+0x2a4`; the 4 px from the
    /// VM's 1552 → 774) —; then the frame's re-layout.
    pub fn new_group_after(&mut self, i: usize, g: usize, vertical: bool) {
        if self.mdi_widths.is_empty() {
            self.mdi_len = if vertical { self.mdi_area.width() } else { self.mdi_area.height() };
            self.mdi_widths.push(self.mdi_len);
        }
        self.mdi_vertical = vertical;
        let w = self.mdi_widths[g];
        let edge = if g + 1 < self.mdi_widths.len() { SPLIT } else { 0.0 };
        let half = ((w - edge - 4.0) / 2.0).floor();
        self.mdi_widths[g] = w - half;
        self.mdi_widths.insert(g + 1, half);
        for d in self.docs.iter_mut().filter(|d| d.mdi_group > g) {
            d.mdi_group += 1;
        }
        self.move_to_group(i, g + 1);
        self.relayout_groups(self.mdi_len);
    }

    /// Drop the groups left without a document: the room goes to the group
    /// before (the first's to the next); one group left, no orientation.
    pub fn prune_groups(&mut self) {
        let n = self.mdi_widths.len().max(self.group_count());
        let used: Vec<bool> = (0..n).map(|g| self.docs.iter().any(|d| d.mdi_group == g)).collect();
        if self.mdi_widths.len() == n {
            let mut widths: Vec<f32> = Vec::new();
            let mut carry = 0.0;
            for (g, &w) in self.mdi_widths.iter().enumerate() {
                if used[g] {
                    widths.push(w + carry);
                    carry = 0.0;
                } else if let Some(last) = widths.last_mut() {
                    *last += w;
                } else {
                    carry += w;
                }
            }
            self.mdi_widths = widths;
        }
        let map: Vec<usize> = used
            .iter()
            .scan(0, |k, &u| {
                let v = *k;
                *k += usize::from(u);
                Some(v)
            })
            .collect();
        for d in &mut self.docs {
            d.mdi_group = map[d.mdi_group];
        }
        if self.group_count() < 2 {
            self.mdi_widths.clear();
            self.mdi_vertical = true;
        }
    }

    /// The client area's re-layout along the length `len` (`0x100b3a5b`,
    /// from its CalcWindowRect: on every layout of the frame): each group
    /// but the last takes its share of the groups' total in whole percent,
    /// rounded up, of `len`, truncated; the last the rest; every one at
    /// least 40 px. Nothing else is kept, so a share creeps up to a whole
    /// percent.
    pub fn relayout_groups(&mut self, len: f32) {
        let n = self.mdi_widths.len();
        if n < 2 {
            return;
        }
        let total: f32 = self.mdi_widths.iter().sum();
        let mut at = 0.0;
        for g in 0..n {
            let w = if g + 1 == n { len - at } else { (len * (100.0 * self.mdi_widths[g] / total.max(1.0)).ceil() / 100.0).floor() };
            self.mdi_widths[g] = w.max(ZONE);
            at += self.mdi_widths[g];
        }
    }

    /// Lay the groups' windows out across `area` (each but the last with
    /// its edge inside), also kept for the next frame.
    pub fn layout_groups(&mut self, area: Rect) {
        let n = self.group_count();
        self.mdi_area = area;
        let len = if self.mdi_vertical { area.width() } else { area.height() };
        if n < 2 {
            self.mdi_widths.clear();
            self.mdi_len = len;
            self.mdi_wins = vec![area];
            self.mdi_rects = vec![area];
            return;
        }
        if self.mdi_widths.len() != n {
            self.mdi_widths = vec![(len / n as f32).floor(); n];
        }
        self.relayout_groups(len);
        self.mdi_len = len;
        let mut at = 0.0;
        self.mdi_wins.clear();
        for g in 0..n {
            let w = if g + 1 == n { len - at } else { self.mdi_widths[g] };
            self.mdi_wins.push(if self.mdi_vertical {
                Rect::from_min_max(Pos2::new(area.left() + at, area.top()), Pos2::new(area.left() + at + w, area.bottom()))
            } else {
                Rect::from_min_max(Pos2::new(area.left(), area.top() + at), Pos2::new(area.right(), area.top() + at + w))
            });
            at += w;
        }
        // The group's own part stops 2 px short of its window, and the edge
        // lies over its far border and its child's frame (VM: the view's
        // sunken edge ends 6 px in, then the edge's line).
        let v = self.mdi_vertical;
        self.mdi_rects = self.mdi_wins.iter().enumerate().map(|(g, &w)| if g + 1 == n { w } else if v { w.with_max_x(w.right() - 2.0) } else { w.with_max_y(w.bottom() - 2.0) }).collect();
    }

    /// The edge after group `g` let go `d` px along the axis: only the two
    /// neighbours change, each keeping 40 px (`0x1013df2f`, `0x100b4ee8`).
    pub fn drag_split(&mut self, g: usize, d: f32) {
        if g + 1 < self.mdi_widths.len() {
            let total = self.mdi_widths[g] + self.mdi_widths[g + 1];
            let a = (self.mdi_widths[g] + d).round().clamp(ZONE, (total - ZONE).max(ZONE));
            self.mdi_widths[g] = a;
            self.mdi_widths[g + 1] = total - a;
        }
    }

    /// Where document tab `i` dragged to `at` (off its strip) would go,
    /// and the outline the drag shows for it (`0x100b5028`): the far half
    /// of the group under it within 40 px of its far edge, or the whole
    /// group whose strip it is over. `None` with no outline: the drop's
    /// default decides.
    pub fn drop_preview(&self, i: usize, at: Pos2) -> Option<(Drop, Rect)> {
        let src = self.docs[i].mdi_group;
        if self.group_count() == 1 && self.group_len(src) == 1 {
            return None;
        }
        let h = self.mdi_wins.iter().position(|r| r.contains(at))?;
        let win = self.mdi_wins[h];
        let vertical = if self.group_count() > 1 { self.mdi_vertical } else { win.right() - at.x < win.bottom() - at.y };
        let hot = if vertical { at.x >= win.right() - ZONE } else { at.y >= win.bottom() - ZONE };
        if hot && !(h == src && self.group_len(src) == 1) {
            let half = if vertical {
                Rect::from_min_max(Pos2::new(win.right() - (win.width() / 2.0).floor(), win.top()), win.max)
            } else {
                Rect::from_min_max(Pos2::new(win.left(), win.bottom() - (win.height() / 2.0).floor()), win.max)
            };
            return Some((Drop::Split { g: h, vertical }, half));
        }
        if h != src && self.mdi_strips.get(h).is_some_and(|s| s.contains(at)) {
            return Some((Drop::Join(h), win));
        }
        None
    }

    /// Document tab `i` let go at `at`, off its strip (`0x100b537d`).
    pub fn drop_doc_tab(&mut self, i: usize, at: Pos2) {
        let src = self.docs[i].mdi_group;
        let drop = match self.drop_preview(i, at) {
            Some((d, _)) => d,
            None => {
                let Some(h) = self.mdi_wins.iter().position(|r| r.contains(at)) else { return };
                if h != src {
                    Drop::Join(h)
                } else if self.group_len(src) > 1 {
                    Drop::Split { g: h, vertical: self.group_count() < 2 || self.mdi_vertical }
                } else {
                    return;
                }
            }
        };
        match drop {
            Drop::Join(h) => self.move_to_group(i, h),
            Drop::Split { g, vertical } => self.new_group_after(i, g, vertical),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Command};
    use eframe::egui::{pos2, vec2, Rect};

    fn three() -> App {
        let mut app = App::new();
        for _ in 0..3 {
            app.dispatch(Command::Demo);
        }
        app.mdi_vertical = true;
        app.layout_groups(Rect::from_min_size(pos2(0.0, 0.0), vec2(1552.0, 1000.0)));
        app
    }

    fn at(app: &App, serial: u64) -> usize {
        app.docs.iter().position(|d| d.serial == serial).unwrap()
    }

    fn groups(app: &App) -> Vec<(u64, usize)> {
        app.docs.iter().map(|d| (d.serial, d.mdi_group)).collect()
    }

    /// The VM's runs with three documents in a 1552 px area: the third tab
    /// let go over the view makes a second group (791 | 761); the second
    /// then over its own group's view a group between (403 | 403 | 746);
    /// over the other group's view instead it joins that group, last; an
    /// emptied group goes.
    #[test]
    fn tabs_make_join_and_leave_groups() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(1552.0, 1000.0));
        let mut app = three();
        app.drop_doc_tab(at(&app, 3), pos2(700.0, 700.0));
        assert_eq!(groups(&app), [(1, 0), (2, 0), (3, 1)]);
        assert_eq!(app.active, Some(at(&app, 3)));
        assert_eq!(app.docs[at(&app, 3)].tab_color, None, "a new tab, a new colour");
        app.layout_groups(area);
        assert_eq!(app.mdi_widths, [791.0, 761.0]);
        assert_eq!((app.mdi_wins[0].right(), app.mdi_wins[1].left()), (791.0, 791.0), "the first group's edge inside its window");
        let mut b = App::new();
        for _ in 0..3 {
            b.dispatch(Command::Demo);
        }
        (b.mdi_vertical, b.mdi_len) = (true, 1552.0);
        for (d, e) in b.docs.iter_mut().zip(&app.docs) {
            d.mdi_group = e.mdi_group;
        }
        b.mdi_widths.clone_from(&app.mdi_widths);
        b.layout_groups(area);
        // Over its own group's view: a group between.
        b.drop_doc_tab(at(&b, 2), pos2(300.0, 700.0));
        assert_eq!(groups(&b), [(1, 0), (3, 2), (2, 1)]);
        b.layout_groups(area);
        assert_eq!(b.mdi_widths, [403.0, 403.0, 746.0]);
        // Over the other group's view: it joins it, last.
        app.drop_doc_tab(at(&app, 2), pos2(1200.0, 700.0));
        assert_eq!(app.docs.iter().filter(|d| d.mdi_group == 1).map(|d| d.serial).collect::<Vec<_>>(), [3, 2]);
        // The first group's last tab over the second: one group again.
        app.layout_groups(area);
        app.drop_doc_tab(at(&app, 1), pos2(1200.0, 700.0));
        assert_eq!(app.group_count(), 1);
        assert!(app.mdi_widths.is_empty());
    }

    /// The hot zone: within 40 px of the right edge the far half is
    /// offered; with one group not yet oriented, near the bottom edge the
    /// bottom half (horizontal), and the groups then stack.
    #[test]
    fn the_hot_zone_offers_a_half() {
        use super::Drop;
        let mut app = three();
        let (r, _) = app.drop_preview(2, pos2(1530.0, 300.0)).expect("near the right edge");
        assert_eq!(r, Drop::Split { g: 0, vertical: true });
        let (r, half) = app.drop_preview(2, pos2(700.0, 980.0)).expect("near the bottom edge");
        assert_eq!((r, half.top()), (Drop::Split { g: 0, vertical: false }, 500.0));
        assert_eq!(app.drop_preview(2, pos2(700.0, 500.0)), None, "the middle: no outline");
        app.drop_doc_tab(2, pos2(700.0, 980.0));
        assert!(!app.mdi_vertical);
        app.layout_groups(Rect::from_min_size(pos2(0.0, 0.0), vec2(1552.0, 1000.0)));
        assert!(app.mdi_rects[1].top() > 400.0 && app.mdi_rects[1].left() == 0.0);
    }

    /// A group with one tab does not split; the front follows the active
    /// document; closing a group's last document drops the group; an edge
    /// drag keeps 40 px each side.
    #[test]
    fn fronts_closing_and_the_edge() {
        let mut app = three();
        app.drop_doc_tab(2, pos2(700.0, 700.0));
        app.layout_groups(Rect::from_min_size(pos2(0.0, 0.0), vec2(1552.0, 1000.0)));
        let lone = app.active.unwrap();
        app.drop_doc_tab(lone, pos2(1200.0, 700.0));
        assert_eq!(app.group_count(), 2, "its only tab: no new group");
        app.drag_split(0, -5000.0);
        assert_eq!(app.mdi_widths, [40.0, 1512.0]);
        app.dispatch(Command::ActivateDoc(0));
        app.touch_front();
        assert_eq!((app.group_front(0), app.group_front(1)), (Some(0), Some(lone)));
        app.dispatch(Command::ActivateDoc(1));
        app.touch_front();
        assert_eq!(app.group_front(0), Some(1));
        app.close_doc_now(lone);
        assert_eq!((app.group_count(), app.docs.len()), (1, 2));
    }
}
