//! Channel selection and the per-channel commands that act on it, as in the
//! original (ledger L9): clicks on the stats panel select channels
//! (`WM_LBUTTONDOWN`/`WM_RBUTTONDOWN` `0x45dc10`, per-channel `+0x100` flag) and
//! the channel context menu (MENU 561) and per-channel commands apply to the
//! selected channels (`FUN_0042f4b0`, `FUN_00430340`, `0x467370`, `0x4654f0`).

use crate::app::{ChanKey, Doc, ValueMode};
use std::collections::BTreeSet;

/// The file analog channels among `keys`.
fn analog_only(keys: Vec<ChanKey>) -> impl Iterator<Item = usize> {
    keys.into_iter().filter_map(|k| if let ChanKey::Analog(i) = k { Some(i) } else { None })
}

/// How a stats-panel row was clicked (`nFlags` of the button-down message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelClick {
    /// Left button: select only this channel.
    Plain,
    /// Left button + Ctrl: toggle this channel, keep the others.
    Ctrl,
    /// Left button + Shift: select the range anchor..this in row order.
    Shift,
    /// Left button + Ctrl + Shift: add that range, keeping the others
    /// (`0x45dce3` skips the deselect-all for MK_CONTROL).
    CtrlShift,
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
    if !matches!(how, SelClick::Right | SelClick::Ctrl | SelClick::CtrlShift) {
        sel.clear();
    }
    match how {
        SelClick::Right => {
            if !sel.contains(&hit) {
                sel.clear();
                sel.insert(hit);
            }
        }
        SelClick::Shift | SelClick::CtrlShift => {
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

    /// Selected digital channels, in index order.
    pub fn selected_digital(&self) -> Vec<usize> {
        self.selection
            .iter()
            .filter_map(|k| match k {
                ChanKey::Digital(i) if *i < self.osc.digital.len() => Some(*i),
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

    /// The group of a lane's channel (file analog or computed).
    pub fn group_of(&self, k: ChanKey) -> Option<u32> {
        match k {
            ChanKey::Analog(i) => self.chans.get(i)?.group,
            ChanKey::Computed(j) => self.computed.get(j)?.group,
            ChanKey::Digital(_) => None,
        }
    }

    fn set_group(&mut self, k: ChanKey, g: Option<u32>) {
        match k {
            ChanKey::Analog(i) => self.chans[i].group = g,
            ChanKey::Computed(j) => self.computed[j].group = g,
            ChanKey::Digital(_) => {}
        }
    }

    /// The analog-kind channels, file ones then computed.
    fn kind0_keys(&self) -> impl Iterator<Item = ChanKey> {
        (0..self.chans.len()).map(ChanKey::Analog).chain((0..self.computed.len()).map(ChanKey::Computed))
    }

    /// Dissolve the groups of `keys` (`0x455df0`): a horizontal визир on a
    /// member goes back to "never placed" (hidden, value 0, no channel),
    /// then the members are ungrouped.
    pub fn dissolve_groups(&mut self, keys: &[ChanKey]) {
        let members: Vec<ChanKey> = self.with_groups(keys).into_iter().filter(|&k| self.group_of(k).is_some()).collect();
        for h in &mut self.hviz {
            if h.chan.is_some_and(|c| members.contains(&ChanKey::Analog(c))) {
                *h = crate::app::HVizir::default();
            }
        }
        for k in members {
            if let Some(g) = self.group_of(k) {
                self.group_names.remove(&g);
            }
            self.set_group(k, None);
        }
    }

    /// `keys` plus every member of the groups they belong to.
    pub fn with_groups(&self, keys: &[ChanKey]) -> Vec<ChanKey> {
        let groups: BTreeSet<u32> = keys.iter().filter_map(|&k| self.group_of(k)).collect();
        let mut out: BTreeSet<ChanKey> = keys.iter().copied().collect();
        out.extend(self.kind0_keys().filter(|&k| self.group_of(k).is_some_and(|g| groups.contains(&g))));
        out.into_iter().collect()
    }

    /// The order a group's curves are drawn in (`0x45fbe0` → `0x4311e0`):
    /// the member set's, by the original names' ANSI bytes (case-sensitive),
    /// the first one drawing the zero line.
    pub fn group_paint_order(&self, members: &[ChanKey]) -> Vec<ChanKey> {
        let mut v = members.to_vec();
        v.sort_by_cached_key(|&k| self.member_name(k));
        v
    }

    /// After a formula evaluation (`0x455c70`): every group with a member
    /// name that no longer resolves (`0x42f020`: exact, else ignoring case)
    /// is dissolved whole.
    pub fn prune_groups(&mut self) {
        let names: Vec<Vec<u8>> = self.kind0_keys().map(|k| self.member_name(k)).collect();
        let resolves = |n: &Vec<u8>| names.iter().any(|m| crate::setfile::ansi_fold(m) == crate::setfile::ansi_fold(n));
        let dead: Vec<u32> = self.group_names.iter().filter(|(_, ns)| !ns.iter().all(resolves)).map(|(&g, _)| g).collect();
        for g in dead {
            let members: Vec<ChanKey> = self.kind0_keys().filter(|&k| self.group_of(k) == Some(g)).collect();
            self.group_names.remove(&g);
            self.dissolve_groups(&members);
        }
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
    /// computed channels are analog channels with bit 13; ADC counts skip
    /// them).
    fn computed_flag_targets(&self, f: ChanFlag) -> Vec<usize> {
        match f {
            ChanFlag::OnFreq | ChanFlag::OnVector | ChanFlag::OnCircle => self.selected_computed(),
            ChanFlag::Invert | ChanFlag::RemoveDc => self.selected_computed(),
            _ => Vec::new(),
        }
    }


    pub fn flag_state(&self, f: ChanFlag) -> ItemState {
        let comp = self.computed_flag_targets(f);
        let cflag = |j: usize| match f {
            ChanFlag::OnFreq => self.computed[j].on_freq,
            ChanFlag::OnVector => self.computed[j].on_vector,
            ChanFlag::Invert => self.computed[j].invert,
            ChanFlag::RemoveDc => self.computed[j].remove_dc,
            _ => self.computed[j].on_circle,
        };
        // 32823 / 32824 count selected digital channels too (`0x42fc60`).
        let dig: Vec<bool> = match f {
            ChanFlag::Invert => self.selected_digital().into_iter().map(|i| self.dig(i).invert).collect(),
            ChanFlag::RemoveDc => self.selected_digital().into_iter().map(|i| self.dig(i).remove_dc).collect(),
            _ => Vec::new(),
        };
        let mut st = state_of(self.selected_analog().into_iter().map(|i| self.flag(i, f)).chain(comp.iter().map(|&j| cflag(j))).chain(dig));
        // CMDUI 32849..32851 (`0x430650`): enabled while the record has an
        // analog channel and no selected channel is ineligible by unit
        // (vector A / V / «ном» / none, circle Ohm / W / VA / var).
        let unit_ok = |u: &str| match f {
            ChanFlag::OnVector => crate::diagram::vector_unit_ok(u),
            ChanFlag::OnCircle => crate::diagram::circle_flag_unit_ok(u),
            _ => true,
        };
        if matches!(f, ChanFlag::OnFreq | ChanFlag::OnVector | ChanFlag::OnCircle) {
            // Our computed channels carry a unit only when the generator
            // sets one (S0 = VA); an empty unit is not held against them.
            // The canonical unit, none in ADC mode (`0x414c30`).
            let bad = self.selected_analog().into_iter().any(|i| !unit_ok(if self.chans[i].adc_counts { "" } else { &self.osc.analog[i].unit }))
                || comp.iter().any(|&j| {
                    let u = self.computed[j].unit.trim();
                    !u.is_empty() && !unit_ok(u)
                });
            st.enabled = !self.chans.is_empty() && !bad;
        }
        // «Отображать в отсчетах АЦП» counts the selected formula channels
        // (analog records) for its enabling, but they never take the mode:
        // with only those selected it is enabled, unchecked, and does
        // nothing (VM).
        if f == ChanFlag::Adc && !self.selected_computed().is_empty() {
            st.enabled = true;
        }
        st
    }

    /// Toggle-all over the selection: when every target has the flag it is
    /// cleared on all of them, otherwise it is set on all of them.
    pub fn toggle_flag_selected(&mut self, f: ChanFlag) {
        let st = self.flag_state(f);
        if !st.enabled || self.selection.is_empty() {
            return;
        }
        let v = !st.checked;
        for i in self.selected_analog() {
            self.set_flag(i, f, v);
        }
        for i in self.selected_digital() {
            match f {
                ChanFlag::Invert => self.dig_mut(i).invert = v,
                ChanFlag::RemoveDc => self.dig_mut(i).remove_dc = v,
                _ => {}
            }
        }
        for j in self.computed_flag_targets(f) {
            match f {
                ChanFlag::OnFreq => self.computed[j].on_freq = v,
                ChanFlag::OnVector => self.computed[j].on_vector = v,
                ChanFlag::Invert => self.computed[j].invert = v,
                ChanFlag::RemoveDc => self.computed[j].remove_dc = v,
                _ => self.computed[j].on_circle = v,
            }
            self.reflag_computed(j);
        }
    }

    /// 32820..32822's state (`0x42fc60`): the selected channels of every
    /// kind, no group expansion — enabled with any selection, checked when
    /// every selected channel is in the mode (digital ones carry the bits —
    /// 0x80 by default, set by 32820–32822 and read by nothing else).
    pub fn mode_state(&self, m: ValueMode) -> ItemState {
        let modes = self.selected_analog()
            .into_iter()
            .map(|i| self.chans[i].mode == m)
            .chain(self.selected_computed().into_iter().map(|j| self.computed[j].mode == m))
            .chain(self.selected_digital().into_iter().map(|i| self.dig(i).mode == m));
        let st = state_of(modes);
        ItemState { enabled: !self.selection.is_empty(), checked: st.checked }
    }

    pub fn set_mode_selected(&mut self, m: ValueMode) {
        let sel: Vec<ChanKey> = self.selected_analog().into_iter().map(ChanKey::Analog).chain(self.selected_computed().into_iter().map(ChanKey::Computed)).collect();
        for k in self.with_groups(&sel) {
            match k {
                ChanKey::Analog(i) => self.chans[i].mode = m,
                ChanKey::Computed(j) => {
                    self.computed[j].mode = m;
                    self.reflag_computed(j);
                }
                ChanKey::Digital(_) => {}
            }
        }
        for i in self.selected_digital() {
            self.dig_mut(i).mode = m;
        }
    }

    /// 32848 «Скрыть»: hide the selected channels and their groups, dissolve
    /// those groups and deselect them.
    pub fn hide_selected(&mut self) {
        let sel: Vec<ChanKey> = self.selected_analog().into_iter().map(ChanKey::Analog).collect();
        let members = self.with_groups(&sel);
        self.dissolve_groups(&members);
        for k in members {
            match k {
                ChanKey::Analog(i) => self.chans[i].visible = false,
                ChanKey::Computed(j) => self.computed[j].visible = false,
                ChanKey::Digital(_) => {}
            }
            self.selection.remove(&k);
        }
    }

    /// 141 «Вернуть к исходному размеру» (`0x4654f0`): drop the window zoom
    /// of the selected analog channels and their groups.
    pub fn reset_zoom_selected(&mut self) {
        let sel: Vec<ChanKey> = self.selected_analog().into_iter().map(ChanKey::Analog).collect();
        for i in analog_only(self.with_groups(&sel)) {
            self.chans[i].window = None;
        }
    }

    /// Mode 8 release (`0x45e6b0`, `0x42afb0`): the rectangle's y-span as
    /// fractions `a < b` of the lane applied to channel `chan` and its group,
    /// each from its current range; equal ends reset their zoom.
    pub fn apply_window_zoom(&mut self, chan: usize, a: f64, b: f64, ranges: &dyn Fn(usize) -> (f64, f64)) {
        for i in analog_only(self.with_groups(&[ChanKey::Analog(chan)])) {
            if a == b {
                self.chans[i].window = None;
                continue;
            }
            let kind = match self.chans[i].mode {
                ValueMode::Instant => 0,
                ValueMode::Rms => 1,
                ValueMode::FirstHarmonic => 2,
            };
            let (lo, hi) = ranges(i);
            if let Some(w) = crate::oscgeom::window_from_fractions(lo, hi, a, b, kind) {
                self.chans[i].window = Some(w);
            }
        }
    }

    /// Selected visible analog-kind channels (file and computed; the
    /// combine / split target set).
    fn selected_visible_kind0(&self) -> Vec<ChanKey> {
        self.selection
            .iter()
            .copied()
            .filter(|&k| match k {
                ChanKey::Analog(i) => self.chans.get(i).is_some_and(|c| c.visible),
                ChanKey::Computed(j) => self.computed.get(j).is_some_and(|c| c.visible),
                ChanKey::Digital(_) => false,
            })
            .collect()
    }

    fn unit_of(&self, k: ChanKey) -> &str {
        match k {
            ChanKey::Analog(i) => self.osc.analog[i].unit.trim(),
            ChanKey::Computed(j) => self.computed[j].unit.trim(),
            ChanKey::Digital(_) => "",
        }
    }

    /// CMDUI 33035 (`0x468c80`): over the selected visible channels — a
    /// digital one, an empty unit or a pasted one refuses — at least two,
    /// all with the first one's unit; formula channels take part.
    pub fn can_combine(&self) -> bool {
        let mut unit: Option<&str> = None;
        let mut ok = false;
        for &k in &self.selection {
            let visible = match k {
                ChanKey::Analog(i) => self.chans.get(i).is_some_and(|c| c.visible),
                ChanKey::Computed(j) => self.computed.get(j).is_some_and(|c| c.visible),
                ChanKey::Digital(d) => self.digital_visible.get(d).copied().unwrap_or(true),
            };
            if !visible {
                continue;
            }
            let u = self.unit_of(k);
            if matches!(k, ChanKey::Digital(_)) || u.is_empty() || matches!(k, ChanKey::Computed(j) if self.computed[j].formula == crate::merge_ui::PENDING) {
                return false;
            }
            match unit {
                None => unit = Some(u),
                Some(f) if f == u => ok = true,
                Some(_) => return false,
            }
        }
        ok
    }

    /// A channel's rate as 33035 compares it (`0x467370`): its own rate when
    /// non-zero, else the record's integer rate (`rec+0x228`).
    fn rate_of(&self, k: ChanKey) -> f64 {
        let own = match k {
            ChanKey::Analog(i) => self.osc.analog[i].own_rate,
            ChanKey::Computed(j) => self.computed[j].own_rate,
            ChanKey::Digital(_) => 0.0,
        };
        if own != 0.0 {
            own
        } else {
            self.osc.int_rate_hz()
        }
    }

    /// The scale the lane painters apply to a channel with an own rate
    /// (`0x4295dc`, `0x463e2d`): the record's integer rate ÷ the channel's
    /// own, 1 without one. A pasted lane has its own model.
    pub fn lane_ratio(&self, k: ChanKey) -> f64 {
        let own = match k {
            ChanKey::Analog(i) => self.osc.analog.get(i).map_or(0.0, |c| c.own_rate),
            ChanKey::Computed(j) => self.computed.get(j).map_or(0.0, |c| c.own_rate),
            ChanKey::Digital(i) => self.osc.digital.get(i).map_or(0.0, |c| c.own_rate),
        };
        crate::oscgeom::lane_ratio(self.osc.int_rate_hz(), own)
    }

    /// 33035's rate test: the directly selected visible analog channels
    /// (formula channels too) have exactly the same rate.
    pub fn combine_rates_match(&self) -> bool {
        let mut rates = self.selected_visible_kind0().into_iter().map(|k| self.rate_of(k));
        rates.next().is_none_or(|first| rates.all(|r| r == first))
    }

    /// 33035 «Совместить каналы»: one new group of the selection (and the
    /// groups it touches); members lose their vertical zoom and switch to
    /// instantaneous values. `false` when the rates differ (message 239:
    /// nothing changed, no group dissolved).
    pub fn combine_selected(&mut self) -> bool {
        if !self.can_combine() {
            return true;
        }
        if !self.combine_rates_match() {
            return false;
        }
        let members = self.with_groups(&self.selected_visible_kind0());
        // The old groups are dissolved first (`0x467796`).
        self.dissolve_groups(&members);
        let g = self.next_group;
        self.next_group += 1;
        for &k in &members {
            self.set_group(k, Some(g));
            if let ChanKey::Analog(i) = k {
                self.chans[i].window = None;
            }
        }
        let names = members.iter().map(|&k| self.member_name(k)).collect();
        self.group_names.insert(g, names);
        self.set_mode_selected(ValueMode::Instant);
        true
    }

    /// CMDUI 33037: some selected visible channel is in a group.
    pub fn can_split(&self) -> bool {
        self.selected_visible_kind0().iter().any(|&k| self.group_of(k).is_some())
    }

    /// 33037 «Разъединить каналы»: dissolve every group a selected visible
    /// channel belongs to.
    pub fn split_selected(&mut self) {
        let sel = self.selected_visible_kind0();
        self.dissolve_groups(&sel);
    }

    /// The colour «Цвет канала» starts from (`0x465850`): the first selected
    /// record in vector order, a digital one too.
    pub fn first_selected_color(&self) -> Option<eframe::egui::Color32> {
        crate::chandlg::rows(self).into_iter().find(|k| self.selection.contains(k)).map(|k| self.color_of(k))
    }

    /// 33030 OK (`0x4310b0`): every selected record, digital ones too, gets
    /// the colour as its own (`ch+0x140` set).
    pub fn set_color_selected(&mut self, col: eframe::egui::Color32) {
        for k in self.selection.clone() {
            match k {
                ChanKey::Analog(i) => {
                    if let Some(c) = self.chans.get_mut(i) {
                        (c.color, c.custom_color) = (col, true);
                    }
                }
                ChanKey::Computed(i) => {
                    if let Some(c) = self.computed.get_mut(i) {
                        (c.color, c.custom_color) = (col, true);
                    }
                }
                ChanKey::Digital(i) => {
                    if i < self.osc.digital.len() {
                        self.dig_mut(i).color = Some(col);
                    }
                }
            }
        }
    }

    /// A record's colour (`0x42b520`).
    pub fn color_of(&self, k: ChanKey) -> eframe::egui::Color32 {
        match k {
            ChanKey::Analog(i) => self.chans[i].color,
            ChanKey::Computed(j) => self.computed[j].color,
            ChanKey::Digital(i) => self.dig(i).color.unwrap_or_else(|| {
                let place = crate::chandlg::rows(self).iter().position(|&r| r == k);
                place.map_or(eframe::egui::Color32::BLACK, crate::app::table_color)
            }),
        }
    }

    /// The colours without a custom one follow the records' current places
    /// (`0x42b520` reads the place `0x42b5f0` at every call): the phase /
    /// name class for an analog-kind record, else the table by place mod
    /// 10 — a pasted digital lane is kind 1, never classed.
    pub fn recolor(&mut self) {
        for (place, k) in crate::chandlg::rows(self).into_iter().enumerate() {
            match k {
                ChanKey::Analog(i) if !self.chans[i].custom_color => {
                    let a = &self.osc.analog[i];
                    self.chans[i].color = crate::app::channel_color(&a.id, &a.phase, place);
                }
                ChanKey::Computed(j) if !self.computed[j].custom_color => {
                    let color = match self.pending_of(j).map(|p| &p.pasted.channel) {
                        Some(Err(_)) => crate::app::table_color(place),
                        Some(Ok(a)) => crate::app::channel_color(&self.computed[j].name, &a.phase, place),
                        None => crate::app::channel_color(&self.computed[j].name, "", place),
                    };
                    self.computed[j].color = color;
                }
                _ => {}
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

    /// 33035 with a formula channel (`0x468c80`): it joins the group of a
    /// file channel of the same unit; the curves are drawn in the member
    /// names' byte order (`0x4311e0`); a member name that no longer
    /// resolves dissolves the whole group at the next evaluation
    /// (`0x455c70`).
    /// VM: with only formula channels selected «Отображать в отсчетах АЦП»
    /// is enabled and unchecked, and a click changes nothing.
    #[test]
    fn the_adc_item_with_only_formula_channels() {
        let mut app = crate::app::App::new();
        app.dispatch(crate::app::Command::Demo);
        let d = app.active_doc_mut().unwrap();
        d.computed.push(crate::app::Computed::new("A0".into(), "1".into(), vec![0.0; d.osc.time.len()], eframe::egui::Color32::RED));
        let j = d.computed.len() - 1;
        d.selection = BTreeSet::from([ChanKey::Computed(j)]);
        assert_eq!(d.flag_state(ChanFlag::Adc), ItemState { enabled: true, checked: false });
        let before = d.chans.iter().map(|c| c.adc_counts).collect::<Vec<_>>();
        d.toggle_flag_selected(ChanFlag::Adc);
        assert_eq!(d.chans.iter().map(|c| c.adc_counts).collect::<Vec<_>>(), before);
        assert_eq!(d.flag_state(ChanFlag::Adc), ItemState { enabled: true, checked: false });
    }

    #[test]
    fn formula_channels_join_groups() {
        let mut app = crate::app::App::new();
        app.dispatch(crate::app::Command::Demo);
        let d = app.active_doc_mut().unwrap();
        let unit = d.osc.analog[0].unit.clone();
        let mut c = crate::app::Computed::new("A0".into(), "1".into(), vec![0.0; d.osc.time.len()], eframe::egui::Color32::RED);
        c.unit = unit.clone();
        d.computed.push(c);
        let j = d.computed.len() - 1;
        d.selection = BTreeSet::from([Analog(0), ChanKey::Computed(j)]);
        assert!(d.can_combine());
        d.computed[j].unit = String::new();
        assert!(!d.can_combine(), "an empty unit refuses");
        d.computed[j].unit = unit;
        d.selection.insert(Digital(0));
        assert!(!d.can_combine(), "a visible digital one refuses");
        d.selection.remove(&Digital(0));
        d.combine_selected();
        assert!(d.group_of(Analog(0)).is_some() && d.group_of(Analog(0)) == d.group_of(ChanKey::Computed(j)));
        let rows = crate::lanes::lane_layout(d, 93.0, false);
        let row = rows.iter().find(|r| r.key == Analog(0)).unwrap();
        assert_eq!(row.members, [Analog(0), ChanKey::Computed(j)], "one lane, the stats in channel order");
        assert!(!rows.iter().any(|r| r.key == ChanKey::Computed(j)));
        // "A0" sorts before the file name when that starts with a later byte.
        let file = d.member_name(Analog(0));
        let want = if file.as_slice() < b"A0".as_slice() { [Analog(0), ChanKey::Computed(j)] } else { [ChanKey::Computed(j), Analog(0)] };
        assert_eq!(d.group_paint_order(&row.members), want);
        // Renaming the formula member: the group survives until a formula
        // evaluation, then goes whole.
        d.computed[j].name = "B0".into();
        assert!(d.group_of(Analog(0)).is_some());
        d.prune_groups();
        assert_eq!((d.group_of(Analog(0)), d.group_of(ChanKey::Computed(j))), (None, None));
        assert!(d.group_names.is_empty());
    }

    /// `0x42b520`: a colour without a custom one follows the record's place;
    /// 33030 sets custom colours on every selected record, digital too.
    #[test]
    fn colours_follow_the_place_until_set() {
        let mut app = crate::app::App::new();
        app.dispatch(crate::app::Command::Demo);
        let d = app.active_doc_mut().unwrap();
        let n = d.osc.time.len();
        for name in ["P", "Q"] {
            d.computed.push(crate::app::Computed::new(name.into(), "1".into(), vec![0.0; n], eframe::egui::Color32::RED));
        }
        d.channels_changed();
        let place = |d: &Doc, k| crate::chandlg::rows(d).iter().position(|&r| r == k).unwrap();
        let (p, q) = (d.computed.len() - 2, d.computed.len() - 1);
        assert_eq!(d.computed[q].color, crate::app::table_color(place(d, ChanKey::Computed(q))));
        assert_ne!(d.computed[p].color, d.computed[q].color, "two places, two colours");
        let dig = d.color_of(Digital(0));
        assert_eq!(dig, crate::app::table_color(place(d, Digital(0))));
        // A custom colour sticks when the places move.
        let blue = eframe::egui::Color32::from_rgb(1, 2, 3);
        d.selection = BTreeSet::from([ChanKey::Computed(q), Digital(0)]);
        assert_eq!(d.first_selected_color(), Some(d.computed[q].color), "the first in vector order");
        d.set_color_selected(blue);
        assert!(d.removing_channels(|d| { d.computed.remove(p); }));
        let q = q - 1;
        assert_eq!((d.computed[q].color, d.color_of(Digital(0))), (blue, blue));
    }

    /// 32850 ends in the refresh-all (`0x430340` → `0x475420`), whose circle
    /// sweep keeps only Ohm / VA channels: a W one ticked leaves at once.
    #[test]
    fn the_circle_flag_is_swept_by_the_refresh() {
        let mut app = crate::app::App::new();
        app.dispatch(crate::app::Command::Demo);
        for (unit, kept) in [("W", false), ("Ohm", true)] {
            let d = app.active_doc_mut().unwrap();
            d.osc.analog[0].unit = unit.into();
            d.chans[0].on_circle = false;
            d.selection = BTreeSet::from([Analog(0)]);
            app.dispatch(crate::app::Command::ToggleFlag(ChanFlag::OnCircle));
            assert_eq!(app.active_doc().unwrap().chans[0].on_circle, kept, "{unit}");
        }
    }

    #[test]
    fn ansi_case_fold() {
        let b = |t: &str| fvb_core::comtrade::encode_text(t, fvb_core::comtrade::Encoding::Cp1251);
        assert_eq!(crate::setfile::ansi_fold(&b("UaЁЖ")), b("uaёж"));
        assert_eq!(crate::setfile::ansi_fold(&b("ЄІЇҐЎЈЅЂЃЉЊЌЋЏ")), b("єіїґўјѕђѓљњќћџ"), "every 1251 pair");
    }

    /// CMDUI 32850 / 32851 (`0x430650`): a selected channel of the wrong
    /// unit disables the diagram flag.
    #[test]
    fn diagram_flags_follow_the_unit_rules() {
        let mut app = crate::app::App::new();
        app.dispatch(crate::app::Command::Demo);
        let d = app.active_doc_mut().unwrap();
        d.selection = BTreeSet::from([Analog(0)]);
        for (unit, vector, circle) in [("A", true, false), ("Hz", false, false), ("Ohm", false, true), ("var", false, true), ("", true, false)] {
            d.osc.analog[0].unit = unit.into();
            assert_eq!(d.flag_state(ChanFlag::OnVector).enabled, vector, "vector, unit {unit:?}");
            assert_eq!(d.flag_state(ChanFlag::OnCircle).enabled, circle, "circle, unit {unit:?}");
            assert!(d.flag_state(ChanFlag::OnFreq).enabled);
        }
        d.osc.analog[0].unit = "Hz".into();
        let before = d.chans[0].on_vector;
        d.toggle_flag_selected(ChanFlag::OnVector);
        assert_eq!(d.chans[0].on_vector, before, "a disabled item does nothing");
    }

    #[test]
    fn shift_click_without_anchor_selects_nothing() {
        let (mut sel, mut anchor) = (BTreeSet::from([Analog(0)]), None);
        click(&mut sel, &mut anchor, Analog(2), SelClick::Shift);
        assert!(sel.is_empty());
    }

    #[test]
    fn ctrl_shift_click_adds_the_range() {
        let (mut sel, mut anchor) = (BTreeSet::new(), None);
        click(&mut sel, &mut anchor, Analog(0), SelClick::Plain);
        click(&mut sel, &mut anchor, Digital(0), SelClick::Ctrl);
        anchor = Some(Analog(0));
        click(&mut sel, &mut anchor, Analog(2), SelClick::CtrlShift);
        assert_eq!(sel, BTreeSet::from([Analog(0), Analog(1), Analog(2), Digital(0)]), "the range added, Digital(0) kept");
    }
}
