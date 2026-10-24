//! «Объединение» in the GUI (ledger L9.15, `notes/UI_BEHAVIOUR_SPEC.md`
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
}

impl ClipChan {
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
    /// 1089: a base analog channel; 1090: a pending analog channel.
    pub ch1: Option<usize>,
    pub ch2: Option<usize>,
    pub status: String,
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
    let dur_ms = (duration(&d.osc.time) * 1000.0).round().to_string();
    let mut rec = ClipRecord {
        rate: d.osc.rate_r,
        time_multiplier: 1.0,
        times_us: d.osc.time.iter().map(|t| (t * 1e6).round() as i64).collect(),
        // `0x430bc4`: the record's track, 0 past its end.
        net_frequency: (0..n).map(|i| d.osc.freq_track.get(i).copied().unwrap_or(0.0)).collect(),
        num_values: fvb_core::views::fmt_g(n as f64),
        station_id: d.osc.station.clone(),
        block_id: d.osc.device.clone(),
        title: d.name.clone(),
        hdr_file: d.hdr.clone().unwrap_or_default(),
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
                    desc2: a.phase.clone(),
                    primary_values: a.is_primary,
                    primary_mode: v.primary,
                    mul: a.a,
                    offset: a.b,
                    primary_value: a.primary,
                    secondary_value: a.secondary,
                    file_primary_value: a.primary,
                    file_secondary_value: a.secondary,
                    color: (v.color != App::palette(i)).then(|| colorref(v.color)),
                    data: a.samples.clone(),
                    ..common(&a.id)
                });
            }
            ChanKey::Digital(i) if i < d.osc.digital.len() => {
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
                Err(fvb_core::comtrade::DigitalChannel { index: 0, id: c.name.clone(), phase: c.desc2.clone(), normal: 0, samples: c.data.iter().map(|&v| v != 0.0).collect() })
            } else {
                Ok(fvb_core::comtrade::AnalogChannel {
                    index: 0,
                    id: c.name.clone(),
                    phase: c.desc2.clone(),
                    ccbm: String::new(),
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
                    primary_tok: String::new(),
                    secondary_tok: String::new(),
                    is_primary: c.primary_values,
                    samples: c.data.clone(),
                    // `0x45ad02` → `0x424cb0` (`0x425002`): net_frequency to ch+0x50.
                    own_freq: net_freq.clone(),
                })
            };
            ClipChan { pasted: Pasted { channel, times, rate: rec.rate, pad: 0.0 }, color: c.color.map_or(egui::Color32::TRANSPARENT, from_colorref), start }
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
            let name = match &p.pasted.channel {
                Ok(a) => &a.id,
                Err(d) => &d.id,
            };
            let Some(c) = self.computed.iter_mut().find(|c| c.formula == PENDING && &c.name == name) else { continue };
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

    /// The pasted channel behind computed lane `j`, if it is pending.
    pub fn pending_of(&self, j: usize) -> Option<&ClipChan> {
        let c = self.computed.get(j).filter(|c| c.formula == PENDING)?;
        self.pending.iter().find(|p| p.name() == c.name)
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
        let names: Vec<String> = self.computed.iter().filter(|c| c.formula == PENDING).map(|c| c.name.clone()).collect();
        self.pending.retain(|p| names.iter().any(|n| n == p.name()));
        if self.pending.is_empty() {
            self.merge_offset = 0.0;
        }
    }

    /// Move the channels appended after `nb` analog / `nd` digital ones to
    /// their insertion points `at_a` / `at_d` (`0x42e5c0`), shifting every
    /// stored channel index at or after them.
    pub fn move_appended(&mut self, at_a: usize, nb: usize, at_d: usize, nd: usize) {
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
        let focus = match self.copy_focus {
            CopyFocus::Diagram(p) if !self.pane_canvas.iter().any(|(q, _)| *q == p) => CopyFocus::View,
            CopyFocus::Grid if self.pane_grid.is_empty() => CopyFocus::View,
            f => f,
        };
        if let CopyFocus::Diagram(pane) = focus {
            let picture = self.diagram_picture(pane);
            self.status = if self.clip.put(None, picture.as_ref()) {
                "Диаграмма скопирована в буфер обмена.".into()
            } else {
                "Не удалось записать в буфер обмена.".into()
            };
            return;
        }
        let Some(d) = self.active_doc() else { return };
        let rec = clip_record(d);
        if rec.channels.is_empty() {
            self.status = "Не выбраны каналы для копирования.".into();
            return;
        }
        let n = rec.channels.len();
        let picture = if focus == CopyFocus::View { self.view_picture() } else { None };
        self.status = if self.clip.put(Some(&rec.to_text()), picture.as_ref()) {
            format!("Скопировано каналов для объединения: {n}")
        } else {
            "Не удалось записать в буфер обмена.".into()
        };
    }

    /// 57637 (`0x45ad02`): read the clipboard document, undo snapshot, clear
    /// the ОМП result, deselect all, add the channels as pending lanes that
    /// come in selected (duplicate names get "(2)", "(3)"… without a space),
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
        // Insertion points, taken before deselecting: the first selected
        // analog / digital channel, else the end of its block.
        let at_analog = d.selection.iter().find_map(|k| match *k {
            ChanKey::Analog(i) => Some(ChanKey::Analog(i)),
            _ => None,
        });
        let at_digital = d.selection.iter().find_map(|k| matches!(k, ChanKey::Digital(_)).then_some(*k)).unwrap_or(ChanKey::Digital(d.osc.digital.len()));
        d.selection.clear();
        for mut c in clip {
            let taken = |n: &str, d: &Doc| {
                d.osc.analog.iter().any(|a| a.id == n) || d.osc.digital.iter().any(|x| x.id == n) || d.computed.iter().any(|x| x.name == n)
            };
            let mut name = c.name().to_string();
            let mut k = 2;
            while taken(&name, d) {
                name = format!("{}({k})", c.name());
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
            if let Ok(a) = &c.pasted.channel {
                lane.unit = a.unit.clone();
            }
            lane.before = if c.pasted.channel.is_ok() { at_analog } else { Some(at_digital) };
            d.computed.push(lane);
            d.selection.insert(ChanKey::Computed(d.computed.len() - 1));
            d.pending.push(c);
        }
        d.channels_changed();
        // The record comment becomes the two-record summary (`Comments/
        // Codepage` = 1).
        let s = &d.osc.stamps;
        let own_start = format!("{}, {}", s.start_date, s.start_time);
        let own_hdr = d.hdr.clone().unwrap_or_default();
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
        let ch1 = (!d.osc.analog.is_empty()).then_some(0);
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
                let x1 = &d.flagged.analog[c1].samples;
                // Under the frequency correction both read corrected values
                // (L9.126): the pasted channel at its own rate, the base
                // frequency at its own index.
                let corr = fvb_core::freqcorr::of(&d.osc);
                let s1 = SyncChan { times: &t1, x: x1, n: n1, corr: corr.and_then(|c| c.for_chan(c1)) };
                let corr2 = corr.map(|c| fvb_core::freqcorr::FreqCorr { chan: fvb_core::freqcorr::OTHER, fs: if p.pasted.rate > 0.0 { p.pasted.rate } else { c.fs }, ..c });
                let s2 = SyncChan { times: &p.pasted.times, x: &a2.samples, n: n2, corr: corr2 };
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
        let lane_of = |p: &ClipChan| d.computed.iter().find(|c| c.formula == PENDING && c.name == p.name());
        let pasted: Vec<Pasted> = d
            .pending
            .iter()
            .map(|p| {
                let mut q = p.pasted.clone();
                if let Some(c) = lane_of(p).filter(|c| !c.pending_own.is_empty()) {
                    // The accessor's reading of a zero sample: −DC, negated.
                    let sign = if c.invert { -1.0 } else { 1.0 };
                    match &mut q.channel {
                        Ok(a) => {
                            let x0 = a.samples.first().copied().filter(|v| !v.is_nan()).unwrap_or(0.0);
                            q.pad = c.pending_own[0] - sign * x0;
                            a.samples = c.pending_own.clone();
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
        let flags: Vec<(String, bool, bool, ValueMode)> = d.pending.iter().filter_map(|p| lane_of(p).map(|c| (c.name.clone(), c.invert, c.remove_dc, c.mode))).collect();
        // Every channel is resampled as the accessor reads it (`0x426020`:
        // ADC code, minus the DC, inverted, ×k, NaN as 0), then divided by
        // k (`0x47e598`): the flags' transforms are baked into the data
        // while the flags stay set.
        let mut src = d.osc.clone();
        for (i, a) in src.analog.iter_mut().enumerate() {
            let k = crate::app::display_k(&d.osc.analog[i], d.chans[i].primary);
            let k = if k != 0.0 { k } else { 1.0 };
            a.samples = d.flagged.analog[i].samples.iter().map(|v| if v.is_nan() { 0.0 } else { v / k }).collect();
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
                let v = crate::app::flag_series(&z, &d.chans[i], d.dc_mean[i], k)[0];
                if k != 0.0 && v.is_finite() {
                    v / k
                } else {
                    0.0
                }
            })
            .collect();
        let dig_pads: Vec<bool> = (0..d.osc.digital.len()).map(|i| d.dig(i).invert).collect();
        let merged = merge::merge(&src, &pasted, d.merge_offset, &pads, &dig_pads);
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
        for i in nb..d.osc.analog.len() {
            let mut v = ChanView::new(&d.osc.analog[i], i);
            if let Some(p) = d.pending.iter().find(|p| p.name() == d.osc.analog[i].id) {
                v.color = p.color;
            }
            if let Some(&(_, inv, dc, mode)) = flags.iter().find(|f| f.0 == d.osc.analog[i].id) {
                (v.invert, v.remove_dc, v.mode) = (inv, dc, mode);
            }
            d.chans.push(v);
        }
        d.digital_visible.resize(d.osc.digital.len(), true);
        for i in nd..d.osc.digital.len() {
            if let Some(&(_, invert, remove_dc, mode)) = flags.iter().find(|f| f.0 == d.osc.digital[i].id) {
                *d.dig_mut(i) = crate::app::DigFlags { invert, remove_dc, mode };
            }
        }
        d.move_appended(at_a, nb, at_d, nd);
        d.computed.retain(|c| c.formula != PENDING);
        d.pending.clear();
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
        // at x 15 / 290 / 450, the status line at its bottom. No close
        // button: hiding the bar undoes the paste (`0x479960`).
        let r = ui.max_rect();
        // The bar's controls in the 9 pt UI font (12 px).
        for st in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut().text_styles.insert(st, egui::FontId::proportional(12.0));
        }
        // egui's combo adds its padding and arrow to the given width.
        let sp = ui.spacing().clone();
        let combo_w = |w: f32| w - 2.0 * sp.button_padding.x - sp.icon_width - sp.icon_spacing;
        let at = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(r.min + egui::vec2(x, y), egui::vec2(w, h));
        let font = egui::FontId::proportional(12.0);
        let ink = ui.visuals().text_color();
        const METHODS: [&str; 3] = ["По времени начала записи", "По авар. составляющей", "По доаварийному режиму"];
        ui.put(at(110.0, 10.0, 170.0, 21.0), |ui: &mut egui::Ui| {
            egui::ComboBox::from_id_salt("merge_method")
                .width(combo_w(170.0))
                .truncate()
                .selected_text(METHODS[bar.method as usize])
                .show_ui(ui, |ui| {
                    for (k, t) in METHODS.into_iter().enumerate() {
                        ui.selectable_value(&mut bar.method, k as u8, t);
                    }
                })
                .response
        });
        let on = bar.method != 0;
        let name1 = bar.ch1.map(|i| d.osc.analog[i].id.clone()).unwrap_or_default();
        ui.put(at(350.0, 10.0, 90.0, 21.0), |ui: &mut egui::Ui| {
            ui.add_enabled_ui(on, |ui| {
                egui::ComboBox::from_id_salt("merge_ch1").width(combo_w(90.0)).truncate().selected_text(name1).show_ui(ui, |ui| {
                    for (i, c) in d.osc.analog.iter().enumerate() {
                        ui.selectable_value(&mut bar.ch1, Some(i), &c.id);
                    }
                });
            })
            .response
        });
        let name2 = bar.ch2.and_then(|i| d.pending.get(i)).map(|p| p.name().to_string()).unwrap_or_default();
        ui.put(at(510.0, 10.0, 90.0, 21.0), |ui: &mut egui::Ui| {
            ui.add_enabled_ui(on, |ui| {
                egui::ComboBox::from_id_salt("merge_ch2").width(combo_w(90.0)).truncate().selected_text(name2).show_ui(ui, |ui| {
                    for (i, p) in d.pending.iter().enumerate().filter(|(_, p)| p.pasted.channel.is_ok()) {
                        ui.selectable_value(&mut bar.ch2, Some(i), p.name());
                    }
                });
            })
            .response
        });
        let w = r.width() - 20.0;
        sync = ui.put(at(w - 220.0, 9.0, 100.0, 23.0), egui::Button::new("Синхронизация").wrap_mode(egui::TextWrapMode::Extend)).clicked() || sync;
        join = ui.put(at(w - 110.0, 9.0, 100.0, 23.0), egui::Button::new("Объединить").wrap_mode(egui::TextWrapMode::Extend)).clicked() || join;
        let text = r.shrink2(egui::vec2(15.0, 14.0));
        for (x, t) in [(15.0, "Синхронизация"), (290.0, "Канал 1"), (450.0, "Канал 2")] {
            ui.painter().text(egui::pos2(r.left() + x, text.top()), egui::Align2::LEFT_TOP, t, font.clone(), ink);
        }
        ui.painter().text(egui::pos2(text.left(), text.bottom()), egui::Align2::LEFT_BOTTOM, &bar.status, font, ink);
        self.merge = Some(bar);
        if sync {
            self.synchronise();
        }
        if join {
            self.merge_records();
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
        let rows = crate::lanes::lane_layout(d, 93.0, false);
        let lane = |digital: bool| rows.iter().find(|r| matches!(r.key, ChanKey::Computed(j) if d.pending_of(j).is_some_and(|p| p.pasted.channel.is_err() == digital))).unwrap();
        assert_eq!(lane(true).h, crate::oscgeom::DIGITAL_H);
        assert!(lane(false).h >= 93.0);
        let Ok(a) = &d.pending_of(match lane(false).key { ChanKey::Computed(j) => j, _ => unreachable!() }).unwrap().pasted.channel else { unreachable!() };
        let own = a.samples.iter().copied().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| (l.min(v), h.max(v)));
        assert_eq!(crate::lanes::row_range(d, lane(false), false), own);
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
        assert!(d.computed.iter().any(|c| c.formula == PENDING && c.name.ends_with("(2)") && !c.name.ends_with(" (2)")), "duplicate names get (2), no space");
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
        let rows = crate::lanes::lane_layout(d, 93.0, false);
        let pos = rows.iter().position(|r| matches!(r.key, ChanKey::Computed(_))).unwrap();
        assert!(matches!(rows[pos - 1].key, ChanKey::Analog(0)) && matches!(rows[pos + 1].key, ChanKey::Analog(1)), "shown in front of channel 1");
        assert!(d.selection.iter().all(|k| matches!(k, ChanKey::Computed(_))) && d.selection.len() == 1, "the pasted lane comes in selected");
        app.merge_records();
        let d = app.active_doc().unwrap();
        assert_eq!(d.osc.analog.len(), n0 + 1);
        assert_eq!(d.osc.analog[0].id, id0);
        assert_eq!(d.osc.analog[1].id, format!("{id0}(2)"), "merged in place");
        assert_eq!(d.osc.analog[2].id, id1);
        assert_eq!(d.hviz[0].chan, Some(2), "stored indices follow");
        assert_eq!(d.chans.len(), d.osc.analog.len());
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
}
