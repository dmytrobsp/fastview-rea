//! «Объединение» in the GUI (ledger L9.15, `docs/original/specs/ui-behaviour.md`
//! §7.8, §4): 57634 copies the selected channels with their own time base,
//! 57637 pastes them as *pending* channels (bit 14) shown at the merge offset,
//! the merge bar (CMergeBar, 70 px) synchronises them by start time, fault
//! component or pre-fault mode, a pending lane can be dragged in time, and
//! «Объединить» resamples everything onto one grid.

use crate::app::{App, ChanKey, ChanView, Computed, CopyFocus, Doc, ValueMode};
use eframe::egui;
use fvb_core::clipdata::{merged_header, ClipChannel, ClipRecord, HeaderSide};
use fvb_core::merge::{self, Pasted, SyncChan, SyncMsg};

/// A channel on the merge clipboard: its data with its own times, the
/// colour, and its record's start time and network frequency.
#[derive(Debug, Clone)]
pub struct ClipChan {
    pub pasted: Pasted,
    pub color: egui::Color32,
    pub start: Option<f64>,
    /// The copied channel's display mode (`primary_mode`, `ch+0x89`): the
    /// pasted lane reads its values ×k as the channel did (VM: a copied
    /// channel with a ratio, primary values shown secondary, pastes at the
    /// shown value, not multiplied by the ratio).
    pub primary_mode: bool,
    /// Its lane's `Computed::seq` once pasted (0 on the clipboard): the
    /// link between the two, as the original's lane is the channel itself
    /// (bit 14) — not its name, which «Кодировка» or a rename may make
    /// another lane's too.
    pub lane: u64,
}

impl ClipChan {
    /// The factor from the pasted samples to the values shown.
    pub fn display_k(&self) -> f64 {
        match &self.pasted.channel {
            Ok(a) => crate::app::display_k(a, self.primary_mode),
            Err(_) => 1.0,
        }
    }

    pub fn name(&self) -> &str {
        match &self.pasted.channel {
            Ok(a) => &a.id,
            Err(d) => &d.id,
        }
    }
}

/// The merge bar's state.
#[derive(Debug, Clone, Default)]
pub struct MergeBar {
    /// 1088: 0 by start time, 1 by the fault component, 2 by the pre-fault mode.
    pub method: u8,
    /// 1089: a base analog-kind channel (file or formula, `0x479ff0`);
    /// 1090: a pending analog channel.
    pub ch1: Option<ChanKey>,
    pub ch2: Option<usize>,
    pub status: String,
    /// The close button as last drawn.
    pub close: Option<egui::Rect>,
}

/// COLORREF (0x00BBGGRR) ↔ colour.
fn colorref(c: egui::Color32) -> u32 {
    u32::from(c.r()) | u32::from(c.g()) << 8 | u32::from(c.b()) << 16
}

fn from_colorref(v: u32) -> egui::Color32 {
    egui::Color32::from_rgb(v as u8, (v >> 8) as u8, (v >> 16) as u8)
}

/// The clipboard document of the selected file channels (`0x430af0`,
/// per channel `0x427f20`).
pub fn clip_record(d: &Doc) -> ClipRecord {
    let s = &d.osc.stamps;
    let n = d.osc.time.len();
    // `duration_milli_sec` holds microseconds, `%g` (seen on the VM: a
    // record of several seconds writes its duration in exponent form).
    let dur_ms = fvb_core::views::fmt_g((duration(&d.osc.time) * 1e6).round());
    let mut rec = ClipRecord {
        rate: d.osc.rate_r,
        time_multiplier: 1.0,
        times_us: d.osc.time.iter().map(|t| (t * 1e6).round() as i64).collect(),
        // `0x430bc4`: the record's track, 0 past its end.
        net_frequency: (0..n).map(|i| d.osc.freq_track.get(i).copied().unwrap_or(0.0)).collect(),
        num_values: fvb_core::views::fmt_g(n as f64),
        station_id: d.osc.station.clone(),
        block_id: d.osc.device.clone(),
        // The document's title, its extension kept (seen on the VM).
        title: d.title(),
        hdr_file: d.hdr_text(),
        start_time: format!("{}, {}", s.start_date, s.start_time),
        channels: Vec::new(),
        valid: true,
    };
    let common = |name: &str| ClipChannel {
        name: name.to_string(),
        record_start_date: s.start_date.clone(),
        record_start_time: s.start_time.clone(),
        duration_ms: dur_ms.clone(),
        ..Default::default()
    };
    for k in &d.selection {
        match *k {
            ChanKey::Analog(i) if i < d.osc.analog.len() => {
                let (a, v) = (&d.osc.analog[i], &d.chans[i]);
                rec.channels.push(ClipChannel {
                    dimension: a.unit.clone(),
                    // `desc2` / `desc3` ← ph / ccbm (`0x427ff4`, `0x428026`).
                    desc2: a.phase.clone(),
                    desc3: a.ccbm.clone(),
                    primary_values: a.is_primary,
                    primary_mode: v.primary,
                    mul: a.a,
                    offset: a.b,
                    primary_value: a.primary,
                    secondary_value: a.secondary,
                    file_primary_value: a.primary,
                    file_secondary_value: a.secondary,
                    // `ch+0x140` / `ch+0x144` (L9.200): only a set colour.
                    color: v.custom_color.then(|| colorref(v.color)),
                    data: a.samples.clone(),
                    ..common(&a.id)
                });
            }
            // Computed channels (bit 13) are not copied: the ОМП flags.
            ChanKey::Digital(i) if i < d.osc.digital.len() && !d.dig_computed(i) => {
                let x = &d.osc.digital[i];
                rec.channels.push(ClipChannel {
                    discrete: true,
                    desc2: x.phase.clone(),
                    data: x.samples.iter().map(|&b| f64::from(u8::from(b))).collect(),
                    ..common(&x.id)
                });
            }
            _ => {}
        }
    }
    rec
}

/// The pasted channels of a clipboard document (`0x424cb0`): times
/// relative to the first, in seconds; past the `times` array (the
/// `num_values` quirk) a channel's samples continue at 1/rate (inferred).
fn clip_chans(rec: &ClipRecord, rev_year: i32) -> Vec<ClipChan> {
    let t0 = rec.times_us.first().copied().unwrap_or(0);
    let start = rec.start_time.split_once(", ").and_then(|(dt, tm)| fvb_core::comtrade::stamp_seconds(dt, tm, rev_year));
    let step = if rec.rate > 0.0 { 1.0 / rec.rate } else { 0.0 };
    // `net_frequency` as many as `num_values` says (`_wtoi`), zero-filled.
    let mut net_freq = rec.net_frequency.clone();
    net_freq.resize(fvb_core::clipdata::wtoi(&rec.num_values).max(0) as usize, 0.0);
    rec.channels
        .iter()
        .map(|c| {
            let mut times: Vec<f64> = rec.times_us.iter().map(|&t| (t - t0) as f64 / 1e6).collect();
            while times.len() < c.data.len() {
                times.push(times.last().map_or(0.0, |t| t + step));
            }
            times.truncate(c.data.len());
            let channel = if c.discrete {
                Err(fvb_core::comtrade::DigitalChannel { index: 0, id: c.name.clone(), phase: c.desc2.clone(), normal: 0, samples: c.data.iter().map(|&v| v != 0.0).collect(), own_rate: 0.0 })
            } else {
                Ok(fvb_core::comtrade::AnalogChannel {
                    index: 0,
                    id: c.name.clone(),
                    // ph / ccbm from `desc2` / `desc3` (`0x424f8b`–`0x424fe1`).
                    phase: c.desc2.clone(),
                    ccbm: c.desc3.clone(),
                    // The clip's values are already in base units.
                    unit: fvb_core::units::normalize(&c.dimension).unit,
                    raw_unit: c.dimension.clone(),
                    unit_mult: 1.0,
                    per_second: fvb_core::units::normalize(&c.dimension).per_second,
                    a: c.mul,
                    b: c.offset,
                    skew: 0.0,
                    primary: c.primary_value,
                    secondary: c.secondary_value,
                    // Written back as `%g` (VM: a pasted channel saved with
                    // short ratio tokens, unlike the record's own padded ones).
                    primary_tok: fvb_core::views::fmt_g(c.primary_value),
                    secondary_tok: fvb_core::views::fmt_g(c.secondary_value),
                    is_primary: c.primary_values,
                    samples: c.data.clone(),
                    // `0x45ad02` → `0x424cb0` (`0x425002`): net_frequency to ch+0x50.
                    own_freq: net_freq.clone(),
                    own_rate: 0.0,
                })
            };
            ClipChan { pasted: Pasted { channel, times, rate: rec.rate, pad: 0.0 }, color: c.color.map_or(egui::Color32::TRANSPARENT, from_colorref), start, primary_mode: c.primary_mode, lane: 0 }
        })
        .collect()
}

/// The record start of a document in seconds.
fn doc_start(d: &Doc) -> Option<f64> {
    let s = &d.osc.stamps;
    fvb_core::comtrade::stamp_seconds(&s.start_date, &s.start_time, d.osc.rev_year)
}

fn duration(times: &[f64]) -> f64 {
    times.last().copied().unwrap_or(0.0) - times.first().copied().unwrap_or(0.0)
}

impl Doc {
    /// Rebuild the lanes of the pending channels at the merge offset: each is
    /// shown on the document's grid (linear interpolation, NaN outside).
    pub fn refresh_pending(&mut self) {
        let t0 = self.osc.time.first().copied().unwrap_or(0.0);
        let grid: Vec<f64> = self.osc.time.iter().map(|t| t - t0 - self.merge_offset).collect();
        for p in &self.pending {
            let Some(c) = self.computed.iter_mut().find(|c| c.formula == PENDING && c.seq == p.lane) else { continue };
            let ts = &p.pasted.times;
            c.samples = grid
                .iter()
                .map(|&t| {
                    if ts.is_empty() || t < ts[0] || t > ts[ts.len() - 1] {
                        return f64::NAN;
                    }
                    let v = |k: usize| match &p.pasted.channel {
                        Ok(a) => a.samples[k],
                        Err(d) => f64::from(u8::from(d.samples[k])),
                    };
                    if ts.len() == 1 {
                        return v(0);
                    }
                    let i = ts.partition_point(|&s| s <= t).clamp(1, ts.len() - 1);
                    let w = (t - ts[i - 1]) / (ts[i] - ts[i - 1]).max(1e-12);
                    v(i - 1) + (v(i) - v(i - 1)) * w.clamp(0.0, 1.0)
                })
                .collect();
        }
    }

    /// The analog-kind records in the original's vector order (L9.188):
    /// file channels with a pasted analog lane in front of the one it was
    /// pasted before, then the computed channels in creation order with a
    /// lane pasted before a formula channel in front of it; a lane pasted
    /// with no analog-kind channel selected goes with them at the block's
    /// end (`doc+0x1d8`, `0x45ad77`–`0x45add1`).
    pub fn kind0_order(&self) -> Vec<ChanKey> {
        let n = self.osc.analog.len();
        let analog_lane = |j: usize| !self.pending_digital(j);
        let anchored_before = |j: usize, at: ChanKey| analog_lane(j) && self.computed[j].before == Some(at);
        // A lane whose anchor is gone keeps the block's end.
        let anchored = |j: usize| match self.computed[j].before {
            Some(ChanKey::Analog(i)) => i < n,
            Some(ChanKey::Computed(f)) => f < self.computed.len() && f != j && self.computed[f].formula != PENDING,
            _ => false,
        };
        // A merged channel among the formula channels (`ChanView::merged_after`).
        let merged_at = |i: usize| self.chans.get(i).and_then(|c| c.merged_after.as_ref());
        let merged = |at: &str| -> Vec<ChanKey> { (0..n).filter(|&i| merged_at(i).is_some_and(|m| m.0.as_deref() == Some(at))).map(ChanKey::Analog).collect() };
        let formula = |name: &str| self.computed.iter().any(|c| c.formula != PENDING && c.name == name);
        let mut v = Vec::new();
        for i in 0..n {
            v.extend((0..self.computed.len()).filter(|&j| anchored_before(j, ChanKey::Analog(i))).map(ChanKey::Computed));
            if self.chans.get(i).is_none_or(|c| c.merged_after.is_none()) {
                v.push(ChanKey::Analog(i));
            }
        }
        // The block's end in creation order: computed channels and lanes
        // pasted there, and channels merged from such lanes (their anchor
        // gone too).
        let mut tail: Vec<(u64, ChanKey)> = (0..self.computed.len()).filter(|&j| analog_lane(j) && !anchored(j)).map(|j| (self.computed[j].seq, ChanKey::Computed(j))).collect();
        tail.extend((0..n).filter_map(|i| merged_at(i).filter(|m| m.0.as_deref().is_none_or(|f| !formula(f))).map(|m| (m.1, ChanKey::Analog(i)))));
        tail.sort_by_key(|t| t.0);
        for (_, k) in tail {
            if let ChanKey::Computed(j) = k {
                if self.computed[j].formula != PENDING {
                    v.extend(merged(&self.computed[j].name));
                    v.extend((0..self.computed.len()).filter(|&p| anchored_before(p, k)).map(ChanKey::Computed));
                }
            }
            v.push(k);
        }
        v
    }

    /// «Канал 1»'s entries (`0x479ff0`): every analog-kind record but the
    /// pending ones, in vector order.
    pub fn base_channels(&self) -> Vec<ChanKey> {
        self.kind0_order().into_iter().filter(|&k| !matches!(k, ChanKey::Computed(j) if self.computed[j].formula == PENDING)).collect()
    }

    /// The pasted channel behind computed lane `j`, if it is pending.
    pub fn pending_of(&self, j: usize) -> Option<&ClipChan> {
        let c = self.computed.get(j).filter(|c| c.formula == PENDING)?;
        self.pending.iter().find(|p| p.lane == c.seq)
    }

    /// [`Doc::pending_of`], mutable.
    pub fn pending_of_mut(&mut self, j: usize) -> Option<&mut ClipChan> {
        let seq = self.computed.get(j).filter(|c| c.formula == PENDING)?.seq;
        self.pending.iter_mut().find(|p| p.lane == seq)
    }

    /// The time mapping of pending lane `j` against the record and its
    /// pasted channel.
    pub fn pasted_times(&self, j: usize) -> Option<(crate::oscgeom::PastedTimes<'_>, &ClipChan)> {
        let p = self.pending_of(j)?;
        let t = crate::oscgeom::PastedTimes { own: &p.pasted.times, own_rate: p.pasted.rate, rec: &self.osc.time, rec_rate: self.osc.sample_rate, offset: self.merge_offset };
        Some((t, p))
    }

    /// Is computed lane `j` a pasted digital channel (drawn as a digital
    /// lane, 20 px)?
    pub fn pending_digital(&self, j: usize) -> bool {
        self.pending_of(j).is_some_and(|p| p.pasted.channel.is_err())
    }

    /// Keep only pending channels that still have their lane (an undo of the
    /// paste removes the lanes).
    pub fn prune_pending(&mut self) {
        let lanes: Vec<u64> = self.computed.iter().filter(|c| c.formula == PENDING).map(|c| c.seq).collect();
        self.pending.retain(|p| lanes.contains(&p.lane));
        if self.pending.is_empty() {
            self.merge_offset = 0.0;
        }
    }

    /// Move the channels appended after `nb` analog / `nd` digital ones to
    /// their insertion points `at_a` / `at_d` (`0x42e5c0`), shifting every
    /// stored channel index at or after them.
    pub fn move_appended(&mut self, at_a: usize, nb: usize, at_d: usize, nd: usize) {
        self.move_appended_keys(at_a, nb, at_d, nd);
        let (ka, kd) = (self.osc.analog.len() - nb, self.osc.digital.len() - nd);
        self.osc.analog[at_a..].rotate_right(ka);
        self.chans[at_a..].rotate_right(ka);
        self.osc.digital[at_d..].rotate_right(kd);
        self.digital_visible[at_d..].rotate_right(kd);
        self.dig_flags.resize(self.osc.digital.len(), Default::default());
        self.dig_flags[at_d..].rotate_right(kd);
        let key = |k: ChanKey| match k {
            ChanKey::Analog(i) if i >= at_a => ChanKey::Analog(i + ka),
            ChanKey::Digital(i) if i >= at_d => ChanKey::Digital(i + kd),
            k => k,
        };
        let analog = |i: usize| if i >= at_a { i + ka } else { i };
        self.selection = self.selection.iter().map(|&k| key(k)).collect();
        self.sel_anchor = self.sel_anchor.map(key);
        for h in &mut self.hviz {
            h.chan = h.chan.map(analog);
        }
        for c in &mut self.comments {
            use crate::comments::CommentKind::*;
            match &mut c.kind {
                Level { chan, .. } => *chan = key(*chan),
                Area { chan_from, chan_to, .. } => (*chan_from, *chan_to) = (key(*chan_from), key(*chan_to)),
                Moment { .. } | Kept(_) => {}
            }
        }
    }

    /// The allowed merge offsets: the pasted record may slide until one
    /// period of overlap is left.
    pub fn offset_range(&self) -> (f64, f64) {
        let f = self.osc.line_freq.max(1.0);
        let base = duration(&self.osc.time);
        let pasted = self.pending.iter().map(|p| duration(&p.pasted.times)).fold(0.0, f64::max);
        (-(pasted - 1.0 / f), base - 1.0 / f)
    }

    pub fn set_merge_offset(&mut self, offset: f64) {
        let (lo, hi) = self.offset_range();
        self.merge_offset = offset.clamp(lo.min(hi), hi.max(lo));
        self.refresh_pending();
    }
}

/// The formula marker of a pending (pasted, not yet merged) lane.
pub const PENDING: &str = "объединение";

impl App {
    /// 57634 (`0x45be28` → `0x430af0`): the selected file channels, analog
    /// then discrete, with the record's times, rate and header, as the
    /// clipboard document on the system clipboard, after a picture of the
    /// view (`0x405160`). With a diagram pane focused only that diagram's
    /// picture lands (the original sets the document after its diagram copy
    /// has closed the clipboard, so the document is lost); with a property
    /// grid focused there is no picture, only the document.
    pub fn copy_channels_for_merge(&mut self) {
        if self.active_doc().is_none() {
            return;
        }
        // A pane window focused (its caption or strip button pressed): its
        // diagram's picture only, as for a press in it (VM: Copy then holds
        // a metafile and no document).
        let focus = match self.copy_focus {
            CopyFocus::Pane(u) => self.panes.front(self.docks.members(self.docks.host(u))).map_or(CopyFocus::View, CopyFocus::Diagram),
            f => f,
        };
        let focus = match focus {
            CopyFocus::Diagram(p) if !self.pane_canvas.iter().chain(&self.canvas_memo).any(|(q, _)| *q == p) => CopyFocus::View,
            f => f,
        };
        if let CopyFocus::Diagram(pane) = focus {
            let picture = self.diagram_picture(pane);
            // The original says nothing on success (no status line).
            if !self.clip.put(None, picture.as_ref()) {
                self.status = "Не удалось записать в буфер обмена.".into();
            }
            return;
        }
        let Some(d) = self.active_doc() else { return };
        let rec = clip_record(d);
        if rec.channels.is_empty() {
            self.status = "Не выбраны каналы для копирования.".into();
            return;
        }
        let picture = if matches!(focus, CopyFocus::View | CopyFocus::Other) { self.view_picture() } else { None };
        if !self.clip.put(Some(&rec.to_text()), picture.as_ref()) {
            self.status = "Не удалось записать в буфер обмена.".into();
        }
    }

    /// 57637 (`0x45ad02`): read the clipboard document, undo snapshot, clear
    /// the ОМП result, deselect all, add the channels as pending lanes that
    /// come in selected, each named with « (вст.)» after its own name
    /// (string 3024, `0x424cb0`; VM) and, while that is taken, "(2)",
    /// "(3)"… without a space (`0x45b120`),
    /// rewrite the record comment as the merged-record summary and show the
    /// merge bar.
    pub fn paste_channels_for_merge(&mut self) {
        let Some(ai) = self.active else { return };
        if crate::omp_ui::has_pending_paste(&self.docs[ai]) {
            return;
        }
        let Some(text) = self.clip.get() else { return };
        let rec = ClipRecord::parse(&text).unwrap_or_default();
        self.snapshot_undo();
        self.omp_clear(ai);
        let rev = self.docs[ai].osc.rev_year;
        let clip = clip_chans(&rec, rev);
        let d = &mut self.docs[ai];
        // Insertion points, taken before deselecting (`0x45ad77`–`0x45ae64`):
        // the first selected analog-kind record in vector order — a formula
        // channel too — / digital channel, else the end of its block.
        let at_analog = d.kind0_order().into_iter().find(|k| d.selection.contains(k));
        let at_digital = d.selection.iter().find_map(|k| matches!(k, ChanKey::Digital(_)).then_some(*k)).unwrap_or(ChanKey::Digital(d.osc.digital.len()));
        d.selection.clear();
        for mut c in clip {
            let taken = |n: &str, d: &Doc| {
                d.osc.analog.iter().any(|a| a.id == n) || d.osc.digital.iter().any(|x| x.id == n) || d.computed.iter().any(|x| x.name == n)
            };
            let base = format!("{} (вст.)", c.name());
            let mut name = base.clone();
            let mut k = 2;
            while taken(&name, d) {
                name = format!("{base}({k})");
                k += 1;
            }
            match &mut c.pasted.channel {
                Ok(a) => a.id = name.clone(),
                Err(x) => x.id = name.clone(),
            }
            let color = if c.color == egui::Color32::TRANSPARENT {
                let phase = c.pasted.channel.as_ref().map(|a| a.phase.clone()).unwrap_or_default();
                crate::app::channel_color(&name, &phase, d.osc.analog.len() + d.computed.len())
            } else {
                c.color
            };
            let mut lane = Computed::new(name, PENDING.into(), Vec::new(), color);
            // A colour the clipboard carries stays the lane's own.
            lane.custom_color = c.color != egui::Color32::TRANSPARENT;
            if let Ok(a) = &c.pasted.channel {
                lane.unit = a.unit.clone();
            }
            lane.before = if c.pasted.channel.is_ok() { at_analog } else { Some(at_digital) };
            c.lane = lane.seq;
            // Its raw name (`+0x64`) is the name as shown now, in the record's
            // code page (VM: pasted while the record shows 866, it reads back
            // in 1251 as its 866 bytes do — «(вст.)» → «(ўбв.)»); a record
            // read in UTF-8 / KOI8-R keeps it as bytes the way it reads its
            // own (`recdlg::shown_bytes_of`).
            let bytes = crate::recdlg::shown_bytes_of(&lane.name, d.read_enc, d.codepage);
            lane.name_raw = Some(crate::recdlg::added_name(&lane.name, bytes, d.read_enc, d.codepage));
            d.computed.push(lane);
            d.selection.insert(ChanKey::Computed(d.computed.len() - 1));
            d.pending.push(c);
        }
        d.channels_changed();
        // The record comment becomes the two-record summary (`Comments/
        // Codepage` = 1).
        let s = &d.osc.stamps;
        let own_start = format!("{}, {}", s.start_date, s.start_time);
        let own_hdr = d.hdr_text();
        let own = HeaderSide { title: &d.name, station: &d.osc.station, block: &d.osc.device, start: &own_start, comment: &own_hdr };
        let other = HeaderSide { title: &rec.title, station: &rec.station_id, block: &rec.block_id, start: &rec.start_time, comment: &rec.hdr_file };
        d.hdr = Some(merged_header(&own, &other));
        d.hdr_cp = crate::setfile::cp_of(1);
        d.merge_offset = 0.0;
        d.refresh_pending();
        // The new channels' caches (`0x4254b9`), flags 0x40e1.
        for j in 0..d.computed.len() {
            if d.computed[j].formula == PENDING {
                d.reflag_computed(j);
            }
        }
        // The bar's fill (`0x479ff0`): both channel combos refilled and set
        // to their first entry; the method and the status text untouched.
        let ch1 = d.base_channels().first().copied();
        let ch2 = d.pending.iter().position(|p| p.pasted.channel.is_ok());
        let kept = self.merge.take().unwrap_or_default();
        self.merge = Some(MergeBar { ch1, ch2, ..kept });
    }

    /// 1091 «Синхронизация» (`0x47bb90`) by the chosen method; the result
    /// text goes to the bar.
    fn synchronise(&mut self) {
        let Some(ai) = self.active else { return };
        let Some(bar) = self.merge.clone() else { return };
        let d = &self.docs[ai];
        let result: Result<(f64, SyncMsg), SyncMsg> = match bar.method {
            0 => {
                let p = d.pending.first();
                match (doc_start(d), p.and_then(|p| p.start)) {
                    (Some(b), Some(s)) => merge::sync_by_start(b, duration(&d.osc.time), s, p.map_or(0.0, |p| duration(&p.pasted.times))).map(|o| (o, SyncMsg::Done)),
                    _ => Err(SyncMsg::NoOverlap),
                }
            }
            m => {
                let (Some(c1), Some(c2)) = (bar.ch1, bar.ch2) else {
                    self.merge.as_mut().unwrap().status = "Выберите каналы".into();
                    return;
                };
                let Some(p) = d.pending.get(c2) else { return };
                let Ok(a2) = &p.pasted.channel else { return };
                let t0 = d.osc.time.first().copied().unwrap_or(0.0);
                let t1: Vec<f64> = d.osc.time.iter().map(|t| t - t0).collect();
                // Samples per period and the cursor in the pasted channel
                // (`0x47bd43` / `0x47c555`): a pasted channel by its own
                // rate ÷ 50, the record's by R div trunc(f).
                // R = rec+0x228 (L9.142).
                let rate = d.osc.int_rate_hz();
                let n1 = merge::period_samples(0.0, rate, d.osc.line_freq);
                let n2 = merge::period_samples(p.pasted.rate, rate, d.osc.line_freq);
                // Under the frequency correction both read corrected values
                // (L9.126): the pasted channel at its own rate, the base
                // frequency at its own index. A formula channel reads its
                // flagged values (`0x47a1b0` → `0x42edf0` by display name).
                let corr = fvb_core::freqcorr::of(&d.osc);
                let (x1, corr1) = match c1 {
                    ChanKey::Computed(j) => (&d.computed[j].samples[..], d.computed_corr(j).map(|c| fvb_core::freqcorr::FreqCorr { chan: fvb_core::freqcorr::OTHER, ..c })),
                    ChanKey::Analog(i) | ChanKey::Digital(i) => (d.shown_samples(i), corr.and_then(|c| c.for_chan(i))),
                };
                let s1 = SyncChan { times: &t1, x: x1, n: n1, corr: corr1 };
                let corr2 = corr.map(|c| fvb_core::freqcorr::FreqCorr { chan: fvb_core::freqcorr::OTHER, fs: if p.pasted.rate > 0.0 { p.pasted.rate } else { c.fs }, ..c });
                // The pasted channel through the accessor `0x426020` (no
                // bit-14 branch: its own samples, DC and invert applied —
                // the lane's `pending_own`), not the clip's raw values.
                let own = d.computed.iter().find(|c| c.formula == PENDING && c.seq == p.lane).map(|c| &c.pending_own).filter(|v| !v.is_empty());
                let s2 = SyncChan { times: &p.pasted.times, x: own.unwrap_or(&a2.samples), n: n2, corr: corr2 };
                let k1 = d.main_cursor;
                let k2 = merge::cursor_in(k1, rate, p.pasted.rate, d.merge_offset);
                if m == 1 {
                    if n1 < 2 || n2 < 2 || k2 < 0 {
                        Err(SyncMsg::CursorBefore3Periods)
                    } else {
                        merge::sync_by_fault(&s1, &s2, k1, k2 as usize)
                    }
                } else {
                    merge::sync_by_prefault(&s1, &s2, k1, k2, d.osc.line_freq)
                }
            }
        };
        let text = match result {
            Ok((offset, msg)) => {
                self.docs[ai].set_merge_offset(offset);
                msg.text()
            }
            // By start time the failure is a box (219, MB_ICONHAND,
            // `0x47d915`) and the bar's text stays.
            Err(SyncMsg::NoOverlap) if bar.method == 0 => {
                self.dlg.push_alert_icon(SyncMsg::NoOverlap.text(), crate::dialogs::MsgIcon::Error);
                return;
            }
            Err(msg) => msg.text(),
        };
        if let Some(b) = self.merge.as_mut() {
            b.status = text.into();
        }
    }

    /// 1092 «Объединить» (`0x47d990`): the record and the pending channels
    /// resampled onto one grid. The undo snapshot is taken after the
    /// resampling but before the record's rate is set (`0x47e834`), so the
    /// first Отмена only reverts the rate and a second one the paste. Визиры,
    /// comments and the view keep their sample indices; then the silent ОМП
    /// run, the modified mark and the bar hidden.
    pub(crate) fn merge_records(&mut self) {
        let Some(ai) = self.active else { return };
        let d = &mut self.docs[ai];
        if d.pending.is_empty() {
            return;
        }
        // Pending channels are read through the accessor too (`0x47e3fb`,
        // `0x47e6a9`): their flags baked in (k = 1), and kept on the merged
        // channels below — applied a second time, as in the original.
        let lane_of = |p: &ClipChan| d.computed.iter().find(|c| c.formula == PENDING && c.seq == p.lane);
        let pasted: Vec<Pasted> = d
            .pending
            .iter()
            .map(|p| {
                let mut q = p.pasted.clone();
                if let Some(c) = lane_of(p).filter(|c| !c.pending_own.is_empty()) {
                    // The accessor's reading of a zero sample: −DC, negated.
                    let sign = if c.invert { -1.0 } else { 1.0 };
                    // The lane's own values back from the shown ones (÷k).
                    let k = p.display_k();
                    let k = if k != 0.0 { k } else { 1.0 };
                    match &mut q.channel {
                        Ok(a) => {
                            let x0 = a.samples.first().copied().filter(|v| !v.is_nan()).unwrap_or(0.0);
                            q.pad = c.pending_own[0] / k - sign * x0;
                            a.samples = c.pending_own.iter().map(|v| v / k).collect();
                        }
                        Err(dch) => {
                            q.pad = f64::from(u8::from(c.invert));
                            dch.samples = c.pending_own.iter().map(|v| *v != 0.0).collect();
                        }
                    }
                }
                q
            })
            .collect();
        // Each pending channel's lane, by its link: the merged channels come
        // out in the pending order (analog ones, then digital ones), so the
        // k-th new channel of a kind is the k-th pending one of that kind —
        // names may coincide.
        struct Pend {
            flags: Option<(bool, bool, ValueMode)>,
            color: Option<(egui::Color32, bool)>,
            // The pasted channels' display modes (`primary_mode`, `ch+0x89`).
            mode: bool,
            /// Pasted at the block's end or before a formula channel.
            after: Option<(Option<String>, u64)>,
            picked: bool,
            /// The lane's name as read (its bytes, its alias flag).
            raw: Option<crate::app::RawName>,
        }
        let pend = |p: &ClipChan| {
            let j = d.computed.iter().position(|c| c.formula == PENDING && c.seq == p.lane);
            let c = j.map(|j| &d.computed[j]);
            let after = c.and_then(|c| {
                let at = match c.before {
                    Some(ChanKey::Analog(_)) => None,
                    Some(ChanKey::Computed(f)) => Some(d.computed.get(f).map(|c| c.name.clone())),
                    _ => Some(None),
                };
                at.map(|at| (at, c.seq))
            });
            Pend {
                flags: c.map(|c| (c.invert, c.remove_dc, c.mode)),
                color: c.map(|c| (c.color, c.custom_color)),
                mode: p.primary_mode,
                after,
                picked: j.is_some_and(|j| d.selection.contains(&ChanKey::Computed(j))),
                raw: c.and_then(|c| c.name_raw.clone()),
            }
        };
        let (pend_a, pend_d): (Vec<Pend>, Vec<Pend>) = {
            let (a, dg): (Vec<&ClipChan>, Vec<&ClipChan>) = d.pending.iter().partition(|p| p.pasted.channel.is_ok());
            (a.into_iter().map(pend).collect(), dg.into_iter().map(pend).collect())
        };
        // Every channel is resampled as the accessor reads it (`0x426020`:
        // ADC code, minus the DC, inverted, ×k, NaN as 0), then divided by
        // k (`0x47e598`): the flags' transforms are baked into the data
        // while the flags stay set. Ours' flagged series has the DC and
        // the invert but not ×k, so a value comes back as it is; only an
        // ADC-mode code (which the accessor does not scale) ends ÷k. (A
        // plain ÷k multiplied a channel shown secondary by its ratio —
        // VM: after a merge such a channel came out multiplied, the shown
        // value in the original.)
        let unscale = |i: usize, v: f64| {
            let k = crate::app::display_k(&d.osc.analog[i], d.chans[i].primary);
            if !v.is_finite() {
                0.0
            } else if d.chans[i].adc_counts && k != 0.0 {
                v / k
            } else {
                v
            }
        };
        let mut src = d.osc.clone();
        for (i, a) in src.analog.iter_mut().enumerate() {
            a.samples = d.flagged.analog[i].samples.iter().map(|&v| unscale(i, v)).collect();
        }
        // Digital channels are picked through the accessor too (`0x47e6a9`):
        // an inverted one is baked in, its flag stays.
        for (i, dch) in src.digital.iter_mut().enumerate() {
            if d.dig(i).invert {
                dch.samples.iter_mut().for_each(|v| *v = !*v);
            }
        }
        // The padding's zeros are read through the accessor too.
        let pads: Vec<f64> = (0..d.osc.analog.len())
            .map(|i| {
                let k = crate::app::display_k(&d.osc.analog[i], d.chans[i].primary);
                let mut z = d.osc.analog[i].clone();
                z.samples = vec![0.0];
                unscale(i, crate::app::flag_series(&z, &d.chans[i], d.dc_mean[i], k)[0])
            })
            .collect();
        let dig_pads: Vec<bool> = (0..d.osc.digital.len()).map(|i| d.dig(i).invert).collect();
        // The formula channels present count in R with their own rates
        // (`0x47db30`), else the record's.
        let formula_rates: Vec<f64> = d.computed.iter().filter(|c| c.formula != PENDING).map(|c| c.own_rate).collect();
        let merged = merge::merge_with(&src, &pasted, d.merge_offset, &pads, &dig_pads, &formula_rates);
        let old_rate = d.osc.sample_rate;
        let (nb, nd) = (d.osc.analog.len(), d.osc.digital.len());
        let anchor = |digital: bool| {
            d.computed.iter().filter(|c| c.formula == PENDING).find_map(|c| match c.before {
                Some(ChanKey::Analog(i)) if !digital => Some(i),
                Some(ChanKey::Digital(i)) if digital => Some(i),
                _ => None,
            })
        };
        let (at_a, at_d) = (anchor(false).unwrap_or(nb).min(nb), anchor(true).unwrap_or(nd).min(nd));
        d.osc = merged;
        // The formula channels present get the merged rate too (`0x47d990`).
        for c in d.computed.iter_mut().filter(|c| c.formula != PENDING) {
            c.own_rate = d.osc.sample_rate;
        }
        for (i, p) in (nb..d.osc.analog.len()).zip(&pend_a) {
            let mut v = ChanView::new(&d.osc.analog[i], i);
            if let Some((color, custom)) = p.color {
                (v.color, v.custom_color) = (color, custom);
            }
            if let Some((inv, dc, mode)) = p.flags {
                (v.invert, v.remove_dc, v.mode) = (inv, dc, mode);
            }
            v.primary = p.mode;
            // Pasted at the block's end or before a formula channel: the
            // merged channel stays there, among the formula channels.
            v.merged_after = p.after.clone();
            d.chans.push(v);
        }
        d.digital_visible.resize(d.osc.digital.len(), true);
        for (i, p) in (nd..d.osc.digital.len()).zip(&pend_d) {
            if let Some((invert, remove_dc, mode)) = p.flags {
                *d.dig_mut(i) = crate::app::DigFlags { invert, remove_dc, mode, color: None, omp: false, omp_alias: false };
            }
        }
        d.move_appended(at_a, nb, at_d, nd);
        // The merged channels keep their lanes' names as read: the bytes as
        // pasted, the alias of a rename.
        let (ka, kd) = (d.osc.analog.len() - nb, d.osc.digital.len() - nd);
        if d.name_raw.len() == 2 + d.osc.analog.len() + d.osc.digital.len() {
            let na = d.osc.analog.len();
            for (k, p) in pend_a.iter().enumerate().take(ka) {
                if let Some(r) = &p.raw {
                    d.name_raw[2 + at_a + k] = r.clone();
                }
            }
            for (k, p) in pend_d.iter().enumerate().take(kd) {
                if let Some(r) = &p.raw {
                    d.name_raw[2 + na + at_d + k] = r.clone();
                }
            }
        }
        // A selected pending lane stays selected as its merged channel (VM:
        // a pasted channel still highlighted after «Объединить»); the
        // formula channels' keys follow their new places.
        let mut kept = 0;
        let new_at: Vec<Option<usize>> = d
            .computed
            .iter()
            .map(|c| {
                (c.formula != PENDING).then(|| {
                    kept += 1;
                    kept - 1
                })
            })
            .collect();
        let remap = |k: ChanKey| match k {
            ChanKey::Computed(j) => new_at.get(j).copied().flatten().map(ChanKey::Computed),
            k => Some(k),
        };
        d.selection = d.selection.iter().filter_map(|&k| remap(k)).collect();
        d.sel_anchor = d.sel_anchor.and_then(remap);
        for (i, p) in (at_a..at_a + ka).zip(&pend_a) {
            if p.picked {
                d.selection.insert(ChanKey::Analog(i));
            }
        }
        for (i, p) in (at_d..at_d + kd).zip(&pend_d) {
            if p.picked {
                d.selection.insert(ChanKey::Digital(i));
            }
        }
        d.computed.retain(|c| c.formula != PENDING);
        d.pending.clear();
        d.recolor();
        d.channels_changed();
        d.merge_offset = 0.0;
        let mut snap = Self::snapshot_of_doc(d);
        snap.osc.sample_rate = old_rate;
        Self::push_undo(d, snap);
        let last = d.osc.time.len().saturating_sub(1);
        for k in [&mut d.main_cursor, &mut d.aux_cursor, &mut d.v3_cursor, &mut d.first] {
            *k = (*k).min(last);
        }
        d.recompute_signals();
        d.set.modified = true;
        self.run_omp_silent(ai);
        self.merge = None;
        // MessageBeep(MB_OK) when done (`0x47e910`).
        #[cfg(windows)]
        // SAFETY: a plain user32 call with a constant argument.
        unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(0);
        }
    }

    /// The merge bar (CMergeBar, 70 px): method, the two channels,
    /// «Синхронизация», «Объединить», the status text. Hiding it while
    /// channels are pending undoes the paste.
    pub fn merge_bar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(ai) = self.active else { return };
        self.docs[ai].prune_pending();
        if self.docs[ai].pending.is_empty() {
            self.merge = None;
            return;
        }
        let Some(mut bar) = self.merge.clone() else { return };
        let (mut sync, mut join) = (false, false);
        let d = &self.docs[ai];
        // CMergeBar's fixed layout (`0x479aa0`, `0x479db0`, `0x479e80`): the
        // method combo 1088 at (110, 10), 170 wide, the channel combos 1089 /
        // 1090 at (350, 10) / (510, 10), 90 wide; «Синхронизация» (3455)
        // and «Объединить» (3319), 100 × 23 at W − 220 / W − 110, y 9, W the
        // width less 20; the labels on the top line of the bar less (15, 14)
        // at x 15 / 290 / 450, the status line at its bottom. A close
        // button (message-bar mode): hiding the bar undoes the paste while
        // a pasted channel is pending (`0x479960`, 57643). The layout is in
        // the bar window's coordinates, 4 px above and below the box.
        let r = ui.max_rect().expand2(egui::vec2(0.0, 4.0));
        // The bar's controls in the 9 pt UI font (12 px). At > 100 % the
        // places stay the app's pixels; the font and the combos' height
        // (Windows' from the font) follow the system DPI.
        let size = crate::dpi::font(12.0);
        for st in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut().text_styles.insert(st, egui::FontId::proportional(size));
        }
        let at = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(r.min + egui::vec2(x, y), egui::vec2(w, if h == 21.0 { crate::dpi::s(21.0) } else { h }));
        let font = egui::FontId::proportional(size);
        let ink = ui.visuals().text_color();
        // Win32 drop-down lists (VM: the text clipped at the button, no
        // ellipsis) and push buttons.
        use crate::dialogs::{win_button, win_combo_list_in};
        const METHODS: [&str; 3] = ["По времени начала записи", "По авар. составляющей", "По доаварийному режиму"];
        let methods: Vec<String> = METHODS.iter().map(|m| m.to_string()).collect();
        if let Some(k) = win_combo_list_in(ui, at(110.0, 10.0, 170.0, 21.0), egui::Id::new("merge_method"), &methods, bar.method as usize, true, font.clone()) {
            bar.method = k as u8;
        }
        let on = bar.method != 0;
        let base = d.base_channels();
        let names1: Vec<String> = base.iter().map(|&k| crate::chandlg::row_name(d, k)).collect();
        let cur1 = bar.ch1.and_then(|c| base.iter().position(|&k| k == c)).unwrap_or(usize::MAX);
        if let Some(k) = win_combo_list_in(ui, at(350.0, 10.0, 90.0, 21.0), egui::Id::new("merge_ch1"), &names1, cur1, on, font.clone()) {
            bar.ch1 = Some(base[k]);
        }
        let analog: Vec<usize> = d.pending.iter().enumerate().filter(|(_, p)| p.pasted.channel.is_ok()).map(|(i, _)| i).collect();
        let names2: Vec<String> = analog.iter().map(|&i| d.pending[i].name().to_string()).collect();
        let cur2 = bar.ch2.and_then(|c| analog.iter().position(|&i| i == c)).unwrap_or(usize::MAX);
        if let Some(k) = win_combo_list_in(ui, at(510.0, 10.0, 90.0, 21.0), egui::Id::new("merge_ch2"), &names2, cur2, on, font.clone()) {
            bar.ch2 = Some(analog[k]);
        }
        let w = r.width() - 20.0;
        // The enabled ones take the focus when pressed (`track_copy_focus`).
        self.focus_other.extend([at(110.0, 10.0, 170.0, 21.0), at(w - 220.0, 9.0, 100.0, 23.0), at(w - 110.0, 9.0, 100.0, 23.0)]);
        if on {
            self.focus_other.extend([at(350.0, 10.0, 90.0, 21.0), at(510.0, 10.0, 90.0, 21.0)]);
        }
        sync = win_button(ui, at(w - 220.0, 9.0, 100.0, 23.0), "Синхронизация", false).clicked() || sync;
        join = win_button(ui, at(w - 110.0, 9.0, 100.0, 23.0), "Объединить", false).clicked() || join;
        let text = r.shrink2(egui::vec2(15.0, 14.0));
        // Each static cut at the control after it (VM, 150 %: «Синхрониза»
        // under the larger font).
        for (x, end, t) in [(15.0, 110.0, "Синхронизация"), (290.0, 350.0, "Канал 1"), (450.0, 510.0, "Канал 2")] {
            let clip = egui::Rect::from_min_max(egui::pos2(r.left() + x, r.top()), egui::pos2(r.left() + end, r.bottom())).intersect(ui.clip_rect());
            ui.painter().with_clip_rect(clip).text(egui::pos2(r.left() + x, text.top()), egui::Align2::LEFT_TOP, t, font.clone(), ink);
        }
        ui.painter().text(egui::pos2(text.left(), text.bottom()), egui::Align2::LEFT_BOTTOM, &bar.status, font, ink);
        let close = crate::dialogs::caption_close(ui, ui.max_rect(), ui.id().with("fv_merge_close"));
        bar.close = Some(egui::Rect::from_min_size(egui::pos2(ui.max_rect().right() - 20.0, ui.max_rect().top()), egui::vec2(17.0, 17.0)));
        self.merge = Some(bar);
        if close {
            self.merge = None;
            if self.active_doc().is_some_and(crate::omp_ui::has_pending_paste) {
                self.dispatch(crate::app::Command::Undo);
            }
            return;
        }
        self.merge_bar_buttons(sync, join);
    }

    /// The bar's «Синхронизация» (1091, `0x449930`) and «Объединить» (1092, `0x449960`): both end in the
    /// refresh-all whatever the merge did.
    pub(crate) fn merge_bar_buttons(&mut self, sync: bool, join: bool) {
        if sync {
            self.synchronise();
            self.refresh_blocks();
        }
        if join {
            self.merge_records();
            self.refresh_blocks();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Command;

    fn demo_app() -> App {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        app
    }

    /// Pending lanes keep their kind (`0x429380` dispatches on `+0x5c`): a
    /// pasted digital channel is a 20 px digital row; the analog one's
    /// range is its own samples'.
    #[test]
    fn pasted_lanes_keep_their_kind() {
        let mut app = demo_app();
        app.active_doc_mut().unwrap().selection.extend([ChanKey::Analog(0), ChanKey::Digital(0)]);
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        let rows = crate::lanes::lane_layout(d, 93.0);
        let lane = |digital: bool| rows.iter().find(|r| matches!(r.key, ChanKey::Computed(j) if d.pending_of(j).is_some_and(|p| p.pasted.channel.is_err() == digital))).unwrap();
        assert_eq!(lane(true).h, crate::oscgeom::DIGITAL_H);
        assert!(lane(false).h >= 93.0);
        let Ok(a) = &d.pending_of(match lane(false).key { ChanKey::Computed(j) => j, _ => unreachable!() }).unwrap().pasted.channel else { unreachable!() };
        let own = a.samples.iter().copied().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| (l.min(v), h.max(v)));
        assert_eq!(crate::lanes::row_range(d, lane(false)), own);
        // The analog one carries its own phasor cache (`0x490120`'s
        // source), the digital one none.
        let cj = |digital: bool| match lane(digital).key { ChanKey::Computed(j) => &d.computed[j], _ => unreachable!() };
        assert_eq!(cj(false).pending_phasors.len(), a.samples.len());
        assert!(cj(false).pending_phasors.iter().any(|z| z.norm() > 0.0));
        assert!(cj(true).pending_phasors.is_empty());
    }

    /// The paste's analog point (`0x45ad77`–`0x45add1`): a selected formula
    /// channel is one — the lane goes in front of it; with nothing
    /// selected after the formula channels.
    #[test]
    fn a_selected_formula_channel_is_the_paste_point() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.computed.push(Computed::new("F".into(), "1".into(), vec![1.0; d.osc.time.len()], egui::Color32::RED));
        d.selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        d.selection = [ChanKey::Computed(0)].into();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        let j = d.computed.iter().position(|c| c.formula == PENDING).unwrap();
        let order = d.kind0_order();
        let n = d.osc.analog.len();
        assert_eq!(order[n..], [ChanKey::Computed(j), ChanKey::Computed(0)]);
        assert_eq!(crate::chandlg::rows(d)[n..n + 2], [ChanKey::Computed(j), ChanKey::Computed(0)]);
        let lanes: Vec<ChanKey> = crate::lanes::lane_layout(d, 93.0).into_iter().map(|r| r.key).collect();
        let at = |k| lanes.iter().position(|&x| x == k).unwrap();
        assert!(at(ChanKey::Computed(j)) + 1 == at(ChanKey::Computed(0)));
    }

    /// «Коэффициенты трансформации» (`0x47ac80`, `0x47b610`): pasted
    /// analog lanes are rows, formula channels are not; a row placed after
    /// a formula channel is appended with empty cells (its vector index is
    /// past the list) and an OK with an edit reads "" for it — k = 0.
    #[test]
    fn the_coefficient_grid_lists_pasted_lanes() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.computed.push(Computed::new("F".into(), "1".into(), vec![1.0; d.osc.time.len()], egui::Color32::RED));
        d.selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.active_doc_mut().unwrap().selection.clear();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let n = d.osc.analog.len();
        let j = d.computed.iter().position(|c| c.formula == PENDING).unwrap();
        let mut g = crate::chandlg::CoefDlg::new(d);
        assert_eq!(g.keys.len(), n + 1);
        assert_eq!(g.keys[n], (ChanKey::Computed(j), n + 1));
        assert_eq!(g.cells[n], [String::new(), String::new()]);
        assert_eq!(g.cells[0], ["1".to_string(), "1".to_string()]);
        g.double_click((0, 0), false);
        g.edit.as_mut().unwrap().1 = "2".into();
        let changed = g.apply(d).unwrap();
        assert_eq!(changed, [ChanKey::Analog(0), ChanKey::Computed(j)]);
        let Ok(a) = &d.pending_of(j).unwrap().pasted.channel else { unreachable!() };
        assert_eq!((a.primary, a.secondary), (0.0, 0.0));
    }

    /// «Объединить» moves no record (`0x47d990`): a channel pasted with
    /// nothing selected stays after the formula channels once merged; one
    /// pasted before a formula channel stays in front of it.
    #[test]
    fn a_merged_channel_keeps_its_place_among_the_formula_channels() {
        for before_formula in [false, true] {
            let mut app = demo_app();
            let d = app.active_doc_mut().unwrap();
            d.computed.push(Computed::new("F".into(), "1".into(), vec![1.0; d.osc.time.len()], egui::Color32::RED));
            d.selection.insert(ChanKey::Analog(0));
            app.copy_channels_for_merge();
            let d = app.active_doc_mut().unwrap();
            d.selection = if before_formula { [ChanKey::Computed(0)].into() } else { Default::default() };
            let n0 = d.osc.analog.len();
            app.paste_channels_for_merge();
            app.merge_records();
            let d = app.active_doc().unwrap();
            assert_eq!(d.osc.analog.len(), n0 + 1);
            let order = d.kind0_order();
            let tail = if before_formula { [ChanKey::Analog(n0), ChanKey::Computed(0)] } else { [ChanKey::Computed(0), ChanKey::Analog(n0)] };
            assert_eq!(order[n0..], tail, "before a formula channel: {before_formula}");
            assert_eq!(crate::chandlg::rows(d)[n0..n0 + 2], tail);
        }
    }

    /// A pasted lane can be renamed in «Каналы» (`0x435ac0`); the paste
    /// keeps its channel and the merge takes the new name.
    #[test]
    fn a_pending_lane_can_be_renamed() {
        let mut app = demo_app();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc_mut().unwrap();
        let j = d.computed.iter().position(|c| c.formula == PENDING).unwrap();
        assert_eq!(crate::chandlg::rename(d, ChanKey::Computed(j), "Новый"), Ok(()));
        assert!(d.pending_of(j).is_some_and(|p| p.name() == "Новый"), "still linked");
        app.merge_records();
        assert!(app.active_doc().unwrap().osc.analog.iter().any(|c| c.id == "Новый"));
    }

    /// «Канал 1» (`0x479ff0`): file and formula channels, not pending ones.
    #[test]
    fn base_channel_combo_takes_formula_channels() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.computed.push(Computed::new("F".into(), "1".into(), vec![1.0; d.osc.time.len()], egui::Color32::RED));
        d.selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        let f = d.computed.iter().position(|c| c.name == "F").unwrap();
        let base = d.base_channels();
        assert_eq!(base.len(), d.osc.analog.len() + 1);
        assert_eq!(base.last(), Some(&ChanKey::Computed(f)));
        assert_eq!(app.merge.as_ref().unwrap().ch1, Some(ChanKey::Analog(0)));
    }

    #[test]
    fn paste_is_pending_then_merges_onto_one_grid() {
        let mut app = demo_app();
        let n0 = app.active_doc().unwrap().osc.analog.len();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        assert_eq!(d.pending.len(), 1);
        let id0 = d.osc.analog[0].id.clone();
        assert!(d.computed.iter().any(|c| c.formula == PENDING && c.name == format!("{id0} (вст.)")), "« (вст.)» after the name (VM)");

        assert!(crate::omp_ui::has_pending_paste(d), "ОМП and another paste are blocked");
        assert!(app.merge.is_some(), "the merge bar shows");
        // Shift by 2 ms; out-of-range offsets are clamped.
        app.active_doc_mut().unwrap().set_merge_offset(0.002);
        app.active_doc_mut().unwrap().set_merge_offset(1e9);
        let (lo, hi) = app.active_doc().unwrap().offset_range();
        assert!(app.active_doc().unwrap().merge_offset <= hi && lo <= 0.0);
        app.active_doc_mut().unwrap().set_merge_offset(0.002);
        app.merge_records();
        let d = app.active_doc().unwrap();
        assert_eq!(d.osc.analog.len(), n0 + 1, "the pasted channel became a file channel");
        assert!(d.pending.is_empty() && !crate::omp_ui::has_pending_paste(d));
        assert_eq!(d.chans.len(), d.osc.analog.len());
        assert!(d.osc.analog.iter().all(|c| c.samples.len() == d.osc.time.len()));
        assert!(app.merge.is_none(), "the bar is hidden");
    }

    /// A pasted channel is read in its copied display mode (VM: primary
    /// values with a ratio, shown secondary, paste as the source shows them); the
    /// merge writes the file values back.
    #[test]
    fn a_pasted_lane_shows_values_as_its_channel_did() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        let a = &mut d.osc.analog[0];
        (a.primary, a.secondary, a.is_primary) = (2380.0, 1.0, true);
        d.chans[0].primary = false;
        let src = d.osc.analog[0].samples.clone();
        d.selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        let j = d.computed.iter().position(|c| c.formula == PENDING).unwrap();
        let own = &d.computed[j].pending_own;
        assert!(own.iter().zip(&src).all(|(o, s)| (o - s / 2380.0).abs() < 1e-9), "shown ÷2380, secondary");
        assert!(crate::lanes::key_icons(d, ChanKey::Computed(j)).contains(&"*K"), "the ratio's icon");
        app.merge_records();
        let d = app.active_doc().unwrap();
        let merged = d.osc.analog.iter().find(|a| a.id.ends_with("(вст.)")).expect("the merged channel");
        assert!(merged.samples.iter().zip(&src).all(|(m, s)| (m - s).abs() < 1e-6 * s.abs().max(1.0)), "the file values merged");
        // The copied channel itself stays shown secondary (VM).
        let i = d.osc.analog.iter().position(|a| !a.id.ends_with("(вст.)") && a.primary == 2380.0).expect("the source");
        let a = &d.osc.analog[i];
        assert_eq!((a.is_primary, a.primary, a.secondary, d.chans[i].primary), (true, 2380.0, 1.0, false), "the source's ratio and mode kept");
        assert!(a.samples.iter().zip(&src).all(|(m, s)| (m - s).abs() < 1e-6 * s.abs().max(1.0)), "the source's file values kept: {} vs {}", a.samples[5], src[5]);
        // The merged channel, now first, keeps the copied mode: the
        // document stays secondary (VM).
        assert!(!d.primary_values(), "the document's mode is its first channel's");
    }

    /// A pending lane's flags (L9.111's residual; `0x428490` with bit 14):
    /// invert / DC over its own samples, shown before the merge; the merge
    /// reads it through the accessor (baked) and the merged channel keeps
    /// the flag — so it shows un-inverted again.
    #[test]
    fn pending_lanes_carry_their_flags_into_the_merge() {
        use crate::chansel::ChanFlag;
        let mut app = demo_app();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        let j = d.computed.iter().position(|c| c.formula == PENDING).unwrap();
        let own: Vec<f64> = d.computed[j].pending_own.clone();
        assert!(!own.is_empty(), "built at the paste");
        app.active_doc_mut().unwrap().selection = std::collections::BTreeSet::from([ChanKey::Computed(j)]);
        assert!(app.active_doc().unwrap().flag_state(ChanFlag::Invert).enabled, "a pending lane counts");
        app.dispatch(Command::ToggleFlag(ChanFlag::Invert));
        let d = app.active_doc().unwrap();
        assert!(d.computed[j].pending_own.iter().zip(&own).all(|(a, b)| (a + b).abs() < 1e-12), "negated before the merge");
        let raw0 = d.osc.analog[0].samples.clone();
        let n0 = d.osc.analog.len();
        app.merge_records();
        let d = app.active_doc().unwrap();
        assert_eq!(d.osc.analog.len(), n0 + 1);
        let m = d.chans.iter().position(|c| c.invert).expect("the merged channel keeps the flag");
        let dot: f64 = d.osc.analog[m].samples.iter().zip(&raw0).map(|(a, b)| a * b).sum();
        assert!(dot < 0.0, "the data baked inverted");
    }

    /// «Каналы» lists rows in the record's vector order (L9.188): a pasted
    /// digital channel among the digital rows, after the pair rows; it is
    /// no analog record for the diagrams' lists.
    #[test]
    fn pasted_digital_rows_sit_with_the_digital_channels() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        assert!(!d.osc.digital.is_empty(), "the demo has a digital channel");
        d.selection.insert(ChanKey::Digital(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        let j = (0..d.computed.len()).find(|&j| d.pending_digital(j)).expect("a pasted digital lane");
        let rows = crate::chandlg::dlg_rows(d);
        let pos = |r: crate::chandlg::DlgRow| rows.iter().position(|&x| x == r).unwrap();
        use crate::chandlg::DlgRow::Chan;
        assert!(pos(Chan(ChanKey::Computed(j))) > pos(Chan(ChanKey::Analog(d.osc.analog.len() - 1))), "past the analog block");
        assert!(crate::chandlg::digital_row(d, ChanKey::Computed(j)));
        assert!(!crate::chandlg::rows(d).iter().take(d.osc.analog.len()).any(|&k| k == ChanKey::Computed(j)));
    }

    /// «Синхронизация» by start time with records that do not overlap:
    /// message 219 in a box with the stop icon (`0x47d915`), the bar's text
    /// unchanged; a second box queues behind it.
    #[test]
    fn no_overlap_is_a_box_and_boxes_queue() {
        let mut app = demo_app();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        app.active_doc_mut().unwrap().pending[0].start = Some(1e12);
        let before = app.merge.as_ref().unwrap().status.clone();
        app.synchronise();
        assert_eq!(app.dlg.alerts.front(), Some(&(SyncMsg::NoOverlap.text().to_string(), crate::dialogs::MsgIcon::Error)));
        assert_eq!(app.merge.as_ref().unwrap().status, before, "the bar's text stays");
        app.dlg.push_alert("второе");
        assert_eq!(app.dlg.alerts.len(), 2, "queued, not overwritten");
    }

    /// A file that does not load: "<file> - <message>" in an MB_OK box
    /// (the posted exception, `0x475680` → `0x4756e0`).
    #[test]
    fn a_failed_open_is_reported_in_a_box() {
        let dir = std::env::temp_dir().join(format!("fv_badopen_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.cfg");
        std::fs::write(&path, "not a comtrade header").unwrap();
        let mut app = App::new();
        app.load_comtrade_path(&path);
        for _ in 0..200 {
            app.poll_loading();
            if app.loading.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
        assert!(app.docs.is_empty());
        let (msg, icon) = app.dlg.alerts.front().expect("a box");
        // "<message> - <file>" (`0x42dfe0`).
        assert!(msg.ends_with(&format!(" - {}", path.display())), "{msg}");
        assert_eq!(*icon, crate::dialogs::MsgIcon::Exclamation);
    }

    #[test]
    fn merge_is_undone_in_two_steps_and_history_is_per_document() {
        let mut app = demo_app();
        let n0 = app.active_doc().unwrap().osc.analog.len();
        let rate0 = app.active_doc().unwrap().osc.sample_rate;
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        app.active_doc_mut().unwrap().main_cursor = 123;
        app.merge_records();
        let d = app.active_doc().unwrap();
        assert!(d.set.modified, "«Объединить» marks the document modified");
        assert_eq!(d.main_cursor, 123, "визиры keep their sample index");
        assert_eq!(d.undo.len(), 2);
        assert_eq!(d.undo[1].osc.sample_rate, rate0, "the snapshot carries the old rate");
        assert_eq!(d.undo[1].osc.analog.len(), n0 + 1, "… and the resampled channels");
        // A comment made after the paste survives every undo.
        app.active_doc_mut().unwrap().comments.push(crate::comments::Comment::new(crate::comments::CommentKind::Moment { pos: 5 }));
        app.undo();
        let d = app.active_doc().unwrap();
        assert_eq!((d.osc.analog.len(), d.chans.len()), (n0 + 1, n0 + 1), "first Отмена: still merged");
        app.undo();
        let d = app.active_doc().unwrap();
        assert_eq!((d.osc.analog.len(), d.chans.len()), (n0, n0), "second Отмена: before the paste");
        assert!(d.pending.is_empty() && d.undo.is_empty() && d.redo.len() == 2);
        assert_eq!(d.comments.len(), 1, "comments are not part of the record");
        // Another document has its own (empty) history.
        app.dispatch(Command::Demo);
        assert!(app.active_doc().unwrap().redo.is_empty());
        app.undo();
        app.dispatch(Command::ActivateDoc(0));
        app.redo();
        app.redo();
        let d = app.active_doc().unwrap();
        assert_eq!(d.osc.analog.len(), n0 + 1);
        assert!(d.redo.is_empty() && d.undo.len() == 2);
    }

    #[test]
    fn pasted_channels_go_before_the_first_selected_channel() {
        let mut app = demo_app();
        let n0 = app.active_doc().unwrap().osc.analog.len();
        let id0 = app.active_doc().unwrap().osc.analog[0].id.clone();
        let id1 = app.active_doc().unwrap().osc.analog[1].id.clone();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        // Select channel 1 of the target: the paste goes in front of it.
        let d = app.active_doc_mut().unwrap();
        d.selection = [ChanKey::Analog(1)].into();
        d.hviz[0].chan = Some(1);
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        let rows = crate::lanes::lane_layout(d, 93.0);
        let pos = rows.iter().position(|r| matches!(r.key, ChanKey::Computed(_))).unwrap();
        assert!(matches!(rows[pos - 1].key, ChanKey::Analog(0)) && matches!(rows[pos + 1].key, ChanKey::Analog(1)), "shown in front of channel 1");
        assert!(d.selection.iter().all(|k| matches!(k, ChanKey::Computed(_))) && d.selection.len() == 1, "the pasted lane comes in selected");
        app.merge_records();
        let d = app.active_doc().unwrap();
        assert_eq!(d.osc.analog.len(), n0 + 1);
        assert_eq!(d.osc.analog[0].id, id0);
        assert_eq!(d.osc.analog[1].id, format!("{id0} (вст.)"), "merged in place");
        let m = &d.osc.analog[1];
        assert_eq!((m.primary_tok.clone(), m.secondary_tok.clone()), (fvb_core::views::fmt_g(m.primary), fvb_core::views::fmt_g(m.secondary)), "the ratios written %g");
        assert_eq!(d.osc.analog[2].id, id1);
        assert_eq!(d.hviz[0].chan, Some(2), "stored indices follow");
        assert_eq!(d.chans.len(), d.osc.analog.len());
        // Pasted again: « (вст.)» is taken now, so "(2)" without a space.
        app.active_doc_mut().unwrap().selection = [ChanKey::Analog(0)].into();
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        assert!(d.computed.iter().any(|c| c.name == format!("{id0} (вст.)(2)")), "{:?}", d.computed.iter().map(|c| &c.name).collect::<Vec<_>>());
    }

    #[test]
    fn closing_the_bar_with_pending_channels_undoes_the_paste() {
        let mut app = demo_app();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        app.undo();
        let d = app.active_doc_mut().unwrap();
        d.prune_pending();
        assert!(d.pending.is_empty());
        assert!(!crate::omp_ui::has_pending_paste(d));
    }

    /// #23: the merge bar's buttons 1091 / 1092 (`0x449930`, `0x449960`) end in the refresh-all.
    #[test]
    fn the_merge_bars_buttons_end_in_the_refresh_all() {
        for (sync, join) in [(true, false), (false, true)] {
            let mut app = demo_app();
            crate::app::stale_flags::make(&mut app);
            assert!(crate::app::stale_flags::is_stale(&app));
            app.merge_bar_buttons(sync, join);
            assert!(crate::app::stale_flags::swept(&app), "sync {sync}, join {join}");
        }
    }
}
