//! Channel selection and the per-channel commands that act on it, as in the
//! original (ledger L9): clicks on the stats panel select channels
//! (`WM_LBUTTONDOWN`/`WM_RBUTTONDOWN` `0x45dc10`, per-channel `+0x100` flag) and
//! the channel context menu (MENU 561) and per-channel commands apply to the
//! selected channels (`FUN_0042f4b0`, `FUN_00430340`, `0x467370`, `0x4654f0`).

use crate::app::{ChanKey, Doc, ValueMode};
use std::collections::BTreeSet;

/// How a stats-panel row was clicked (`nFlags` of the button-down message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelClick {
    /// Left button: select only this channel.
    Plain,
    /// Left button + Ctrl: toggle this channel, keep the others.
    Ctrl,
    /// Left button + Shift: select the range anchor..this in row order.
    Shift,
    /// Right button: select this channel unless it is already selected.
    Right,
}

/// Apply a stats-panel click on row `hit`; `order` is the rows top to bottom.
pub fn select_click(
    sel: &mut BTreeSet<ChanKey>,
    anchor: &mut Option<ChanKey>,
    order: &[ChanKey],
    hit: ChanKey,
    how: SelClick,
) {
    // Without the right button or Ctrl every channel is deselected first.
    if !matches!(how, SelClick::Right | SelClick::Ctrl) {
        sel.clear();
    }
    match how {
        SelClick::Right => {
            if !sel.contains(&hit) {
                sel.clear();
                sel.insert(hit);
            }
        }
        SelClick::Shift => {
            // No anchor yet: the cleared selection stays empty.
            let Some(a) = *anchor else { return };
            let (Some(ia), Some(ih)) = (order.iter().position(|k| *k == a), order.iter().position(|k| *k == hit)) else {
                return;
            };
            for k in &order[ia.min(ih)..=ia.max(ih)] {
                sel.insert(*k);
            }
        }
        SelClick::Plain | SelClick::Ctrl => {
            if sel.remove(&hit) {
                return;
            }
            sel.insert(hit);
            *anchor = Some(hit);
        }
    }
}

/// Enable/check state of a menu item over the selection: disabled when the
/// target set is empty, checked when every member has the flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemState {
    pub enabled: bool,
    pub checked: bool,
}

fn state_of(flags: impl Iterator<Item = bool>) -> ItemState {
    let (mut n, mut all) = (0usize, true);
    for f in flags {
        n += 1;
        all &= f;
    }
    ItemState { enabled: n > 0, checked: n > 0 && all }
}

/// A per-channel flag the channel menu toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChanFlag {
    OnFreq,
    OnVector,
    OnCircle,
    Adc,
    Invert,
    RemoveDc,
}

impl Doc {
    /// Selected analog channels, in index order.
    pub fn selected_analog(&self) -> Vec<usize> {
        self.selection
            .iter()
            .filter_map(|k| match k {
                ChanKey::Analog(i) if *i < self.chans.len() => Some(*i),
                _ => None,
            })
            .collect()
    }

    /// Selected computed channels, in index order.
    pub fn selected_computed(&self) -> Vec<usize> {
        self.selection
            .iter()
            .filter_map(|k| match k {
                ChanKey::Computed(i) if *i < self.computed.len() => Some(*i),
                _ => None,
            })
            .collect()
    }

    /// `chans` plus every member of the groups they belong to.
    pub fn with_groups(&self, chans: &[usize]) -> Vec<usize> {
        let groups: BTreeSet<u32> = chans.iter().filter_map(|&i| self.chans[i].group).collect();
        let mut out: BTreeSet<usize> = chans.iter().copied().collect();
        for (i, c) in self.chans.iter().enumerate() {
            if c.group.is_some_and(|g| groups.contains(&g)) {
                out.insert(i);
            }
        }
        out.into_iter().collect()
    }

    fn flag(&self, i: usize, f: ChanFlag) -> bool {
        let c = &self.chans[i];
        match f {
            ChanFlag::OnFreq => c.on_freq,
            ChanFlag::OnVector => c.on_vector,
            ChanFlag::OnCircle => c.on_circle,
            ChanFlag::Adc => c.adc_counts,
            ChanFlag::Invert => c.invert,
            ChanFlag::RemoveDc => c.remove_dc,
        }
    }

    fn set_flag(&mut self, i: usize, f: ChanFlag, v: bool) {
        let c = &mut self.chans[i];
        match f {
            ChanFlag::OnFreq => c.on_freq = v,
            ChanFlag::OnVector => c.on_vector = v,
            ChanFlag::OnCircle => c.on_circle = v,
            ChanFlag::Adc => c.adc_counts = v,
            ChanFlag::Invert => c.invert = v,
            ChanFlag::RemoveDc => c.remove_dc = v,
        }
    }

    /// Selected computed channels a flag also applies to (the original's
    /// computed channels are analog channels with bit 13; ADC counts skip them
    /// and our computed channels carry only the vector/circle flags).
    fn computed_flag_targets(&self, f: ChanFlag) -> Vec<usize> {
        match f {
            ChanFlag::OnVector | ChanFlag::OnCircle => self.selected_computed(),
            _ => Vec::new(),
        }
    }

    pub fn flag_state(&self, f: ChanFlag) -> ItemState {
        let comp = self.computed_flag_targets(f);
        let cflag = |j: usize| match f {
            ChanFlag::OnVector => self.computed[j].on_vector,
            _ => self.computed[j].on_circle,
        };
        state_of(self.selected_analog().into_iter().map(|i| self.flag(i, f)).chain(comp.into_iter().map(cflag)))
    }

    /// Toggle-all over the selection: when every target has the flag it is
    /// cleared on all of them, otherwise it is set on all of them.
    pub fn toggle_flag_selected(&mut self, f: ChanFlag) {
        let st = self.flag_state(f);
        if !st.enabled {
            return;
        }
        let v = !st.checked;
        for i in self.selected_analog() {
            self.set_flag(i, f, v);
        }
        for j in self.computed_flag_targets(f) {
            match f {
                ChanFlag::OnVector => self.computed[j].on_vector = v,
                _ => self.computed[j].on_circle = v,
            }
        }
    }

    /// 32820..32822: the display mode of the selected channels and their groups.
    pub fn mode_state(&self, m: ValueMode) -> ItemState {
        state_of(self.with_groups(&self.selected_analog()).into_iter().map(|i| self.chans[i].mode == m))
    }

    pub fn set_mode_selected(&mut self, m: ValueMode) {
        for i in self.with_groups(&self.selected_analog()) {
            self.chans[i].mode = m;
        }
    }

    /// 32848 «Скрыть»: hide the selected channels and their groups, dissolve
    /// those groups and deselect them.
    pub fn hide_selected(&mut self) {
        for i in self.with_groups(&self.selected_analog()) {
            self.chans[i].visible = false;
            self.chans[i].group = None;
            self.selection.remove(&ChanKey::Analog(i));
        }
    }

    /// 141 «Вернуть к исходному размеру»: un-zoom the selected channels and
    /// their groups.
    pub fn reset_zoom_selected(&mut self) {
        for i in self.with_groups(&self.selected_analog()) {
            self.chans[i].vzoom = 1.0;
        }
    }

    /// Selected visible analog channels (the combine/split target set).
    fn selected_visible_analog(&self) -> Vec<usize> {
        self.selected_analog().into_iter().filter(|&i| self.chans[i].visible).collect()
    }

    /// CMDUI 33035: at least two selected visible channels, all analog, all
    /// with the same non-empty unit.
    pub fn can_combine(&self) -> bool {
        let vis: Vec<&crate::app::ChanKey> = self
            .selection
            .iter()
            .filter(|k| match k {
                ChanKey::Analog(i) => self.chans.get(*i).is_some_and(|c| c.visible),
                _ => true,
            })
            .collect();
        if vis.len() < 2 || vis.iter().any(|k| !matches!(k, ChanKey::Analog(_))) {
            return false;
        }
        let units: BTreeSet<&str> = self.selected_visible_analog().iter().map(|&i| self.osc.analog[i].unit.trim()).collect();
        units.len() == 1 && !units.iter().next().unwrap().is_empty()
    }

    /// 33035 «Совместить каналы»: one new group of the selection (and the
    /// groups it touches); members lose their vertical zoom and switch to
    /// instantaneous values.
    pub fn combine_selected(&mut self) {
        if !self.can_combine() {
            return;
        }
        let members = self.with_groups(&self.selected_visible_analog());
        let g = self.next_group;
        self.next_group += 1;
        for &i in &members {
            self.chans[i].group = Some(g);
            self.chans[i].vzoom = 1.0;
        }
        self.set_mode_selected(ValueMode::Instant);
    }

    /// CMDUI 33037: some selected visible channel is in a group.
    pub fn can_split(&self) -> bool {
        self.selected_visible_analog().iter().any(|&i| self.chans[i].group.is_some())
    }

    /// 33037 «Разъединить каналы»: dissolve every group a selected visible
    /// channel belongs to.
    pub fn split_selected(&mut self) {
        for i in self.with_groups(&self.selected_visible_analog()) {
            self.chans[i].group = None;
        }
    }

    /// The colour «Цвет канала» starts from: the first selected channel's.
    pub fn first_selected_color(&self) -> Option<eframe::egui::Color32> {
        self.selection.iter().find_map(|k| match k {
            ChanKey::Analog(i) => self.chans.get(*i).map(|c| c.color),
            ChanKey::Computed(i) => self.computed.get(*i).map(|c| c.color),
            ChanKey::Digital(_) => None,
        })
    }

    /// 33030 OK: every selected channel gets the colour.
    pub fn set_color_selected(&mut self, col: eframe::egui::Color32) {
        for k in self.selection.clone() {
            match k {
                ChanKey::Analog(i) => {
                    if let Some(c) = self.chans.get_mut(i) {
                        c.color = col;
                    }
                }
                ChanKey::Computed(i) => {
                    if let Some(c) = self.computed.get_mut(i) {
                        c.color = col;
                    }
                }
                ChanKey::Digital(_) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ChanKey::{Analog, Digital};

    const ROWS: [ChanKey; 4] = [Analog(0), Analog(1), Analog(2), Digital(0)];

    fn click(sel: &mut BTreeSet<ChanKey>, anchor: &mut Option<ChanKey>, hit: ChanKey, how: SelClick) {
        select_click(sel, anchor, &ROWS, hit, how);
    }

    #[test]
    fn plain_click_selects_only_that_row() {
        let (mut sel, mut anchor) = (BTreeSet::from([Analog(0), Analog(2)]), None);
        click(&mut sel, &mut anchor, Analog(1), SelClick::Plain);
        assert_eq!(sel, BTreeSet::from([Analog(1)]));
        assert_eq!(anchor, Some(Analog(1)));
    }

    #[test]
    fn ctrl_click_toggles_and_shift_click_selects_the_range() {
        let (mut sel, mut anchor) = (BTreeSet::new(), None);
        click(&mut sel, &mut anchor, Analog(0), SelClick::Plain);
        click(&mut sel, &mut anchor, Analog(2), SelClick::Ctrl);
        assert_eq!(sel, BTreeSet::from([Analog(0), Analog(2)]));
        click(&mut sel, &mut anchor, Analog(2), SelClick::Ctrl);
        assert_eq!(sel, BTreeSet::from([Analog(0)]));
        // Deselecting does not move the anchor (it is set only when a row
        // becomes selected), so the range starts at Analog(2).
        click(&mut sel, &mut anchor, Digital(0), SelClick::Shift);
        assert_eq!(sel, BTreeSet::from([Analog(2), Digital(0)]));
        click(&mut sel, &mut anchor, Analog(0), SelClick::Plain);
        click(&mut sel, &mut anchor, Digital(0), SelClick::Shift);
        assert_eq!(sel, BTreeSet::from(ROWS));
    }

    #[test]
    fn right_click_selects_first_but_keeps_a_selection_it_lands_in() {
        let (mut sel, mut anchor) = (BTreeSet::from([Analog(0), Analog(1)]), None);
        click(&mut sel, &mut anchor, Analog(1), SelClick::Right);
        assert_eq!(sel, BTreeSet::from([Analog(0), Analog(1)]), "inside the selection: kept");
        click(&mut sel, &mut anchor, Analog(2), SelClick::Right);
        assert_eq!(sel, BTreeSet::from([Analog(2)]), "outside: only the clicked row");
    }

    #[test]
    fn shift_click_without_anchor_selects_nothing() {
        let (mut sel, mut anchor) = (BTreeSet::from([Analog(0)]), None);
        click(&mut sel, &mut anchor, Analog(2), SelClick::Shift);
        assert!(sel.is_empty());
    }
}
